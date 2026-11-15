use crate::features::enums::DataType;
use crate::infrastructure::db::MediaFileMetadata;
use crate::infrastructure::db::MediaFileMetadataRead;
use plain_libs::anyhow::Result;
use std::path::Path;

pub fn read_file_metadata(path: &Path, data_type: DataType) -> Result<MediaFileMetadata> {
    Ok(read_file_metadata_with_availability(path, data_type)?.metadata)
}

pub fn read_file_metadata_with_availability(
    path: &Path,
    data_type: DataType,
) -> Result<MediaFileMetadataRead> {
    crate::features::media::file_metadata::read(
        path,
        data_type,
        plain_libs::media::image_io::file_dimensions,
    )
}
