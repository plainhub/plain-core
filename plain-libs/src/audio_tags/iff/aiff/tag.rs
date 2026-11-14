use crate::audio_tags::error::{LoftyError, Result};
use crate::audio_tags::iff::chunk::Chunks;
use crate::audio_tags::macros::err;
use crate::audio_tags::tag::{
    Accessor, ItemKey, ItemValue, MergeTag, SplitTag, Tag, TagExt, TagItem, TagType,
};

use std::borrow::Cow;
use std::convert::TryFrom;
use std::io::{SeekFrom, Write};

use crate::binary::BigEndian;

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct Comment {
    pub timestamp: u32,

    pub marker_id: u16,

    pub text: String,
}

#[derive(Default, Clone, Debug, PartialEq, Eq)]
pub struct AiffTextChunks {
    pub name: Option<String>,

    pub author: Option<String>,

    pub copyright: Option<String>,

    pub annotations: Option<Vec<String>>,

    pub comments: Option<Vec<Comment>>,
}

impl Accessor for AiffTextChunks {
    fn artist(&self) -> Option<Cow<'_, str>> {
        self.author.as_deref().map(Cow::Borrowed)
    }
    fn set_artist(&mut self, value: String) {
        self.author = Some(value)
    }
    fn remove_artist(&mut self) {
        self.author = None
    }

    fn title(&self) -> Option<Cow<'_, str>> {
        self.name.as_deref().map(Cow::Borrowed)
    }
    fn set_title(&mut self, value: String) {
        self.name = Some(value)
    }
    fn remove_title(&mut self) {
        self.name = None
    }

    fn comment(&self) -> Option<Cow<'_, str>> {
        if let Some(ref anno) = self.annotations {
            if !anno.is_empty() {
                return anno.first().map(String::as_str).map(Cow::Borrowed);
            }
        }

        if let Some(ref comm) = self.comments {
            return comm.first().map(|c| c.text.as_str()).map(Cow::Borrowed);
        }

        None
    }
    fn set_comment(&mut self, value: String) {
        self.annotations = Some(vec![value]);
    }
    fn remove_comment(&mut self) {
        self.annotations = None;
    }
}

impl AiffTextChunks {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn copyright(&self) -> Option<&str> {
        self.copyright.as_deref()
    }

    pub fn set_copyright(&mut self, value: String) {
        self.copyright = Some(value)
    }

    pub fn remove_copyright(&mut self) {
        self.copyright = None
    }
}

impl TagExt for AiffTextChunks {
    type Err = LoftyError;
    type RefKey<'a> = &'a ItemKey;

    #[inline]
    fn tag_type(&self) -> TagType {
        TagType::AiffText
    }

    fn len(&self) -> usize {
        usize::from(self.name.is_some())
            + usize::from(self.author.is_some())
            + usize::from(self.copyright.is_some())
            + self.annotations.as_ref().map_or(0, Vec::len)
            + self.comments.as_ref().map_or(0, Vec::len)
    }

    fn contains<'a>(&'a self, key: Self::RefKey<'a>) -> bool {
        match key {
            ItemKey::TrackTitle => self.name.is_some(),
            ItemKey::TrackArtist => self.author.is_some(),
            ItemKey::CopyrightMessage => self.copyright.is_some(),
            ItemKey::Comment => self.annotations.is_some() || self.comments.is_some(),
            _ => false,
        }
    }

    fn is_empty(&self) -> bool {
        matches!(
            self,
            AiffTextChunks {
                name: None,
                author: None,
                copyright: None,
                annotations: None,
                comments: None
            }
        )
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

#[derive(Debug, Clone, Default)]
pub struct SplitTagRemainder;

impl SplitTag for AiffTextChunks {
    type Remainder = SplitTagRemainder;

    fn split_tag(self) -> (Self::Remainder, Tag) {
        (SplitTagRemainder, self.into())
    }
}

impl MergeTag for SplitTagRemainder {
    type Merged = AiffTextChunks;

    fn merge_tag(self, tag: Tag) -> Self::Merged {
        tag.into()
    }
}

impl From<AiffTextChunks> for Tag {
    fn from(input: AiffTextChunks) -> Self {
        let mut tag = Self::new(TagType::AiffText);

        let push_item = |field: Option<String>, item_key: ItemKey, tag: &mut Tag| {
            if let Some(text) = field {
                tag.items
                    .push(TagItem::new(item_key, ItemValue::Text(text)))
            }
        };

        push_item(input.name, ItemKey::TrackTitle, &mut tag);
        push_item(input.author, ItemKey::TrackArtist, &mut tag);
        push_item(input.copyright, ItemKey::CopyrightMessage, &mut tag);

        if let Some(annotations) = input.annotations {
            for anno in annotations {
                tag.items
                    .push(TagItem::new(ItemKey::Comment, ItemValue::Text(anno)));
            }
        }

        if let Some(comments) = input.comments {
            for comt in comments {
                tag.items
                    .push(TagItem::new(ItemKey::Comment, ItemValue::Text(comt.text)));
            }
        }

        tag
    }
}

impl From<Tag> for AiffTextChunks {
    fn from(mut input: Tag) -> Self {
        let name = input.take_strings(&ItemKey::TrackTitle).next();
        let author = input.take_strings(&ItemKey::TrackArtist).next();
        let copyright = input.take_strings(&ItemKey::CopyrightMessage).next();
        let annotations = input.take_strings(&ItemKey::Comment).collect::<Vec<_>>();

        Self {
            name,
            author,
            copyright,
            annotations: (!annotations.is_empty()).then_some(annotations),
            comments: None,
        }
    }
}

pub(crate) struct AiffTextChunksRef<'a, T, AI>
where
    AI: IntoIterator<Item = T>,
{
    pub name: Option<&'a str>,
    pub author: Option<&'a str>,
    pub copyright: Option<&'a str>,
    pub annotations: Option<AI>,
    pub comments: Option<&'a [Comment]>,
}

impl<T, AI> AiffTextChunksRef<'_, T, AI>
where
    T: AsRef<str>,
    AI: IntoIterator<Item = T>,
{
    fn create_text_chunks(tag: &mut AiffTextChunksRef<'_, T, AI>) -> Result<Vec<u8>> {
        fn write_chunk(writer: &mut Vec<u8>, key: &str, value: Option<&str>) {
            if let Some(val) = value {
                if let Ok(len) = u32::try_from(val.len()) {
                    writer.extend(key.as_bytes());
                    writer.extend(len.to_be_bytes());
                    writer.extend(val.as_bytes());

                    // AIFF only needs a terminator if the string is on an odd boundary,
                    // unlike RIFF, which makes use of both C-strings and even boundaries
                    if len % 2 != 0 {
                        writer.push(0);
                    }
                }
            }
        }

        let mut text_chunks = Vec::new();

        if let Some(comments) = tag.comments.take() {
            if !comments.is_empty() {
                let comment_count = comments.len();

                if let Ok(len) = u16::try_from(comment_count) {
                    text_chunks.extend(b"COMT");
                    text_chunks.extend(len.to_be_bytes());

                    for comt in comments {
                        text_chunks.extend(comt.timestamp.to_be_bytes());
                        text_chunks.extend(comt.marker_id.to_be_bytes());

                        let comt_len = comt.text.len();

                        if comt_len > u16::MAX as usize {
                            err!(TooMuchData);
                        }

                        text_chunks.extend((comt_len as u16).to_be_bytes());
                        text_chunks.extend(comt.text.as_bytes());

                        if comt_len % 2 != 0 {
                            text_chunks.push(0);
                        }
                    }

                    // Get the size of the COMT chunk
                    let comt_len = text_chunks.len() - 4;

                    if let Ok(chunk_len) = u32::try_from(comt_len) {
                        let mut i = 4;

                        // Write the size back
                        for b in chunk_len.to_be_bytes() {
                            text_chunks.insert(i, b);
                            i += 1;
                        }
                    } else {
                        err!(TooMuchData);
                    }

                    if (text_chunks.len() - 4) % 2 != 0 {
                        text_chunks.push(0);
                    }
                }
            }
        }

        write_chunk(&mut text_chunks, "NAME", tag.name);
        write_chunk(&mut text_chunks, "AUTH", tag.author);
        write_chunk(&mut text_chunks, "(c) ", tag.copyright);

        if let Some(annotations) = tag.annotations.take() {
            for anno in annotations {
                write_chunk(&mut text_chunks, "ANNO", Some(anno.as_ref()));
            }
        }

        log::debug!(
            "Created AIFF text chunks, size: {} bytes",
            text_chunks.len()
        );
        Ok(text_chunks)
    }
}

#[cfg(test)]
#[path = "../../../../../tests/plain-libs/unit/audio_tags/iff/aiff/tag/tests.rs"]
mod tests;
