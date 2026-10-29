use super::super::u32_at;

pub(super) struct Sizes {
    pub count: u32,
    pub uniform: u32,
    pub entries: Vec<u32>,
}

impl Sizes {
    pub fn read(data: &[u8]) -> Option<Self> {
        if u32_at(data, 0)? != 0 {
            return None;
        }
        let uniform = u32_at(data, 4)?;
        let count = u32_at(data, 8)?;
        let data = data.get(12..)?;
        let entries = if uniform == 0 {
            if usize::try_from(count).ok()?.checked_mul(4)? != data.len() {
                return None;
            }
            data.chunks_exact(4)
                .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
                .collect()
        } else {
            if !data.is_empty() {
                return None;
            }
            Vec::new()
        };
        Some(Self {
            count,
            uniform,
            entries,
        })
    }

    pub fn get(&self, index: u32) -> Option<u32> {
        if index >= self.count {
            return None;
        }
        if self.uniform != 0 {
            Some(self.uniform)
        } else {
            self.entries.get(index as usize).copied()
        }
    }

    pub fn sum(&self, start: u32, count: u32) -> Option<u64> {
        let end = start.checked_add(count)?;
        if end > self.count {
            return None;
        }
        if self.uniform != 0 {
            return u64::from(count).checked_mul(u64::from(self.uniform));
        }
        self.entries
            .get(start as usize..end as usize)?
            .iter()
            .try_fold(0u64, |sum, size| sum.checked_add(u64::from(*size)))
    }
}
