use crate::regex_engine::automata::{MatchKind, util::syntax};
pub use crate::regex_engine::automata::{meta::Regex, util::captures::Captures};
pub use crate::regex_engine::syntax::escape;

pub fn compile(pattern: &str) -> Result<Regex, crate::regex_engine::automata::meta::BuildError> {
    // Preserve the regex 1.13 string API's UTF-8 rules and compilation limits.
    Regex::builder()
        .configure(
            Regex::config()
                .match_kind(MatchKind::LeftmostFirst)
                .utf8_empty(true)
                .nfa_size_limit(Some(10 * 1024 * 1024))
                .hybrid_cache_capacity(2 * 1024 * 1024),
        )
        .syntax(syntax::Config::default().utf8(true))
        .build(pattern)
}

pub fn captures(regex: &Regex, text: &str) -> Option<Captures> {
    let mut captures = regex.create_captures();
    regex.captures(text, &mut captures);
    captures.is_match().then_some(captures)
}

#[cfg(test)]
#[path = "../../tests/plain-libs/unit/regex.rs"]
mod tests;

#[cfg(test)]
#[path = "../../tests/unit/regex.rs"]
mod consolidation_tests;
