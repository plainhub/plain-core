pub mod frame;
mod message;
pub use self::{frame::CloseFrame, message::Message};
use self::{
    frame::{
        Frame, FrameCodec,
        coding::{CloseCode, Control as OpCtl, Data as OpData, OpCode},
    },
    message::{IncompleteMessage, MessageType},
};
use crate::tungstenite::{
    error::{CapacityError, Error, ProtocolError, Result},
    protocol::frame::Utf8Bytes,
};
use log::*;
use std::{
    io::{self, Read, Write},
    mem::replace,
};
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Server,
    Client,
}
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct WebSocketConfig {
    pub read_buffer_size: usize,
    pub write_buffer_size: usize,
    pub max_write_buffer_size: usize,
    pub max_message_size: Option<usize>,
    pub max_frame_size: Option<usize>,
    pub accept_unmasked_frames: bool,
}
impl Default for WebSocketConfig {
    fn default() -> Self {
        Self {
            read_buffer_size: 128 * 1024,
            write_buffer_size: 128 * 1024,
            max_write_buffer_size: usize::MAX,
            max_message_size: Some(64 << 20),
            max_frame_size: Some(16 << 20),
            accept_unmasked_frames: false,
        }
    }
}
impl WebSocketConfig {
    pub fn read_buffer_size(mut self, read_buffer_size: usize) -> Self {
        self.read_buffer_size = read_buffer_size;
        self
    }
    pub fn write_buffer_size(mut self, write_buffer_size: usize) -> Self {
        self.write_buffer_size = write_buffer_size;
        self
    }
    pub fn max_write_buffer_size(mut self, max_write_buffer_size: usize) -> Self {
        self.max_write_buffer_size = max_write_buffer_size;
        self
    }
    pub fn max_message_size(mut self, max_message_size: Option<usize>) -> Self {
        self.max_message_size = max_message_size;
        self
    }
    pub fn max_frame_size(mut self, max_frame_size: Option<usize>) -> Self {
        self.max_frame_size = max_frame_size;
        self
    }
    pub fn accept_unmasked_frames(mut self, accept_unmasked_frames: bool) -> Self {
        self.accept_unmasked_frames = accept_unmasked_frames;
        self
    }
    pub(crate) fn assert_valid(&self) {
        assert!(
            self.max_write_buffer_size > self.write_buffer_size,
            "WebSocketConfig::max_write_buffer_size must be greater than write_buffer_size, \
            see WebSocketConfig docs`"
        );
    }
}
#[derive(Debug)]
pub struct WebSocket<Stream> {
    socket: Stream,
    context: WebSocketContext,
}
impl<Stream> WebSocket<Stream> {
    pub fn from_raw_socket(stream: Stream, role: Role, config: Option<WebSocketConfig>) -> Self {
        WebSocket {
            socket: stream,
            context: WebSocketContext::new(role, config),
        }
    }
    pub fn from_partially_read(
        stream: Stream,
        part: Vec<u8>,
        role: Role,
        config: Option<WebSocketConfig>,
    ) -> Self {
        WebSocket {
            socket: stream,
            context: WebSocketContext::from_partially_read(part, role, config),
        }
    }
    pub fn into_inner(self) -> Stream {
        self.socket
    }
    pub fn get_ref(&self) -> &Stream {
        &self.socket
    }
    pub fn get_mut(&mut self) -> &mut Stream {
        &mut self.socket
    }
    pub fn set_config(&mut self, set_func: impl FnOnce(&mut WebSocketConfig)) {
        self.context.set_config(set_func);
    }
    pub fn get_config(&self) -> &WebSocketConfig {
        self.context.get_config()
    }
    pub fn can_read(&self) -> bool {
        self.context.can_read()
    }
    pub fn can_write(&self) -> bool {
        self.context.can_write()
    }
}
impl<Stream: Read + Write> WebSocket<Stream> {
    pub fn read(&mut self) -> Result<Message> {
        self.context.read(&mut self.socket)
    }
    pub fn send(&mut self, message: Message) -> Result<()> {
        self.write(message)?;
        self.flush()
    }
    pub fn write(&mut self, message: Message) -> Result<()> {
        self.context.write(&mut self.socket, message)
    }
    pub fn flush(&mut self) -> Result<()> {
        self.context.flush(&mut self.socket)
    }
    pub fn close(&mut self, code: Option<CloseFrame>) -> Result<()> {
        self.context.close(&mut self.socket, code)
    }
}
#[derive(Debug)]
pub struct WebSocketContext {
    role: Role,
    frame: FrameCodec,
    state: WebSocketState,
    incomplete: Option<IncompleteMessage>,
    additional_send: Option<Frame>,
    unflushed_additional: bool,
    config: WebSocketConfig,
}
impl WebSocketContext {
    pub fn new(role: Role, config: Option<WebSocketConfig>) -> Self {
        let conf = config.unwrap_or_default();
        Self::_new(role, FrameCodec::new(conf.read_buffer_size), conf)
    }
    pub fn from_partially_read(part: Vec<u8>, role: Role, config: Option<WebSocketConfig>) -> Self {
        let conf = config.unwrap_or_default();
        Self::_new(
            role,
            FrameCodec::from_partially_read(part, conf.read_buffer_size),
            conf,
        )
    }
    fn _new(role: Role, mut frame: FrameCodec, config: WebSocketConfig) -> Self {
        config.assert_valid();
        frame.set_max_out_buffer_len(config.max_write_buffer_size);
        frame.set_out_buffer_write_len(config.write_buffer_size);
        Self {
            role,
            frame,
            state: WebSocketState::Active,
            incomplete: None,
            additional_send: None,
            unflushed_additional: false,
            config,
        }
    }
    pub fn set_config(&mut self, set_func: impl FnOnce(&mut WebSocketConfig)) {
        set_func(&mut self.config);
        self.config.assert_valid();
        self.frame
            .set_max_out_buffer_len(self.config.max_write_buffer_size);
        self.frame
            .set_out_buffer_write_len(self.config.write_buffer_size);
    }
    pub fn get_config(&self) -> &WebSocketConfig {
        &self.config
    }
    pub fn can_read(&self) -> bool {
        self.state.can_read()
    }
    pub fn can_write(&self) -> bool {
        self.state.is_active()
    }
    pub fn read<Stream>(&mut self, stream: &mut Stream) -> Result<Message>
    where
        Stream: Read + Write,
    {
        self.state.check_not_terminated()?;
        loop {
            if self.additional_send.is_some() || self.unflushed_additional {
                match self.flush(stream) {
                    Ok(_) => {}
                    Err(Error::Io(err)) if err.kind() == io::ErrorKind::WouldBlock => {
                        self.unflushed_additional = true;
                    }
                    Err(err) => return Err(err),
                }
            } else if self.role == Role::Server && !self.state.can_read() {
                self.state = WebSocketState::Terminated;
                return Err(Error::ConnectionClosed);
            }
            if let Some(message) = self.read_message_frame(stream)? {
                trace!("Received message {message}");
                return Ok(message);
            }
        }
    }
    pub fn write<Stream>(&mut self, stream: &mut Stream, message: Message) -> Result<()>
    where
        Stream: Read + Write,
    {
        self.state.check_not_terminated()?;
        if !self.state.is_active() {
            return Err(Error::Protocol(ProtocolError::SendAfterClosing));
        }
        let frame = match message {
            Message::Text(data) => Frame::message(data, OpCode::Data(OpData::Text), true),
            Message::Binary(data) => Frame::message(data, OpCode::Data(OpData::Binary), true),
            Message::Ping(data) => Frame::ping(data),
            Message::Pong(data) => {
                self.set_additional(Frame::pong(data));
                return self._write(stream, None).map(|_| ());
            }
            Message::Close(code) => return self.close(stream, code),
            Message::Frame(f) => f,
        };
        let should_flush = self._write(stream, Some(frame))?;
        if should_flush {
            self.flush(stream)?;
        }
        Ok(())
    }
    #[inline]
    pub fn flush<Stream>(&mut self, stream: &mut Stream) -> Result<()>
    where
        Stream: Read + Write,
    {
        self._write(stream, None)?;
        self.frame.write_out_buffer(stream)?;
        stream.flush()?;
        self.unflushed_additional = false;
        Ok(())
    }
    fn _write<Stream>(&mut self, stream: &mut Stream, data: Option<Frame>) -> Result<bool>
    where
        Stream: Read + Write,
    {
        if let Some(data) = data {
            self.buffer_frame(stream, data)?;
        }
        let should_flush = if let Some(msg) = self.additional_send.take() {
            trace!("Sending pong/close");
            match self.buffer_frame(stream, msg) {
                Err(Error::WriteBufferFull(msg)) => {
                    if let Message::Frame(msg) = *msg {
                        self.set_additional(msg);
                        false
                    } else {
                        unreachable!()
                    }
                }
                Err(err) => return Err(err),
                Ok(_) => true,
            }
        } else {
            self.unflushed_additional
        };
        if self.role == Role::Server && !self.state.can_read() {
            self.frame.write_out_buffer(stream)?;
            self.state = WebSocketState::Terminated;
            Err(Error::ConnectionClosed)
        } else {
            Ok(should_flush)
        }
    }
    pub fn close<Stream>(&mut self, stream: &mut Stream, code: Option<CloseFrame>) -> Result<()>
    where
        Stream: Read + Write,
    {
        if let WebSocketState::Active = self.state {
            self.state = WebSocketState::ClosedByUs;
            let frame = Frame::close(code);
            self._write(stream, Some(frame))?;
        }
        self.flush(stream)
    }
    fn read_message_frame(&mut self, stream: &mut impl Read) -> Result<Option<Message>> {
        let frame = match self
            .frame
            .read_frame(
                stream,
                self.config.max_frame_size,
                matches!(self.role, Role::Server),
                self.config.accept_unmasked_frames,
            )
            .check_connection_reset(self.state)?
        {
            None => {
                return match replace(&mut self.state, WebSocketState::Terminated) {
                    WebSocketState::ClosedByPeer | WebSocketState::CloseAcknowledged => {
                        Err(Error::ConnectionClosed)
                    }
                    _ => Err(Error::Protocol(ProtocolError::ResetWithoutClosingHandshake)),
                };
            }
            Some(frame) => frame,
        };
        if !self.state.can_read() {
            return Err(Error::Protocol(ProtocolError::ReceivedAfterClosing));
        }
        {
            let hdr = frame.header();
            if hdr.rsv1 || hdr.rsv2 || hdr.rsv3 {
                return Err(Error::Protocol(ProtocolError::NonZeroReservedBits));
            }
        }
        if self.role == Role::Client && frame.is_masked() {
            return Err(Error::Protocol(ProtocolError::MaskedFrameFromServer));
        }
        match frame.header().opcode {
            OpCode::Control(ctl) => match ctl {
                _ if !frame.header().is_final => {
                    Err(Error::Protocol(ProtocolError::FragmentedControlFrame))
                }
                _ if frame.payload().len() > 125 => {
                    Err(Error::Protocol(ProtocolError::ControlFrameTooBig))
                }
                OpCtl::Close => Ok(self.do_close(frame.into_close()?).map(Message::Close)),
                OpCtl::Reserved(i) => {
                    Err(Error::Protocol(ProtocolError::UnknownControlFrameType(i)))
                }
                OpCtl::Ping => {
                    let data = frame.into_payload();
                    if self.state.is_active() {
                        self.set_additional(Frame::pong(data.clone()));
                    }
                    Ok(Some(Message::Ping(data)))
                }
                OpCtl::Pong => Ok(Some(Message::Pong(frame.into_payload()))),
            },
            OpCode::Data(data) => {
                let fin = frame.header().is_final;
                let payload = match (data, self.incomplete.as_mut()) {
                    (OpData::Continue, None) => Err(ProtocolError::UnexpectedContinueFrame),
                    (OpData::Continue, Some(incomplete)) => {
                        incomplete.extend(frame.into_payload(), self.config.max_message_size)?;
                        Ok(None)
                    }
                    (_, Some(_)) => Err(ProtocolError::ExpectedFragment(data)),
                    (OpData::Text, _) => Ok(Some((frame.into_payload(), MessageType::Text))),
                    (OpData::Binary, _) => Ok(Some((frame.into_payload(), MessageType::Binary))),
                    (OpData::Reserved(i), _) => Err(ProtocolError::UnknownDataFrameType(i)),
                }?;
                match (payload, fin) {
                    (None, true) => Ok(Some(self.incomplete.take().unwrap().complete()?)),
                    (None, false) => Ok(None),
                    (Some((payload, t)), true) => {
                        check_max_size(payload.len(), self.config.max_message_size)?;
                        match t {
                            MessageType::Text => Ok(Some(Message::Text(payload.try_into()?))),
                            MessageType::Binary => Ok(Some(Message::Binary(payload))),
                        }
                    }
                    (Some((payload, t)), false) => {
                        let mut incomplete = IncompleteMessage::new(t);
                        incomplete.extend(payload, self.config.max_message_size)?;
                        self.incomplete = Some(incomplete);
                        Ok(None)
                    }
                }
            }
        }
    }
    fn do_close(&mut self, close: Option<CloseFrame>) -> Option<Option<CloseFrame>> {
        debug!("Received close frame: {close:?}");
        match self.state {
            WebSocketState::Active => {
                self.state = WebSocketState::ClosedByPeer;
                let close = close.map(|frame| {
                    if !frame.code.is_allowed() {
                        CloseFrame {
                            code: CloseCode::Protocol,
                            reason: Utf8Bytes::from_static("Protocol violation"),
                        }
                    } else {
                        frame
                    }
                });
                let reply = Frame::close(close.clone());
                debug!("Replying to close with {reply:?}");
                self.set_additional(reply);
                Some(close)
            }
            WebSocketState::ClosedByPeer | WebSocketState::CloseAcknowledged => None,
            WebSocketState::ClosedByUs => {
                self.state = WebSocketState::CloseAcknowledged;
                Some(close)
            }
            WebSocketState::Terminated => unreachable!(),
        }
    }
    fn buffer_frame<Stream>(&mut self, stream: &mut Stream, mut frame: Frame) -> Result<()>
    where
        Stream: Read + Write,
    {
        match self.role {
            Role::Server => {}
            Role::Client => {
                frame.set_random_mask();
            }
        }
        trace!("Sending frame: {frame:?}");
        self.frame
            .buffer_frame(stream, frame)
            .check_connection_reset(self.state)
    }
    fn set_additional(&mut self, add: Frame) {
        let empty_or_pong = self
            .additional_send
            .as_ref()
            .is_none_or(|f| f.header().opcode == OpCode::Control(OpCtl::Pong));
        if empty_or_pong {
            self.additional_send.replace(add);
        }
    }
}
fn check_max_size(size: usize, max_size: Option<usize>) -> crate::tungstenite::Result<()> {
    if let Some(max_size) = max_size {
        if size > max_size {
            return Err(Error::Capacity(CapacityError::MessageTooLong {
                size,
                max_size,
            }));
        }
    }
    Ok(())
}
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum WebSocketState {
    Active,
    ClosedByUs,
    ClosedByPeer,
    CloseAcknowledged,
    Terminated,
}
impl WebSocketState {
    fn is_active(self) -> bool {
        matches!(self, WebSocketState::Active)
    }
    fn can_read(self) -> bool {
        matches!(self, WebSocketState::Active | WebSocketState::ClosedByUs)
    }
    fn check_not_terminated(self) -> Result<()> {
        match self {
            WebSocketState::Terminated => Err(Error::AlreadyClosed),
            _ => Ok(()),
        }
    }
}
trait CheckConnectionReset {
    fn check_connection_reset(self, state: WebSocketState) -> Self;
}
impl<T> CheckConnectionReset for Result<T> {
    fn check_connection_reset(self, state: WebSocketState) -> Self {
        match self {
            Err(Error::Io(io_error)) => Err({
                if !state.can_read() && io_error.kind() == io::ErrorKind::ConnectionReset {
                    Error::ConnectionClosed
                } else {
                    Error::Io(io_error)
                }
            }),
            x => x,
        }
    }
}
