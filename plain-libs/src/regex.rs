use regex_automata::{MatchKind, util::syntax};
pub use regex_automata::{meta::Regex, util::captures::Captures};
pub use regex_syntax::escape;

pub fn compile(pattern: &str) -> Result<Regex, regex_automata::meta::BuildError> {
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
