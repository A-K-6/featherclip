// SPDX-License-Identifier: MIT
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import St from 'gi://St';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const NAME = 'io.github.AK6.FeatherClip.Gnome';
const PATH = '/io/github/AK6/FeatherClip/Gnome';
const XML = `<node><interface name="${NAME}">
  <method name="ReadText"><arg type="s" direction="out"/></method>
  <method name="WriteText"><arg type="s" direction="in"/></method>
</interface></node>`;

export default class FeatherClipBridge extends Extension {
    enable() {
        this._clipboard = St.Clipboard.get_default();
        this._object = Gio.DBusExportedObject.wrapJSObject(XML, this);
        this._owner = Gio.bus_own_name(Gio.BusType.SESSION, NAME, Gio.BusNameOwnerFlags.NONE,
            connection => this._object.export(connection, PATH), null, null);
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

    disable() {
        if (this._owner) Gio.bus_unown_name(this._owner);
        this._object?.unexport();
        this._owner = null;
        this._object = null;
        this._clipboard = null;
    }
}
