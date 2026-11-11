//! What DrawingML pictures say about the files they reference.
//!
//! A picture (`pic:pic` in Word, `p:pic` in PowerPoint, `xdr:pic` in Excel) names the
//! image it shows in its `a:blip r:embed`. The blip's extension list can name more files
//! that belong to that one picture:
//!
//! - `asvg:svgBlip r:embed` — the SVG original of a picture whose `a:blip` is its raster
//!   rendering. Office writes both and draws the SVG where it can.
//! - `a14:imgProps/a14:imgLayer r:embed` — an HD Photo (JPEG XR, `.wdp`) layer that
//!   carries the picture's artistic effects.
//!
//! These companions are relationships of the part like any image, so a parser that lists
//! a part's image relationships lists them as pictures of their own. This reads the part
//! once and says which relationship is which.

use std::collections::HashMap;

use plain_libs::xml::events::{BytesStart, Event};

use crate::model::ResourceRole;

/// The pictures of one package part, by relationship id.
#[derive(Debug, Default)]
pub(crate) struct PictureRefs {
    /// Companion relationship id → its role and the primary relationship id it belongs to.
    pub companions: HashMap<String, (ResourceRole, String)>,
    /// Primary relationship id → the picture's description (`docPr`/`cNvPr` `descr`),
    /// from the first picture that has one.
    pub alt_texts: HashMap<String, String>,
    /// Every relationship id a picture references, primary or companion, in order.
    pub referenced: Vec<String>,
}

/// Read the pictures of a part.
pub(crate) fn scan_pictures(xml: &str) -> PictureRefs {
    let mut refs = PictureRefs::default();
    let mut reader = crate::decode::reader_for(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();

    // The description of the picture being read, waiting for its blip.
    let mut pending_alt: Option<String> = None;
    // The primary relationship of the `a:blip` being read, while inside it.
    let mut open_blip: Option<String> = None;
    // Inside an `a:buBlip`: the picture of a list marker, which has no description of its own.
    let mut in_bullet = false;

    loop {
        let (e, is_start) = match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => (e, true),
            Ok(Event::Empty(e)) => (e, false),
            Ok(Event::End(e)) => {
                match e.name().local_name().as_ref() {
                    "blip" => open_blip = None,
                    "buBlip" => in_bullet = false,
                    // A shape that holds no picture takes its description with it: the
                    // next picture must not inherit it.
                    "sp" | "cxnSp" | "graphicFrame" => pending_alt = None,
                    _ => {}
                }
                buf.clear();
                continue;
            }
            Ok(Event::Eof) | Err(_) => break,
            Ok(_) => {
                buf.clear();
                continue;
            }
        };
        match e.name().local_name().as_ref() {
            "docPr" | "cNvPr" => {
                if let Some(descr) = attr(&e, "descr").filter(|d| !d.trim().is_empty()) {
                    pending_alt = Some(descr);
                }
            }
            // A new drawing starts without its predecessor's description.
            "drawing" => pending_alt = None,
            "buBlip" => in_bullet = is_start,
            "blip" => {
                if let Some(id) = attr(&e, "embed") {
                    if let Some(alt) = pending_alt.take().filter(|_| !in_bullet) {
                        refs.alt_texts.entry(id.clone()).or_insert(alt);
                    }
                    refs.referenced.push(id.clone());
                    if is_start {
                        open_blip = Some(id);
                    }
                }
            }
            "svgBlip" | "imgLayer" => {
                if let (Some(primary), Some(id)) = (&open_blip, attr(&e, "embed")) {
                    let role = if e.name().local_name().as_ref() == "svgBlip" {
                        ResourceRole::Alternate
                    } else {
                        ResourceRole::Layer
                    };
                    refs.referenced.push(id.clone());
                    refs.companions.insert(id, (role, primary.clone()));
                }
            }
            _ => {}
        }
        buf.clear();
    }
    refs
}

/// The relationship id of the picture a part's background is filled with: the blip of a
/// PowerPoint `p:bg` (`a:blipFill`) or the `v:fill`/`v:imagedata` of a Word `w:background`.
/// `None` when the background is a colour or gradient, or the part has none.
#[cfg(any(feature = "docx", feature = "pptx"))]
pub(crate) fn background_picture(xml: &str) -> Option<String> {
    let mut reader = crate::decode::reader_for(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut in_background = false;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) if in_background => {
                let found = match e.name().local_name().as_ref() {
                    "blip" => attr(&e, "embed"),
                    "fill" | "imagedata" => attr(&e, "id"),
                    _ => None,
                };
                if found.is_some() {
                    return found;
                }
            }
            Ok(Event::Start(e))
                if matches!(e.name().local_name().as_ref(), "bg" | "background") =>
            {
                in_background = true;
            }
            Ok(Event::End(e)) if matches!(e.name().local_name().as_ref(), "bg" | "background") => {
                in_background = false;
            }
            Ok(Event::Eof) | Err(_) => return None,
            _ => {}
        }
        buf.clear();
    }
}

/// An attribute's value by local name, with entities resolved.
fn attr(e: &BytesStart<'_>, local: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.local_name().as_ref() == local)
        .map(|a| crate::decode::attr_value(&a))
}

#[cfg(test)]
#[path = "../tests/unit/drawing.rs"]
mod tests;
