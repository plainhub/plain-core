use crate::audio_tags::config::ParseOptions;
use crate::audio_tags::error::Result;
use crate::audio_tags::file::{AudioFile, FileType, TaggedFile};
use crate::audio_tags::tag::TagType;

use std::collections::HashMap;
use std::io::{Read, Seek};
use std::marker::PhantomData;
use std::sync::{Arc, Mutex, OnceLock};

pub trait FileResolver: Send + Sync + AudioFile {
    fn extension() -> Option<&'static str>;

    fn primary_tag_type() -> TagType;

    fn supported_tag_types() -> &'static [TagType];

    fn guess(buf: &[u8]) -> Option<FileType>;
}

// Just broken out to its own type to make `CUSTOM_RESOLVER`'s type shorter :)
type ResolverMap = HashMap<&'static str, &'static dyn ObjectSafeFileResolver>;

pub(crate) fn custom_resolvers() -> &'static Arc<Mutex<ResolverMap>> {
    static INSTANCE: OnceLock<Arc<Mutex<ResolverMap>>> = OnceLock::new();
    INSTANCE.get_or_init(Default::default)
}

pub(crate) fn lookup_resolver(name: &'static str) -> &'static dyn ObjectSafeFileResolver {
    let res = custom_resolvers().lock().unwrap();

    if let Some(resolver) = res.get(name).copied() {
        return resolver;
    }

    panic!(
        "Encountered an unregistered custom `FileType` named `{}`",
        name
    );
}

// A `Read + Seek` supertrait for use in [`ObjectSafeFileResolver::read_from`]
pub(crate) trait SeekRead: Read + Seek {}
impl<T: Seek + Read> SeekRead for T {}

// `FileResolver` isn't object safe itself, so we need this wrapper trait
pub(crate) trait ObjectSafeFileResolver: Send + Sync {
    fn extension(&self) -> Option<&'static str>;
    fn primary_tag_type(&self) -> TagType;
    fn supported_tag_types(&self) -> &'static [TagType];
    fn guess(&self, buf: &[u8]) -> Option<FileType>;

    // A mask for the `AudioFile::read_from` impl
    fn read_from(
        &self,
        reader: &mut dyn SeekRead,
        parse_options: ParseOptions,
    ) -> Result<TaggedFile>;
}

// A fake `FileResolver` implementer, so we don't need to construct the type in `register_custom_resolver`
pub(crate) struct GhostlyResolver<T: 'static>(PhantomData<T>);
impl<T: FileResolver> ObjectSafeFileResolver for GhostlyResolver<T> {
    fn extension(&self) -> Option<&'static str> {
        T::extension()
    }

    fn primary_tag_type(&self) -> TagType {
        T::primary_tag_type()
    }

    fn supported_tag_types(&self) -> &'static [TagType] {
        T::supported_tag_types()
    }

    fn guess(&self, buf: &[u8]) -> Option<FileType> {
        T::guess(buf)
    }

    fn read_from(
        &self,
        reader: &mut dyn SeekRead,
        parse_options: ParseOptions,
    ) -> Result<TaggedFile> {
        Ok(<T as AudioFile>::read_from(&mut Box::new(reader), parse_options)?.into())
    }
}

pub fn register_custom_resolver<T: FileResolver + 'static>(name: &'static str) {
    let mut res = custom_resolvers().lock().unwrap();
    assert!(
        res.iter().all(|(n, _)| *n != name),
        "Resolver `{}` already exists!",
        name
    );

    let ghost = GhostlyResolver::<T>(PhantomData);
    let b: Box<dyn ObjectSafeFileResolver> = Box::new(ghost);

    res.insert(name, Box::leak::<'static>(b));
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/audio_tags/resolve/tests.rs"]
mod tests;
