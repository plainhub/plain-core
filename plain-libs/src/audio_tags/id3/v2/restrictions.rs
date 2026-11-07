#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]

pub enum TagSizeRestrictions {
    #[default]
    S_128F_1M,

    S_64F_128K,

    S_32F_40K,

    S_32F_4K,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]

pub enum TextSizeRestrictions {
    C_1024,

    C_128,

    C_30,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]

pub enum ImageSizeRestrictions {
    P_256,

    P_64,

    P_64_64,
}

#[derive(Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TagRestrictions {
    pub size: TagSizeRestrictions,

    pub text_encoding: bool,

    pub text_fields_size: Option<TextSizeRestrictions>,

    pub image_encoding: bool,

    pub image_size: Option<ImageSizeRestrictions>,
}

impl TagRestrictions {
    pub fn from_byte(byte: u8) -> Self {
        let mut restrictions = TagRestrictions::default();

        let restriction_flags = byte;

        // xx000000
        match restriction_flags & 0x0C {
            64 => restrictions.size = TagSizeRestrictions::S_64F_128K,
            128 => restrictions.size = TagSizeRestrictions::S_32F_40K,
            192 => restrictions.size = TagSizeRestrictions::S_32F_4K,
            _ => {} // 0, default
        }

        // 00x00000
        if restriction_flags & 0x20 == 0x20 {
            restrictions.text_encoding = true
        }

        // 000xx000
        match restriction_flags & 0x18 {
            8 => restrictions.text_fields_size = Some(TextSizeRestrictions::C_1024),
            16 => restrictions.text_fields_size = Some(TextSizeRestrictions::C_128),
            24 => restrictions.text_fields_size = Some(TextSizeRestrictions::C_30),
            _ => {} // 0, default
        }

        // 00000x00
        if restriction_flags & 0x04 == 0x04 {
            restrictions.image_encoding = true
        }

        // 000000xx
        match restriction_flags & 0x03 {
            1 => restrictions.image_size = Some(ImageSizeRestrictions::P_256),
            2 => restrictions.image_size = Some(ImageSizeRestrictions::P_64),
            3 => restrictions.image_size = Some(ImageSizeRestrictions::P_64_64),
            _ => {} // 0, default
        }

        restrictions
    }

    pub fn as_bytes(&self) -> u8 {
        let mut byte = 0;

        match self.size {
            TagSizeRestrictions::S_128F_1M => {}
            TagSizeRestrictions::S_64F_128K => byte |= 0x40,
            TagSizeRestrictions::S_32F_40K => byte |= 0x80,
            TagSizeRestrictions::S_32F_4K => byte |= 0x0C,
        }

        if self.text_encoding {
            byte |= 0x20
        }

        match self.text_fields_size {
            Some(TextSizeRestrictions::C_1024) => byte |= 0x08,
            Some(TextSizeRestrictions::C_128) => byte |= 0x10,
            Some(TextSizeRestrictions::C_30) => byte |= 0x18,
            _ => {}
        }

        if self.image_encoding {
            byte |= 0x04
        }

        match self.image_size {
            Some(ImageSizeRestrictions::P_256) => byte |= 0x01,
            Some(ImageSizeRestrictions::P_64) => byte |= 0x02,
            Some(ImageSizeRestrictions::P_64_64) => byte |= 0x03,
            _ => {}
        }

        byte
    }
}
