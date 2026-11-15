// SPDX-License-Identifier: MIT
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';
import Clutter from 'gi://Clutter';
import Meta from 'gi://Meta';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const NAME = 'io.github.AK6.FeatherClip.Gnome';
const PATH = '/io/github/AK6/FeatherClip/Gnome';
const XML = `<node><interface name="${NAME}">
  <method name="Revision"><arg type="u" direction="out"/></method>
  <method name="ReadContent"><arg type="s" direction="out"/><arg type="ay" direction="out"/></method>
  <method name="WriteContent"><arg type="s" direction="in"/><arg type="ay" direction="in"/></method>
  <method name="ReadText"><arg type="s" direction="out"/></method>
  <method name="WriteText"><arg type="s" direction="in"/></method>
  <method name="RememberTarget"><arg type="u" direction="in"/><arg type="b" direction="out"/></method>
  <method name="FocusPopup"><arg type="u" direction="in"/><arg type="b" direction="out"/></method>
  <method name="Paste"><arg type="b" direction="out"/></method>
</interface></node>`;

export default class FeatherClipBridge extends Extension {
    enable() {
        this._clipboard = St.Clipboard.get_default();
        this._revision = 0;
        this._selection = global.display.get_selection();
        this._changed = this._selection.connect('owner-changed', (_selection, type) => {
            if (type === Meta.SelectionType.CLIPBOARD) this._revision = (this._revision + 1) >>> 0;
        });
        this._object = Gio.DBusExportedObject.wrapJSObject(XML, this);
        this._owner = Gio.bus_own_name(Gio.BusType.SESSION, NAME, Gio.BusNameOwnerFlags.NONE,
            connection => this._object.export(connection, PATH), null, null);
    }

    Revision() { return this._revision; }

    ReadContentAsync(_params, invocation) {
        const reply = (mime, bytes) => invocation.return_value(new GLib.Variant('(say)', [mime, bytes]));
        const types = this._clipboard.get_mimetypes(St.ClipboardType.CLIPBOARD);
        if (types.some(type => ['org.nspasteboard.ConcealedType', 'org.nspasteboard.TransientType', 'x-kde-passwordManagerHint'].includes(type))) {
            reply('', []);
            return;
        }
        const type = ['text/uri-list', 'x-special/gnome-copied-files', 'image/png', 'image/tiff',
            'text/plain;charset=utf-8', 'UTF8_STRING', 'text/plain', 'STRING'].find(t => types.includes(t));
        if (!type) { reply('', []); return; }
        const revision = this._revision;
        const limit = type.startsWith('image/') ? 8 * 1024 * 1024 : 102400;
        const stream = Gio.MemoryOutputStream.new_resizable();
        const cancel = new Gio.Cancellable();
        let timeout = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 1500, () => {
            timeout = 0;
            cancel.cancel();
            return GLib.SOURCE_REMOVE;
        });
        // Bound the transfer inside Shell; St.get_content() uses an unlimited transfer.
        this._selection.transfer_async(Meta.SelectionType.CLIPBOARD, type, limit + 1, stream, cancel, (selection, result) => {
            if (timeout) GLib.source_remove(timeout);
            try {
                selection.transfer_finish(result);
                stream.close(null);
                const bytes = stream.steal_as_bytes();
                if (bytes.get_size() > limit || this._revision !== revision) reply('', []);
                else reply(type.startsWith('image/') ? type : ['text/uri-list', 'x-special/gnome-copied-files'].includes(type) ? 'text/uri-list' : 'text/plain', bytes.get_data());
            } catch { reply('', []); }
        });
    }

    WriteContent(mime, bytes) {
        const image = ['image/png', 'image/tiff'].includes(mime);
        if ((!image && !['text/plain', 'text/uri-list'].includes(mime)) || bytes.length > (image ? 8 * 1024 * 1024 : 102400))
            throw new Error('Unsupported clipboard content');
        if (mime === 'text/plain') this._clipboard.set_text(St.ClipboardType.CLIPBOARD, new TextDecoder().decode(bytes));
        else this._clipboard.set_content(St.ClipboardType.CLIPBOARD, mime, new GLib.Bytes(bytes));
    }

    // Read only when the running desktop app requests it; the bridge stores no text.
    ReadTextAsync(_params, invocation) {
        const types = this._clipboard.get_mimetypes(St.ClipboardType.CLIPBOARD);
        const concealed = ['org.nspasteboard.ConcealedType', 'org.nspasteboard.TransientType',
            'x-kde-passwordManagerHint'];
        if (types.some(type => concealed.includes(type))) {
            invocation.return_value(new GLib.Variant('(s)', ['']));
            return;
        }
        this._clipboard.get_text(St.ClipboardType.CLIPBOARD, (_clipboard, text) => {
            invocation.return_value(new GLib.Variant('(s)', [text && text.length <= 102400 ? text : '']));
        });
    }

    WriteText(text) {
        if (text.length <= 102400)
            this._clipboard.set_text(St.ClipboardType.CLIPBOARD, text);
    }

    RememberTarget(ownPid) {
        if (this._target && this._targetClosed) this._target.disconnect(this._targetClosed);
        const window = global.display.focus_window;
        this._target = window && window.get_pid() !== ownPid ? window : null;
        this._targetClosed = this._target?.connect('unmanaged', () => {
            this._target = null;
            this._targetClosed = 0;
        }) ?? 0;
        return !!this._target;
    }

    FocusPopup(ownPid) {
        const window = global.get_window_actors().map(actor => actor.meta_window)
            .find(window => window.get_pid() === ownPid);
        if (!window) return false;
        window.activate(global.get_current_time());
        return true;
    }

    PasteAsync(_params, invocation) {
        const target = this._target;
        if (target && this._targetClosed) target.disconnect(this._targetClosed);
        this._targetClosed = 0;
        this._target = null; // A request consumes its target; never retry a paste implicitly.
        if (!target || this._pasteTimer) {
            invocation.return_value(new GLib.Variant('(b)', [false]));
            return;
        }
        try { target.activate(global.get_current_time()); }
        catch { invocation.return_value(new GLib.Variant('(b)', [false])); return; }
        let attempts = 0;
        this._pasteTimer = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 20, () => {
            if (global.display.focus_window !== target) {
                if (++attempts < 20) return GLib.SOURCE_CONTINUE;
                invocation.return_value(new GLib.Variant('(b)', [false]));
            } else {
                this._keyboard ??= Clutter.get_default_backend().get_default_seat()
                    .create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
                const time = GLib.get_monotonic_time();
                const id = `${target.get_gtk_application_id?.() ?? ''} ${target.get_wm_class() ?? ''}`.toLowerCase();
                const terminal = ['gnome-terminal', 'gnome.terminal', 'gnome.console', 'kgx', 'ptyxis',
                    'konsole', 'kitty', 'alacritty', 'xfce4-terminal', 'tilix', 'terminator', 'wezterm']
                    .some(name => id.includes(name));
                let sent = false;
                try {
                    this._keyboard.notify_keyval(time, Clutter.KEY_Control_L, Clutter.KeyState.PRESSED);
                    if (terminal) this._keyboard.notify_keyval(time, Clutter.KEY_Shift_L, Clutter.KeyState.PRESSED);
                    this._keyboard.notify_keyval(time, Clutter.KEY_v, Clutter.KeyState.PRESSED);
                    sent = true;
                } finally {
                    this._keyboard.notify_keyval(time, Clutter.KEY_v, Clutter.KeyState.RELEASED);
                    if (terminal) this._keyboard.notify_keyval(time, Clutter.KEY_Shift_L, Clutter.KeyState.RELEASED);
                    this._keyboard.notify_keyval(time, Clutter.KEY_Control_L, Clutter.KeyState.RELEASED);
                    invocation.return_value(new GLib.Variant('(b)', [sent]));
                }
            }
            this._pasteTimer = 0;
            return GLib.SOURCE_REMOVE;
        });
    }

    disable() {
        if (this._changed) this._selection.disconnect(this._changed);
        this._changed = 0;
        this._selection = null;
        if (this._pasteTimer) GLib.source_remove(this._pasteTimer);
        if (this._target && this._targetClosed) this._target.disconnect(this._targetClosed);
        this._targetClosed = 0;
        this._pasteTimer = 0;
        this._target = null;
        this._keyboard?.run_dispose();
        this._keyboard = null;
        if (this._owner) Gio.bus_unown_name(this._owner);
        this._object?.unexport();
        this._owner = null;
        this._object = null;
        this._clipboard = null;
    }
}
