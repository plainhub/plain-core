//! Resource (image, media) model structures.

use serde::{Deserialize, Serialize};

/// The file extensions a resource can carry, with its type and MIME type.
///
/// One table for every format: the parsers, the MIME lookup and the extension a saved
/// resource gets all read it, so they cannot disagree about a file.
const EXTENSIONS: &[(&str, ResourceType, &str)] = &[
    ("png", ResourceType::Image, "image/png"),
    ("jpg", ResourceType::Image, "image/jpeg"),
    ("jpeg", ResourceType::Image, "image/jpeg"),
    ("gif", ResourceType::Image, "image/gif"),
    ("bmp", ResourceType::Image, "image/bmp"),
    ("tiff", ResourceType::Image, "image/tiff"),
    ("tif", ResourceType::Image, "image/tiff"),
    ("webp", ResourceType::Image, "image/webp"),
    ("svg", ResourceType::Image, "image/svg+xml"),
    ("wmf", ResourceType::Image, "image/x-wmf"),
    ("emf", ResourceType::Image, "image/x-emf"),
    // JPEG XR. Office stores HD Photo layers (`.wdp`) in this format.
    ("wdp", ResourceType::Image, "image/vnd.ms-photo"),
    ("hdp", ResourceType::Image, "image/vnd.ms-photo"),
    ("jxr", ResourceType::Image, "image/vnd.ms-photo"),
    ("mp3", ResourceType::Audio, "audio/mpeg"),
    ("wav", ResourceType::Audio, "audio/wav"),
    ("ogg", ResourceType::Audio, "audio/ogg"),
    ("m4a", ResourceType::Audio, "audio/mp4"),
    ("wma", ResourceType::Audio, "audio/x-ms-wma"),
    ("mp4", ResourceType::Video, "video/mp4"),
    ("avi", ResourceType::Video, "video/x-msvideo"),
    ("mov", ResourceType::Video, "video/quicktime"),
    ("wmv", ResourceType::Video, "video/x-ms-wmv"),
    ("webm", ResourceType::Video, "video/webm"),
];

fn extension_entry(ext: &str) -> Option<&'static (&'static str, ResourceType, &'static str)> {
    let ext = ext.to_ascii_lowercase();
    EXTENSIONS.iter().find(|(e, _, _)| *e == ext)
}

/// Type of resource.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceType {
    /// Image (PNG, JPEG, GIF, BMP, TIFF, WMF, EMF)
    Image,
    /// Audio file
    Audio,
    /// Video file
    Video,
    /// Chart (extracted as image)
    Chart,
    /// Embedded OLE object
    Ole,
    /// Other binary data
    Other,
}

impl ResourceType {
    /// Determine resource type from MIME type.
    pub fn from_mime_type(mime: &str) -> Self {
        let mime_lower = mime.to_lowercase();
        if mime_lower.starts_with("image/") {
            ResourceType::Image
        } else if mime_lower.starts_with("audio/") {
            ResourceType::Audio
        } else if mime_lower.starts_with("video/") {
            ResourceType::Video
        } else if mime_lower.contains("chart") {
            ResourceType::Chart
        } else if mime_lower.contains("ole") || mime_lower.contains("oleobject") {
            ResourceType::Ole
        } else {
            ResourceType::Other
        }
    }

    /// Determine resource type from file extension.
    pub fn from_extension(ext: &str) -> Self {
        extension_entry(ext).map_or(ResourceType::Other, |&(_, kind, _)| kind)
    }
}

/// What a resource is to the document's content.
///
/// A picture in an Office document can reference more than one file: the image it shows,
/// and companions of that image — the vector original of a raster picture, or an effects
/// layer composited onto it. A companion is not a picture of its own; taking every
/// resource as an image the document shows counts each picture two or three times, in
/// formats a reader may not decode.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceRole {
    /// An image the document shows, or a media file it plays.
    #[default]
    Primary,
    /// Another encoding of a primary image: the SVG original of a picture whose raster
    /// rendering (PNG, usually) is the primary. `companion_of` names the primary.
    Alternate,
    /// A layer composited onto a primary image: an HD Photo (JPEG XR, `.wdp`) layer that
    /// carries a picture's artistic effects. `companion_of` names the primary.
    Layer,
}

/// A binary resource (image, media file, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Resource {
    /// Resource type
    pub resource_type: ResourceType,

    /// Original filename (if known)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub filename: Option<String>,

    /// MIME type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,

    /// Binary data
    #[serde(skip)]
    pub data: Vec<u8>,

    /// Size in bytes
    pub size: usize,

    /// Width in pixels (for images)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,

    /// Height in pixels (for images)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,

    /// Alt text / description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub alt_text: Option<String>,

    /// What this resource is to the document's content. Only a `Primary` resource is an
    /// image the document shows (or media it plays).
    #[serde(default)]
    pub role: ResourceRole,

    /// For an `Alternate` or a `Layer`, the id of the primary resource it belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub companion_of: Option<String>,
}

impl Resource {
    /// Create a new resource.
    pub fn new(resource_type: ResourceType, data: Vec<u8>) -> Self {
        let size = data.len();
        Self {
            resource_type,
            filename: None,
            mime_type: None,
            data,
            size,
            width: None,
            height: None,
            alt_text: None,
            role: ResourceRole::Primary,
            companion_of: None,
        }
    }

    /// Create a resource from a package part: type and MIME type from its extension,
    /// pixel size from the image header when the format is one this reads (PNG, JPEG,
    /// GIF, BMP).
    pub fn from_part(part_path: &str, data: Vec<u8>) -> Self {
        let filename = part_path
            .rsplit('/')
            .next()
            .unwrap_or(part_path)
            .to_string();
        let entry = filename
            .rsplit_once('.')
            .and_then(|(_, ext)| extension_entry(ext));
        let (width, height) = image_dimensions(&data).unzip();
        Self {
            resource_type: entry.map_or(ResourceType::Other, |&(_, kind, _)| kind),
            mime_type: entry.map(|&(_, _, mime)| mime.to_string()),
            filename: Some(filename),
            size: data.len(),
            data,
            width,
            height,
            alt_text: None,
            role: ResourceRole::Primary,
            companion_of: None,
        }
    }

    /// Create an image resource.
    pub fn image(data: Vec<u8>, filename: Option<String>) -> Self {
        let size = data.len();
        let mime_type = filename.as_ref().and_then(|f| Self::mime_from_filename(f));
        let (width, height) = image_dimensions(&data).unzip();
        Self {
            resource_type: ResourceType::Image,
            filename,
            mime_type,
            data,
            size,
            width,
            height,
            alt_text: None,
            role: ResourceRole::Primary,
            companion_of: None,
        }
    }

    /// Get the file extension for this resource.
    pub fn extension(&self) -> Option<&str> {
        self.filename.as_ref().and_then(|f| {
            f.rsplit('.')
                .next()
                .filter(|ext| ext.len() <= 5 && ext.chars().all(|c| c.is_alphanumeric()))
        })
    }

    /// Determine MIME type from filename.
    pub fn mime_from_filename(filename: &str) -> Option<String> {
        let (_, ext) = filename.rsplit_once('.')?;
        extension_entry(ext).map(|&(_, _, mime)| mime.to_string())
    }

    /// Generate a suggested filename for this resource.
    pub fn suggested_filename(&self, id: &str) -> String {
        if let Some(ref filename) = self.filename {
            filename.clone()
        } else {
            let ext = match self.resource_type {
                ResourceType::Image => self
                    .mime_type
                    .as_ref()
                    .and_then(|m| Self::extension_from_mime(m))
                    .unwrap_or("png"),
                ResourceType::Audio => "mp3",
                ResourceType::Video => "mp4",
                ResourceType::Chart => "png",
                ResourceType::Ole => "bin",
                ResourceType::Other => "bin",
            };
            format!("{}.{}", id, ext)
        }
    }

    /// Get extension from MIME type — the first extension the table lists for it.
    fn extension_from_mime(mime: &str) -> Option<&'static str> {
        EXTENSIONS
            .iter()
            .find(|(_, _, m)| *m == mime)
            .map(|&(ext, _, _)| ext)
    }

    /// Check if this is an image.
    pub fn is_image(&self) -> bool {
        matches!(
            self.resource_type,
            ResourceType::Image | ResourceType::Chart
        )
    }

    /// Check if this is a media file (audio/video).
    pub fn is_media(&self) -> bool {
        matches!(
            self.resource_type,
            ResourceType::Audio | ResourceType::Video
        )
    }
}

/// Pixel size of an image, read from its header. `None` for a format this does not read
/// (vector and metafile formats have no pixel size; TIFF and JPEG XR are not read) or a
/// header too short to hold one.
pub(crate) fn image_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    let be32 = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
    };
    let be16 = |at: usize| -> Option<u32> {
        Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?) as u32)
    };
    let le16 = |at: usize| -> Option<u32> {
        Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?) as u32)
    };
    let le32 = |at: usize| -> Option<i32> {
        Some(i32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?))
    };

    let size = if data.starts_with(b"\x89PNG\r\n\x1a\n") && data.get(12..16) == Some(b"IHDR") {
        (be32(16)?, be32(20)?)
    } else if data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a") {
        (le16(6)?, le16(8)?)
    } else if data.starts_with(b"BM") {
        // BITMAPINFOHEADER: a negative height means a top-down bitmap.
        (le32(18)?.unsigned_abs(), le32(22)?.unsigned_abs())
    } else if data.starts_with(&[0xFF, 0xD8]) {
        // Walk the JPEG segments to the first start-of-frame marker.
        let mut at = 2;
        loop {
            while data.get(at) == Some(&0xFF) && data.get(at + 1) == Some(&0xFF) {
                at += 1;
            }
            if data.get(at) != Some(&0xFF) {
                return None;
            }
            let marker = *data.get(at + 1)?;
            let is_sof = matches!(marker, 0xC0..=0xCF) && !matches!(marker, 0xC4 | 0xC8 | 0xCC);
            if is_sof {
                break (be16(at + 7)?, be16(at + 5)?);
            }
            // Markers without a length field.
            if matches!(marker, 0xD0..=0xD9 | 0x01) {
                at += 2;
                continue;
            }
            at += 2 + be16(at + 2)? as usize;
        }
    } else {
        return None;
    };
    (size.0 > 0 && size.1 > 0).then_some(size)
}

#[cfg(test)]
#[path = "../../tests/unit/model/resource.rs"]
mod tests;
