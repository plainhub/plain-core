use crate::audio_tags::aac::AacFile;
use crate::audio_tags::ape::ApeFile;
use crate::audio_tags::config::{ParseOptions, global_options};
use crate::audio_tags::error::Result;
use crate::audio_tags::file::{AudioFile, FileType, FileTypeGuessResult, TaggedFile};
use crate::audio_tags::flac::FlacFile;
use crate::audio_tags::iff::aiff::AiffFile;
use crate::audio_tags::iff::wav::WavFile;
use crate::audio_tags::macros::err;
use crate::audio_tags::mp4::Mp4File;
use crate::audio_tags::mpeg::MpegFile;
use crate::audio_tags::mpeg::header::search_for_frame_sync;
use crate::audio_tags::musepack::MpcFile;
use crate::audio_tags::ogg::opus::OpusFile;
use crate::audio_tags::ogg::speex::SpeexFile;
use crate::audio_tags::ogg::vorbis::VorbisFile;
use crate::audio_tags::resolve::custom_resolvers;
use crate::audio_tags::wavpack::WavPackFile;

use std::fs::File;
use std::io::{BufReader, Cursor, Read, Seek, SeekFrom};
use std::path::Path;

pub struct Probe<R: Read> {
    inner: R,
    options: Option<ParseOptions>,
    f_ty: Option<FileType>,
}

impl<R: Read> Probe<R> {
    #[must_use]
    pub const fn new(reader: R) -> Self {
        Self {
            inner: reader,
            options: None,
            f_ty: None,
        }
    }

    pub fn with_file_type(reader: R, file_type: FileType) -> Self {
        Self {
            inner: reader,
            options: None,
            f_ty: Some(file_type),
        }
    }

    pub fn file_type(&self) -> Option<FileType> {
        self.f_ty
    }

    pub fn set_file_type(mut self, file_type: FileType) -> Self {
        self.f_ty = Some(file_type);
        self
    }

    #[must_use]
    pub fn options(mut self, options: ParseOptions) -> Self {
        self.options = Some(options);
        self
    }

    pub fn into_inner(self) -> R {
        self.inner
    }
}

impl Probe<BufReader<File>> {
    pub fn open<P>(path: P) -> Result<Self>
    where
        P: AsRef<Path>,
    {
        let path = path.as_ref();
        log::debug!("Probe: Opening `{}` for reading", path.display());

        let file_type = FileType::from_path(path);
        log::debug!("Probe: Guessed file type `{:?}` from extension", file_type);

        Ok(Self {
            inner: BufReader::new(File::open(path)?),
            options: None,
            f_ty: file_type,
        })
    }
}

impl<R: Read + Seek> Probe<R> {
    pub fn guess_file_type(mut self) -> std::io::Result<Self> {
        let max_junk_bytes = self
            .options
            .map_or(ParseOptions::DEFAULT_MAX_JUNK_BYTES, |options| {
                options.max_junk_bytes
            });

        let f_ty = self.guess_inner(max_junk_bytes)?;
        self.f_ty = f_ty.or(self.f_ty);

        log::debug!("Probe: Guessed file type: {:?}", self.f_ty);

        Ok(self)
    }

    fn guess_inner(&mut self, max_junk_bytes: usize) -> std::io::Result<Option<FileType>> {
        // temporary buffer for storing 36 bytes
        // (36 is just a guess as to how long the data for estimating the file type might be)
        let mut buf = [0; 36];

        let starting_position = self.inner.stream_position()?;
        // Read (up to) 36 bytes
        let buf_len = std::io::copy(
            &mut self.inner.by_ref().take(buf.len() as u64),
            &mut Cursor::new(&mut buf[..]),
        )? as usize;

        self.inner.seek(SeekFrom::Start(starting_position))?;

        // Give custom resolvers priority
        if global_options().use_custom_resolvers {
            if let Ok(lock) = custom_resolvers().lock() {
                for (_, resolve) in lock.iter() {
                    if let ret @ Some(_) = resolve.guess(&buf[..buf_len]) {
                        return Ok(ret);
                    }
                }
            }
        }

        // Guess the file type by using these 36 bytes
        let Some(file_type_guess) = FileType::from_buffer_inner(&buf[..buf_len]) else {
            return Ok(None);
        };

        match file_type_guess {
            // We were able to determine a file type
            FileTypeGuessResult::Determined(file_ty) => Ok(Some(file_ty)),
            // The file starts with an ID3v2 tag; this means other data can follow (e.g. APE or MP3 frames)
            FileTypeGuessResult::MaybePrecededById3(id3_len) => {
                // `id3_len` is the size of the tag, not including the header (10 bytes)
                log::debug!("Probe: ID3v2 tag detected, skipping {} bytes", 10 + id3_len);
                let position_after_id3_block = self
                    .inner
                    .seek(SeekFrom::Current(i64::from(10 + id3_len)))?;

                // try to guess the file type after the ID3 block by inspecting the first 4 bytes
                let mut ident = [0; 4];
                std::io::copy(
                    &mut self.inner.by_ref().take(ident.len() as u64),
                    &mut Cursor::new(&mut ident[..]),
                )?;

                self.inner.seek(SeekFrom::Start(position_after_id3_block))?;

                let file_type_after_id3_block = match &ident {
                    [b'M', b'A', b'C', ..] => Ok(Some(FileType::Ape)),
                    b"fLaC" => Ok(Some(FileType::Flac)),
                    b"MPCK" | [b'M', b'P', b'+', ..] => Ok(Some(FileType::Mpc)),
                    // Search for a frame sync, which may be preceded by junk
                    _ => self.check_mpeg_or_aac(max_junk_bytes),
                };

                // before returning any result for a file type, seek back to the front
                self.inner.seek(SeekFrom::Start(starting_position))?;

                file_type_after_id3_block
            }
            // TODO: Check more than MPEG/AAC
            FileTypeGuessResult::MaybePrecededByJunk => {
                log::debug!(
                    "Probe: Possible junk bytes detected, searching up to {} bytes",
                    max_junk_bytes
                );

                let ret = self.check_mpeg_or_aac(max_junk_bytes);

                // before returning any result for a file type, seek back to the front
                self.inner.seek(SeekFrom::Start(starting_position))?;

                ret
            }
        }
    }

    fn check_mpeg_or_aac(&mut self, max_junk_bytes: usize) -> std::io::Result<Option<FileType>> {
        {
            let mut restricted_reader = self.inner.by_ref().take(max_junk_bytes as u64);
            if search_for_frame_sync(&mut restricted_reader)?.is_none() {
                return Ok(None);
            }
        }

        // Seek back to the start of the frame sync to check if we are dealing with
        // an AAC or MPEG file. See `FileType::quick_type_guess` for explanation.
        let sync_pos = self.inner.seek(SeekFrom::Current(-2))?;
        log::debug!("Probe: Found possible frame sync at position {}", sync_pos);

        let mut buf = [0; 2];
        self.inner.read_exact(&mut buf)?;

        if buf[1] & 0b10000 > 0 && buf[1] & 0b110 == 0 {
            Ok(Some(FileType::Aac))
        } else {
            Ok(Some(FileType::Mpeg))
        }
    }

    pub fn read(mut self) -> Result<TaggedFile> {
        let reader = &mut self.inner;
        let options = self.options.unwrap_or_default();

        if !options.read_tags && !options.read_properties {
            log::warn!("Skipping both tag and property reading, file will be empty");
        }

        match self.f_ty {
            Some(f_type) => Ok(match f_type {
                FileType::Aac => AacFile::read_from(reader, options)?.into(),
                FileType::Aiff => AiffFile::read_from(reader, options)?.into(),
                FileType::Ape => ApeFile::read_from(reader, options)?.into(),
                FileType::Flac => FlacFile::read_from(reader, options)?.into(),
                FileType::Mpeg => MpegFile::read_from(reader, options)?.into(),
                FileType::Opus => OpusFile::read_from(reader, options)?.into(),
                FileType::Vorbis => VorbisFile::read_from(reader, options)?.into(),
                FileType::Wav => WavFile::read_from(reader, options)?.into(),
                FileType::Mp4 => Mp4File::read_from(reader, options)?.into(),
                FileType::Mpc => MpcFile::read_from(reader, options)?.into(),
                FileType::Speex => SpeexFile::read_from(reader, options)?.into(),
                FileType::WavPack => WavPackFile::read_from(reader, options)?.into(),
                FileType::Custom(c) => {
                    if !global_options().use_custom_resolvers {
                        err!(UnknownFormat)
                    }

                    let resolver = crate::audio_tags::resolve::lookup_resolver(c);
                    resolver.read_from(reader, options)?
                }
            }),
            None => err!(UnknownFormat),
        }
    }
}

pub fn read_from(file: &mut File) -> Result<TaggedFile> {
    Probe::new(BufReader::new(file)).guess_file_type()?.read()
}

pub fn read_from_path<P>(path: P) -> Result<TaggedFile>
where
    P: AsRef<Path>,
{
    Probe::open(path)?.read()
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/audio_tags/probe/tests.rs"]
mod tests;
