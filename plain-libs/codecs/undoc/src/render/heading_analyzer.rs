//! Sophisticated heading detection with multi-level priority analysis.
//!
//! This module implements a two-pass analysis system for accurate heading detection:
//! - Pass 1: Collect document statistics (font sizes, patterns)
//! - Pass 2: Apply priority-based heading decisions
//!
//! Priority order:
//! 1. Explicit styles (outline_level) - always trusted
//! 2. Statistical inference (font size 1.2x + bold)
//! 3. Exclusion conditions (sequential numbers, list markers)

use std::collections::HashMap;

use super::style_mapping::StyleMapping;
use crate::model::{Block, Document, HeadingLevel, Paragraph, Section};

/// Bullet and symbol markers that open a list item rather than a title, Korean document
/// conventions included. Numbered prefixes (`1.`, `가.`) are not here: a standalone
/// "1. 서론" is a heading, and runs of them are told apart by sequence analysis.
const LIST_MARKERS: &[char] = &[
    '-', '*', '>', '–', '—', '※', '○', '•', '●', '◦', '◎', '□', '■', '▪', '▫', '◇', '◆', '★', '☆',
    '☐', '☑', '☒', '✓', '✗', 'ㅇ', // Korean jamo (circle)
    'ㆍ', // Korean middle dot (U+318D)
    '·',  // Middle dot (U+00B7)
    '∙',  // Bullet operator (U+2219)
    '→', '←', '↔', '⇒', '⇐', '⇔', '►', '▶', '▷', '▹', '▻', '◀', '◁', '◃', '◂',
];

/// Whether `text` opens with a bullet or symbol list marker.
pub(crate) fn looks_like_list_item(text: &str) -> bool {
    text.chars()
        .next()
        .is_some_and(|c| LIST_MARKERS.contains(&c))
}

/// Whether a paragraph with an explicit heading style is not a heading after all.
///
/// The style is the author's statement and is taken unless the paragraph plainly reads as
/// something else: a list item (it opens with a bullet marker), or body text — more than
/// one sentence. Length alone does not decide it: a long, single-sentence title
/// ("3. Perspective of supply and demand balance of wood pellets and cost structure in
/// Japan") is a title, while a paragraph of running text set in a heading style, as some
/// documents do, is several sentences.
pub(crate) fn explicit_heading_misused(text: &str) -> bool {
    looks_like_list_item(text) || reads_as_several_sentences(text)
}

/// Whether `text` holds a sentence boundary followed by more text: a terminator closing a
/// sentence of at least three words, then whitespace, then more. The word count keeps
/// "3. Overview", "Fig. 2 shows" and "Part II. Methods" from counting — what precedes
/// their full stop is a number or a label, not a sentence — and a short Latin word before
/// the stop ("vs.", "et al.") is an abbreviation.
fn reads_as_several_sentences(text: &str) -> bool {
    const TERMINATORS: &[char] = &['.', '!', '?', '。', '！', '？'];
    const MIN_SENTENCE_WORDS: usize = 3;

    let mut sentence_start = 0;
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if !TERMINATORS.contains(&c) {
            continue;
        }
        let end = i + c.len_utf8();
        let followed_by_text = chars.peek().is_some_and(|&(_, next)| next.is_whitespace())
            && !text[end..].trim().is_empty();
        if !followed_by_text {
            continue;
        }
        let sentence = &text[sentence_start..i];
        // A short Latin word before the stop is an abbreviation ("vs.", "etc.", "Dr.",
        // "et al.", "No."), not a sentence end.
        let abbreviation = sentence.split_whitespace().last().is_some_and(|word| {
            word.chars().count() <= 3 && word.chars().all(|ch| ch.is_ascii_alphabetic())
        });
        if abbreviation {
            continue;
        }
        if sentence.split_whitespace().count() >= MIN_SENTENCE_WORDS {
            return true;
        }
        sentence_start = end;
    }
    false
}

/// Configuration for heading analysis.
#[derive(Debug, Clone)]
pub struct HeadingConfig {
    /// Maximum heading level to emit (1-6).
    pub max_heading_level: u8,

    /// Maximum text length for a paragraph *without* a heading style to be inferred as a
    /// heading. A paragraph with an explicit heading style is not measured by length: a
    /// long title is a title. See [`explicit_heading_misused`].
    pub max_text_length: usize,

    /// Font size ratio threshold (compared to base font).
    /// Default: 1.2 (120% of base font size)
    pub size_threshold_ratio: f32,

    /// Trust explicit styles (outline_level) unconditionally.
    pub trust_explicit_styles: bool,

    /// Analyze sequential patterns to detect lists.
    pub analyze_sequences: bool,

    /// Minimum consecutive items to consider as a list.
    pub min_sequence_count: usize,

    /// Style name to heading level mapping.
    /// When set, style names are checked first before other detection methods.
    pub style_mapping: Option<StyleMapping>,
}

impl Default for HeadingConfig {
    fn default() -> Self {
        Self {
            max_heading_level: 4,
            max_text_length: 80,
            size_threshold_ratio: 1.2,
            trust_explicit_styles: true,
            analyze_sequences: true,
            min_sequence_count: 2,
            style_mapping: None,
        }
    }
}

impl HeadingConfig {
    /// Create a new heading config with default settings.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the maximum heading level.
    pub fn with_max_level(mut self, level: u8) -> Self {
        self.max_heading_level = level.clamp(1, 6);
        self
    }

    /// Set the maximum text length for headings.
    pub fn with_max_text_length(mut self, length: usize) -> Self {
        self.max_text_length = length;
        self
    }

    /// Set the font size ratio threshold.
    pub fn with_size_ratio(mut self, ratio: f32) -> Self {
        self.size_threshold_ratio = ratio.max(1.0);
        self
    }

    /// Set whether to trust explicit styles.
    pub fn with_trust_explicit(mut self, trust: bool) -> Self {
        self.trust_explicit_styles = trust;
        self
    }

    /// Set whether to analyze sequences.
    pub fn with_sequence_analysis(mut self, analyze: bool) -> Self {
        self.analyze_sequences = analyze;
        self
    }

    /// Set style mapping for name-based heading detection.
    pub fn with_style_mapping(mut self, mapping: StyleMapping) -> Self {
        self.style_mapping = Some(mapping);
        self
    }

    /// Enable default style mapping (English and Korean style names).
    pub fn with_default_style_mapping(mut self) -> Self {
        self.style_mapping = Some(StyleMapping::with_defaults());
        self
    }
}

/// Statistics collected from a document for heading analysis.
#[derive(Debug, Clone, Default)]
pub struct DocumentStats {
    /// Font size distribution (size in half-points → occurrence count).
    pub font_sizes: HashMap<u32, usize>,

    /// Detected base font size (most frequent, in half-points).
    pub base_font_size: Option<u32>,

    /// Number of bold paragraphs.
    pub bold_paragraphs: usize,

    /// Total number of paragraphs.
    pub total_paragraphs: usize,

    /// Number of paragraphs with explicit heading styles.
    pub explicit_heading_count: usize,
}

impl DocumentStats {
    /// Calculate the base (body) font size from the distribution.
    /// Uses the most frequent font size as the baseline.
    pub fn calculate_base_font_size(&mut self) {
        self.base_font_size = self
            .font_sizes
            .iter()
            .max_by_key(|(_, count)| *count)
            .map(|(size, _)| *size);
    }

    /// Check if a font size is significantly larger than the base.
    pub fn is_larger_than_base(&self, size: u32, ratio: f32) -> bool {
        if let Some(base) = self.base_font_size {
            let threshold = (base as f32 * ratio) as u32;
            size >= threshold
        } else {
            false
        }
    }
}

/// Result of heading analysis for a paragraph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeadingDecision {
    /// Heading from explicit style (outline_level).
    Explicit(HeadingLevel),

    /// Heading inferred from formatting (font size + bold).
    Inferred(HeadingLevel),

    /// Demoted from heading to normal paragraph (e.g., list item).
    Demoted,

    /// Not a heading.
    None,
}

impl HeadingDecision {
    /// Check if this decision results in a heading.
    pub fn is_heading(&self) -> bool {
        matches!(
            self,
            HeadingDecision::Explicit(_) | HeadingDecision::Inferred(_)
        )
    }

    /// Get the heading level if this is a heading.
    pub fn level(&self) -> Option<HeadingLevel> {
        match self {
            HeadingDecision::Explicit(level) | HeadingDecision::Inferred(level) => Some(*level),
            _ => None,
        }
    }
}

/// Analyzer for sophisticated heading detection.
pub struct HeadingAnalyzer {
    config: HeadingConfig,
    stats: DocumentStats,
}

impl HeadingAnalyzer {
    /// Create a new heading analyzer with the given configuration.
    pub fn new(config: HeadingConfig) -> Self {
        Self {
            config,
            stats: DocumentStats::default(),
        }
    }

    /// Create a heading analyzer with default configuration.
    pub fn with_defaults() -> Self {
        Self::new(HeadingConfig::default())
    }

    /// Analyze a document and return heading decisions for all paragraphs.
    ///
    /// This performs a two-pass analysis:
    /// 1. Collect statistics from all paragraphs
    /// 2. Make heading decisions based on priorities
    pub fn analyze(&mut self, doc: &Document) -> Vec<Vec<HeadingDecision>> {
        // Pass 1: Collect statistics
        self.collect_stats(doc);

        // Pass 2: Analyze headings
        self.analyze_sections(&doc.sections)
    }

    /// Analyze sections (for use with pre-collected stats).
    pub fn analyze_sections(&self, sections: &[Section]) -> Vec<Vec<HeadingDecision>> {
        sections
            .iter()
            .map(|section| self.analyze_section(section))
            .collect()
    }

    /// Analyze a single section.
    fn analyze_section(&self, section: &Section) -> Vec<HeadingDecision> {
        let paragraphs: Vec<&Paragraph> = section
            .content
            .iter()
            .filter_map(|block| {
                if let Block::Paragraph(para) = block {
                    Some(para)
                } else {
                    None
                }
            })
            .collect();

        self.analyze_paragraphs(&paragraphs)
    }

    /// Analyze a sequence of paragraphs with context awareness.
    fn analyze_paragraphs(&self, paragraphs: &[&Paragraph]) -> Vec<HeadingDecision> {
        let mut decisions = Vec::with_capacity(paragraphs.len());

        // First pass: make initial decisions
        for para in paragraphs {
            decisions.push(self.decide_heading(para));
        }

        // Second pass: check sequential patterns if enabled
        if self.config.analyze_sequences {
            self.apply_sequence_analysis(paragraphs, &mut decisions);
        }

        decisions
    }

    /// Collect statistics from the document (Pass 1).
    fn collect_stats(&mut self, doc: &Document) {
        self.stats = DocumentStats::default();

        for section in &doc.sections {
            for block in &section.content {
                if let Block::Paragraph(para) = block {
                    self.collect_paragraph_stats(para);
                }
            }
        }

        self.stats.calculate_base_font_size();
    }

    /// Collect statistics from a single paragraph.
    fn collect_paragraph_stats(&mut self, para: &Paragraph) {
        self.stats.total_paragraphs += 1;

        // Check for explicit heading
        if para.heading.is_heading() {
            self.stats.explicit_heading_count += 1;
        }

        // Collect font sizes and check for bold
        let mut has_bold = false;
        for run in &para.runs {
            if let Some(size) = run.style.size {
                *self.stats.font_sizes.entry(size).or_insert(0) += 1;
            }
            if run.style.bold {
                has_bold = true;
            }
        }

        if has_bold {
            self.stats.bold_paragraphs += 1;
        }
    }

    /// Make a heading decision for a single paragraph (Pass 2).
    fn decide_heading(&self, para: &Paragraph) -> HeadingDecision {
        let plain_text = para.plain_text();
        let trimmed = plain_text.trim();

        // P0: Style mapping takes highest priority (before explicit styles)
        // This allows style name like "제목 1" to be recognized as heading
        if let Some(ref mapping) = self.config.style_mapping {
            if let Some(level) = mapping.get(para.style_id.as_deref(), para.style_name.as_deref()) {
                let capped = self.cap_heading_level(level);
                return HeadingDecision::Explicit(capped);
            }
        }

        // P1: Explicit style with full trust (skip all exclusion checks)
        if para.heading.is_heading() && self.config.trust_explicit_styles {
            let level = self.cap_heading_level(para.heading);
            return HeadingDecision::Explicit(level);
        }

        // P3: Exclusion conditions. An explicit heading style is demoted only when the
        // paragraph reads as something else — the same contract the renderer applies when
        // no analyzer runs.
        if para.heading.is_heading() {
            if explicit_heading_misused(trimmed) {
                return HeadingDecision::Demoted;
            }
        } else if looks_like_list_item(trimmed)
            || trimmed.chars().count() > self.config.max_text_length
        {
            // Numbered patterns are handled in sequence analysis only.
            return HeadingDecision::None;
        }

        // P2: Statistical inference (for paragraphs without explicit style)
        if let Some(inferred) = self.infer_heading_from_style(para) {
            return HeadingDecision::Inferred(inferred);
        }

        // Fallback: Use explicit style if present (even when trust=false)
        // This handles numbered headings like "1. 서론" with explicit H1 style
        // They pass exclusion checks (no bullet marker, not too long)
        // Sequence analysis may still demote them if they form a consecutive pattern
        if para.heading.is_heading() {
            let level = self.cap_heading_level(para.heading);
            return HeadingDecision::Explicit(level);
        }

        HeadingDecision::None
    }

    /// Infer heading level from text style (font size + bold).
    fn infer_heading_from_style(&self, para: &Paragraph) -> Option<HeadingLevel> {
        // Need at least one run with text
        if para.runs.is_empty() || para.plain_text().trim().is_empty() {
            return None;
        }

        // Check if all runs are bold
        let all_bold = para
            .runs
            .iter()
            .filter(|r| !r.text.is_empty())
            .all(|r| r.style.bold);

        // Get the dominant font size
        let dominant_size = self.get_dominant_font_size(para);

        // Need both bold and larger font size
        if !all_bold {
            return None;
        }

        if let Some(size) = dominant_size {
            if self
                .stats
                .is_larger_than_base(size, self.config.size_threshold_ratio)
            {
                // Infer level based on size ratio
                let level = self.infer_level_from_size(size);
                return Some(self.cap_heading_level(level));
            }
        }

        None
    }

    /// Get the dominant (most frequent) font size in a paragraph.
    fn get_dominant_font_size(&self, para: &Paragraph) -> Option<u32> {
        let mut sizes: HashMap<u32, usize> = HashMap::new();

        for run in &para.runs {
            if let Some(size) = run.style.size {
                let text_len = run.text.chars().count();
                *sizes.entry(size).or_insert(0) += text_len;
            }
        }

        sizes
            .into_iter()
            .max_by_key(|(_, count)| *count)
            .map(|(size, _)| size)
    }

    /// Infer heading level from font size.
    fn infer_level_from_size(&self, size: u32) -> HeadingLevel {
        let base = self.stats.base_font_size.unwrap_or(24); // Default 12pt = 24 half-points

        let ratio = size as f32 / base as f32;

        if ratio >= 2.0 {
            HeadingLevel::H1
        } else if ratio >= 1.5 {
            HeadingLevel::H2
        } else if ratio >= 1.2 {
            HeadingLevel::H3
        } else {
            HeadingLevel::H4
        }
    }

    /// Cap heading level to configured maximum.
    fn cap_heading_level(&self, level: HeadingLevel) -> HeadingLevel {
        let current = level.level();
        if current > self.config.max_heading_level {
            HeadingLevel::from_number(self.config.max_heading_level)
        } else {
            level
        }
    }

    /// Apply sequence analysis to detect list patterns.
    fn apply_sequence_analysis(
        &self,
        paragraphs: &[&Paragraph],
        decisions: &mut [HeadingDecision],
    ) {
        if paragraphs.len() < self.config.min_sequence_count {
            return;
        }

        // Find sequences of numbered paragraphs
        let mut i = 0;
        while i < paragraphs.len() {
            if let Some(seq_len) = self.detect_sequence_at(paragraphs, i) {
                if seq_len >= self.config.min_sequence_count {
                    // Demote all items in the sequence
                    for decision in decisions.iter_mut().skip(i).take(seq_len) {
                        if decision.is_heading() {
                            *decision = HeadingDecision::Demoted;
                        }
                    }
                    i += seq_len;
                    continue;
                }
            }
            i += 1;
        }
    }

    /// Detect a numbered sequence starting at the given index.
    /// Returns the length of the sequence if found.
    fn detect_sequence_at(&self, paragraphs: &[&Paragraph], start: usize) -> Option<usize> {
        let first_text = paragraphs[start].plain_text();
        let first_trimmed = first_text.trim();

        // Try to parse the first number/marker
        let first_marker = self.extract_sequence_marker(first_trimmed)?;

        let mut seq_len = 1;
        let mut expected_next = self.next_marker(&first_marker)?;

        for para in paragraphs.iter().skip(start + 1) {
            let text = para.plain_text();
            let trimmed = text.trim();

            if let Some(marker) = self.extract_sequence_marker(trimmed) {
                if marker == expected_next {
                    seq_len += 1;
                    if let Some(next) = self.next_marker(&marker) {
                        expected_next = next;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }

        if seq_len >= 2 {
            Some(seq_len)
        } else {
            None
        }
    }

    /// Extract a sequence marker from text (e.g., "1", "가", "a").
    fn extract_sequence_marker(&self, text: &str) -> Option<String> {
        let text = text.trim_start();
        if text.is_empty() {
            return None;
        }

        let chars: Vec<char> = text.chars().take(10).collect();

        // Check "(N)" pattern
        if chars[0] == '(' {
            if let Some(close_idx) = chars.iter().position(|&c| c == ')') {
                let inner: String = chars[1..close_idx].iter().collect();
                if !inner.is_empty()
                    && (inner.chars().all(|c| c.is_ascii_digit())
                        || inner.chars().count() == 1
                            && inner.chars().next().is_some_and(|c| c.is_ascii_lowercase())
                        || inner.chars().count() == 1
                            && inner.chars().next().is_some_and(is_korean_sequence_char))
                {
                    return Some(inner);
                }
            }
        }

        // Check "N." or "N)" pattern for numbers
        let mut num_end = 0;
        for (i, &c) in chars.iter().enumerate() {
            if c.is_ascii_digit() {
                num_end = i + 1;
            } else {
                break;
            }
        }

        if num_end > 0 && num_end < chars.len() {
            let next = chars[num_end];
            if next == '.' || next == ')' {
                return Some(chars[..num_end].iter().collect());
            }
        }

        // Check Korean "가." pattern
        if chars.len() >= 2
            && is_korean_sequence_char(chars[0])
            && (chars[1] == '.' || chars[1] == ')')
        {
            return Some(chars[0].to_string());
        }

        // Check "a." or "a)" pattern
        if chars.len() >= 2 && chars[0].is_ascii_lowercase() && (chars[1] == '.' || chars[1] == ')')
        {
            return Some(chars[0].to_string());
        }

        None
    }

    /// Get the next expected marker in a sequence.
    fn next_marker(&self, marker: &str) -> Option<String> {
        // Number sequence
        if let Ok(n) = marker.parse::<u32>() {
            return Some((n + 1).to_string());
        }

        // Single character sequences (Korean, alphabetic)
        if marker.chars().count() == 1 {
            let c = marker.chars().next()?;

            // Korean "가나다라..." sequence
            const KOREAN_SEQ: &[char] = &[
                '가', '나', '다', '라', '마', '바', '사', '아', '자', '차', '카', '타', '파', '하',
            ];
            if let Some(idx) = KOREAN_SEQ.iter().position(|&x| x == c) {
                if idx + 1 < KOREAN_SEQ.len() {
                    return Some(KOREAN_SEQ[idx + 1].to_string());
                }
            }

            // Alphabetic sequence
            if c.is_ascii_lowercase() && c != 'z' {
                return Some(((c as u8) + 1) as char).map(|c| c.to_string());
            }
        }

        None
    }

    /// Get the collected document statistics.
    pub fn stats(&self) -> &DocumentStats {
        &self.stats
    }

    /// Get a reference to the configuration.
    pub fn config(&self) -> &HeadingConfig {
        &self.config
    }
}

/// Check if a character is part of the Korean sequence (가나다라...).
fn is_korean_sequence_char(c: char) -> bool {
    const KOREAN_SEQ: &[char] = &[
        '가', '나', '다', '라', '마', '바', '사', '아', '자', '차', '카', '타', '파', '하',
    ];
    KOREAN_SEQ.contains(&c)
}

#[cfg(test)]
#[path = "../../tests/unit/render/heading_analyzer.rs"]
mod tests;
