use crate::audio_tags::tag::{ItemKey, TagItem};

use std::fmt::Display;

pub(crate) const NUMBER_PAIR_SEPARATOR: char = '/';

// This is used as the default number of track and disk.
pub(crate) const DEFAULT_NUMBER_IN_PAIR: u32 = 0;

// These keys have the part of the number pair.
pub(crate) const NUMBER_PAIR_KEYS: &[ItemKey] = &[
    ItemKey::TrackNumber,
    ItemKey::TrackTotal,
    ItemKey::DiscNumber,
    ItemKey::DiscTotal,
];

pub(crate) fn format_number_pair<N, T>(number: Option<N>, total: Option<T>) -> Option<String>
where
    N: Display,
    T: Display,
{
    match (number, total) {
        (Some(number), None) => Some(number.to_string()),
        (None, Some(total)) => Some(format!(
            "{DEFAULT_NUMBER_IN_PAIR}{NUMBER_PAIR_SEPARATOR}{total}"
        )),
        (Some(number), Some(total)) => Some(format!("{number}{NUMBER_PAIR_SEPARATOR}{total}")),
        (None, None) => None,
    }
}

pub(crate) fn set_number<F: FnMut(u32)>(item: &TagItem, mut setter: F) {
    let text = item.value().text();

    let trimmed_text = text.unwrap_or_default().trim();
    if trimmed_text.is_empty() {
        log::warn!("Value does not have text in {:?}", item.key());
        return;
    }

    match trimmed_text.parse::<u32>() {
        Ok(number) => setter(number),
        Err(parse_error) => {
            log::warn!(
                "\"{}\" cannot be parsed as number in {:?}: {parse_error}",
                text.unwrap(),
                item.key()
            )
        }
    }
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-libs/unit/audio_tags/id3/v2/util/pairs/tests.rs"]
mod tests;
