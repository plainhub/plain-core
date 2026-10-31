pub fn normalize_artist(value: &str) -> String {
    value.replace("<unknown>", "")
}

pub fn album_id(album: &str, album_artist: &str, artist: &str) -> String {
    use sha2::{Digest, Sha256};
    let album = album.trim().to_lowercase();
    if album.is_empty() {
        return String::new();
    }
    let album_artist = normalize_artist(album_artist);
    let artist = normalize_artist(artist);
    let owner = if album_artist.trim().is_empty() {
        artist.trim()
    } else {
        album_artist.trim()
    }
    .to_lowercase();
    let mut digest = Sha256::new();
    for part in [album.as_bytes(), owner.as_bytes()] {
        digest.update((part.len() as u64).to_le_bytes());
        digest.update(part);
    }
    format!(
        "album:{}",
        crate::libs::utils::hex::bytes_to_hex(&digest.finalize())
    )
}

#[cfg(test)]
#[path = "../../../../../tests/plain-rs/unit/features/media/audio/metadata.rs"]
mod tests;
