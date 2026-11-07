use crate::audio_tags::error::Result;
use crate::audio_tags::util::math::F80;

use std::io::{Read, Seek};

// TODO: https://github.com/rust-lang/rust/issues/59359
pub(crate) trait SeekStreamLen: Seek {
    fn stream_len_hack(&mut self) -> crate::audio_tags::error::Result<u64> {
        use std::io::SeekFrom;

        let current_pos = self.stream_position()?;
        let len = self.seek(SeekFrom::End(0))?;

        self.seek(SeekFrom::Start(current_pos))?;

        Ok(len)
    }
}

impl<T> SeekStreamLen for T where T: Seek {}

pub(crate) trait ReadExt: Read {
    fn read_f80(&mut self) -> Result<F80>;
}

impl<R> ReadExt for R
where
    R: Read,
{
    fn read_f80(&mut self) -> Result<F80> {
        let mut bytes = [0; 10];
        self.read_exact(&mut bytes)?;

        Ok(F80::from_be_bytes(bytes))
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/audio_tags/util/io/tests.rs"]
mod tests;
