use super::*;

#[derive(Debug, PartialEq, Eq, FromRepr, EnumCount)]
#[repr(i16)]
#[strum(crate = "crate")]
enum Sample {
    Below = -3,
    Zero = 0,
    Above,
    #[strum(disabled)]
    Hidden = 8,
}

#[test]
fn retained_derives_handle_signed_sparse_and_disabled_variants() {
    const BELOW: Option<Sample> = Sample::from_repr(-3);
    assert_eq!(BELOW, Some(Sample::Below));
    assert_eq!(Sample::from_repr(0), Some(Sample::Zero));
    assert_eq!(Sample::from_repr(1), Some(Sample::Above));
    assert_eq!(Sample::from_repr(-2), None);
    assert_eq!(Sample::from_repr(Sample::Hidden as i16), None);
    assert_eq!(Sample::COUNT, 3);
}
