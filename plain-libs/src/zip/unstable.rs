use std::{
    borrow::Cow,
    io::{self, Read},
    path::{Component, MAIN_SEPARATOR, Path},
};
pub trait LittleEndianReadExt: Read {
    fn read_u16_le(&mut self) -> io::Result<u16> {
        let mut out = [0u8; 2];
        self.read_exact(&mut out)?;
        Ok(u16::from_le_bytes(out))
    }
    fn read_u32_le(&mut self) -> io::Result<u32> {
        let mut out = [0u8; 4];
        self.read_exact(&mut out)?;
        Ok(u32::from_le_bytes(out))
    }
    fn read_u64_le(&mut self) -> io::Result<u64> {
        let mut out = [0u8; 8];
        self.read_exact(&mut out)?;
        Ok(u64::from_le_bytes(out))
    }
}
impl<R: Read> LittleEndianReadExt for R {}
pub fn path_to_string<T: AsRef<Path>>(path: T) -> Result<Box<str>, std::io::Error> {
    let mut maybe_original = None;
    if let Some(original) = path.as_ref().to_str() {
        if original.is_empty() || original == "." || original == ".." {
            return Ok(String::new().into_boxed_str());
        }
        if original.starts_with(MAIN_SEPARATOR) {
            if original.len() == 1 {
                return Ok(MAIN_SEPARATOR.to_string().into_boxed_str());
            } else if (MAIN_SEPARATOR == '/' || !original[1..].contains(MAIN_SEPARATOR))
                && !original.ends_with('.')
                && !original.contains([MAIN_SEPARATOR, MAIN_SEPARATOR])
                && !original.contains([MAIN_SEPARATOR, '.', MAIN_SEPARATOR])
                && !original.contains([MAIN_SEPARATOR, '.', '.', MAIN_SEPARATOR])
            {
                maybe_original = Some(&original[1..]);
            }
        } else if !original.contains(MAIN_SEPARATOR) {
            return Ok(original.into());
        }
    }
    let mut recreate = maybe_original.is_none();
    let mut normalized_components = Vec::new();
    for component in path.as_ref().components() {
        match component {
            Component::Normal(os_str) => {
                if let Some(valid_str) = os_str.to_str() {
                    normalized_components.push(Cow::Borrowed(valid_str));
                } else {
                    recreate = true;
                    normalized_components.push(os_str.to_string_lossy());
                }
            }
            Component::ParentDir => {
                recreate = true;
                normalized_components.pop();
            }
            _ => {
                recreate = true;
            }
        }
    }
    if recreate {
        Ok(normalized_components.join("/").into())
    } else {
        Ok(maybe_original
            .ok_or_else(|| std::io::Error::other("Original path is empty"))?
            .into())
    }
}
