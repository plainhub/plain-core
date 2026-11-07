pub mod flags {
    use crate::audio_tags::mp4::AtomIdent;

    pub const PODCAST: AtomIdent<'_> = AtomIdent::Fourcc(*b"pcst");

    pub const GAPLESS: AtomIdent<'_> = AtomIdent::Fourcc(*b"pgap");

    pub const SHOW_WORK: AtomIdent<'_> = AtomIdent::Fourcc(*b"shwm");

    pub const HD_VIDEO: AtomIdent<'_> = AtomIdent::Fourcc(*b"hdvd");

    pub const COMPILATION: AtomIdent<'_> = AtomIdent::Fourcc(*b"cpil");
}

pub(crate) const WELL_KNOWN_TYPE_SET: u8 = 0;
