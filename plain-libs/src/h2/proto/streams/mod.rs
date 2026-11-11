mod buffer;
mod counts;
mod flow_control;
mod prioritize;
mod recv;
mod send;
mod state;
mod store;
mod stream;
mod streams;
use self::buffer::Buffer;
use self::counts::Counts;
use self::flow_control::FlowControl;
use self::prioritize::Prioritize;
pub(crate) use self::prioritize::Prioritized;
pub(crate) use self::recv::Open;
use self::recv::Recv;
pub(crate) use self::send::PollReset;
use self::send::Send;
use self::state::State;
use self::store::Store;
use self::stream::Stream;
pub(crate) use self::streams::{DynStreams, OpaqueStreamRef, StreamRef, Streams};
use crate::h2::frame::{StreamId, StreamIdOverflow};
use crate::h2::proto::*;
use bytes::Bytes;
use std::time::Duration;
#[derive(Debug, Eq, PartialEq)]
pub(super) enum BufferStatus {
    Complete,
    CodecFull,
}
#[derive(Debug)]
pub struct Config {
    pub initial_max_send_streams: usize,
    pub local_max_buffer_size: usize,
    pub local_next_stream_id: StreamId,
    pub local_push_enabled: bool,
    pub extended_connect_protocol_enabled: bool,
    pub local_reset_duration: Duration,
    pub local_reset_max: usize,
    pub remote_reset_max: usize,
    pub remote_init_window_sz: WindowSize,
    pub remote_max_initiated: Option<usize>,
    pub local_max_error_reset_streams: Option<usize>,
    pub data_frame_budget: usize,
}
trait DebugStructExt<'a, 'b> {
    fn h2_field_if(&mut self, name: &str, val: &bool) -> &mut std::fmt::DebugStruct<'a, 'b>;
    fn h2_field_if_then<T: std::fmt::Debug>(
        &mut self,
        name: &str,
        cond: bool,
        val: &T,
    ) -> &mut std::fmt::DebugStruct<'a, 'b>;
    fn h2_field_some<T: std::fmt::Debug>(
        &mut self,
        name: &str,
        val: &Option<T>,
    ) -> &mut std::fmt::DebugStruct<'a, 'b>;
}
impl<'a, 'b> DebugStructExt<'a, 'b> for std::fmt::DebugStruct<'a, 'b> {
    fn h2_field_if(&mut self, name: &str, val: &bool) -> &mut std::fmt::DebugStruct<'a, 'b> {
        if *val { self.field(name, val) } else { self }
    }
    fn h2_field_if_then<T: std::fmt::Debug>(
        &mut self,
        name: &str,
        cond: bool,
        val: &T,
    ) -> &mut std::fmt::DebugStruct<'a, 'b> {
        if cond { self.field(name, val) } else { self }
    }
    fn h2_field_some<T: std::fmt::Debug>(
        &mut self,
        name: &str,
        val: &Option<T>,
    ) -> &mut std::fmt::DebugStruct<'a, 'b> {
        if val.is_some() {
            self.field(name, val)
        } else {
            self
        }
    }
}
