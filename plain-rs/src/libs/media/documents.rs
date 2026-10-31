pub const MIME_TYPES: &[&str] = &[
    "application/pdf",
    "application/msword",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
    "application/javascript",
];

pub fn is_document_mime(mime: &str) -> bool {
    mime.starts_with("text/") || MIME_TYPES.contains(&mime)
}
