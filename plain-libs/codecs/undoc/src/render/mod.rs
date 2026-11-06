mod cleanup;
mod grid;
pub mod heading_analyzer;
mod markdown;
mod options;
pub mod style_mapping;

pub use cleanup::{clean_text, detect_mojibake};
pub use heading_analyzer::{DocumentStats, HeadingAnalyzer, HeadingConfig, HeadingDecision};
pub use markdown::{render_section_to_string, to_markdown};
pub use options::{
    CleanupOptions, CleanupPreset, RenderOptions, RevisionHandling, SectionMarkerStyle,
    TableFallback,
};
pub use style_mapping::StyleMapping;
