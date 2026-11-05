/// Best-effort MIME type guess from a filename extension. Returns
/// `application/octet-stream` for unknown / extensionless inputs.
///
/// Merged table (2026-09-04): union of the former plain-rs table and the
/// richer server `fsx::guess_mime` table (which mirrored the Go implementation),
/// plus plain-app conventions where they overlapped. Charset handling is
/// deliberately excluded — callers append `; charset=utf-8` for `text/*`
/// if their transport wants it.
pub fn mime_from_ext(filename: &str) -> &'static str {
    match filename
        .rsplit('.')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        // Text
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" | "mjs" => "application/javascript",
        "json" => "application/json",
        "xml" => "application/xml",
        "txt" | "log" => "text/plain",
        "csv" => "text/csv",
        "tsv" => "text/tab-separated-values",
        "md" => "text/markdown",
        // Images
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "apng" => "image/apng",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "bmp" => "image/bmp",
        "tif" | "tiff" => "image/tiff",
        "heic" | "heif" => "image/heic",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        // Videos
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        "3gp" => "video/3gpp",
        "avi" => "video/x-msvideo",
        "3gpp" => "video/3gpp",
        "ogv" => "video/ogg",
        "ts" => "video/mp2t",
        // Audio
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "wav" => "audio/wav",
        "ogg" | "oga" => "audio/ogg",
        "flac" => "audio/flac",
        "opus" => "audio/opus",
        "aac" => "audio/aac",
        "wma" => "audio/x-ms-wma",
        // Fonts
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        // Archives
        "zip" => "application/zip",
        "tar" => "application/x-tar",
        "gz" | "tgz" => "application/gzip",
        "bz2" => "application/x-bzip2",
        "7z" => "application/x-7z-compressed",
        "rar" => "application/vnd.rar",
        "xz" => "application/x-xz",
        // Documents
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        // Default
        _ => "application/octet-stream",
    }
}

/// Reverse of `mime_from_ext`: pick a reasonable file extension for a
/// given MIME type. Returns `"bin"` for unknown / opaque types — the
/// caller can decide whether to surface a generic name. Matches
/// `plain-app` `AppFileStore.extFromMime` (Android `MimeTypeMap`), which
/// is why the image aliases (e.g. `image/x-icon` → `ico`) are included
/// here — favicon fetches use that MIME and must not lose the extension.
pub fn mime_extension(mime: &str) -> &'static str {
    match mime.to_ascii_lowercase().as_str() {
        // Images
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/apng" => "apng",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "image/svg+xml" => "svg",
        "image/bmp" => "bmp",
        "image/tiff" => "tif",
        "image/heic" => "heic",
        "image/avif" => "avif",
        "image/x-icon" | "image/vnd.microsoft.icon" => "ico",
        // Videos
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/quicktime" => "mov",
        "video/x-matroska" => "mkv",
        "video/3gpp" | "video/3gpp2" => "3gp",
        "video/x-msvideo" => "avi",
        "video/ogg" => "ogv",
        "video/mp2t" => "ts",
        // Audio
        "audio/mpeg" => "mp3",
        "audio/mp4" => "m4a",
        "audio/wav" | "audio/x-wav" => "wav",
        "audio/ogg" => "ogg",
        "audio/flac" => "flac",
        "audio/opus" => "opus",
        "audio/aac" => "aac",
        "audio/x-ms-wma" => "wma",
        // Documents / data
        "application/pdf" => "pdf",
        "application/msword" => "doc",
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx",
        "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet" => "xlsx",
        "application/zip" | "application/x-zip-compressed" => "zip",
        "application/json" => "json",
        "application/xml" | "text/xml" => "xml",
        "application/x-rar-compressed" | "application/vnd.rar" => "rar",
        "application/x-7z-compressed" => "7z",
        "application/x-tar" => "tar",
        "application/gzip" | "application/x-gzip" => "gz",
        "application/x-bzip2" => "bz2",
        "application/x-xz" => "xz",
        "application/vnd.android.package-archive" => "apk",
        // Text
        "text/plain" => "txt",
        "text/html" => "html",
        "text/css" => "css",
        "application/javascript" => "js",
        "text/csv" => "csv",
        "text/tab-separated-values" => "tsv",
        "text/markdown" => "md",
        // Fonts
        "font/ttf" | "application/x-font-ttf" => "ttf",
        "font/otf" => "otf",
        "font/woff" => "woff",
        "font/woff2" => "woff2",
        // Unknown / opaque
        _ => "bin",
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/utils/mime.rs"]
mod tests;
