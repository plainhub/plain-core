use std::io::{Read, Seek, SeekFrom};

#[derive(Debug)]
pub(super) struct StreamInfo {
    pub width: u32,
    pub height: u32,
    pub frame_rate: Option<f64>,
}
#[derive(Default, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    pub width: u32,
    pub height: u32,
    pub rotation: i32,
    pub duration_ms: i64,
    pub bitrate: i64,
    pub frame_rate: f64,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: String,
    pub genre: String,
    pub writer: String,
    pub composer: String,
    pub raw_taken_at: Option<String>,
    pub raw_location: Option<String>,
}

pub fn read(reader: &mut (impl Read + Seek)) -> Option<VideoInfo> {
    reader.seek(SeekFrom::Start(0)).ok()?;
    let mut signature = [0; 4];
    reader.read_exact(&mut signature).ok()?;
    if signature == [0x1a, 0x45, 0xdf, 0xa3] {
        super::matroska::video(reader)
    } else if signature == super::asf::HEADER[..4] {
        super::asf::video(reader)
    } else if &signature[..3] == b"FLV" {
        super::flv::video(reader)
    } else if &signature == b"RIFF" {
        super::avi::video(reader)
    } else {
        super::isobmff::video_reader(reader).or_else(|| super::transport_stream::video(reader))
    }
}
