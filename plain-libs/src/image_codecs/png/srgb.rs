use crate::image_codecs::png::{ScaledFloat, SourceChromaticities};
pub fn substitute_gamma() -> ScaledFloat {
    ScaledFloat::from_scaled(45455)
}
pub fn substitute_chromaticities() -> SourceChromaticities {
    SourceChromaticities {
        white: (
            ScaledFloat::from_scaled(31270),
            ScaledFloat::from_scaled(32900),
        ),
        red: (
            ScaledFloat::from_scaled(64000),
            ScaledFloat::from_scaled(33000),
        ),
        green: (
            ScaledFloat::from_scaled(30000),
            ScaledFloat::from_scaled(60000),
        ),
        blue: (
            ScaledFloat::from_scaled(15000),
            ScaledFloat::from_scaled(6000),
        ),
    }
}
