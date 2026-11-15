extern crate plain_libs;
use plain_libs::utils::source_reader::{CancellableReader, SourceReader};

#[unsafe(no_mangle)]
pub fn source_metadata(
    reader: &mut SourceReader,
    path: &str,
) -> plain_libs::media::probe::ProbedMeta {
    plain_libs::media::probe::AUDIO_ENTRY(reader, path)
}

#[unsafe(no_mangle)]
pub fn source_duration(reader: &mut SourceReader) -> Option<i64> {
    plain_libs::media::probe::DURATION_ENTRY(reader)
}

#[unsafe(no_mangle)]
pub fn file_duration(reader: &mut CancellableReader<std::fs::File>) -> Option<i64> {
    plain_libs::media::probe::FILE_DURATION_ENTRY(reader)
}

#[unsafe(no_mangle)]
pub fn source_cover(reader: &mut SourceReader) -> Option<plain_libs::media::cover::CoverData> {
    plain_libs::media::cover::COVER_ENTRY(reader)
}
