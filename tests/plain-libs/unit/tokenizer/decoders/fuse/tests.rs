use super::*;

#[test]
fn decode() {
    let decoder = Fuse::new();
    let res = decoder
        .decode_chain(vec!["Hey".into(), " friend!".into()])
        .unwrap();
    assert_eq!(res, vec!["Hey friend!"]);
}
