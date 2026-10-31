use super::*;

#[test]
fn default_app_dictionary_and_unicode_case_behavior_are_preserved() {
    for (input, expected) in [
        ("", ""),
        ("照片", "zhaopian"),
        ("中国", "zhongguo"),
        ("重庆", "zhongqing"),
        ("音乐", "yinle"),
        ("吕梁", "lvliang"),
        ("〇一龥", "lingyiyue"),
        ("AΩΣ", "aως"),
        ("🙂𠮷龦", "🙂𠮷龦"),
        ("A中国 12.JPG", "azhongguo 12.jpg"),
    ] {
        assert_eq!(sort_name(input), expected, "{input}");
    }
}

#[test]
fn character_dictionary_indices_are_valid_and_unknown_characters_are_preserved() {
    assert_eq!(
        CHARACTERS.len(),
        ((LAST_CHARACTER - FIRST_CHARACTER + 1) * 2) as usize
    );
    for value in FIRST_CHARACTER..=LAST_CHARACTER {
        let character = char::from_u32(value).unwrap();
        let offset = (value - FIRST_CHARACTER) as usize * 2;
        let index = u16::from_le_bytes([CHARACTERS[offset], CHARACTERS[offset + 1]]) as usize;
        assert!(index < syllables::SYLLABLES.len());
        if index == 0 {
            assert_eq!(sort_name(&character.to_string()), character.to_string());
        } else {
            assert!(!syllable(character).unwrap().is_empty());
        }
    }
}
