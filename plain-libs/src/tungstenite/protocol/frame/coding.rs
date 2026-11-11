use std::{
    convert::{From, Into},
    fmt,
};
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum OpCode {
    Data(Data),
    Control(Control),
}
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Data {
    Continue,
    Text,
    Binary,
    Reserved(u8),
}
#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum Control {
    Close,
    Ping,
    Pong,
    Reserved(u8),
}
impl fmt::Display for Data {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Data::Continue => write!(f, "CONTINUE"),
            Data::Text => write!(f, "TEXT"),
            Data::Binary => write!(f, "BINARY"),
            Data::Reserved(x) => write!(f, "RESERVED_DATA_{x}"),
        }
    }
}
impl fmt::Display for Control {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Control::Close => write!(f, "CLOSE"),
            Control::Ping => write!(f, "PING"),
            Control::Pong => write!(f, "PONG"),
            Control::Reserved(x) => write!(f, "RESERVED_CONTROL_{x}"),
        }
    }
}
impl fmt::Display for OpCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            OpCode::Data(d) => d.fmt(f),
            OpCode::Control(c) => c.fmt(f),
        }
    }
}
impl From<OpCode> for u8 {
    fn from(code: OpCode) -> Self {
        use self::{
            Control::{Close, Ping, Pong},
            Data::{Binary, Continue, Text},
            OpCode::*,
        };
        match code {
            Data(Continue) => 0,
            Data(Text) => 1,
            Data(Binary) => 2,
            Data(self::Data::Reserved(i)) => i,
            Control(Close) => 8,
            Control(Ping) => 9,
            Control(Pong) => 10,
            Control(self::Control::Reserved(i)) => i,
        }
    }
}
impl From<u8> for OpCode {
    fn from(byte: u8) -> OpCode {
        use self::{
            Control::{Close, Ping, Pong},
            Data::{Binary, Continue, Text},
            OpCode::*,
        };
        match byte {
            0 => Data(Continue),
            1 => Data(Text),
            2 => Data(Binary),
            i @ 3..=7 => Data(self::Data::Reserved(i)),
            8 => Control(Close),
            9 => Control(Ping),
            10 => Control(Pong),
            i @ 11..=15 => Control(self::Control::Reserved(i)),
            _ => panic!("Bug: OpCode out of range"),
        }
    }
}
use self::CloseCode::*;
#[derive(Debug, Eq, PartialEq, Clone, Copy)]
pub enum CloseCode {
    Normal,
    Away,
    Protocol,
    Unsupported,
    Status,
    Abnormal,
    Invalid,
    Policy,
    Size,
    Extension,
    Error,
    Restart,
    Again,
    Tls,
    Reserved(u16),
    Iana(u16),
    Library(u16),
    Bad(u16),
}
impl CloseCode {
    pub fn is_allowed(self) -> bool {
        !matches!(self, Bad(_) | Reserved(_) | Status | Abnormal | Tls)
    }
}
impl fmt::Display for CloseCode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let code: u16 = self.into();
        write!(f, "{code}")
    }
}
impl From<CloseCode> for u16 {
    fn from(code: CloseCode) -> u16 {
        match code {
            Normal => 1000,
            Away => 1001,
            Protocol => 1002,
            Unsupported => 1003,
            Status => 1005,
            Abnormal => 1006,
            Invalid => 1007,
            Policy => 1008,
            Size => 1009,
            Extension => 1010,
            Error => 1011,
            Restart => 1012,
            Again => 1013,
            Tls => 1015,
            Reserved(code) => code,
            Iana(code) => code,
            Library(code) => code,
            Bad(code) => code,
        }
    }
}
impl<'t> From<&'t CloseCode> for u16 {
    fn from(code: &'t CloseCode) -> u16 {
        (*code).into()
    }
}
impl From<u16> for CloseCode {
    fn from(code: u16) -> CloseCode {
        match code {
            1000 => Normal,
            1001 => Away,
            1002 => Protocol,
            1003 => Unsupported,
            1005 => Status,
            1006 => Abnormal,
            1007 => Invalid,
            1008 => Policy,
            1009 => Size,
            1010 => Extension,
            1011 => Error,
            1012 => Restart,
            1013 => Again,
            1015 => Tls,
            1..=999 => Bad(code),
            1016..=2999 => Reserved(code),
            3000..=3999 => Iana(code),
            4000..=4999 => Library(code),
            _ => Bad(code),
        }
    }
}
