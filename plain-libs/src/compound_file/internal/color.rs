use crate::compound_file::internal::consts;

//===========================================================================//

/// The "color" of a directory entry (which can be used for maintaining a
/// red-black tree).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Color {
    Red,
    Black,
}

impl Color {
    pub fn from_byte(byte: u8) -> Option<Color> {
        if byte == consts::COLOR_RED {
            Some(Color::Red)
        } else if byte == consts::COLOR_BLACK {
            Some(Color::Black)
        } else {
            None
        }
    }
}

//===========================================================================//
