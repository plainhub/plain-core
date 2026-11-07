use super::*;

#[test]
fn decode() {
    let decoder = Strip::new('H', 1, 0);
    let res = decoder
        .decode_chain(vec!["Hey".into(), " friend!".into(), "HHH".into()])
        .unwrap();
    assert_eq!(res, vec!["ey", " friend!", "HH"]);

    let decoder = Strip::new('y', 0, 1);
    let res = decoder
        .decode_chain(vec!["Hey".into(), " friend!".into()])
        .unwrap();
    assert_eq!(res, vec!["He", " friend!"]);
}
