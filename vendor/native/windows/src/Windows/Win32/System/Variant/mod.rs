#[inline]
pub unsafe fn VariantClear(pvarg: *mut windows_core::VARIANT) -> windows_core::Result<()> {
    windows_targets :: link ! ("oleaut32.dll" "system" fn VariantClear (pvarg : * mut core :: mem :: MaybeUninit < windows_core :: VARIANT >) -> windows_core :: HRESULT);
    VariantClear(core::mem::transmute(pvarg)).ok()
}
