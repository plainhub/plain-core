use crate::audio_tags::error::{LoftyError, Result};
use crate::audio_tags::file::FileType;
use crate::audio_tags::macros::err;
use crate::audio_tags::ogg::picture_storage::OggPictureStorage;
use crate::audio_tags::picture::{Picture, PictureInformation};
use crate::audio_tags::probe::Probe;
use crate::audio_tags::tag::{
    Accessor, ItemKey, ItemValue, MergeTag, SplitTag, Tag, TagExt, TagItem, TagType, try_parse_year,
};
use crate::audio_tags::util::flag_item;

use std::borrow::Cow;
use std::io::Write;
use std::ops::Deref;


macro_rules! impl_accessor {
	($($name:ident => $key:literal;)+) => {
		paste::paste! {
			$(
				fn $name(&self) -> Option<Cow<'_, str>> {
					self.get($key).map(Cow::Borrowed)
				}

				fn [<set_ $name>](&mut self, value: String) {
					self.insert(String::from($key), value)
				}

				fn [<remove_ $name>](&mut self) {
					let _ = self.remove($key);
				}
			)+
		}
	}
}

#[derive(Default, PartialEq, Eq, Debug, Clone)]
pub struct VorbisComments {
    pub(crate) vendor: String,

    pub(crate) items: Vec<(String, String)>,

    pub(crate) pictures: Vec<(Picture, PictureInformation)>,
}

impl VorbisComments {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn vendor(&self) -> &str {
        &self.vendor
    }

    pub fn set_vendor(&mut self, vendor: String) {
        self.vendor = vendor
    }

    pub fn items(&self) -> impl ExactSizeIterator<Item = (&str, &str)> + Clone {
        self.items.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    pub fn take_items(&mut self) -> impl ExactSizeIterator<Item = (String, String)> + use<> {
        let items = std::mem::take(&mut self.items);
        items.into_iter()
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        if !verify_key(key) {
            return None;
        }

        self.items
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(key))
            .map(|(_, v)| v.as_str())
    }

    pub fn get_all<'a>(&'a self, key: &'a str) -> impl Iterator<Item = &'a str> + Clone + 'a {
        self.items
            .iter()
            .filter_map(move |(k, v)| (k.eq_ignore_ascii_case(key)).then_some(v.as_str()))
    }

    pub fn insert(&mut self, key: String, value: String) {
        if !verify_key(&key) {
            return;
        }

        self.items.retain(|(k, _)| !k.eq_ignore_ascii_case(&key));
        self.items.push((key, value))
    }

    pub fn push(&mut self, key: String, value: String) {
        if !verify_key(&key) {
            return;
        }

        self.items.push((key, value))
    }

    pub fn remove<'a>(&'a mut self, key: &str) -> impl Iterator<Item = String> + use<'a> {
        // TODO: drain_filter
        let mut split_idx = 0_usize;

        for read_idx in 0..self.items.len() {
            if self.items[read_idx].0.eq_ignore_ascii_case(key) {
                self.items.swap(split_idx, read_idx);
                split_idx += 1;
            }
        }

        self.items.drain(..split_idx).map(|(_, v)| v)
    }
}

// A case-insensitive field name that may consist of ASCII 0x20 through 0x7D, 0x3D ('=') excluded.
// ASCII 0x41 through 0x5A inclusive (A-Z) is to be considered equivalent to ASCII 0x61 through 0x7A inclusive (a-z).
fn verify_key(key: &str) -> bool {
    if key.is_empty() {
        return false;
    }

    key.bytes()
        .all(|byte| (0x20..=0x7D).contains(&byte) && byte != 0x3D)
}

impl OggPictureStorage for VorbisComments {
    fn pictures(&self) -> &[(Picture, PictureInformation)] {
        &self.pictures
    }
}

impl Accessor for VorbisComments {
    impl_accessor!(
        artist  => "ARTIST";
        title   => "TITLE";
        album   => "ALBUM";
        genre   => "GENRE";
        comment => "COMMENT";
    );

    fn track(&self) -> Option<u32> {
        if let Some(item) = self
            .get("TRACKNUMBER")
            .map_or_else(|| self.get("TRACKNUM"), Some)
        {
            return item.parse::<u32>().ok();
        }

        None
    }

    fn set_track(&mut self, value: u32) {
        self.remove_track();
        self.insert(String::from("TRACKNUMBER"), value.to_string());
    }

    fn remove_track(&mut self) {
        let _ = self.remove("TRACKNUMBER");
        let _ = self.remove("TRACKNUM");
    }

    fn track_total(&self) -> Option<u32> {
        if let Some(item) = self
            .get("TRACKTOTAL")
            .map_or_else(|| self.get("TOTALTRACKS"), Some)
        {
            return item.parse::<u32>().ok();
        }

        None
    }

    fn set_track_total(&mut self, value: u32) {
        self.insert(String::from("TRACKTOTAL"), value.to_string());
        let _ = self.remove("TOTALTRACKS");
    }

    fn remove_track_total(&mut self) {
        let _ = self.remove("TRACKTOTAL");
        let _ = self.remove("TOTALTRACKS");
    }

    fn disk(&self) -> Option<u32> {
        if let Some(item) = self.get("DISCNUMBER") {
            return item.parse::<u32>().ok();
        }

        None
    }

    fn set_disk(&mut self, value: u32) {
        self.insert(String::from("DISCNUMBER"), value.to_string());
    }

    fn remove_disk(&mut self) {
        let _ = self.remove("DISCNUMBER");
    }

    fn disk_total(&self) -> Option<u32> {
        if let Some(item) = self
            .get("DISCTOTAL")
            .map_or_else(|| self.get("TOTALDISCS"), Some)
        {
            return item.parse::<u32>().ok();
        }

        None
    }

    fn set_disk_total(&mut self, value: u32) {
        self.insert(String::from("DISCTOTAL"), value.to_string());
        let _ = self.remove("TOTALDISCS");
    }

    fn remove_disk_total(&mut self) {
        let _ = self.remove("DISCTOTAL");
        let _ = self.remove("TOTALDISCS");
    }

    fn year(&self) -> Option<u32> {
        if let Some(item) = self.get("YEAR").map_or_else(|| self.get("DATE"), Some) {
            return try_parse_year(item);
        }

        None
    }

    fn set_year(&mut self, value: u32) {
        // DATE is the preferred way of storing the year, but it is still possible we will
        // encounter YEAR
        self.insert(String::from("DATE"), value.to_string());
        let _ = self.remove("YEAR");
    }

    fn remove_year(&mut self) {
        // DATE is not valid without a year, so we can remove them as well
        let _ = self.remove("DATE");
        let _ = self.remove("YEAR");
    }
}

impl TagExt for VorbisComments {
    type Err = LoftyError;
    type RefKey<'a> = &'a str;

    #[inline]
    fn tag_type(&self) -> TagType {
        TagType::VorbisComments
    }

    fn len(&self) -> usize {
        self.items.len() + self.pictures.len()
    }

    fn contains<'a>(&'a self, key: Self::RefKey<'a>) -> bool {
        self.items
            .iter()
            .any(|(item_key, _)| item_key.eq_ignore_ascii_case(key))
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
pub struct SplitTagRemainder(VorbisComments);

impl From<SplitTagRemainder> for VorbisComments {
    fn from(from: SplitTagRemainder) -> Self {
        from.0
    }
}

impl Deref for SplitTagRemainder {
    type Target = VorbisComments;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SplitTag for VorbisComments {
    type Remainder = SplitTagRemainder;

    fn split_tag(mut self) -> (Self::Remainder, Tag) {
        let mut tag = Tag::new(TagType::VorbisComments);

        for (k, v) in std::mem::take(&mut self.items) {
            tag.items.push(TagItem::new(
                ItemKey::from_key(TagType::VorbisComments, &k),
                ItemValue::Text(v),
            ));
        }

        // We need to preserve the vendor string
        if !tag
            .items
            .iter()
            .any(|i| i.key() == &ItemKey::EncoderSoftware)
        {
            tag.items.push(TagItem::new(
                ItemKey::EncoderSoftware,
                // Preserve the original vendor by cloning
                ItemValue::Text(self.vendor.clone()),
            ));
        }

        for (pic, _info) in std::mem::take(&mut self.pictures) {
            tag.push_picture(pic)
        }

        (SplitTagRemainder(self), tag)
    }
}

impl MergeTag for SplitTagRemainder {
    type Merged = VorbisComments;

    fn merge_tag(self, mut tag: Tag) -> Self::Merged {
        let Self(mut merged) = self;

        if let Some(TagItem {
            item_value: ItemValue::Text(val),
            ..
        }) = tag.take(&ItemKey::EncoderSoftware).next()
        {
            merged.vendor = val;
        }

        for item in tag.items {
            let item_key = item.item_key;
            let item_value = item.item_value;

            // Discard binary items, as they are not allowed in Vorbis comments
            let (ItemValue::Text(mut val) | ItemValue::Locator(mut val)) = item_value else {
                continue;
            };

            // Normalize flag items
            if item_key == ItemKey::FlagCompilation {
                let Some(flag) = flag_item(&val) else {
                    continue;
                };

                val = u8::from(flag).to_string();
            }

            let key;
            match item_key {
                ItemKey::Unknown(unknown) => {
                    if !verify_key(&unknown) {
                        continue; // Bad key, discard the item
                    }

                    key = unknown
                }
                _ => match item_key.map_key(TagType::VorbisComments, false) {
                    Some(mapped_key) => key = mapped_key.to_string(),
                    None => continue, // No mapping exists, discard the item
                },
            }

            merged.items.push((key, val));
        }

        for picture in tag.pictures {
            if let Ok(information) = PictureInformation::from_picture(&picture) {
                merged.pictures.push((picture, information))
            }
        }

        merged
    }
}

impl From<VorbisComments> for Tag {
    fn from(input: VorbisComments) -> Self {
        input.split_tag().1
    }
}

impl From<Tag> for VorbisComments {
    fn from(input: Tag) -> Self {
        SplitTagRemainder::default().merge_tag(input)
    }
}

pub(crate) struct VorbisCommentsRef<'a, II, IP>
where
    II: Iterator<Item = (&'a str, &'a str)>,
    IP: Iterator<Item = (&'a Picture, PictureInformation)>,
{
    pub vendor: Cow<'a, str>,
    pub items: II,
    pub pictures: IP,
}

impl<'a, II, IP> VorbisCommentsRef<'a, II, IP>
where
    II: Iterator<Item = (&'a str, &'a str)>,
    IP: Iterator<Item = (&'a Picture, PictureInformation)>,
{
}

pub(crate) fn create_vorbis_comments_ref(
    tag: &Tag,
) -> (
    &str,
    impl Iterator<Item = (&str, &str)>,
    impl Iterator<Item = (&Picture, PictureInformation)>,
) {
    let vendor = tag.get_string(&ItemKey::EncoderSoftware).unwrap_or("");

    let items = tag.items.iter().filter_map(|i| match i.value() {
        ItemValue::Text(val) | ItemValue::Locator(val) => i
            .key()
            .map_key(TagType::VorbisComments, true)
            .map(|key| (key, val.as_str())),
        _ => None,
    });

    let pictures = tag
        .pictures
        .iter()
        .map(|p| (p, PictureInformation::from_picture(p).unwrap_or_default()));
    (vendor, items, pictures)
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/audio_tags/ogg/tag/tests.rs"]
mod tests;
