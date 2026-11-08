// Unicode 9 normalization and alignment data from unicode-normalization-alignments 0.1.12 (SOURCE.json).
mod decompose;
mod lookups;
mod normalize;
mod perfect_hash;
mod recompose;
mod tables;
#[cfg(feature = "document-nfc")]
mod unicode18;

#[cfg(feature = "ai-image-search")]
pub(crate) use lookups::is_combining_mark;

#[inline]
#[cfg(feature = "ai-image-search")]
pub(crate) fn nfd(text: &str) -> impl Iterator<Item = (char, isize)> + '_ {
    decompose::new_canonical::<_, _, false>(text.chars())
}

#[inline]
#[cfg(feature = "ai-image-search")]
pub(crate) fn nfkd(text: &str) -> impl Iterator<Item = (char, isize)> + '_ {
    decompose::new_compatible(text.chars())
}

#[inline]
#[cfg(feature = "ai-image-search")]
pub(crate) fn nfc(text: &str) -> impl Iterator<Item = (char, isize)> + '_ {
    recompose::new_canonical::<_, _, false>(text.chars())
}

#[inline]
#[cfg(feature = "ai-image-search")]
pub(crate) fn nfkc(text: &str) -> impl Iterator<Item = (char, isize)> + '_ {
    recompose::new_compatible(text.chars())
}

#[cfg(feature = "document-nfc")]
pub fn nfc_text(text: &str) -> String {
    if text.is_ascii() {
        return text.to_owned();
    }
    recompose::new_canonical::<_, char, true>(text.chars()).collect()
}

trait Character: Copy {
    type Entry: Copy;

    fn entry(class: u8, ch: char, first: bool) -> Self::Entry;
    fn class(entry: &Self::Entry) -> u8;
    fn item(entry: Self::Entry) -> Self;
    fn character(self) -> char;
    fn merge(self, other: Self, composed: char) -> Self;
}

impl Character for (char, isize) {
    type Entry = (u8, char, isize);

    #[inline]
    fn entry(class: u8, ch: char, first: bool) -> Self::Entry {
        (class, ch, if first { 0 } else { 1 })
    }

    #[inline]
    fn class(entry: &Self::Entry) -> u8 {
        entry.0
    }

    #[inline]
    fn item((_, ch, change): Self::Entry) -> Self {
        (ch, change)
    }

    #[inline]
    fn character(self) -> char {
        self.0
    }

    #[inline]
    fn merge(self, other: Self, composed: char) -> Self {
        (composed, self.1 + other.1 - 1)
    }
}

#[cfg(feature = "document-nfc")]
impl Character for char {
    type Entry = (u8, char);

    #[inline]
    fn entry(class: u8, ch: char, _: bool) -> Self::Entry {
        (class, ch)
    }

    #[inline]
    fn class(entry: &Self::Entry) -> u8 {
        entry.0
    }

    #[inline]
    fn item((_, ch): Self::Entry) -> Self {
        ch
    }

    #[inline]
    fn character(self) -> char {
        self
    }

    #[inline]
    fn merge(self, _: Self, composed: char) -> Self {
        composed
    }
}
