#[inline]
pub unsafe fn MFCreateAttributes(
    ppmfattributes: *mut Option<IMFAttributes>,
    cinitialsize: u32,
) -> windows_core::Result<()> {
    windows_targets :: link ! ("mfplat.dll" "system" fn MFCreateAttributes (ppmfattributes : * mut * mut core :: ffi :: c_void , cinitialsize : u32) -> windows_core :: HRESULT);
    MFCreateAttributes(core::mem::transmute(ppmfattributes), cinitialsize).ok()
}
#[inline]
pub unsafe fn MFCreateMediaType() -> windows_core::Result<IMFMediaType> {
    windows_targets :: link ! ("mfplat.dll" "system" fn MFCreateMediaType (ppmftype : * mut * mut core :: ffi :: c_void) -> windows_core :: HRESULT);
    let mut result__ = core::mem::zeroed();
    MFCreateMediaType(&mut result__).and_then(|| windows_core::Type::from_abi(result__))
}
#[inline]
pub unsafe fn MFCreateSinkWriterFromURL<P0, P1, P2>(
    pwszoutputurl: P0,
    pbytestream: P1,
    pattributes: P2,
) -> windows_core::Result<IMFSinkWriter>
where
    P0: windows_core::Param<windows_core::PCWSTR>,
    P1: windows_core::Param<IMFByteStream>,
    P2: windows_core::Param<IMFAttributes>,
{
    windows_targets :: link ! ("mfreadwrite.dll" "system" fn MFCreateSinkWriterFromURL (pwszoutputurl : windows_core :: PCWSTR , pbytestream : * mut core :: ffi :: c_void , pattributes : * mut core :: ffi :: c_void , ppsinkwriter : * mut * mut core :: ffi :: c_void) -> windows_core :: HRESULT);
    let mut result__ = core::mem::zeroed();
    MFCreateSinkWriterFromURL(
        pwszoutputurl.param().abi(),
        pbytestream.param().abi(),
        pattributes.param().abi(),
        &mut result__,
    )
    .and_then(|| windows_core::Type::from_abi(result__))
}
#[inline]
pub unsafe fn MFCreateSourceReaderFromURL<P0, P1>(
    pwszurl: P0,
    pattributes: P1,
) -> windows_core::Result<IMFSourceReader>
where
    P0: windows_core::Param<windows_core::PCWSTR>,
    P1: windows_core::Param<IMFAttributes>,
{
    windows_targets :: link ! ("mfreadwrite.dll" "system" fn MFCreateSourceReaderFromURL (pwszurl : windows_core :: PCWSTR , pattributes : * mut core :: ffi :: c_void , ppsourcereader : * mut * mut core :: ffi :: c_void) -> windows_core :: HRESULT);
    let mut result__ = core::mem::zeroed();
    MFCreateSourceReaderFromURL(
        pwszurl.param().abi(),
        pattributes.param().abi(),
        &mut result__,
    )
    .and_then(|| windows_core::Type::from_abi(result__))
}
#[inline]
pub unsafe fn MFShutdown() -> windows_core::Result<()> {
    windows_targets :: link ! ("mfplat.dll" "system" fn MFShutdown () -> windows_core :: HRESULT);
    MFShutdown().ok()
}
#[inline]
pub unsafe fn MFStartup(version: u32, dwflags: u32) -> windows_core::Result<()> {
    windows_targets :: link ! ("mfplat.dll" "system" fn MFStartup (version : u32 , dwflags : u32) -> windows_core :: HRESULT);
    MFStartup(version, dwflags).ok()
}
windows_core::imp::define_interface!(
    IMFAttributes,
    IMFAttributes_Vtbl,
    0x2cd2d921_c447_44a7_a13c_4adabfc247e3
);
impl core::ops::Deref for IMFAttributes {
    type Target = windows_core::IUnknown;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(IMFAttributes, windows_core::IUnknown);
impl IMFAttributes {
    pub unsafe fn GetUINT32(
        &self,
        guidkey: *const windows_core::GUID,
    ) -> windows_core::Result<u32> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).GetUINT32)(
            windows_core::Interface::as_raw(self),
            guidkey,
            &mut result__,
        )
        .map(|| result__)
    }
    pub unsafe fn GetUINT64(
        &self,
        guidkey: *const windows_core::GUID,
    ) -> windows_core::Result<u64> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).GetUINT64)(
            windows_core::Interface::as_raw(self),
            guidkey,
            &mut result__,
        )
        .map(|| result__)
    }
    pub unsafe fn GetGUID(
        &self,
        guidkey: *const windows_core::GUID,
    ) -> windows_core::Result<windows_core::GUID> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).GetGUID)(
            windows_core::Interface::as_raw(self),
            guidkey,
            &mut result__,
        )
        .map(|| result__)
    }
    pub unsafe fn SetUINT32(
        &self,
        guidkey: *const windows_core::GUID,
        unvalue: u32,
    ) -> windows_core::Result<()> {
        (windows_core::Interface::vtable(self).SetUINT32)(
            windows_core::Interface::as_raw(self),
            guidkey,
            unvalue,
        )
        .ok()
    }
    pub unsafe fn SetUINT64(
        &self,
        guidkey: *const windows_core::GUID,
        unvalue: u64,
    ) -> windows_core::Result<()> {
        (windows_core::Interface::vtable(self).SetUINT64)(
            windows_core::Interface::as_raw(self),
            guidkey,
            unvalue,
        )
        .ok()
    }
    pub unsafe fn SetGUID(
        &self,
        guidkey: *const windows_core::GUID,
        guidvalue: *const windows_core::GUID,
    ) -> windows_core::Result<()> {
        (windows_core::Interface::vtable(self).SetGUID)(
            windows_core::Interface::as_raw(self),
            guidkey,
            guidvalue,
        )
        .ok()
    }
}
#[repr(C)]
pub struct IMFAttributes_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub GetItem: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut core::mem::MaybeUninit<windows_core::PROPVARIANT>,
    ) -> windows_core::HRESULT,
    pub GetItemType: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut MF_ATTRIBUTE_TYPE,
    ) -> windows_core::HRESULT,
    pub CompareItem: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *const core::mem::MaybeUninit<windows_core::PROPVARIANT>,
        *mut super::super::Foundation::BOOL,
    ) -> windows_core::HRESULT,
    pub Compare: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        MF_ATTRIBUTES_MATCH_TYPE,
        *mut super::super::Foundation::BOOL,
    ) -> windows_core::HRESULT,
    pub GetUINT32: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub GetUINT64: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut u64,
    ) -> windows_core::HRESULT,
    pub GetDouble: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut f64,
    ) -> windows_core::HRESULT,
    pub GetGUID: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut windows_core::GUID,
    ) -> windows_core::HRESULT,
    pub GetStringLength: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub GetString: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        windows_core::PWSTR,
        u32,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub GetAllocatedString: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut windows_core::PWSTR,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub GetBlobSize: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub GetBlob: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut u8,
        u32,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub GetAllocatedBlob: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut *mut u8,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub GetUnknown: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SetItem: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *const core::mem::MaybeUninit<windows_core::PROPVARIANT>,
    ) -> windows_core::HRESULT,
    pub DeleteItem: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
    ) -> windows_core::HRESULT,
    pub DeleteAllItems: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub SetUINT32: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        u32,
    ) -> windows_core::HRESULT,
    pub SetUINT64: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        u64,
    ) -> windows_core::HRESULT,
    pub SetDouble: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        f64,
    ) -> windows_core::HRESULT,
    pub SetGUID: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *const windows_core::GUID,
    ) -> windows_core::HRESULT,
    pub SetString: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        windows_core::PCWSTR,
    ) -> windows_core::HRESULT,
    pub SetBlob: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *const u8,
        u32,
    ) -> windows_core::HRESULT,
    pub SetUnknown: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub LockStore: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub UnlockStore: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub GetCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    pub GetItemByIndex: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut windows_core::GUID,
        *mut core::mem::MaybeUninit<windows_core::PROPVARIANT>,
    ) -> windows_core::HRESULT,
    pub CopyAllItems: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IMFByteStream,
    IMFByteStream_Vtbl,
    0xad4c1b00_4bf7_422f_9175_756693d9130d
);
impl core::ops::Deref for IMFByteStream {
    type Target = windows_core::IUnknown;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(IMFByteStream, windows_core::IUnknown);
impl IMFByteStream {
    pub unsafe fn Read(&self, pb: &mut [u8], pcbread: *mut u32) -> windows_core::Result<()> {
        (windows_core::Interface::vtable(self).Read)(
            windows_core::Interface::as_raw(self),
            core::mem::transmute(pb.as_ptr()),
            pb.len().try_into().unwrap(),
            pcbread,
        )
        .ok()
    }
    pub unsafe fn Write(&self, pb: &[u8]) -> windows_core::Result<u32> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).Write)(
            windows_core::Interface::as_raw(self),
            core::mem::transmute(pb.as_ptr()),
            pb.len().try_into().unwrap(),
            &mut result__,
        )
        .map(|| result__)
    }
    pub unsafe fn Seek(
        &self,
        seekorigin: MFBYTESTREAM_SEEK_ORIGIN,
        llseekoffset: i64,
        dwseekflags: u32,
    ) -> windows_core::Result<u64> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).Seek)(
            windows_core::Interface::as_raw(self),
            seekorigin,
            llseekoffset,
            dwseekflags,
            &mut result__,
        )
        .map(|| result__)
    }
}
#[repr(C)]
pub struct IMFByteStream_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub GetCapabilities:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    pub GetLength:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u64) -> windows_core::HRESULT,
    pub SetLength: unsafe extern "system" fn(*mut core::ffi::c_void, u64) -> windows_core::HRESULT,
    pub GetCurrentPosition:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u64) -> windows_core::HRESULT,
    pub SetCurrentPosition:
        unsafe extern "system" fn(*mut core::ffi::c_void, u64) -> windows_core::HRESULT,
    pub IsEndOfStream: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut super::super::Foundation::BOOL,
    ) -> windows_core::HRESULT,
    pub Read: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut u8,
        u32,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub BeginRead: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut u8,
        u32,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub EndRead: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub Write: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const u8,
        u32,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub BeginWrite: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const u8,
        u32,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub EndWrite: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub Seek: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        MFBYTESTREAM_SEEK_ORIGIN,
        i64,
        u32,
        *mut u64,
    ) -> windows_core::HRESULT,
    pub Flush: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub Close: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IMFMediaEventGenerator,
    IMFMediaEventGenerator_Vtbl,
    0x2cd0bd52_bcd5_4b89_b62c_eadc0c031e7d
);
impl core::ops::Deref for IMFMediaEventGenerator {
    type Target = windows_core::IUnknown;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(IMFMediaEventGenerator, windows_core::IUnknown);
impl IMFMediaEventGenerator {}
#[repr(C)]
pub struct IMFMediaEventGenerator_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub GetEvent: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub BeginGetEvent: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub EndGetEvent: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub QueueEvent: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *const windows_core::GUID,
        windows_core::HRESULT,
        *const core::mem::MaybeUninit<windows_core::PROPVARIANT>,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IMFMediaSource,
    IMFMediaSource_Vtbl,
    0x279a808d_aec7_40c8_9c6b_a6b492c78a66
);
impl core::ops::Deref for IMFMediaSource {
    type Target = IMFMediaEventGenerator;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IMFMediaSource,
    windows_core::IUnknown,
    IMFMediaEventGenerator
);
impl IMFMediaSource {
    pub unsafe fn CreatePresentationDescriptor(
        &self,
    ) -> windows_core::Result<IMFPresentationDescriptor> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).CreatePresentationDescriptor)(
            windows_core::Interface::as_raw(self),
            &mut result__,
        )
        .and_then(|| windows_core::Type::from_abi(result__))
    }
    pub unsafe fn Stop(&self) -> windows_core::Result<()> {
        (windows_core::Interface::vtable(self).Stop)(windows_core::Interface::as_raw(self)).ok()
    }
}
#[repr(C)]
pub struct IMFMediaSource_Vtbl {
    pub base__: IMFMediaEventGenerator_Vtbl,
    pub GetCharacteristics:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    pub CreatePresentationDescriptor: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub Start: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *const core::mem::MaybeUninit<windows_core::PROPVARIANT>,
    ) -> windows_core::HRESULT,
    pub Stop: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub Pause: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub Shutdown: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IMFMediaType,
    IMFMediaType_Vtbl,
    0x44ae0fa8_ea31_4109_8d2e_4cae4997c555
);
impl core::ops::Deref for IMFMediaType {
    type Target = IMFAttributes;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(IMFMediaType, windows_core::IUnknown, IMFAttributes);
impl IMFMediaType {}
#[repr(C)]
pub struct IMFMediaType_Vtbl {
    pub base__: IMFAttributes_Vtbl,
    pub GetMajorType: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut windows_core::GUID,
    ) -> windows_core::HRESULT,
    pub IsCompressedFormat: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut super::super::Foundation::BOOL,
    ) -> windows_core::HRESULT,
    pub IsEqual: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub GetRepresentation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub FreeRepresentation: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        windows_core::GUID,
        *const core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IMFPresentationDescriptor,
    IMFPresentationDescriptor_Vtbl,
    0x03cb2711_24d7_4db6_a17f_f3a7a479a536
);
impl core::ops::Deref for IMFPresentationDescriptor {
    type Target = IMFAttributes;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(
    IMFPresentationDescriptor,
    windows_core::IUnknown,
    IMFAttributes
);
impl IMFPresentationDescriptor {
    pub unsafe fn GetStreamDescriptorCount(&self) -> windows_core::Result<u32> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).GetStreamDescriptorCount)(
            windows_core::Interface::as_raw(self),
            &mut result__,
        )
        .map(|| result__)
    }
    pub unsafe fn Clone(&self) -> windows_core::Result<IMFPresentationDescriptor> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).Clone)(
            windows_core::Interface::as_raw(self),
            &mut result__,
        )
        .and_then(|| windows_core::Type::from_abi(result__))
    }
}
#[repr(C)]
pub struct IMFPresentationDescriptor_Vtbl {
    pub base__: IMFAttributes_Vtbl,
    pub GetStreamDescriptorCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    pub GetStreamDescriptorByIndex: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut super::super::Foundation::BOOL,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SelectStream:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    pub DeselectStream:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    pub Clone: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IMFSample,
    IMFSample_Vtbl,
    0xc40a00f2_b93a_4d80_ae8c_5a1c634f58e4
);
impl core::ops::Deref for IMFSample {
    type Target = IMFAttributes;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(IMFSample, windows_core::IUnknown, IMFAttributes);
impl IMFSample {}
#[repr(C)]
pub struct IMFSample_Vtbl {
    pub base__: IMFAttributes_Vtbl,
    pub GetSampleFlags:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    pub SetSampleFlags:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    pub GetSampleTime:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub SetSampleTime:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub GetSampleDuration:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut i64) -> windows_core::HRESULT,
    pub SetSampleDuration:
        unsafe extern "system" fn(*mut core::ffi::c_void, i64) -> windows_core::HRESULT,
    pub GetBufferCount:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    pub GetBufferByIndex: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub ConvertToContiguousBuffer: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub AddBuffer: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub RemoveBufferByIndex:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    pub RemoveAllBuffers:
        unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub GetTotalLength:
        unsafe extern "system" fn(*mut core::ffi::c_void, *mut u32) -> windows_core::HRESULT,
    pub CopyToBuffer: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IMFSinkWriter,
    IMFSinkWriter_Vtbl,
    0x3137f1cd_fe5e_4805_a5d8_fb477448cb3d
);
impl core::ops::Deref for IMFSinkWriter {
    type Target = windows_core::IUnknown;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(IMFSinkWriter, windows_core::IUnknown);
impl IMFSinkWriter {
    pub unsafe fn AddStream<P0>(&self, ptargetmediatype: P0) -> windows_core::Result<u32>
    where
        P0: windows_core::Param<IMFMediaType>,
    {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).AddStream)(
            windows_core::Interface::as_raw(self),
            ptargetmediatype.param().abi(),
            &mut result__,
        )
        .map(|| result__)
    }
    pub unsafe fn SetInputMediaType<P0, P1>(
        &self,
        dwstreamindex: u32,
        pinputmediatype: P0,
        pencodingparameters: P1,
    ) -> windows_core::Result<()>
    where
        P0: windows_core::Param<IMFMediaType>,
        P1: windows_core::Param<IMFAttributes>,
    {
        (windows_core::Interface::vtable(self).SetInputMediaType)(
            windows_core::Interface::as_raw(self),
            dwstreamindex,
            pinputmediatype.param().abi(),
            pencodingparameters.param().abi(),
        )
        .ok()
    }
    pub unsafe fn BeginWriting(&self) -> windows_core::Result<()> {
        (windows_core::Interface::vtable(self).BeginWriting)(windows_core::Interface::as_raw(self))
            .ok()
    }
    pub unsafe fn WriteSample<P0>(
        &self,
        dwstreamindex: u32,
        psample: P0,
    ) -> windows_core::Result<()>
    where
        P0: windows_core::Param<IMFSample>,
    {
        (windows_core::Interface::vtable(self).WriteSample)(
            windows_core::Interface::as_raw(self),
            dwstreamindex,
            psample.param().abi(),
        )
        .ok()
    }
    pub unsafe fn SendStreamTick(
        &self,
        dwstreamindex: u32,
        lltimestamp: i64,
    ) -> windows_core::Result<()> {
        (windows_core::Interface::vtable(self).SendStreamTick)(
            windows_core::Interface::as_raw(self),
            dwstreamindex,
            lltimestamp,
        )
        .ok()
    }
    pub unsafe fn Finalize(&self) -> windows_core::Result<()> {
        (windows_core::Interface::vtable(self).Finalize)(windows_core::Interface::as_raw(self)).ok()
    }
    pub unsafe fn GetServiceForStream(
        &self,
        dwstreamindex: u32,
        guidservice: *const windows_core::GUID,
        riid: *const windows_core::GUID,
        ppvobject: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        (windows_core::Interface::vtable(self).GetServiceForStream)(
            windows_core::Interface::as_raw(self),
            dwstreamindex,
            guidservice,
            riid,
            ppvobject,
        )
        .ok()
    }
}
#[repr(C)]
pub struct IMFSinkWriter_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub AddStream: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
        *mut u32,
    ) -> windows_core::HRESULT,
    pub SetInputMediaType: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut core::ffi::c_void,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub BeginWriting: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub WriteSample: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SendStreamTick:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32, i64) -> windows_core::HRESULT,
    pub PlaceMarker: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *const core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub NotifyEndOfSegment:
        unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    pub Flush: unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    pub Finalize: unsafe extern "system" fn(*mut core::ffi::c_void) -> windows_core::HRESULT,
    pub GetServiceForStream: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *const windows_core::GUID,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetStatistics: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut MF_SINK_WRITER_STATISTICS,
    ) -> windows_core::HRESULT,
}
windows_core::imp::define_interface!(
    IMFSourceReader,
    IMFSourceReader_Vtbl,
    0x70ae66f2_c809_4e4f_8915_bdcb406b7993
);
impl core::ops::Deref for IMFSourceReader {
    type Target = windows_core::IUnknown;
    fn deref(&self) -> &Self::Target {
        unsafe { core::mem::transmute(self) }
    }
}
windows_core::imp::interface_hierarchy!(IMFSourceReader, windows_core::IUnknown);
impl IMFSourceReader {
    pub unsafe fn SetStreamSelection<P0>(
        &self,
        dwstreamindex: u32,
        fselected: P0,
    ) -> windows_core::Result<()>
    where
        P0: windows_core::Param<super::super::Foundation::BOOL>,
    {
        (windows_core::Interface::vtable(self).SetStreamSelection)(
            windows_core::Interface::as_raw(self),
            dwstreamindex,
            fselected.param().abi(),
        )
        .ok()
    }
    pub unsafe fn GetNativeMediaType(
        &self,
        dwstreamindex: u32,
        dwmediatypeindex: u32,
    ) -> windows_core::Result<IMFMediaType> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).GetNativeMediaType)(
            windows_core::Interface::as_raw(self),
            dwstreamindex,
            dwmediatypeindex,
            &mut result__,
        )
        .and_then(|| windows_core::Type::from_abi(result__))
    }
    pub unsafe fn GetCurrentMediaType(
        &self,
        dwstreamindex: u32,
    ) -> windows_core::Result<IMFMediaType> {
        let mut result__ = core::mem::zeroed();
        (windows_core::Interface::vtable(self).GetCurrentMediaType)(
            windows_core::Interface::as_raw(self),
            dwstreamindex,
            &mut result__,
        )
        .and_then(|| windows_core::Type::from_abi(result__))
    }
    pub unsafe fn SetCurrentMediaType<P0>(
        &self,
        dwstreamindex: u32,
        pdwreserved: Option<*const u32>,
        pmediatype: P0,
    ) -> windows_core::Result<()>
    where
        P0: windows_core::Param<IMFMediaType>,
    {
        (windows_core::Interface::vtable(self).SetCurrentMediaType)(
            windows_core::Interface::as_raw(self),
            dwstreamindex,
            core::mem::transmute(pdwreserved.unwrap_or(std::ptr::null())),
            pmediatype.param().abi(),
        )
        .ok()
    }
    pub unsafe fn ReadSample(
        &self,
        dwstreamindex: u32,
        dwcontrolflags: u32,
        pdwactualstreamindex: Option<*mut u32>,
        pdwstreamflags: Option<*mut u32>,
        plltimestamp: Option<*mut i64>,
        ppsample: Option<*mut Option<IMFSample>>,
    ) -> windows_core::Result<()> {
        (windows_core::Interface::vtable(self).ReadSample)(
            windows_core::Interface::as_raw(self),
            dwstreamindex,
            dwcontrolflags,
            core::mem::transmute(pdwactualstreamindex.unwrap_or(std::ptr::null_mut())),
            core::mem::transmute(pdwstreamflags.unwrap_or(std::ptr::null_mut())),
            core::mem::transmute(plltimestamp.unwrap_or(std::ptr::null_mut())),
            core::mem::transmute(ppsample.unwrap_or(std::ptr::null_mut())),
        )
        .ok()
    }
    pub unsafe fn GetServiceForStream(
        &self,
        dwstreamindex: u32,
        guidservice: *const windows_core::GUID,
        riid: *const windows_core::GUID,
        ppvobject: *mut *mut core::ffi::c_void,
    ) -> windows_core::Result<()> {
        (windows_core::Interface::vtable(self).GetServiceForStream)(
            windows_core::Interface::as_raw(self),
            dwstreamindex,
            guidservice,
            riid,
            ppvobject,
        )
        .ok()
    }
}
#[repr(C)]
pub struct IMFSourceReader_Vtbl {
    pub base__: windows_core::IUnknown_Vtbl,
    pub GetStreamSelection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut super::super::Foundation::BOOL,
    ) -> windows_core::HRESULT,
    pub SetStreamSelection: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        super::super::Foundation::BOOL,
    ) -> windows_core::HRESULT,
    pub GetNativeMediaType: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetCurrentMediaType: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SetCurrentMediaType: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *const u32,
        *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub SetCurrentPosition: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        *const windows_core::GUID,
        *const core::mem::MaybeUninit<windows_core::PROPVARIANT>,
    ) -> windows_core::HRESULT,
    pub ReadSample: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        u32,
        *mut u32,
        *mut u32,
        *mut i64,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub Flush: unsafe extern "system" fn(*mut core::ffi::c_void, u32) -> windows_core::HRESULT,
    pub GetServiceForStream: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *const windows_core::GUID,
        *const windows_core::GUID,
        *mut *mut core::ffi::c_void,
    ) -> windows_core::HRESULT,
    pub GetPresentationAttribute: unsafe extern "system" fn(
        *mut core::ffi::c_void,
        u32,
        *const windows_core::GUID,
        *mut core::mem::MaybeUninit<windows_core::PROPVARIANT>,
    ) -> windows_core::HRESULT,
}
pub const MFMediaType_Audio: windows_core::GUID =
    windows_core::GUID::from_u128(0x73647561_0000_0010_8000_00aa00389b71);
pub const MFMediaType_Video: windows_core::GUID =
    windows_core::GUID::from_u128(0x73646976_0000_0010_8000_00aa00389b71);
pub const MFSTARTUP_FULL: u32 = 0u32;
pub const MFVideoFormat_H264: windows_core::GUID =
    windows_core::GUID::from_u128(0x34363248_0000_0010_8000_00aa00389b71);
pub const MFVideoFormat_NV12: windows_core::GUID =
    windows_core::GUID::from_u128(0x3231564e_0000_0010_8000_00aa00389b71);
pub const MF_E_ATTRIBUTENOTFOUND: windows_core::HRESULT =
    windows_core::HRESULT(0xC00D36E6_u32 as _);
pub const MF_MT_AVG_BITRATE: windows_core::GUID =
    windows_core::GUID::from_u128(0x20332624_fb0d_4d9e_bd0d_cbf6786c102e);
pub const MF_MT_FRAME_RATE: windows_core::GUID =
    windows_core::GUID::from_u128(0xc459a2e8_3d2c_4e44_b132_fee5156c7bb0);
pub const MF_MT_FRAME_SIZE: windows_core::GUID =
    windows_core::GUID::from_u128(0x1652c33d_d6b2_4012_b834_72030849a37d);
pub const MF_MT_INTERLACE_MODE: windows_core::GUID =
    windows_core::GUID::from_u128(0xe2724bb8_e676_4806_b4b2_a8d6efb44ccd);
pub const MF_MT_MAJOR_TYPE: windows_core::GUID =
    windows_core::GUID::from_u128(0x48eba18e_f8c9_4687_bf11_0a74c9f96a8f);
pub const MF_MT_PIXEL_ASPECT_RATIO: windows_core::GUID =
    windows_core::GUID::from_u128(0xc6376a1e_8d0a_4027_be45_6d9a0ad39bb6);
pub const MF_MT_SUBTYPE: windows_core::GUID =
    windows_core::GUID::from_u128(0xf7e34c9a_42e8_4714_b74b_cb29d72c35e5);
pub const MF_MT_VIDEO_ROTATION: windows_core::GUID =
    windows_core::GUID::from_u128(0xc380465d_2271_428c_9b83_ecea3b4a85c1);
pub const MF_READWRITE_ENABLE_HARDWARE_TRANSFORMS: windows_core::GUID =
    windows_core::GUID::from_u128(0xa634a91c_822b_41b9_a494_4de4643612b0);
pub const MF_SOURCE_READERF_CURRENTMEDIATYPECHANGED: MF_SOURCE_READER_FLAG =
    MF_SOURCE_READER_FLAG(32i32);
pub const MF_SOURCE_READERF_ENDOFSTREAM: MF_SOURCE_READER_FLAG = MF_SOURCE_READER_FLAG(2i32);
pub const MF_SOURCE_READERF_ERROR: MF_SOURCE_READER_FLAG = MF_SOURCE_READER_FLAG(1i32);
pub const MF_SOURCE_READERF_STREAMTICK: MF_SOURCE_READER_FLAG = MF_SOURCE_READER_FLAG(256i32);
pub const MF_SOURCE_READER_ALL_STREAMS: MF_SOURCE_READER_CONSTANTS =
    MF_SOURCE_READER_CONSTANTS(-2i32);
pub const MF_SOURCE_READER_ANY_STREAM: MF_SOURCE_READER_CONSTANTS =
    MF_SOURCE_READER_CONSTANTS(-2i32);
pub const MF_SOURCE_READER_MEDIASOURCE: MF_SOURCE_READER_CONSTANTS =
    MF_SOURCE_READER_CONSTANTS(-1i32);
pub const MF_VERSION: u32 = 131184u32;
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS(pub u32);
impl windows_core::TypeKind for MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("MEDIA_EVENT_GENERATOR_GET_EVENT_FLAGS")
            .field(&self.0)
            .finish()
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct MFBYTESTREAM_SEEK_ORIGIN(pub i32);
impl windows_core::TypeKind for MFBYTESTREAM_SEEK_ORIGIN {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for MFBYTESTREAM_SEEK_ORIGIN {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("MFBYTESTREAM_SEEK_ORIGIN")
            .field(&self.0)
            .finish()
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct MF_ATTRIBUTES_MATCH_TYPE(pub i32);
impl windows_core::TypeKind for MF_ATTRIBUTES_MATCH_TYPE {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for MF_ATTRIBUTES_MATCH_TYPE {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("MF_ATTRIBUTES_MATCH_TYPE")
            .field(&self.0)
            .finish()
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct MF_ATTRIBUTE_TYPE(pub i32);
impl windows_core::TypeKind for MF_ATTRIBUTE_TYPE {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for MF_ATTRIBUTE_TYPE {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("MF_ATTRIBUTE_TYPE").field(&self.0).finish()
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct MF_SOURCE_READER_CONSTANTS(pub i32);
impl windows_core::TypeKind for MF_SOURCE_READER_CONSTANTS {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for MF_SOURCE_READER_CONSTANTS {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("MF_SOURCE_READER_CONSTANTS")
            .field(&self.0)
            .finish()
    }
}
#[repr(transparent)]
#[derive(PartialEq, Eq, Copy, Clone, Default)]
pub struct MF_SOURCE_READER_FLAG(pub i32);
impl windows_core::TypeKind for MF_SOURCE_READER_FLAG {
    type TypeKind = windows_core::CopyType;
}
impl core::fmt::Debug for MF_SOURCE_READER_FLAG {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("MF_SOURCE_READER_FLAG")
            .field(&self.0)
            .finish()
    }
}
impl MF_SOURCE_READER_FLAG {
    pub const fn contains(&self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
impl core::ops::BitOr for MF_SOURCE_READER_FLAG {
    type Output = Self;
    fn bitor(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}
impl core::ops::BitAnd for MF_SOURCE_READER_FLAG {
    type Output = Self;
    fn bitand(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
}
impl core::ops::BitOrAssign for MF_SOURCE_READER_FLAG {
    fn bitor_assign(&mut self, other: Self) {
        self.0.bitor_assign(other.0)
    }
}
impl core::ops::BitAndAssign for MF_SOURCE_READER_FLAG {
    fn bitand_assign(&mut self, other: Self) {
        self.0.bitand_assign(other.0)
    }
}
impl core::ops::Not for MF_SOURCE_READER_FLAG {
    type Output = Self;
    fn not(self) -> Self {
        Self(self.0.not())
    }
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MF_SINK_WRITER_STATISTICS {
    pub cb: u32,
    pub llLastTimestampReceived: i64,
    pub llLastTimestampEncoded: i64,
    pub llLastTimestampProcessed: i64,
    pub llLastStreamTickReceived: i64,
    pub llLastSinkSampleRequest: i64,
    pub qwNumSamplesReceived: u64,
    pub qwNumSamplesEncoded: u64,
    pub qwNumSamplesProcessed: u64,
    pub qwNumStreamTicksReceived: u64,
    pub dwByteCountQueued: u32,
    pub qwByteCountProcessed: u64,
    pub dwNumOutstandingSinkSampleRequests: u32,
    pub dwAverageSampleRateReceived: u32,
    pub dwAverageSampleRateEncoded: u32,
    pub dwAverageSampleRateProcessed: u32,
}
impl windows_core::TypeKind for MF_SINK_WRITER_STATISTICS {
    type TypeKind = windows_core::CopyType;
}
impl Default for MF_SINK_WRITER_STATISTICS {
    fn default() -> Self {
        unsafe { core::mem::zeroed() }
    }
}
#[cfg(any())]
core::include!("impl.rs");
