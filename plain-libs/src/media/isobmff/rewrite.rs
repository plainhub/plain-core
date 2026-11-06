use std::{
    io::{self, Read, Write},
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) fn check_cancelled(cancelled: &AtomicBool) -> io::Result<()> {
    if cancelled.load(Ordering::Acquire) {
        return Err(io::Error::other("MP4 video processing cancelled"));
    }
    Ok(())
}

pub(super) fn copy(
    reader: &mut impl Read,
    writer: &mut impl Write,
    mut bytes: u64,
    cancelled: &AtomicBool,
) -> io::Result<()> {
    let mut buffer = [0; 64 * 1024];
    while bytes > 0 {
        check_cancelled(cancelled)?;
        let count = bytes.min(buffer.len() as u64) as usize;
        reader.read_exact(&mut buffer[..count])?;
        writer.write_all(&buffer[..count])?;
        bytes -= count as u64;
    }
    Ok(())
}
