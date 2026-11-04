mod id3;

use lofty::config::ParseOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::probe::Probe;
use lofty::tag::{Accessor, TagType};
use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
};

#[derive(Default)]
pub struct ProbedMeta {
    pub duration_secs: u32,
    pub duration_ms: i64,
    pub duration_available: bool,
    pub artist: String,
    pub title: String,
    pub album: String,
    pub album_artist: String,
    /// False preserves existing tags when the file cannot be parsed.
    pub parsed: bool,
}

pub fn probe_media(path: &str, kind: &str) -> ProbedMeta {
    if !matches!(kind, "audio" | "video") {
        return ProbedMeta::default();
    }
    if kind != "audio" {
        let duration = probe_duration_milliseconds(path);
        return ProbedMeta {
            duration_secs: seconds(duration.unwrap_or_default()),
            duration_ms: duration.unwrap_or_default(),
            duration_available: duration.is_some(),
            artist: String::new(),
            title: String::new(),
            album: String::new(),
            album_artist: String::new(),
            parsed: duration.is_some(),
        };
    }
    let Ok(mut reader) = File::open(path) else {
        return ProbedMeta::default();
    };
    probe_audio_reader_named(&mut reader, path)
}

pub fn probe_audio_reader(reader: &mut (impl Read + Seek)) -> ProbedMeta {
    probe_audio(reader, None)
}

pub fn probe_audio_reader_named(reader: &mut (impl Read + Seek), path: &str) -> ProbedMeta {
    probe_audio(reader, Some(path))
}

fn probe_audio(reader: &mut (impl Read + Seek), path: Option<&str>) -> ProbedMeta {
    if reader.seek(SeekFrom::Start(0)).is_err() {
        return ProbedMeta::default();
    }
    let mut signature = [0; 16];
    if reader.read_exact(&mut signature).is_ok() && signature == super::asf::HEADER {
        return super::asf::metadata(reader)
            .map(|(facts, duration)| ProbedMeta {
                duration_secs: seconds(duration.unwrap_or_default()),
                duration_ms: duration.unwrap_or_default(),
                duration_available: duration.is_some(),
                artist: facts.artist,
                title: facts.title,
                album: facts.album,
                album_artist: facts.album_artist,
                parsed: true,
            })
            .unwrap_or_default();
    }
    if reader.seek(SeekFrom::Start(0)).is_err() {
        return ProbedMeta::default();
    }
    let Ok(projected) = id3::reader(&mut *reader, true) else {
        return ProbedMeta::default();
    };
    let tags_complete = projected.tags_complete;
    let latin1_title_artist = projected.latin1_title_artist;
    let result = Probe::new(BufReader::new(projected))
        .options(ParseOptions::new().read_cover_art(false))
        .guess_file_type()
        .ok()
        .and_then(|probe| probe.read().ok());
    let (mut duration, tags, parsed) = match result {
        Some(file) => (
            i64::try_from(file.properties().duration().as_millis()).ok(),
            tag_strings(&file, path, latin1_title_artist),
            true,
        ),
        None => (None, TagStrings::default(), false),
    };
    if duration.is_none_or(|duration| duration == 0) {
        let raw = super::raw_audio::milliseconds(reader);
        let fallback = raw.or_else(|| {
            reader
                .seek(SeekFrom::End(0))
                .ok()
                .and_then(|length| super::isobmff::mp4_file_milliseconds(reader, 0, length))
        });
        duration = fallback.or(duration);
    }
    ProbedMeta {
        duration_secs: seconds(duration.unwrap_or_default()),
        duration_ms: duration.unwrap_or_default(),
        duration_available: duration.is_some(),
        artist: tags.artist,
        title: tags.title,
        album: tags.album,
        album_artist: tags.album_artist,
        parsed: parsed && tags_complete,
    }
}

fn seconds(milliseconds: i64) -> u32 {
    u32::try_from(milliseconds.max(0) / 1000).unwrap_or(u32::MAX)
}

pub fn probe_duration_secs(path: &str) -> Option<u32> {
    u32::try_from(probe_duration_milliseconds(path)? / 1000).ok()
}

pub fn probe_duration_milliseconds(path: &str) -> Option<i64> {
    let mut reader = File::open(path).ok()?;
    probe_duration_reader(&mut reader)
}

/// None is unreadable; Some(0) is parsed without a measurable duration.
pub fn probe_duration_reader(reader: &mut (impl Read + Seek)) -> Option<i64> {
    if let Some(duration) = super::raw_audio::milliseconds(reader) {
        return Some(duration);
    }
    if let Some(duration) = super::flv::milliseconds(reader) {
        return Some(duration);
    }
    if let Some(duration) = super::avi::milliseconds(reader) {
        return Some(duration);
    }
    if let Some(duration) = super::matroska::milliseconds(reader) {
        return Some(duration);
    }
    let length = reader.seek(SeekFrom::End(0)).ok()?;
    if let Some(duration) = super::isobmff::mp4_file_milliseconds(reader, 0, length) {
        return Some(duration);
    }
    reader.seek(SeekFrom::Start(0)).ok()?;
    let projected = id3::reader(reader, false).ok()?;
    let file = Probe::new(BufReader::new(projected))
        .options(ParseOptions::new().read_tags(false))
        .guess_file_type()
        .ok()?
        .read()
        .ok()?;
    i64::try_from(file.properties().duration().as_millis()).ok()
}

#[derive(Default)]
struct TagStrings {
    artist: String,
    title: String,
    album: String,
    album_artist: String,
}

fn tag_strings(
    tagged_file: &lofty::file::TaggedFile,
    path: Option<&str>,
    latin1_title_artist: bool,
) -> TagStrings {
    let mut fields = TagStrings::default();
    let tags = [
        TagType::Id3v2,
        TagType::Ape,
        TagType::Id3v1,
        TagType::VorbisComments,
        TagType::Mp4Ilst,
    ]
    .into_iter()
    .filter_map(|kind| tagged_file.tag(kind))
    .chain(
        tagged_file
            .primary_tag()
            .or_else(|| tagged_file.first_tag()),
    );
    for tag in tags {
        let decoded = (tag.tag_type() == TagType::Id3v1
            || (tag.tag_type() == TagType::Id3v2 && latin1_title_artist))
            .then(|| {
                let title = decode_gbk(tag.title()?.as_ref())?;
                let artist = decode_gbk(tag.artist()?.as_ref())?;
                let stem = std::path::Path::new(path?).file_stem()?.to_str()?;
                let (file_artist, file_title) = stem.split_once('-')?;
                (file_artist.trim() == artist && file_title.trim() == title)
                    .then_some((title, artist))
            })
            .flatten();
        for (target, value) in [
            (
                &mut fields.artist,
                decoded
                    .as_ref()
                    .map(|(_, artist)| std::borrow::Cow::Borrowed(artist.as_str()))
                    .or_else(|| tag.artist()),
            ),
            (
                &mut fields.title,
                decoded
                    .as_ref()
                    .map(|(title, _)| std::borrow::Cow::Borrowed(title.as_str()))
                    .or_else(|| tag.title()),
            ),
            (&mut fields.album, tag.album()),
            (
                &mut fields.album_artist,
                tag.get_string(&lofty::tag::ItemKey::AlbumArtist)
                    .map(std::borrow::Cow::Borrowed),
            ),
        ] {
            if target.is_empty() {
                if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
                    *target = value.trim().to_owned();
                }
            }
        }
    }
    fields
}

fn decode_gbk(text: &str) -> Option<String> {
    let bytes = text
        .chars()
        .map(|c| u8::try_from(c as u32))
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    encoding_rs::GBK
        .decode_without_bom_handling_and_without_replacement(&bytes)
        .map(|s| s.trim().to_owned())
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/media/probe.rs"]
mod tests;
