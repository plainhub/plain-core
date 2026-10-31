use anyhow::Result;
use plain_rs::{
    db::{MediaFileMetadata, MediaFileMetadataRead},
    enums::DataType,
};
use std::{fs::File, io::BufReader, path::Path};

pub fn read_file_metadata(path: &Path, data_type: DataType) -> Result<MediaFileMetadata> {
    Ok(read_file_metadata_with_availability(path, data_type)?.metadata)
}

pub fn read_file_metadata_with_availability(
    path: &Path,
    data_type: DataType,
) -> Result<MediaFileMetadataRead> {
    plain_rs::media::file_metadata::read(path, data_type, |reader: File| {
        image::ImageReader::new(BufReader::new(reader))
            .with_guessed_format()
            .ok()?
            .into_dimensions()
            .ok()
    })
}
