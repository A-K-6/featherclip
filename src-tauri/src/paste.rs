use std::sync::{atomic::AtomicBool, Mutex};

#[derive(Clone, Copy, Debug, Default)]
pub enum Target {
    #[default]
    None,
    #[cfg(target_os = "macos")]
    Mac { pid: i32, launched: Option<f64> },
    #[cfg(target_os = "linux")]
    X11(u32),
    #[cfg(target_os = "linux")]
    Gnome,
}

#[derive(Default)]
pub struct Controller {
    pub target: Mutex<Target>,
    pub busy: AtomicBool,
    pub popup_session: std::sync::atomic::AtomicU64,
    #[cfg(target_os = "macos")]
    pub previous: std::sync::Arc<Mutex<Target>>,
}

impl Target {
    pub fn available(self) -> bool {
        !matches!(self, Self::None)
    }
}

#[cfg(target_os = "macos")]
pub fn watch_external_apps(controller: &Controller) {
    let previous = std::sync::Arc::clone(&controller.previous);
    *previous.lock().unwrap() = remember();
    let callback = block2::RcBlock::new(
        move |_: std::ptr::NonNull<objc2_foundation::NSNotification>| {
            let target = remember();
            if target.available() {
                *previous.lock().unwrap() = target;
            }
        },
    );
    // The workspace owns this observer until process exit. No foreground polling.
    unsafe {
        objc2_app_kit::NSWorkspace::sharedWorkspace()
            .notificationCenter()
            .addObserverForName_object_queue_usingBlock(
                Some(objc2_app_kit::NSWorkspaceDidActivateApplicationNotification),
                None,
                None,
                &callback,
            );
    }
}

pub fn access_allowed() -> bool {
    #[cfg(target_os = "macos")]
    {
        objc2_core_graphics::CGPreflightPostEventAccess()
    }
    #[cfg(target_os = "linux")]
    {
        true
    }
}

pub fn request_access() -> bool {
    #[cfg(target_os = "macos")]
    {
        objc2_core_graphics::CGRequestPostEventAccess()
    }
    #[cfg(target_os = "linux")]
    {
        true
    }
}

/// Capture before the popup takes focus, and never use FeatherClip as its own target.
pub fn remember() -> Target {
    #[cfg(target_os = "macos")]
    {
        objc2_app_kit::NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .filter(|app| {
                app.processIdentifier() > 0
                    && app.processIdentifier() as u32 != std::process::id()
                    && app
                        .bundleIdentifier()
                        .is_none_or(|id| id.to_string() != "io.github.AK6.FeatherClip")
            })
            .map(|app| Target::Mac {
                pid: app.processIdentifier(),
                launched: app.launchDate().map(|date| date.timeIntervalSince1970()),
            })
            .unwrap_or_default()
    }
    #[cfg(target_os = "linux")]
    {
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
}

pub fn perform(target: Target) -> Result<(), String> {
    match target {
        Target::None => Err("Copied. Open the popup while your destination app is active, then choose the clip again.".into()),
        #[cfg(target_os = "macos")]
        Target::Mac { pid, launched } => mac_paste(pid, launched),
        #[cfg(target_os = "linux")]
        Target::X11(window) => x11::paste(window).map_err(|_| "Copied, but the original window couldn't receive Paste. Use Ctrl+V manually.".into()),
        #[cfg(target_os = "linux")]
        Target::Gnome => {
            let sent = gnome_call("Paste", None)?.get::<(bool,)>().is_some_and(|v| v.0);
            if sent { Ok(()) } else { Err("Copied, but the original window couldn't be restored. Use Ctrl+V manually.".into()) }
        }
    }
}

#[cfg(target_os = "macos")]
fn mac_paste(pid: i32, launched: Option<f64>) -> Result<(), String> {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication, NSWorkspace};
    use objc2_core_graphics::{
        CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation, CGEventType,
    };
    if !access_allowed() {
        return Err("Copied. Allow FeatherClip in System Settings → Privacy & Security → Accessibility to enable direct paste.".into());
    }
    let app = NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
        .filter(|app| !app.isTerminated())
        .filter(|app| {
            launched.is_none_or(|time| {
                app.launchDate()
                    .is_some_and(|d| d.timeIntervalSince1970() == time)
            })
        })
        .ok_or("Copied, but the original app has closed. Paste manually in another app.")?;
    if !app.activateWithOptions(NSApplicationActivationOptions::empty()) {
        return Err("Copied, but the original app couldn't be activated.".into());
    }
    // Wait for activation and verify the destination before using the native session stream.
    for _ in 0..20 {
        if NSWorkspace::sharedWorkspace()
            .frontmostApplication()
            .is_some_and(|a| a.processIdentifier() == pid)
        {
            std::thread::sleep(std::time::Duration::from_millis(60));
            if !NSWorkspace::sharedWorkspace()
                .frontmostApplication()
                .is_some_and(|a| a.processIdentifier() == pid)
            {
                return Err(
                    "Copied. The destination changed before Paste, so no keys were sent.".into(),
                );
            }
            let source = CGEventSource::new(CGEventSourceStateID::CombinedSessionState)
                .ok_or("Couldn't create Paste source.")?;
            let command = CGEventFlags::from_bits_retain(CGEventFlags::MaskCommand.0 | 8);
            for (key, down, modifier, flags) in [
                (55, true, true, command),
                (9, true, false, command),
                (9, false, false, command),
                (55, false, true, CGEventFlags::empty()),
            ] {
                let event = CGEvent::new_keyboard_event(Some(&source), key, down)
                    .ok_or("Couldn't create Paste event.")?;
                if modifier {
                    CGEvent::set_type(Some(&event), CGEventType::FlagsChanged);
                }
                CGEvent::set_flags(Some(&event), flags);
                CGEvent::post(CGEventTapLocation::SessionEventTap, Some(&event));
            }
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
    Err("Copied. The original app didn't regain focus, so no Paste keys were sent.".into())
}

#[cfg(target_os = "linux")]
pub fn gnome_call(method: &str, params: Option<&glib::Variant>) -> Result<glib::Variant, String> {
    gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE)
        .and_then(|bus| bus.call_sync(Some("io.github.AK6.FeatherClip.Gnome"),
            "/io/github/AK6/FeatherClip/Gnome", "io.github.AK6.FeatherClip.Gnome", method,
            params, None, gio::DBusCallFlags::NONE, 2000, gio::Cancellable::NONE))
        .map_err(|_| "The GNOME bridge couldn't complete Paste. Enable the updated bridge, then reopen history.".into())
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
