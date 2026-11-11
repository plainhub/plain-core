use bytes::Buf;
use std::io::{Cursor, Read, Result as IoResult};
#[derive(Debug)]
pub struct ReadBuffer<const CHUNK_SIZE: usize> {
    storage: Cursor<Vec<u8>>,
    chunk: Box<[u8; CHUNK_SIZE]>,
}
impl<const CHUNK_SIZE: usize> ReadBuffer<CHUNK_SIZE> {
    pub fn new() -> Self {
        Self::with_capacity(CHUNK_SIZE)
    }
    pub fn with_capacity(capacity: usize) -> Self {
        Self::from_partially_read(Vec::with_capacity(capacity))
    }
    pub fn from_partially_read(part: Vec<u8>) -> Self {
        Self {
            storage: Cursor::new(part),
            chunk: Box::new([0; CHUNK_SIZE]),
        }
    }
    pub fn as_cursor(&self) -> &Cursor<Vec<u8>> {
        &self.storage
    }
    pub fn as_cursor_mut(&mut self) -> &mut Cursor<Vec<u8>> {
        &mut self.storage
    }
    pub fn into_vec(mut self) -> Vec<u8> {
        self.clean_up();
        self.storage.into_inner()
    }
    pub fn read_from<S: Read>(&mut self, stream: &mut S) -> IoResult<usize> {
        self.clean_up();
        let size = stream.read(&mut *self.chunk)?;
        self.storage
            .get_mut()
            .extend_from_slice(&self.chunk[..size]);
        Ok(size)
    }
    fn clean_up(&mut self) {
        let pos = self.storage.position() as usize;
        self.storage.get_mut().drain(0..pos).count();
        self.storage.set_position(0);
    }
}
impl<const CHUNK_SIZE: usize> Buf for ReadBuffer<CHUNK_SIZE> {
    fn remaining(&self) -> usize {
        Buf::remaining(self.as_cursor())
    }
    fn chunk(&self) -> &[u8] {
        Buf::chunk(self.as_cursor())
    }
    fn advance(&mut self, cnt: usize) {
        Buf::advance(self.as_cursor_mut(), cnt);
    }
}
impl<const CHUNK_SIZE: usize> Default for ReadBuffer<CHUNK_SIZE> {
    fn default() -> Self {
        Self::new()
    }
}
