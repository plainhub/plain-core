use super::*;
use serde_json::json;

#[test]
fn provider_types_are_projected_to_domain_types_without_confusing_their_numeric_codes() {
    let rows = identities(json!([
        {"id":"image","path":"/files/image","providerType":1,"mimeType":"text/plain"},
        {"id":"audio","path":"/files/audio","providerType":2,"mimeType":null},
        {"id":"video","path":"/files/video","providerType":3,"mimeType":"application/pdf"},
        {"id":"text","path":"/files/text","providerType":0,"mimeType":"text/markdown"},
        {"id":"zip","path":"/files/zip","providerType":0,"mimeType":"application/zip"},
        {"id":"unknown","path":"/files/unknown","providerType":0,"mimeType":null}
    ]))
    .unwrap();
    assert_eq!(
        rows.iter()
            .map(|row| (row.file_id.as_str(), row.media_type))
            .collect::<Vec<_>>(),
        vec![
            ("image", DataType::Image.kind()),
            ("audio", DataType::Audio.kind()),
            ("video", DataType::Video.kind()),
            ("text", DataType::Doc.kind())
        ]
    );
    assert_eq!(rows[3].path, "/files/text");
}

#[test]
fn all_document_mime_rules_come_from_the_shared_document_module() {
    for mime in plain_rs::media::documents::MIME_TYPES {
        let rows = identities(
            json!([{"id":"document","path":"/files/doc","providerType":0,"mimeType":mime}]),
        )
        .unwrap();
        assert_eq!(rows.len(), 1, "{mime}");
        assert_eq!(rows[0].media_type, DataType::Doc.kind());
    }
    for facts in [
        json!([{"mediaId":"old","path":"/files/old","mediaType":24}]),
        json!([{"id":"invalid","path":"/files/invalid","providerType":"audio","mimeType":null}]),
    ] {
        assert!(identities(facts).is_err());
    }
}
