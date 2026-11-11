use crate::image_codecs::png::adam7::{Adam7Info, Adam7Iterator};
use std::ops::Range;
#[derive(Clone, Copy, Debug)]
pub enum InterlaceInfo {
    Null(NullInfo),
    Adam7(Adam7Info),
}
#[derive(Clone, Copy, Debug)]
pub struct NullInfo {
    line: u32,
}
impl InterlaceInfo {
    pub(crate) fn line_number(&self) -> u32 {
        match self {
            InterlaceInfo::Null(NullInfo { line }) => *line,
            InterlaceInfo::Adam7(Adam7Info { line, .. }) => *line,
        }
    }
    pub(crate) fn get_adam7_info(&self) -> Option<&Adam7Info> {
        match self {
            InterlaceInfo::Null(_) => None,
            InterlaceInfo::Adam7(adam7info) => Some(adam7info),
        }
    }
}
pub(crate) struct InterlaceInfoIter(IterImpl);
impl InterlaceInfoIter {
    pub fn empty() -> Self {
        Self(IterImpl::None(0..0))
    }
    pub fn new(width: u32, height: u32, interlaced: bool) -> Self {
        if interlaced {
            Self(IterImpl::Adam7(Adam7Iterator::new(width, height)))
        } else {
            Self(IterImpl::None(0..height))
        }
    }
}
impl Iterator for InterlaceInfoIter {
    type Item = InterlaceInfo;
    fn next(&mut self) -> Option<InterlaceInfo> {
        match self.0 {
            IterImpl::Adam7(ref mut adam7) => Some(InterlaceInfo::Adam7(adam7.next()?)),
            IterImpl::None(ref mut height) => Some(InterlaceInfo::Null(NullInfo {
                line: height.next()?,
            })),
        }
    }
}
enum IterImpl {
    None(Range<u32>),
    Adam7(Adam7Iterator),
}
