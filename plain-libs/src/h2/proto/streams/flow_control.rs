use crate::h2::frame::Reason;
use crate::h2::proto::{MAX_WINDOW_SIZE, WindowSize};
use std::fmt;
const UNCLAIMED_NUMERATOR: i32 = 1;
const UNCLAIMED_DENOMINATOR: i32 = 2;
#[derive(Copy, Clone, Debug)]
pub struct FlowControl {
    window_size: Window,
    available: Window,
}
impl FlowControl {
    pub fn new() -> FlowControl {
        FlowControl {
            window_size: Window(0),
            available: Window(0),
        }
    }
    pub fn window_size(&self) -> WindowSize {
        self.window_size.as_size()
    }
    pub fn available(&self) -> Window {
        self.available
    }
    pub fn has_unavailable(&self) -> bool {
        if self.window_size < 0 {
            return false;
        }
        self.window_size > self.available
    }
    pub fn claim_capacity(&mut self, capacity: WindowSize) -> Result<(), Reason> {
        self.available.decrease_by(capacity)
    }
    pub fn assign_capacity(&mut self, capacity: WindowSize) -> Result<(), Reason> {
        self.available.increase_by(capacity)
    }
    pub fn unclaimed_capacity(&self) -> Option<WindowSize> {
        let available = self.available;
        if self.window_size >= available {
            return None;
        }
        let unclaimed = available.0 - self.window_size.0;
        let threshold = self.window_size.0 / UNCLAIMED_DENOMINATOR * UNCLAIMED_NUMERATOR;
        if unclaimed < threshold {
            None
        } else {
            Some(unclaimed as WindowSize)
        }
    }
    pub fn inc_window(&mut self, sz: WindowSize) -> Result<(), Reason> {
        let (val, overflow) = self.window_size.0.overflowing_add(sz as i32);
        if overflow {
            return Err(Reason::FLOW_CONTROL_ERROR);
        }
        if val > MAX_WINDOW_SIZE as i32 {
            return Err(Reason::FLOW_CONTROL_ERROR);
        }
        tracing::trace!(
            "inc_window; sz={}; old={}; new={}",
            sz,
            self.window_size,
            val
        );
        self.window_size = Window(val);
        Ok(())
    }
    pub fn dec_send_window(&mut self, sz: WindowSize) -> Result<(), Reason> {
        tracing::trace!(
            "dec_window; sz={}; window={}, available={}",
            sz,
            self.window_size,
            self.available
        );
        self.window_size.decrease_by(sz)?;
        Ok(())
    }
    pub fn dec_recv_window(&mut self, sz: WindowSize) -> Result<(), Reason> {
        tracing::trace!(
            "dec_recv_window; sz={}; window={}, available={}",
            sz,
            self.window_size,
            self.available
        );
        self.window_size.decrease_by(sz)?;
        self.available.decrease_by(sz)?;
        Ok(())
    }
    pub fn send_data(&mut self, sz: WindowSize) -> Result<(), Reason> {
        tracing::trace!(
            "send_data; sz={}; window={}; available={}",
            sz,
            self.window_size,
            self.available
        );
        if sz > 0 {
            assert!(self.window_size.0 >= sz as i32);
            self.window_size.decrease_by(sz)?;
            self.available.decrease_by(sz)?;
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd)]
pub struct Window(i32);
impl Window {
    pub fn as_size(&self) -> WindowSize {
        if self.0 < 0 { 0 } else { self.0 as WindowSize }
    }
    pub fn checked_size(&self) -> WindowSize {
        assert!(self.0 >= 0, "negative Window");
        self.0 as WindowSize
    }
    pub fn decrease_by(&mut self, other: WindowSize) -> Result<(), Reason> {
        if let Some(v) = self.0.checked_sub(other as i32) {
            self.0 = v;
            Ok(())
        } else {
            Err(Reason::FLOW_CONTROL_ERROR)
        }
    }
    pub fn increase_by(&mut self, other: WindowSize) -> Result<(), Reason> {
        let other = self.add(other)?;
        self.0 = other.0;
        Ok(())
    }
    pub fn add(&self, other: WindowSize) -> Result<Self, Reason> {
        if let Some(v) = self.0.checked_add(other as i32) {
            Ok(Self(v))
        } else {
            Err(Reason::FLOW_CONTROL_ERROR)
        }
    }
}
impl PartialEq<usize> for Window {
    fn eq(&self, other: &usize) -> bool {
        if self.0 < 0 {
            false
        } else {
            (self.0 as usize).eq(other)
        }
    }
}
impl PartialOrd<usize> for Window {
    fn partial_cmp(&self, other: &usize) -> Option<::std::cmp::Ordering> {
        if self.0 < 0 {
            Some(::std::cmp::Ordering::Less)
        } else {
            (self.0 as usize).partial_cmp(other)
        }
    }
}
impl fmt::Display for Window {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}
impl From<Window> for isize {
    fn from(w: Window) -> isize {
        w.0 as isize
    }
}
