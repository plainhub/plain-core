use super::*;

#[test]
fn decode() {
    let decoder = ByteFallback::new();
    let res = decoder
        .decode_chain(vec!["Hey".into(), "friend!".into()])
        .unwrap();
    assert_eq!(res, vec!["Hey", "friend!"]);

    let res = decoder.decode_chain(vec!["<0x61>".into()]).unwrap();
    assert_eq!(res, vec!["a"]);

    let res = decoder.decode_chain(vec!["<0xE5>".into()]).unwrap();
    assert_eq!(res, vec!["�"]);

    let res = decoder
        .decode_chain(vec!["<0xE5>".into(), "<0x8f>".into()])
        .unwrap();
    assert_eq!(res, vec!["�", "�"]);

    // 叫
    let res = decoder
        .decode_chain(vec!["<0xE5>".into(), "<0x8f>".into(), "<0xab>".into()])
        .unwrap();
    assert_eq!(res, vec!["叫"]);

    let res = decoder
        .decode_chain(vec![
            "<0xE5>".into(),
            "<0x8f>".into(),
            "<0xab>".into(),
            "a".into(),
        ])
        .unwrap();
    assert_eq!(res, vec!["叫", "a"]);

    let res = decoder
        .decode_chain(vec!["<0xE5>".into(), "<0x8f>".into(), "a".into()])
        .unwrap();
    assert_eq!(res, vec!["�", "�", "a"]);
}
