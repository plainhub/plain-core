use super::video_info::{StreamInfo, VideoInfo};
use std::io::{BufReader, Read, Seek, SeekFrom};

mod codec;
mod packet;
mod pes;
use codec::{Codec, elementary};
use packet::{Packet, Sections, valid_section};
use pes::Consumer;

const MAX_PACKETS: u64 = 4096;

#[derive(Clone, Copy)]
struct Program {
    number: u16,
    pid: u16,
}
struct Video {
    pid: u16,
    consumer: Consumer,
}
#[derive(Default)]
struct Context {
    pat: Sections,
    pmt: Sections,
    program: Option<Program>,
    video: Option<Video>,
}

impl Context {
    fn packet(&mut self, bytes: &[u8; 188]) -> Option<StreamInfo> {
        let packet = Packet::read(bytes)?;
        if packet.pid == 0 {
            for section in self.pat.packet(packet, bytes) {
                self.pat(&section);
            }
        } else if self
            .program
            .is_some_and(|program| program.pid == packet.pid)
        {
            for section in self.pmt.packet(packet, bytes) {
                self.pmt(&section);
            }
        } else if let Some(video) = &mut self.video {
            if video.pid == packet.pid {
                video.consumer.packet(packet, bytes);
                return video.consumer.video.take();
            }
        }
        None
    }

    fn pat(&mut self, bytes: &[u8]) {
        if self.program.is_some() || !valid_section(bytes, 0, 12) {
            return;
        }
        let entries = &bytes[8..bytes.len() - 4];
        if entries.len() % 4 != 0 {
            return;
        }
        for entry in entries.chunks_exact(4) {
            let number = u16::from_be_bytes([entry[0], entry[1]]);
            let pid = (u16::from(entry[2] & 31) << 8) | u16::from(entry[3]);
            if number != 0 && pid != 0 && pid != 0x1fff {
                self.program = Some(Program { number, pid });
                break;
            }
        }
    }

    fn pmt(&mut self, bytes: &[u8]) {
        if !valid_section(bytes, 2, 16) {
            return;
        }
        let Some(program) = self.program else {
            return;
        };
        if u16::from_be_bytes([bytes[3], bytes[4]]) != program.number {
            return;
        }
        let end = bytes.len() - 4;
        let mut offset = 12 + ((usize::from(bytes[10] & 15) << 8) | usize::from(bytes[11]));
        if offset > end {
            return;
        }
        let mut selected = None;
        while offset < end {
            if end - offset < 5 {
                return;
            }
            let entry = &bytes[offset..end];
            let pid = (u16::from(entry[1] & 31) << 8) | u16::from(entry[2]);
            let length = (usize::from(entry[3] & 15) << 8) | usize::from(entry[4]);
            offset += 5 + length;
            if offset > end {
                return;
            }
            let codec = match entry[0] {
                0x1b => Codec::H264,
                0x24 => Codec::H265,
                1 | 2 => Codec::Mpeg,
                _ => continue,
            };
            if selected.is_none()
                && self.video.as_ref().is_none_or(|video| video.pid == pid)
                && pid != 0x1fff
            {
                selected = Some((pid, codec));
            }
        }
        if let Some((pid, codec)) = selected {
            if self
                .video
                .as_ref()
                .is_none_or(|video| video.pid != pid || video.consumer.codec != codec)
            {
                self.video = Some(Video {
                    pid,
                    consumer: Consumer::new(codec),
                });
            }
        }
    }
}

fn layout(bytes: &[u8]) -> Option<(usize, usize)> {
    for stride in [188, 192, 204] {
        for offset in 0..stride {
            if (0..3).all(|index| bytes.get(offset + index * stride) == Some(&0x47)) {
                return Some((offset, stride));
            }
        }
    }
    None
}
pub(super) fn video(reader: &mut (impl Read + Seek)) -> Option<VideoInfo> {
    let length = reader.seek(SeekFrom::End(0)).ok()?;
    reader.seek(SeekFrom::Start(0)).ok()?;
    let mut probe = vec![0; length.min(1024) as usize];
    reader.read_exact(&mut probe).ok()?;
    let (offset, stride) = layout(&probe)?;
    reader.seek(SeekFrom::Start(offset as u64)).ok()?;
    let packets = length.checked_sub(offset as u64)?.checked_sub(188)? / stride as u64 + 1;
    let mut reader = BufReader::with_capacity(stride * 32, reader);
    let mut context = Context::default();
    let mut packet = [0; 188];
    let mut padding = [0; 16];
    for index in 0..packets.min(MAX_PACKETS) {
        reader.read_exact(&mut packet).ok()?;
        if packet[0] != 0x47 {
            return None;
        }
        if let Some(video) = context.packet(&packet) {
            return Some(VideoInfo {
                width: video.width,
                height: video.height,
                frame_rate: video.frame_rate.unwrap_or_default(),
                ..Default::default()
            });
        }
        if index + 1 < packets {
            reader.read_exact(&mut padding[..stride - 188]).ok()?;
        }
    }
    None
}

#[cfg(test)]
#[path = "../../tests/unit/media/transport_stream.rs"]
mod tests;
