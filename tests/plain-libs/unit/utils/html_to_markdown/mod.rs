use super::*;

#[derive(serde::Deserialize)]
struct Case {
    name: String,
    html: String,
    markdown: String,
}

#[test]
fn kotlin_markdown_behavior_contract() {
    let cases: Vec<Case> = serde_json::from_str(include_str!(
        "../../../../../plain-rs/testdata/html_to_markdown.json"
    ))
    .unwrap();
    assert!(cases.len() >= 212);
    let mut failures = Vec::new();
    for case in cases {
        let actual = html_to_markdown(&case.html);
        if actual != case.markdown {
            failures.push(format!(
                "{}: {:?}\nexpected {:?}\nactual   {:?}",
                case.name, case.html, case.markdown, actual
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn numeric_entities_use_unicode_scalars() {
    assert_eq!(html_to_markdown("&#128512; &#x1F600;"), "😀 😀");
    assert_eq!(
        html_to_markdown("&#xD800; &#1114112;"),
        "&#xD800; &#1114112;"
    );
}

#[test]
fn conversions_have_no_shared_mutable_state() {
    let workers = (0..8)
        .map(|_| {
            std::thread::spawn(|| {
                for _ in 0..100 {
                    assert_eq!(
                        html_to_markdown("<h2>Title</h2><p>A <b>B</b>.</p>"),
                        "Title\n-----\n\nA **B**."
                    );
                    assert_eq!(html_to_markdown(""), "");
                }
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        worker.join().unwrap();
    }
}

#[test]
fn fixed_markdown_escapes_preserve_prefix_rules() {
    for (input, expected) in [
        ("-x", "\\-x"),
        ("+ x", "\\+ x"),
        ("+x", "+x"),
        ("===", "\\==="),
        ("###### x", "\\###### x"),
        ("####### x", "####### x"),
        ("#x", "#x"),
        ("~~~x", "\\~~~x"),
        ("~~x", "~~x"),
        ("123. x", "123\\. x"),
        ("123.x", "123.x"),
        ("١. x", "١. x"),
        ("\\*_[]`中", "\\\\\\*\\_\\[\\]\\`中"),
    ] {
        assert_eq!(escape(input), expected, "{input:?}");
    }
}

#[test]
fn attribute_newlines_only_consume_following_ascii_whitespace() {
    assert_eq!(clean_attribute(" a\n \t\r\n\u{b}\u{c}b "), " a\nb ");
    assert_eq!(
        clean_attribute("a\n\u{a0}\u{2003} b"),
        "a\n\u{a0}\u{2003} b"
    );
    assert_eq!(clean_attribute("a \t\rb"), "a \t\rb");
}

#[test]
fn malformed_and_nested_entities_preserve_the_original_text() {
    for (input, expected) in [
        ("&amp;amp;", "&amp;"),
        ("&&amp;", "&&"),
        ("&#x41;&#65;", "AA"),
        ("&#X41;", "&#X41;"),
        ("&#xD800; &#1114112;", "&#xD800; &#1114112;"),
        ("&#999999999999999999999999;", "&#999999999999999999999999;"),
        ("&unknown; &AMP; &amp", "&unknown; &AMP; &amp"),
        ("&中&amp;", "&中&"),
        ("&#0;", "\0"),
    ] {
        assert_eq!(entities::decode(input), expected, "{input:?}");
    }
}
