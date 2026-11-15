#[derive(Clone, Copy, Debug, Default)]
pub enum Target {
    #[default]
    None,
    X11(u32),
    Gnome,
}
impl Target {
    pub fn available(self) -> bool {
        !matches!(self, Self::None)
    }
}
pub fn remember() -> Target {
    if crate::clipboard::needs_gnome_bridge() {
        use glib::variant::ToVariant;
        return if gnome_call("RememberTarget", Some(&(std::process::id(),).to_variant()))
            .ok()
            .and_then(|v| v.get::<(bool,)>())
            .is_some_and(|v| v.0)
        {
            Target::Gnome
        } else {
            Target::None
        };
    }
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return Target::None;
    }
    x11::remember().map(Target::X11).unwrap_or_default()
}
pub fn perform(target: Target) -> Result<(), String> {
    match target {
        Target::None => Err("Copied. Reopen the popup from your destination app to paste.".into()),
        Target::X11(window) => x11::paste(window)
            .map_err(|_| "Copied, but the destination couldn't receive Paste. Use Ctrl+V.".into()),
        Target::Gnome => {
            if gnome_call("Paste", None)?
                .get::<(bool,)>()
                .is_some_and(|v| v.0)
            {
                Ok(())
            } else {
                Err("Copied. The original window couldn't regain focus.".into())
            }
        }
    }
}
pub fn gnome_call(method: &str, params: Option<&glib::Variant>) -> Result<glib::Variant, String> {
    gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE)
        .and_then(|b| {
            b.call_sync(
                Some("io.github.AK6.FeatherClip.Gnome"),
                "/io/github/AK6/FeatherClip/Gnome",
                "io.github.AK6.FeatherClip.Gnome",
                method,
                params,
                None,
                gio::DBusCallFlags::NONE,
                2000,
                gio::Cancellable::NONE,
            )
        })
        .map_err(|_| "Enable the bundled GNOME bridge, then reopen the clipboard.".into())
}
#[cfg(target_os = "linux")]
mod x11 {
    use x11rb::{
        connection::Connection,
        protocol::{xproto::*, xtest::ConnectionExt as _},
        rust_connection::RustConnection,
        CURRENT_TIME,
    };
    type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

    fn active(connection: &RustConnection, root: Window) -> Result<Option<Window>> {
        let atom = connection
            .intern_atom(false, b"_NET_ACTIVE_WINDOW")?
            .reply()?
            .atom;
        Ok(connection
            .get_property(false, root, atom, AtomEnum::WINDOW, 0, 1)?
            .reply()?
            .value32()
            .and_then(|mut v| v.next())
            .filter(|w| *w > 1))
    }

    pub fn remember() -> Result<Window> {
        let (connection, screen) = x11rb::connect(None)?;
        let root = connection.setup().roots[screen].root;
        let window =
            active(&connection, root)?.unwrap_or(connection.get_input_focus()?.reply()?.focus);
        if window <= 1 || window == root {
            return Err("No destination window".into());
        }
        let atom = connection.intern_atom(false, b"_NET_WM_PID")?.reply()?.atom;
        let owner = connection
            .get_property(false, window, atom, AtomEnum::CARDINAL, 0, 1)?
            .reply()?;
        if owner.value32().and_then(|mut v| v.next()) == Some(std::process::id()) {
            return Err("Own window".into());
        }
        Ok(window)
    }

    pub fn paste(window: Window) -> Result<()> {
        let (connection, screen) = x11rb::connect(None)?;
        let root = connection.setup().roots[screen].root;
        if connection.get_window_attributes(window)?.reply()?.map_state != MapState::VIEWABLE {
            return Err("Destination is no longer visible".into());
        }
        let managed = active(&connection, root)?.is_some();
        if managed {
            let atom = connection
                .intern_atom(false, b"_NET_ACTIVE_WINDOW")?
                .reply()?
                .atom;
            let event = ClientMessageEvent::new(32, window, atom, [2, CURRENT_TIME, 0, 0, 0]);
            connection
                .send_event(
                    false,
                    root,
                    EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
                    event,
                )?
                .check()?;
        } else {
            connection
                .set_input_focus(InputFocus::PARENT, window, CURRENT_TIME)?
                .check()?;
        }
        connection.flush()?;
        let mut focused = false;
        for _ in 0..20 {
            focused = if managed {
                active(&connection, root)? == Some(window)
            } else {
                connection.get_input_focus()?.reply()?.focus == window
            };
            if focused {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        if !focused {
            return Err("Destination did not regain focus".into());
        }
        let setup = connection.setup();
        let map = connection
            .get_keyboard_mapping(setup.min_keycode, setup.max_keycode - setup.min_keycode + 1)?
            .reply()?;
        let code = |symbol| {
            map.keysyms
                .chunks(map.keysyms_per_keycode as usize)
                .position(|s| s.contains(&symbol))
                .map(|i| setup.min_keycode + i as u8)
                .ok_or("Keyboard layout doesn't support Paste")
        };
        let control = code(0xffe3)?;
        let v = code(0x76)?;
        let class = connection
            .get_property(false, window, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 256)?
            .reply()?;
        let class = String::from_utf8_lossy(&class.value).to_lowercase();
        let terminal = [
            "gnome-terminal",
            "gnome.terminal",
            "gnome.console",
            "kgx",
            "ptyxis",
            "konsole",
            "kitty",
            "alacritty",
            "xfce4-terminal",
            "tilix",
            "terminator",
            "wezterm",
        ]
        .iter()
        .any(|name| class.contains(name));
        let shift = if terminal { Some(code(0xffe1)?) } else { None };
        let mut events = vec![(KEY_PRESS_EVENT, control)];
        if let Some(shift) = shift {
            events.push((KEY_PRESS_EVENT, shift));
        }
        events.extend([(KEY_PRESS_EVENT, v), (KEY_RELEASE_EVENT, v)]);
        if let Some(shift) = shift {
            events.push((KEY_RELEASE_EVENT, shift));
        }
        events.push((KEY_RELEASE_EVENT, control));
        for (kind, key) in events {
            connection
                .xtest_fake_input(kind, key, CURRENT_TIME, root, 0, 0, 0)?
                .check()?;
        }
        connection.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn absent_target_never_sends_paste() {
        assert!(!Target::None.available());
        assert!(perform(Target::None).is_err());
    }
}

#[cfg(all(test, target_os = "linux"))]
mod desktop_tests {
    use super::*;
    use glib::variant::ToVariant;

    fn fixture(method: &str, params: Option<&glib::Variant>) -> glib::Variant {
        gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE)
            .unwrap()
            .call_sync(
                Some("io.github.AK6.FeatherClip.PasteFixture"),
                "/io/github/AK6/FeatherClip/PasteFixture",
                "io.github.AK6.FeatherClip.PasteFixture",
                method,
                params,
                None,
                gio::DBusCallFlags::NONE,
                4000,
                gio::Cancellable::NONE,
            )
            .unwrap()
    }

    fn roundtrip() {
        assert!(std::env::var("FEATHERCLIP_ISOLATED_DISPLAY").is_ok());
        fixture("Focus", Some(&("A",).to_variant()));
        let target = remember();
        assert!(target.available(), "No fixture target");
        fixture("Focus", Some(&("B",).to_variant()));
        let mut clipboard = crate::clipboard::Clipboard::new(false).unwrap();
        let text = "Paste into the previous window — سلام 👋\nsecond line";
        clipboard.write(text.into()).unwrap();
        perform(target).unwrap();
        let mut result = (String::new(), String::new());
        for _ in 0..50 {
            result = fixture("Read", None).get::<(String, String)>().unwrap();
            if result.0 == text {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        assert_eq!(
            result.0, text,
            "The previous window did not receive the clip"
        );
        assert!(result.1.is_empty(), "The intervening window received text");
    }

    #[test]
    #[ignore = "Requires an isolated headless GNOME desktop and GTK fixture"]
    fn isolated_gnome_paste_roundtrip() {
        assert!(crate::clipboard::needs_gnome_bridge());
        roundtrip();
    }

    #[test]
    #[ignore = "Requires an isolated Xvfb/Openbox desktop and GTK fixture"]
    fn isolated_x11_paste_roundtrip() {
        roundtrip();
    }
}
