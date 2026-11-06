use super::*;
use crate::model::{RevisionType, TextRun, TextStyle};

fn make_paragraph(text: &str, heading: HeadingLevel) -> Paragraph {
    Paragraph {
        runs: vec![TextRun::plain(text)],
        heading,
        ..Default::default()
    }
}

fn make_bold_paragraph(text: &str, font_size: u32) -> Paragraph {
    Paragraph {
        runs: vec![TextRun {
            text: text.to_string(),
            style: TextStyle {
                bold: true,
                size: Some(font_size),
                ..Default::default()
            },
            hyperlink: None,
            line_break: false,
            page_break: false,
            revision: RevisionType::None,
            note: None,
        }],
        heading: HeadingLevel::None,
        ..Default::default()
    }
}

#[test]
fn test_explicit_heading_trusted() {
    // When trust_explicit_styles is true, explicit headings are ALWAYS trusted
    let config = HeadingConfig::default();
    let analyzer = HeadingAnalyzer::new(config);
    let para = make_paragraph("제목", HeadingLevel::H1);

    let decision = analyzer.decide_heading(&para);
    assert!(matches!(
        decision,
        HeadingDecision::Explicit(HeadingLevel::H1)
    ));
}

#[test]
fn test_explicit_heading_with_list_marker_trusted() {
    // When trust_explicit_styles is true, even list markers are trusted as headings
    let config = HeadingConfig::default();
    let analyzer = HeadingAnalyzer::new(config);
    let para = make_paragraph("ㅇ 항목 내용", HeadingLevel::H2);

    // Explicit style takes priority - list marker check is skipped
    let decision = analyzer.decide_heading(&para);
    assert!(matches!(
        decision,
        HeadingDecision::Explicit(HeadingLevel::H2)
    ));
}

#[test]
fn test_list_marker_demoted_when_untrusted() {
    // When trust_explicit_styles is false, list markers cause demotion
    let config = HeadingConfig::default().with_trust_explicit(false);
    let analyzer = HeadingAnalyzer::new(config);
    let para = make_paragraph("ㅇ 항목 내용", HeadingLevel::H2);

    let decision = analyzer.decide_heading(&para);
    assert_eq!(decision, HeadingDecision::Demoted);
}

#[test]
fn test_numbered_heading_preserved_when_standalone() {
    // Standalone numbered headings like "1. 첫 번째" are NOT demoted
    // Only consecutive patterns (1, 2, 3...) are demoted via sequence analysis
    let config = HeadingConfig::default().with_trust_explicit(false);
    let analyzer = HeadingAnalyzer::new(config);
    let para = make_paragraph("1. 첫 번째 항목", HeadingLevel::H2);

    let decision = analyzer.decide_heading(&para);
    // Explicit style is used as fallback (no bullet marker, not too long)
    assert!(matches!(
        decision,
        HeadingDecision::Explicit(HeadingLevel::H2)
    ));
}

#[test]
fn test_several_sentences_read_as_body_text() {
    assert!(reads_as_several_sentences(
        "이것은 매우 긴 문장입니다. 제목으로 쓰기에는 너무 깁니다."
    ));
    assert!(reads_as_several_sentences(
        "The results were mixed overall. Further work is needed."
    ));
    // A label or a number before the full stop is not a sentence.
    assert!(!reads_as_several_sentences("3. Overview of the method"));
    assert!(!reads_as_several_sentences("Fig. 2 shows the setup"));
    assert!(!reads_as_several_sentences(
        "Part II. Methods and materials"
    ));
    // Abbreviations inside a title.
    assert!(!reads_as_several_sentences(
        "HOMO ECONOMICUS VS. HOMO SAPIENS"
    ));
    assert!(!reads_as_several_sentences(
        "Results of Kim et al. on rice yield"
    ));
    // One sentence, however long, ending in its full stop.
    assert!(!reads_as_several_sentences(
        "A single long title that happens to end with a full stop."
    ));
}

/// The analyzer and the renderer's no-analyzer path apply one contract to an
/// explicit heading style: a long single-sentence title stays a heading even when
/// explicit styles are not trusted unconditionally.
#[test]
fn test_long_single_sentence_explicit_heading_kept_when_untrusted() {
    let config = HeadingConfig::default().with_trust_explicit(false);
    let analyzer = HeadingAnalyzer::new(config);
    let para = make_paragraph(
        "3. Perspective of supply and demand balance of wood pellets and cost structure in Japan",
        HeadingLevel::H1,
    );
    assert!(matches!(
        analyzer.decide_heading(&para),
        HeadingDecision::Explicit(HeadingLevel::H1)
    ));
}

#[test]
fn test_long_text_demoted_when_untrusted() {
    // When trust_explicit_styles is false, long text causes demotion
    let config = HeadingConfig::default().with_trust_explicit(false);
    let analyzer = HeadingAnalyzer::new(config);
    let long_text = "이것은 매우 긴 텍스트입니다. ".repeat(5);
    let para = make_paragraph(&long_text, HeadingLevel::H2);

    let decision = analyzer.decide_heading(&para);
    assert_eq!(decision, HeadingDecision::Demoted);
}

#[test]
fn test_list_marker_none_without_heading() {
    // Paragraphs without explicit heading and with list markers return None
    let config = HeadingConfig::default();
    let analyzer = HeadingAnalyzer::new(config);
    let para = make_paragraph("ㅇ 항목 내용", HeadingLevel::None);

    let decision = analyzer.decide_heading(&para);
    assert_eq!(decision, HeadingDecision::None);
}

#[test]
fn test_inferred_heading() {
    let config = HeadingConfig::default();
    let mut analyzer = HeadingAnalyzer::new(config);

    // Set up stats with base font size of 24 (12pt)
    analyzer.stats.base_font_size = Some(24);

    // Bold + large font (32 = 16pt, 1.33x larger)
    let para = make_bold_paragraph("추론된 제목", 32);

    let decision = analyzer.decide_heading(&para);
    assert!(matches!(decision, HeadingDecision::Inferred(_)));
}

#[test]
fn test_sequence_detection() {
    let config = HeadingConfig::default();
    let analyzer = HeadingAnalyzer::new(config);

    // Use extract_sequence_marker to test pattern detection
    assert!(analyzer.extract_sequence_marker("1. 항목").is_some());
    assert!(analyzer.extract_sequence_marker("2) 항목").is_some());
    assert!(analyzer.extract_sequence_marker("(3) 항목").is_some());
    assert!(analyzer.extract_sequence_marker("가. 항목").is_some());
    assert!(analyzer.extract_sequence_marker("a. 항목").is_some());

    assert!(analyzer.extract_sequence_marker("일반 텍스트").is_none());
    assert!(analyzer.extract_sequence_marker("제목").is_none());
}

#[test]
fn test_sequence_marker_extraction() {
    let config = HeadingConfig::default();
    let analyzer = HeadingAnalyzer::new(config);

    assert_eq!(
        analyzer.extract_sequence_marker("1. 항목"),
        Some("1".to_string())
    );
    assert_eq!(
        analyzer.extract_sequence_marker("2) 항목"),
        Some("2".to_string())
    );
    assert_eq!(
        analyzer.extract_sequence_marker("(3) 항목"),
        Some("3".to_string())
    );
    assert_eq!(
        analyzer.extract_sequence_marker("가. 항목"),
        Some("가".to_string())
    );
    assert_eq!(
        analyzer.extract_sequence_marker("a. 항목"),
        Some("a".to_string())
    );
}

#[test]
fn test_next_marker() {
    let config = HeadingConfig::default();
    let analyzer = HeadingAnalyzer::new(config);

    assert_eq!(analyzer.next_marker("1"), Some("2".to_string()));
    assert_eq!(analyzer.next_marker("9"), Some("10".to_string()));
    assert_eq!(analyzer.next_marker("가"), Some("나".to_string()));
    assert_eq!(analyzer.next_marker("a"), Some("b".to_string()));
}

#[test]
fn test_korean_sequence_patterns() {
    let config = HeadingConfig::default();
    let analyzer = HeadingAnalyzer::new(config);

    // Use extract_sequence_marker to test Korean sequence detection
    assert_eq!(
        analyzer.extract_sequence_marker("가. 첫째"),
        Some("가".to_string())
    );
    assert_eq!(
        analyzer.extract_sequence_marker("나) 둘째"),
        Some("나".to_string())
    );
    assert_eq!(
        analyzer.extract_sequence_marker("(다) 셋째"),
        Some("다".to_string())
    );
    assert!(analyzer.extract_sequence_marker("각. 항목").is_none()); // '각' is not in sequence
}

#[test]
fn test_arrow_marker_demoted_when_untrusted() {
    // When trust_explicit_styles is false, arrow markers cause demotion
    let config = HeadingConfig::default().with_trust_explicit(false);
    let analyzer = HeadingAnalyzer::new(config);
    let para = make_paragraph("→ 화살표 항목", HeadingLevel::H2);

    let decision = analyzer.decide_heading(&para);
    assert_eq!(decision, HeadingDecision::Demoted);
}

#[test]
fn test_max_heading_level_capped() {
    let config = HeadingConfig::default().with_max_level(2);
    let analyzer = HeadingAnalyzer::new(config);
    let para = make_paragraph("제목", HeadingLevel::H4);

    let decision = analyzer.decide_heading(&para);
    assert!(matches!(
        decision,
        HeadingDecision::Explicit(HeadingLevel::H2)
    ));
}

#[test]
fn test_sequence_analysis_demotes_consecutive() {
    // Test that consecutive numbered items are all demoted
    let config = HeadingConfig::default().with_trust_explicit(false);
    let analyzer = HeadingAnalyzer::new(config);

    let paras = [
        make_paragraph("1. 첫째", HeadingLevel::H2),
        make_paragraph("2. 둘째", HeadingLevel::H2),
        make_paragraph("3. 셋째", HeadingLevel::H2),
    ];
    let para_refs: Vec<&Paragraph> = paras.iter().collect();

    let decisions = analyzer.analyze_paragraphs(&para_refs);

    // All should be demoted due to sequential pattern
    assert!(decisions
        .iter()
        .all(|d| matches!(d, HeadingDecision::Demoted)));
}

#[test]
fn test_standalone_numbered_heading_preserved() {
    // Standalone numbered headings like "1. 서론" should NOT be demoted
    // when they are not part of a consecutive sequence
    let config = HeadingConfig::default().with_trust_explicit(false);
    let analyzer = HeadingAnalyzer::new(config);

    // Numbered headings separated by plain text (not consecutive)
    let paras = [
        make_paragraph("1. 서론", HeadingLevel::H2),
        make_paragraph("본문 내용입니다.", HeadingLevel::None),
        make_paragraph("2. 본론", HeadingLevel::H2),
    ];
    let para_refs: Vec<&Paragraph> = paras.iter().collect();

    let decisions = analyzer.analyze_paragraphs(&para_refs);

    // "1. 서론" and "2. 본론" are NOT consecutive (separated by plain text)
    // So they should be preserved as headings
    assert!(
        matches!(decisions[0], HeadingDecision::Explicit(HeadingLevel::H2)),
        "First heading should be preserved: {:?}",
        decisions[0]
    );
    assert!(
        matches!(decisions[2], HeadingDecision::Explicit(HeadingLevel::H2)),
        "Third heading should be preserved: {:?}",
        decisions[2]
    );
}

#[test]
fn test_numbered_heading_without_explicit_style() {
    // Numbered text without explicit heading style should return None (not Demoted)
    let config = HeadingConfig::default();
    let analyzer = HeadingAnalyzer::new(config);

    let para = make_paragraph("1. 서론", HeadingLevel::None);
    let decision = analyzer.decide_heading(&para);

    // No explicit style, no inference → None (not Demoted)
    assert_eq!(decision, HeadingDecision::None);
}

#[test]
fn test_numbered_heading_with_explicit_style_trusted() {
    // When trust_explicit_styles=true, numbered headings are preserved
    let config = HeadingConfig::default(); // trust_explicit_styles=true by default
    let analyzer = HeadingAnalyzer::new(config);

    let para = make_paragraph("1. 서론", HeadingLevel::H1);
    let decision = analyzer.decide_heading(&para);

    // Explicit style is trusted, so it's a heading regardless of number pattern
    assert!(matches!(
        decision,
        HeadingDecision::Explicit(HeadingLevel::H1)
    ));
}

#[test]
fn test_style_mapping_korean() {
    // Test that Korean style names are recognized via style mapping
    let config = HeadingConfig::default().with_default_style_mapping();
    let analyzer = HeadingAnalyzer::new(config);

    // Create paragraph with Korean style name but no explicit heading level
    let mut para = make_paragraph("제목 내용입니다", HeadingLevel::None);
    para.style_name = Some("제목 1".to_string());

    let decision = analyzer.decide_heading(&para);

    // Should be recognized as H1 via style mapping
    assert!(
        matches!(decision, HeadingDecision::Explicit(HeadingLevel::H1)),
        "Korean style name should be recognized: {:?}",
        decision
    );
}

#[test]
fn test_style_mapping_english() {
    // Test that English style names are recognized via style mapping
    let config = HeadingConfig::default().with_default_style_mapping();
    let analyzer = HeadingAnalyzer::new(config);

    // Create paragraph with English style name but no explicit heading level
    let mut para = make_paragraph("Some heading text", HeadingLevel::None);
    para.style_name = Some("Heading 2".to_string());

    let decision = analyzer.decide_heading(&para);

    // Should be recognized as H2 via style mapping
    assert!(
        matches!(decision, HeadingDecision::Explicit(HeadingLevel::H2)),
        "English style name should be recognized: {:?}",
        decision
    );
}

#[test]
fn test_style_mapping_takes_priority() {
    // Style mapping should take priority over explicit heading level
    let config = HeadingConfig::default().with_default_style_mapping();
    let analyzer = HeadingAnalyzer::new(config);

    // Create paragraph with style name and explicit heading level
    let mut para = make_paragraph("Title text", HeadingLevel::H3);
    para.style_name = Some("Title".to_string()); // Should map to H1

    let decision = analyzer.decide_heading(&para);

    // Style mapping (H1) should take priority over explicit (H3)
    assert!(
        matches!(decision, HeadingDecision::Explicit(HeadingLevel::H1)),
        "Style mapping should take priority: {:?}",
        decision
    );
}

#[test]
fn test_style_id_fallback() {
    // Test that style ID is used as fallback when style name is not set
    let config = HeadingConfig::default().with_default_style_mapping();
    let analyzer = HeadingAnalyzer::new(config);

    let mut para = make_paragraph("Heading text", HeadingLevel::None);
    para.style_id = Some("Heading3".to_string());
    para.style_name = None;

    let decision = analyzer.decide_heading(&para);

    // Should be recognized as H3 via style ID
    assert!(
        matches!(decision, HeadingDecision::Explicit(HeadingLevel::H3)),
        "Style ID should be recognized: {:?}",
        decision
    );
}
