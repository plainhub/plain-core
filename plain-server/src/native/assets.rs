use crate::http::web_bundle::{EmbeddedBundle, WebBundle};

pub fn file_icon(extension: &str) -> &'static str {
    let selected = if extension.len() <= 128 {
        let extension = extension
            .trim()
            .trim_start_matches('.')
            .to_ascii_lowercase();
        if !extension.is_empty()
            && extension
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            EmbeddedBundle.read(&format!("ficons/{extension}.svg"))
        } else {
            None
        }
    } else {
        None
    };
    let bytes = selected
        .or_else(|| EmbeddedBundle.read("ficons/default.svg"))
        .expect("Embedded default file icon missing");
    std::str::from_utf8(bytes).expect("Embedded file icon must be UTF-8")
}

#[cfg(test)]
#[path = "../../tests/unit/native/assets.rs"]
mod tests;
