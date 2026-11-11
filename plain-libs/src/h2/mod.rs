macro_rules! proto_err { (conn : $($msg : tt) +) => { tracing::debug ! ("connection error PROTOCOL_ERROR -- {};" , format_args ! ($($msg) +)) } ; (stream : $($msg : tt) +) => { tracing::debug ! ("stream error PROTOCOL_ERROR -- {};" , format_args ! ($($msg) +)) } ; }
macro_rules! ready {
    ($e : expr) => {
        match $e {
            ::std::task::Poll::Ready(r) => r,
            ::std::task::Poll::Pending => return ::std::task::Poll::Pending,
        }
    };
}
pub mod client;
mod codec;
mod error;
pub mod ext;
mod frame;
mod hpack;
mod proto;
pub mod server;
mod share;
pub use crate::h2::error::{Error, Reason};
pub use crate::h2::share::{FlowControl, Ping, PingPong, Pong, RecvStream, SendStream, StreamId};
#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/h2.rs"]
mod tests;
