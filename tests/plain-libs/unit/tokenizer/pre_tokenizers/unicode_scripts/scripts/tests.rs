use super::*;

#[test]
fn test_unicode_script() {
    assert_eq!(Script::Han, get_script('京'));
    assert_eq!(Script::Han, get_script('太'));
    assert_eq!(Script::Hiragana, get_script('い'));
    assert_eq!(Script::Katakana, get_script('グ'));
    assert_eq!(Script::Common, get_script('ー'));
    assert_eq!(Script::Latin, get_script('a'));
    assert_eq!(Script::Latin, get_script('A'));
    assert_eq!(Script::Common, get_script('0'));
    assert_eq!(Script::Common, get_script('$'));
    assert_eq!(Script::Common, get_script('@'));
    assert_eq!(Script::Common, get_script('-'));
    assert_eq!(Script::Common, get_script(' '));
    assert_eq!(Script::Common, get_script('�'));
}
