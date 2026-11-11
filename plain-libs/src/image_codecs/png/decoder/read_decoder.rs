use super::Limits;
use super::stream::{DecodeOptions, Decoded, DecodingError, FormatErrorInner, StreamingDecoder};
use super::zlib::UnfilterBuf;
use crate::image_codecs::png::chunk;
use crate::image_codecs::png::common::Info;
use std::io::{BufRead, ErrorKind, Read, Seek};
pub(crate) struct ReadDecoder<R: Read> {
    reader: R,
    decoder: StreamingDecoder,
}
impl<R: BufRead + Seek> ReadDecoder<R> {
    pub fn new(r: R) -> Self {
        Self {
            reader: r,
            decoder: StreamingDecoder::new(),
        }
    }
    pub fn with_options(r: R, options: DecodeOptions) -> Self {
        let mut decoder = StreamingDecoder::new_with_options(options);
        decoder.limits = Limits::default();
        Self { reader: r, decoder }
    }
    pub fn set_limits(&mut self, limits: Limits) {
        self.decoder.limits = limits;
    }
    pub fn reserve_bytes(&mut self, bytes: usize) -> Result<(), DecodingError> {
        self.decoder.limits.reserve_bytes(bytes)
    }
    pub fn set_ignore_text_chunk(&mut self, ignore_text_chunk: bool) {
        self.decoder.set_ignore_text_chunk(ignore_text_chunk);
    }
    pub fn set_ignore_iccp_chunk(&mut self, ignore_iccp_chunk: bool) {
        self.decoder.set_ignore_iccp_chunk(ignore_iccp_chunk);
    }
    pub fn ignore_checksums(&mut self, ignore_checksums: bool) {
        self.decoder.set_ignore_adler32(ignore_checksums);
        self.decoder.set_ignore_crc(ignore_checksums);
    }
    fn decode_next(
        &mut self,
        image_data: Option<&mut UnfilterBuf<'_>>,
    ) -> Result<Decoded, DecodingError> {
        let (consumed, result) = {
            let buf = self.reader.fill_buf()?;
            if buf.is_empty() {
                return Err(DecodingError::IoError(ErrorKind::UnexpectedEof.into()));
            }
            self.decoder.update(buf, image_data)?
        };
        self.reader.consume(consumed);
        Ok(result)
    }
    pub fn read_header_info(&mut self) -> Result<&Info<'static>, DecodingError> {
        while self.info().is_none() {
            if let Decoded::ChunkComplete(chunk::IEND) = self.decode_next(None)? {
                unreachable!()
            }
        }
        Ok(self.info().unwrap())
    }
    pub fn read_until_image_data(&mut self) -> Result<(), DecodingError> {
        loop {
            match self.decode_next(None)? {
                Decoded::ChunkBegin(_, chunk::IDAT) | Decoded::ChunkBegin(_, chunk::fdAT) => break,
                Decoded::ChunkComplete(chunk::IEND) => {
                    return Err(DecodingError::Format(
                        FormatErrorInner::MissingImageData.into(),
                    ));
                }
                _ => {}
            }
        }
        Ok(())
    }
    pub fn decode_image_data(
        &mut self,
        image_data: Option<&mut UnfilterBuf<'_>>,
    ) -> Result<ImageDataCompletionStatus, DecodingError> {
        match self.decode_next(image_data)? {
            Decoded::ImageData => Ok(ImageDataCompletionStatus::ExpectingMoreData),
            Decoded::ImageDataFlushed => Ok(ImageDataCompletionStatus::Done),
            _ => Ok(ImageDataCompletionStatus::ExpectingMoreData),
        }
    }
    pub fn finish_decoding_image_data(&mut self) -> Result<(), DecodingError> {
        loop {
            if let ImageDataCompletionStatus::Done = self.decode_image_data(None)? {
                return Ok(());
            }
        }
    }
    pub fn read_until_end_of_input(&mut self) -> Result<(), DecodingError> {
        while !matches!(self.decode_next(None)?, Decoded::ChunkComplete(chunk::IEND)) {}
        Ok(())
    }
    pub fn info(&self) -> Option<&Info<'static>> {
        self.decoder.info.as_ref()
    }
}
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum ImageDataCompletionStatus {
    ExpectingMoreData,
    Done,
}
