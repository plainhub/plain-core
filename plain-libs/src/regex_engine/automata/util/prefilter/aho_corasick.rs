use crate::regex_engine::automata::util::{
    prefilter::PrefilterI,
    search::{MatchKind, Span},
};
#[derive(Clone, Debug)]
pub(crate) struct AhoCorasick {
    ac: crate::regex_engine::aho::AhoCorasick,
}
impl AhoCorasick {
    pub(crate) fn new<B: AsRef<[u8]>>(kind: MatchKind, needles: &[B]) -> Option<AhoCorasick> {
        {
            let ac_match_kind = match kind {
                MatchKind::LeftmostFirst | MatchKind::All => {
                    crate::regex_engine::aho::MatchKind::LeftmostFirst
                }
            };
            let ac_kind = if needles.len() <= 500 {
                crate::regex_engine::aho::AhoCorasickKind::DFA
            } else {
                crate::regex_engine::aho::AhoCorasickKind::ContiguousNFA
            };
            let result = crate::regex_engine::aho::AhoCorasick::builder()
                .kind(Some(ac_kind))
                .match_kind(ac_match_kind)
                .start_kind(crate::regex_engine::aho::StartKind::Both)
                .prefilter(false)
                .build(needles);
            let ac = match result {
                Ok(ac) => ac,
                Err(_err) => {
                    debug!("aho-corasick prefilter failed to build: {_err}");
                    return None;
                }
            };
            Some(AhoCorasick { ac })
        }
    }
}
impl PrefilterI for AhoCorasick {
    fn name(&self) -> &'static str {
        "aho-corasick"
    }
    fn find(&self, haystack: &[u8], span: Span) -> Option<Span> {
        {
            let input = crate::regex_engine::aho::Input::new(haystack).span(span.start..span.end);
            self.ac.find(input).map(|m| Span {
                start: m.start(),
                end: m.end(),
            })
        }
    }
    fn prefix(&self, haystack: &[u8], span: Span) -> Option<Span> {
        {
            let input = crate::regex_engine::aho::Input::new(haystack)
                .anchored(crate::regex_engine::aho::Anchored::Yes)
                .span(span.start..span.end);
            self.ac.find(input).map(|m| Span {
                start: m.start(),
                end: m.end(),
            })
        }
    }
    fn memory_usage(&self) -> usize {
        { self.ac.memory_usage() }
    }
    fn is_fast(&self) -> bool {
        { false }
    }
}
