mod combine;
use core::fmt;
mod baseline;
mod specialized;
mod table;
pub fn hash(buf: &[u8]) -> u32 {
    let mut h = Hasher::new();
    h.update(buf);
    h.finalize()
}
#[derive(Clone)]
enum State {
    Baseline(baseline::State),
    Specialized(specialized::State),
}
#[derive(Clone)]
pub struct Hasher {
    amount: u32,
    state: State,
}
const DEFAULT_INIT_STATE: u32 = 0;
impl Hasher {
    pub fn new() -> Self {
        Self::new_with_initial(DEFAULT_INIT_STATE)
    }
    pub fn new_with_initial(init: u32) -> Self {
        Self {
            amount: 0,
            state: specialized::State::new(init)
                .map(State::Specialized)
                .unwrap_or_else(|| State::Baseline(baseline::State::new(init))),
        }
    }
    pub fn update(&mut self, buf: &[u8]) {
        self.amount = self.amount.wrapping_add(buf.len() as u32);
        match self.state {
            State::Baseline(ref mut state) => state.update(buf),
            State::Specialized(ref mut state) => state.update(buf),
        }
    }
    pub fn finalize(self) -> u32 {
        match self.state {
            State::Baseline(state) => state.finalize(),
            State::Specialized(state) => state.finalize(),
        }
    }
    pub fn amount(&self) -> u32 {
        self.amount
    }
    pub fn sum(&self) -> u32 {
        self.clone().finalize()
    }
    pub fn reset(&mut self) {
        self.amount = 0;
        match self.state {
            State::Baseline(ref mut state) => state.reset(),
            State::Specialized(ref mut state) => state.reset(),
        }
    }
}
impl fmt::Debug for Hasher {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("crc32fast::Hasher").finish()
    }
}
impl Default for Hasher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/checksum/crc32.rs"]
mod tests;
