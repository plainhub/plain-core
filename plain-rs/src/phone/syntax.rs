use super::plan::PLANS;
use regex::Regex;
use std::{borrow::Cow, sync::LazyLock};

// Phone syntax follows Google libphonenumber v9.0.41 (Apache-2.0); attribution is in data/NOTICE.
const PUNCTUATION: &str = r"-x\u{2010}-\u{2015}\u{2212}\u{30fc}\u{ff0d}-\u{ff0f} \u{a0}\u{ad}\u{200b}\u{2060}\u{3000}()\u{ff08}\u{ff09}\u{ff3b}\u{ff3d}.\[\]/~\u{2053}\u{223c}";

static VIABLE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
    r"(?i)^(?:[+＋]*(?:[{PUNCTUATION}*]*\p{{Nd}}){{3,}}[{PUNCTUATION}*a-z\p{{Nd}}]*|\p{{Nd}}{{2}})$"
)).unwrap()
});
static SECOND: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(.*)[\\/] *x").unwrap());
static EXTENSION: LazyLock<Regex> = LazyLock::new(|| {
    let label = r"(?:e?xt(?:ensi(?:o\u{0301}?|ó))?n?|ｅ?ｘｔｎ?|доб|anexo)";
    let ambiguous = r"(?:[xｘ#＃~～]|int|ｉｎｔ)";
    let separators = r"[ \u{a0}\t,]*";
    let after = r"[:.．]?[ \u{a0}\t,-]*";
    Regex::new(&format!(r"(?i)(?:;ext=\p{{Nd}}{{1,20}}|{separators}{label}{after}\p{{Nd}}{{1,20}}#?|{separators}{ambiguous}{after}\p{{Nd}}{{1,9}}#?|[- ]+\p{{Nd}}{{1,6}}#|[ \u{{a0}}\t]*(?:,{{2}}|;){after}\p{{Nd}}{{1,15}}#?|[ \u{{a0}}\t]*,+{after}\p{{Nd}}{{1,9}}#?)$")).unwrap()
});
static CONTEXT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
    r"^(?:\+[\p{Nd}().-]+|(?:[a-zA-Z0-9](?:[a-zA-Z0-9-]*[a-zA-Z0-9])?\.)*[a-zA-Z](?:[a-zA-Z0-9-]*[a-zA-Z0-9])?\.?)$"
).unwrap()
});

pub(super) struct Normalized {
    pub digits: String,
    pub plus: bool,
}

pub(super) fn prepare(input: &str) -> Option<Normalized> {
    if input.len() > 250 {
        return None;
    }
    let uri = uri(input)?;
    let start = uri
        .char_indices()
        .find(|(_, ch)| matches!(ch, '+' | '＋') || digit(*ch).is_some())?
        .0;
    let number = uri[start..].trim_end_matches(|ch: char| !ch.is_alphanumeric() && ch != '#');
    let number = SECOND
        .captures(number)
        .and_then(|captures| captures.get(1))
        .map_or(number, |value| value.as_str());
    let number = if let Some(extension) = EXTENSION.find(number) {
        if VIABLE.is_match(&number[..extension.start()]) {
            &number[..extension.start()]
        } else {
            number
        }
    } else {
        number
    };
    if !VIABLE.is_match(number) {
        return None;
    }
    let alpha = number
        .bytes()
        .filter(u8::is_ascii_alphabetic)
        .take(3)
        .count()
        == 3;
    let digits = number
        .chars()
        .filter_map(|ch| {
            digit(ch).or_else(|| {
                if !alpha {
                    return None;
                }
                match ch.to_ascii_uppercase() {
                    'A'..='C' => Some('2'),
                    'D'..='F' => Some('3'),
                    'G'..='I' => Some('4'),
                    'J'..='L' => Some('5'),
                    'M'..='O' => Some('6'),
                    'P'..='S' => Some('7'),
                    'T'..='V' => Some('8'),
                    'W'..='Z' => Some('9'),
                    _ => None,
                }
            })
        })
        .collect();
    Some(Normalized {
        digits,
        plus: number.starts_with(['+', '＋']),
    })
}

fn uri(input: &str) -> Option<Cow<'_, str>> {
    let input = if input
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("tel:"))
    {
        &input[4..]
    } else {
        input
    };
    let value = if let Some((number, context)) = input.split_once(";phone-context=") {
        let context = context.split(';').next().unwrap();
        if !CONTEXT.is_match(context)
            || (context.starts_with('+') && !context.chars().any(|ch| digit(ch).is_some()))
        {
            return None;
        }
        if context.starts_with('+') {
            Cow::Owned(format!("{context}{number}"))
        } else {
            Cow::Borrowed(number)
        }
    } else {
        Cow::Borrowed(input)
    };
    if let Some(index) = value.find(";isub=") {
        return Some(Cow::Owned(value[..index].to_owned()));
    }
    Some(value)
}

pub(super) fn digit(ch: char) -> Option<char> {
    let point = ch as u32;
    let index = PLANS
        .decimal_zeroes
        .partition_point(|zero| *zero <= point)
        .checked_sub(1)?;
    let digit = point - PLANS.decimal_zeroes[index];
    (digit < 10).then(|| char::from(b'0' + digit as u8))
}

#[cfg(test)]
#[path = "../../tests/unit/phone/syntax.rs"]
mod tests;
