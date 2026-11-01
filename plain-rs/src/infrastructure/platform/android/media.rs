#[path = "media/audio.rs"]
mod audio;
#[path = "media_ffi.rs"]
mod ffi;
#[path = "media/video.rs"]
mod video;
use anyhow::{Result, anyhow, ensure};
use std::{
    ffi::{CStr, CString},
    fs::{File, OpenOptions},
    os::fd::AsRawFd,
    path::Path,
    ptr::{self, NonNull},
};

fn status(value: i32, operation: &str) -> Result<()> {
    ensure!(value == 0, "Android media {operation} failed ({value})");
    Ok(())
}
struct Format(NonNull<ffi::Format>);
impl Format {
    fn owned(value: *mut ffi::Format) -> Result<Self> {
        Ok(Self(
            NonNull::new(value).ok_or_else(|| anyhow!("Missing Android media format"))?,
        ))
    }
    fn int(&self, name: &CStr) -> Option<i32> {
        let mut value = 0;
        unsafe { ffi::AMediaFormat_getInt32(self.0.as_ptr(), name.as_ptr(), &mut value) }
            .then_some(value)
    }
    fn string(&self, name: &CStr) -> Option<String> {
        let mut value = ptr::null();
        if unsafe { ffi::AMediaFormat_getString(self.0.as_ptr(), name.as_ptr(), &mut value) }
            && !value.is_null()
        {
            Some(
                unsafe { CStr::from_ptr(value) }
                    .to_string_lossy()
                    .into_owned(),
            )
        } else {
            None
        }
    }
    fn set_int(&self, name: &CStr, value: i32) {
        unsafe { ffi::AMediaFormat_setInt32(self.0.as_ptr(), name.as_ptr(), value) }
    }
    fn set_string(&self, name: &CStr, value: &CStr) {
        unsafe { ffi::AMediaFormat_setString(self.0.as_ptr(), name.as_ptr(), value.as_ptr()) }
    }
}
impl Drop for Format {
    fn drop(&mut self) {
        unsafe {
            ffi::AMediaFormat_delete(self.0.as_ptr());
        }
    }
}
struct Input {
    file: File,
    offset: i64,
    length: i64,
}
impl Input {
    fn open(path: &Path) -> Result<Self> {
        let file = File::open(path)?;
        let length = i64::try_from(file.metadata()?.len())?;
        Self::from_file(file, 0, length)
    }
    fn from_file(file: File, offset: i64, length: i64) -> Result<Self> {
        ensure!(
            offset >= 0 && length >= 0 && offset.checked_add(length).is_some(),
            "Invalid media file range"
        );
        Ok(Self {
            file,
            offset,
            length,
        })
    }
}
struct Extractor {
    raw: NonNull<ffi::Extractor>,
    _file: File,
}
impl Extractor {
    fn open(input: &Input) -> Result<Self> {
        let file = input.file.try_clone()?;
        let raw = NonNull::new(unsafe { ffi::AMediaExtractor_new() })
            .ok_or_else(|| anyhow!("Missing Android extractor"))?;
        let result = Self { raw, _file: file };
        status(
            unsafe {
                ffi::AMediaExtractor_setDataSourceFd(
                    result.raw.as_ptr(),
                    result._file.as_raw_fd(),
                    input.offset,
                    input.length,
                )
            },
            "open input",
        )?;
        Ok(result)
    }
    fn formats(&self) -> Result<Vec<(usize, Format)>> {
        (0..unsafe { ffi::AMediaExtractor_getTrackCount(self.raw.as_ptr()) })
            .map(|index| {
                Ok((
                    index,
                    Format::owned(unsafe {
                        ffi::AMediaExtractor_getTrackFormat(self.raw.as_ptr(), index)
                    })?,
                ))
            })
            .collect()
    }
    fn select(&self, index: usize) -> Result<()> {
        status(
            unsafe { ffi::AMediaExtractor_selectTrack(self.raw.as_ptr(), index) },
            "select track",
        )
    }
    fn track(&self) -> i32 {
        unsafe { ffi::AMediaExtractor_getSampleTrackIndex(self.raw.as_ptr()) }
    }
    fn sample(&self, data: &mut Vec<u8>) -> Result<ffi::BufferInfo> {
        let size = unsafe { ffi::AMediaExtractor_getSampleSize(self.raw.as_ptr()) };
        ensure!(size >= 0, "Missing Android sample");
        data.resize(size as usize, 0);
        let read = unsafe {
            ffi::AMediaExtractor_readSampleData(self.raw.as_ptr(), data.as_mut_ptr(), data.len())
        };
        ensure!(read == size, "Incomplete Android sample");
        Ok(ffi::BufferInfo {
            offset: 0,
            size: i32::try_from(size)?,
            presentation_time_us: unsafe { ffi::AMediaExtractor_getSampleTime(self.raw.as_ptr()) },
            flags: unsafe { ffi::AMediaExtractor_getSampleFlags(self.raw.as_ptr()) } & 1,
        })
    }
    fn advance(&self) {
        unsafe {
            ffi::AMediaExtractor_advance(self.raw.as_ptr());
        }
    }
}
impl Drop for Extractor {
    fn drop(&mut self) {
        unsafe {
            ffi::AMediaExtractor_delete(self.raw.as_ptr());
        }
    }
}
struct Muxer {
    raw: NonNull<ffi::Muxer>,
    _file: File,
    started: bool,
}
impl Muxer {
    fn open(path: &Path) -> Result<Self> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(true)
            .open(path)?;
        let raw = NonNull::new(unsafe { ffi::AMediaMuxer_new(file.as_raw_fd(), 0) })
            .ok_or_else(|| anyhow!("Missing Android muxer"))?;
        Ok(Self {
            raw,
            _file: file,
            started: false,
        })
    }
    fn add(&self, format: &Format) -> Result<usize> {
        let index = unsafe { ffi::AMediaMuxer_addTrack(self.raw.as_ptr(), format.0.as_ptr()) };
        ensure!(index >= 0, "Android muxer rejected track ({index})");
        Ok(index as usize)
    }
    fn rotation(&self, degrees: i32) -> Result<()> {
        status(
            unsafe { ffi::AMediaMuxer_setOrientationHint(self.raw.as_ptr(), degrees) },
            "rotation",
        )
    }
    fn start(&mut self) -> Result<()> {
        status(
            unsafe { ffi::AMediaMuxer_start(self.raw.as_ptr()) },
            "start muxer",
        )?;
        self.started = true;
        Ok(())
    }
    fn write(&self, track: usize, data: &[u8], info: ffi::BufferInfo) -> Result<()> {
        ensure!(
            info.offset >= 0
                && info.size >= 0
                && (info.offset as usize)
                    .checked_add(info.size as usize)
                    .is_some_and(|end| end <= data.len()),
            "Invalid Android output sample bounds"
        );
        status(
            unsafe {
                ffi::AMediaMuxer_writeSampleData(self.raw.as_ptr(), track, data.as_ptr(), &info)
            },
            "write sample",
        )
    }
    fn finish(&mut self) -> Result<()> {
        status(
            unsafe { ffi::AMediaMuxer_stop(self.raw.as_ptr()) },
            "stop muxer",
        )?;
        self.started = false;
        Ok(())
    }
}
impl Drop for Muxer {
    fn drop(&mut self) {
        unsafe {
            if self.started {
                ffi::AMediaMuxer_stop(self.raw.as_ptr());
            }
            ffi::AMediaMuxer_delete(self.raw.as_ptr());
        }
    }
}

fn remux(input: &Input, output: &Path, cancel: &std::sync::atomic::AtomicBool) -> Result<bool> {
    let extractor = Extractor::open(input)?;
    let formats = extractor.formats()?;
    let mut muxer = Muxer::open(output)?;
    let mut tracks = std::collections::HashMap::new();
    for (index, format) in formats {
        let mime = format.string(c"mime").unwrap_or_default();
        if !mime.starts_with("video/") && !mime.starts_with("audio/") {
            continue;
        }
        if mime.starts_with("video/") {
            muxer.rotation(format.int(c"rotation-degrees").unwrap_or(0))?;
        }
        tracks.insert(index as i32, muxer.add(&format)?);
        extractor.select(index)?;
    }
    if tracks.is_empty() {
        return Ok(false);
    }
    muxer.start()?;
    let mut bytes = Vec::new();
    while extractor.track() >= 0 {
        ensure!(
            !cancel.load(std::sync::atomic::Ordering::Acquire),
            "Video conversion cancelled"
        );
        let track = *tracks
            .get(&extractor.track())
            .ok_or_else(|| anyhow!("Unexpected Android track"))?;
        let info = extractor.sample(&mut bytes)?;
        muxer.write(track, &bytes, info)?;
        extractor.advance();
    }
    muxer.finish()?;
    Ok(true)
}

struct Codec {
    raw: NonNull<ffi::Codec>,
    started: bool,
}
impl Codec {
    fn create(mime: &CStr, encoder: bool) -> Result<Self> {
        let raw = unsafe {
            if encoder {
                ffi::AMediaCodec_createEncoderByType(mime.as_ptr())
            } else {
                ffi::AMediaCodec_createDecoderByType(mime.as_ptr())
            }
        };
        Ok(Self {
            raw: NonNull::new(raw)
                .ok_or_else(|| anyhow!("Android codec unavailable: {}", mime.to_string_lossy()))?,
            started: false,
        })
    }
    fn configure(&self, format: &Format, window: *mut ffi::Window, encoder: bool) -> Result<()> {
        status(
            unsafe {
                ffi::AMediaCodec_configure(
                    self.raw.as_ptr(),
                    format.0.as_ptr(),
                    window,
                    ptr::null_mut(),
                    u32::from(encoder),
                )
            },
            "configure codec",
        )
    }
    fn start(&mut self) -> Result<()> {
        status(
            unsafe { ffi::AMediaCodec_start(self.raw.as_ptr()) },
            "start codec",
        )?;
        self.started = true;
        Ok(())
    }
    fn input(&self) -> Result<Option<usize>> {
        let index = unsafe { ffi::AMediaCodec_dequeueInputBuffer(self.raw.as_ptr(), 0) };
        if index == -1 {
            return Ok(None);
        }
        ensure!(index >= 0, "Android input dequeue failed ({index})");
        Ok(Some(index as usize))
    }
    fn queue(&self, index: usize, bytes: &[u8], pts: i64, flags: u32) -> Result<()> {
        if !bytes.is_empty() {
            let mut capacity = 0;
            let buffer =
                unsafe { ffi::AMediaCodec_getInputBuffer(self.raw.as_ptr(), index, &mut capacity) };
            ensure!(
                !buffer.is_null() && bytes.len() <= capacity,
                "Android codec input buffer is too small"
            );
            unsafe {
                ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len());
            }
        }
        status(
            unsafe {
                ffi::AMediaCodec_queueInputBuffer(
                    self.raw.as_ptr(),
                    index,
                    0,
                    bytes.len(),
                    u64::try_from(pts.max(0))?,
                    flags,
                )
            },
            "queue codec input",
        )
    }
    fn output(&self, timeout_us: i64) -> Result<Option<(isize, ffi::BufferInfo)>> {
        let mut info = ffi::BufferInfo::default();
        let index = unsafe {
            ffi::AMediaCodec_dequeueOutputBuffer(self.raw.as_ptr(), &mut info, timeout_us)
        };
        if index == -1 {
            return Ok(None);
        }
        ensure!(
            index >= 0 || index == -2 || index == -3,
            "Android output dequeue failed ({index})"
        );
        Ok(Some((index, info)))
    }
    fn bytes(&self, index: usize) -> Result<&[u8]> {
        let mut size = 0;
        let buffer =
            unsafe { ffi::AMediaCodec_getOutputBuffer(self.raw.as_ptr(), index, &mut size) };
        ensure!(
            !buffer.is_null() && size <= isize::MAX as usize,
            "Missing Android codec output"
        );
        Ok(unsafe { std::slice::from_raw_parts(buffer, size) })
    }
    fn release(&self, index: usize, render: bool) -> Result<()> {
        status(
            unsafe { ffi::AMediaCodec_releaseOutputBuffer(self.raw.as_ptr(), index, render) },
            "release codec output",
        )
    }
    fn format(&self) -> Result<Format> {
        Format::owned(unsafe { ffi::AMediaCodec_getOutputFormat(self.raw.as_ptr()) })
    }
}
impl Drop for Codec {
    fn drop(&mut self) {
        unsafe {
            if self.started {
                ffi::AMediaCodec_stop(self.raw.as_ptr());
            }
            ffi::AMediaCodec_delete(self.raw.as_ptr());
        }
    }
}
struct Window(*mut ffi::Window);
impl Drop for Window {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                ffi::ANativeWindow_release(self.0);
            }
        }
    }
}
struct Sample {
    bytes: Vec<u8>,
    info: ffi::BufferInfo,
}
pub fn capabilities() -> Vec<&'static str> {
    static HEVC: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        Codec::create(c"video/avc", true).is_ok() && Codec::create(c"video/hevc", false).is_ok()
    });
    static THREE_GP: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| {
        Codec::create(c"video/avc", true).is_ok()
            && Codec::create(c"video/3gpp", false).is_ok()
            && Codec::create(c"audio/mp4a-latm", true).is_ok()
            && Codec::create(c"audio/3gpp", false).is_ok()
    });
    let mut values = vec!["VIDEO_REMUX"];
    if *THREE_GP {
        values.push("VIDEO_3GP_CONVERT");
    }
    if *HEVC {
        values.push("VIDEO_TRANSCODE");
    }
    values
}

#[derive(Clone, Copy)]
pub enum Operation {
    Remux,
    Transcode,
    Convert3gp,
}

pub fn encode(
    input: &Path,
    output: &Path,
    operation: Operation,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<bool> {
    let input = Input::open(input)?;
    encode_input(&input, output, operation, cancel)
}

pub fn encode_file(
    file: File,
    offset: i64,
    length: i64,
    output: &Path,
    operation: Operation,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<bool> {
    encode_input(
        &Input::from_file(file, offset, length)?,
        output,
        operation,
        cancel,
    )
}

fn encode_input(
    input: &Input,
    output: &Path,
    operation: Operation,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<bool> {
    match operation {
        Operation::Remux => remux(input, output, cancel),
        Operation::Transcode => video::transcode(input, output, cancel),
        Operation::Convert3gp => audio::convert_3gp(input, output, cancel),
    }
}
