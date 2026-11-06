use super::*;
use std::mem::{offset_of, size_of};

// The upstream COM ABI includes unused methods; removing their slots corrupts every later call.
const _: () = {
    const SLOT: usize = size_of::<usize>();
    assert!(size_of::<IMFAttributes_Vtbl>() == 33 * SLOT);
    assert!(size_of::<IMFSinkWriter_Vtbl>() == 14 * SLOT);
    assert!(size_of::<IMFSourceReader_Vtbl>() == 13 * SLOT);
    assert!(size_of::<IMFSample_Vtbl>() == 47 * SLOT);
    assert!(size_of::<IMFMediaType_Vtbl>() == 38 * SLOT);
    assert!(size_of::<IMFMediaSource_Vtbl>() == 13 * SLOT);
    assert!(size_of::<IMFPresentationDescriptor_Vtbl>() == 38 * SLOT);
    assert!(offset_of!(IMFSourceReader_Vtbl, ReadSample) == 9 * SLOT);
    assert!(offset_of!(IMFSourceReader_Vtbl, GetServiceForStream) == 11 * SLOT);
    assert!(offset_of!(IMFSinkWriter_Vtbl, GetStatistics) == 13 * SLOT);
};

#[test]
fn media_interfaces_keep_their_upstream_identifiers() {
    assert_eq!(
        IMFSourceReader::IID,
        GUID::from_u128(0x70ae66f2_c809_4e4f_8915_bdcb406b7993)
    );
    assert_eq!(
        IMFSinkWriter::IID,
        GUID::from_u128(0x3137f1cd_fe5e_4805_a5d8_fb477448cb3d)
    );
    assert_eq!(
        IMFAttributes::IID,
        GUID::from_u128(0x2cd2d921_c447_44a7_a13c_4adabfc247e3)
    );
}
