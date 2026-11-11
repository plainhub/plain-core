use crate::regex_engine::aho::util::int::Usize;
#[derive(Clone, Copy)]
pub(crate) struct ByteClasses([u8; 256]);
impl ByteClasses {
    pub(crate) fn empty() -> ByteClasses {
        ByteClasses([0; 256])
    }
    pub(crate) fn singletons() -> ByteClasses {
        let mut classes = ByteClasses::empty();
        for b in 0..=255 {
            classes.set(b, b);
        }
        classes
    }
    #[inline]
    pub(crate) fn set(&mut self, byte: u8, class: u8) {
        self.0[usize::from(byte)] = class;
    }
    #[inline]
    pub(crate) fn get(&self, byte: u8) -> u8 {
        self.0[usize::from(byte)]
    }
    #[inline]
    pub(crate) fn alphabet_len(&self) -> usize {
        usize::from(self.0[255]) + 1
    }
    pub(crate) fn stride2(&self) -> usize {
        let zeros = self.alphabet_len().next_power_of_two().trailing_zeros();
        usize::try_from(zeros).unwrap()
    }
    pub(crate) fn stride(&self) -> usize {
        1 << self.stride2()
    }
    #[inline]
    pub(crate) fn is_singleton(&self) -> bool {
        self.alphabet_len() == 256
    }
    pub(crate) fn iter(&self) -> ByteClassIter {
        ByteClassIter {
            it: 0..self.alphabet_len(),
        }
    }
    pub(crate) fn elements(&self, class: u8) -> ByteClassElements {
        ByteClassElements {
            classes: self,
            class,
            bytes: 0..=255,
        }
    }
    fn element_ranges(&self, class: u8) -> ByteClassElementRanges {
        ByteClassElementRanges {
            elements: self.elements(class),
            range: None,
        }
    }
}
impl core::fmt::Debug for ByteClasses {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_singleton() {
            write!(f, "ByteClasses(<one-class-per-byte>)")
        } else {
            write!(f, "ByteClasses(")?;
            for (i, class) in self.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{:?} => [", class)?;
                for (start, end) in self.element_ranges(class) {
                    if start == end {
                        write!(f, "{:?}", start)?;
                    } else {
                        write!(f, "{:?}-{:?}", start, end)?;
                    }
                }
                write!(f, "]")?;
            }
            write!(f, ")")
        }
    }
}
#[derive(Debug)]
pub(crate) struct ByteClassIter {
    it: core::ops::Range<usize>,
}
impl Iterator for ByteClassIter {
    type Item = u8;
    fn next(&mut self) -> Option<u8> {
        self.it.next().map(|class| class.as_u8())
    }
}
#[derive(Debug)]
pub(crate) struct ByteClassElements<'a> {
    classes: &'a ByteClasses,
    class: u8,
    bytes: core::ops::RangeInclusive<u8>,
}
impl<'a> Iterator for ByteClassElements<'a> {
    type Item = u8;
    fn next(&mut self) -> Option<u8> {
        while let Some(byte) = self.bytes.next() {
            if self.class == self.classes.get(byte) {
                return Some(byte);
            }
        }
        None
    }
}
#[derive(Debug)]
pub(crate) struct ByteClassElementRanges<'a> {
    elements: ByteClassElements<'a>,
    range: Option<(u8, u8)>,
}
impl<'a> Iterator for ByteClassElementRanges<'a> {
    type Item = (u8, u8);
    fn next(&mut self) -> Option<(u8, u8)> {
        loop {
            let element = match self.elements.next() {
                None => return self.range.take(),
                Some(element) => element,
            };
            match self.range.take() {
                None => {
                    self.range = Some((element, element));
                }
                Some((start, end)) => {
                    if usize::from(end) + 1 != usize::from(element) {
                        self.range = Some((element, element));
                        return Some((start, end));
                    }
                    self.range = Some((start, element));
                }
            }
        }
    }
}
#[derive(Clone, Debug)]
pub(crate) struct ByteClassSet(ByteSet);
impl Default for ByteClassSet {
    fn default() -> ByteClassSet {
        ByteClassSet::empty()
    }
}
impl ByteClassSet {
    pub(crate) fn empty() -> Self {
        ByteClassSet(ByteSet::empty())
    }
    pub(crate) fn set_range(&mut self, start: u8, end: u8) {
        debug_assert!(start <= end);
        if start > 0 {
            self.0.add(start - 1);
        }
        self.0.add(end);
    }
    pub(crate) fn byte_classes(&self) -> ByteClasses {
        let mut classes = ByteClasses::empty();
        let mut class = 0u8;
        let mut b = 0u8;
        loop {
            classes.set(b, class);
            if b == 255 {
                break;
            }
            if self.0.contains(b) {
                class = class.checked_add(1).unwrap();
            }
            b = b.checked_add(1).unwrap();
        }
        classes
    }
}
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct ByteSet {
    bits: BitSet,
}
#[derive(Clone, Copy, Default, Eq, PartialEq)]
struct BitSet([u128; 2]);
impl ByteSet {
    pub(crate) fn empty() -> ByteSet {
        ByteSet {
            bits: BitSet([0; 2]),
        }
    }
    pub(crate) fn add(&mut self, byte: u8) {
        let bucket = byte / 128;
        let bit = byte % 128;
        self.bits.0[usize::from(bucket)] |= 1 << bit;
    }
    pub(crate) fn contains(&self, byte: u8) -> bool {
        let bucket = byte / 128;
        let bit = byte % 128;
        self.bits.0[usize::from(bucket)] & (1 << bit) > 0
    }
}
impl core::fmt::Debug for BitSet {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let mut fmtd = f.debug_set();
        for b in 0u8..=255 {
            if (ByteSet { bits: *self }).contains(b) {
                fmtd.entry(&b);
            }
        }
        fmtd.finish()
    }
}
