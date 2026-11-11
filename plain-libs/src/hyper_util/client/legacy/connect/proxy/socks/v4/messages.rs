use super::super::{ParsingError, SerializeError};
use bytes::{Buf, BufMut};
use std::net::SocketAddrV4;
#[derive(Debug, PartialEq)]
pub struct Request<'a>(pub &'a Address);
#[derive(Debug, PartialEq)]
pub struct Response(pub Status);
#[derive(Debug, PartialEq)]
pub enum Address {
    Socket(SocketAddrV4),
    Domain(String, u16),
}
#[derive(Debug, PartialEq)]
pub enum Status {
    Success = 90,
    Failed = 91,
    IdentFailure = 92,
    IdentMismatch = 93,
}
impl Request<'_> {
    pub fn write_to_buf<B: BufMut>(&self, mut buf: B) -> Result<usize, SerializeError> {
        match self.0 {
            Address::Socket(socket) => {
                if buf.remaining_mut() < 9 {
                    return Err(SerializeError::WouldOverflow);
                }
                buf.put_u8(0x04);
                buf.put_u8(0x01);
                buf.put_u16(socket.port());
                buf.put_slice(&socket.ip().octets());
                buf.put_u8(0x00);
                Ok(9)
            }
            Address::Domain(domain, port) => {
                if buf.remaining_mut() < 9 + domain.len() + 1 {
                    return Err(SerializeError::WouldOverflow);
                }
                buf.put_u8(0x04);
                buf.put_u8(0x01);
                buf.put_u16(*port);
                buf.put_slice(&[0x00, 0x00, 0x00, 0xFF]);
                buf.put_u8(0x00);
                buf.put_slice(domain.as_bytes());
                buf.put_u8(0x00);
                Ok(9 + domain.len() + 1)
            }
        }
    }
}
impl TryFrom<&mut &[u8]> for Response {
    type Error = ParsingError;
    fn try_from(buf: &mut &[u8]) -> Result<Self, Self::Error> {
        if buf.remaining() < 8 {
            return Err(ParsingError::Incomplete);
        }
        if buf.get_u8() != 0x00 {
            return Err(ParsingError::Other);
        }
        let status = buf.get_u8().try_into()?;
        let _addr = {
            let port = buf.get_u16();
            let mut ip = [0; 4];
            buf.copy_to_slice(&mut ip);
            SocketAddrV4::new(ip.into(), port)
        };
        Ok(Self(status))
    }
}
impl TryFrom<u8> for Status {
    type Error = ParsingError;
    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        Ok(match byte {
            90 => Self::Success,
            91 => Self::Failed,
            92 => Self::IdentFailure,
            93 => Self::IdentMismatch,
            _ => return Err(ParsingError::Other),
        })
    }
}
impl std::fmt::Display for Status {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Success => "success",
            Self::Failed => "server failed to execute command",
            Self::IdentFailure => "server ident service failed",
            Self::IdentMismatch => "server ident service did not recognise client identifier",
        })
    }
}
