use super::*;

#[test]
fn test_default_options() {
    let opts = RenderOptions::default();
    assert!(!opts.include_frontmatter);
    assert_eq!(opts.table_fallback, TableFallback::Markdown);
    // Default max_heading_level is 4 (not 6) because Office documents
    // often misuse deep heading levels for visual styling
    assert_eq!(opts.max_heading_level, 4);
}

#[test]
fn test_builder_pattern() {
    let opts = RenderOptions::new().with_frontmatter(true).with_cleanup();

    assert!(opts.include_frontmatter);
    assert!(opts.cleanup.is_some());
}

#[test]
fn test_cleanup_presets() {
    let minimal = CleanupOptions::minimal();
    assert!(minimal.normalize_strings);
    assert!(!minimal.clean_lines);

    let aggressive = CleanupOptions::aggressive();
    assert!(aggressive.normalize_strings);
    assert!(aggressive.clean_lines);
    assert!(aggressive.filter_structure);
    assert!(aggressive.remove_pua);
}

#[test]
fn test_max_heading_clamp() {
    let opts = RenderOptions::new().with_max_heading(10);
    assert_eq!(opts.max_heading_level, 6);

    let opts = RenderOptions::new().with_max_heading(0);
    assert_eq!(opts.max_heading_level, 1);
}

#[test]
fn test_section_marker_style_default_is_none() {
    let opts = RenderOptions::new();
    assert_eq!(opts.section_markers, SectionMarkerStyle::None);
}

#[test]
fn test_section_marker_style_builder() {
    let opts = RenderOptions::new().with_section_markers(SectionMarkerStyle::Comment);
    assert_eq!(opts.section_markers, SectionMarkerStyle::Comment);
}
