use std::fmt;

const GROUPS: [usize; 5] = [8, 4, 4, 4, 12];
const GROUP_STARTS: [usize; 5] = [0, 9, 14, 19, 24];

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ParseError {
    Character { character: char, index: usize },
    GroupCount(usize),
    GroupLength { group: usize, length: usize },
    Length(usize),
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::Character { character, index } => {
                write!(f, "invalid character: found `{character}` at {index}")
            }
            Self::GroupCount(count) => write!(f, "invalid group count: expected 5, found {count}"),
            Self::GroupLength { group, length } => write!(
                f,
                "invalid group length in group {group}: expected {}, found {length}",
                GROUPS[group]
            ),
            Self::Length(length) => write!(f, "invalid length: found {length}"),
        }
    }
}

impl std::error::Error for ParseError {}

pub(super) fn parse(value: &str) -> Result<[u8; 16], ParseError> {
    let result = match value.len() {
        32 => {
            let hex = value
                .as_bytes()
                .try_into()
                .expect("exact UUID string length");
            return decode_hex32(hex).ok_or_else(|| diagnose(value, false));
        }
        36 => Some(value),
        38 => value
            .strip_prefix('{')
            .and_then(|inner| inner.strip_suffix('}')),
        45 => value.strip_prefix("urn:uuid:"),
        _ => None,
    };
    match result {
        Some(inner) => parse_hyphenated(inner).ok_or_else(|| diagnose(inner, true)),
        None => Err(diagnose(value, false)),
    }
}

fn parse_hyphenated(value: &str) -> Option<[u8; 16]> {
    let s: &[u8; 36] = value.as_bytes().try_into().ok()?;
    if [s[8], s[13], s[18], s[23]] != [b'-'; 4] {
        return None;
    }
    let mut hex = [0; 32];
    hex[..8].copy_from_slice(&s[..8]);
    hex[8..12].copy_from_slice(&s[9..13]);
    hex[12..16].copy_from_slice(&s[14..18]);
    hex[16..20].copy_from_slice(&s[19..23]);
    hex[20..].copy_from_slice(&s[24..]);
    decode_hex32(&hex)
}

fn diagnose(value: &str, mut hyphenated: bool) -> ParseError {
    if value.is_empty() || value.len() > 45 {
        return ParseError::Length(value.len());
    }
    let (offset, inner) = if !hyphenated {
        if let Some(inner) = value
            .strip_prefix('{')
            .and_then(|inner| inner.strip_suffix('}'))
        {
            hyphenated = true;
            (1, inner)
        } else if let Some(inner) = value.strip_prefix("urn:uuid:") {
            hyphenated = true;
            (9, inner)
        } else {
            (0, value)
        }
    } else {
        (0, value)
    };

    let mut hyphens = [0; 4];
    let mut count = 0;
    for (index, character) in inner.char_indices() {
        if character == '-' {
            hyphenated = true;
            if count < hyphens.len() {
                hyphens[count] = index;
            }
            count += 1;
        } else if !character.is_ascii_hexdigit() {
            return ParseError::Character {
                character,
                index: offset + index,
            };
        }
    }
    if !hyphenated {
        return ParseError::Length(value.len());
    }
    if count != 4 {
        return ParseError::GroupCount(count + 1);
    }
    for (group, position) in hyphens.into_iter().enumerate() {
        if position != GROUP_STARTS[group + 1] - 1 {
            return ParseError::GroupLength {
                group,
                length: position - GROUP_STARTS[group],
            };
        }
    }
    ParseError::GroupLength {
        group: 4,
        length: value.len() - GROUP_STARTS[4],
    }
}
#[inline]
const fn decode_hex32(hex: &[u8; 32]) -> Option<[u8; 16]> {
    let mut nibbles = [0u8; 32];
    let mut bad = 0u8;

    let mut i = 0;
    while i < 32 {
        let c = hex[i];

        // '0'..='9' map to 0..=9; 'a'..='f' and 'A'..='F' (via `| 0x20`) map to
        // 0..=5, offset by 10 to land in 10..=15.
        let digit = c.wrapping_sub(b'0');
        let alpha = (c | 0x20).wrapping_sub(b'a');

        let is_digit = digit < 10;
        let is_alpha = alpha < 6;

        nibbles[i] = if is_digit {
            digit
        } else {
            alpha.wrapping_add(10)
        };

        bad |= if is_digit | is_alpha { 0 } else { 1 };

        i += 1;
    }

    if bad != 0 {
        return None;
    }

    let mut buf = [0u8; 16];
    let mut j = 0;
    while j < 16 {
        buf[j] = (nibbles[j * 2] << 4) | nibbles[j * 2 + 1];
        j += 1;
    }

    Some(buf)
}
