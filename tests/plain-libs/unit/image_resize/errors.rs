use super::*;
use std::error::Error as _;

#[test]
fn image_conversion_retains_typed_cause_without_changing_messages() {
    let error = ResizeError::from(ImageError::UnsupportedPixelType);
    assert_eq!(
        error.to_string(),
        "Source or destination image is not supported"
    );
    assert_eq!(
        error.source().unwrap().downcast_ref::<ImageError>(),
        Some(&ImageError::UnsupportedPixelType)
    );
    assert_eq!(
        error.source().unwrap().to_string(),
        "Pixel type of image is not supported"
    );
    assert!(error.source().unwrap().source().is_none());
    assert_eq!(
        InvalidPixelsSize.to_string(),
        "Size of container with pixels is smaller than required"
    );
    assert!(InvalidPixelsSize.source().is_none());
}
