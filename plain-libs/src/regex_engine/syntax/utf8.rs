use core::{char, fmt, iter::FusedIterator, slice};
use std::{vec, vec::Vec};
const MAX_UTF8_BYTES: usize = 4;
#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord)]
pub enum Utf8Sequence {
    One(Utf8Range),
    Two([Utf8Range; 2]),
    Three([Utf8Range; 3]),
    Four([Utf8Range; 4]),
}
impl Utf8Sequence {
    fn from_encoded_range(start: &[u8], end: &[u8]) -> Self {
        assert_eq!(start.len(), end.len());
        match start.len() {
            2 => Utf8Sequence::Two([
                Utf8Range::new(start[0], end[0]),
                Utf8Range::new(start[1], end[1]),
            ]),
            3 => Utf8Sequence::Three([
                Utf8Range::new(start[0], end[0]),
                Utf8Range::new(start[1], end[1]),
                Utf8Range::new(start[2], end[2]),
            ]),
            4 => Utf8Sequence::Four([
                Utf8Range::new(start[0], end[0]),
                Utf8Range::new(start[1], end[1]),
                Utf8Range::new(start[2], end[2]),
                Utf8Range::new(start[3], end[3]),
            ]),
            n => unreachable!("invalid encoded length: {n}"),
        }
    }
    pub fn as_slice(&self) -> &[Utf8Range] {
        use self::Utf8Sequence::*;
        match *self {
            One(ref r) => slice::from_ref(r),
            Two(ref r) => &r[..],
            Three(ref r) => &r[..],
            Four(ref r) => &r[..],
        }
    }
    pub fn len(&self) -> usize {
        self.as_slice().len()
    }
    pub fn reverse(&mut self) {
        match *self {
            Utf8Sequence::One(_) => {}
            Utf8Sequence::Two(ref mut x) => x.reverse(),
            Utf8Sequence::Three(ref mut x) => x.reverse(),
            Utf8Sequence::Four(ref mut x) => x.reverse(),
        }
    }
    pub fn matches(&self, bytes: &[u8]) -> bool {
        if bytes.len() < self.len() {
            return false;
        }
        for (&b, r) in bytes.iter().zip(self) {
            if !r.matches(b) {
                return false;
            }
        }
        true
    }
}
impl<'a> IntoIterator for &'a Utf8Sequence {
    type IntoIter = slice::Iter<'a, Utf8Range>;
    type Item = &'a Utf8Range;
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}
impl fmt::Debug for Utf8Sequence {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        use self::Utf8Sequence::*;
        match *self {
            One(ref r) => write!(f, "{r:?}"),
            Two(ref r) => write!(f, "{:?}{:?}", r[0], r[1]),
            Three(ref r) => write!(f, "{:?}{:?}{:?}", r[0], r[1], r[2]),
            Four(ref r) => {
                write!(f, "{:?}{:?}{:?}{:?}", r[0], r[1], r[2], r[3])
            }
        }
    }
}
#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub struct Utf8Range {
    pub start: u8,
    pub end: u8,
}
impl Utf8Range {
    fn new(start: u8, end: u8) -> Self {
        Utf8Range { start, end }
    }
    pub fn matches(&self, b: u8) -> bool {
        self.start <= b && b <= self.end
    }
}
impl fmt::Debug for Utf8Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.start == self.end {
            write!(f, "[{:X}]", self.start)
        } else {
            write!(f, "[{:X}-{:X}]", self.start, self.end)
        }
    }
}
#[derive(Debug)]
pub struct Utf8Sequences {
    range_stack: Vec<ScalarRange>,
}
impl Utf8Sequences {
    pub fn new(start: char, end: char) -> Self {
        let range = ScalarRange {
            start: u32::from(start),
            end: u32::from(end),
        };
        Utf8Sequences {
            range_stack: vec![range],
        }
    }
    pub fn reset(&mut self, start: char, end: char) {
        self.range_stack.clear();
        self.push(u32::from(start), u32::from(end));
    }
    fn push(&mut self, start: u32, end: u32) {
        self.range_stack.push(ScalarRange { start, end });
    }
}
struct ScalarRange {
    start: u32,
    end: u32,
}
impl fmt::Debug for ScalarRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "ScalarRange({:X}, {:X})", self.start, self.end)
    }
}
impl Iterator for Utf8Sequences {
    type Item = Utf8Sequence;
    fn next(&mut self) -> Option<Self::Item> {
        'TOP: while let Some(mut r) = self.range_stack.pop() {
            'INNER: loop {
                if let Some((r1, r2)) = r.split() {
                    self.push(r2.start, r2.end);
                    r.start = r1.start;
                    r.end = r1.end;
                    continue 'INNER;
                }
                if !r.is_valid() {
                    continue 'TOP;
                }
                for i in 1..MAX_UTF8_BYTES {
                    let max = max_scalar_value(i);
                    if r.start <= max && max < r.end {
                        self.push(max + 1, r.end);
                        r.end = max;
                        continue 'INNER;
                    }
                }
                if let Some(ascii_range) = r.as_ascii() {
                    return Some(Utf8Sequence::One(ascii_range));
                }
                for i in 1..MAX_UTF8_BYTES {
                    let m = (1 << (6 * i)) - 1;
                    if (r.start & !m) != (r.end & !m) {
                        if (r.start & m) != 0 {
                            self.push((r.start | m) + 1, r.end);
                            r.end = r.start | m;
                            continue 'INNER;
                        }
                        if (r.end & m) != m {
                            self.push(r.end & !m, r.end);
                            r.end = (r.end & !m) - 1;
                            continue 'INNER;
                        }
                    }
                }
                let mut start = [0; MAX_UTF8_BYTES];
                let mut end = [0; MAX_UTF8_BYTES];
                let n = r.encode(&mut start, &mut end);
                return Some(Utf8Sequence::from_encoded_range(&start[0..n], &end[0..n]));
            }
        }
        None
    }
}
impl FusedIterator for Utf8Sequences {}
impl ScalarRange {
    fn split(&self) -> Option<(ScalarRange, ScalarRange)> {
        if self.start < 0xE000 && self.end > 0xD7FF {
            Some((
                ScalarRange {
                    start: self.start,
                    end: 0xD7FF,
                },
                ScalarRange {
                    start: 0xE000,
                    end: self.end,
                },
            ))
        } else {
            None
        }
    }
    fn is_valid(&self) -> bool {
        self.start <= self.end
    }
    fn as_ascii(&self) -> Option<Utf8Range> {
        if self.is_ascii() {
            let start = u8::try_from(self.start).unwrap();
            let end = u8::try_from(self.end).unwrap();
            Some(Utf8Range::new(start, end))
        } else {
            None
        }
    }
    fn is_ascii(&self) -> bool {
        self.is_valid() && self.end <= 0x7f
    }
    fn encode(&self, start: &mut [u8], end: &mut [u8]) -> usize {
        let cs = char::from_u32(self.start).unwrap();
        let ce = char::from_u32(self.end).unwrap();
        let ss = cs.encode_utf8(start);
        let se = ce.encode_utf8(end);
        assert_eq!(ss.len(), se.len());
        ss.len()
    }
}
fn max_scalar_value(nbytes: usize) -> u32 {
    match nbytes {
        1 => 0x007F,
        2 => 0x07FF,
        3 => 0xFFFF,
        4 => 0x0010_FFFF,
        _ => unreachable!("invalid UTF-8 byte sequence size"),
    }
}
