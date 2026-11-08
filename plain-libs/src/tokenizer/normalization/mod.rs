// Unicode 9 normalization and alignment data from unicode-normalization-alignments 0.1.12 (SOURCE.json).
mod decompose;
mod lookups;
mod normalize;
mod perfect_hash;
mod recompose;
mod tables;

pub(super) use lookups::is_combining_mark;

#[inline]
pub(super) fn nfd(text: &str) -> impl Iterator<Item = (char, isize)> + '_ {
    decompose::new_canonical(text.chars())
}

#[inline]
pub(super) fn nfkd(text: &str) -> impl Iterator<Item = (char, isize)> + '_ {
    decompose::new_compatible(text.chars())
}

#[inline]
pub(super) fn nfc(text: &str) -> impl Iterator<Item = (char, isize)> + '_ {
    recompose::new_canonical(text.chars())
}

#[inline]
pub(super) fn nfkc(text: &str) -> impl Iterator<Item = (char, isize)> + '_ {
    recompose::new_compatible(text.chars())
}
