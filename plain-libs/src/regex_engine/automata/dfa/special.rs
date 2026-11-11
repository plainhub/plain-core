use crate::regex_engine::automata::{
    dfa::DEAD,
    util::{
        primitives::StateID,
        wire::{self, DeserializeError, Endian, SerializeError},
    },
};
macro_rules! err {
    ($ msg : expr) => {
        return Err(DeserializeError::generic($msg));
    };
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Special {
    pub(crate) max: StateID,
    pub(crate) quit_id: StateID,
    pub(crate) min_match: StateID,
    pub(crate) max_match: StateID,
    pub(crate) min_accel: StateID,
    pub(crate) max_accel: StateID,
    pub(crate) min_start: StateID,
    pub(crate) max_start: StateID,
}
impl Special {
    pub(crate) fn new() -> Special {
        Special {
            max: DEAD,
            quit_id: DEAD,
            min_match: DEAD,
            max_match: DEAD,
            min_accel: DEAD,
            max_accel: DEAD,
            min_start: DEAD,
            max_start: DEAD,
        }
    }
    pub(crate) fn remap(&self, map: impl Fn(StateID) -> StateID) -> Special {
        Special {
            max: map(self.max),
            quit_id: map(self.quit_id),
            min_match: map(self.min_match),
            max_match: map(self.max_match),
            min_accel: map(self.min_accel),
            max_accel: map(self.max_accel),
            min_start: map(self.min_start),
            max_start: map(self.max_start),
        }
    }
    pub(crate) fn from_bytes(mut slice: &[u8]) -> Result<(Special, usize), DeserializeError> {
        wire::check_slice_len(slice, 8 * StateID::SIZE, "special states")?;
        let mut nread = 0;
        let mut read_id = |what| -> Result<StateID, DeserializeError> {
            let (id, nr) = wire::try_read_state_id(slice, what)?;
            nread += nr;
            slice = &slice[StateID::SIZE..];
            Ok(id)
        };
        let max = read_id("special max id")?;
        let quit_id = read_id("special quit id")?;
        let min_match = read_id("special min match id")?;
        let max_match = read_id("special max match id")?;
        let min_accel = read_id("special min accel id")?;
        let max_accel = read_id("special max accel id")?;
        let min_start = read_id("special min start id")?;
        let max_start = read_id("special max start id")?;
        let special = Special {
            max,
            quit_id,
            min_match,
            max_match,
            min_accel,
            max_accel,
            min_start,
            max_start,
        };
        special.validate()?;
        assert_eq!(nread, special.write_to_len());
        Ok((special, nread))
    }
    pub(crate) fn validate(&self) -> Result<(), DeserializeError> {
        if self.min_match == DEAD && self.max_match != DEAD {
            err!("min_match is DEAD, but max_match is not");
        }
        if self.min_match != DEAD && self.max_match == DEAD {
            err!("max_match is DEAD, but min_match is not");
        }
        if self.min_accel == DEAD && self.max_accel != DEAD {
            err!("min_accel is DEAD, but max_accel is not");
        }
        if self.min_accel != DEAD && self.max_accel == DEAD {
            err!("max_accel is DEAD, but min_accel is not");
        }
        if self.min_start == DEAD && self.max_start != DEAD {
            err!("min_start is DEAD, but max_start is not");
        }
        if self.min_start != DEAD && self.max_start == DEAD {
            err!("max_start is DEAD, but min_start is not");
        }
        if self.min_match > self.max_match {
            err!("min_match should not be greater than max_match");
        }
        if self.min_accel > self.max_accel {
            err!("min_accel should not be greater than max_accel");
        }
        if self.min_start > self.max_start {
            err!("min_start should not be greater than max_start");
        }
        if self.matches() && self.quit_id >= self.min_match {
            err!("quit_id should not be greater than min_match");
        }
        if self.accels() && self.quit_id >= self.min_accel {
            err!("quit_id should not be greater than min_accel");
        }
        if self.starts() && self.quit_id >= self.min_start {
            err!("quit_id should not be greater than min_start");
        }
        if self.matches() && self.accels() && self.min_accel < self.min_match {
            err!("min_match should not be greater than min_accel");
        }
        if self.matches() && self.starts() && self.min_start < self.min_match {
            err!("min_match should not be greater than min_start");
        }
        if self.accels() && self.starts() && self.min_start < self.min_accel {
            err!("min_accel should not be greater than min_start");
        }
        if self.max < self.quit_id {
            err!("quit_id should not be greater than max");
        }
        if self.max < self.max_match {
            err!("max_match should not be greater than max");
        }
        if self.max < self.max_accel {
            err!("max_accel should not be greater than max");
        }
        if self.max < self.max_start {
            err!("max_start should not be greater than max");
        }
        Ok(())
    }
    pub(crate) fn validate_state_len(
        &self,
        len: usize,
        stride2: usize,
    ) -> Result<(), DeserializeError> {
        if (self.max.as_usize() >> stride2) >= len {
            err!("max should not be greater than or equal to state length");
        }
        Ok(())
    }
    pub(crate) fn write_to<E: Endian>(&self, dst: &mut [u8]) -> Result<usize, SerializeError> {
        use crate::regex_engine::automata::util::wire::write_state_id as write;
        if dst.len() < self.write_to_len() {
            return Err(SerializeError::buffer_too_small("special state ids"));
        }
        let mut nwrite = 0;
        nwrite += write::<E>(self.max, &mut dst[nwrite..]);
        nwrite += write::<E>(self.quit_id, &mut dst[nwrite..]);
        nwrite += write::<E>(self.min_match, &mut dst[nwrite..]);
        nwrite += write::<E>(self.max_match, &mut dst[nwrite..]);
        nwrite += write::<E>(self.min_accel, &mut dst[nwrite..]);
        nwrite += write::<E>(self.max_accel, &mut dst[nwrite..]);
        nwrite += write::<E>(self.min_start, &mut dst[nwrite..]);
        nwrite += write::<E>(self.max_start, &mut dst[nwrite..]);
        assert_eq!(
            self.write_to_len(),
            nwrite,
            "expected to write certain number of bytes",
        );
        assert_eq!(
            nwrite % 8,
            0,
            "expected to write multiple of 8 bytes for special states",
        );
        Ok(nwrite)
    }
    pub(crate) fn write_to_len(&self) -> usize {
        8 * StateID::SIZE
    }
    pub(crate) fn set_max(&mut self) {
        use core::cmp::max;
        self.max = max(
            self.quit_id,
            max(self.max_match, max(self.max_accel, self.max_start)),
        );
    }
    pub(crate) fn set_no_special_start_states(&mut self) {
        use core::cmp::max;
        self.max = max(self.quit_id, max(self.max_match, self.max_accel));
        self.min_start = DEAD;
        self.max_start = DEAD;
    }
    #[inline]
    pub(crate) fn is_special_state(&self, id: StateID) -> bool {
        id <= self.max
    }
    #[inline]
    pub(crate) fn is_dead_state(&self, id: StateID) -> bool {
        id == DEAD
    }
    #[inline]
    pub(crate) fn is_quit_state(&self, id: StateID) -> bool {
        !self.is_dead_state(id) && self.quit_id == id
    }
    #[inline]
    pub(crate) fn is_match_state(&self, id: StateID) -> bool {
        !self.is_dead_state(id) && self.min_match <= id && id <= self.max_match
    }
    #[inline]
    pub(crate) fn is_accel_state(&self, id: StateID) -> bool {
        !self.is_dead_state(id) && self.min_accel <= id && id <= self.max_accel
    }
    #[inline]
    pub(crate) fn is_start_state(&self, id: StateID) -> bool {
        !self.is_dead_state(id) && self.min_start <= id && id <= self.max_start
    }
    #[inline]
    pub(crate) fn match_len(&self, stride: usize) -> usize {
        if self.matches() {
            (self.max_match.as_usize() - self.min_match.as_usize() + stride) / stride
        } else {
            0
        }
    }
    #[inline]
    pub(crate) fn matches(&self) -> bool {
        self.min_match != DEAD
    }
    pub(crate) fn accel_len(&self, stride: usize) -> usize {
        if self.accels() {
            (self.max_accel.as_usize() - self.min_accel.as_usize() + stride) / stride
        } else {
            0
        }
    }
    #[inline]
    pub(crate) fn accels(&self) -> bool {
        self.min_accel != DEAD
    }
    #[inline]
    pub(crate) fn starts(&self) -> bool {
        self.min_start != DEAD
    }
}
