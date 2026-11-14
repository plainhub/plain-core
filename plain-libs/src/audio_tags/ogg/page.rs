// Read-only framing adapted from ogg_pager 0.7.2 (ogg_pager/SOURCE.json).
use crate::binary::{LittleEndian, ReadBytesExt};
use std::io::{self, Read, Seek, SeekFrom};

type Result<T> = std::result::Result<T, PageError>;

#[derive(Debug)]
pub enum PageError {
    InvalidVersion,
    BadSegmentCount,
    MissingMagic,
    Io(io::Error),
}

impl std::fmt::Display for PageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidVersion => f.write_str("Invalid Ogg stream structure version"),
            Self::BadSegmentCount => f.write_str("Ogg page has no segments"),
            Self::MissingMagic => f.write_str("Missing Ogg page signature"),
            Self::Io(error) => std::fmt::Display::fmt(error, f),
        }
    }
}

impl std::error::Error for PageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            _ => None,
        }
    }
}

impl From<io::Error> for PageError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

pub struct PageHeader {
    pub abgp: u64,
    segments: Vec<u8>,
}

impl PageHeader {
    pub fn read(data: &mut impl Read) -> Result<Self> {
        if data.read_fixed::<4>()? != *b"OggS" {
            return Err(PageError::MissingMagic);
        }
        if data.read_u8()? != 0 {
            return Err(PageError::InvalidVersion);
        }
        let _flags = data.read_u8()?;
        let abgp = data.read_u64::<LittleEndian>()?;
        let _serial_sequence_checksum = data.read_fixed::<12>()?;
        let segment_count = usize::from(data.read_u8()?);
        if segment_count == 0 {
            return Err(PageError::BadSegmentCount);
        }
        let mut segments = vec![0; segment_count];
        data.read_exact(&mut segments)?;
        Ok(Self { abgp, segments })
    }

    fn content_size(&self) -> u64 {
        self.segments.iter().map(|&size| u64::from(size)).sum()
    }
}

pub(super) fn find_last_page(data: &mut (impl Read + Seek)) -> Result<PageHeader> {
    let mut last = PageHeader::read(data)?;
    let mut body = data.stream_position()?;
    data.seek(SeekFrom::Current(last.content_size() as i64))?;
    while let Ok(header) = PageHeader::read(data) {
        last = header;
        body = data.stream_position()?;
        data.seek(SeekFrom::Current(last.content_size() as i64))?;
    }
    data.seek(SeekFrom::Start(body))?;
    let size = last.content_size();
    if io::copy(&mut data.take(size), &mut io::sink())? != size {
        return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
    }
    Ok(last)
}

pub struct Packets(Vec<Vec<u8>>);

impl Packets {
    pub fn read_count(data: &mut impl Read, first: &PageHeader, count: usize) -> Result<Self> {
        let mut packets = Vec::new();
        let mut packet = Vec::new();
        let mut page = None;
        while packets.len() < count {
            for &segment in &page.as_ref().unwrap_or(first).segments {
                let size = usize::from(segment);
                let offset = packet.len();
                packet.try_reserve(size).map_err(io::Error::other)?;
                packet.resize(offset + size, 0);
                data.read_exact(&mut packet[offset..])?;
                if segment < 255 {
                    packets.push(std::mem::take(&mut packet));
                    if packets.len() == count {
                        return Ok(Self(packets));
                    }
                }
            }
            page = Some(PageHeader::read(data)?);
        }
        Ok(Self(packets))
    }

    pub fn get(&self, index: usize) -> Option<&[u8]> {
        self.0.get(index).map(Vec::as_slice)
    }

    pub fn iter(&self) -> impl Iterator<Item = &[u8]> {
        self.0.iter().map(Vec::as_slice)
    }
}
