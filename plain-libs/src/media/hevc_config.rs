#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Truncated(&'static str),
    Malformed(&'static str),
}

pub struct NalArray<'a> {
    data: &'a [u8],
}
impl<'a> NalArray<'a> {
    pub fn bytes(&self) -> &'a [u8] {
        self.data
    }
    pub fn nals(&self) -> NalUnits<'a> {
        NalUnits {
            remaining: &self.data[3..],
            count: u16::from_be_bytes([self.data[1], self.data[2]]),
        }
    }
}

pub struct NalUnits<'a> {
    remaining: &'a [u8],
    count: u16,
}
impl<'a> Iterator for NalUnits<'a> {
    type Item = &'a [u8];
    fn size_hint(&self) -> (usize, Option<usize>) {
        let count = usize::from(self.count);
        (count, Some(count))
    }
    fn next(&mut self) -> Option<Self::Item> {
        if self.count == 0 {
            return None;
        }
        let length = usize::from(u16::from_be_bytes([self.remaining[0], self.remaining[1]]));
        let nal = &self.remaining[2..2 + length];
        self.remaining = &self.remaining[2 + length..];
        self.count -= 1;
        Some(nal)
    }
}
impl ExactSizeIterator for NalUnits<'_> {}

pub struct NalArrays<'a> {
    data: &'a [u8],
    position: usize,
    count: u8,
}
impl<'a> NalArrays<'a> {
    pub fn new(data: &'a [u8], count: u8) -> Self {
        Self {
            data,
            position: 0,
            count,
        }
    }
    fn take(&mut self, length: usize, name: &'static str) -> Result<&'a [u8], Error> {
        let end = self
            .position
            .checked_add(length)
            .ok_or(Error::Truncated(name))?;
        let bytes = self
            .data
            .get(self.position..end)
            .ok_or(Error::Truncated(name))?;
        self.position = end;
        Ok(bytes)
    }
    fn array(&mut self) -> Result<NalArray<'a>, Error> {
        let start = self.position;
        self.take(1, "hvcC array header")?;
        let count = self.take(2, "hvcC nalu count")?;
        let count = u16::from_be_bytes([count[0], count[1]]);
        if usize::from(count) * 2 > self.data.len() - self.position {
            return Err(Error::Malformed(
                "hvcC declares more NAL units than it holds",
            ));
        }
        for _ in 0..count {
            let length = self.take(2, "hvcC nalu length")?;
            let length = usize::from(u16::from_be_bytes([length[0], length[1]]));
            self.take(length, "hvcC nal unit")?;
        }
        Ok(NalArray {
            data: &self.data[start..self.position],
        })
    }
}
impl<'a> Iterator for NalArrays<'a> {
    type Item = Result<NalArray<'a>, Error>;
    fn next(&mut self) -> Option<Self::Item> {
        if self.count == 0 {
            return None;
        }
        self.count -= 1;
        let result = self.array();
        if result.is_err() {
            self.count = 0;
        }
        Some(result)
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/media/hevc_config.rs"]
mod tests;
