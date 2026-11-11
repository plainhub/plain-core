use super::frame::{CloseFrame, Frame};
use crate::tungstenite::{
    error::{CapacityError, Error, Result},
    protocol::frame::Utf8Bytes,
};
use std::{fmt, result::Result as StdResult, str};
mod string_collect {
    use crate::tungstenite::error::{Error, Result};
    use crate::tungstenite::utf8::DecodeError;
    #[derive(Debug)]
    pub struct StringCollector {
        data: String,
        incomplete: Option<crate::tungstenite::utf8::Incomplete>,
    }
    impl StringCollector {
        pub fn new() -> Self {
            StringCollector {
                data: String::new(),
                incomplete: None,
            }
        }
        pub fn len(&self) -> usize {
            self.data
                .len()
                .saturating_add(self.incomplete.map(|i| i.buffer_len as usize).unwrap_or(0))
        }
        pub fn extend<T: AsRef<[u8]>>(&mut self, tail: T) -> Result<()> {
            let mut input: &[u8] = tail.as_ref();
            if let Some(mut incomplete) = self.incomplete.take() {
                if let Some(completed) = incomplete.try_complete(input) {
                    input = completed.remaining_input;
                    match completed.result {
                        Ok(text) => self.data.push_str(text),
                        Err(result_bytes) => {
                            return Err(Error::Utf8(String::from_utf8_lossy(result_bytes).into()));
                        }
                    }
                } else {
                    input = &[];
                    self.incomplete = Some(incomplete);
                }
            }
            if !input.is_empty() {
                match crate::tungstenite::utf8::decode(input) {
                    Ok(text) => {
                        self.data.push_str(text);
                        Ok(())
                    }
                    Err(DecodeError::Incomplete {
                        valid_prefix,
                        incomplete_suffix,
                    }) => {
                        self.data.push_str(valid_prefix);
                        self.incomplete = Some(incomplete_suffix);
                        Ok(())
                    }
                    Err(DecodeError::Invalid {
                        valid_prefix,
                        invalid_sequence,
                        ..
                    }) => {
                        self.data.push_str(valid_prefix);
                        Err(Error::Utf8(
                            String::from_utf8_lossy(invalid_sequence).into(),
                        ))
                    }
                }
            } else {
                Ok(())
            }
        }
        pub fn into_string(self) -> Result<String> {
            if let Some(incomplete) = self.incomplete {
                Err(Error::Utf8(format!("incomplete string: {incomplete:?}")))
            } else {
                Ok(self.data)
            }
        }
    }
}
use self::string_collect::StringCollector;
use bytes::Bytes;
#[derive(Debug)]
pub struct IncompleteMessage {
    collector: IncompleteMessageCollector,
}
#[derive(Debug)]
enum IncompleteMessageCollector {
    Text(StringCollector),
    Binary(Vec<u8>),
}
impl IncompleteMessage {
    pub fn new(message_type: MessageType) -> Self {
        IncompleteMessage {
            collector: match message_type {
                MessageType::Binary => IncompleteMessageCollector::Binary(Vec::new()),
                MessageType::Text => IncompleteMessageCollector::Text(StringCollector::new()),
            },
        }
    }
    pub fn len(&self) -> usize {
        match self.collector {
            IncompleteMessageCollector::Text(ref t) => t.len(),
            IncompleteMessageCollector::Binary(ref b) => b.len(),
        }
    }
    pub fn extend<T: AsRef<[u8]>>(&mut self, tail: T, size_limit: Option<usize>) -> Result<()> {
        let max_size = size_limit.unwrap_or_else(usize::max_value);
        let my_size = self.len();
        let portion_size = tail.as_ref().len();
        if my_size > max_size || portion_size > max_size - my_size {
            return Err(Error::Capacity(CapacityError::MessageTooLong {
                size: my_size + portion_size,
                max_size,
            }));
        }
        match self.collector {
            IncompleteMessageCollector::Binary(ref mut v) => {
                v.extend(tail.as_ref());
                Ok(())
            }
            IncompleteMessageCollector::Text(ref mut t) => t.extend(tail),
        }
    }
    pub fn complete(self) -> Result<Message> {
        match self.collector {
            IncompleteMessageCollector::Binary(v) => Ok(Message::Binary(v.into())),
            IncompleteMessageCollector::Text(t) => {
                let text = t.into_string()?;
                Ok(Message::text(text))
            }
        }
    }
}
pub enum MessageType {
    Text,
    Binary,
}
#[derive(Debug, Eq, PartialEq, Clone)]
pub enum Message {
    Text(Utf8Bytes),
    Binary(Bytes),
    Ping(Bytes),
    Pong(Bytes),
    Close(Option<CloseFrame>),
    Frame(Frame),
}
impl Message {
    pub fn text<S>(string: S) -> Message
    where
        S: Into<Utf8Bytes>,
    {
        Message::Text(string.into())
    }
    pub fn binary<B>(bin: B) -> Message
    where
        B: Into<Bytes>,
    {
        Message::Binary(bin.into())
    }
    pub fn is_text(&self) -> bool {
        matches!(*self, Message::Text(_))
    }

    pub fn len(&self) -> usize {
        match *self {
            Message::Text(ref string) => string.len(),
            Message::Binary(ref data) | Message::Ping(ref data) | Message::Pong(ref data) => {
                data.len()
            }
            Message::Close(ref data) => data.as_ref().map(|d| d.reason.len()).unwrap_or(0),
            Message::Frame(ref frame) => frame.len(),
        }
    }
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    pub fn into_data(self) -> Bytes {
        match self {
            Message::Text(utf8) => utf8.into(),
            Message::Binary(data) | Message::Ping(data) | Message::Pong(data) => data,
            Message::Close(None) => <_>::default(),
            Message::Close(Some(frame)) => frame.reason.into(),
            Message::Frame(frame) => frame.into_payload(),
        }
    }
    pub fn into_text(self) -> Result<Utf8Bytes> {
        match self {
            Message::Text(txt) => Ok(txt),
            Message::Binary(data) | Message::Ping(data) | Message::Pong(data) => {
                Ok(data.try_into()?)
            }
            Message::Close(None) => Ok(<_>::default()),
            Message::Close(Some(frame)) => Ok(frame.reason),
            Message::Frame(frame) => Ok(frame.into_text()?),
        }
    }
    pub fn to_text(&self) -> Result<&str> {
        match *self {
            Message::Text(ref string) => Ok(string.as_str()),
            Message::Binary(ref data) | Message::Ping(ref data) | Message::Pong(ref data) => {
                Ok(str::from_utf8(data)?)
            }
            Message::Close(None) => Ok(""),
            Message::Close(Some(ref frame)) => Ok(&frame.reason),
            Message::Frame(ref frame) => Ok(frame.to_text()?),
        }
    }
}
impl From<String> for Message {
    #[inline]
    fn from(string: String) -> Self {
        Message::text(string)
    }
}
impl<'s> From<&'s str> for Message {
    #[inline]
    fn from(string: &'s str) -> Self {
        Message::text(string)
    }
}
impl<'b> From<&'b [u8]> for Message {
    #[inline]
    fn from(data: &'b [u8]) -> Self {
        Message::binary(Bytes::copy_from_slice(data))
    }
}
impl From<Bytes> for Message {
    fn from(data: Bytes) -> Self {
        Message::binary(data)
    }
}
impl From<Vec<u8>> for Message {
    #[inline]
    fn from(data: Vec<u8>) -> Self {
        Message::binary(data)
    }
}
impl From<Message> for Bytes {
    #[inline]
    fn from(message: Message) -> Self {
        message.into_data()
    }
}
impl fmt::Display for Message {
    fn fmt(&self, f: &mut fmt::Formatter) -> StdResult<(), fmt::Error> {
        if let Ok(string) = self.to_text() {
            write!(f, "{string}")
        } else {
            write!(f, "Binary Data<length={}>", self.len())
        }
    }
}
