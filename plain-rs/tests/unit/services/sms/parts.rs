use super::*;

fn part(mime: &str, text: &str, data: bool) -> Part {
    Part {
        path: "content://mms/part/1".into(),
        content_type: mime.into(),
        name: String::new(),
        file_name: "fallback.bin".into(),
        has_data: data,
        text: text.into(),
    }
}

#[test]
fn text_assembly_preserves_order_and_trims_only_the_joined_body() {
    let message = message(&[
        part("TEXT/PLAIN", "  first ", true),
        part("text/plain", "", false),
        part("text/plain", " second  ", false),
        part("application/smil", "presentation", false),
    ]);
    assert_eq!(message.body, "first \n second");
    assert!(message.attachments.is_empty());
}

#[test]
fn attachment_selection_preserves_raw_mime_and_name_precedence() {
    let mut named = part("Image/JPEG", "ignored", false);
    named.name = "photo.jpg".into();
    let message = message(&[
        named,
        part("application/octet-stream", "", true),
        part("video/mp4", "", false),
        part("audio/aac", "", false),
        part("application/smil", "", false),
    ]);
    assert_eq!(message.body, "[MMS]");
    assert_eq!(message.attachments.len(), 4);
    assert_eq!(message.attachments[0].content_type, "Image/JPEG");
    assert_eq!(message.attachments[0].name, "photo.jpg");
    assert_eq!(message.attachments[1].name, "fallback.bin");
    assert_eq!(message.attachments[0].path, "content://mms/part/1");
}

#[test]
fn placeholders_require_an_attachment_and_do_not_override_a_real_body() {
    assert_eq!(message(&[]).body, "");
    assert_eq!(message(&[part("text/plain", " \n ", true)]).body, "");
    assert_eq!(
        message(&[
            part("text/plain", " body ", false),
            part("image/png", "", false)
        ])
        .body,
        "body"
    );
}

#[test]
fn archived_snippet_uses_the_newest_candidate_and_shared_mms_projection() {
    let candidates = vec![
        Snippet {
            date_millis: 999,
            value: "older SMS".into(),
            parts: None,
        },
        Snippet {
            date_millis: 1000,
            value: String::new(),
            parts: Some(vec![part("image/png", "", true)]),
        },
    ];
    assert_eq!(snippet(&candidates).as_deref(), Some("[MMS]"));
    assert_eq!(snippet(&[]), None);
    let candidates = vec![
        Snippet {
            date_millis: 1000,
            value: "SMS wins equal date".into(),
            parts: None,
        },
        Snippet {
            date_millis: 1000,
            value: String::new(),
            parts: Some(vec![part("image/png", "", true)]),
        },
    ];
    assert_eq!(snippet(&candidates).as_deref(), Some("SMS wins equal date"));
}
