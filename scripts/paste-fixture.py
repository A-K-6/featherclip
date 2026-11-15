"""Two disposable GTK editors for isolated desktop integration tests only."""
import gi
gi.require_version('Gtk', '3.0')
from gi.repository import Gio, GLib, Gtk

NAME = 'io.github.AK6.FeatherClip.PasteFixture'
PATH = '/io/github/AK6/FeatherClip/PasteFixture'
XML = f'''<node><interface name="{NAME}">
<method name="Focus"><arg type="s" direction="in"/></method>
<method name="Read"><arg type="s" direction="out"/><arg type="s" direction="out"/></method>
</interface></node>'''
windows = {}
editors = {}
for label in ('A', 'B'):
    window = Gtk.Window(title=f'FeatherClip paste fixture {label}')
    window.set_default_size(400, 200)
    editor = Gtk.TextView()
    window.add(editor)
    window.show_all()
    windows[label] = window
    editors[label] = editor

def contents(label):
    buffer = editors[label].get_buffer()
    return buffer.get_text(buffer.get_start_iter(), buffer.get_end_iter(), True)

def call(_bus, _sender, _path, _interface, method, params, invocation):
    if method == 'Read':
        invocation.return_value(GLib.Variant('(ss)', (contents('A'), contents('B'))))
    elif method == 'Focus':
        label, = params.unpack()
        for editor in editors.values():
            editor.get_buffer().set_text('')
        window = windows[label]
        window.present()
        editors[label].grab_focus()
        attempts = [0]
        def wait_for_focus():
            attempts[0] += 1
            if window.get_property('is-active') or attempts[0] >= 100:
                invocation.return_value(GLib.Variant('()', ()))
                return False
            return True
        GLib.timeout_add(20, wait_for_focus)

def acquired(bus, _name):
    info = Gio.DBusNodeInfo.new_for_xml(XML).interfaces[0]
    bus.register_object(PATH, info, call, None, None)

Gio.bus_own_name(Gio.BusType.SESSION, NAME, Gio.BusNameOwnerFlags.NONE, acquired, None, None)
Gtk.main()
