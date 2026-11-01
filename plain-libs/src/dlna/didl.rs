//! DIDL-Lite metadata builder. We don't need a full XML encoder for the
//! few fields we emit, so we hand-build the payload as a string. The
//! shape is byte-for-byte compatible with Go's `didlLiteMetadata`.

use crate::dlna::types::MediaType;
use crate::dlna::util::xml_escape;

/// Build a DIDL-Lite `<DIDL-Lite>` document describing a single media
/// item. Returned as a `String` (UTF-8 XML), suitable for inclusion in
/// the `CurrentURIMetaData` argument of `SetAVTransportURI`.
pub fn didl_lite_metadata(
    media_url: &str,
    title: &str,
    mime: &str,
    media_type: MediaType,
) -> String {
    metadata(media_url, title, mime, media_type, "", false)
}

pub fn metadata(
    media_url: &str,
    title: &str,
    mime: &str,
    media_type: MediaType,
    album_art: &str,
    compact: bool,
) -> String {
    let upnp_class = match media_type {
        MediaType::Video => "object.item.videoItem",
        MediaType::Audio => "object.item.audioItem.musicTrack",
        MediaType::Image => "object.item.imageItem.photo",
        MediaType::Unknown => "object.item",
    };

    let mime = if mime.trim().is_empty() {
        "application/octet-stream"
    } else {
        mime
    };

    let title_esc = xml_escape(title);
    let url_esc = xml_escape(media_url);

    let art = if album_art.is_empty() {
        String::new()
    } else {
        format!(
            "<upnp:albumArtURI>{}</upnp:albumArtURI>",
            xml_escape(album_art)
        )
    };
    let parent = if compact { "-1" } else { "0" };
    let restricted = if compact { "0" } else { "1" };
    let declaration = if compact {
        ""
    } else {
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>"
    };
    let dlna = if compact {
        ""
    } else {
        " xmlns:dlna=\"urn:schemas-dlna-org:metadata-1-0/\""
    };
    format!(
        r#"{declaration}<DIDL-Lite xmlns="urn:schemas-upnp-org:metadata-1-0/DIDL-Lite/" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:upnp="urn:schemas-upnp-org:metadata-1-0/upnp/"{dlna}><item id="0" parentID="{parent}" restricted="{restricted}"><dc:title>{title_esc}</dc:title><upnp:class>{upnp_class}</upnp:class><res protocolInfo="http-get:*:{mime}:*">{url_esc}</res>{art}</item></DIDL-Lite>"#
    )
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/dlna/didl.rs"]
mod tests;
