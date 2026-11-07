use super::audio_file::AudioFile;
use super::file_type::FileType;
use crate::audio_tags::config::ParseOptions;
use crate::audio_tags::error::{LoftyError, Result};
use crate::audio_tags::properties::FileProperties;
use crate::audio_tags::tag::{Tag, TagExt, TagType};

use std::fs::File;
use std::io::{Read, Seek};

pub trait TaggedFileExt {
    fn file_type(&self) -> FileType;

    fn tags(&self) -> &[Tag];

    fn primary_tag_type(&self) -> TagType {
        self.file_type().primary_tag_type()
    }

    fn supports_tag_type(&self, tag_type: TagType) -> bool {
        self.file_type().supports_tag_type(tag_type)
    }

    fn tag(&self, tag_type: TagType) -> Option<&Tag>;

    fn tag_mut(&mut self, tag_type: TagType) -> Option<&mut Tag>;

    fn primary_tag(&self) -> Option<&Tag> {
        self.tag(self.primary_tag_type())
    }

    fn primary_tag_mut(&mut self) -> Option<&mut Tag> {
        self.tag_mut(self.primary_tag_type())
    }

    fn first_tag(&self) -> Option<&Tag> {
        self.tags().first()
    }

    fn first_tag_mut(&mut self) -> Option<&mut Tag>;

    fn insert_tag(&mut self, tag: Tag) -> Option<Tag>;

    fn remove(&mut self, tag_type: TagType) -> Option<Tag>;

    fn clear(&mut self);
}

pub struct TaggedFile {
    pub(crate) ty: FileType,

    pub(crate) properties: FileProperties,

    pub(crate) tags: Vec<Tag>,
}

impl TaggedFile {
    #[must_use]
    pub const fn new(ty: FileType, properties: FileProperties, tags: Vec<Tag>) -> Self {
        Self {
            ty,
            properties,
            tags,
        }
    }

    pub fn change_file_type(&mut self, file_type: FileType) {
        self.ty = file_type;
        self.properties = FileProperties::default();
        self.tags
            .retain(|t| self.ty.supports_tag_type(t.tag_type()));
    }
}

impl TaggedFileExt for TaggedFile {
    fn file_type(&self) -> FileType {
        self.ty
    }

    fn tags(&self) -> &[Tag] {
        self.tags.as_slice()
    }

    fn tag(&self, tag_type: TagType) -> Option<&Tag> {
        self.tags.iter().find(|i| i.tag_type() == tag_type)
    }

    fn tag_mut(&mut self, tag_type: TagType) -> Option<&mut Tag> {
        self.tags.iter_mut().find(|i| i.tag_type() == tag_type)
    }

    fn first_tag_mut(&mut self) -> Option<&mut Tag> {
        self.tags.first_mut()
    }

    fn insert_tag(&mut self, tag: Tag) -> Option<Tag> {
        let tag_type = tag.tag_type();

        if self.supports_tag_type(tag_type) {
            let ret = self.remove(tag_type);
            self.tags.push(tag);

            return ret;
        }

        None
    }

    fn remove(&mut self, tag_type: TagType) -> Option<Tag> {
        self.tags
            .iter()
            .position(|t| t.tag_type() == tag_type)
            .map(|pos| self.tags.remove(pos))
    }

    fn clear(&mut self) {
        self.tags.clear()
    }
}

impl AudioFile for TaggedFile {
    type Properties = FileProperties;

    fn read_from<R>(reader: &mut R, parse_options: ParseOptions) -> Result<Self>
    where
        R: Read + Seek,
        Self: Sized,
    {
        crate::audio_tags::probe::Probe::new(reader)
            .guess_file_type()?
            .options(parse_options)
            .read()
    }

    fn properties(&self) -> &Self::Properties {
        &self.properties
    }
}
