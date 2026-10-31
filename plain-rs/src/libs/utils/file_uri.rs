pub fn local_path(path: &str) -> Result<String, String> {
    if !path
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file://"))
    {
        return Ok(path.to_owned());
    }
    let value = &path[7..];
    let local = if value.starts_with('/') {
        value.to_owned()
    } else if value
        .get(..10)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("localhost/"))
    {
        format!("/{}", &value[10..])
    } else {
        return Err("Unsupported file URI host".into());
    };
    let decoded = super::query::percent_decode(&local.replace('+', "%2B"));
    #[cfg(windows)]
    let decoded = if decoded.as_bytes().get(0) == Some(&b'/')
        && decoded
            .as_bytes()
            .get(1)
            .is_some_and(u8::is_ascii_alphabetic)
        && decoded.as_bytes().get(2) == Some(&b':')
    {
        decoded[1..].to_owned()
    } else {
        decoded
    };
    Ok(decoded)
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/libs/utils/file_uri.rs"]
mod tests;
