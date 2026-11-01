use std::io::{Read, Seek, SeekFrom};

#[derive(Clone, Copy)]
pub(super) struct Element {
    pub id: u32,
    pub data: u64,
    pub end: u64,
    pub unknown: bool,
}

pub(super) struct Parser<'a, R> {
    reader: &'a mut R,
    elements: usize,
    bytes: usize,
}
impl<'a, R: Read + Seek> Parser<'a, R> {
    pub fn new(reader: &'a mut R) -> Self {
        Self {
            reader,
            elements: 100_000,
            bytes: 4 * 1024 * 1024,
        }
    }
    fn read(&mut self, bytes: &mut [u8]) -> Option<()> {
        self.bytes = self.bytes.checked_sub(bytes.len())?;
        self.reader.read_exact(bytes).ok()
    }
    fn vint(&mut self, id: bool) -> Option<(u64, bool)> {
        let mut first = [0];
        self.read(&mut first)?;
        let width = first[0].leading_zeros() + 1;
        if width > if id { 4 } else { 8 } {
            return None;
        }
        let mut value = u64::from(if id {
            first[0]
        } else {
            first[0] & (0xffu16 >> width) as u8
        });
        for _ in 1..width {
            self.read(&mut first)?;
            value = value.checked_mul(256)?.checked_add(u64::from(first[0]))?;
        }
        Some((value, !id && value == (1u64 << (7 * width)) - 1))
    }
    pub fn element(&mut self, offset: u64, limit: u64) -> Option<Element> {
        self.elements = self.elements.checked_sub(1)?;
        if offset >= limit || limit.checked_sub(offset)? < 2 {
            return None;
        }
        self.reader.seek(SeekFrom::Start(offset)).ok()?;
        let (id, _) = self.vint(true)?;
        let (size, unknown) = self.vint(false)?;
        let data = self.reader.stream_position().ok()?;
        let id = u32::try_from(id).ok()?;
        if unknown && !matches!(id, 0x18538067 | 0x1f43b675) {
            return None;
        }
        let end = if unknown {
            limit
        } else {
            data.checked_add(size)?
        };
        if data > limit || end > limit {
            return None;
        }
        Some(Element {
            id,
            data,
            end,
            unknown,
        })
    }
    pub fn children(
        &mut self,
        parent: Element,
        mut visit: impl FnMut(&mut Self, Element) -> Option<()>,
    ) -> Option<()> {
        if parent.unknown {
            return None;
        }
        let mut offset = parent.data;
        while offset < parent.end {
            let child = self.element(offset, parent.end)?;
            offset = child.end;
            visit(self, child)?;
        }
        Some(())
    }
    pub fn bytes(&mut self, element: Element, maximum: usize) -> Option<Vec<u8>> {
        let size = usize::try_from(element.end.checked_sub(element.data)?).ok()?;
        if element.unknown || size > maximum || size > self.bytes {
            return None;
        }
        let mut data = vec![0; size];
        self.reader.seek(SeekFrom::Start(element.data)).ok()?;
        self.read(&mut data)?;
        Some(data)
    }
    pub fn unsigned(&mut self, element: Element) -> Option<u64> {
        Some(
            self.bytes(element, 8)?
                .into_iter()
                .fold(0, |value, byte| (value << 8) | u64::from(byte)),
        )
    }
    pub fn float(&mut self, element: Element) -> Option<f64> {
        let bytes = self.bytes(element, 8)?;
        let value = match bytes.len() {
            0 => 0.0,
            4 => f64::from(f32::from_be_bytes(bytes.try_into().ok()?)),
            8 => f64::from_be_bytes(bytes.try_into().ok()?),
            _ => return None,
        };
        value.is_finite().then_some(value)
    }
    pub fn text(&mut self, element: Element) -> Option<String> {
        String::from_utf8(self.bytes(element, 64 * 1024)?)
            .ok()
            .map(|s| s.trim_end_matches('\0').to_owned())
    }
    pub fn next_segment_element(&mut self, cluster: Element) -> Option<u64> {
        if !cluster.unknown {
            return Some(cluster.end);
        }
        let mut offset = cluster.data;
        while offset < cluster.end {
            let child = self.element(offset, cluster.end)?;
            if matches!(
                child.id,
                0x114d9b74
                    | 0x1549a966
                    | 0x1654ae6b
                    | 0x1f43b675
                    | 0x1c53bb6b
                    | 0x1941a469
                    | 0x1043a770
                    | 0x1254c367
            ) {
                return Some(offset);
            }
            if child.unknown {
                return None;
            }
            offset = child.end;
        }
        Some(offset)
    }
}
