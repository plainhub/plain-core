use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DarkTheme {
    UseDeviceTheme,
    On,
    Off,
}

impl<'de> Deserialize<'de> for DarkTheme {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        super::enum_value::deserialize(
            deserializer,
            &[
                (0, "USE_DEVICE_THEME", Self::UseDeviceTheme),
                (1, "ON", Self::On),
                (2, "OFF", Self::Off),
            ],
        )
    }
}
