pub(super) struct Bits<'a> {
    bytes: &'a [u8],
    position: usize,
}
impl<'a> Bits<'a> {
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, position: 0 }
    }
    pub fn read(&mut self, count: usize) -> Option<u32> {
        if count > 32 || self.position.checked_add(count)? > self.bytes.len().checked_mul(8)? {
            return None;
        }
        let mut value = 0;
        for _ in 0..count {
            value = (value << 1)
                | u32::from((self.bytes[self.position / 8] >> (7 - self.position % 8)) & 1);
            self.position += 1;
        }
        Some(value)
    }
    pub fn flag(&mut self) -> Option<bool> {
        Some(self.read(1)? != 0)
    }
    pub fn unsigned(&mut self) -> Option<u32> {
        let mut zeros = 0;
        while !self.flag()? {
            zeros += 1;
            if zeros > 31 {
                return None;
            }
        }
        ((1u32 << zeros) - 1).checked_add(self.read(zeros)?)
    }
    pub fn signed(&mut self) -> Option<i64> {
        let value = u64::from(self.unsigned()?);
        Some(if value % 2 == 0 {
            -(value as i64 / 2)
        } else {
            (value as i64 + 1) / 2
        })
    }
}

pub(super) fn rbsp(nal: &[u8]) -> Option<Vec<u8>> {
    if nal.len() > 64 * 1024 {
        return None;
    }
    let mut bytes = Vec::with_capacity(nal.len());
    let mut zeros = 0;
    let mut offset = 0;
    while offset < nal.len() {
        let byte = nal[offset];
        if zeros == 2 && byte == 3 {
            if *nal.get(offset + 1)? > 3 {
                return None;
            }
            zeros = 0;
            offset += 1;
            continue;
        }
        bytes.push(byte);
        zeros = if byte == 0 { (zeros + 1).min(2) } else { 0 };
        offset += 1;
    }
    Some(bytes)
}
