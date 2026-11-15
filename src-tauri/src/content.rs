use serde::Serialize;
use std::{
    hash::{Hash, Hasher},
    sync::Arc,
};

pub const MAX_TEXT: usize = 100 * 1024;
pub const MAX_IMAGE: usize = 8 * 1024 * 1024;
pub const MAX_HISTORY: usize = 24 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Text,
    Image,
    Files,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Content {
    Text(Arc<str>),
    Image {
        mime: &'static str,
        bytes: Arc<[u8]>,
    },
    Files(Arc<[String]>),
}

impl From<String> for Content {
    fn from(text: String) -> Self {
        Self::Text(text.into())
    }
}
impl From<&str> for Content {
    fn from(text: &str) -> Self {
        Self::Text(text.into())
    }
}

impl Content {
    pub fn kind(&self) -> Kind {
        match self {
            Self::Text(_) => Kind::Text,
            Self::Image { .. } => Kind::Image,
            Self::Files(_) => Kind::Files,
        }
    }
    pub fn size(&self) -> usize {
        match self {
            Self::Text(s) => s.len(),
            Self::Image { bytes, .. } => bytes.len(),
            Self::Files(uris) => uris.iter().map(String::len).sum(),
        }
    }
    pub fn valid(&self) -> bool {
        match self {
            Self::Text(text) => !text.trim().is_empty() && text.len() <= MAX_TEXT,
            Self::Image { mime, bytes } => {
                matches!(*mime, "image/png" | "image/tiff")
                    && !bytes.is_empty()
                    && bytes.len() <= MAX_IMAGE
            }
            Self::Files(uris) => {
                !uris.is_empty()
                    && uris.len() <= 100
                    && self.size() <= MAX_TEXT
                    && uris
                        .iter()
                        .all(|uri| url::Url::parse(uri).is_ok_and(|u| u.to_file_path().is_ok()))
            }
        }
    }
    pub fn label(&self) -> String {
        match self {
            Self::Text(text) => text.to_string(),
            Self::Image { mime, bytes } => format!(
                "Image · {} · {} KiB",
                if *mime == "image/png" { "PNG" } else { "TIFF" },
                bytes.len().div_ceil(1024)
            ),
            Self::Files(uris) => uris
                .iter()
                .map(|uri| {
                    url::Url::parse(uri)
                        .ok()
                        .and_then(|u| u.to_file_path().ok())
                        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
                        .unwrap_or_else(|| "File".into())
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
    pub fn fingerprint(&self) -> u64 {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        self.hash(&mut hash);
        hash.finish()
    }
    pub fn preview(&self) -> Option<String> {
        use base64::Engine;
        let Self::Image { mime, bytes } = self else {
            return None;
        };
        let format = if *mime == "image/png" {
            image::ImageFormat::Png
        } else {
            image::ImageFormat::Tiff
        };
        let mut reader = image::ImageReader::with_format(std::io::Cursor::new(bytes), format);
        let mut limits = image::Limits::default();
        limits.max_alloc = Some(32 * 1024 * 1024);
        limits.max_image_width = Some(8192);
        limits.max_image_height = Some(8192);
        reader.limits(limits);
        let thumbnail = reader.decode().ok()?.thumbnail(96, 64);
        let mut png = std::io::Cursor::new(Vec::new());
        thumbnail.write_to(&mut png, image::ImageFormat::Png).ok()?;
        let png = png.into_inner();
        if png.len() > 24 * 1024 {
            return None;
        }
        Some(format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(png)
        ))
    }
    #[cfg(test)]
    pub fn text(&self) -> Option<&str> {
        if let Self::Text(s) = self {
            Some(s)
        } else {
            None
        }
    }
}

pub fn file_uris(bytes: &[u8]) -> Option<Content> {
    if bytes.len() > MAX_TEXT {
        return None;
    }
    let uris: Vec<String> = std::str::from_utf8(bytes)
        .ok()?
        .lines()
        .filter(|line| {
            !line.is_empty() && !line.starts_with('#') && *line != "copy" && *line != "cut"
        })
        .map(str::to_owned)
        .collect();
    let content = Content::Files(uris.into());
    content.valid().then_some(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn file_references_accept_local_urls_and_normalize_copy_or_cut() {
        let content = file_uris(b"cut\nfile:///tmp/a%20b.png\nfile:///tmp/report.pdf\n").unwrap();
        assert_eq!(content.label(), "a b.png\nreport.pdf");
        assert!(file_uris(b"https://example.com/file").is_none());
        assert!(file_uris(b"file://remote-server/shared/file").is_none());
        assert!(file_uris(b"copy\n").is_none());
    }
}
