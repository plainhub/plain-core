use crate::memchr::ext::Pointer;
use crate::regex_engine::automata::util::wire::{self, DeserializeError, Endian, SerializeError};
use std::{vec, vec::Vec};
type AccelTy = u32;
const ACCEL_TY_SIZE: usize = core::mem::size_of::<AccelTy>();
const ACCEL_LEN: usize = 4;
const ACCEL_CAP: usize = 8;
#[cfg_attr(all(), inline(always))]
pub(crate) fn find_fwd(needles: &[u8], haystack: &[u8], at: usize) -> Option<usize> {
    let bs = needles;
    let i = match needles.len() {
        1 => crate::memchr::memchr(bs[0], &haystack[at..])?,
        2 => crate::memchr::memchr2(bs[0], bs[1], &haystack[at..])?,
        3 => crate::memchr::memchr3(bs[0], bs[1], bs[2], &haystack[at..])?,
        0 => panic!("cannot find with empty needles"),
        n => panic!("invalid needles length: {n}"),
    };
    Some(at + i)
}
#[cfg_attr(all(), inline(always))]
pub(crate) fn find_rev(needles: &[u8], haystack: &[u8], at: usize) -> Option<usize> {
    let bs = needles;
    match needles.len() {
        1 => crate::memchr::memrchr(bs[0], &haystack[..at]),
        2 => crate::memchr::memrchr2(bs[0], bs[1], &haystack[..at]),
        3 => crate::memchr::memrchr3(bs[0], bs[1], bs[2], &haystack[..at]),
        0 => panic!("cannot find with empty needles"),
        n => panic!("invalid needles length: {n}"),
    }
}
#[derive(Clone)]
pub(crate) struct Accels<A> {
    accels: A,
}
impl Accels<Vec<AccelTy>> {
    pub fn empty() -> Accels<Vec<AccelTy>> {
        Accels { accels: vec![0] }
    }
    pub fn add(&mut self, accel: Accel) {
        self.accels.extend_from_slice(&accel.as_accel_tys());
        let len = self.len();
        self.set_len(len + 1);
    }
    fn set_len(&mut self, new_len: usize) {
        let new_len = AccelTy::try_from(new_len).unwrap();
        self.accels[0] = new_len;
    }
}
impl<'a> Accels<&'a [AccelTy]> {
    pub fn from_bytes_unchecked(
        mut slice: &'a [u8],
    ) -> Result<(Accels<&'a [AccelTy]>, usize), DeserializeError> {
        let slice_start = slice.as_ptr().as_usize();
        let (accel_len, _) = wire::try_read_u32_as_usize(slice, "accelerators length")?;
        let accel_tys_len = wire::add(
            wire::mul(accel_len, 2, "total number of accelerator accel_tys")?,
            1,
            "total number of accel_tys",
        )?;
        let accel_tys_bytes_len = wire::mul(
            ACCEL_TY_SIZE,
            accel_tys_len,
            "total number of bytes in accelerators",
        )?;
        wire::check_slice_len(slice, accel_tys_bytes_len, "accelerators")?;
        wire::check_alignment::<AccelTy>(slice)?;
        let accel_tys = &slice[..accel_tys_bytes_len];
        slice = &slice[accel_tys_bytes_len..];
        let accels = unsafe {
            core::slice::from_raw_parts(accel_tys.as_ptr().cast::<AccelTy>(), accel_tys_len)
        };
        Ok((Accels { accels }, slice.as_ptr().as_usize() - slice_start))
    }
}
impl<A: AsRef<[AccelTy]>> Accels<A> {
    pub fn to_owned(&self) -> Accels<std::vec::Vec<AccelTy>> {
        Accels {
            accels: self.accels.as_ref().to_vec(),
        }
    }
    pub fn as_ref(&self) -> Accels<&[AccelTy]> {
        Accels {
            accels: self.accels.as_ref(),
        }
    }
    pub fn as_bytes(&self) -> &[u8] {
        let accels = self.accels.as_ref();
        unsafe {
            core::slice::from_raw_parts(accels.as_ptr().cast::<u8>(), accels.len() * ACCEL_TY_SIZE)
        }
    }
    pub fn memory_usage(&self) -> usize {
        self.as_bytes().len()
    }
    #[cfg_attr(all(), inline(always))]
    pub fn needles(&self, i: usize) -> &[u8] {
        if i >= self.len() {
            panic!("invalid accelerator index {i}");
        }
        let bytes = self.as_bytes();
        let offset = ACCEL_TY_SIZE + i * ACCEL_CAP;
        let len = usize::from(bytes[offset]);
        &bytes[offset + 1..offset + 1 + len]
    }
    pub fn len(&self) -> usize {
        usize::try_from(self.accels.as_ref()[0]).unwrap()
    }
    fn get(&self, i: usize) -> Option<Accel> {
        if i >= self.len() {
            return None;
        }
        let offset = ACCEL_TY_SIZE + i * ACCEL_CAP;
        let accel = Accel::from_slice(&self.as_bytes()[offset..])
            .expect("Accels must contain valid accelerators");
        Some(accel)
    }
    fn iter(&self) -> IterAccels<'_, A> {
        IterAccels { accels: self, i: 0 }
    }
    pub fn write_to<E: Endian>(&self, dst: &mut [u8]) -> Result<usize, SerializeError> {
        let nwrite = self.write_to_len();
        assert_eq!(
            nwrite % ACCEL_TY_SIZE,
            0,
            "expected accelerator bytes written to be a multiple \
             of {ACCEL_TY_SIZE}",
        );
        if dst.len() < nwrite {
            return Err(SerializeError::buffer_too_small("accelerators"));
        }
        E::write_u32(AccelTy::try_from(self.len()).unwrap(), dst);
        dst[ACCEL_TY_SIZE..nwrite].copy_from_slice(&self.as_bytes()[ACCEL_TY_SIZE..nwrite]);
        Ok(nwrite)
    }
    pub fn validate(&self) -> Result<(), DeserializeError> {
        for chunk in self.as_bytes()[ACCEL_TY_SIZE..].chunks(ACCEL_CAP) {
            let _ = Accel::from_slice(chunk)?;
        }
        Ok(())
    }
    pub fn write_to_len(&self) -> usize {
        self.as_bytes().len()
    }
}
impl<A: AsRef<[AccelTy]>> core::fmt::Debug for Accels<A> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "Accels(")?;
        let mut list = f.debug_list();
        for a in self.iter() {
            list.entry(&a);
        }
        list.finish()?;
        write!(f, ")")
    }
}
#[derive(Debug)]
struct IterAccels<'a, A: AsRef<[AccelTy]>> {
    accels: &'a Accels<A>,
    i: usize,
}
impl<'a, A: AsRef<[AccelTy]>> Iterator for IterAccels<'a, A> {
    type Item = Accel;
    fn next(&mut self) -> Option<Accel> {
        let accel = self.accels.get(self.i)?;
        self.i += 1;
        Some(accel)
    }
}
#[derive(Clone)]
pub(crate) struct Accel {
    bytes: [u8; ACCEL_CAP],
}
impl Accel {
    pub fn new() -> Accel {
        Accel {
            bytes: [0; ACCEL_CAP],
        }
    }
    pub fn from_slice(mut slice: &[u8]) -> Result<Accel, DeserializeError> {
        slice = &slice[..core::cmp::min(ACCEL_LEN, slice.len())];
        let bytes = slice
            .try_into()
            .map_err(|_| DeserializeError::buffer_too_small("accelerator"))?;
        Accel::from_bytes(bytes)
    }
    fn from_bytes(bytes: [u8; 4]) -> Result<Accel, DeserializeError> {
        if usize::from(bytes[0]) >= ACCEL_LEN {
            return Err(DeserializeError::generic(
                "accelerator bytes cannot have length more than 3",
            ));
        }
        Ok(Accel::from_bytes_unchecked(bytes))
    }
    fn from_bytes_unchecked(bytes: [u8; 4]) -> Accel {
        Accel {
            bytes: [bytes[0], bytes[1], bytes[2], bytes[3], 0, 0, 0, 0],
        }
    }
    pub fn add(&mut self, byte: u8) -> bool {
        if self.len() >= 3 {
            return false;
        }
        if byte == b' ' {
            return false;
        }
        assert!(
            !self.contains(byte),
            "accelerator already contains {:?}",
            crate::regex_engine::automata::util::escape::DebugByte(byte)
        );
        self.bytes[self.len() + 1] = byte;
        self.bytes[0] += 1;
        true
    }
    pub fn len(&self) -> usize {
        usize::from(self.bytes[0])
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn needles(&self) -> &[u8] {
        &self.bytes[1..1 + self.len()]
    }
    fn contains(&self, byte: u8) -> bool {
        self.needles().iter().position(|&b| b == byte).is_some()
    }
    fn as_accel_tys(&self) -> [AccelTy; 2] {
        assert_eq!(ACCEL_CAP, 8);
        let first = AccelTy::from_ne_bytes(self.bytes[0..4].try_into().unwrap());
        let second = AccelTy::from_ne_bytes(self.bytes[4..8].try_into().unwrap());
        [first, second]
    }
}
impl core::fmt::Debug for Accel {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "Accel(")?;
        let mut set = f.debug_set();
        for &b in self.needles() {
            set.entry(&crate::regex_engine::automata::util::escape::DebugByte(b));
        }
        set.finish()?;
        write!(f, ")")
    }
}
