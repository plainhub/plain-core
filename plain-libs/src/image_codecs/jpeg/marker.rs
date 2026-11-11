#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Marker {
    SOF(u8),
    DHT,
    DAC,
    RST(u8),
    SOI,
    EOI,
    SOS,
    DQT,
    DNL,
    DRI,
    APP(u8),
    COM,
    UNKNOWN(u8),
}
impl Marker {
    pub fn from_u8(n: u8) -> Option<Marker> {
        use self::Marker::{APP, COM, DAC, DHT, DNL, DQT, DRI, EOI, RST, SOF, SOI, SOS, UNKNOWN};
        match n {
            0xFE => Some(COM),
            0xC0 => Some(SOF(0)),
            0xC1 => Some(SOF(1)),
            0xC2 => Some(SOF(2)),
            0xC4 => Some(DHT),
            0xCC => Some(DAC),
            0xD0 => Some(RST(0)),
            0xD1 => Some(RST(1)),
            0xD2 => Some(RST(2)),
            0xD3 => Some(RST(3)),
            0xD4 => Some(RST(4)),
            0xD5 => Some(RST(5)),
            0xD6 => Some(RST(6)),
            0xD7 => Some(RST(7)),
            0xD8 => Some(SOI),
            0xD9 => Some(EOI),
            0xDA => Some(SOS),
            0xDB => Some(DQT),
            0xDC => Some(DNL),
            0xDD => Some(DRI),
            0xE0 => Some(APP(0)),
            0xE1 => Some(APP(1)),
            0xE2 => Some(APP(2)),
            0xED => Some(APP(13)),
            0xEE => Some(APP(14)),
            _ => Some(UNKNOWN(n)),
        }
    }
}
