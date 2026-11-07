use crate::audio_tags::error::Result;
use crate::audio_tags::macros::err;

use crate::audio_tags::config::global_options;

trait VecFallibleRepeat<T>: Sized {
    fn fallible_repeat(self, element: T, expected_size: usize) -> Result<Self>
    where
        T: Clone;
}

impl<T> VecFallibleRepeat<T> for Vec<T> {
    fn fallible_repeat(mut self, element: T, expected_size: usize) -> Result<Self>
    where
        T: Clone,
    {
        if expected_size == 0 {
            return Ok(self);
        }

        if expected_size > global_options().allocation_limit {
            err!(TooMuchData);
        }

        self.try_reserve(expected_size)?;

        self.resize(expected_size, element);

        Ok(self)
    }
}

pub(crate) fn fallible_vec_from_element<T>(element: T, expected_size: usize) -> Result<Vec<T>>
where
    T: Clone,
{
    Vec::new().fallible_repeat(element, expected_size)
}

pub(crate) trait VecFallibleCapacity<T>: Sized {
    fn try_with_capacity_stable(capacity: usize) -> Result<Self>;
}

impl<T> VecFallibleCapacity<T> for Vec<T> {
    fn try_with_capacity_stable(capacity: usize) -> Result<Self> {
        if capacity > global_options().allocation_limit {
            err!(TooMuchData);
        }

        let mut v = Vec::new();
        v.try_reserve(capacity)?;

        Ok(v)
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/audio_tags/util/alloc/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/audio_tags/util/alloc_safety.rs"]
mod safety_tests;
