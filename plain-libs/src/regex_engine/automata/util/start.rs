use crate::regex_engine::automata::util::{
    look::LookMatcher,
    search::{Anchored, Input},
    wire::{self, DeserializeError, SerializeError},
};
#[derive(Clone, Debug)]
pub struct Config {
    look_behind: Option<u8>,
    anchored: Anchored,
}
impl Config {
    pub fn new() -> Config {
        Config {
            anchored: Anchored::No,
            look_behind: None,
        }
    }
    pub fn from_input_forward(input: &Input<'_>) -> Config {
        let look_behind = input
            .start()
            .checked_sub(1)
            .and_then(|i| input.haystack().get(i).copied());
        Config {
            look_behind,
            anchored: input.get_anchored(),
        }
    }
    pub fn from_input_reverse(input: &Input<'_>) -> Config {
        let look_behind = input.haystack().get(input.end()).copied();
        Config {
            look_behind,
            anchored: input.get_anchored(),
        }
    }
    pub fn look_behind(mut self, byte: Option<u8>) -> Config {
        self.look_behind = byte;
        self
    }
    pub fn anchored(mut self, mode: Anchored) -> Config {
        self.anchored = mode;
        self
    }
    pub fn get_look_behind(&self) -> Option<u8> {
        self.look_behind
    }
    pub fn get_anchored(&self) -> Anchored {
        self.anchored
    }
}
#[derive(Clone)]
pub(crate) struct StartByteMap {
    map: [Start; 256],
}
impl StartByteMap {
    pub(crate) fn new(lookm: &LookMatcher) -> StartByteMap {
        let mut map = [Start::NonWordByte; 256];
        map[usize::from(b'\n')] = Start::LineLF;
        map[usize::from(b'\r')] = Start::LineCR;
        map[usize::from(b'_')] = Start::WordByte;
        let mut byte = b'0';
        while byte <= b'9' {
            map[usize::from(byte)] = Start::WordByte;
            byte += 1;
        }
        byte = b'A';
        while byte <= b'Z' {
            map[usize::from(byte)] = Start::WordByte;
            byte += 1;
        }
        byte = b'a';
        while byte <= b'z' {
            map[usize::from(byte)] = Start::WordByte;
            byte += 1;
        }
        let lineterm = lookm.get_line_terminator();
        if lineterm != b'\r' && lineterm != b'\n' {
            map[usize::from(lineterm)] = Start::CustomLineTerminator;
        }
        StartByteMap { map }
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn get(&self, byte: u8) -> Start {
        self.map[usize::from(byte)]
    }
    pub(crate) fn from_bytes(slice: &[u8]) -> Result<(StartByteMap, usize), DeserializeError> {
        wire::check_slice_len(slice, 256, "start byte map")?;
        let mut map = [Start::NonWordByte; 256];
        for (i, &repr) in slice[..256].iter().enumerate() {
            map[i] = match Start::from_usize(usize::from(repr)) {
                Some(start) => start,
                None => {
                    return Err(DeserializeError::generic(
                        "found invalid starting configuration",
                    ));
                }
            };
        }
        Ok((StartByteMap { map }, 256))
    }
    pub(crate) fn write_to(&self, dst: &mut [u8]) -> Result<usize, SerializeError> {
        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small("start byte map"));
        }
        for (i, &start) in self.map.iter().enumerate() {
            dst[i] = start.as_u8();
        }
        Ok(nwrite)
    }
    pub(crate) fn write_to_len(&self) -> usize {
        256
    }
}
impl core::fmt::Debug for StartByteMap {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use crate::regex_engine::automata::util::escape::DebugByte;
        write!(f, "StartByteMap{{")?;
        for byte in 0..=255 {
            if byte > 0 {
                write!(f, ", ")?;
            }
            let start = self.map[usize::from(byte)];
            write!(f, "{:?} => {:?}", DebugByte(byte), start)?;
        }
        write!(f, "}}")?;
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Start {
    NonWordByte = 0,
    WordByte = 1,
    Text = 2,
    LineLF = 3,
    LineCR = 4,
    CustomLineTerminator = 5,
}
impl Start {
    pub(crate) fn from_usize(n: usize) -> Option<Start> {
        match n {
            0 => Some(Start::NonWordByte),
            1 => Some(Start::WordByte),
            2 => Some(Start::Text),
            3 => Some(Start::LineLF),
            4 => Some(Start::LineCR),
            5 => Some(Start::CustomLineTerminator),
            _ => None,
        }
    }
    pub(crate) fn len() -> usize {
        6
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn as_u8(&self) -> u8 {
        *self as u8
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn as_usize(&self) -> usize {
        usize::from(self.as_u8())
    }
}
