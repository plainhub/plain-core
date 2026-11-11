use crate::zip::extra_fields::CustomExtraField;
use crate::zip::extra_fields::DataStreamAlignment;
use crate::zip::extra_fields::ExtendedTimestamp;
use crate::zip::extra_fields::Ntfs;
use crate::zip::extra_fields::UnicodeExtraField;
use crate::zip::extra_fields::UsedExtraField;
use crate::zip::extra_fields::Zip64ExtendedInformation;
use crate::zip::extra_fields::zip64_extended_information::Zip64Sizes;
use crate::zip::format::blocks::ZipEntryBlock;
use crate::zip::format::flags::ZipFlags;
use crate::zip::result::ZipResult;
use crate::zip::result::invalid;
use crate::zip::types::ZipFileData;
use crate::zip::unstable::LittleEndianReadExt;
use std::io::ErrorKind;
use std::io::{Cursor, Read, Write};
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum ExtraField {
    Ntfs(Ntfs),
    ExtendedTimestamp(ExtendedTimestamp),
    Zip64ExtendedInformation(Zip64ExtendedInformation),
    UnicodeComment(UnicodeExtraField),
    UnicodePath(UnicodeExtraField),
    DataStreamAlignment(DataStreamAlignment),
    Custom(CustomExtraField),
}
#[derive(Debug, Clone, Default)]
pub struct ExtraFields {
    pub(crate) inner: Vec<ExtraField>,
}
impl ExtraFields {
    pub(crate) fn parse<B: ZipEntryBlock>(
        buff: &[u8],
        block: &B,
        is_local_header: bool,
    ) -> ZipResult<Self> {
        let mut reader = Cursor::new(buff);
        let mut extra_fields = Vec::new();
        while (reader.position() as usize) < buff.len() {
            let parsed_extra_field = ExtraField::parse(&mut reader, block, is_local_header)?;
            let Some(parsed_extra_field) = parsed_extra_field else {
                break;
            };
            extra_fields.push(parsed_extra_field);
        }
        Ok(Self {
            inner: extra_fields,
        })
    }
    pub(crate) fn local_extra_fields_mut(&mut self) -> impl Iterator<Item = &mut ExtraField> {
        self.inner.iter_mut()
    }
    pub(crate) fn local_extra_fields(&self) -> impl Iterator<Item = &ExtraField> {
        self.inner.iter().filter(|ef| match ef {
            ExtraField::Custom(cef) => !cef.central_only,
            _ => true,
        })
    }
    pub(crate) fn central_extra_fields(&self) -> impl Iterator<Item = &ExtraField> {
        self.inner
            .iter()
            .filter(|ef| !matches!(ef, ExtraField::DataStreamAlignment(_)))
    }
}
impl ExtraField {
    pub(crate) fn parse<R: Read, B: ZipEntryBlock>(
        reader: &mut R,
        file: &B,
        is_local_header: bool,
    ) -> ZipResult<Option<Self>> {
        let extra_field_header_id = match reader.read_u16_le() {
            Ok(value) => value,
            Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        let decoded_extra_field = UsedExtraField::try_from(extra_field_header_id);
        let len = match decoded_extra_field {
            Ok(known_field) => match reader.read_u16_le() {
                Ok(len) => len,
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => {
                    return Err(invalid!("Extra field {} header truncated", known_field));
                }
                Err(e) => return Err(e.into()),
            },
            Err(()) => match reader.read_u16_le() {
                Ok(len) => len,
                Err(e) if e.kind() == ErrorKind::UnexpectedEof => return Ok(None),
                Err(_e) => {
                    let _ = std::io::copy(reader, &mut std::io::sink());
                    return Ok(None);
                }
            },
        };
        let parsed_extra_field =
            match decoded_extra_field {
                Ok(UsedExtraField::Zip64ExtendedInfo) => {
                    ExtraField::Zip64ExtendedInformation(Zip64ExtendedInformation::parse(
                        reader,
                        len,
                        file.get_uncompressed_size(),
                        file.get_compressed_size(),
                        file.get_header_start(),
                    )?)
                }
                Ok(UsedExtraField::Ntfs) => {
                    let mut data = vec![0; len as usize];
                    reader.read_exact(&mut data)?;
                    match Ntfs::try_from_reader(&mut Cursor::new(&data), len) {
                        Ok(ntfs) => ExtraField::Ntfs(ntfs),
                        Err(crate::zip::result::ZipError::UnsupportedArchive(_)) => {
                            ExtraField::Custom(CustomExtraField::new(
                                false,
                                extra_field_header_id,
                                data.into_boxed_slice(),
                            ))
                        }
                        Err(e) => return Err(e),
                    }
                }
                Ok(UsedExtraField::ExtendedTimestamp) => ExtraField::ExtendedTimestamp(
                    ExtendedTimestamp::try_from_reader(reader, len, is_local_header)?,
                ),
                Ok(UsedExtraField::UnicodeComment) => {
                    let unicode = UnicodeExtraField::try_from_reader(reader, len)?;
                    ExtraField::UnicodeComment(unicode)
                }
                Ok(UsedExtraField::UnicodePath) => {
                    let unicode = UnicodeExtraField::try_from_reader(reader, len)?;
                    ExtraField::UnicodePath(unicode)
                }
                Ok(UsedExtraField::DataStreamAlignment)
                | Ok(UsedExtraField::AndroidZipAlignment) => ExtraField::DataStreamAlignment(
                    DataStreamAlignment::try_from_reader(reader, extra_field_header_id, len)?,
                ),
                _ => {
                    let mut buf = vec![0u8; len as usize];
                    if let Err(e) = reader.read_exact(&mut buf) {
                        if e.kind() == ErrorKind::UnexpectedEof {
                            return Err(invalid!("Extra field content truncated"));
                        }
                        return Err(e.into());
                    }
                    ExtraField::Custom(CustomExtraField::new(
                        false,
                        extra_field_header_id,
                        buf.into_boxed_slice(),
                    ))
                }
            };
        Ok(Some(parsed_extra_field))
    }
    pub(crate) fn size(&self, is_local_header: bool) -> usize {
        match self {
            ExtraField::Zip64ExtendedInformation(zip64_extra) => {
                zip64_extra.full_size(is_local_header)
            }
            ExtraField::Ntfs(_ntfs) => 0,
            ExtraField::ExtendedTimestamp(_extended_timestamp) => 0,
            ExtraField::UnicodeComment(unicode_comment) => unicode_comment.full_size(),
            ExtraField::UnicodePath(unicode_path) => unicode_path.full_size(),
            ExtraField::Custom(custom) => custom.full_size(is_local_header),
            ExtraField::DataStreamAlignment(data_stream_alignment) => {
                data_stream_alignment.full_size(is_local_header)
            }
        }
    }
    pub(crate) fn write<W: Write>(&self, writer: &mut W, is_local_header: bool) -> ZipResult<()> {
        match self {
            ExtraField::Zip64ExtendedInformation(zip64_extra) => {
                zip64_extra.write(writer, is_local_header)?;
            }
            ExtraField::Custom(custom) => {
                custom.write(writer, is_local_header)?;
            }
            ExtraField::UnicodeComment(unicode_comment) => {
                let magic = UsedExtraField::UnicodeComment.as_u16();
                writer.write_all(&magic.to_le_bytes())?;
                unicode_comment.write(writer)?;
            }
            ExtraField::UnicodePath(unicode_path) => {
                let magic = UsedExtraField::UnicodePath.as_u16();
                writer.write_all(&magic.to_le_bytes())?;
                unicode_path.write(writer)?;
            }
            ExtraField::DataStreamAlignment(data_stream_alignment) => {
                data_stream_alignment.write(writer, is_local_header)?
            }
            _ => {}
        }
        Ok(())
    }
}
impl ZipFileData {
    pub(crate) fn apply_extra_fields(&mut self, file_name_raw: &mut Vec<u8>) -> ZipResult<()> {
        let mut orig_name_crc = None;
        let mut orig_comment_crc = None;
        for one_extra_field in &self.extra_fields.inner {
            match one_extra_field {
                ExtraField::Zip64ExtendedInformation(zip64_block) => {
                    self.large_file = true;
                    if let Some(Zip64Sizes {
                        uncompressed_size,
                        compressed_size,
                    }) = zip64_block.sizes
                    {
                        self.uncompressed_size = uncompressed_size;
                        self.compressed_size = compressed_size;
                    }
                    if let Some(head_start) = zip64_block.header_start {
                        self.header_start = head_start;
                    }
                }
                ExtraField::UnicodeComment(unicode) => {
                    let crc = *orig_comment_crc
                        .get_or_insert_with(|| crate::checksum::crc32::hash(self.file_comment.as_bytes()));
                    if unicode.crc32 == crc
                        && let Ok(comment) = String::from_utf8(unicode.content.to_vec())
                    {
                        self.file_comment = comment.into_boxed_str();
                    }
                }
                ExtraField::UnicodePath(unicode) => {
                    let crc = *orig_name_crc.get_or_insert_with(|| crate::checksum::crc32::hash(file_name_raw));
                    if unicode.crc32 == crc && std::str::from_utf8(&unicode.content).is_ok() {
                        *file_name_raw = unicode.content.to_vec();
                        self.flags |= ZipFlags::LanguageEncoding.as_u16();
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }
}
