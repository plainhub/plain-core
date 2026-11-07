mod audio_file;
mod file_type;
mod tagged_file;

pub use audio_file::AudioFile;
pub use file_type::FileType;
pub use tagged_file::{TaggedFile, TaggedFileExt};

pub(crate) use file_type::FileTypeGuessResult;
