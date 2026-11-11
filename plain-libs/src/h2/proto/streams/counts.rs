use super::*;
#[derive(Debug)]
struct Budget {
    available: usize,
    max: usize,
}
#[derive(Debug)]
pub(super) struct BudgetExhausted;
impl Budget {
    fn new(max: usize) -> Self {
        Budget {
            available: max,
            max,
        }
    }
    fn consume(&mut self, amount: usize) -> Result<(), BudgetExhausted> {
        self.available = self.available.checked_sub(amount).ok_or(BudgetExhausted)?;
        Ok(())
    }
    fn replenish(&mut self, amount: usize) {
        self.available = self.available.saturating_add(amount).min(self.max);
    }
}
#[derive(Debug)]
pub(super) struct Counts {
    peer: peer::Dyn,
    max_send_streams: usize,
    num_send_streams: usize,
    max_recv_streams: usize,
    num_recv_streams: usize,
    max_local_reset_streams: usize,
    num_local_reset_streams: usize,
    max_remote_reset_streams: usize,
    num_remote_reset_streams: usize,
    max_local_error_reset_streams: Option<usize>,
    num_local_error_reset_streams: usize,
    data_frame_budget: Budget,
    num_recv_empty_data_frames: usize,
}
impl Counts {
    pub fn new(peer: peer::Dyn, config: &Config) -> Self {
        Counts {
            peer,
            max_send_streams: config.initial_max_send_streams,
            num_send_streams: 0,
            max_recv_streams: config.remote_max_initiated.unwrap_or(usize::MAX),
            num_recv_streams: 0,
            max_local_reset_streams: config.local_reset_max,
            num_local_reset_streams: 0,
            max_remote_reset_streams: config.remote_reset_max,
            num_remote_reset_streams: 0,
            max_local_error_reset_streams: config.local_max_error_reset_streams,
            num_local_error_reset_streams: 0,
            data_frame_budget: Budget::new(config.data_frame_budget),
            num_recv_empty_data_frames: 0,
        }
    }
    pub fn record_data_frame(&mut self, payload_len: usize) -> Result<(), BudgetExhausted> {
        if payload_len == 0 {
            self.num_recv_empty_data_frames = self
                .num_recv_empty_data_frames
                .checked_add(1)
                .ok_or(BudgetExhausted)?;
            if self.num_recv_empty_data_frames > MAX_RECV_EMPTY_DATA_FRAMES {
                return Err(BudgetExhausted);
            }
            Ok(())
        } else if payload_len < DEFAULT_DATA_FRAME_OVERHEAD_THRESHOLD {
            self.data_frame_budget
                .consume(DEFAULT_DATA_FRAME_OVERHEAD_THRESHOLD - payload_len)
        } else {
            self.data_frame_budget
                .replenish(payload_len - DEFAULT_DATA_FRAME_OVERHEAD_THRESHOLD);
            Ok(())
        }
    }
    pub fn release_data_frame(&mut self, payload_len: usize) {
        if payload_len != 0 && payload_len < DEFAULT_DATA_FRAME_OVERHEAD_THRESHOLD {
            self.data_frame_budget
                .replenish(DEFAULT_DATA_FRAME_OVERHEAD_THRESHOLD - payload_len);
        }
    }
    pub fn next_send_stream_will_reach_capacity(&self) -> bool {
        self.max_send_streams <= (self.num_send_streams + 1)
    }
    pub fn peer(&self) -> peer::Dyn {
        self.peer
    }
    pub fn has_streams(&self) -> bool {
        self.num_send_streams != 0 || self.num_recv_streams != 0
    }
    pub fn can_inc_num_local_error_resets(&self) -> bool {
        if let Some(max) = self.max_local_error_reset_streams {
            max > self.num_local_error_reset_streams
        } else {
            true
        }
    }
    pub fn inc_num_local_error_resets(&mut self) {
        assert!(self.can_inc_num_local_error_resets());
        self.num_local_error_reset_streams += 1;
    }
    pub(crate) fn max_local_error_resets(&self) -> Option<usize> {
        self.max_local_error_reset_streams
    }
    pub fn can_inc_num_recv_streams(&self) -> bool {
        self.max_recv_streams > self.num_recv_streams
    }
    pub fn inc_num_recv_streams(&mut self, stream: &mut store::Ptr) {
        assert!(self.can_inc_num_recv_streams());
        assert!(!stream.is_counted);
        self.num_recv_streams += 1;
        stream.is_counted = true;
    }
    pub fn can_inc_num_send_streams(&self) -> bool {
        self.max_send_streams > self.num_send_streams
    }
    pub fn inc_num_send_streams(&mut self, stream: &mut store::Ptr) {
        assert!(self.can_inc_num_send_streams());
        assert!(!stream.is_counted);
        self.num_send_streams += 1;
        stream.is_counted = true;
    }
    pub fn can_inc_num_reset_streams(&self) -> bool {
        self.max_local_reset_streams > self.num_local_reset_streams
    }
    pub fn inc_num_reset_streams(&mut self) {
        assert!(self.can_inc_num_reset_streams());
        self.num_local_reset_streams += 1;
    }
    pub(crate) fn max_remote_reset_streams(&self) -> usize {
        self.max_remote_reset_streams
    }
    pub(crate) fn can_inc_num_remote_reset_streams(&self) -> bool {
        self.max_remote_reset_streams > self.num_remote_reset_streams
    }
    pub(crate) fn inc_num_remote_reset_streams(&mut self) {
        assert!(self.can_inc_num_remote_reset_streams());
        self.num_remote_reset_streams += 1;
    }
    pub(crate) fn dec_num_remote_reset_streams(&mut self) {
        assert!(self.num_remote_reset_streams > 0);
        self.num_remote_reset_streams -= 1;
    }
    pub fn apply_remote_settings(&mut self, settings: &frame::Settings, is_initial: bool) {
        match settings.max_concurrent_streams() {
            Some(val) => self.max_send_streams = val as usize,
            None if is_initial => self.max_send_streams = usize::MAX,
            None => {}
        }
    }
    pub fn transition<F, U>(&mut self, mut stream: store::Ptr, f: F) -> U
    where
        F: FnOnce(&mut Self, &mut store::Ptr) -> U,
    {
        let is_pending_reset = stream.is_pending_reset_expiration();
        let ret = f(self, &mut stream);
        self.transition_after(stream, is_pending_reset);
        ret
    }
    pub fn transition_after(&mut self, mut stream: store::Ptr, is_reset_counted: bool) {
        tracing::trace!(
            "transition_after; stream={:?}; state={:?}; is_closed={:?}; \
             pending_send_empty={:?}; buffered_send_data={}; \
             num_recv={}; num_send={}",
            stream.id,
            stream.state,
            stream.is_closed(),
            stream.pending_send.is_empty(),
            stream.buffered_send_data,
            self.num_recv_streams,
            self.num_send_streams
        );
        if stream.is_closed() {
            if !stream.is_pending_reset_expiration() {
                stream.unlink();
                if is_reset_counted {
                    self.dec_num_reset_streams();
                }
            }
            if !stream.state.is_scheduled_reset() && stream.is_counted {
                tracing::trace!("dec_num_streams; stream={:?}", stream.id);
                self.dec_num_streams(&mut stream);
            }
        }
        if stream.is_released() {
            stream.remove();
        }
    }
    pub(crate) fn max_send_streams(&self) -> usize {
        self.max_send_streams
    }
    pub(crate) fn max_recv_streams(&self) -> usize {
        self.max_recv_streams
    }
    fn dec_num_streams(&mut self, stream: &mut store::Ptr) {
        assert!(stream.is_counted);
        if self.peer.is_local_init(stream.id) {
            assert!(self.num_send_streams > 0);
            self.num_send_streams -= 1;
            stream.is_counted = false;
        } else {
            assert!(self.num_recv_streams > 0);
            self.num_recv_streams -= 1;
            stream.is_counted = false;
        }
    }
    fn dec_num_reset_streams(&mut self) {
        assert!(self.num_local_reset_streams > 0);
        self.num_local_reset_streams -= 1;
    }
}
impl Drop for Counts {
    fn drop(&mut self) {
        use std::thread;
        if !thread::panicking() {
            debug_assert!(!self.has_streams());
        }
    }
}
