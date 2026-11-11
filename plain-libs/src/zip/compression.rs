use crate::zip::format::compression::Compression;
use crate::zip::result::ZipError;
use core::fmt;
use core::fmt::Debug;
use core::fmt::Formatter;
use core::mem::replace;
use core::panic::RefUnwindSafe;
use std::io;
use std::io::{BufRead, Read};
use std::panic::UnwindSafe;
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[cfg_attr(any(), derive(arbitrary::Arbitrary))]
#[non_exhaustive]
pub enum CompressionMethod {
    Stored,
    Deflated,
    Unsupported(u16),
}
impl CompressionMethod {
    pub const STORE: Self = CompressionMethod::Stored;
    pub const SHRINK: Self = CompressionMethod::Unsupported(1);
    pub const REDUCE_1: Self = CompressionMethod::Unsupported(2);
    pub const REDUCE_2: Self = CompressionMethod::Unsupported(3);
    pub const REDUCE_3: Self = CompressionMethod::Unsupported(4);
    pub const REDUCE_4: Self = CompressionMethod::Unsupported(5);
    pub const IMPLODE: Self = CompressionMethod::Unsupported(6);
    pub const DEFLATE: Self = CompressionMethod::Deflated;
    pub const DEFLATE64: Self = CompressionMethod::Unsupported(9);
    pub const PKWARE_IMPLODE: Self = CompressionMethod::Unsupported(10);
    pub const BZIP2: Self = CompressionMethod::Unsupported(12);
    pub const LZMA: Self = CompressionMethod::Unsupported(14);
    pub const IBM_ZOS_CMPSC: Self = CompressionMethod::Unsupported(16);
    pub const IBM_TERSE: Self = CompressionMethod::Unsupported(18);
    pub const ZSTD_DEPRECATED: Self = CompressionMethod::Unsupported(20);
    pub const ZSTD: Self = CompressionMethod::Unsupported(93);
    pub const MP3: Self = CompressionMethod::Unsupported(94);
    pub const XZ: Self = CompressionMethod::Unsupported(95);
    pub const JPEG: Self = CompressionMethod::Unsupported(96);
    pub const WAVPACK: Self = CompressionMethod::Unsupported(97);
    pub const PPMD: Self = CompressionMethod::Unsupported(98);
    pub const AES: Self = CompressionMethod::Unsupported(99);
    pub const DEFAULT: Self = CompressionMethod::Deflated;
}
impl CompressionMethod {
    #[must_use]
    pub const fn name_from_u16(val: u16) -> &'static str {
        match val {
            0 => "Stored",
            1 => "Shrink",
            2 => "Reduce(1)",
            3 => "Reduce(2)",
            4 => "Reduce(3)",
            5 => "Reduce(4)",
            6 => "Implode",
            8 => "Deflated",
            9 => "Deflate64",
            12 => "Bzip2",
            14 => "Lzma",
            95 => "Xz",
            93 => "Zstd",
            98 => "Ppmd",
            99 => "Aes",
            _ => "Unknown",
        }
    }
    pub(crate) const fn parse_from_u16(val: u16) -> Self {
        match val {
            0 => CompressionMethod::Stored,
            8 => CompressionMethod::Deflated,
            v => CompressionMethod::Unsupported(v),
        }
    }
    pub(crate) const fn serialize_to_u16(self) -> u16 {
        match self {
            CompressionMethod::Stored => 0,
            CompressionMethod::Deflated => 8,
            CompressionMethod::Unsupported(v) => v,
        }
    }
    #[must_use]
    pub const fn into_compression(self) -> Compression {
        match self {
            CompressionMethod::Stored => Compression::Stored,
            CompressionMethod::Deflated => Compression::Deflated,
            CompressionMethod::Unsupported(v) => Compression::Unknown(v),
        }
    }
}
impl From<CompressionMethod> for Compression {
    fn from(value: CompressionMethod) -> Self {
        value.into_compression()
    }
}
impl Default for CompressionMethod {
    fn default() -> Self {
        CompressionMethod::Deflated
    }
}
impl fmt::Display for CompressionMethod {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let method_u16 = self.serialize_to_u16();
        let name = Self::name_from_u16(method_u16);
        write!(f, "{name}")
    }
}
pub const SUPPORTED_COMPRESSION_METHODS: &[CompressionMethod] = &[
    CompressionMethod::Stored,
    #[cfg(all())]
    CompressionMethod::Deflated,
    #[cfg(any())]
    CompressionMethod::Deflate64,
    #[cfg(any())]
    CompressionMethod::Bzip2,
    #[cfg(any())]
    CompressionMethod::Zstd,
    #[cfg(any())]
    CompressionMethod::Xz,
    #[cfg(any())]
    CompressionMethod::Ppmd,
];
pub(crate) enum LazyDecompressor<R: BufRead> {
    Uninitialized {
        inner: R,
        parameters: InstantiationParameters,
    },
    Initialized(Decompressor<R>),
    Failed(Option<R>),
}
impl<R: BufRead> UnwindSafe for LazyDecompressor<R> {}
impl<R: BufRead> RefUnwindSafe for LazyDecompressor<R> {}
impl<R: BufRead> Debug for LazyDecompressor<R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            LazyDecompressor::Uninitialized { parameters, .. } => {
                f.write_fmt(format_args!("Uninitialized({parameters:?}"))
            }
            LazyDecompressor::Initialized(r) => f.write_fmt(format_args!("Initialized({r:?})")),
            LazyDecompressor::Failed(None) => f.write_str("Failed (inner writer lost)"),
            LazyDecompressor::Failed(Some(_)) => f.write_str("Failed (inner writer held)"),
        }
    }
}
#[derive(Default, Copy, Clone, Debug)]
pub(crate) struct InstantiationParameters {
    pub(crate) compression_method: CompressionMethod,
}
fn instantiate<R: BufRead>(
    #[cfg_attr(not(any(any(), any())), allow(unused_mut))] mut reader: R,
    parameters: InstantiationParameters,
) -> Result<Decompressor<R>, (ZipError, Option<R>)> {
    match parameters.compression_method {
        CompressionMethod::Stored => Ok(Decompressor::Stored(reader)),
        CompressionMethod::Deflated => Ok(Decompressor::Deflated(
            crate::compression::bufread::DeflateDecoder::new(reader),
        )),
        method => {
            let method = method.serialize_to_u16();
            Err((
                ZipError::CompressionMethodNotSupported(method),
                Some(reader),
            ))
        }
    }
}
impl<R: BufRead> Read for LazyDecompressor<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut decompressor = match replace(self, LazyDecompressor::Failed(None)) {
            LazyDecompressor::Initialized(decompressor) => decompressor,
            LazyDecompressor::Uninitialized { inner, parameters } => instantiate(inner, parameters)
                .map_err(|(error, inner)| {
                    *self = LazyDecompressor::Failed(inner);
                    error
                })?,
            LazyDecompressor::Failed(inner) => {
                *self = LazyDecompressor::Failed(inner);
                return Err(io::Error::other(
                    "Decompressor previously failed to initialize or panicked while reading",
                ));
            }
        };
        let result = decompressor.read(buf);
        *self = LazyDecompressor::Initialized(decompressor);
        result
    }
}
pub(crate) enum Decompressor<R: BufRead> {
    Stored(R),
    Deflated(crate::compression::bufread::DeflateDecoder<R>),
}
impl<R: BufRead> Debug for Decompressor<R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stored(_) => write!(f, "StoredDecompressor"),
            Self::Deflated(_) => write!(f, "DeflatedDecompressor"),
        }
    }
}
impl<R: BufRead> Read for Decompressor<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        match self {
            Decompressor::Stored(r) => r.read(buf),
            Decompressor::Deflated(r) => r.read(buf),
        }
    }
}
impl<R: BufRead> LazyDecompressor<R> {
    pub fn new(reader: R, parameters: InstantiationParameters) -> Self {
        LazyDecompressor::Uninitialized {
            inner: reader,
            parameters,
        }
    }
    pub fn into_inner(self) -> io::Result<R> {
        match self {
            LazyDecompressor::Uninitialized { inner, .. } => Ok(inner),
            LazyDecompressor::Failed(None) => Err(io::Error::other(
                "Decompressor previously failed to initialize or panicked while reading",
            )),
            LazyDecompressor::Failed(Some(inner)) => Ok(inner),
            LazyDecompressor::Initialized(d) => Ok(match d {
                Decompressor::Stored(r) => r,
                Decompressor::Deflated(r) => r.into_inner(),
            }),
        }
    }
    pub fn inner(&self) -> Option<&R> {
        match self {
            LazyDecompressor::Initialized(decompressor) => Some(match decompressor {
                Decompressor::Stored(r) => r,
                Decompressor::Deflated(r) => r.get_ref(),
            }),
            LazyDecompressor::Uninitialized { inner, .. } => Some(inner),
            LazyDecompressor::Failed(maybe_inner) => maybe_inner.as_ref(),
        }
    }
}
