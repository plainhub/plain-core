use super::*;

#[test]
fn test_heading_level() {
    assert_eq!(HeadingLevel::from_number(1), HeadingLevel::H1);
    assert_eq!(HeadingLevel::from_number(6), HeadingLevel::H6);
    assert_eq!(HeadingLevel::from_number(7), HeadingLevel::None);
    assert_eq!(HeadingLevel::from_number(0), HeadingLevel::None);

    assert_eq!(HeadingLevel::H3.level(), 3);
    assert!(HeadingLevel::H1.is_heading());
    assert!(!HeadingLevel::None.is_heading());
}

#[test]
fn test_text_run() {
    let plain = TextRun::plain("Hello");
    assert_eq!(plain.text, "Hello");
    assert!(!plain.is_link());

    let link = TextRun::link("Click here", "https://example.com");
    assert!(link.is_link());
    assert_eq!(link.hyperlink, Some("https://example.com".to_string()));
}

#[test]
fn test_text_style() {
    let style = TextStyle::bold();
    assert!(style.bold);
    assert!(style.has_formatting());

    let plain = TextStyle::default();
    assert!(!plain.has_formatting());
}

#[test]
fn test_paragraph() {
    let para = Paragraph::with_text("Hello, World!");
    assert_eq!(para.plain_text(), "Hello, World!");
    assert!(!para.is_heading());
    assert!(!para.is_empty());

    let heading = Paragraph::heading(HeadingLevel::H1, "Title");
    assert!(heading.is_heading());
    assert_eq!(heading.heading.level(), 1);
}

#[test]
fn test_paragraph_plain_text_preserves_run_breaks() {
    let para = Paragraph {
        runs: vec![
            TextRun {
                text: "First line".to_string(),
                line_break: true,
                ..Default::default()
            },
            TextRun {
                text: "Second line".to_string(),
                page_break: true,
                ..Default::default()
            },
            TextRun::plain("Third line"),
        ],
        ..Default::default()
    };

    assert_eq!(
        para.plain_text(),
        "First line\nSecond line\n---\nThird line"
    );
}

#[test]
fn test_paragraph_serialization() {
    let para = Paragraph::with_text("Test");
    let json = serde_json::to_string(&para).unwrap();
    // Default values should not be serialized
    assert!(!json.contains("heading"));
    assert!(!json.contains("alignment"));
}

#[test]
fn merging_keeps_note_references_and_tracked_changes_apart() {
    let inserted = TextRun {
        revision: RevisionType::Inserted,
        ..TextRun::plain("new")
    };
    let mut para = Paragraph::new();
    para.runs = vec![
        TextRun::plain("text"),
        TextRun::note_reference("1"),
        TextRun::plain(" and "),
        inserted,
    ];
    para.merge_adjacent_runs();
    let shape: Vec<_> = para
        .runs
        .iter()
        .map(|r| (r.text.as_str(), r.note.as_deref(), r.revision))
        .collect();
    assert_eq!(
        shape,
        [
            ("text", None, RevisionType::None),
            ("", Some("1"), RevisionType::None),
            (" and ", None, RevisionType::None),
            ("new", None, RevisionType::Inserted),
        ]
    );
}

#[test]
fn test_merge_adjacent_runs_ascii_no_split() {
    // Test fix for word splitting bug: "DRBD" stored as ["DRB", "D"] should merge to "DRBD"
    let mut para = Paragraph::new();
    para.runs.push(TextRun::plain("DRB"));
    para.runs.push(TextRun::plain("D"));
    para.merge_adjacent_runs();

    assert_eq!(para.runs.len(), 1);
    assert_eq!(para.runs[0].text, "DRBD"); // NOT "DRB D"
}

#[test]
fn test_merge_adjacent_runs_ping() {
    // Test fix: "PING" stored as ["P", "ING"] should merge to "PING"
    let mut para = Paragraph::new();
    para.runs.push(TextRun::plain("P"));
    para.runs.push(TextRun::plain("ING"));
    para.merge_adjacent_runs();

    assert_eq!(para.runs.len(), 1);
    assert_eq!(para.runs[0].text, "PING"); // NOT "P ING"
}

#[test]
fn test_merge_adjacent_runs_tcp() {
    // Test fix: "TCP" stored as ["T", "CP"] should merge to "TCP"
    let mut para = Paragraph::new();
    para.runs.push(TextRun::plain("T"));
    para.runs.push(TextRun::plain("CP"));
    para.merge_adjacent_runs();

    assert_eq!(para.runs.len(), 1);
    assert_eq!(para.runs[0].text, "TCP"); // NOT "T CP"
}

#[test]
fn test_merge_adjacent_runs_cjk_ascii_no_space() {
    // Same-style runs merge WITHOUT space - even across script boundaries
    // Example: "VIP리소스" is a valid Korean compound where VIP is a brand/term
    let mut para = Paragraph::new();
    para.runs.push(TextRun::plain("리소스"));
    para.runs.push(TextRun::plain("DRBD"));
    para.merge_adjacent_runs();

    assert_eq!(para.runs.len(), 1);
    assert_eq!(para.runs[0].text, "리소스DRBD"); // No space - same-style runs = same word
}

#[test]
fn test_merge_adjacent_runs_ascii_cjk_no_space() {
    // Same-style runs merge WITHOUT space - even across script boundaries
    // Example: "CJ대한통운" is a brand name where CJ is part of the Korean name
    let mut para = Paragraph::new();
    para.runs.push(TextRun::plain("CJ"));
    para.runs.push(TextRun::plain("대한통운"));
    para.merge_adjacent_runs();

    assert_eq!(para.runs.len(), 1);
    assert_eq!(para.runs[0].text, "CJ대한통운"); // No space - brand name
}

#[test]
fn test_merge_adjacent_runs_korean_no_space() {
    // Korean: Same-style runs merge WITHOUT space (like ASCII, Chinese, Japanese)
    // Word may split a single word into multiple runs for various reasons.
    // Example: "정의" stored as ["정", "의"] should become "정의", not "정 의"
    let mut para = Paragraph::new();
    para.runs.push(TextRun::plain("네트워크"));
    para.runs.push(TextRun::plain("카드"));
    para.merge_adjacent_runs();

    assert_eq!(para.runs.len(), 1);
    assert_eq!(para.runs[0].text, "네트워크카드"); // No space - same word or compound
}

#[test]
fn test_merge_adjacent_runs_korean_syllables() {
    // When Word splits Korean syllables, they should merge without space
    // This was the bug: "정의" → "정 의" (wrong) instead of "정의" (correct)
    let mut para = Paragraph::new();
    para.runs.push(TextRun::plain("정"));
    para.runs.push(TextRun::plain("의"));
    para.merge_adjacent_runs();

    assert_eq!(para.runs.len(), 1);
    assert_eq!(para.runs[0].text, "정의"); // Syllables merge without space
}

#[test]
fn test_merge_adjacent_runs_korean_with_explicit_space() {
    // When source has explicit space, it's preserved in the run text
    let mut para = Paragraph::new();
    para.runs.push(TextRun::plain("서버 ")); // Note: space is in the text
    para.runs.push(TextRun::plain("리부팅"));
    para.merge_adjacent_runs();

    assert_eq!(para.runs.len(), 1);
    assert_eq!(para.runs[0].text, "서버 리부팅"); // Space preserved from source
}

#[test]
fn test_merge_adjacent_runs_chinese_no_space() {
    // Chinese: Same-style runs merge without space
    let mut para = Paragraph::new();
    para.runs.push(TextRun::plain("中文"));
    para.runs.push(TextRun::plain("测试"));
    para.merge_adjacent_runs();

    assert_eq!(para.runs.len(), 1);
    assert_eq!(para.runs[0].text, "中文测试"); // No space between Chinese
}

#[test]
fn test_merge_adjacent_runs_japanese_no_space() {
    // Japanese: Same-style runs merge without space
    let mut para = Paragraph::new();
    para.runs.push(TextRun::plain("日本語"));
    para.runs.push(TextRun::plain("テスト"));
    para.merge_adjacent_runs();

    assert_eq!(para.runs.len(), 1);
    assert_eq!(para.runs[0].text, "日本語テスト"); // No space between Japanese
}

#[test]
fn test_merge_adjacent_runs_different_styles_not_merged() {
    // Runs with different styles should NOT be merged
    let mut para = Paragraph::new();
    para.runs.push(TextRun::plain("normal"));
    para.runs.push(TextRun::styled("bold", TextStyle::bold()));
    para.merge_adjacent_runs();

    assert_eq!(para.runs.len(), 2); // Still 2 runs
    assert_eq!(para.runs[0].text, "normal");
    assert_eq!(para.runs[1].text, "bold");
}

#[test]
fn test_merge_preserves_existing_spaces() {
    // Existing spaces should be preserved
    let mut para = Paragraph::new();
    para.runs.push(TextRun::plain("Hello "));
    para.runs.push(TextRun::plain("World"));
    para.merge_adjacent_runs();

    assert_eq!(para.runs.len(), 1);
    assert_eq!(para.runs[0].text, "Hello World"); // Original space preserved
}

#[test]
fn test_merge_adjacent_runs_preserves_page_break() {
    let mut para = Paragraph {
        runs: vec![
            TextRun::plain("Before"),
            TextRun {
                text: "After".to_string(),
                page_break: true,
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    para.merge_adjacent_runs();

    assert!(
        para.runs.iter().any(|r| r.page_break),
        "page_break lost after merge: runs = {:?}",
        para.runs
    );
}

#[test]
fn test_merge_adjacent_runs_blocks_on_last_page_break() {
    let mut para = Paragraph {
        runs: vec![
            TextRun {
                text: "Before".to_string(),
                page_break: true,
                ..Default::default()
            },
            TextRun::plain("After"),
        ],
        ..Default::default()
    };
    para.merge_adjacent_runs();

    assert_eq!(para.runs.len(), 2, "must not merge across a page_break");
    assert!(para.runs[0].page_break);
    assert!(!para.runs[1].page_break);
}
