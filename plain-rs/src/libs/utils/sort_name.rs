mod syllables;

const FIRST_CHARACTER: u32 = 0x4e00;
const LAST_CHARACTER: u32 = 0x9fa5;
const CHARACTERS: &[u8; 41804] = include_bytes!("sort_name/characters.bin");

pub fn sort_name(value: &str) -> String {
    let mut converted = String::with_capacity(value.len());
    for character in value.chars() {
        if character == '〇' {
            converted.push_str("LING");
        } else if let Some(syllable) = syllable(character) {
            converted.push_str(syllable);
        } else {
            converted.push(character);
        }
    }
    converted.to_lowercase()
}

fn syllable(character: char) -> Option<&'static str> {
    let value = character as u32;
    if !(FIRST_CHARACTER..=LAST_CHARACTER).contains(&value) {
        return None;
    }
    let offset = (value - FIRST_CHARACTER) as usize * 2;
    let index = u16::from_le_bytes([CHARACTERS[offset], CHARACTERS[offset + 1]]) as usize;
    (index != 0).then(|| syllables::SYLLABLES[index])
}

#[cfg(test)]
#[path = "../../../../tests/plain-rs/unit/libs/utils/sort_name.rs"]
mod tests;
