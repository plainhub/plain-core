pub(crate) mod item;
pub(crate) mod read;

use crate::audio_tags::ape::tag::item::{ApeItem, ApeItemRef};
use crate::audio_tags::error::{LoftyError, Result};
use crate::audio_tags::id3::v2::util::pairs::{NUMBER_PAIR_KEYS, format_number_pair, set_number};
use crate::audio_tags::tag::item::ItemValueRef;
use crate::audio_tags::tag::{
    Accessor, ItemKey, ItemValue, MergeTag, SplitTag, Tag, TagExt, TagItem, TagType, try_parse_year,
};
use crate::audio_tags::util::flag_item;

use std::borrow::Cow;
use std::io::Write;
use std::ops::Deref;


macro_rules! impl_accessor {
	($($name:ident => $($key:literal)|+;)+) => {
		paste::paste! {
			$(
				fn $name(&self) -> Option<Cow<'_, str>> {
					$(
						if let Some(i) = self.get($key) {
							if let ItemValue::Text(val) = i.value() {
								return Some(Cow::Borrowed(val));
							}
						}
					)+

					None
				}

				fn [<set_ $name>](&mut self, value: String) {
					self.insert(ApeItem {
						read_only: false,
						key: String::from(crate::audio_tags::tag::item::first_key!($($key)|*)),
						value: ItemValue::Text(value)
					})
				}

				fn [<remove_ $name>](&mut self) {
					$(
						self.remove($key);
					)+
				}
			)+
		}
	}
}

#[derive(Default, Debug, PartialEq, Eq, Clone)]
pub struct ApeTag {
    pub read_only: bool,
    pub(super) items: Vec<ApeItem>,
}

impl ApeTag {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &str) -> Option<&ApeItem> {
        self.items
            .iter()
            .find(|i| i.key().eq_ignore_ascii_case(key))
    }

    pub fn insert(&mut self, value: ApeItem) {
        self.remove(value.key());
        self.items.push(value);
    }

    pub fn remove(&mut self, key: &str) {
        self.items.retain(|i| !i.key().eq_ignore_ascii_case(key));
    }

    fn insert_item(&mut self, item: TagItem) {
        match item.key() {
            ItemKey::TrackNumber => set_number(&item, |number| self.set_track(number)),
            ItemKey::TrackTotal => set_number(&item, |number| self.set_track_total(number)),
            ItemKey::DiscNumber => set_number(&item, |number| self.set_disk(number)),
            ItemKey::DiscTotal => set_number(&item, |number| self.set_disk_total(number)),

            // Normalize flag items
            ItemKey::FlagCompilation => {
                let Some(text) = item.item_value.text() else {
                    return;
                };

                let Some(flag) = flag_item(text) else {
                    return;
                };

                let value = u8::from(flag).to_string();
                self.insert(ApeItem::text("Compilation", value));
            }
            _ => {
                if let Ok(item) = item.try_into() {
                    self.insert(item);
                }
            }
        }
    }

    fn split_num_pair(&self, key: &str) -> (Option<u32>, Option<u32>) {
        if let Some(ApeItem {
            value: ItemValue::Text(text),
            ..
        }) = self.get(key)
        {
            let mut split = text.split('/').flat_map(str::parse::<u32>);
            return (split.next(), split.next());
        }

        (None, None)
    }

    fn insert_number_pair(&mut self, key: &'static str, number: Option<u32>, total: Option<u32>) {
        if let Some(value) = format_number_pair(number, total) {
            self.insert(ApeItem::text(key, value));
        } else {
            log::warn!("{key} is not set. number: {number:?}, total: {total:?}");
        }
    }
}

impl IntoIterator for ApeTag {
    type Item = ApeItem;
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.into_iter()
    }
}

impl<'a> IntoIterator for &'a ApeTag {
    type Item = &'a ApeItem;
    type IntoIter = std::slice::Iter<'a, ApeItem>;

    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

impl Accessor for ApeTag {
    impl_accessor!(
        artist  => "Artist";
        title   => "Title";
        album   => "Album";
        genre   => "GENRE";
        comment => "Comment";
    );

    fn track(&self) -> Option<u32> {
        self.split_num_pair("Track").0
    }

    fn set_track(&mut self, value: u32) {
        self.insert_number_pair("Track", Some(value), self.track_total());
    }

    fn remove_track(&mut self) {
        self.remove("Track");
    }

    fn track_total(&self) -> Option<u32> {
        self.split_num_pair("Track").1
    }

    fn set_track_total(&mut self, value: u32) {
        self.insert_number_pair("Track", self.track(), Some(value));
    }

    fn remove_track_total(&mut self) {
        let existing_track_number = self.track();
        self.remove("Track");

        if let Some(track) = existing_track_number {
            self.insert(ApeItem::text("Track", track.to_string()));
        }
    }

    fn disk(&self) -> Option<u32> {
        self.split_num_pair("Disc").0
    }

    fn set_disk(&mut self, value: u32) {
        self.insert_number_pair("Disc", Some(value), self.disk_total());
    }

    fn remove_disk(&mut self) {
        self.remove("Disc");
    }

    fn disk_total(&self) -> Option<u32> {
        self.split_num_pair("Disc").1
    }

    fn set_disk_total(&mut self, value: u32) {
        self.insert_number_pair("Disc", self.disk(), Some(value));
    }

    fn remove_disk_total(&mut self) {
        let existing_track_number = self.track();
        self.remove("Disc");

        if let Some(track) = existing_track_number {
            self.insert(ApeItem::text("Disc", track.to_string()));
        }
    }

    fn year(&self) -> Option<u32> {
        if let Some(ApeItem {
            value: ItemValue::Text(text),
            ..
        }) = self.get("Year")
        {
            return try_parse_year(text);
        }

        None
    }

    fn set_year(&mut self, value: u32) {
        self.insert(ApeItem::text("Year", value.to_string()));
    }

    fn remove_year(&mut self) {
        self.remove("Year");
    }
}

impl TagExt for ApeTag {
    type Err = LoftyError;
    type RefKey<'a> = &'a str;

    #[inline]
    fn tag_type(&self) -> TagType {
        TagType::Ape
    }

    fn len(&self) -> usize {
        self.items.len()
    }

    fn contains<'a>(&'a self, key: Self::RefKey<'a>) -> bool {
        self.items.iter().any(|i| i.key().eq_ignore_ascii_case(key))
    }

    fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    fn clear(&mut self) {
        self.items.clear();
    }
}

#[derive(Debug, Clone, Default)]
pub struct SplitTagRemainder(ApeTag);

impl From<SplitTagRemainder> for ApeTag {
    fn from(from: SplitTagRemainder) -> Self {
        from.0
    }
}

impl Deref for SplitTagRemainder {
    type Target = ApeTag;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SplitTag for ApeTag {
    type Remainder = SplitTagRemainder;

    fn split_tag(mut self) -> (Self::Remainder, Tag) {
        fn split_pair(
            content: &str,
            tag: &mut Tag,
            current_key: ItemKey,
            total_key: ItemKey,
        ) -> Option<()> {
            let mut split = content.splitn(2, '/');
            let current = split.next()?.to_string();
            tag.items
                .push(TagItem::new(current_key, ItemValue::Text(current)));

            if let Some(total) = split.next() {
                tag.items
                    .push(TagItem::new(total_key, ItemValue::Text(total.to_string())))
            }

            Some(())
        }

        let mut tag = Tag::new(TagType::Ape);

        for item in std::mem::take(&mut self.items) {
            let item_key = ItemKey::from_key(TagType::Ape, item.key());

            // The text pairs need some special treatment
            match (item_key, item.value()) {
                (ItemKey::TrackNumber | ItemKey::TrackTotal, ItemValue::Text(val))
                    if split_pair(val, &mut tag, ItemKey::TrackNumber, ItemKey::TrackTotal)
                        .is_some() =>
                {
                    continue; // Item consumed
                }
                (ItemKey::DiscNumber | ItemKey::DiscTotal, ItemValue::Text(val))
                    if split_pair(val, &mut tag, ItemKey::DiscNumber, ItemKey::DiscTotal)
                        .is_some() =>
                {
                    continue; // Item consumed
                }
                (ItemKey::MovementNumber | ItemKey::MovementTotal, ItemValue::Text(val))
                    if split_pair(
                        val,
                        &mut tag,
                        ItemKey::MovementNumber,
                        ItemKey::MovementTotal,
                    )
                    .is_some() =>
                {
                    continue; // Item consumed
                }
                (k, _) => {
                    tag.items.push(TagItem::new(k, item.value));
                }
            }
        }

        (SplitTagRemainder(self), tag)
    }
}

impl MergeTag for SplitTagRemainder {
    type Merged = ApeTag;

    fn merge_tag(self, tag: Tag) -> Self::Merged {
        let Self(mut merged) = self;

        for item in tag.items {
            merged.insert_item(item);
        }

        for pic in tag.pictures {
            if let Some(key) = pic.pic_type.as_ape_key() {
                if let Ok(item) =
                    ApeItem::new(key.to_string(), ItemValue::Binary(pic.as_ape_bytes()))
                {
                    merged.insert(item)
                }
            }
        }

        merged
    }
}

impl From<ApeTag> for Tag {
    fn from(input: ApeTag) -> Self {
        input.split_tag().1
    }
}

impl From<Tag> for ApeTag {
    fn from(input: Tag) -> Self {
        SplitTagRemainder::default().merge_tag(input)
    }
}

pub(crate) struct ApeTagRef<'a, I>
where
    I: Iterator<Item = ApeItemRef<'a>>,
{
    pub(crate) read_only: bool,
    pub(crate) items: I,
}

impl<'a, I> ApeTagRef<'a, I> where I: Iterator<Item = ApeItemRef<'a>> {}

pub(crate) fn tagitems_into_ape(tag: &Tag) -> impl Iterator<Item = ApeItemRef<'_>> {
    fn create_apeitemref_for_number_pair<'a>(
        number: Option<&str>,
        total: Option<&str>,
        key: &'a str,
    ) -> Option<ApeItemRef<'a>> {
        format_number_pair(number, total).map(|value| ApeItemRef {
            read_only: false,
            key,
            value: ItemValueRef::Text(Cow::Owned(value)),
        })
    }

    tag.items()
        .filter(|item| !NUMBER_PAIR_KEYS.contains(item.key()))
        .filter_map(|i| {
            i.key().map_key(TagType::Ape, true).map(|key| ApeItemRef {
                read_only: false,
                key,
                value: (&i.item_value).into(),
            })
        })
        .chain(create_apeitemref_for_number_pair(
            tag.get_string(&ItemKey::TrackNumber),
            tag.get_string(&ItemKey::TrackTotal),
            "Track",
        ))
        .chain(create_apeitemref_for_number_pair(
            tag.get_string(&ItemKey::DiscNumber),
            tag.get_string(&ItemKey::DiscTotal),
            "Disk",
        ))
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/audio_tags/ape/tag/mod/tests.rs"]
mod tests;
