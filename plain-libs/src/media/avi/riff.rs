use std::io::{Read, Seek, SeekFrom};

#[derive(Clone, Copy)]
pub(super) struct Chunk {
    pub id: [u8; 4],
    pub data: u64,
    pub end: u64,
}
pub(super) struct Parser<'a, R> {
    reader: &'a mut R,
    chunks: usize,
    bytes: usize,
}
impl<'a, R: Read + Seek> Parser<'a, R> {
    pub fn new(reader: &'a mut R) -> Self {
        Self {
            reader,
            chunks: 100_000,
            bytes: 4 * 1024 * 1024,
        }
    }
    pub fn chunk(&mut self, offset: u64, limit: u64) -> Option<Chunk> {
        self.chunks = self.chunks.checked_sub(1)?;
        if limit.checked_sub(offset)? < 8 {
            return None;
        }
        self.reader.seek(SeekFrom::Start(offset)).ok()?;
        let mut header = [0; 8];
        self.reader.read_exact(&mut header).ok()?;
        let data = offset.checked_add(8)?;
        let end = data.checked_add(u64::from(u32::from_le_bytes(header[4..].try_into().ok()?)))?;
        if end > limit {
            return None;
        }
        Some(Chunk {
            id: header[..4].try_into().ok()?,
            data,
            end,
        })
    }
    pub fn children(
        &mut self,
        start: u64,
        end: u64,
        mut visit: impl FnMut(&mut Self, Chunk) -> Option<()>,
    ) -> Option<()> {
        let mut offset = start;
        while offset < end {
            let child = self.chunk(offset, end)?;
            offset = child.end.checked_add((child.end - child.data) % 2)?;
            if offset > end {
                return None;
            }
            visit(self, child)?;
        }
        Some(())
    }
    pub fn prefix(&mut self, chunk: Chunk, count: usize) -> Option<Vec<u8>> {
        if chunk.end - chunk.data < count as u64 {
            return None;
        }
        self.bytes = self.bytes.checked_sub(count)?;
        let mut bytes = vec![0; count];
        self.reader.seek(SeekFrom::Start(chunk.data)).ok()?;
        self.reader.read_exact(&mut bytes).ok()?;
        Some(bytes)
    }
    pub fn kind(&mut self, chunk: Chunk) -> Option<[u8; 4]> {
        self.prefix(chunk, 4)?.try_into().ok()
    }
    pub fn text(&mut self, chunk: Chunk) -> Option<String> {
        let size = usize::try_from(chunk.end - chunk.data).ok()?;
        if size > 64 * 1024 {
            return None;
        }
        String::from_utf8(self.prefix(chunk, size)?)
            .ok()
            .map(|value| value.trim_end_matches('\0').to_owned())
    }
}
