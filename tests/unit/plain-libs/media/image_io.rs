#[path = "../codegen.rs"]
mod codegen;

#[test]
fn application_image_operations_keep_codecs_in_the_library() {
    let workspace = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let read = |name| std::fs::read_to_string(workspace.join(name)).unwrap();
    let thumbnail = read("plain-rs/src/features/services/images/thumbnail_decoder.rs");
    let resources = read("plain-rs/src/features/services/files/resources.rs");
    let native = resources.split("async fn native(").nth(1).unwrap();
    let dimensions = read("plain-rs/src/features/media/file_facts.rs");
    let info = read("plain-rs/src/features/services/files/info.rs");
    let expression = info
        .split("if let Some((width, height)) = ")
        .nth(1)
        .unwrap()
        .split('{')
        .next()
        .unwrap()
        .trim();
    let fixture = include_str!("image_io_codegen.rs")
        .replace(
            "THUMBNAIL_BODY",
            block_after(&thumbnail, "tokio::task::spawn_blocking(move ||"),
        )
        .replace(
            "CONVERSION_BODY",
            block_after(native, "tokio::task::spawn_blocking(move ||"),
        )
        .replace("FILE_DIMENSIONS_BODY", &dimension_body(&dimensions))
        .replace("SOURCE_DIMENSIONS_EXPRESSION", expression)
        .replace("BufReader::", "std::io::BufReader::");
    let ir = codegen::compile_consumer(
        &fixture,
        &[
            "src/media/mod.rs",
            "src/image/mod.rs",
            "src/media/image_io.rs",
        ],
    );
    assert!(
        !ir.lines().any(|line| line.starts_with("define ")
            && ["JpegDecoder", "LosslessDecoder", "PngEncoder", "Vp8Decoder"]
                .iter()
                .any(|name| line.contains(name))),
        "the application instantiated image codec implementations"
    );
}

#[test]
fn dimension_queries_do_not_instantiate_pixel_decoders() {
    let fixture = r#"
        extern crate plain_libs;
        #[unsafe(no_mangle)]
        pub fn dimensions(bytes: &[u8]) -> plain_libs::image::ImageResult<(u32, u32)> {
            plain_libs::image::ImageReader::new(std::io::BufReader::new(std::io::Cursor::new(bytes)))
                .with_guessed_format()?.into_dimensions()
        }
    "#;
    let ir = codegen::compile_consumer(fixture, &["src/image/io/image_reader_type.rs"]);
    assert!(
        !ir.lines()
            .any(|line| line.starts_with("define ") && line.contains("read_image_boxed")),
        "dimension queries instantiated pixel decoding through decoder vtables"
    );
}

fn block_after<'a>(source: &'a str, marker: &str) -> &'a str {
    let tail = source.split_once(marker).unwrap().1;
    let begin = tail.find('{').unwrap() + 1;
    let mut depth = 1;
    for (offset, character) in tail[begin..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => depth -= 1,
            _ => {}
        }
        if depth == 0 {
            return &tail[begin..begin + offset];
        }
    }
    panic!("unclosed production block");
}

fn dimension_body(source: &str) -> String {
    if source.contains("|reader: File|") {
        return block_after(source, "|reader: File|").into();
    }
    let entry = source
        .split("file_metadata::read(")
        .nth(1)
        .unwrap()
        .split(',')
        .nth(2)
        .unwrap()
        .trim();
    format!("{entry}(reader)")
}

use plain_libs::{serde_json::Value, utils::source_reader::SourceReader};
fn thumbnail(path: String, width: u32, height: u32) -> Result<Option<Vec<u8>>, String> {
    plain_libs::media::image_io::thumbnail(std::path::Path::new(&path), width, height)
}
fn convert(path: String, output: String) -> Result<Value, String> {
    plain_libs::media::image_io::convert_to_png(
        std::path::Path::new(&path),
        std::path::Path::new(&output),
    )
    .map(Value::Bool)
}
fn file_dimensions(reader: std::fs::File) -> Option<(u32, u32)> {
    plain_libs::media::image_io::file_dimensions(reader)
}
fn source_dimensions(reader: &mut SourceReader) -> Option<(u32, u32)> {
    plain_libs::media::image_io::source_dimensions(reader)
}
#[path = "image_io_behavior.rs"]
mod behavior;
