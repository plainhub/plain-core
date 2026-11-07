// *********************
// Reference Conversions
// *********************

use crate::audio_tags::error::{LoftyError, Result};
use crate::audio_tags::mp4::{Atom, AtomData, AtomIdent, Ilst};

use std::io::Write;

impl Ilst {
    pub(crate) fn as_ref(&self) -> IlstRef<'_, impl IntoIterator<Item = &AtomData>> {
        IlstRef {
            atoms: Box::new(self.atoms.iter().map(Atom::as_ref)),
        }
    }
}

pub(crate) struct IlstRef<'a, I> {
    pub(super) atoms: Box<dyn Iterator<Item = AtomRef<'a, I>> + 'a>,
}

impl<'a, I: 'a> IlstRef<'a, I> where I: IntoIterator<Item = &'a AtomData> {}

impl Atom<'_> {
    pub(super) fn as_ref(&self) -> AtomRef<'_, impl IntoIterator<Item = &AtomData>> {
        AtomRef {
            ident: self.ident.as_borrowed(),
            data: (&self.data).into_iter(),
        }
    }
}

pub(crate) struct AtomRef<'a, I> {
    pub(crate) ident: AtomIdent<'a>,
    pub(crate) data: I,
}
