use crate::regex_engine::automata::util::{
    int::Pointer,
    primitives::{PatternID, PatternIDError, StateID, StateIDError},
};
use core::{cmp, mem::size_of};
use std::{vec, vec::Vec};
#[repr(C)]
#[derive(Debug)]
pub struct AlignAs<B: ?Sized, T> {
    pub _align: [T; 0],
    pub bytes: B,
}
#[derive(Debug)]
pub struct SerializeError {
    what: &'static str,
}
impl SerializeError {
    pub(crate) fn buffer_too_small(what: &'static str) -> SerializeError {
        SerializeError { what }
    }
}
impl core::fmt::Display for SerializeError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "destination buffer is too small to write {}", self.what)
    }
}
impl std::error::Error for SerializeError {}
#[derive(Debug)]
pub struct DeserializeError(DeserializeErrorKind);
#[derive(Debug)]
enum DeserializeErrorKind {
    Generic {
        msg: &'static str,
    },
    BufferTooSmall {
        what: &'static str,
    },
    InvalidUsize {
        what: &'static str,
    },
    VersionMismatch {
        expected: u32,
        found: u32,
    },
    EndianMismatch {
        expected: u32,
        found: u32,
    },
    AlignmentMismatch {
        alignment: usize,
        address: usize,
    },
    LabelMismatch {
        expected: &'static str,
    },
    ArithmeticOverflow {
        what: &'static str,
    },
    PatternID {
        err: PatternIDError,
        what: &'static str,
    },
    StateID {
        err: StateIDError,
        what: &'static str,
    },
}
impl DeserializeError {
    pub(crate) fn generic(msg: &'static str) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::Generic { msg })
    }
    pub(crate) fn buffer_too_small(what: &'static str) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::BufferTooSmall { what })
    }
    fn invalid_usize(what: &'static str) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::InvalidUsize { what })
    }
    fn version_mismatch(expected: u32, found: u32) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::VersionMismatch { expected, found })
    }
    fn endian_mismatch(expected: u32, found: u32) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::EndianMismatch { expected, found })
    }
    fn alignment_mismatch(alignment: usize, address: usize) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::AlignmentMismatch { alignment, address })
    }
    fn label_mismatch(expected: &'static str) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::LabelMismatch { expected })
    }
    fn arithmetic_overflow(what: &'static str) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::ArithmeticOverflow { what })
    }
    fn pattern_id_error(err: PatternIDError, what: &'static str) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::PatternID { err, what })
    }
    pub(crate) fn state_id_error(err: StateIDError, what: &'static str) -> DeserializeError {
        DeserializeError(DeserializeErrorKind::StateID { err, what })
    }
}
impl std::error::Error for DeserializeError {}
impl core::fmt::Display for DeserializeError {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        use self::DeserializeErrorKind::*;
        match self.0 {
            Generic { msg } => write!(f, "{msg}"),
            BufferTooSmall { what } => {
                write!(f, "buffer is too small to read {what}")
            }
            InvalidUsize { what } => {
                write!(f, "{what} is too big to fit in a usize")
            }
            VersionMismatch { expected, found } => write!(
                f,
                "unsupported version: \
                 expected version {expected} but found version {found}",
            ),
            EndianMismatch { expected, found } => write!(
                f,
                "endianness mismatch: expected 0x{expected:X} but \
                 got 0x{found:X}. (Are you trying to load an object \
                 serialized with a different endianness?)",
            ),
            AlignmentMismatch { alignment, address } => write!(
                f,
                "alignment mismatch: slice starts at address 0x{address:X}, \
                 which is not aligned to a {alignment} byte boundary",
            ),
            LabelMismatch { expected } => write!(
                f,
                "label mismatch: start of serialized object should \
                 contain a NUL terminated {expected:?} label, but a different \
                 label was found",
            ),
            ArithmeticOverflow { what } => {
                write!(f, "arithmetic overflow for {what}")
            }
            PatternID { ref err, what } => {
                write!(f, "failed to read pattern ID for {what}: {err}")
            }
            StateID { ref err, what } => {
                write!(f, "failed to read state ID for {what}: {err}")
            }
        }
    }
}
#[cfg_attr(all(), inline(always))]
pub(crate) fn u32s_to_state_ids(slice: &[u32]) -> &[StateID] {
    unsafe { core::slice::from_raw_parts(slice.as_ptr().cast::<StateID>(), slice.len()) }
}
pub(crate) fn u32s_to_state_ids_mut(slice: &mut [u32]) -> &mut [StateID] {
    unsafe { core::slice::from_raw_parts_mut(slice.as_mut_ptr().cast::<StateID>(), slice.len()) }
}
#[cfg_attr(all(), inline(always))]
pub(crate) fn u32s_to_pattern_ids(slice: &[u32]) -> &[PatternID] {
    unsafe { core::slice::from_raw_parts(slice.as_ptr().cast::<PatternID>(), slice.len()) }
}
pub(crate) fn check_alignment<T>(slice: &[u8]) -> Result<(), DeserializeError> {
    let alignment = core::mem::align_of::<T>();
    let address = slice.as_ptr().as_usize();
    if address % alignment == 0 {
        return Ok(());
    }
    Err(DeserializeError::alignment_mismatch(alignment, address))
}
pub(crate) fn skip_initial_padding(slice: &[u8]) -> usize {
    let mut nread = 0;
    while nread < 7 && nread < slice.len() && slice[nread] == 0 {
        nread += 1;
    }
    nread
}
pub(crate) fn alloc_aligned_buffer<T>(size: usize) -> (Vec<u8>, usize) {
    let buf = vec![0; size];
    let align = core::mem::align_of::<T>();
    let address = buf.as_ptr().as_usize();
    if address % align == 0 {
        return (buf, 0);
    }
    let extra = align - 1;
    let mut buf = vec![0; size + extra];
    let address = buf.as_ptr().as_usize();
    if address % align == 0 {
        buf.truncate(size);
        return (buf, 0);
    }
    let padding = ((address & !(align - 1)).checked_add(align).unwrap())
        .checked_sub(address)
        .unwrap();
    assert!(padding <= 7, "padding of {padding} is bigger than 7");
    assert!(
        padding <= extra,
        "padding of {padding} is bigger than extra {extra} bytes",
    );
    buf.truncate(size + padding);
    assert_eq!(size + padding, buf.len());
    assert_eq!(
        0,
        buf[padding..].as_ptr().as_usize() % align,
        "expected end of initial padding to be aligned to {align}",
    );
    (buf, padding)
}
pub(crate) fn read_label(
    slice: &[u8],
    expected_label: &'static str,
) -> Result<usize, DeserializeError> {
    let first_nul = slice[..cmp::min(slice.len(), 256)]
        .iter()
        .position(|&b| b == 0);
    let first_nul = match first_nul {
        Some(first_nul) => first_nul,
        None => {
            return Err(DeserializeError::generic(
                "could not find NUL terminated label \
                 at start of serialized object",
            ));
        }
    };
    let len = first_nul + padding_len(first_nul);
    if slice.len() < len {
        return Err(DeserializeError::generic(
            "could not find properly sized label at start of serialized object",
        ));
    }
    if expected_label.as_bytes() != &slice[..first_nul] {
        return Err(DeserializeError::label_mismatch(expected_label));
    }
    Ok(len)
}
pub(crate) fn write_label(label: &str, dst: &mut [u8]) -> Result<usize, SerializeError> {
    let nwrite = write_label_len(label);
    if dst.len() < nwrite {
        return Err(SerializeError::buffer_too_small("label"));
    }
    dst[..label.len()].copy_from_slice(label.as_bytes());
    for i in 0..(nwrite - label.len()) {
        dst[label.len() + i] = 0;
    }
    assert_eq!(nwrite % 4, 0);
    Ok(nwrite)
}
pub(crate) fn write_label_len(label: &str) -> usize {
    assert!(
        label.len() <= 255,
        "label must not be longer than 255 bytes"
    );
    assert!(
        label.bytes().all(|b| b != 0),
        "label must not contain NUL bytes"
    );
    let label_len = label.len() + 1;
    label_len + padding_len(label_len)
}
pub(crate) fn read_endianness_check(slice: &[u8]) -> Result<usize, DeserializeError> {
    let (n, nr) = try_read_u32(slice, "endianness check")?;
    assert_eq!(nr, write_endianness_check_len());
    if n != 0xFEFF {
        return Err(DeserializeError::endian_mismatch(0xFEFF, n));
    }
    Ok(nr)
}
pub(crate) fn write_endianness_check<E: Endian>(dst: &mut [u8]) -> Result<usize, SerializeError> {
    let nwrite = write_endianness_check_len();
    if dst.len() < nwrite {
        return Err(SerializeError::buffer_too_small("endianness check"));
    }
    E::write_u32(0xFEFF, dst);
    Ok(nwrite)
}
pub(crate) fn write_endianness_check_len() -> usize {
    size_of::<u32>()
}
pub(crate) fn read_version(slice: &[u8], expected_version: u32) -> Result<usize, DeserializeError> {
    let (n, nr) = try_read_u32(slice, "version")?;
    assert_eq!(nr, write_version_len());
    if n != expected_version {
        return Err(DeserializeError::version_mismatch(expected_version, n));
    }
    Ok(nr)
}
pub(crate) fn write_version<E: Endian>(
    version: u32,
    dst: &mut [u8],
) -> Result<usize, SerializeError> {
    let nwrite = write_version_len();
    if dst.len() < nwrite {
        return Err(SerializeError::buffer_too_small("version number"));
    }
    E::write_u32(version, dst);
    Ok(nwrite)
}
pub(crate) fn write_version_len() -> usize {
    size_of::<u32>()
}

pub(crate) fn read_pattern_id_unchecked(slice: &[u8]) -> (PatternID, usize) {
    let pid = PatternID::from_ne_bytes_unchecked(slice[..PatternID::SIZE].try_into().unwrap());
    (pid, PatternID::SIZE)
}
pub(crate) fn write_pattern_id<E: Endian>(pid: PatternID, dst: &mut [u8]) -> usize {
    E::write_u32(pid.as_u32(), dst);
    PatternID::SIZE
}
pub(crate) fn try_read_state_id(
    slice: &[u8],
    what: &'static str,
) -> Result<(StateID, usize), DeserializeError> {
    if slice.len() < StateID::SIZE {
        return Err(DeserializeError::buffer_too_small(what));
    }
    read_state_id(slice, what)
}
pub(crate) fn read_state_id(
    slice: &[u8],
    what: &'static str,
) -> Result<(StateID, usize), DeserializeError> {
    let bytes: [u8; StateID::SIZE] = slice[..StateID::SIZE].try_into().unwrap();
    let sid =
        StateID::from_ne_bytes(bytes).map_err(|err| DeserializeError::state_id_error(err, what))?;
    Ok((sid, StateID::SIZE))
}

pub(crate) fn write_state_id<E: Endian>(sid: StateID, dst: &mut [u8]) -> usize {
    E::write_u32(sid.as_u32(), dst);
    StateID::SIZE
}

pub(crate) fn try_read_u32_as_usize(
    slice: &[u8],
    what: &'static str,
) -> Result<(usize, usize), DeserializeError> {
    try_read_u32(slice, what).and_then(|(n, nr)| {
        usize::try_from(n)
            .map(|n| (n, nr))
            .map_err(|_| DeserializeError::invalid_usize(what))
    })
}
pub(crate) fn try_read_u16(
    slice: &[u8],
    what: &'static str,
) -> Result<(u16, usize), DeserializeError> {
    check_slice_len(slice, size_of::<u16>(), what)?;
    Ok((read_u16(slice), size_of::<u16>()))
}
pub(crate) fn try_read_u32(
    slice: &[u8],
    what: &'static str,
) -> Result<(u32, usize), DeserializeError> {
    check_slice_len(slice, size_of::<u32>(), what)?;
    Ok((read_u32(slice), size_of::<u32>()))
}
pub(crate) fn try_read_u128(
    slice: &[u8],
    what: &'static str,
) -> Result<(u128, usize), DeserializeError> {
    check_slice_len(slice, size_of::<u128>(), what)?;
    Ok((read_u128(slice), size_of::<u128>()))
}
#[cfg_attr(all(), inline(always))]
pub(crate) fn read_u16(slice: &[u8]) -> u16 {
    let bytes: [u8; 2] = slice[..size_of::<u16>()].try_into().unwrap();
    u16::from_ne_bytes(bytes)
}
#[cfg_attr(all(), inline(always))]
pub(crate) fn read_u32(slice: &[u8]) -> u32 {
    let bytes: [u8; 4] = slice[..size_of::<u32>()].try_into().unwrap();
    u32::from_ne_bytes(bytes)
}
pub(crate) fn read_u128(slice: &[u8]) -> u128 {
    let bytes: [u8; 16] = slice[..size_of::<u128>()].try_into().unwrap();
    u128::from_ne_bytes(bytes)
}
pub(crate) fn check_slice_len<T>(
    slice: &[T],
    at_least_len: usize,
    what: &'static str,
) -> Result<(), DeserializeError> {
    if slice.len() < at_least_len {
        return Err(DeserializeError::buffer_too_small(what));
    }
    Ok(())
}
pub(crate) fn mul(a: usize, b: usize, what: &'static str) -> Result<usize, DeserializeError> {
    match a.checked_mul(b) {
        Some(c) => Ok(c),
        None => Err(DeserializeError::arithmetic_overflow(what)),
    }
}
pub(crate) fn add(a: usize, b: usize, what: &'static str) -> Result<usize, DeserializeError> {
    match a.checked_add(b) {
        Some(c) => Ok(c),
        None => Err(DeserializeError::arithmetic_overflow(what)),
    }
}
pub(crate) fn shl(a: usize, b: usize, what: &'static str) -> Result<usize, DeserializeError> {
    let amount = u32::try_from(b).map_err(|_| DeserializeError::arithmetic_overflow(what))?;
    match a.checked_shl(amount) {
        Some(c) => Ok(c),
        None => Err(DeserializeError::arithmetic_overflow(what)),
    }
}
pub(crate) fn padding_len(non_padding_len: usize) -> usize {
    (4 - (non_padding_len & 0b11)) & 0b11
}
pub(crate) trait Endian {
    fn write_u16(n: u16, dst: &mut [u8]);
    fn write_u32(n: u32, dst: &mut [u8]);
    fn write_u128(n: u128, dst: &mut [u8]);
}
pub(crate) enum LE {}
pub(crate) enum BE {}
#[cfg(target_endian = "little")]
pub(crate) type NE = LE;
#[cfg(target_endian = "big")]
pub(crate) type NE = BE;
impl Endian for LE {
    fn write_u16(n: u16, dst: &mut [u8]) {
        dst[..2].copy_from_slice(&n.to_le_bytes());
    }
    fn write_u32(n: u32, dst: &mut [u8]) {
        dst[..4].copy_from_slice(&n.to_le_bytes());
    }
    fn write_u128(n: u128, dst: &mut [u8]) {
        dst[..16].copy_from_slice(&n.to_le_bytes());
    }
}
impl Endian for BE {
    fn write_u16(n: u16, dst: &mut [u8]) {
        dst[..2].copy_from_slice(&n.to_be_bytes());
    }
    fn write_u32(n: u32, dst: &mut [u8]) {
        dst[..4].copy_from_slice(&n.to_be_bytes());
    }
    fn write_u128(n: u128, dst: &mut [u8]) {
        dst[..16].copy_from_slice(&n.to_be_bytes());
    }
}
