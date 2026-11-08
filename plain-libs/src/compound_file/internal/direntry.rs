use crate::compound_file::ReadLeNumber;
use crate::compound_file::internal::consts::{self, MAX_REGULAR_STREAM_ID, NO_STREAM};
use crate::compound_file::internal::{self, Color, ObjType, Version};
use std::io::{self, Read};

//===========================================================================//

macro_rules! malformed {
    ($e:expr) => { invalid_data!("Malformed directory entry ({})", $e) };
    ($fmt:expr, $($arg:tt)+) => {
        invalid_data!("Malformed directory entry ({})",
                      format!($fmt, $($arg)+))
    };
}

//===========================================================================//

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirEntry {
    pub name: String,
    pub obj_type: ObjType,
    pub left_sibling: u32,
    pub right_sibling: u32,
    pub child: u32,
    pub start_sector: u32,
    pub stream_len: u64,
}

impl DirEntry {
    pub fn read_from<R: Read>(reader: &mut R, version: Version) -> io::Result<DirEntry> {
        // Read the whole entry in one go, then parse it from memory.
        let mut buffer = [0u8; consts::DIR_ENTRY_LEN];
        reader.read_exact(&mut buffer)?;
        let reader = &mut &buffer[..];
        let mut name: String = {
            let mut name_chars = [0; 32];
            for unit in &mut name_chars {
                *unit = reader.read_le_u16()?;
            }
            let name_len_bytes = reader.read_le_u16()?;
            if name_len_bytes > 64 {
                malformed!("name length too large: {}", name_len_bytes);
            } else if name_len_bytes % 2 != 0 {
                malformed!("odd name length: {}", name_len_bytes);
            }
            let name_len_chars = if name_len_bytes > 0 {
                (name_len_bytes / 2 - 1) as usize
            } else {
                0
            };
            debug_assert!(name_len_chars < name_chars.len());
            // According to section 2.6.1 of the MS-CFB spec, "The name MUST be
            // terminated with a UTF-16 terminating null character."  (Even
            // though the directory entry aready includes the length of the
            // name.  And also, that length *includes* the null character?
            // Look, CFB is a weird format.)  Anyway, some CFB files in the
            // wild don't do this, so under Permissive validation we don't
            // enforce it.
            match String::from_utf16(&name_chars[0..name_len_chars]) {
                Ok(name) => name,
                Err(_) => malformed!("name not valid UTF-16"),
            }
        };

        let obj_type = {
            let mut buf = [0u8];
            reader.read_exact(&mut buf)?;
            let [obj_type_byte] = buf;
            match ObjType::from_byte(obj_type_byte) {
                Some(obj_type) => obj_type,
                None => malformed!("invalid object type: {}", obj_type_byte),
            }
        };

        // According to section 2.6.2 of the MS-CFB spec, "The root directory
        // entry's Name field MUST contain the null-terminated string 'Root
        // Entry' in Unicode UTF-16."  However, some CFB files in the wild
        // don't do this, so under Permissive validation we don't enforce it;
        // instead, for the root entry we just ignore the actual name in the
        // file and treat it as though it were what it's supposed to be.
        if obj_type == ObjType::Root {
            if name != consts::ROOT_DIR_NAME {
                name = consts::ROOT_DIR_NAME.to_string();
            }
        } else {
            internal::path::validate_name(&name)?;
        }

        {
            let mut buf = [0u8];
            reader.read_exact(&mut buf)?;
            let [color_byte] = buf;
            match Color::from_byte(color_byte) {
                Some(_) => (),
                None => malformed!("invalid color: {}", color_byte),
            }
        };
        let left_sibling = reader.read_le_u32()?;
        if left_sibling != NO_STREAM && left_sibling > MAX_REGULAR_STREAM_ID {
            malformed!("invalid left sibling: {}", left_sibling);
        }
        let right_sibling = reader.read_le_u32()?;
        if right_sibling != NO_STREAM && right_sibling > MAX_REGULAR_STREAM_ID {
            malformed!("invalid right sibling: {}", right_sibling);
        }
        let child = reader.read_le_u32()?;
        if child != NO_STREAM {
            if obj_type == ObjType::Stream {
                malformed!("non-empty stream child: {}", child);
            } else if child > MAX_REGULAR_STREAM_ID {
                malformed!("invalid child: {}", child);
            }
        }

        // Skip CLSID, state bits, and timestamps.
        reader.read_exact(&mut [0; 36])?;

        // According to the MS-CFB spec section 2.6.3, the starting sector and
        // stream length fields should both be set to zero for storage entries.
        // However, some CFB implementations use FREE_SECTOR or END_OF_CHAIN
        // instead for the starting sector, or even just leave these fields
        // with uninitialized garbage values, so under Permissive validation we
        // don't enforce this; instead, for storage objects we just treat these
        // fields as though they were zero.
        let mut start_sector = reader.read_le_u32()?;
        let mut stream_len = reader.read_le_u64()? & version.stream_len_mask();
        if obj_type == ObjType::Storage {
            start_sector = 0;
            stream_len = 0;
        }

        Ok(DirEntry {
            name,
            obj_type,
            left_sibling,
            right_sibling,
            child,
            start_sector,
            stream_len,
        })
    }
}

//===========================================================================//

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/compound_file/direntry.rs"]
mod tests;
