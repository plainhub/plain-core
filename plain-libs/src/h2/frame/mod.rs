use crate::h2::hpack;
use bytes::Bytes;
use std::fmt;
macro_rules! unpack_octets_4 {
    ($buf : expr , $offset : expr , $tip : ty) => {
        (($buf[$offset + 0] as $tip) << 24)
            | (($buf[$offset + 1] as $tip) << 16)
            | (($buf[$offset + 2] as $tip) << 8)
            | (($buf[$offset + 3] as $tip) << 0)
    };
}
mod data;
mod go_away;
mod head;
mod headers;
mod ping;
mod priority;
mod reason;
mod reset;
mod settings;
mod stream_id;
mod util;
mod window_update;
pub use self::data::Data;
pub use self::go_away::GoAway;
pub use self::head::{Head, Kind};
pub use self::headers::{
    Continuation, Headers, Pseudo, PushPromise, PushPromiseHeaderError, parse_u64,
};
pub use self::ping::Ping;
pub use self::priority::{Priority, StreamDependency};
pub use self::reason::Reason;
pub use self::reset::Reset;
pub use self::settings::Settings;
pub use self::settings::{
    DEFAULT_INITIAL_WINDOW_SIZE, DEFAULT_MAX_FRAME_SIZE, DEFAULT_SETTINGS_HEADER_TABLE_SIZE,
    MAX_MAX_FRAME_SIZE,
};
pub use self::stream_id::{StreamId, StreamIdOverflow};
pub use self::window_update::WindowUpdate;
pub type FrameSize = u32;
pub const HEADER_LEN: usize = 9;
#[derive(Eq, PartialEq)]
pub enum Frame<T = Bytes> {
    Data(Data<T>),
    Headers(Headers),
    Priority(Priority),
    PushPromise(PushPromise),
    Settings(Settings),
    Ping(Ping),
    GoAway(GoAway),
    WindowUpdate(WindowUpdate),
    Reset(Reset),
}
impl<T> Frame<T> {
    pub fn map<F, U>(self, f: F) -> Frame<U>
    where
        F: FnOnce(T) -> U,
    {
        use self::Frame::*;
        match self {
            Data(frame) => frame.map(f).into(),
            Headers(frame) => frame.into(),
            Priority(frame) => frame.into(),
            PushPromise(frame) => frame.into(),
            Settings(frame) => frame.into(),
            Ping(frame) => frame.into(),
            GoAway(frame) => frame.into(),
            WindowUpdate(frame) => frame.into(),
            Reset(frame) => frame.into(),
        }
    }
}
impl<T> fmt::Debug for Frame<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        use self::Frame::*;
        match *self {
            Data(ref frame) => fmt::Debug::fmt(frame, fmt),
            Headers(ref frame) => fmt::Debug::fmt(frame, fmt),
            Priority(ref frame) => fmt::Debug::fmt(frame, fmt),
            PushPromise(ref frame) => fmt::Debug::fmt(frame, fmt),
            Settings(ref frame) => fmt::Debug::fmt(frame, fmt),
            Ping(ref frame) => fmt::Debug::fmt(frame, fmt),
            GoAway(ref frame) => fmt::Debug::fmt(frame, fmt),
            WindowUpdate(ref frame) => fmt::Debug::fmt(frame, fmt),
            Reset(ref frame) => fmt::Debug::fmt(frame, fmt),
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    BadFrameSize,
    TooMuchPadding,
    InvalidSettingValue,
    InvalidWindowUpdateValue,
    InvalidPayloadLength,
    InvalidPayloadAckSettings,
    InvalidStreamId,
    MalformedMessage,
    HeaderListWayTooLarge,
    InvalidDependencyId,
    Hpack(hpack::DecoderError),
}
