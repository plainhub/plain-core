pub mod client;
pub mod headers;
pub mod machine;
pub mod server;
use self::machine::{HandshakeMachine, RoundResult, StageResult, TryParse};
use crate::http::Version;
use crate::tungstenite::error::{Error, ProtocolError};
use sha1::{Digest, Sha1};
use std::{
    error::Error as ErrorTrait,
    fmt,
    io::{Read, Write},
};
#[derive(Debug)]
pub struct MidHandshake<Role: HandshakeRole> {
    role: Role,
    machine: HandshakeMachine<Role::InternalStream>,
}
impl<Role: HandshakeRole> MidHandshake<Role> {
    pub fn get_ref(&self) -> &HandshakeMachine<Role::InternalStream> {
        &self.machine
    }
    pub fn get_mut(&mut self) -> &mut HandshakeMachine<Role::InternalStream> {
        &mut self.machine
    }
    pub fn handshake(mut self) -> Result<Role::FinalResult, HandshakeError<Role>> {
        let mut mach = self.machine;
        loop {
            mach = match mach.single_round()? {
                RoundResult::WouldBlock(m) => {
                    return Err(HandshakeError::Interrupted(MidHandshake {
                        machine: m,
                        ..self
                    }));
                }
                RoundResult::Incomplete(m) => m,
                RoundResult::StageFinished(s) => match self.role.stage_finished(s)? {
                    ProcessingResult::Continue(m) => m,
                    ProcessingResult::Done(result) => return Ok(result),
                },
            }
        }
    }
}
pub enum HandshakeError<Role: HandshakeRole> {
    Interrupted(MidHandshake<Role>),
    Failure(Error),
}
impl<Role: HandshakeRole> fmt::Debug for HandshakeError<Role> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            HandshakeError::Interrupted(_) => write!(f, "HandshakeError::Interrupted(...)"),
            HandshakeError::Failure(ref e) => write!(f, "HandshakeError::Failure({e:?})"),
        }
    }
}
impl<Role: HandshakeRole> fmt::Display for HandshakeError<Role> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            HandshakeError::Interrupted(_) => write!(f, "Interrupted handshake (WouldBlock)"),
            HandshakeError::Failure(ref e) => write!(f, "{e}"),
        }
    }
}
impl<Role: HandshakeRole> ErrorTrait for HandshakeError<Role> {}
impl<Role: HandshakeRole> From<Error> for HandshakeError<Role> {
    fn from(err: Error) -> Self {
        HandshakeError::Failure(err)
    }
}
pub trait HandshakeRole {
    type IncomingData: TryParse;
    type InternalStream: Read + Write;
    type FinalResult;
    fn stage_finished(
        &mut self,
        finish: StageResult<Self::IncomingData, Self::InternalStream>,
    ) -> Result<ProcessingResult<Self::InternalStream, Self::FinalResult>, Error>;
}
#[derive(Debug)]
pub enum ProcessingResult<Stream, FinalResult> {
    Continue(HandshakeMachine<Stream>),
    Done(FinalResult),
}
pub fn derive_accept_key(request_key: &[u8]) -> String {
    const WS_GUID: &[u8] = b"258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let mut sha1 = Sha1::default();
    sha1.update(request_key);
    sha1.update(WS_GUID);
    crate::utils::base64::base64_encode(&sha1.finalize())
}
fn version_as_str(ver: Version) -> crate::tungstenite::Result<&'static str> {
    match ver {
        ver if ver == Version::HTTP_09 => Ok("HTTP/0.9"),
        ver if ver == Version::HTTP_10 => Ok("HTTP/1.0"),
        ver if ver == Version::HTTP_11 => Ok("HTTP/1.1"),
        _ => Err(Error::Protocol(ProtocolError::WrongHttpVersion)),
    }
}
