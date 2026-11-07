use super::*;

#[test]
fn rectangle_bounds_reject_overflow_without_adding_untrusted_coordinates() {
    assert_eq!(
        check_crop_box(u32::MAX, u32::MAX, u32::MAX - 1, 0, 4, 1),
        Err(CropBoxError::SizeIsOutOfImageBoundaries)
    );
    assert_eq!(
        check_crop_box(u32::MAX, u32::MAX, 0, u32::MAX - 1, 1, 4),
        Err(CropBoxError::SizeIsOutOfImageBoundaries)
    );
    assert_eq!(
        check_crop_box(8, 8, 8, 0, 1, 1),
        Err(CropBoxError::PositionIsOutOfImageBoundaries)
    );
    assert!(check_crop_box(8, 8, 1, 2, 7, 6).is_ok());
}
