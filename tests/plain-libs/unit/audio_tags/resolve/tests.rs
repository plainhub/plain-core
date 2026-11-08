use crate::audio_tags::config::{GlobalOptions, ParseOptions};
use crate::audio_tags::file::{AudioFile, FileType, TaggedFile, TaggedFileExt};
use crate::audio_tags::id3::v2::Id3v2Tag;
use crate::audio_tags::properties::FileProperties;
use crate::audio_tags::resolve::{FileResolver, register_custom_resolver};
use crate::audio_tags::tag::{Accessor, TagType};

use std::fs::File;
use std::io::{Read, Seek};
use std::panic;

struct MyFile {
    id3v2_tag: Option<Id3v2Tag>,
    properties: FileProperties,
}

impl FileResolver for MyFile {
    fn extension() -> Option<&'static str> {
        Some("myfile")
    }

    fn primary_tag_type() -> TagType {
        TagType::Id3v2
    }

    fn supported_tag_types() -> &'static [TagType] {
        &[TagType::Id3v2]
    }

    fn guess(buf: &[u8]) -> Option<FileType> {
        if buf.starts_with(b"myfile") {
            return Some(FileType::Custom("MyFile"));
        }

        None
    }
}

impl AudioFile for MyFile {
    type Properties = FileProperties;

    fn read_from<R: Read + Seek>(
        _reader: &mut R,
        _parse_options: ParseOptions,
    ) -> crate::audio_tags::error::Result<Self> {
        let mut tag = Id3v2Tag::default();
        tag.set_artist(String::from("All is well!"));

        Ok(Self {
            id3v2_tag: Some(tag),
            properties: FileProperties::default(),
        })
    }
    fn properties(&self) -> &Self::Properties {
        &self.properties
    }
}

impl From<MyFile> for TaggedFile {
    fn from(file: MyFile) -> Self {
        Self::new(
            FileType::Custom("MyFile"),
            file.properties,
            file.id3v2_tag.into_iter().map(Into::into).collect(),
        )
    }
}

#[test]
fn custom_resolver() {
    register_custom_resolver::<MyFile>("MyFile");

    let global_options = GlobalOptions::new().use_custom_resolvers(true);
    crate::audio_tags::config::apply_global_options(global_options);

    let dir = crate::temporary::tempdir().unwrap();
    let path = dir.path().join("fixture.myfile");
    std::fs::write(&path, b"myfile").unwrap();
    let read = crate::audio_tags::read_from_path(&path).unwrap();
    assert_eq!(read.file_type(), FileType::Custom("MyFile"));

    let read_content = crate::audio_tags::read_from(&mut File::open(path).unwrap()).unwrap();
    assert_eq!(read_content.file_type(), FileType::Custom("MyFile"));

    assert!(
        panic::catch_unwind(|| {
            register_custom_resolver::<MyFile>("MyFile");
        })
        .is_err(),
        "We didn't panic on double register!"
    );
}
