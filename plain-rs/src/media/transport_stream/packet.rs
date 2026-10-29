#[derive(Clone, Copy)]
pub(super) struct Packet<'a> {
    pub pid: u16,
    pub start: bool,
    pub counter: u8,
    pub discontinuity: bool,
    pub has_pcr: bool,
    pub payload: &'a [u8],
}

impl<'a> Packet<'a> {
    pub fn read(bytes: &'a [u8; 188]) -> Option<Self> {
        if bytes[0] != 0x47 || bytes[1] & 0x80 != 0 || bytes[3] & 0xc0 != 0 {
            return None;
        }
        let control = (bytes[3] >> 4) & 3;
        if control == 0 {
            return None;
        }
        let mut offset = 4;
        let mut discontinuity = false;
        let mut has_pcr = false;
        if control & 2 != 0 {
            offset = 5 + usize::from(bytes[4]);
            if (control == 2 && bytes[4] != 183) || (control == 3 && bytes[4] > 182) {
                return None;
            }
            discontinuity = bytes[4] > 0 && bytes[5] & 0x80 != 0;
            has_pcr = bytes[4] > 0 && bytes[5] & 0x10 != 0;
            if has_pcr && bytes[4] < 7 {
                return None;
            }
        }
        Some(Self {
            pid: (u16::from(bytes[1] & 31) << 8) | u16::from(bytes[2]),
            start: bytes[1] & 0x40 != 0,
            counter: bytes[3] & 15,
            discontinuity,
            has_pcr,
            payload: if control & 1 != 0 {
                &bytes[offset..]
            } else {
                &[]
            },
        })
    }
}

#[derive(Default)]
pub(super) struct Continuity(Option<[u8; 188]>);

pub(super) enum Order {
    Next,
    Duplicate,
    Gap,
}

impl Continuity {
    pub fn reset(&mut self) {
        self.0 = None;
    }

    pub fn check(&mut self, packet: Packet<'_>, bytes: &[u8; 188]) -> Order {
        let mut content = *bytes;
        // H.222.0 §2.4.3.3 permits updated PCR values in otherwise identical duplicate packets.
        if packet.has_pcr {
            content[6..12].fill(0);
        }
        let previous = self.0.replace(content);
        match previous {
            Some(previous) if previous[3] & 15 == packet.counter && previous == content => {
                Order::Duplicate
            }
            Some(previous) if ((previous[3] & 15) + 1) & 15 != packet.counter => Order::Gap,
            _ => Order::Next,
        }
    }
}

#[derive(Default)]
pub(super) struct Sections {
    bytes: Vec<u8>,
    continuity: Continuity,
}

impl Sections {
    pub fn packet(&mut self, packet: Packet<'_>, bytes: &[u8; 188]) -> Vec<Vec<u8>> {
        if packet.discontinuity {
            self.bytes.clear();
            self.continuity.reset();
        }
        if packet.payload.is_empty() {
            return Vec::new();
        }
        match self.continuity.check(packet, bytes) {
            Order::Duplicate => return Vec::new(),
            Order::Gap => self.bytes.clear(),
            Order::Next => {}
        }
        let mut result = Vec::new();
        if packet.start {
            let pointer = usize::from(packet.payload[0]);
            let Some(next) = packet.payload.get(1 + pointer..) else {
                self.bytes.clear();
                return result;
            };
            if !self.bytes.is_empty() {
                self.append(&packet.payload[1..1 + pointer], &mut result);
            }
            self.bytes.clear();
            self.append(next, &mut result);
        } else if !self.bytes.is_empty() {
            self.append(packet.payload, &mut result);
        }
        result
    }

    fn append(&mut self, mut bytes: &[u8], result: &mut Vec<Vec<u8>>) {
        while !bytes.is_empty() {
            if self.bytes.is_empty() && bytes[0] == 0xff {
                break;
            }
            let length = if self.bytes.len() < 3 {
                3
            } else {
                3 + ((usize::from(self.bytes[1] & 15) << 8) | usize::from(self.bytes[2]))
            };
            let count = (length - self.bytes.len()).min(bytes.len());
            self.bytes.extend_from_slice(&bytes[..count]);
            bytes = &bytes[count..];
            if self.bytes.len() == 3 {
                let length = (usize::from(self.bytes[1] & 15) << 8) | usize::from(self.bytes[2]);
                if !(9..=1021).contains(&length) {
                    self.bytes.clear();
                    break;
                }
            } else if self.bytes.len() == length {
                result.push(std::mem::take(&mut self.bytes));
            }
        }
    }
}

pub(super) fn valid_section(bytes: &[u8], table: u8, minimum: usize) -> bool {
    if bytes.len() < minimum
        || bytes[0] != table
        || bytes[1] & 0xf0 != 0xb0
        || bytes[5] & 1 == 0
        || bytes[6] > bytes[7]
    {
        return false;
    }
    crc(bytes) == 0
}

pub(super) fn crc(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 == 0 {
                crc << 1
            } else {
                (crc << 1) ^ 0x04c1_1db7
            };
        }
    }
    crc
}

#[cfg(test)]
#[path = "../../../tests/unit/media/transport_stream/packet.rs"]
mod tests;
