use super::*;

#[test]
fn expansion_followed_by_removal() {
    // Simulate transformations from "™\x1eg" to "TMg"
    let mut transformations = vec![];

    let mut n = NormalizedString::from("™\x1eg");
    replace(&mut transformations, "™", "TM");
    replace(&mut transformations, "\x1e", "");
    transformations.push(('g', 0));

    n.transform(transformations, 0);

    assert_eq!(n.get(), "TMg");
}
