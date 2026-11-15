pub fn needs_gnome_bridge() -> bool {
    cfg!(target_os = "linux")
        && std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_lowercase()
            .contains("gnome")
}

use crate::content::Content;
#[cfg(target_os = "linux")]
#[path = "clipboard_linux.rs"]
mod linux;
#[cfg(target_os = "macos")]
#[path = "clipboard_mac.rs"]
mod mac;

pub enum Clipboard {
    #[cfg(target_os = "macos")]
    Apple(objc2::rc::Retained<objc2_app_kit::NSPasteboard>, isize),
    #[cfg(target_os = "linux")]
    Native(linux::Native),
    #[cfg(target_os = "linux")]
    Gnome(gio::DBusConnection, Option<u32>),
    /// Synthetic data only, for release smoke checks; never touches the real clipboard.
    Demo(Option<Content>),
}

impl Clipboard {
    pub fn new(demo: bool) -> Result<Self, String> {
        if demo {
            return Ok(Self::Demo(None));
        }
        #[cfg(target_os = "linux")]
        if needs_gnome_bridge() {
            return gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE)
                .map(|bus| Self::Gnome(bus, None))
                .map_err(|_| "Couldn't connect to the desktop session.".into());
        }
        #[cfg(target_os = "macos")]
        {
            Ok(Self::Apple(
                objc2_app_kit::NSPasteboard::generalPasteboard(),
                -1,
            ))
        }
        #[cfg(target_os = "linux")]
        {
            linux::Native::new().map(Self::Native).map_err(|_| {
                "Couldn't connect to the clipboard. Start FeatherClip inside your desktop session."
                    .into()
            })
        }
    }

    pub fn changed(&mut self) -> bool {
        #[cfg(target_os = "macos")]
        if let Self::Apple(board, last_count) = self {
            let count = board.changeCount();
            if count == *last_count {
                return false;
            }
            *last_count = count;
        }
        #[cfg(target_os = "linux")]
        match self {
            Self::Native(native) => return native.changed(),
            Self::Gnome(bus, last) => {
                let revision = linux::gnome_call(bus, "Revision", None)
                    .ok()
                    .and_then(|v| v.get::<(u32,)>())
                    .map(|v| v.0);
                let changed = revision.is_none() || *last != revision;
                *last = revision;
                return changed;
            }
            _ => {}
        }
        true
    }

    pub fn read(&mut self) -> Result<Option<Content>, String> {
        match self {
            Self::Demo(text) => Ok(text.clone()),
            #[cfg(target_os = "macos")]
            Self::Apple(board, _) => mac::read(board),
            #[cfg(target_os = "linux")]
            Self::Native(clipboard) => clipboard.read(),
            #[cfg(target_os = "linux")]
            Self::Gnome(connection, _) => linux::gnome_read(connection),
        }
    }

    pub fn write(&mut self, text: Content) -> Result<(), String> {
        match self {
            Self::Demo(value) => {
                *value = Some(text);
                Ok(())
            }
            #[cfg(target_os = "macos")]
            Self::Apple(board, _) => mac::write(board, &text),
            #[cfg(target_os = "linux")]
            Self::Native(clipboard) => clipboard.write(&text),
            #[cfg(target_os = "linux")]
            Self::Gnome(connection, _) => linux::gnome_write(connection, &text),
        }
    }
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use objc2_app_kit::NSPasteboard;
    use objc2_foundation::NSString;

    #[test]
    fn mac_pasteboard_roundtrip_and_concealed_text_exclusion() {
        // A unique pasteboard exercises the actual OS API without touching personal clips.
        let board = NSPasteboard::pasteboardWithUniqueName();
        let mut clipboard = Clipboard::Apple(board.clone(), -1);
        let text = "سلام 👋\nexact text\t ";
        clipboard.write(text.into()).unwrap();
        assert!(clipboard.changed());
        assert!(!clipboard.changed());
        assert_eq!(
            clipboard.read().unwrap().as_ref().and_then(Content::text),
            Some(text)
        );
        board.setString_forType(
            &NSString::from_str(""),
            &NSString::from_str("org.nspasteboard.ConcealedType"),
        );
        assert_eq!(clipboard.read().unwrap(), None);
        let image = Content::Image {
            mime: "image/png",
            bytes: include_bytes!("../icons/32x32.png").to_vec().into(),
        };
        clipboard.write(image.clone()).unwrap();
        assert_eq!(clipboard.read().unwrap(), Some(image));
        let files = crate::content::file_uris(
            b"file:///tmp/FeatherClip%20test.txt\nfile:///tmp/FeatherClip%20test.png",
        )
        .unwrap();
        clipboard.write(files.clone()).unwrap();
        assert_eq!(clipboard.read().unwrap(), Some(files));
        let _: () = unsafe { objc2::msg_send![&*board, releaseGlobally] };
    }
}

#[cfg(all(test, target_os = "linux"))]
mod linux_tests {
    use super::*;

    #[test]
    #[ignore = "Requires the bridge in an isolated headless GNOME session"]
    fn isolated_gnome_bridge_roundtrip() {
        assert!(std::env::var("FEATHERCLIP_ISOLATED_DISPLAY").is_ok());
        assert!(needs_gnome_bridge());
        let mut clipboard = Clipboard::new(false).unwrap();
        let text = "FeatherClip GNOME Wayland integration — سلام 👋\nnext line";
        clipboard.write(text.into()).unwrap();
        assert_eq!(
            clipboard.read().unwrap().as_ref().and_then(Content::text),
            Some(text)
        );
        rich_roundtrip(&mut clipboard);
    }

    #[test]
    #[ignore = "Run only under an isolated Xvfb display, never on a user's desktop"]
    fn isolated_x11_clipboard_roundtrip() {
        assert!(std::env::var("FEATHERCLIP_ISOLATED_DISPLAY").is_ok());
        let mut clipboard = Clipboard::new(false).unwrap();
        let text = "FeatherClip isolated integration test — سلام 👋";
        clipboard.write(text.into()).unwrap();
        assert_eq!(
            clipboard.read().unwrap().as_ref().and_then(Content::text),
            Some(text)
        );
        rich_roundtrip(&mut clipboard);
    }

    fn rich_roundtrip(clipboard: &mut Clipboard) {
        let image = Content::Image {
            mime: "image/png",
            bytes: include_bytes!("../icons/32x32.png").to_vec().into(),
        };
        clipboard.write(image.clone()).unwrap();
        let read = clipboard.read().unwrap().unwrap();
        let Content::Image { bytes, .. } = read else {
            panic!("Image was lost")
        };
        let expected = image::load_from_memory(include_bytes!("../icons/32x32.png"))
            .unwrap()
            .into_rgba8();
        assert_eq!(
            image::load_from_memory(&bytes).unwrap().into_rgba8(),
            expected
        );
        let directory =
            std::env::temp_dir().join(format!("featherclip-files-{}", std::process::id()));
        std::fs::create_dir_all(&directory).unwrap();
        let uris = ["clipboard test.txt", "clipboard test.png"].map(|name| {
            let path = directory.join(name);
            std::fs::write(&path, b"synthetic clipboard fixture").unwrap();
            url::Url::from_file_path(&path).unwrap().to_string()
        });
        let files = crate::content::file_uris(uris.join("\n").as_bytes()).unwrap();
        clipboard.write(files.clone()).unwrap();
        assert_eq!(clipboard.read().unwrap(), Some(files));
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[cfg(target_os = "linux")]
fn gnome_help() -> String {
    "GNOME Wayland needs the bundled FeatherClip GNOME Bridge. Enable it below. After a first install, log out and back in if GNOME hasn't discovered it yet.".into()
}
