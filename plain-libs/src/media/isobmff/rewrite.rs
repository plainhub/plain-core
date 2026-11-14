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
    let mut buffer = [0; 64 * 1024];
    while bytes > 0 {
        check_canceled(canceled)?;
        let count = bytes.min(buffer.len() as u64) as usize;
        reader.read_exact(&mut buffer[..count])?;
        writer.write_all(&buffer[..count])?;
        bytes -= count as u64;
    }
    Ok(())
}
