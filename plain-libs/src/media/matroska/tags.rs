use super::ebml::{Element, Parser};
use crate::media::video_info::VideoInfo;
use std::io::{Read, Seek};

fn simple<R: Read + Seek>(
    parser: &mut Parser<'_, R>,
    parent: Element,
    depth: u8,
    result: &mut VideoInfo,
) -> Option<()> {
    if depth > 8 {
        return None;
    }
    let mut name = None;
    let mut value = None;
    let mut default = true;
    let mut nested = Vec::new();
    parser.children(parent, |parser, element| {
        match element.id {
            0x45a3 => name = Some(parser.text(element)?),
            0x4487 if element.end - element.data <= 64 * 1024 => {
                value = Some(parser.text(element)?)
            }
            0x4484 => default = parser.unsigned(element)? != 0,
            0x67c8 => nested.push(element),
            _ => {}
        }
        Some(())
    })?;
    if default && let (Some(name), Some(value)) = (name, value) {
        match name.to_ascii_uppercase().as_str() {
            "TITLE" => result.title = value,
            "ARTIST" | "LEAD_PERFORMER" => result.artist = value,
            "ALBUM" => result.album = value,
            "GENRE" => result.genre = value,
            "WRITTEN_BY" => result.writer = value,
            "COMPOSER" => result.composer = value,
            "DATE_RECORDED" => result.raw_taken_at = Some(value),
            _ => {}
        }
    }
    for element in nested {
        simple(parser, element, depth + 1, result)?;
    }
    Some(())
}
pub(super) fn apply<R: Read + Seek>(
    parser: &mut Parser<'_, R>,
    parent: Element,
    uid: u64,
    result: &mut VideoInfo,
) -> Option<()> {
    parser.children(parent, |parser, tag| {
        if tag.id != 0x7373 {
            return Some(());
        }
        let mut has_track_target = false;
        let mut track_matches = false;
        let mut other_target = false;
        let mut roots = Vec::new();
        parser.children(tag, |parser, element| {
            match element.id {
                0x63c0 => parser.children(element, |parser, target| {
                    match target.id {
                        0x63c5 => {
                            let target_uid = parser.unsigned(target)?;
                            has_track_target = true;
                            track_matches |= target_uid == 0 || (uid != 0 && target_uid == uid);
                        }
                        0x63c4 | 0x63c6 | 0x63c9 => other_target = true,
                        _ => {}
                    }
                    Some(())
                })?,
                0x67c8 => roots.push(element),
                _ => {}
            }
            Some(())
        })?;
        if !other_target && (!has_track_target || track_matches) {
            for root in roots {
                simple(parser, root, 0, result)?;
            }
        }
        Some(())
    })
}
