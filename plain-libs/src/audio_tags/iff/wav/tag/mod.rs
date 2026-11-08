use crate::audio_tags::tag::text_accessors;
pub(super) mod read;

use crate::audio_tags::error::{LoftyError, Result};
use crate::audio_tags::tag::{
    Accessor, ItemKey, ItemValue, MergeTag, SplitTag, Tag, TagExt, TagItem, TagType, try_parse_year,
};

use std::borrow::Cow;
use std::io::Write;



#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct RiffInfoList {
    pub(crate) items: Vec<(String, String)>,
}

impl RiffInfoList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.items
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    pub fn insert(&mut self, key: String, value: String) {
        if read::verify_key(key.as_str()) {
            self.items
                .iter()
                .position(|(k, _)| k.eq_ignore_ascii_case(key.as_str()))
                .map(|p| self.items.remove(p));
            self.items.push((key, value))
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<String> {
        if let Some((_, value)) = self
            .items
            .iter()
            .position(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|p| self.items.remove(p))
        {
            return Some(value);
        }

        None
    }
}

impl Accessor for RiffInfoList {
    text_accessors!(
        artist, set_artist, remove_artist => "IART";
        title, set_title, remove_title => "INAM";
        album, set_album, remove_album => "IPRD";
        genre, set_genre, remove_genre => "IGNR";
        comment, set_comment, remove_comment => "ICMT";
    );

    fn track(&self) -> Option<u32> {
        if let Some(item) = self.get("IPRT") {
            return item.parse::<u32>().ok();
        }

        None
    }

    fn set_track(&mut self, value: u32) {
        self.insert(String::from("IPRT"), value.to_string());
    }

    fn remove_track(&mut self) {
        self.remove("IPRT");
    }

    fn track_total(&self) -> Option<u32> {
        if let Some(item) = self.get("IFRM") {
            return item.parse::<u32>().ok();
        }

        None
    }

    fn set_track_total(&mut self, value: u32) {
        self.insert(String::from("IFRM"), value.to_string());
    }

    fn remove_track_total(&mut self) {
        self.remove("IFRM");
    }

    fn year(&self) -> Option<u32> {
        if let Some(item) = self.get("ICRD") {
            return try_parse_year(item);
        }

        None
    }

    fn set_year(&mut self, value: u32) {
        self.insert(String::from("ICRD"), value.to_string());
    }

    fn remove_year(&mut self) {
        let _ = self.remove("ICRD");
    }
}

impl IntoIterator for RiffInfoList {
    type Item = (String, String);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a> IntoIterator for &'a RiffInfoList {
    type Item = &'a (String, String);
    type IntoIter = std::slice::Iter<'a, (String, String)>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

impl TagExt for RiffInfoList {
    type Err = LoftyError;
    type RefKey<'a> = &'a str;

    #[inline]
    fn tag_type(&self) -> TagType {
        TagType::RiffInfo
    }

    fn len(&self) -> usize {
        self.items.len()
    }

    fn contains<'a>(&'a self, key: Self::RefKey<'a>) -> bool {
        self.items
            .iter()
            .any(|(item_key, _)| item_key.eq_ignore_ascii_case(key))
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn clear(&mut self) {
        self.items.clear();
    }
}

#[derive(Debug, Clone, Default)]
pub struct SplitTagRemainder;

impl SplitTag for RiffInfoList {
    type Remainder = SplitTagRemainder;

    fn split_tag(self) -> (Self::Remainder, Tag) {
        (SplitTagRemainder, self.into())
    }
}

impl MergeTag for SplitTagRemainder {
    type Merged = RiffInfoList;

    fn merge_tag(self, tag: Tag) -> Self::Merged {
        tag.into()
    }
}

impl From<RiffInfoList> for Tag {
    fn from(input: RiffInfoList) -> Self {
        let mut tag = Self::new(TagType::RiffInfo);

        for (k, v) in input.items {
            let item_key = ItemKey::from_key(TagType::RiffInfo, &k);

            tag.items.push(TagItem::new(
                item_key,
                ItemValue::Text(v.trim_matches('\0').to_string()),
            ));
        }

        tag
    }
}

impl From<Tag> for RiffInfoList {
    fn from(input: Tag) -> Self {
        let mut riff_info = RiffInfoList::default();

        for item in input.items {
            if let ItemValue::Text(val) | ItemValue::Locator(val) = item.item_value {
                match item.item_key {
                    ItemKey::Unknown(unknown) => {
                        if read::verify_key(&unknown) {
                            riff_info.items.push((unknown, val))
                        }
                    }
                    k => {
                        if let Some(key) = k.map_key(TagType::RiffInfo, false) {
                            riff_info.items.push((key.to_string(), val))
                        }
                    }
                }
            }
        }

        riff_info
    }
}

pub(crate) struct RIFFInfoListRef<'a, I>
where
    I: Iterator<Item = (&'a str, &'a str)>,
{
    pub(crate) items: I,
}

impl<'a, I> RIFFInfoListRef<'a, I>
where
    I: Iterator<Item = (&'a str, &'a str)>,
{
    pub(crate) fn new(items: I) -> RIFFInfoListRef<'a, I> {
        RIFFInfoListRef { items }
    }
}

pub(crate) fn tagitems_into_riff<'a>(
    items: impl IntoIterator<Item = &'a TagItem>,
) -> impl Iterator<Item = (&'a str, &'a str)> {
    items.into_iter().filter_map(|i| {
        let item_key = i.key().map_key(TagType::RiffInfo, true);

        match (item_key, i.value()) {
            (Some(key), ItemValue::Text(val) | ItemValue::Locator(val))
                if read::verify_key(key) =>
            {
                Some((key, val.as_str()))
            }
            _ => None,
        }
    })
}

#[cfg(test)]
#[path = "../../../../../../tests/plain-libs/unit/audio_tags/iff/wav/tag/mod/tests.rs"]
mod tests;
