use super::{CHARSET, Indexed, Mime, MimeIter, ParamSource, Source, UTF_8};

use std::error::Error;
use std::fmt;
use std::iter::Enumerate;
use std::str::Bytes;
#[derive(Debug)]
pub enum ParseError {
    MissingSlash,
    MissingEqual,
    MissingQuote,
    InvalidToken { pos: usize, byte: u8 },
}
impl ParseError {
    fn s(&self) -> &str {
        use self::ParseError::*;
        match *self {
            MissingSlash => "a slash (/) was missing between the type and subtype",
            MissingEqual => "an equals sign (=) was missing between a parameter and its value",
            MissingQuote => "a quote (\") was missing from a parameter value",
            InvalidToken { .. } => "an invalid token was encountered",
        }
    }
}
impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        if let ParseError::InvalidToken { pos, byte } = *self {
            write!(f, "{}, {:X} at position {}", self.s(), byte, pos)
        } else {
            f.write_str(self.s())
        }
    }
}
impl Error for ParseError {
    fn description(&self) -> &str {
        self.s()
    }
}
impl<'a> MimeIter<'a> {
    pub fn new(s: &'a str) -> Self {
        Self { pos: 0, source: s }
    }
}
impl<'a> Iterator for MimeIter<'a> {
    type Item = Result<Mime, &'a str>;
    fn next(&mut self) -> Option<Self::Item> {
        let start = self.pos;
        let len = self.source.bytes().len();
        if start >= len {
            return None;
        }
        match parse(&self.source[start..len]) {
            Ok(value) => {
                self.pos = len;
                Some(Ok(value))
            }
            Err(ParseError::InvalidToken { pos, .. }) => {
                if pos == 0 {
                    self.pos += 1;
                    return self.next();
                }
                let slice = &self.source[start..start + pos];
                return match parse(slice) {
                    Ok(mime) => {
                        self.pos = start + pos + 1;
                        Some(Ok(mime))
                    }
                    Err(_) => {
                        if start + pos < len {
                            self.pos = start + pos;
                            Some(Err(slice))
                        } else {
                            None
                        }
                    }
                };
            }
            Err(_) => None,
        }
    }
}
pub fn parse(s: &str) -> Result<Mime, ParseError> {
    if s == "*/*" {
        return Ok(crate::mime::STAR_STAR);
    }
    let mut iter = s.bytes().enumerate();
    let mut start;
    let slash;
    loop {
        match iter.next() {
            Some((_, c)) if is_token(c) => (),
            Some((i, b'/')) if i > 0 => {
                slash = i;
                start = i + 1;
                break;
            }
            None => return Err(ParseError::MissingSlash),
            Some((pos, byte)) => {
                return Err(ParseError::InvalidToken {
                    pos: pos,
                    byte: byte,
                });
            }
        };
    }
    let mut plus = None;
    loop {
        match iter.next() {
            Some((i, b'+')) if i > start => {
                plus = Some(i);
            }
            Some((i, b';')) if i > start => {
                start = i;
                break;
            }
            Some((_, c)) if is_token(c) => (),
            None => {
                return Ok(Mime {
                    source: Source::Dynamic(s.to_ascii_lowercase()),
                    slash: slash,
                    plus: plus,
                    params: ParamSource::None,
                });
            }
            Some((pos, byte)) => {
                return Err(ParseError::InvalidToken {
                    pos: pos,
                    byte: byte,
                });
            }
        };
    }
    let params = params_from_str(s, &mut iter, start)?;
    let src = match params {
        ParamSource::Utf8(_) => s.to_ascii_lowercase(),
        ParamSource::Custom(semicolon, ref indices) => {
            lower_ascii_with_params(s, semicolon, indices)
        }
        ParamSource::None => s[..start].to_ascii_lowercase(),
    };
    Ok(Mime {
        source: Source::Dynamic(src),
        slash: slash,
        plus: plus,
        params: params,
    })
}
fn params_from_str(
    s: &str,
    iter: &mut Enumerate<Bytes>,
    mut start: usize,
) -> Result<ParamSource, ParseError> {
    let semicolon = start;
    start += 1;
    let mut params = ParamSource::None;
    'params: while start < s.len() {
        let name;
        'name: loop {
            match iter.next() {
                Some((i, b' ')) if i == start => {
                    start = i + 1;
                    continue 'params;
                }
                Some((_, c)) if is_token(c) => (),
                Some((i, b'=')) if i > start => {
                    name = Indexed(start, i);
                    start = i + 1;
                    break 'name;
                }
                None => return Err(ParseError::MissingEqual),
                Some((pos, byte)) => {
                    return Err(ParseError::InvalidToken {
                        pos: pos,
                        byte: byte,
                    });
                }
            }
        }
        let value;
        let mut is_quoted = false;
        'value: loop {
            if is_quoted {
                match iter.next() {
                    Some((i, b'"')) if i > start => {
                        value = Indexed(start, i);
                        break 'value;
                    }
                    Some((_, c)) if is_restricted_quoted_char(c) => (),
                    None => return Err(ParseError::MissingQuote),
                    Some((pos, byte)) => {
                        return Err(ParseError::InvalidToken {
                            pos: pos,
                            byte: byte,
                        });
                    }
                }
            } else {
                match iter.next() {
                    Some((i, b'"')) if i == start => {
                        is_quoted = true;
                        start = i + 1;
                    }
                    Some((_, c)) if is_token(c) => (),
                    Some((i, b';')) if i > start => {
                        value = Indexed(start, i);
                        start = i + 1;
                        break 'value;
                    }
                    None => {
                        value = Indexed(start, s.len());
                        start = s.len();
                        break 'value;
                    }
                    Some((pos, byte)) => {
                        return Err(ParseError::InvalidToken {
                            pos: pos,
                            byte: byte,
                        });
                    }
                }
            }
        }
        if is_quoted {
            'ws: loop {
                match iter.next() {
                    Some((i, b';')) => {
                        start = i + 1;
                        break 'ws;
                    }
                    Some((_, b' ')) => {}
                    None => {
                        start = s.len();
                        break 'ws;
                    }
                    Some((pos, byte)) => {
                        return Err(ParseError::InvalidToken {
                            pos: pos,
                            byte: byte,
                        });
                    }
                }
            }
        }
        match params {
            ParamSource::Utf8(i) => {
                let i = i + 2;
                let charset = Indexed(i, "charset".len() + i);
                let utf8 = Indexed(charset.1 + 1, charset.1 + "utf-8".len() + 1);
                params = ParamSource::Custom(semicolon, vec![(charset, utf8), (name, value)]);
            }
            ParamSource::Custom(_, ref mut vec) => {
                vec.push((name, value));
            }
            ParamSource::None => {
                if semicolon + 2 == name.0 && CHARSET == &s[name.0..name.1] {
                    if UTF_8 == &s[value.0..value.1] {
                        params = ParamSource::Utf8(semicolon);
                        continue 'params;
                    }
                }
                params = ParamSource::Custom(semicolon, vec![(name, value)]);
            }
        }
    }
    Ok(params)
}
fn lower_ascii_with_params(s: &str, semi: usize, params: &[(Indexed, Indexed)]) -> String {
    let mut owned = s.to_owned();
    owned[..semi].make_ascii_lowercase();
    for &(ref name, ref value) in params {
        owned[name.0..name.1].make_ascii_lowercase();
        if &owned[name.0..name.1] == CHARSET.source {
            owned[value.0..value.1].make_ascii_lowercase();
        }
    }
    owned
}
macro_rules! byte_map { ($($flag : expr ,) *) => ([$($flag != 0 ,) *]) }
static TOKEN_MAP: [bool; 256] = byte_map![
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 1, 0, 1, 1, 1, 1, 1, 0, 0, 1, 1, 0, 1, 1, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0,
    0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 1, 1,
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 1, 0, 1, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
fn is_token(c: u8) -> bool {
    TOKEN_MAP[c as usize]
}
fn is_restricted_quoted_char(c: u8) -> bool {
    c > 31 && c != 127
}
