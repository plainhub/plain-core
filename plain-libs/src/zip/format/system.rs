#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
#[repr(u8)]
#[non_exhaustive]
pub enum System {
    Dos = 0,
    Amiga = 1,
    OpenVMS = 2,
    Unix = 3,
    VmCms = 4,
    AtariSt = 5,
    Os2 = 6,
    Macintosh = 7,
    ZSystem = 8,
    CPM = 9,
    WindowsNTFS = 10,
    MVS = 11,
    VSE = 12,
    AcornRisc = 13,
    VFAT = 14,
    AlternateMVS = 15,
    BeOS = 16,
    Tandem = 17,
    Os400 = 18,
    OsDarwin = 19,
    #[default]
    Unknown = 255,
}
impl System {
    pub(crate) fn extract_bytes(version_made_by: u16) -> (u8, Self) {
        let bytes = version_made_by.to_le_bytes();
        (bytes[0], Self::from(bytes[1]))
    }
}
impl From<u8> for System {
    fn from(system: u8) -> Self {
        match system {
            0 => System::Dos,
            1 => System::Amiga,
            2 => System::OpenVMS,
            3 => System::Unix,
            4 => System::VmCms,
            5 => System::AtariSt,
            6 => System::Os2,
            7 => System::Macintosh,
            8 => System::ZSystem,
            9 => System::CPM,
            10 => System::WindowsNTFS,
            11 => System::MVS,
            12 => System::VSE,
            13 => System::AcornRisc,
            14 => System::VFAT,
            15 => System::AlternateMVS,
            16 => System::BeOS,
            17 => System::Tandem,
            18 => System::Os400,
            19 => System::OsDarwin,
            _ => System::Unknown,
        }
    }
}
impl From<System> for u8 {
    fn from(system: System) -> Self {
        system as u8
    }
}
