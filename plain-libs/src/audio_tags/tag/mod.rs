mod accessor;
pub(crate) mod companion_tag;
pub(crate) mod item;
pub mod items;
mod split_merge_tag;
mod tag_ext;
mod tag_type;
#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/audio_tags/tag/utils.rs"]
pub(crate) mod utils;

use crate::audio_tags::error::{LoftyError, Result};
use crate::audio_tags::macros::err;
use crate::audio_tags::picture::{Picture, PictureType};
use crate::audio_tags::probe::Probe;

use std::borrow::Cow;
use std::io::Write;
use std::path::Path;

// Exports
pub use accessor::Accessor;
pub(crate) use accessor::text_accessors;
pub use item::{ItemKey, ItemValue, TagItem};
pub use split_merge_tag::{MergeTag, SplitTag};
pub use tag_ext::TagExt;
pub use tag_type::TagType;

macro_rules! impl_accessor {
	($($item_key:ident => $name:ident, $set:ident, $remove:ident),+) => {
		$(
			fn $name(&self) -> Option<Cow<'_, str>> {
				if let Some(ItemValue::Text(txt)) = self.get(&ItemKey::$item_key).map(TagItem::value) {
					return Some(Cow::Borrowed(txt))
				}

				None
			}

			fn $set(&mut self, value: String) {
				self.insert(TagItem::new(ItemKey::$item_key, ItemValue::Text(value)));
			}

			fn $remove(&mut self) {
				self.retain(|i| i.item_key != ItemKey::$item_key)
			}
		)+
	}
}

#[derive(Clone)]
pub struct Tag {
    tag_type: TagType,
    pub(crate) pictures: Vec<Picture>,
    pub(crate) items: Vec<TagItem>,
    pub(crate) companion_tag: Option<companion_tag::CompanionTag>,
}

#[must_use]
pub(crate) fn try_parse_year(input: &str) -> Option<u32> {
    let (num_digits, year) = input
        .chars()
        .skip_while(|c| c.is_whitespace())
        .take_while(char::is_ascii_digit)
        .take(4)
        .fold((0usize, 0u32), |(num_digits, year), c| {
            let decimal_digit = c.to_digit(10).expect("decimal digit");
            (num_digits + 1, year * 10 + decimal_digit)
        });
    (num_digits == 4).then_some(year)
}

impl Accessor for Tag {
    impl_accessor!(
        TrackArtist => artist, set_artist, remove_artist,
        TrackTitle  => title, set_title, remove_title,
        AlbumTitle  => album, set_album, remove_album,
        Genre       => genre, set_genre, remove_genre,
        Comment     => comment, set_comment, remove_comment
    );

    fn track(&self) -> Option<u32> {
        self.get_u32_from_string(&ItemKey::TrackNumber)
    }

    fn set_track(&mut self, value: u32) {
        self.insert_text(ItemKey::TrackNumber, value.to_string());
    }

    fn remove_track(&mut self) {
        self.remove_key(&ItemKey::TrackNumber);
    }

    fn track_total(&self) -> Option<u32> {
        self.get_u32_from_string(&ItemKey::TrackTotal)
    }

    fn set_track_total(&mut self, value: u32) {
        self.insert_text(ItemKey::TrackTotal, value.to_string());
    }

    fn remove_track_total(&mut self) {
        self.remove_key(&ItemKey::TrackTotal);
    }

    fn disk(&self) -> Option<u32> {
        self.get_u32_from_string(&ItemKey::DiscNumber)
    }

    fn set_disk(&mut self, value: u32) {
        self.insert_text(ItemKey::DiscNumber, value.to_string());
    }

    fn remove_disk(&mut self) {
        self.remove_key(&ItemKey::DiscNumber);
    }

    fn disk_total(&self) -> Option<u32> {
        self.get_u32_from_string(&ItemKey::DiscTotal)
    }

    fn set_disk_total(&mut self, value: u32) {
        self.insert_text(ItemKey::DiscTotal, value.to_string());
    }

    fn remove_disk_total(&mut self) {
        self.remove_key(&ItemKey::DiscTotal);
    }

    fn year(&self) -> Option<u32> {
        if let Some(item) = self
            .get_string(&ItemKey::Year)
            .map_or_else(|| self.get_string(&ItemKey::RecordingDate), Some)
        {
            return try_parse_year(item);
        }

        None
    }

    fn set_year(&mut self, value: u32) {
        if let Some(item) = self.get_string(&ItemKey::RecordingDate) {
            if item.len() >= 4 {
                let (_, remaining) = item.split_at(4);
                self.insert_text(ItemKey::RecordingDate, format!("{value}{remaining}"));
                return;
            }
        }

        // Some formats have a dedicated item for `Year`, others just have it as
        // a part of `RecordingDate`
        if ItemKey::Year.map_key(self.tag_type, false).is_some() {
            self.insert_text(ItemKey::Year, value.to_string());
        } else {
            self.insert_text(ItemKey::RecordingDate, value.to_string());
        }
    }

    fn remove_year(&mut self) {
        self.remove_key(&ItemKey::Year);
        self.remove_key(&ItemKey::RecordingDate);
    }
}

impl Tag {
    #[must_use]
    pub const fn new(tag_type: TagType) -> Self {
        Self {
            tag_type,
            pictures: Vec::new(),
            items: Vec::new(),
            companion_tag: None,
        }
    }

    pub fn re_map(&mut self, tag_type: TagType) {
        if let Some(companion_tag) = self.companion_tag.take() {
            log::warn!("Discarding format-specific items due to remap");
            drop(companion_tag);
        }

        self.retain(|i| i.re_map(tag_type));
        self.tag_type = tag_type
    }

    pub fn has_format_specific_items(&self) -> bool {
        self.companion_tag.is_some()
    }

    pub fn tag_type(&self) -> TagType {
        self.tag_type
    }

    pub fn item_count(&self) -> u32 {
        self.items.len() as u32
    }

    pub fn picture_count(&self) -> u32 {
        self.pictures.len() as u32
    }

    pub fn items(&self) -> impl Iterator<Item = &TagItem> + Clone {
        self.items.iter()
    }

    pub fn get(&self, item_key: &ItemKey) -> Option<&TagItem> {
        self.items.iter().find(|i| &i.item_key == item_key)
    }

    pub fn get_string(&self, item_key: &ItemKey) -> Option<&str> {
        if let Some(ItemValue::Text(ret)) = self.get(item_key).map(TagItem::value) {
            return Some(ret);
        }

        None
    }

    fn get_u32_from_string(&self, key: &ItemKey) -> Option<u32> {
        let i = self.get_string(key)?;
        i.parse::<u32>().ok()
    }

    pub fn get_binary(&self, item_key: &ItemKey, convert: bool) -> Option<&[u8]> {
        if let Some(item) = self.get(item_key) {
            match item.value() {
                ItemValue::Text(text) | ItemValue::Locator(text) if convert => {
                    return Some(text.as_bytes());
                }
                ItemValue::Binary(binary) => return Some(binary),
                _ => {}
            }
        }

        None
    }

    pub fn insert(&mut self, item: TagItem) -> bool {
        if item.re_map(self.tag_type) {
            self.insert_unchecked(item);
            return true;
        }

        false
    }

    pub fn insert_unchecked(&mut self, item: TagItem) {
        self.retain(|i| i.item_key != item.item_key);
        self.items.push(item);
    }

    pub fn push(&mut self, item: TagItem) -> bool {
        if item.re_map(self.tag_type) {
            self.items.push(item);
            return true;
        }

        false
    }

    pub fn push_unchecked(&mut self, item: TagItem) {
        self.items.push(item);
    }

    pub fn insert_text(&mut self, item_key: ItemKey, text: String) -> bool {
        self.insert(TagItem::new(item_key, ItemValue::Text(text)))
    }

    pub fn take<'a>(&'a mut self, key: &ItemKey) -> impl Iterator<Item = TagItem> + use<'a> {
        self.take_filter(key, |_| true)
    }

    pub fn take_filter<'a, F>(
        &'a mut self,
        key: &ItemKey,
        mut filter: F,
    ) -> impl Iterator<Item = TagItem> + use<'a, F>
    where
        F: FnMut(&TagItem) -> bool,
    {
        // TODO: drain_filter
        let mut split_idx = 0;

        for read_idx in 0..self.items.len() {
            let item = &self.items[read_idx];
            if item.key() == key && filter(item) {
                self.items.swap(split_idx, read_idx);
                split_idx += 1;
            }
        }

        self.items.drain(..split_idx)
    }

    pub fn take_strings<'a>(&'a mut self, key: &ItemKey) -> impl Iterator<Item = String> + use<'a> {
        self.take(key).filter_map(|i| i.item_value.into_string())
    }

    pub fn get_items<'a>(&'a self, key: &'a ItemKey) -> impl Iterator<Item = &'a TagItem> + Clone {
        self.items.iter().filter(move |i| i.key() == key)
    }

    pub fn get_strings<'a>(&'a self, key: &'a ItemKey) -> impl Iterator<Item = &'a str> + Clone {
        self.items.iter().filter_map(move |i| {
            if i.key() == key {
                i.value().text()
            } else {
                None
            }
        })
    }

    pub fn get_locators<'a>(&'a self, key: &'a ItemKey) -> impl Iterator<Item = &'a str> + Clone {
        self.items.iter().filter_map(move |i| {
            if i.key() == key {
                i.value().locator()
            } else {
                None
            }
        })
    }

    pub fn get_bytes<'a>(&'a self, key: &'a ItemKey) -> impl Iterator<Item = &'a [u8]> + Clone {
        self.items.iter().filter_map(move |i| {
            if i.key() == key {
                i.value().binary()
            } else {
                None
            }
        })
    }

    pub fn remove_key(&mut self, key: &ItemKey) {
        self.items.retain(|i| i.key() != key)
    }

    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&TagItem) -> bool,
    {
        self.items.retain(f)
    }

    pub fn remove_empty(&mut self) {
        self.items.retain(|item| !item.value().is_empty());
    }

    pub fn pictures(&self) -> &[Picture] {
        &self.pictures
    }

    pub fn get_picture_type(&self, picture_type: PictureType) -> Option<&Picture> {
        self.pictures
            .iter()
            .find(|picture| picture.pic_type() == picture_type)
    }

    pub fn push_picture(&mut self, picture: Picture) {
        self.pictures.push(picture)
    }

    pub fn remove_picture_type(&mut self, picture_type: PictureType) {
        self.pictures.retain(|p| p.pic_type != picture_type)
    }

    pub fn set_picture(&mut self, index: usize, picture: Picture) {
        if index >= self.pictures.len() {
            self.push_picture(picture);
        } else {
            self.pictures[index] = picture;
        }
    }

    pub fn remove_picture(&mut self, index: usize) -> Picture {
        self.pictures.remove(index)
    }
}

impl TagExt for Tag {
    type Err = LoftyError;
    type RefKey<'a> = &'a ItemKey;

    #[inline]
    fn tag_type(&self) -> TagType {
        self.tag_type
    }

    fn len(&self) -> usize {
        self.items.len() + self.pictures.len()
    }

    fn contains<'a>(&'a self, key: Self::RefKey<'a>) -> bool {
        self.items.iter().any(|item| item.key() == key)
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty() && self.pictures.is_empty()
    }

    fn clear(&mut self) {
        self.items.clear();
        self.pictures.clear();
    }
}

#[derive(Debug, Clone, Default)]

pub struct SplitTagRemainder;

impl SplitTag for Tag {
    type Remainder = SplitTagRemainder;

    fn split_tag(self) -> (Self::Remainder, Self) {
        (SplitTagRemainder, self)
    }
}

impl MergeTag for SplitTagRemainder {
    type Merged = Tag;

    fn merge_tag(self, tag: Tag) -> Self::Merged {
        tag
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/audio_tags/tag/mod/tests.rs"]
mod tests;
