use std::{
    io::{self, Read, Write},
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) fn check_canceled(canceled: &AtomicBool) -> io::Result<()> {
    if canceled.load(Ordering::Acquire) {
        return Err(io::Error::other("MP4 video processing canceled"));
    }
    Ok(())
}

pub(super) fn copy(
    reader: &mut impl Read,
    writer: &mut impl Write,
    mut bytes: u64,
    canceled: &AtomicBool,
) -> io::Result<()> {
    while bytes > 0 {
        check_canceled(canceled)?;
        let count = bytes.min(64 * 1024);
        if io::copy(&mut reader.by_ref().take(count), writer)? != count {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        bytes -= count;
    }
    Ok(())
}

#[derive(Clone, Copy)]
pub enum PreviewRewrite {
    Metadata,
    Fragments,
    FastStart,
}

impl PreviewRewrite {
    pub fn apply(
        self,
        reader: &mut (impl Read + io::Seek),
        writer: &mut (impl Write + io::Seek),
        canceled: &AtomicBool,
    ) -> io::Result<bool> {
        match self {
            Self::Metadata => super::remove_mebx_tracks(reader, writer, canceled),
            Self::Fragments => super::flatten_fragments(reader, writer, canceled),
            Self::FastStart => super::fast_start::apply(reader, writer, canceled),
        }
    }
}

pub(super) fn shift_chunks(
    movie: &mut [u8],
    delta: u64,
    media: std::ops::RangeInclusive<u64>,
) -> Option<()> {
    fn visit(
        mut boxes: &mut [u8],
        levels: &[&[u8; 4]],
        delta: u64,
        media: &std::ops::RangeInclusive<u64>,
    ) -> Option<()> {
        while !boxes.is_empty() {
            let length = super::u32_at(boxes, 0)? as usize;
            let (entry, rest) = boxes.split_at_mut_checked(length)?;
            let (head, body) = entry.split_at_mut_checked(8)?;
            let kind = &head[4..];
            if let Some((level, children)) = levels.split_first() {
                if kind == *level {
                    visit(body, children, delta, media)?;
                }
            } else if kind == b"stco" || kind == b"co64" {
                let width = if kind == b"stco" { 4 } else { 8 };
                let count = super::u32_at(body, 4)? as usize;
                if super::u32_at(body, 0)? != 0
                    || count.checked_mul(width)? != body.len().checked_sub(8)?
                {
                    return None;
                }
                for slot in body[8..].chunks_exact_mut(width) {
                    let offset = if width == 4 {
                        u64::from(super::u32_at(slot, 0)?)
                    } else {
                        super::u64_at(slot, 0)?
                    };
                    if !media.contains(&offset) {
                        return None;
                    }
                    let shifted = offset.checked_add(delta)?;
                    if width == 4 {
                        slot.copy_from_slice(&u32::try_from(shifted).ok()?.to_be_bytes());
                    } else {
                        slot.copy_from_slice(&shifted.to_be_bytes());
                    }
                }
            }
            boxes = rest;
        }
        Some(())
    }
    visit(movie, &[b"trak", b"mdia", b"minf", b"stbl"], delta, &media)
}

pub(super) fn sample_descriptions(data: &[u8]) -> Option<Vec<super::Atom<'_>>> {
    let entries = super::atoms(data.get(8..)?)?;
    (entries.len() == super::u32_at(data, 4)? as usize
        && entries.iter().all(|entry| {
            matches!(
                &entry.kind,
                b"avc1"
                    | b"avc3"
                    | b"hvc1"
                    | b"hev1"
                    | b"mp4a"
                    | b"av01"
                    | b"vp09"
                    | b"Opus"
                    | b"tmcd"
            )
        }))
    .then_some(entries)
}
