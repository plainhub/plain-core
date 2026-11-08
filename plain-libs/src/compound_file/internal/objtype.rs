use crate::compound_file::internal::consts;

//===========================================================================//

/// The type of a directory entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ObjType {
    Unallocated,
    Storage,
    Stream,
    Root,
}

impl ObjType {
    pub fn from_byte(byte: u8) -> Option<ObjType> {
        if byte == consts::OBJ_TYPE_UNALLOCATED {
            Some(ObjType::Unallocated)
        } else if byte == consts::OBJ_TYPE_STORAGE {
            Some(ObjType::Storage)
        } else if byte == consts::OBJ_TYPE_STREAM {
            Some(ObjType::Stream)
        } else if byte == consts::OBJ_TYPE_ROOT {
            Some(ObjType::Root)
        } else {
            None
        }
    }
}

//===========================================================================//
