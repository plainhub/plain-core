use super::sizes::Sizes;

pub(super) struct Run {
    pub first: u32,
    pub time: u64,
    pub description: u32,
    pub offset: u64,
    pub sizes: Sizes,
    pub(super) durations: Vec<u32>,
    pub(super) default_duration: u32,
    pub(super) flags: Vec<u32>,
    pub(super) default_flags: u32,
    pub(super) first_flags: Option<u32>,
}

impl Run {
    pub fn end_time(&self) -> Option<u64> {
        let ticks = if self.durations.is_empty() {
            u64::from(self.default_duration).checked_mul(u64::from(self.sizes.count))?
        } else {
            self.durations
                .iter()
                .try_fold(0u64, |sum, d| sum.checked_add(u64::from(*d)))?
        };
        self.time.checked_add(ticks)
    }
    pub fn candidate(&self, target: u64) -> u32 {
        if self.durations.is_empty() {
            return ((target.saturating_sub(self.time) / u64::from(self.default_duration.max(1)))
                .min(u64::from(self.sizes.count.saturating_sub(1)))) as u32;
        }
        let mut time = self.time;
        for (index, duration) in self.durations.iter().enumerate() {
            let next = time.saturating_add(u64::from((*duration).max(1)));
            if target < next {
                return index as u32;
            }
            time = next;
        }
        self.sizes.count.saturating_sub(1)
    }
    fn sync(&self, index: u32) -> bool {
        let flags = self.flags.get(index as usize).copied().unwrap_or_else(|| {
            if index == 0 {
                self.first_flags.unwrap_or(self.default_flags)
            } else {
                self.default_flags
            }
        });
        flags & 0x10000 == 0
    }
    pub fn sync_before(&self, limit: u32) -> Option<u32> {
        if self.sizes.count == 0 {
            return None;
        }
        let limit = limit.min(self.sizes.count - 1);
        if !self.flags.is_empty() {
            return (0..=limit).rev().find(|index| self.sync(*index));
        }
        if limit > 0 && self.default_flags & 0x10000 == 0 {
            return Some(limit);
        }
        self.sync(0).then_some(0)
    }
    pub fn first_sync(&self) -> Option<u32> {
        if !self.flags.is_empty() {
            return (0..self.sizes.count).find(|index| self.sync(*index));
        }
        if self.sizes.count > 0 && self.sync(0) {
            return Some(0);
        }
        (self.sizes.count > 1 && self.default_flags & 0x10000 == 0).then_some(1)
    }
}
