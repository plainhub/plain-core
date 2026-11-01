use std::ffi::{c_char, c_int, c_long, c_void};

#[repr(C)]
pub struct Format {
    _private: [u8; 0],
}
#[repr(C)]
pub struct Extractor {
    _private: [u8; 0],
}
#[repr(C)]
pub struct Codec {
    _private: [u8; 0],
}
#[repr(C)]
pub struct Muxer {
    _private: [u8; 0],
}
#[repr(C)]
pub struct Window {
    _private: [u8; 0],
}
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct BufferInfo {
    pub offset: i32,
    pub size: i32,
    pub presentation_time_us: i64,
    pub flags: u32,
}

#[link(name = "mediandk")]
unsafe extern "C" {
    pub fn AMediaFormat_new() -> *mut Format;
    pub fn AMediaFormat_delete(format: *mut Format) -> c_int;
    pub fn AMediaFormat_getInt32(format: *mut Format, name: *const c_char, value: *mut i32)
    -> bool;

    pub fn AMediaFormat_getString(
        format: *mut Format,
        name: *const c_char,
        value: *mut *const c_char,
    ) -> bool;
    pub fn AMediaFormat_setInt32(format: *mut Format, name: *const c_char, value: i32);
    pub fn AMediaFormat_setString(format: *mut Format, name: *const c_char, value: *const c_char);
    pub fn AMediaExtractor_new() -> *mut Extractor;
    pub fn AMediaExtractor_delete(extractor: *mut Extractor) -> c_int;
    pub fn AMediaExtractor_setDataSourceFd(
        extractor: *mut Extractor,
        fd: c_int,
        offset: i64,
        length: i64,
    ) -> c_int;
    pub fn AMediaExtractor_getTrackCount(extractor: *mut Extractor) -> usize;
    pub fn AMediaExtractor_getTrackFormat(extractor: *mut Extractor, index: usize) -> *mut Format;
    pub fn AMediaExtractor_selectTrack(extractor: *mut Extractor, index: usize) -> c_int;
    pub fn AMediaExtractor_getSampleTrackIndex(extractor: *mut Extractor) -> c_int;
    pub fn AMediaExtractor_getSampleTime(extractor: *mut Extractor) -> i64;
    pub fn AMediaExtractor_getSampleFlags(extractor: *mut Extractor) -> u32;
    pub fn AMediaExtractor_getSampleSize(extractor: *mut Extractor) -> isize;
    pub fn AMediaExtractor_readSampleData(
        extractor: *mut Extractor,
        buffer: *mut u8,
        capacity: usize,
    ) -> isize;
    pub fn AMediaExtractor_advance(extractor: *mut Extractor) -> bool;
    pub fn AMediaCodec_createEncoderByType(mime: *const c_char) -> *mut Codec;
    pub fn AMediaCodec_createDecoderByType(mime: *const c_char) -> *mut Codec;
    pub fn AMediaCodec_configure(
        codec: *mut Codec,
        format: *const Format,
        surface: *mut Window,
        crypto: *mut c_void,
        flags: u32,
    ) -> c_int;
    pub fn AMediaCodec_start(codec: *mut Codec) -> c_int;
    pub fn AMediaCodec_stop(codec: *mut Codec) -> c_int;
    pub fn AMediaCodec_delete(codec: *mut Codec) -> c_int;
    pub fn AMediaCodec_dequeueInputBuffer(codec: *mut Codec, timeout_us: i64) -> isize;
    pub fn AMediaCodec_getInputBuffer(codec: *mut Codec, index: usize, size: *mut usize)
    -> *mut u8;
    pub fn AMediaCodec_queueInputBuffer(
        codec: *mut Codec,
        index: usize,
        offset: c_long,
        size: usize,
        time_us: u64,
        flags: u32,
    ) -> c_int;
    pub fn AMediaCodec_dequeueOutputBuffer(
        codec: *mut Codec,
        info: *mut BufferInfo,
        timeout_us: i64,
    ) -> isize;
    pub fn AMediaCodec_getOutputBuffer(
        codec: *mut Codec,
        index: usize,
        size: *mut usize,
    ) -> *mut u8;
    pub fn AMediaCodec_getOutputFormat(codec: *mut Codec) -> *mut Format;
    pub fn AMediaCodec_releaseOutputBuffer(codec: *mut Codec, index: usize, render: bool) -> c_int;
    pub fn AMediaCodec_createInputSurface(codec: *mut Codec, surface: *mut *mut Window) -> c_int;
    pub fn AMediaCodec_signalEndOfInputStream(codec: *mut Codec) -> c_int;
    pub fn AMediaMuxer_new(fd: c_int, format: c_int) -> *mut Muxer;
    pub fn AMediaMuxer_delete(muxer: *mut Muxer) -> c_int;
    pub fn AMediaMuxer_setOrientationHint(muxer: *mut Muxer, degrees: c_int) -> c_int;
    pub fn AMediaMuxer_addTrack(muxer: *mut Muxer, format: *const Format) -> isize;
    pub fn AMediaMuxer_start(muxer: *mut Muxer) -> c_int;
    pub fn AMediaMuxer_stop(muxer: *mut Muxer) -> c_int;
    pub fn AMediaMuxer_writeSampleData(
        muxer: *mut Muxer,
        track: usize,
        data: *const u8,
        info: *const BufferInfo,
    ) -> c_int;
}
#[link(name = "android")]
unsafe extern "C" {
    pub fn ANativeWindow_release(window: *mut Window);
}
