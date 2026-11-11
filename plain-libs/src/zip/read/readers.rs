use crate::zip::compression::{CompressionMethod, InstantiationParameters, LazyDecompressor};
use crate::zip::crc32::Crc32Reader;
use crate::zip::result::{ZipError, ZipResult};
use crate::zip::types::ZipFileData;
use crate::zip::zipcrypto::{ZipCryptoReader, ZipCryptoReaderValid, ZipCryptoValidator};
use std::io::{self, Read, Seek, SeekFrom};
pub(crate) enum ZipFileSeekReader<'a, R: ?Sized> {
    Raw(SeekableTake<'a, R>),
}
pub(crate) struct SeekableTake<'a, R: ?Sized> {
    inner: &'a mut R,
    inner_starting_offset: u64,
    length: u64,
    current_offset: u64,
}
impl<'a, R: Seek + ?Sized> SeekableTake<'a, R> {
    pub fn new(inner: &'a mut R, length: u64) -> io::Result<Self> {
        let inner_starting_offset = inner.stream_position()?;
        if inner_starting_offset.checked_add(length).is_none() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "end of seekable range overflows u64",
            ));
        }
        Ok(Self {
            inner,
            inner_starting_offset,
            length,
            current_offset: 0,
        })
    }
}
impl<R: Seek + ?Sized> Seek for SeekableTake<'_, R> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let offset = match pos {
            SeekFrom::Start(offset) => Some(offset),
            SeekFrom::End(offset) => self.length.checked_add_signed(offset),
            SeekFrom::Current(offset) => self.current_offset.checked_add_signed(offset),
        };
        match offset {
            None => Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid seek to a negative or overflowing position",
            )),
            Some(offset) => {
                let clamped_offset = std::cmp::min(self.length, offset);
                self.inner
                    .seek(SeekFrom::Start(self.inner_starting_offset + clamped_offset))?;
                self.current_offset = clamped_offset;
                Ok(self.current_offset)
            }
        }
    }
}
impl<R: Read + ?Sized> Read for SeekableTake<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let written = self
            .inner
            .take(self.length.saturating_sub(self.current_offset))
            .read(buf)?;
        self.current_offset += written as u64;
        Ok(written)
    }
}
pub(crate) enum CryptoReader<'a, R: Read + ?Sized> {
    Plaintext(io::Take<&'a mut R>),
    ZipCrypto(ZipCryptoReaderValid<io::Take<&'a mut R>>),
}
impl<R: Read + ?Sized> core::fmt::Debug for CryptoReader<'_, R> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            CryptoReader::Plaintext(_) => f.write_str("CryptoReader::Plaintext"),
            CryptoReader::ZipCrypto(_) => f.write_str("CryptoReader::ZipCrypto"),
        }
    }
}
impl<R: Read + ?Sized> Read for CryptoReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            CryptoReader::Plaintext(r) => r.read(buf),
            CryptoReader::ZipCrypto(r) => r.read(buf),
        }
    }
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        match self {
            CryptoReader::Plaintext(r) => r.read_to_end(buf),
            CryptoReader::ZipCrypto(r) => r.read_to_end(buf),
        }
    }
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        match self {
            CryptoReader::Plaintext(r) => r.read_to_string(buf),
            CryptoReader::ZipCrypto(r) => r.read_to_string(buf),
        }
    }
}
impl<'a, R: Read + ?Sized> CryptoReader<'a, R> {
    fn is_aes(&self) -> bool {
        false
    }
    fn finish(&mut self) -> io::Result<()> {
        Ok(())
    }
    pub fn into_inner(self) -> io::Take<&'a mut R> {
        match self {
            CryptoReader::Plaintext(r) => r,
            CryptoReader::ZipCrypto(r) => r.into_inner(),
        }
    }
    pub fn inner(&self) -> &io::Take<&'a mut R> {
        match self {
            CryptoReader::Plaintext(r) => r,
            CryptoReader::ZipCrypto(r) => r.inner(),
        }
    }
}
macro_rules! invalid_state {
    ($ ($ x : expr) ,*) => {
        Err(io::Error::other("ZipFileReader was in an invalid state"))
    };
}
#[derive(Debug)]
pub(crate) enum ZipFileReader<'a, R: Read + ?Sized> {
    NoReader,
    Raw(io::Take<&'a mut R>),
    Stored(Box<Crc32Reader<CryptoReader<'a, R>>>),
    Compressed(Box<Crc32Reader<LazyDecompressor<io::BufReader<CryptoReader<'a, R>>>>>),
    CompressedAes(Box<Crc32Reader<LazyDecompressor<io::BufReader<CryptoReader<'a, R>>>>>),
}
impl<R: Read + ?Sized> Read for ZipFileReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self {
            ZipFileReader::NoReader => invalid_state!(),
            ZipFileReader::Raw(r) => r.read(buf),
            ZipFileReader::Stored(r) => r.read(buf),
            ZipFileReader::Compressed(r) => r.read(buf),
            ZipFileReader::CompressedAes(r) => {
                let n = r.read(buf)?;
                if n == 0 && !buf.is_empty() {
                    self.finish_aes()?;
                }
                Ok(n)
            }
        }
    }
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        match self {
            ZipFileReader::NoReader => invalid_state!(),
            ZipFileReader::Raw(r) => r.read_to_end(buf),
            ZipFileReader::Stored(r) => r.read_to_end(buf),
            ZipFileReader::Compressed(r) => r.read_to_end(buf),
            ZipFileReader::CompressedAes(r) => {
                let n = r.read_to_end(buf)?;
                self.finish_aes()?;
                Ok(n)
            }
        }
    }
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        match self {
            ZipFileReader::NoReader => invalid_state!(),
            ZipFileReader::Raw(r) => r.read_to_string(buf),
            ZipFileReader::Stored(r) => r.read_to_string(buf),
            ZipFileReader::Compressed(r) => r.read_to_string(buf),
            ZipFileReader::CompressedAes(r) => {
                let n = r.read_to_string(buf)?;
                self.finish_aes()?;
                Ok(n)
            }
        }
    }
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        match self {
            ZipFileReader::NoReader => invalid_state!(),
            ZipFileReader::Raw(r) => r.read_exact(buf),
            ZipFileReader::Stored(r) => r.read_exact(buf),
            ZipFileReader::Compressed(r) => r.read_exact(buf),
            ZipFileReader::CompressedAes(r) => r.read_exact(buf),
        }
    }
}
impl<'a, R: Read + ?Sized> ZipFileReader<'a, R> {
    pub(crate) fn into_inner(self) -> io::Result<io::Take<&'a mut R>> {
        match self {
            ZipFileReader::NoReader => invalid_state!(),
            ZipFileReader::Raw(r) => Ok(r),
            ZipFileReader::Stored(r) => Ok(r.into_inner().into_inner()),
            ZipFileReader::Compressed(r) | ZipFileReader::CompressedAes(r) => {
                Ok(r.into_inner().into_inner()?.into_inner().into_inner())
            }
        }
    }
    pub(crate) fn inner(&self) -> io::Result<&io::Take<&'a mut R>> {
        match self {
            ZipFileReader::NoReader => invalid_state!(),
            ZipFileReader::Raw(r) => Ok(r),
            ZipFileReader::Stored(r) => Ok(r.inner().inner()),
            ZipFileReader::Compressed(r) | ZipFileReader::CompressedAes(r) => {
                if let Some(buf) = r.inner().inner() {
                    Ok(buf.get_ref().inner())
                } else {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "Not initialized reader",
                    ))
                }
            }
        }
    }
    fn finish_aes(&mut self) -> io::Result<()> {
        let ZipFileReader::CompressedAes(r) = std::mem::replace(self, ZipFileReader::NoReader)
        else {
            return Ok(());
        };
        let mut crypto = r.into_inner().into_inner()?.into_inner();
        crypto.finish()?;
        let raw = crypto.into_inner();
        debug_assert_eq!(raw.limit(), 0);
        *self = ZipFileReader::Raw(raw);
        Ok(())
    }
}
pub(crate) fn make_crypto_reader<'a, R: Read + ?Sized>(
    data: &ZipFileData,
    reader: io::Take<&'a mut R>,
    password: Option<&[u8]>,
) -> ZipResult<CryptoReader<'a, R>> {
    {
        if let CompressionMethod::Unsupported(id) = data.compression_method {
            return Err(ZipError::CompressionMethodNotSupported(id));
        }
    }
    let reader = match (password, data.aes_settings()) {
        (Some(_), Some(_)) => {
            return Err(ZipError::UnsupportedArchive(
                "AES encrypted files cannot be decrypted without the aes-crypto feature.",
            ));
        }
        (Some(password), None) => {
            let validator = if data.is_using_data_descriptor() {
                ZipCryptoValidator::InfoZipMsdosTime(
                    data.last_modified_time.map_or(0, |x| x.timepart()),
                )
            } else {
                ZipCryptoValidator::PkzipCrc32(data.crc32)
            };
            CryptoReader::ZipCrypto(ZipCryptoReader::new(reader, password).validate(validator)?)
        }
        (None, Some(_)) => return Err(ZipError::InvalidPassword),
        (None, None) => CryptoReader::Plaintext(reader),
    };
    Ok(reader)
}
pub(crate) fn make_reader<R: Read + ?Sized>(
    compression_method: CompressionMethod,
    #[cfg_attr(not(any(any(), any())), allow(unused))] uncompressed_size: u64,
    size_limit: Option<u64>,
    crc32: Option<u32>,
    aes_vendor_version: Option<crate::zip::format::aes::AesVendorVersion>,
    reader: CryptoReader<'_, R>,
    #[cfg(any())] flags: u16,
) -> ZipResult<ZipFileReader<'_, R>> {
    let (should_disable, crc32) = if let Some(data_crc32) = crc32 {
        (
            aes_vendor_version.is_some_and(|v| v.is_ae2_encrypted()),
            data_crc32,
        )
    } else {
        (true, 0)
    };
    if compression_method == CompressionMethod::Stored {
        return Ok(ZipFileReader::Stored(Box::new(
            Crc32Reader::new(reader, crc32, should_disable).with_size_limit(size_limit),
        )));
    }
    let is_aes = reader.is_aes();
    let reader = Box::new(
        Crc32Reader::new(
            LazyDecompressor::new(
                io::BufReader::new(reader),
                InstantiationParameters { compression_method },
            ),
            crc32,
            should_disable,
        )
        .with_size_limit(size_limit),
    );
    Ok(if is_aes {
        ZipFileReader::CompressedAes(reader)
    } else {
        ZipFileReader::Compressed(reader)
    })
}
