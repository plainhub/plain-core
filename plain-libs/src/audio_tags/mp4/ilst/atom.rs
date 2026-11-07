use crate::audio_tags::error::Result;
use crate::audio_tags::macros::err;
use crate::audio_tags::mp4::AtomIdent;
use crate::audio_tags::mp4::ilst::data_type::DataType;
use crate::audio_tags::picture::Picture;

use std::fmt::{Debug, Formatter};

// Atoms with multiple values aren't all that common,
// so there's no need to create a bunch of single-element Vecs
#[derive(PartialEq, Clone)]
pub(super) enum AtomDataStorage {
    Single(AtomData),
    Multiple(Vec<AtomData>),
}

impl AtomDataStorage {
    pub(super) fn first_mut(&mut self) -> &mut AtomData {
        match self {
            AtomDataStorage::Single(val) => val,
            AtomDataStorage::Multiple(data) => data.first_mut().expect("not empty"),
        }
    }

    pub(super) fn is_pictures(&self) -> bool {
        match self {
            AtomDataStorage::Single(v) => matches!(v, AtomData::Picture(_)),
            AtomDataStorage::Multiple(v) => v.iter().all(|p| matches!(p, AtomData::Picture(_))),
        }
    }

    pub(super) fn from_vec(mut v: Vec<AtomData>) -> Option<Self> {
        match v.len() {
            0 => None,
            1 => Some(AtomDataStorage::Single(v.remove(0))),
            _ => Some(AtomDataStorage::Multiple(v)),
        }
    }
}

impl Debug for AtomDataStorage {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match &self {
            AtomDataStorage::Single(v) => write!(f, "{:?}", v),
            AtomDataStorage::Multiple(v) => f.debug_list().entries(v.iter()).finish(),
        }
    }
}

impl IntoIterator for AtomDataStorage {
    type Item = AtomData;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            AtomDataStorage::Single(s) => vec![s].into_iter(),
            AtomDataStorage::Multiple(v) => v.into_iter(),
        }
    }
}

impl<'a> IntoIterator for &'a AtomDataStorage {
    type Item = &'a AtomData;
    type IntoIter = AtomDataStorageIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        let cap = match self {
            AtomDataStorage::Single(_) => 0,
            AtomDataStorage::Multiple(v) => v.len(),
        };

        Self::IntoIter {
            storage: Some(self),
            idx: 0,
            cap,
        }
    }
}

pub(super) struct AtomDataStorageIter<'a> {
    storage: Option<&'a AtomDataStorage>,
    idx: usize,
    cap: usize,
}

impl<'a> Iterator for AtomDataStorageIter<'a> {
    type Item = &'a AtomData;

    fn next(&mut self) -> Option<Self::Item> {
        match self.storage {
            Some(AtomDataStorage::Single(data)) => {
                self.storage = None;
                Some(data)
            }
            Some(AtomDataStorage::Multiple(data)) => {
                if self.idx == self.cap {
                    self.storage = None;
                    return None;
                }

                let ret = &data[self.idx];
                self.idx += 1;

                Some(ret)
            }
            _ => None,
        }
    }
}

#[derive(PartialEq, Clone)]
pub struct Atom<'a> {
    pub(crate) ident: AtomIdent<'a>,
    pub(super) data: AtomDataStorage,
}

impl<'a> Atom<'a> {
    #[must_use]
    pub const fn new(ident: AtomIdent<'a>, data: AtomData) -> Self {
        Self {
            ident,
            data: AtomDataStorage::Single(data),
        }
    }

    pub fn from_collection(ident: AtomIdent<'a>, mut data: Vec<AtomData>) -> Option<Self> {
        let data = match data.len() {
            0 => return None,
            1 => AtomDataStorage::Single(data.swap_remove(0)),
            _ => AtomDataStorage::Multiple(data),
        };

        Some(Self { ident, data })
    }

    pub fn ident(&self) -> &AtomIdent<'_> {
        &self.ident
    }

    pub fn data(&self) -> impl Iterator<Item = &AtomData> {
        (&self.data).into_iter()
    }

    pub fn into_data(self) -> impl Iterator<Item = AtomData> + use<> {
        self.data.into_iter()
    }

    pub fn push_data(&mut self, data: AtomData) {
        match self.data {
            AtomDataStorage::Single(ref s) => {
                self.data = AtomDataStorage::Multiple(vec![s.clone(), data])
            }
            AtomDataStorage::Multiple(ref mut m) => m.push(data),
        }
    }

    pub fn merge(&mut self, other: Atom<'_>) -> Result<()> {
        if self.ident != other.ident {
            err!(AtomMismatch);
        }

        for data in other.data {
            self.push_data(data)
        }

        Ok(())
    }

    // Used internally, has no correctness checks
    pub(crate) fn unknown_implicit(ident: AtomIdent<'_>, data: Vec<u8>) -> Self {
        Self {
            ident: ident.into_owned(),
            data: AtomDataStorage::Single(AtomData::Unknown {
                code: DataType::Reserved,
                data,
            }),
        }
    }

    pub(crate) fn text(ident: AtomIdent<'_>, data: String) -> Self {
        Self {
            ident: ident.into_owned(),
            data: AtomDataStorage::Single(AtomData::UTF8(data)),
        }
    }

    pub(crate) fn into_owned(self) -> Atom<'static> {
        let Self { ident, data } = self;
        Atom {
            ident: ident.into_owned(),
            data,
        }
    }
}

impl Debug for Atom<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Atom")
            .field("ident", &self.ident)
            .field("data", &self.data)
            .finish()
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AtomData {
    UTF8(String),

    UTF16(String),

    Picture(Picture),

    SignedInteger(i32),

    UnsignedInteger(u32),

    Bool(bool),

    Unknown { code: DataType, data: Vec<u8> },
}

impl AtomData {
    pub fn data_type(&self) -> DataType {
        match self {
            AtomData::UTF8(_) => DataType::Utf8,
            AtomData::UTF16(_) => DataType::Utf16,
            AtomData::SignedInteger(_) | AtomData::Bool(_) => DataType::BeSignedInteger,
            AtomData::UnsignedInteger(_) => DataType::BeUnsignedInteger,
            AtomData::Picture(p) => {
                let Some(mime) = p.mime_type() else {
                    return DataType::Reserved;
                };

                DataType::from(mime)
            }
            AtomData::Unknown { code, .. } => *code,
        }
    }
}
