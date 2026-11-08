pub(crate) mod constants;
pub(crate) mod opus;
pub(crate) mod page;
mod picture_storage;
pub(crate) mod read;
pub(crate) mod speex;
pub(crate) mod tag;
pub(crate) mod vorbis;

use crate::audio_tags::error::Result;
use crate::audio_tags::macros::decode_err;

use page::find_last_page;

// Exports

pub use opus::OpusFile;
pub use opus::properties::OpusProperties;
pub use picture_storage::OggPictureStorage;
pub use speex::SpeexFile;
pub use speex::properties::SpeexProperties;
pub use tag::VorbisComments;
pub use vorbis::VorbisFile;
pub use vorbis::properties::VorbisProperties;

fn verify_signature(content: &[u8], sig: &[u8]) -> Result<()> {
    let sig_len = sig.len();

    if content.len() < sig_len || &content[..sig_len] != sig {
        decode_err!(@BAIL Vorbis, "File missing magic signature");
    }

    Ok(())
}
