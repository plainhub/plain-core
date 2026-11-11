use std::io;
pub trait WriteBytesExt<T> {
    fn write_le(&mut self, n: T) -> io::Result<()>;
}
impl<W: io::Write + ?Sized> WriteBytesExt<u8> for W {
    #[inline(always)]
    fn write_le(&mut self, n: u8) -> io::Result<()> {
        self.write_all(&[n])
    }
}
impl<W: io::Write + ?Sized> WriteBytesExt<u16> for W {
    #[inline]
    fn write_le(&mut self, n: u16) -> io::Result<()> {
        self.write_all(&n.to_le_bytes())
    }
}
impl<W: io::Write + ?Sized> WriteBytesExt<u32> for W {
    #[inline]
    fn write_le(&mut self, n: u32) -> io::Result<()> {
        self.write_all(&n.to_le_bytes())
    }
}
impl<W: io::Write + ?Sized> WriteBytesExt<u64> for W {
    #[inline]
    fn write_le(&mut self, n: u64) -> io::Result<()> {
        self.write_all(&n.to_le_bytes())
    }
}
