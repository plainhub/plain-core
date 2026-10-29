use super::packet::{Continuity, Order, Packet};
use super::{Codec, StreamInfo, elementary};

const MAX_HEADER_BYTES: usize = 264;
pub(super) const MAX_ELEMENTARY_BYTES: usize = 64 * 1024;

enum Pes {
    Idle,
    Header(Vec<u8>),
    Payload(Option<usize>),
}

pub(super) struct Consumer {
    pub(super) codec: Codec,
    pub prefix: Vec<u8>,
    pub video: Option<StreamInfo>,
    pes: Pes,
    continuity: Continuity,
    #[cfg(test)]
    scans: usize,
}

impl Consumer {
    pub fn new(codec: Codec) -> Self {
        Self {
            codec,
            prefix: Vec::new(),
            video: None,
            pes: Pes::Idle,
            continuity: Continuity::default(),
            #[cfg(test)]
            scans: 0,
        }
    }

    pub fn reset(&mut self) {
        self.prefix.clear();
        self.pes = Pes::Idle;
    }

    pub fn packet(&mut self, packet: Packet<'_>, bytes: &[u8; 188]) {
        if packet.discontinuity {
            self.reset();
            self.continuity.reset();
        }
        if packet.payload.is_empty() {
            return;
        }
        match self.continuity.check(packet, bytes) {
            Order::Duplicate => return,
            Order::Gap => self.reset(),
            Order::Next => {}
        }
        if packet.start {
            self.prefix.clear();
            self.pes = Pes::Header(Vec::new());
        }
        self.consume(packet.payload);
    }

    fn consume(&mut self, bytes: &[u8]) {
        match std::mem::replace(&mut self.pes, Pes::Idle) {
            Pes::Idle => {}
            Pes::Header(mut header) => {
                let count = bytes.len().min(MAX_HEADER_BYTES - header.len());
                header.extend_from_slice(&bytes[..count]);
                match header_length(&header) {
                    Header::Ready(length, remaining) => {
                        self.pes = Pes::Payload(remaining);
                        self.consume(&header[length..]);
                        self.consume(&bytes[count..]);
                    }
                    Header::More if header.len() < MAX_HEADER_BYTES => {
                        self.pes = Pes::Header(header)
                    }
                    _ => self.reset(),
                }
            }
            Pes::Payload(remaining) => {
                let count = remaining.unwrap_or(bytes.len()).min(bytes.len());
                self.push(&bytes[..count]);
                self.pes = match remaining {
                    Some(length) if length == count => Pes::Idle,
                    value => Pes::Payload(value.map(|length| length - count)),
                };
            }
        }
    }

    pub fn push(&mut self, bytes: &[u8]) {
        if self.video.is_some() {
            return;
        }
        let count = bytes
            .len()
            .min(MAX_ELEMENTARY_BYTES.saturating_sub(self.prefix.len()));
        if count == 0 {
            return;
        }
        self.prefix.extend_from_slice(&bytes[..count]);
        #[cfg(test)]
        {
            self.scans += 1;
        }
        self.video = elementary(self.codec, &self.prefix);
    }
}

enum Header {
    More,
    Invalid,
    Ready(usize, Option<usize>),
}

fn header_length(bytes: &[u8]) -> Header {
    if bytes.len() < 7 {
        return Header::More;
    }
    if bytes[..3] != [0, 0, 1] {
        return Header::Invalid;
    }
    let length = if bytes[6] & 0xc0 == 0x80 {
        if bytes.len() < 9 {
            return Header::More;
        }
        let required = match bytes[7] >> 6 {
            0 => 0,
            2 => 5,
            3 => 10,
            _ => return Header::Invalid,
        };
        if usize::from(bytes[8]) < required {
            return Header::Invalid;
        }
        9 + usize::from(bytes[8])
    } else {
        let mut offset = 6;
        while bytes.get(offset) == Some(&0xff) {
            offset += 1;
        }
        let Some(byte) = bytes.get(offset) else {
            return Header::More;
        };
        if byte & 0xc0 == 0x40 {
            offset += 2;
        }
        match bytes.get(offset) {
            Some(byte) if byte >> 4 == 2 => offset + 5,
            Some(byte) if byte >> 4 == 3 => offset + 10,
            Some(0x0f) => offset + 1,
            Some(_) => return Header::Invalid,
            None => return Header::More,
        }
    };
    if length > MAX_HEADER_BYTES {
        return Header::Invalid;
    }
    let packet_length = usize::from(u16::from_be_bytes([bytes[4], bytes[5]]));
    let remaining = if packet_length == 0 {
        None
    } else {
        let Some(length) = packet_length.checked_sub(length - 6) else {
            return Header::Invalid;
        };
        Some(length)
    };
    if bytes.len() < length {
        Header::More
    } else {
        Header::Ready(length, remaining)
    }
}

#[cfg(test)]
#[path = "../../../tests/unit/media/transport_stream/pes.rs"]
mod tests;
