use crate::regex_engine::automata::util::{
    escape::DebugByte,
    wire::{self, DeserializeError, SerializeError},
};
#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
pub struct Unit(UnitKind);
#[derive(Clone, Copy, Eq, PartialEq, PartialOrd, Ord)]
enum UnitKind {
    U8(u8),
    EOI(u16),
}
impl Unit {
    pub fn u8(byte: u8) -> Unit {
        Unit(UnitKind::U8(byte))
    }
    pub fn eoi(num_byte_equiv_classes: usize) -> Unit {
        assert!(
            num_byte_equiv_classes <= 256,
            "max number of byte-based equivalent classes is 256, but got \
             {num_byte_equiv_classes}",
        );
        Unit(UnitKind::EOI(
            u16::try_from(num_byte_equiv_classes).unwrap(),
        ))
    }
    pub fn as_u8(self) -> Option<u8> {
        match self.0 {
            UnitKind::U8(b) => Some(b),
            UnitKind::EOI(_) => None,
        }
    }
    pub fn as_eoi(self) -> Option<u16> {
        match self.0 {
            UnitKind::U8(_) => None,
            UnitKind::EOI(sentinel) => Some(sentinel),
        }
    }
    pub fn as_usize(self) -> usize {
        match self.0 {
            UnitKind::U8(b) => usize::from(b),
            UnitKind::EOI(eoi) => usize::from(eoi),
        }
    }
    pub fn is_byte(self, byte: u8) -> bool {
        self.as_u8().map_or(false, |b| b == byte)
    }
    pub fn is_eoi(self) -> bool {
        self.as_eoi().is_some()
    }
    pub fn is_word_byte(self) -> bool {
        self.as_u8().map_or(
            false,
            crate::regex_engine::automata::util::utf8::is_word_byte,
        )
    }
}
impl core::fmt::Debug for Unit {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self.0 {
            UnitKind::U8(b) => write!(f, "{:?}", DebugByte(b)),
            UnitKind::EOI(_) => write!(f, "EOI"),
        }
    }
}
#[derive(Clone, Copy)]
pub struct ByteClasses([u8; 256]);
impl ByteClasses {
    #[inline]
    pub fn empty() -> ByteClasses {
        ByteClasses([0; 256])
    }
    #[inline]
    pub fn singletons() -> ByteClasses {
        let mut classes = ByteClasses::empty();
        for b in 0..=255 {
            classes.set(b, b);
        }
        classes
    }
    pub(crate) fn from_bytes(slice: &[u8]) -> Result<(ByteClasses, usize), DeserializeError> {
        wire::check_slice_len(slice, 256, "byte class map")?;
        let mut classes = ByteClasses::empty();
        for (b, &class) in slice[..256].iter().enumerate() {
            classes.set(u8::try_from(b).unwrap(), class);
        }
        for &b in classes.0.iter() {
            if usize::from(b) >= classes.alphabet_len() {
                return Err(DeserializeError::generic(
                    "found equivalence class greater than alphabet len",
                ));
            }
        }
        Ok((classes, 256))
    }
    pub(crate) fn write_to(&self, mut dst: &mut [u8]) -> Result<usize, SerializeError> {
        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small("byte class map"));
        }
        for b in 0..=255 {
            dst[0] = self.get(b);
            dst = &mut dst[1..];
        }
        Ok(nwrite)
    }
    pub(crate) fn write_to_len(&self) -> usize {
        256
    }
    #[inline]
    pub fn set(&mut self, byte: u8, class: u8) {
        self.0[usize::from(byte)] = class;
    }
    #[inline]
    pub fn get(&self, byte: u8) -> u8 {
        self.0[usize::from(byte)]
    }
    #[inline]
    pub fn get_by_unit(&self, unit: Unit) -> usize {
        match unit.0 {
            UnitKind::U8(b) => usize::from(self.get(b)),
            UnitKind::EOI(b) => usize::from(b),
        }
    }
    #[inline]
    pub fn eoi(&self) -> Unit {
        Unit::eoi(self.alphabet_len().checked_sub(1).unwrap())
    }
    #[inline]
    pub fn alphabet_len(&self) -> usize {
        usize::from(self.0[255]) + 1 + 1
    }
    #[inline]
    pub fn stride2(&self) -> usize {
        let zeros = self.alphabet_len().next_power_of_two().trailing_zeros();
        usize::try_from(zeros).unwrap()
    }
    #[inline]
    pub fn is_singleton(&self) -> bool {
        self.alphabet_len() == 257
    }
    #[inline]
    pub fn iter(&self) -> ByteClassIter<'_> {
        ByteClassIter {
            classes: self,
            i: 0,
        }
    }
    pub fn representatives<R: core::ops::RangeBounds<u8>>(
        &self,
        range: R,
    ) -> ByteClassRepresentatives<'_> {
        use core::ops::Bound;
        let cur_byte = match range.start_bound() {
            Bound::Included(&i) => usize::from(i),
            Bound::Excluded(&i) => usize::from(i).checked_add(1).unwrap(),
            Bound::Unbounded => 0,
        };
        let end_byte = match range.end_bound() {
            Bound::Included(&i) => Some(usize::from(i).checked_add(1).unwrap()),
            Bound::Excluded(&i) => Some(usize::from(i)),
            Bound::Unbounded => None,
        };
        assert_ne!(
            cur_byte,
            usize::MAX,
            "start range must be less than usize::MAX",
        );
        ByteClassRepresentatives {
            classes: self,
            cur_byte,
            end_byte,
            last_class: None,
        }
    }
    #[inline]
    pub fn elements(&self, class: Unit) -> ByteClassElements<'_> {
        ByteClassElements {
            classes: self,
            class,
            byte: 0,
        }
    }
    fn element_ranges(&self, class: Unit) -> ByteClassElementRanges<'_> {
        ByteClassElementRanges {
            elements: self.elements(class),
            range: None,
        }
    }
}
impl Default for ByteClasses {
    fn default() -> ByteClasses {
        ByteClasses::singletons()
    }
}
impl core::fmt::Debug for ByteClasses {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.is_singleton() {
            write!(f, "ByteClasses({{singletons}})")
        } else {
            write!(f, "ByteClasses(")?;
            for (i, class) in self.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{:?} => [", class.as_usize())?;
                for (start, end) in self.element_ranges(class) {
                    if start == end {
                        write!(f, "{start:?}")?;
                    } else {
                        write!(f, "{start:?}-{end:?}")?;
                    }
                }
                write!(f, "]")?;
            }
            write!(f, ")")
        }
    }
}
#[derive(Debug)]
pub struct ByteClassIter<'a> {
    classes: &'a ByteClasses,
    i: usize,
}
impl<'a> Iterator for ByteClassIter<'a> {
    type Item = Unit;
    fn next(&mut self) -> Option<Unit> {
        if self.i + 1 == self.classes.alphabet_len() {
            self.i += 1;
            Some(self.classes.eoi())
        } else if self.i < self.classes.alphabet_len() {
            let class = u8::try_from(self.i).unwrap();
            self.i += 1;
            Some(Unit::u8(class))
        } else {
            None
        }
    }
}
#[derive(Debug)]
pub struct ByteClassRepresentatives<'a> {
    classes: &'a ByteClasses,
    cur_byte: usize,
    end_byte: Option<usize>,
    last_class: Option<u8>,
}
impl<'a> Iterator for ByteClassRepresentatives<'a> {
    type Item = Unit;
    fn next(&mut self) -> Option<Unit> {
        while self.cur_byte < self.end_byte.unwrap_or(256) {
            let byte = u8::try_from(self.cur_byte).unwrap();
            let class = self.classes.get(byte);
            self.cur_byte += 1;
            if self.last_class != Some(class) {
                self.last_class = Some(class);
                return Some(Unit::u8(byte));
            }
        }
        if self.cur_byte != usize::MAX && self.end_byte.is_none() {
            self.cur_byte = usize::MAX;
            return Some(self.classes.eoi());
        }
        None
    }
}
#[derive(Debug)]
pub struct ByteClassElements<'a> {
    classes: &'a ByteClasses,
    class: Unit,
    byte: usize,
}
impl<'a> Iterator for ByteClassElements<'a> {
    type Item = Unit;
    fn next(&mut self) -> Option<Unit> {
        while self.byte < 256 {
            let byte = u8::try_from(self.byte).unwrap();
            self.byte += 1;
            if self.class.is_byte(self.classes.get(byte)) {
                return Some(Unit::u8(byte));
            }
        }
        if self.byte < 257 {
            self.byte += 1;
            if self.class.is_eoi() {
                return Some(Unit::eoi(256));
            }
        }
        None
    }
}
#[derive(Debug)]
struct ByteClassElementRanges<'a> {
    elements: ByteClassElements<'a>,
    range: Option<(Unit, Unit)>,
}
impl<'a> Iterator for ByteClassElementRanges<'a> {
    type Item = (Unit, Unit);
    fn next(&mut self) -> Option<(Unit, Unit)> {
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
                    if end.as_usize() + 1 != element.as_usize() || element.is_eoi() {
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
    pub(crate) fn add_set(&mut self, set: &ByteSet) {
        for (start, end) in set.iter_ranges() {
            self.set_range(start, end);
        }
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
    pub(crate) fn remove(&mut self, byte: u8) {
        let bucket = byte / 128;
        let bit = byte % 128;
        self.bits.0[usize::from(bucket)] &= !(1 << bit);
    }
    pub(crate) fn contains(&self, byte: u8) -> bool {
        let bucket = byte / 128;
        let bit = byte % 128;
        self.bits.0[usize::from(bucket)] & (1 << bit) > 0
    }
    pub(crate) fn contains_range(&self, start: u8, end: u8) -> bool {
        (start..=end).all(|b| self.contains(b))
    }
    pub(crate) fn iter(&self) -> ByteSetIter<'_> {
        ByteSetIter { set: self, b: 0 }
    }
    pub(crate) fn iter_ranges(&self) -> ByteSetRangeIter<'_> {
        ByteSetRangeIter { set: self, b: 0 }
    }
    #[cfg_attr(all(), inline(always))]
    pub(crate) fn is_empty(&self) -> bool {
        self.bits.0 == [0, 0]
    }
    pub(crate) fn from_bytes(slice: &[u8]) -> Result<(ByteSet, usize), DeserializeError> {
        use core::mem::size_of;
        wire::check_slice_len(slice, 2 * size_of::<u128>(), "byte set")?;
        let mut nread = 0;
        let (low, nr) = wire::try_read_u128(slice, "byte set low bucket")?;
        nread += nr;
        let (high, nr) = wire::try_read_u128(slice, "byte set high bucket")?;
        nread += nr;
        Ok((
            ByteSet {
                bits: BitSet([low, high]),
            },
            nread,
        ))
    }
    pub(crate) fn write_to<E: crate::regex_engine::automata::util::wire::Endian>(
        &self,
        dst: &mut [u8],
    ) -> Result<usize, SerializeError> {
        use core::mem::size_of;
        let nwrite = self.write_to_len();
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small("byte set"));
        }
        let mut nw = 0;
        E::write_u128(self.bits.0[0], &mut dst[nw..]);
        nw += size_of::<u128>();
        E::write_u128(self.bits.0[1], &mut dst[nw..]);
        nw += size_of::<u128>();
        assert_eq!(nwrite, nw, "expected to write certain number of bytes",);
        assert_eq!(
            nw % 8,
            0,
            "expected to write multiple of 8 bytes for byte set",
        );
        Ok(nw)
    }
    pub(crate) fn write_to_len(&self) -> usize {
        2 * core::mem::size_of::<u128>()
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
#[derive(Debug)]
pub(crate) struct ByteSetIter<'a> {
    set: &'a ByteSet,
    b: usize,
}
impl<'a> Iterator for ByteSetIter<'a> {
    type Item = u8;
    fn next(&mut self) -> Option<u8> {
        while self.b <= 255 {
            let b = u8::try_from(self.b).unwrap();
            self.b += 1;
            if self.set.contains(b) {
                return Some(b);
            }
        }
        None
    }
}
#[derive(Debug)]
pub(crate) struct ByteSetRangeIter<'a> {
    set: &'a ByteSet,
    b: usize,
}
impl<'a> Iterator for ByteSetRangeIter<'a> {
    type Item = (u8, u8);
    fn next(&mut self) -> Option<(u8, u8)> {
        let asu8 = |n: usize| u8::try_from(n).unwrap();
        while self.b <= 255 {
            let start = asu8(self.b);
            self.b += 1;
            if !self.set.contains(start) {
                continue;
            }
            let mut end = start;
            while self.b <= 255 && self.set.contains(asu8(self.b)) {
                end = asu8(self.b);
                self.b += 1;
            }
            return Some((start, end));
        }
        None
    }
}
