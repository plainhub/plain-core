pub(crate) struct DebugByte(pub(crate) u8);
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
