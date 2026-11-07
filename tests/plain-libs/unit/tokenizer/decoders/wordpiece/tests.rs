use super::*;

#[test]
fn wordpiece_decoder() {
    let decoder = WordPiece::new("##".to_string(), false);

    assert_eq!(
        decoder
            .decode(vec![
                "##uelo".to_string(),
                "Ara".to_string(),
                "##új".to_string(),
                "##o".to_string(),
                "No".to_string(),
                "##guera".to_string()
            ])
            .unwrap(),
        "##uelo Araújo Noguera"
    );
}
