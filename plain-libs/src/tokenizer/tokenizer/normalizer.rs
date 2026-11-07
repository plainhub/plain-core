use crate::tokenizer::pattern::Pattern;
use crate::tokenizer::{Offsets, Result};
use std::ops::{Bound, RangeBounds};
use unicode_normalization_alignments::UnicodeNormalization;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffsetReferential {
    Original,
    Normalized,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Range<T: RangeBounds<usize> + Clone> {
    Original(T),
    Normalized(T),
}

impl<T> Range<T>
where
    T: RangeBounds<usize> + Clone,
{
    pub fn unwrap(self) -> T {
        match self {
            Self::Original(r) => r,
            Self::Normalized(r) => r,
        }
    }

    pub fn len(&self) -> Option<usize> {
        let range = self.clone().unwrap();

        let end = match range.end_bound() {
            Bound::Unbounded => None,
            Bound::Included(i) => Some(*i + 1),
            Bound::Excluded(i) => Some(*i),
        }?;

        match range.start_bound() {
            Bound::Unbounded => Some(end),
            Bound::Included(i) => Some(end - *i),
            Bound::Excluded(i) => Some(end - (*i + 1)),
        }
    }

    pub fn into_full_range(self, max_len: usize) -> std::ops::Range<usize> {
        let range = self.unwrap();

        let start = match range.start_bound() {
            Bound::Unbounded => 0,
            Bound::Included(i) => *i,
            Bound::Excluded(i) => *i + 1,
        };
        let end = match range.end_bound() {
            Bound::Unbounded => max_len,
            Bound::Included(i) => *i + 1,
            Bound::Excluded(i) => *i,
        };

        start..end
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Eq)]
pub enum SplitDelimiterBehavior {
    Removed,
    Isolated,
    MergedWithPrevious,
    MergedWithNext,
    Contiguous,
}

impl std::fmt::Display for SplitDelimiterBehavior {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.serialize(f)
    }
}

#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct NormalizedString {
    original: String,

    normalized: String,

    alignments: Vec<(usize, usize)>,

    original_shift: usize,
}

impl NormalizedString {
    #[cfg(test)]
    pub(crate) fn new(
        original: String,
        normalized: String,
        alignments: Vec<(usize, usize)>,
        original_shift: usize,
    ) -> Self {
        Self {
            original,
            normalized,
            alignments,
            original_shift,
        }
    }

    pub fn get(&self) -> &str {
        &self.normalized
    }

    pub fn get_original(&self) -> &str {
        &self.original
    }

    pub fn offsets_original(&self) -> Offsets {
        (
            self.original_shift,
            self.original_shift + self.len_original(),
        )
    }

    pub fn convert_offsets<T>(&self, range: Range<T>) -> Option<std::ops::Range<usize>>
    where
        T: RangeBounds<usize> + Clone,
    {
        let len_original = self.len_original();
        let len_normalized = self.len();

        let (target, original) = match range {
            Range::Original(_) => (range.into_full_range(len_original), true),
            Range::Normalized(_) => (range.into_full_range(len_normalized), false),
        };

        // If we target an empty range, let's return the same
        if target.start == target.end {
            return Some(target);
        }
        // If the target goes reverse, return None
        if target.start > target.end {
            return None;
        }

        // If we target 0..0 on an empty string, we want to expand to the entire equivalent
        if original && self.original.is_empty() && target == (0..0) {
            return Some(0..len_normalized);
        }
        if !original && self.normalized.is_empty() && target == (0..0) {
            return Some(0..len_original);
        }

        if original {
            let (mut start, mut end) = (None, None);
            self.alignments
                .iter()
                .enumerate()
                .take_while(|(_, alignment)| target.end >= alignment.1)
                .for_each(|(i, alignment)| {
                    if start.is_none() && target.start <= alignment.0 {
                        // For now, don't update if width == 0
                        if alignment.0 != alignment.1 {
                            start = Some(i);
                        }
                    }
                    if target.end >= alignment.1 {
                        end = Some(i + 1);
                    }
                });

            match (start, end) {
                // Targeting inexistent beginning
                (Some(s), None) => Some(s..s),
                // Targeting inexistent end
                (None, Some(e)) => Some(e..e),
                // Found the range
                (Some(s), Some(e)) => Some(s..e),
                _ => None,
            }
        } else {
            self.alignments.get(target).and_then(expand_alignments)
        }
    }

    pub fn get_range<T>(&self, range: Range<T>) -> Option<&str>
    where
        T: RangeBounds<usize> + Clone,
    {
        match range {
            Range::Original(_) => self.normalized.get(self.convert_offsets(range)?),
            Range::Normalized(_) => self.normalized.get(range.into_full_range(self.len())),
        }
    }

    pub fn get_range_original<T>(&self, range: Range<T>) -> Option<&str>
    where
        T: RangeBounds<usize> + Clone,
    {
        match range {
            Range::Original(_) => self
                .original
                .get(range.into_full_range(self.len_original())),
            Range::Normalized(_) => self.original.get(self.convert_offsets(range)?),
        }
    }

    fn validate_range<T: RangeBounds<usize> + Clone>(
        &self,
        range: Range<T>,
    ) -> Option<Range<std::ops::Range<usize>>> {
        match range {
            Range::Original(_) => {
                let r = range.into_full_range(self.original.len());
                if !(self.original.is_char_boundary(r.start)
                    && self.original.is_char_boundary(r.end))
                {
                    None
                } else {
                    Some(Range::Original(r))
                }
            }
            Range::Normalized(_) => {
                let r = range.into_full_range(self.normalized.len());
                if !(self.normalized.is_char_boundary(r.start)
                    && self.normalized.is_char_boundary(r.end))
                {
                    None
                } else {
                    Some(Range::Normalized(r))
                }
            }
        }
    }

    pub fn slice<T>(&self, range: Range<T>) -> Option<NormalizedString>
    where
        T: RangeBounds<usize> + Clone,
    {
        let full_range = self.validate_range(range)?;
        let (normalized_range, original_range) = match full_range {
            Range::Original(_) => (
                self.convert_offsets(full_range.clone())?,
                full_range.clone().unwrap(),
            ),
            Range::Normalized(_) => (
                full_range.clone().unwrap(),
                self.convert_offsets(full_range.clone())?,
            ),
        };

        let n_shift = original_range.start;

        Some(Self {
            original: self
                .get_range_original(full_range.clone())
                .unwrap_or_default()
                .into(),
            normalized: self.get_range(full_range).unwrap_or_default().into(),
            alignments: self
                .alignments
                .get(normalized_range)?
                .to_vec()
                .iter()
                .map(|(start, end)| (start - n_shift, end - n_shift))
                .collect(),
            original_shift: self.original_shift + original_range.start,
        })
    }

    pub fn transform_range<T, I>(&mut self, range: Range<T>, dest: I, initial_offset: usize)
    where
        T: RangeBounds<usize> + Clone,
        I: IntoIterator<Item = (char, isize)>,
    {
        let n_range = match range {
            Range::Normalized(_) => range.into_full_range(self.len()),
            Range::Original(_) => match self.convert_offsets(range) {
                Some(range) => range,
                None => return,
            },
        };
        log::trace!(
            "===== transform_range call with {n_range:?} (initial_offset: {initial_offset}) ====="
        );

        // Retrieve the original characters that are being replaced. This let us
        // compute the change in byte sizes along the way.
        let mut replaced_normalized = self.normalized[n_range.clone()]
            .chars()
            .collect::<Vec<_>>()
            .into_iter();
        let initial_removed: usize = (&mut replaced_normalized)
            .take(initial_offset)
            .map(|c| c.len_utf8())
            .sum();

        let mut offset = (initial_removed + n_range.start) as isize;
        let mut alignments = Vec::with_capacity(n_range.len());
        log::trace!("=> Applying transformations");
        let normalized = dest
            .into_iter()
            .map(|(c, changes)| {
                log::trace!(
                    "### {:?} with size {}: {} with offset {} ###",
                    c,
                    c.len_utf8(),
                    match changes {
                        0 => "Replacing".into(),
                        ch if ch > 0 => "Adding".into(),
                        ch if ch < 0 => format!("Replacing + removing {ch} following chars"),
                        _ => "Undefined".into(),
                    },
                    offset
                );

                let idx = offset as usize;
                let align = if changes.is_positive() {
                    if idx < 1 {
                        (0, 0)
                    } else {
                        // This is a newly inserted character, so it shares the same alignment
                        // than the previous one
                        self.alignments[idx - 1]
                    }
                } else {
                    self.alignments[idx]
                };

                // If we are replacing a character, find it and compute the change in size
                let replaced_char = if !changes.is_positive() {
                    replaced_normalized.next()
                } else {
                    None
                };
                let replaced_char_size = replaced_char.map_or(0, |c| c.len_utf8());
                let replaced_char_size_change = c.len_utf8() as isize - replaced_char_size as isize;
                if let Some(ref replaced_char) = replaced_char {
                    log::trace!(
                        "Replacing char {replaced_char:?} - with a change in size: {replaced_char_size_change}"
                    );
                }

                // If we are removing some characters, find them too
                let total_bytes_to_remove = if changes.is_negative() {
                    (&mut replaced_normalized)
                        .take(-changes as usize)
                        .map(|c| c.len_utf8())
                        .sum()
                } else {
                    0
                };
                log::trace!("Total bytes to remove: {total_bytes_to_remove}");

                // Keep track of the changes for next offsets
                offset += replaced_char_size as isize;
                offset += total_bytes_to_remove as isize;
                log::trace!("New offset: {offset}");

                log::trace!("New normalized alignment: {}x {:?}", c.len_utf8(), align);
                alignments.extend((0..c.len_utf8()).map(|_| align));

                // Then we keep only the char for string reconstruction
                c
            })
            .collect::<String>();

        self.alignments.splice(n_range.clone(), alignments);

        self.normalized.replace_range(n_range, &normalized);
    }

    pub fn transform<I>(&mut self, dest: I, initial_offset: usize)
    where
        I: IntoIterator<Item = (char, isize)>,
    {
        self.transform_range(Range::Original(..), dest, initial_offset)
    }

    pub fn nfd(&mut self) -> &mut Self {
        self.transform(self.get().to_owned().nfd(), 0);
        self
    }

    pub fn nfkd(&mut self) -> &mut Self {
        self.transform(self.get().to_owned().nfkd(), 0);
        self
    }

    pub fn nfc(&mut self) -> &mut Self {
        self.transform(self.get().to_owned().nfc(), 0);
        self
    }

    pub fn nfkc(&mut self) -> &mut Self {
        self.transform(self.get().to_owned().nfkc(), 0);
        self
    }

    pub fn filter<F: Fn(char) -> bool>(&mut self, keep: F) -> &mut Self {
        let mut removed: isize = 0;
        let mut removed_start: usize = 0;

        let mut transforms = Vec::with_capacity(self.normalized.len());
        let mut last_c = None;
        for c in self.normalized.chars() {
            if keep(c) {
                match last_c {
                    Some(lc) => {
                        transforms.push((lc, -removed));
                    }
                    None => {
                        removed_start = removed as usize;
                    }
                }
                last_c = Some(c);
                removed = 0;
            } else {
                removed += 1;
            }
        }
        if let Some(lc) = last_c {
            transforms.push((lc, -removed));
        }
        self.transform(transforms, removed_start);
        self
    }

    pub fn prepend(&mut self, s: &str) -> &mut Self {
        if let Some(next) = self.normalized.chars().next() {
            let transformations = s
                .chars()
                .enumerate()
                .map(|(i, c)| (c, isize::from(i != 0)))
                .chain(std::iter::once((next, 1)));

            self.transform_range(Range::Normalized(0..next.len_utf8()), transformations, 0);
        }
        self
    }

    pub fn append(&mut self, s: &str) -> &mut Self {
        if let Some((b, prev)) = self.normalized.char_indices().last() {
            let transformations = std::iter::once((prev, 0)).chain(s.chars().map(|c| (c, 1)));
            self.transform_range(Range::Normalized(b..), transformations, 0);
        } else {
            let transformations = s.chars().map(|c| (c, 1));
            self.transform_range(Range::Normalized(..), transformations, 0);
        }
        self
    }

    pub fn map<F: Fn(char) -> char>(&mut self, map: F) -> &mut Self {
        let transformations = self
            .normalized
            .chars()
            .map(|c| (map(c), 0))
            .collect::<Vec<_>>();
        self.transform(transformations, 0);
        self
    }

    pub fn for_each<F: FnMut(char)>(&self, foreach: F) -> &Self {
        self.normalized.chars().for_each(foreach);
        self
    }

    pub fn lowercase(&mut self) -> &mut Self {
        let mut new_chars: Vec<(char, isize)> = vec![];
        self.for_each(|c| {
            c.to_lowercase().enumerate().for_each(|(index, c)| {
                new_chars.push((c, isize::from(index > 0)));
            })
        });
        self.transform(new_chars, 0);
        self
    }

    pub fn uppercase(&mut self) -> &mut Self {
        let mut new_chars: Vec<(char, isize)> = vec![];
        self.for_each(|c| {
            c.to_uppercase().enumerate().for_each(|(index, c)| {
                new_chars.push((c, isize::from(index > 0)));
            })
        });
        self.transform(new_chars, 0);
        self
    }

    pub fn replace<P: Pattern>(&mut self, pattern: P, content: &str) -> Result<()> {
        let mut new_normalized = String::with_capacity(self.normalized.len()); // Initially allocate for the input size
        let mut new_alignments: Vec<(usize, usize)> = Vec::with_capacity(self.alignments.len());
        let mut last_end = 0; // Keep track of the last end position

        pattern
            .find_matches(&self.normalized)?
            .into_iter()
            .for_each(|((start, end), is_match)| {
                if is_match {
                    let range = start..end;

                    let mut new_len = 0;
                    let removed_chars = self.normalized[range.clone()].chars().count();

                    /* The following code is equivalent to this call, but computationally much more efficient
                    self.transform_range(
                        Range::Normalized(range),
                        content.chars().map(|c| {
                            new_len += c.len_utf8();
                            (c, 1)
                        }),
                        removed_chars,
                    ); */

                    // Copy the part of the string that is before the match
                    new_normalized.push_str(&self.normalized[last_end..start]);
                    new_alignments.extend(self.alignments[last_end..start].iter().cloned());

                    let n_range = Range::Normalized(range).into_full_range(self.len());

                    // Retrieve the original characters that are being replaced. This let us
                    // compute the change in byte sizes along the way.
                    let mut replaced_normalized = self.normalized[n_range.clone()]
                        .chars()
                        .collect::<Vec<_>>()
                        .into_iter();
                    let initial_removed: usize = (&mut replaced_normalized)
                        .take(removed_chars)
                        .map(|c| c.len_utf8())
                        .sum();

                    let dest = content.chars().map(|c| {
                        new_len += c.len_utf8();
                        (c, 1)
                    });
                    let mut offset = (initial_removed + n_range.start) as isize;
                    let normalized = dest
                        .into_iter()
                        .map(|(c, changes): (char, i32)| {
                            let idx = offset as usize;
                            let align = if changes.is_positive() {
                                if idx < 1 {
                                    (0, 0)
                                } else {
                                    // This is a newly inserted character, so it shares the same alignment
                                    // than the previous one
                                    self.alignments[idx - 1]
                                }
                            } else {
                                self.alignments[idx]
                            };

                            // If we are replacing a character, find it and compute the change in size
                            let replaced_char = if !changes.is_positive() {
                                replaced_normalized.next()
                            } else {
                                None
                            };
                            let replaced_char_size = replaced_char.map_or(0, |c| c.len_utf8());

                            // If we are removing some characters, find them too
                            let total_bytes_to_remove = if changes.is_negative() {
                                (&mut replaced_normalized)
                                    .take(-changes as usize)
                                    .map(|c| c.len_utf8())
                                    .sum()
                            } else {
                                0
                            };

                            // Keep track of the changes for next offsets
                            offset += replaced_char_size as isize;
                            offset += total_bytes_to_remove as isize;

                            new_alignments.extend((0..c.len_utf8()).map(|_| align));

                            // Then we keep only the char for string reconstruction
                            c
                        })
                        .collect::<String>();

                    new_normalized.push_str(&normalized);
                    last_end = end;
                }
            });

        // Copy the remaining part of the input
        new_normalized.push_str(&self.normalized[last_end..]);
        new_alignments.extend(&self.alignments[last_end..]);

        self.normalized = new_normalized;
        self.alignments = new_alignments;
        Ok(())
    }

    pub fn clear(&mut self) -> usize {
        let len = self.len();
        self.transform(std::iter::empty(), len);
        len
    }

    pub fn split<P: Pattern>(
        &self,
        pattern: P,
        behavior: SplitDelimiterBehavior,
    ) -> Result<Vec<NormalizedString>> {
        let matches = pattern.find_matches(&self.normalized)?;

        // Process the matches according to the selected behavior: Vec<(Offsets, should_remove)>
        use SplitDelimiterBehavior::*;
        let splits = match behavior {
            Isolated => matches
                .into_iter()
                .map(|(offsets, _)| (offsets, false))
                .collect(),
            Removed => matches,
            Contiguous => {
                let mut previous_match = false;
                matches
                    .into_iter()
                    .fold(vec![], |mut acc, (offsets, is_match)| {
                        if is_match == previous_match {
                            if let Some(((_, end), _)) = acc.last_mut() {
                                *end = offsets.1;
                            } else {
                                acc.push((offsets, false));
                            }
                        } else {
                            acc.push((offsets, false));
                        }
                        previous_match = is_match;
                        acc
                    })
            }
            MergedWithPrevious => {
                let mut previous_match = false;
                matches
                    .into_iter()
                    .fold(vec![], |mut acc, (offsets, is_match)| {
                        if is_match && !previous_match {
                            if let Some(((_, end), _)) = acc.last_mut() {
                                *end = offsets.1;
                            } else {
                                acc.push((offsets, false));
                            }
                        } else {
                            acc.push((offsets, false));
                        }
                        previous_match = is_match;
                        acc
                    })
            }
            MergedWithNext => {
                let mut previous_match = false;
                let mut matches =
                    matches
                        .into_iter()
                        .rev()
                        .fold(vec![], |mut acc, (offsets, is_match)| {
                            if is_match && !previous_match {
                                if let Some(((start, _), _)) = acc.last_mut() {
                                    *start = offsets.0;
                                } else {
                                    acc.push((offsets, false));
                                }
                            } else {
                                acc.push((offsets, false));
                            }
                            previous_match = is_match;
                            acc
                        });
                matches.reverse();
                matches
            }
        };

        // Then we split according to the computed splits
        Ok(splits
            .into_iter()
            .filter_map(|(offsets, remove)| {
                if !remove {
                    Some(
                        self.slice(Range::Normalized(offsets.0..offsets.1))
                            .expect("NormalizedString bad split"),
                    )
                } else {
                    None
                }
            })
            .collect())
    }

    pub fn lstrip(&mut self) -> &mut Self {
        self.lrstrip(true, false)
    }

    pub fn rstrip(&mut self) -> &mut Self {
        self.lrstrip(false, true)
    }

    pub fn strip(&mut self) -> &mut Self {
        self.lrstrip(true, true)
    }

    fn lrstrip(&mut self, left: bool, right: bool) -> &mut Self {
        let leading_spaces = if left {
            self.get().chars().take_while(|c| c.is_whitespace()).count()
        } else {
            0
        };
        let trailing_spaces = if right {
            self.get()
                .chars()
                .rev()
                .take_while(|c| c.is_whitespace())
                .count()
        } else {
            0
        };

        if leading_spaces > 0 || trailing_spaces > 0 {
            let count = self.get().chars().count();
            let transformation = self
                .normalized
                .chars()
                .enumerate()
                .filter_map(|(i, c)| {
                    if i < leading_spaces || i >= count - trailing_spaces {
                        None
                    } else if i == self.len() - trailing_spaces - 1 {
                        Some((c, -(trailing_spaces as isize)))
                    } else {
                        Some((c, 0))
                    }
                })
                .collect::<Vec<_>>();
            self.transform(transformation, leading_spaces);
        }
        self
    }

    pub fn len(&self) -> usize {
        self.normalized.len()
    }

    pub fn len_original(&self) -> usize {
        self.original.len()
    }

    pub fn is_empty(&self) -> bool {
        self.normalized.is_empty()
    }

    pub(crate) fn alignments_original(&self) -> Vec<(usize, usize)> {
        // Start, end are in alignments
        // offset, length are in alignments_original
        let mut alignments_original = Vec::with_capacity(self.original.len());

        // Eventual gap before first group
        let start = self.alignments[0].0;
        if start != 0 {
            alignments_original.extend(vec![(0, 0); start]);
        }

        let mut last = (&self.alignments[0].0, &self.alignments[0].1);
        let mut offset = 0;
        let mut length = 0;
        for (start, end) in &self.alignments {
            if last == (start, end) {
                // This is the same group
                length += 1;
            } else {
                // This is a new group
                if start < last.1 {
                    panic!("We can't have overlapping ranges.");
                }

                // Add the old group
                alignments_original.extend(vec![(offset, offset + length); last.1 - last.0]);
                offset += length;
                length = 1;

                // Eventual gap between the 2 groups
                alignments_original.extend(vec![(offset, offset); start - last.1]);
            }

            last = (start, end);
        }
        // Add the last group
        alignments_original.extend(vec![(offset, offset + length); last.1 - last.0]);

        // Add eventual last gap
        offset += length;
        alignments_original.extend(vec![
            (offset, offset);
            self.original.len() - alignments_original.len()
        ]);

        // assert_eq!(alignments_original.len(), self.original.len());
        alignments_original
    }
}

fn expand_alignments(alignments: &[(usize, usize)]) -> Option<std::ops::Range<usize>> {
    if alignments.is_empty() {
        None
    } else {
        let start = alignments[0].0;
        let end = alignments[alignments.len() - 1].1;
        Some(start..end)
    }
}

pub fn get_range_of<T: RangeBounds<usize>>(s: &str, range: T) -> Option<&str> {
    let len = s.chars().count();
    let start = match range.start_bound() {
        Bound::Unbounded => 0,
        Bound::Included(i) => *i,
        Bound::Excluded(i) => *i + 1,
    };
    let end = match range.end_bound() {
        Bound::Unbounded => len,
        Bound::Included(i) => *i + 1,
        Bound::Excluded(i) => *i,
    };

    if start == 0 && end == 0 {
        Some(&s[0..0])
    } else if start >= len || end > len || start >= end {
        None
    } else {
        let start_b = s.char_indices().map(|(i, _)| i).nth(start).unwrap_or(0);
        let end_b = s.char_indices().map(|(i, _)| i).nth(end).unwrap_or(s.len());
        Some(&s[start_b..end_b])
    }
}

pub fn bytes_to_char(s: &str, range: std::ops::Range<usize>) -> Option<std::ops::Range<usize>> {
    let (mut start, mut end) = if range == (0..0) {
        (Some(0), Some(0))
    } else {
        (None, None)
    };

    s.char_indices()
        .enumerate()
        .take_while(|(_, (b, _))| *b <= range.end)
        .filter(|(_, (b, _))| *b >= range.start)
        .for_each(|(i, (b, c))| {
            if b == range.start {
                start = Some(i);
            }
            if b == range.end {
                end = Some(i);
            }
            if b + c.len_utf8() == range.end {
                end = Some(i + 1);
            }
        });

    Some(start?..end?)
}

pub fn char_to_bytes(s: &str, range: std::ops::Range<usize>) -> Option<std::ops::Range<usize>> {
    let (mut start, mut end) = if range == (0..0) {
        (Some(0), Some(0))
    } else {
        (None, None)
    };

    if range.start == range.end {
        s.char_indices()
            .skip(range.start)
            .take(1)
            .for_each(|(b, _)| {
                start = Some(b);
                end = Some(b);
            });
    } else {
        s.char_indices()
            .skip(range.start)
            .take(range.end - range.start)
            .for_each(|(b, c)| {
                if start.is_none() {
                    start = Some(b);
                }
                end = Some(b + c.len_utf8());
            });
    }

    Some(start?..end?)
}

impl From<String> for NormalizedString {
    fn from(s: String) -> Self {
        let alignments = s
            .char_indices()
            .flat_map(|(b, c)| {
                let len = c.len_utf8();
                (0..len).map(move |_| (b, b + len))
            })
            .collect::<Vec<_>>();
        Self {
            original: s.clone(),
            normalized: s,
            alignments,
            original_shift: 0,
        }
    }
}

impl From<&str> for NormalizedString {
    fn from(s: &str) -> Self {
        Self::from(s.to_owned())
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/tokenizer/tokenizer/normalizer/tests.rs"]
mod tests;
