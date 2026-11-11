pub use crate::http;
pub mod buffer;
pub mod client;
pub mod error;
pub mod handshake;
pub mod protocol;
mod server;
pub mod stream;

mod utf8;
pub mod util;
const READ_BUFFER_CHUNK_SIZE: usize = 4096;
type ReadBuffer = buffer::ReadBuffer<READ_BUFFER_CHUNK_SIZE>;
pub use crate::tungstenite::{
    client::{ClientRequestBuilder, client},
    handshake::{HandshakeError, client::ClientHandshake, server::ServerHandshake},
    server::{accept, accept_hdr, accept_hdr_with_config, accept_with_config},
};
pub use crate::tungstenite::{
    error::{Error, Result},
    protocol::{Message, WebSocket, frame::Utf8Bytes},
};
pub use bytes::Bytes;

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/tungstenite.rs"]
mod tests;
