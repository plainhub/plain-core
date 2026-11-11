use crate::tungstenite::{
    ReadBuffer,
    error::{Error, ProtocolError, Result},
    util::NonBlockingResult,
};
use bytes::Buf;
use log::*;
use std::io::{Cursor, Read, Write};
#[derive(Debug)]
pub struct HandshakeMachine<Stream> {
    stream: Stream,
    state: HandshakeState,
}
impl<Stream> HandshakeMachine<Stream> {
    pub fn start_read(stream: Stream) -> Self {
        Self {
            stream,
            state: HandshakeState::Reading(ReadBuffer::new(), AttackCheck::new()),
        }
    }
    pub fn start_write<D: Into<Vec<u8>>>(stream: Stream, data: D) -> Self {
        HandshakeMachine {
            stream,
            state: HandshakeState::Writing(Cursor::new(data.into())),
        }
    }
    pub fn get_ref(&self) -> &Stream {
        &self.stream
    }
    pub fn get_mut(&mut self) -> &mut Stream {
        &mut self.stream
    }
}
impl<Stream: Read + Write> HandshakeMachine<Stream> {
    pub fn single_round<Obj: TryParse>(mut self) -> Result<RoundResult<Obj, Stream>> {
        trace!("Doing handshake round.");
        match self.state {
            HandshakeState::Reading(mut buf, mut attack_check) => {
                let read = buf.read_from(&mut self.stream).no_block()?;
                match read {
                    Some(0) => Err(Error::Protocol(ProtocolError::HandshakeIncomplete)),
                    Some(count) => {
                        attack_check.check_incoming_packet_size(count)?;
                        Ok(
                            if let Some((size, obj)) = Obj::try_parse(Buf::chunk(&buf))? {
                                buf.advance(size);
                                RoundResult::StageFinished(StageResult::DoneReading {
                                    result: obj,
                                    stream: self.stream,
                                    tail: buf.into_vec(),
                                })
                            } else {
                                RoundResult::Incomplete(HandshakeMachine {
                                    state: HandshakeState::Reading(buf, attack_check),
                                    ..self
                                })
                            },
                        )
                    }
                    None => Ok(RoundResult::WouldBlock(HandshakeMachine {
                        state: HandshakeState::Reading(buf, attack_check),
                        ..self
                    })),
                }
            }
            HandshakeState::Writing(mut buf) => {
                assert!(buf.has_remaining());
                if let Some(size) = self.stream.write(Buf::chunk(&buf)).no_block()? {
                    assert!(size > 0);
                    buf.advance(size);
                    Ok(if buf.has_remaining() {
                        RoundResult::Incomplete(HandshakeMachine {
                            state: HandshakeState::Writing(buf),
                            ..self
                        })
                    } else {
                        RoundResult::Incomplete(HandshakeMachine {
                            state: HandshakeState::Flushing,
                            ..self
                        })
                    })
                } else {
                    Ok(RoundResult::WouldBlock(HandshakeMachine {
                        state: HandshakeState::Writing(buf),
                        ..self
                    }))
                }
            }
            HandshakeState::Flushing => Ok(match self.stream.flush().no_block()? {
                Some(()) => RoundResult::StageFinished(StageResult::DoneWriting(self.stream)),
                None => RoundResult::WouldBlock(HandshakeMachine {
                    state: HandshakeState::Flushing,
                    ..self
                }),
            }),
        }
    }
}
#[derive(Debug)]
pub enum RoundResult<Obj, Stream> {
    WouldBlock(HandshakeMachine<Stream>),
    Incomplete(HandshakeMachine<Stream>),
    StageFinished(StageResult<Obj, Stream>),
}
#[derive(Debug)]
pub enum StageResult<Obj, Stream> {
    DoneReading {
        result: Obj,
        stream: Stream,
        tail: Vec<u8>,
    },
    DoneWriting(Stream),
}
pub trait TryParse: Sized {
    fn try_parse(data: &[u8]) -> Result<Option<(usize, Self)>>;
}
#[derive(Debug)]
enum HandshakeState {
    Reading(ReadBuffer, AttackCheck),
    Writing(Cursor<Vec<u8>>),
    Flushing,
}
#[derive(Debug)]
pub(crate) struct AttackCheck {
    number_of_packets: usize,
    number_of_bytes: usize,
}
impl AttackCheck {
    fn new() -> Self {
        Self {
            number_of_packets: 0,
            number_of_bytes: 0,
        }
    }
    fn check_incoming_packet_size(&mut self, size: usize) -> Result<()> {
        self.number_of_packets += 1;
        self.number_of_bytes += size;
        const MAX_BYTES: usize = 65536;
        const MAX_PACKETS: usize = 512;
        const MIN_PACKET_SIZE: usize = 128;
        const MIN_PACKET_CHECK_THRESHOLD: usize = 64;
        if self.number_of_bytes > MAX_BYTES {
            return Err(Error::AttackAttempt);
        }
        if self.number_of_packets > MAX_PACKETS {
            return Err(Error::AttackAttempt);
        }
        if self.number_of_packets > MIN_PACKET_CHECK_THRESHOLD
            && self.number_of_packets * MIN_PACKET_SIZE > self.number_of_bytes
        {
            return Err(Error::AttackAttempt);
        }
        Ok(())
    }
}
