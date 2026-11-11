pub use self::incoming::Incoming;
pub(crate) use self::incoming::Sender;
pub(crate) use self::length::DecodedLength;
pub use crate::http_body::Body;
pub use crate::http_body::Frame;
pub use crate::http_body::SizeHint;
pub use bytes::{Buf, Bytes};
mod incoming;
mod length;
fn _assert_send_sync() {
    fn _assert_send<T: Send>() {}
    fn _assert_sync<T: Sync>() {}
    _assert_send::<Incoming>();
    _assert_sync::<Incoming>();
}
