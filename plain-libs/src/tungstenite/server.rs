pub use crate::tungstenite::handshake::server::ServerHandshake;
use crate::tungstenite::handshake::{
    HandshakeError,
    server::{Callback, NoCallback},
};
use crate::tungstenite::protocol::{WebSocket, WebSocketConfig};
use std::io::{Read, Write};
pub fn accept_with_config<S: Read + Write>(
    stream: S,
    config: Option<WebSocketConfig>,
) -> Result<WebSocket<S>, HandshakeError<ServerHandshake<S, NoCallback>>> {
    accept_hdr_with_config(stream, NoCallback, config)
}
pub fn accept<S: Read + Write>(
    stream: S,
) -> Result<WebSocket<S>, HandshakeError<ServerHandshake<S, NoCallback>>> {
    accept_with_config(stream, None)
}
pub fn accept_hdr_with_config<S: Read + Write, C: Callback>(
    stream: S,
    callback: C,
    config: Option<WebSocketConfig>,
) -> Result<WebSocket<S>, HandshakeError<ServerHandshake<S, C>>> {
    ServerHandshake::start(stream, callback, config).handshake()
}
pub fn accept_hdr<S: Read + Write, C: Callback>(
    stream: S,
    callback: C,
) -> Result<WebSocket<S>, HandshakeError<ServerHandshake<S, C>>> {
    accept_hdr_with_config(stream, callback, None)
}
