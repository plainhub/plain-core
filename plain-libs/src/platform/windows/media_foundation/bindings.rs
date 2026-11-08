// Media Foundation ABI adapted from windows 0.57.0 (https://github.com/microsoft/windows-rs), see ../LICENSE-MIT.

use crate::platform::rename::WidePath;
use std::{
    ffi::c_void,
    fmt,
    marker::PhantomData,
    ptr::{NonNull, null, null_mut},
};

type Raw = *mut c_void;
type Status = i32;
type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Error(Status);

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Media Foundation HRESULT 0x{:08X}", self.0 as u32)
    }
}

impl std::error::Error for Error {}

pub(super) const ATTRIBUTE_NOT_FOUND: Error = Error(0xC00D36E6_u32 as i32);
const E_POINTER: Error = Error(0x80004003_u32 as i32);
const MF_VERSION: u32 = 131184;
pub(super) const ALL_STREAMS: u32 = u32::MAX - 1;
pub(super) const SOURCE_ERROR: u32 = 1;
pub(super) const END_OF_STREAM: u32 = 2;
pub(super) const MEDIA_TYPE_CHANGED: u32 = 32;
pub(super) const STREAM_TICK: u32 = 256;

#[repr(C)]
#[derive(Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

impl Guid {
    const fn from_u128(value: u128) -> Self {
        Self {
            data1: (value >> 96) as u32,
            data2: (value >> 80) as u16,
            data3: (value >> 64) as u16,
            data4: (value as u64).to_be_bytes(),
        }
    }
}

pub(super) const MF_MEDIA_TYPE_AUDIO: Guid =
    Guid::from_u128(0x73647561_0000_0010_8000_00aa00389b71);
pub(super) const MF_MEDIA_TYPE_VIDEO: Guid =
    Guid::from_u128(0x73646976_0000_0010_8000_00aa00389b71);
pub(super) const MF_VIDEO_FORMAT_H264: Guid =
    Guid::from_u128(0x34363248_0000_0010_8000_00aa00389b71);
pub(super) const MF_VIDEO_FORMAT_NV12: Guid =
    Guid::from_u128(0x3231564e_0000_0010_8000_00aa00389b71);
pub(super) const MF_MT_AVG_BITRATE: Guid = Guid::from_u128(0x20332624_fb0d_4d9e_bd0d_cbf6786c102e);
pub(super) const MF_MT_FRAME_RATE: Guid = Guid::from_u128(0xc459a2e8_3d2c_4e44_b132_fee5156c7bb0);
pub(super) const MF_MT_FRAME_SIZE: Guid = Guid::from_u128(0x1652c33d_d6b2_4012_b834_72030849a37d);
pub(super) const MF_MT_INTERLACE_MODE: Guid =
    Guid::from_u128(0xe2724bb8_e676_4806_b4b2_a8d6efb44ccd);
pub(super) const MF_MT_MAJOR_TYPE: Guid = Guid::from_u128(0x48eba18e_f8c9_4687_bf11_0a74c9f96a8f);
pub(super) const MF_MT_PIXEL_ASPECT_RATIO: Guid =
    Guid::from_u128(0xc6376a1e_8d0a_4027_be45_6d9a0ad39bb6);
pub(super) const MF_MT_SUBTYPE: Guid = Guid::from_u128(0xf7e34c9a_42e8_4714_b74b_cb29d72c35e5);
pub(super) const MF_MT_VIDEO_ROTATION: Guid =
    Guid::from_u128(0xc380465d_2271_428c_9b83_ecea3b4a85c1);
pub(super) const MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS: Guid =
    Guid::from_u128(0xa634a91c_822b_41b9_a494_4de4643612b0);
const MEDIA_SOURCE_IID: Guid = Guid::from_u128(0x279a808d_aec7_40c8_9c6b_a6b492c78a66);

#[link(name = "ole32", kind = "raw-dylib")]
unsafe extern "system" {
    fn CoInitializeEx(reserved: *const c_void, concurrency: u32) -> Status;
    fn CoUninitialize();
}

#[link(name = "mfplat", kind = "raw-dylib")]
unsafe extern "system" {
    fn MFStartup(version: u32, flags: u32) -> Status;
    fn MFShutdown() -> Status;
    fn MFCreateAttributes(attributes: *mut Raw, capacity: u32) -> Status;
    fn MFCreateMediaType(media_type: *mut Raw) -> Status;
}

#[link(name = "mfreadwrite", kind = "raw-dylib")]
unsafe extern "system" {
    fn MFCreateSourceReaderFromURL(path: *const u16, attributes: Raw, reader: *mut Raw) -> Status;
    fn MFCreateSinkWriterFromURL(
        path: *const u16,
        stream: Raw,
        attributes: Raw,
        writer: *mut Raw,
    ) -> Status;
}

fn check(status: Status) -> Result<()> {
    if status < 0 {
        Err(Error(status))
    } else {
        Ok(())
    }
}

fn output<T: Default>(call: impl FnOnce(*mut T) -> Status) -> Result<T> {
    let mut value = T::default();
    check(call(&mut value))?;
    Ok(value)
}

pub(super) struct Session;

impl Session {
    pub(super) fn start() -> Result<Self> {
        unsafe {
            check(CoInitializeEx(null(), 0))?;
            if let Err(error) = check(MFStartup(MF_VERSION, 0)) {
                CoUninitialize();
                return Err(error);
            }
        }
        Ok(Self)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        unsafe {
            if let Err(error) = check(MFShutdown()) {
                log::warn!("Media Foundation shutdown: {error}");
            }
            CoUninitialize();
        }
    }
}

#[repr(C)]
pub(super) struct Unknown {
    query_interface: Raw,
    add_ref: Raw,
    release: unsafe extern "system" fn(Raw) -> u32,
}

pub(super) struct Com<V> {
    object: NonNull<c_void>,
    vtable: PhantomData<V>,
}

impl<V> Com<V> {
    unsafe fn owned(call: impl FnOnce(*mut Raw) -> Status) -> Result<Self> {
        NonNull::new(output(call)?)
            .map(Self::from_nonnull)
            .ok_or(E_POINTER)
    }

    fn from_nonnull(object: NonNull<c_void>) -> Self {
        Self {
            object,
            vtable: PhantomData,
        }
    }

    fn raw(&self) -> Raw {
        self.object.as_ptr()
    }

    fn table(&self) -> &V {
        unsafe { &**self.raw().cast::<*const V>() }
    }
}

impl<V> Drop for Com<V> {
    fn drop(&mut self) {
        unsafe {
            let unknown = &**self.raw().cast::<*const Unknown>();
            (unknown.release)(self.raw());
        }
    }
}

#[repr(C)]
pub(super) struct Attributes {
    unknown: Unknown,
    reserved_3_to_6: [Raw; 4],
    get_u32: unsafe extern "system" fn(Raw, *const Guid, *mut u32) -> Status,
    get_u64: unsafe extern "system" fn(Raw, *const Guid, *mut u64) -> Status,
    get_double: Raw,
    get_guid: unsafe extern "system" fn(Raw, *const Guid, *mut Guid) -> Status,
    reserved_11_to_20: [Raw; 10],
    set_u32: unsafe extern "system" fn(Raw, *const Guid, u32) -> Status,
    set_u64: unsafe extern "system" fn(Raw, *const Guid, u64) -> Status,
    set_double: Raw,
    set_guid: unsafe extern "system" fn(Raw, *const Guid, *const Guid) -> Status,
}

#[repr(C)]
pub(super) struct MediaSource {
    unknown: Unknown,
    reserved_3_to_7: [Raw; 5],
    descriptor: unsafe extern "system" fn(Raw, *mut Raw) -> Status,
}

#[repr(C)]
pub(super) struct Descriptor {
    unknown: Unknown,
    attributes: [Raw; 30],
    stream_count: unsafe extern "system" fn(Raw, *mut u32) -> Status,
}

#[repr(C)]
pub(super) struct Reader {
    unknown: Unknown,
    get_selection: Raw,
    select: unsafe extern "system" fn(Raw, u32, i32) -> Status,
    native_media_type: unsafe extern "system" fn(Raw, u32, u32, *mut Raw) -> Status,
    current_media_type: unsafe extern "system" fn(Raw, u32, *mut Raw) -> Status,
    set_media_type: unsafe extern "system" fn(Raw, u32, *const u32, Raw) -> Status,
    set_position: Raw,
    read_sample:
        unsafe extern "system" fn(Raw, u32, u32, *mut u32, *mut u32, *mut i64, *mut Raw) -> Status,
    flush: Raw,
    stream_service:
        unsafe extern "system" fn(Raw, u32, *const Guid, *const Guid, *mut Raw) -> Status,
}

#[repr(C)]
pub(super) struct Writer {
    unknown: Unknown,
    add_stream: unsafe extern "system" fn(Raw, Raw, *mut u32) -> Status,
    set_input: unsafe extern "system" fn(Raw, u32, Raw, Raw) -> Status,
    begin: unsafe extern "system" fn(Raw) -> Status,
    write_sample: unsafe extern "system" fn(Raw, u32, Raw) -> Status,
    send_tick: unsafe extern "system" fn(Raw, u32, i64) -> Status,
    reserved_8_to_10: [Raw; 3],
    finish: unsafe extern "system" fn(Raw) -> Status,
}

pub(super) fn create_attributes() -> Result<Com<Attributes>> {
    unsafe { Com::owned(|out| MFCreateAttributes(out, 1)) }
}

pub(super) fn create_media_type() -> Result<Com<Attributes>> {
    unsafe { Com::owned(|out| MFCreateMediaType(out)) }
}

pub(super) fn source_reader(path: &WidePath, attributes: &Com<Attributes>) -> Result<Com<Reader>> {
    unsafe { Com::owned(|out| MFCreateSourceReaderFromURL(path.as_ptr(), attributes.raw(), out)) }
}

pub(super) fn sink_writer(path: &WidePath, attributes: &Com<Attributes>) -> Result<Com<Writer>> {
    unsafe {
        Com::owned(|out| {
            MFCreateSinkWriterFromURL(path.as_ptr(), null_mut(), attributes.raw(), out)
        })
    }
}

impl Com<Attributes> {
    pub(super) fn u32(&self, key: &Guid) -> Result<u32> {
        output(|out| unsafe { (self.table().get_u32)(self.raw(), key, out) })
    }

    pub(super) fn u64(&self, key: &Guid) -> Result<u64> {
        output(|out| unsafe { (self.table().get_u64)(self.raw(), key, out) })
    }

    pub(super) fn guid(&self, key: &Guid) -> Result<Guid> {
        output(|out| unsafe { (self.table().get_guid)(self.raw(), key, out) })
    }

    pub(super) fn set_u32(&self, key: &Guid, value: u32) -> Result<()> {
        check(unsafe { (self.table().set_u32)(self.raw(), key, value) })
    }

    pub(super) fn set_u64(&self, key: &Guid, value: u64) -> Result<()> {
        check(unsafe { (self.table().set_u64)(self.raw(), key, value) })
    }

    pub(super) fn set_guid(&self, key: &Guid, value: &Guid) -> Result<()> {
        check(unsafe { (self.table().set_guid)(self.raw(), key, value) })
    }
}

impl Com<Reader> {
    pub(super) fn select(&self, stream: u32, enabled: bool) -> Result<()> {
        check(unsafe { (self.table().select)(self.raw(), stream, i32::from(enabled)) })
    }

    pub(super) fn native_media_type(&self, stream: u32) -> Result<Com<Attributes>> {
        unsafe { Com::owned(|out| (self.table().native_media_type)(self.raw(), stream, 0, out)) }
    }

    pub(super) fn current_media_type(&self, stream: u32) -> Result<Com<Attributes>> {
        unsafe { Com::owned(|out| (self.table().current_media_type)(self.raw(), stream, out)) }
    }

    pub(super) fn set_media_type(&self, stream: u32, media_type: &Com<Attributes>) -> Result<()> {
        check(unsafe {
            (self.table().set_media_type)(self.raw(), stream, null(), media_type.raw())
        })
    }

    pub(super) fn read_sample(&self) -> Result<(u32, u32, i64, Option<Com<Unknown>>)> {
        let (mut index, mut flags, mut timestamp, mut sample) = (0, 0, 0, null_mut());
        check(unsafe {
            (self.table().read_sample)(
                self.raw(),
                ALL_STREAMS,
                0,
                &mut index,
                &mut flags,
                &mut timestamp,
                &mut sample,
            )
        })?;
        Ok((
            index,
            flags,
            timestamp,
            NonNull::new(sample).map(Com::from_nonnull),
        ))
    }

    pub(super) fn media_source(&self) -> Result<Com<MediaSource>> {
        unsafe {
            Com::owned(|out| {
                (self.table().stream_service)(
                    self.raw(),
                    u32::MAX,
                    &Guid::default(),
                    &MEDIA_SOURCE_IID,
                    out,
                )
            })
        }
    }
}

impl Com<MediaSource> {
    pub(super) fn descriptor(&self) -> Result<Com<Descriptor>> {
        unsafe { Com::owned(|out| (self.table().descriptor)(self.raw(), out)) }
    }
}

impl Com<Descriptor> {
    pub(super) fn stream_count(&self) -> Result<u32> {
        output(|out| unsafe { (self.table().stream_count)(self.raw(), out) })
    }
}

impl Com<Writer> {
    pub(super) fn add_stream(&self, media_type: &Com<Attributes>) -> Result<u32> {
        output(|out| unsafe { (self.table().add_stream)(self.raw(), media_type.raw(), out) })
    }

    pub(super) fn set_input(&self, stream: u32, media_type: &Com<Attributes>) -> Result<()> {
        check(unsafe { (self.table().set_input)(self.raw(), stream, media_type.raw(), null_mut()) })
    }

    pub(super) fn begin(&self) -> Result<()> {
        check(unsafe { (self.table().begin)(self.raw()) })
    }

    pub(super) fn write_sample(&self, stream: u32, sample: &Com<Unknown>) -> Result<()> {
        check(unsafe { (self.table().write_sample)(self.raw(), stream, sample.raw()) })
    }

    pub(super) fn send_tick(&self, stream: u32, timestamp: i64) -> Result<()> {
        check(unsafe { (self.table().send_tick)(self.raw(), stream, timestamp) })
    }

    pub(super) fn finish(&self) -> Result<()> {
        check(unsafe { (self.table().finish)(self.raw()) })
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/platform/windows/media_foundation.rs"]
mod tests;
