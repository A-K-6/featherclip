use crate::content::{file_uris, Content, MAX_IMAGE};
use glib::variant::ToVariant;

pub struct Native {
    clipboard: arboard::Clipboard,
    watcher: Option<Watch>,
}
impl Native {
    pub fn new() -> Result<Self, arboard::Error> {
        Ok(Self {
            clipboard: arboard::Clipboard::new()?,
            watcher: if std::env::var_os("WAYLAND_DISPLAY").is_none() {
                Watch::new().ok()
            } else {
                None
            },
        })
    }
    pub fn changed(&mut self) -> bool {
        self.watcher.as_mut().is_none_or(Watch::changed)
    }
    pub fn read(&mut self) -> Result<Option<Content>, String> {
        if let Ok(paths) = self.clipboard.get().file_list() {
            let uris: Vec<String> = paths
                .iter()
                .filter_map(|p| url::Url::from_file_path(p).ok())
                .map(String::from)
                .collect();
            if let Some(content) = file_uris(uris.join("\n").as_bytes()) {
                return Ok(Some(content));
            }
        }
        // Decode images only after XFixes reports a clipboard ownership change.
        // Other Wayland compositors retain the lightweight text fallback.
        if self.watcher.is_some() {
            if let Ok(image) = self.clipboard.get_image() {
                use image::ImageEncoder;
                if image.bytes.len() > 32 * 1024 * 1024 {
                    return Ok(None);
                }
                let mut png = Vec::new();
                image::codecs::png::PngEncoder::new(&mut png)
                    .write_image(
                        &image.bytes,
                        image.width as u32,
                        image.height as u32,
                        image::ExtendedColorType::Rgba8,
                    )
                    .map_err(|_| "Couldn't read this image.".to_string())?;
                return Ok((png.len() <= MAX_IMAGE).then(|| Content::Image {
                    mime: "image/png",
                    bytes: png.into(),
                }));
            }
        }
        match self.clipboard.get_text() {
            Ok(text) => Ok(Some(text.into())),
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(_) => Err("Clipboard access is unavailable.".into()),
        }
    }
    pub fn write(&mut self, content: &Content) -> Result<(), String> {
        let result = match content {
            Content::Text(text) => self.clipboard.set_text(text.to_string()),
            Content::Files(uris) => {
                let paths: Vec<std::path::PathBuf> = uris
                    .iter()
                    .filter_map(|uri| url::Url::parse(uri).ok()?.to_file_path().ok())
                    .collect();
                self.clipboard.set().file_list(&paths)
            }
            Content::Image { mime, bytes } => {
                if *mime != "image/png" {
                    return Err("This desktop supports PNG clipboard images.".into());
                }
                let mut reader = image::ImageReader::with_format(
                    std::io::Cursor::new(bytes),
                    image::ImageFormat::Png,
                );
                let mut limits = image::Limits::default();
                limits.max_alloc = Some(32 * 1024 * 1024);
                reader.limits(limits);
                let rgba = reader
                    .decode()
                    .map_err(|_| "This image is too large or couldn't be decoded.".to_string())?
                    .into_rgba8();
                self.clipboard.set_image(arboard::ImageData {
                    width: rgba.width() as usize,
                    height: rgba.height() as usize,
                    bytes: std::borrow::Cow::Owned(rgba.into_raw()),
                })
            }
        };
        result.map_err(|_| "Couldn't copy this clip. Please try again.".into())
    }
}

pub fn gnome_call(
    bus: &gio::DBusConnection,
    method: &str,
    params: Option<&glib::Variant>,
) -> Result<glib::Variant, String> {
    bus.call_sync(
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
    .map_err(|_| super::gnome_help())
}
pub fn gnome_read(bus: &gio::DBusConnection) -> Result<Option<Content>, String> {
    let (mime, bytes) = gnome_call(bus, "ReadContent", None)?
        .get::<(String, Vec<u8>)>()
        .ok_or("Unexpected GNOME clipboard response.")?;
    let content = match mime.as_str() {
        "text/plain" => String::from_utf8(bytes).ok().map(Content::from),
        "text/uri-list" => file_uris(&bytes),
        "image/png" | "image/tiff" => Some(Content::Image {
            mime: if mime == "image/png" {
                "image/png"
            } else {
                "image/tiff"
            },
            bytes: bytes.into(),
        }),
        _ => None,
    };
    Ok(content.filter(Content::valid))
}
pub fn gnome_write(bus: &gio::DBusConnection, content: &Content) -> Result<(), String> {
    let (mime, bytes): (&str, Vec<u8>) = match content {
        Content::Text(text) => ("text/plain", text.as_bytes().to_vec()),
        Content::Image { mime, bytes } => (mime, bytes.to_vec()),
        Content::Files(uris) => (
            "text/uri-list",
            format!("{}\r\n", uris.join("\r\n")).into_bytes(),
        ),
    };
    gnome_call(bus, "WriteContent", Some(&(mime, bytes).to_variant())).map(|_| ())
}

struct Watch {
    connection: x11rb::rust_connection::RustConnection,
    first: bool,
}
impl Watch {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        use x11rb::{
            connection::Connection,
            protocol::{xfixes::ConnectionExt as _, xproto::ConnectionExt as _},
        };
        let (connection, screen) = x11rb::connect(None)?;
        connection.xfixes_query_version(5, 0)?.reply()?;
        let atom = connection.intern_atom(false, b"CLIPBOARD")?.reply()?.atom;
        let mask = x11rb::protocol::xfixes::SelectionEventMask::SET_SELECTION_OWNER
            | x11rb::protocol::xfixes::SelectionEventMask::SELECTION_WINDOW_DESTROY
            | x11rb::protocol::xfixes::SelectionEventMask::SELECTION_CLIENT_CLOSE;
        connection
            .xfixes_select_selection_input(connection.setup().roots[screen].root, atom, mask)?
            .check()?;
        connection.flush()?;
        Ok(Self {
            connection,
            first: true,
        })
    }
    fn changed(&mut self) -> bool {
        use x11rb::connection::Connection;
        let mut changed = std::mem::take(&mut self.first);
        while let Ok(Some(event)) = self.connection.poll_for_event() {
            changed |= matches!(event, x11rb::protocol::Event::XfixesSelectionNotify(_));
        }
        changed
    }
}
