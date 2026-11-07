use std::array::{IntoIter, from_fn};
use std::iter::{Flatten, FusedIterator};

#[derive(Debug, Clone)]
pub struct ArrayChunks<I: Iterator, const N: usize> {
    iter: I,
    remainder: Option<Flatten<IntoIter<Option<I::Item>, N>>>,
}

impl<I: Iterator, const N: usize> ArrayChunks<I, N> {
    pub fn new(iter: I) -> Self {
        assert_ne!(N, 0, "chunk size must be non-zero");
        Self {
            iter,
            remainder: None,
        }
    }

    pub fn into_remainder(self) -> Option<Flatten<IntoIter<Option<I::Item>, N>>> {
        self.remainder
    }
}

impl<I: Iterator, const N: usize> Iterator for ArrayChunks<I, N> {
    type Item = [I::Item; N];

    fn next(&mut self) -> Option<Self::Item> {
        let mut chunk: [Option<I::Item>; N] = from_fn(|_| None);
        for index in 0..N {
            let Some(item) = self.iter.next() else {
                self.remainder.get_or_insert(chunk.into_iter().flatten());
                return None;
            };
            chunk[index] = Some(item);
        }
        Some(chunk.map(Option::unwrap))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.iter.size_hint();
        (lower / N, upper.map(|size| size / N))
    }

    fn count(self) -> usize {
        self.iter.count() / N
    }
}

impl<I: FusedIterator, const N: usize> FusedIterator for ArrayChunks<I, N> {}

impl<I: ExactSizeIterator, const N: usize> ExactSizeIterator for ArrayChunks<I, N> {
    fn len(&self) -> usize {
        self.iter.len() / N
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/image_resize/array_chunks.rs"]
mod tests;
