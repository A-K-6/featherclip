use crate::content::Content;
#[path = "clipboard_linux.rs"]
mod native;

pub fn needs_gnome_bridge() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        && std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .to_lowercase()
            .contains("gnome")
}
pub enum Clipboard {
    Native(native::Native),
    Gnome(gio::DBusConnection, Option<u32>),
    Demo(Option<Content>),
}
impl Clipboard {
    pub fn new(demo: bool) -> Result<Self, String> {
        if demo {
            return Ok(Self::Demo(None));
        }
        if needs_gnome_bridge() {
            return gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE)
                .map(|b| Self::Gnome(b, None))
                .map_err(|_| gnome_help());
        }
        native::Native::new()
            .map(Self::Native)
            .map_err(|_| "Couldn't open the desktop clipboard.".into())
    }
    pub fn changed(&mut self) -> bool {
        match self {
            Self::Native(n) => n.changed(),
            Self::Gnome(bus, last) => {
                let revision = native::gnome_call(bus, "Revision", None)
                    .ok()
                    .and_then(|v| v.get::<(u32,)>())
                    .map(|v| v.0);
                let changed = revision.is_none() || *last != revision;
                *last = revision;
                changed
            }
            Self::Demo(_) => true,
        }
    }
    pub fn read(&mut self) -> Result<Option<Content>, String> {
        match self {
            Self::Native(n) => n.read(),
            Self::Gnome(b, _) => native::gnome_read(b),
            Self::Demo(v) => Ok(v.clone()),
        }
    }
    pub fn write(&mut self, value: Content) -> Result<(), String> {
        match self {
            Self::Native(n) => n.write(&value),
            Self::Gnome(b, _) => native::gnome_write(b, &value),
            Self::Demo(v) => {
                *v = Some(value);
                Ok(())
            }
        }
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
            bytes: include_bytes!("../../assets/sample.png").to_vec().into(),
        };
        clipboard.write(image.clone()).unwrap();
        let read = clipboard.read().unwrap().unwrap();
        let Content::Image { bytes, .. } = read else {
            panic!("Image was lost")
        };
        let expected = image::load_from_memory(include_bytes!("../../assets/sample.png"))
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
