use std::ffi::{CString, OsString};
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStringExt;
use std::path::Path;

#[derive(Debug)]
pub struct RawEvent {
    pub(crate) descriptor: i32,
    mask: u32,
    pub(crate) cookie: u32,
    pub(crate) name: OsString,
}

impl RawEvent {
    pub(crate) fn has(&self, mask: u32) -> bool {
        self.mask & mask != 0
    }
}

pub(crate) fn open() -> io::Result<OwnedFd> {
    let fd = unsafe { libc::inotify_init1(libc::IN_NONBLOCK | libc::IN_CLOEXEC) };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}

pub(crate) fn add(fd: &OwnedFd, path: &Path, mask: u32) -> io::Result<i32> {
    let name = CString::new(path.as_os_str().as_encoded_bytes())?;
    let descriptor = unsafe { libc::inotify_add_watch(fd.as_raw_fd(), name.as_ptr(), mask) };
    if descriptor < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(descriptor)
    }
}

pub(crate) fn remove(fd: &OwnedFd, descriptor: i32) -> io::Result<()> {
    #[cfg(target_os = "android")]
    let descriptor = descriptor.cast_unsigned();
    if unsafe { libc::inotify_rm_watch(fd.as_raw_fd(), descriptor) } < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

pub(crate) fn read(fd: &OwnedFd, buffer: &mut [u8]) -> io::Result<Vec<RawEvent>> {
    let count = unsafe { libc::read(fd.as_raw_fd(), buffer.as_mut_ptr().cast(), buffer.len()) };
    if count < 0 {
        return Err(io::Error::last_os_error());
    }
    if count == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "filesystem watcher closed",
        ));
    }
    decode(&buffer[..count as usize])
}

pub(crate) fn decode(bytes: &[u8]) -> io::Result<Vec<RawEvent>> {
    let mut events = Vec::new();
    let mut offset = 0;
    let header = std::mem::size_of::<libc::inotify_event>();
    while offset < bytes.len() {
        if bytes.len() - offset < header {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "truncated filesystem event",
            ));
        }
        let raw = unsafe {
            std::ptr::read_unaligned(bytes[offset..].as_ptr().cast::<libc::inotify_event>())
        };
        let end = offset
            .checked_add(header)
            .and_then(|n| n.checked_add(raw.len as usize))
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid filesystem event length",
                )
            })?;
        let name = &bytes[offset + header..end];
        let length = name
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(name.len());
        let name = OsString::from_vec(name[..length].to_vec());
        if !name.is_empty()
            && (Path::new(&name).components().count() != 1
                || Path::new(&name)
                    .components()
                    .any(|part| !matches!(part, std::path::Component::Normal(_))))
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid filesystem event name",
            ));
        }
        events.push(RawEvent {
            descriptor: raw.wd,
            mask: raw.mask,
            cookie: raw.cookie,
            name,
        });
        offset = end;
    }
    Ok(events)
}
