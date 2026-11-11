use crate::regex_engine::automata::util::utf8;
#[derive(Clone, Copy)]
pub struct DebugByte(pub u8);
impl core::fmt::Debug for DebugByte {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        if self.0 == b' ' {
            return write!(f, "' '");
        }
        let mut bytes = [0u8; 10];
        let mut len = 0;
        for (i, mut b) in core::ascii::escape_default(self.0).enumerate() {
            if i >= 2 && b'a' <= b && b <= b'f' {
                b -= 32;
            }
            bytes[len] = b;
            len += 1;
        }
        write!(f, "{}", core::str::from_utf8(&bytes[..len]).unwrap())
    }
}
pub struct DebugHaystack<'a>(pub &'a [u8]);
impl<'a> core::fmt::Debug for DebugHaystack<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "\"")?;
        let mut bytes = self.0;
        while let Some(result) = utf8::decode(bytes) {
            let ch = match result {
                Ok(ch) => ch,
                Err(byte) => {
                    write!(f, r"\x{byte:02x}")?;
                    bytes = &bytes[1..];
                    continue;
                }
            };
            bytes = &bytes[ch.len_utf8()..];
            match ch {
                '\0' => write!(f, "\\0")?,
                '\x01'..='\x08' | '\x0b' | '\x0c' | '\x0e'..='\x19' | '\x7f' => {
                    write!(f, "\\x{:02x}", u32::from(ch))?;
                }
                '\n' | '\r' | '\t' | _ => {
                    write!(f, "{}", ch.escape_debug())?;
                }
            }
        }
        write!(f, "\"")?;
        Ok(())
    }
}
