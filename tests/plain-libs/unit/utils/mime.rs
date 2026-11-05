use super::*;

#[test]
fn mime_guesses() {
    assert_eq!(mime_from_ext("cat.jpg"), "image/jpeg");
    assert_eq!(mime_from_ext("cat.JPG"), "image/jpeg");
    assert_eq!(mime_from_ext("animation.apng"), "image/apng");
    assert_eq!(mime_from_ext("animation.APNG"), "image/apng");
    assert_eq!(mime_from_ext("movie.mp4"), "video/mp4");
    assert_eq!(mime_from_ext("thing.unknown"), "application/octet-stream");
}

#[test]
fn mime_handles_no_extension() {
    assert_eq!(mime_from_ext("README"), "application/octet-stream");
}

#[test]
fn mime_handles_empty() {
    assert_eq!(mime_from_ext(""), "application/octet-stream");
}

#[test]
fn ext_roundtrips_through_mime() {
    for ext in [
        "jpg", "png", "apng", "gif", "webp", "svg", "bmp", "tif", "heic", "avif", "mp4", "webm",
        "mov", "mkv", "3gp", "mp3", "m4a", "wav", "ogg", "pdf", "zip", "json", "txt", "html", "md",
        // from the server table merge
        "css", "js", "xml", "csv", "tsv", "ico", "avi", "ogv", "ts", "flac", "opus", "aac", "bz2",
        "xz",
    ] {
        assert_eq!(
            mime_extension(mime_from_ext(&format!("f.{ext}"))),
            ext,
            "roundtrip failed for {ext}"
        );
    }
}

#[test]
fn ext_roundtrips_office_documents() {
    for ext in ["doc", "docx", "xlsx"] {
        assert_eq!(
            mime_extension(mime_from_ext(&format!("f.{ext}"))),
            ext,
            "roundtrip failed for {ext}"
        );
    }
}

#[test]
fn ext_falls_back_to_bin() {
    assert_eq!(mime_extension("application/x-totally-made-up"), "bin");
    assert_eq!(mime_extension(""), "bin");
}

#[test]
fn ext_covers_image_aliases() {
    assert_eq!(mime_extension("image/x-icon"), "ico");
    assert_eq!(mime_extension("image/vnd.microsoft.icon"), "ico");
    assert_eq!(mime_extension("video/3gpp2"), "3gp");
    assert_eq!(mime_extension("audio/x-wav"), "wav");
    assert_eq!(mime_extension("application/x-zip-compressed"), "zip");
    assert_eq!(mime_extension("application/x-gzip"), "gz");
    assert_eq!(mime_extension("application/x-font-ttf"), "ttf");
    assert_eq!(mime_extension("IMAGE/PNG"), "png"); // case-insensitive
}

#[test]
fn office_documents_cover_plain_app_doc_mimes() {
    // plain-app `DocMediaStoreHelper.extraDocumentMimeTypes` — the doc
    // formats beyond `text/*` the media doc queries must recognize.
    assert_eq!(mime_from_ext("report.doc"), "application/msword");
    assert_eq!(
        mime_from_ext("report.DOCX"),
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
    );
    assert_eq!(
        mime_from_ext("sheet.xlsx"),
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
    );
}

#[test]
fn merged_table_covers_former_nas_entries() {
    // Spot-check the entries that came from the previous Go implementation's table.
    assert_eq!(mime_from_ext("index.htm"), "text/html");
    assert_eq!(mime_from_ext("app.mjs"), "application/javascript");
    assert_eq!(mime_from_ext("data.tsv"), "text/tab-separated-values");
    assert_eq!(mime_from_ext("sys.log"), "text/plain");
    assert_eq!(mime_from_ext("favicon.ico"), "image/x-icon");
    assert_eq!(mime_from_ext("clip.m4v"), "video/mp4");
    assert_eq!(mime_from_ext("old.avi"), "video/x-msvideo");
    assert_eq!(mime_from_ext("song.flac"), "audio/flac");
    assert_eq!(mime_from_ext("voice.opus"), "audio/opus");
    assert_eq!(mime_from_ext("pkg.xz"), "application/x-xz");
    assert_eq!(mime_from_ext("backup.tar"), "application/x-tar");
}
