use crate::regex_engine::automata::util::{
    prefilter::PrefilterI,
    search::{MatchKind, Span},
};
#[derive(Clone, Debug)]
pub(crate) struct Teddy {
    searcher: crate::regex_engine::aho::packed::Searcher,
    anchored_ac: crate::regex_engine::aho::dfa::DFA,
    minimum_len: usize,
}
impl Teddy {
    pub(crate) fn new<B: AsRef<[u8]>>(kind: MatchKind, needles: &[B]) -> Option<Teddy> {
        {
            let (packed_match_kind, ac_match_kind) = match kind {
                MatchKind::LeftmostFirst | MatchKind::All => (
                    crate::regex_engine::aho::packed::MatchKind::LeftmostFirst,
                    crate::regex_engine::aho::MatchKind::LeftmostFirst,
                ),
            };
            let minimum_len = needles.iter().map(|n| n.as_ref().len()).min().unwrap_or(0);
            let packed = crate::regex_engine::aho::packed::Config::new()
                .match_kind(packed_match_kind)
                .builder()
                .extend(needles)
                .build()?;
            let anchored_ac = crate::regex_engine::aho::dfa::DFA::builder()
                .match_kind(ac_match_kind)
                .start_kind(crate::regex_engine::aho::StartKind::Anchored)
                .prefilter(false)
                .build(needles)
                .ok()?;
            Some(Teddy {
                searcher: packed,
                anchored_ac,
                minimum_len,
            })
        }
    }
}
impl PrefilterI for Teddy {
    fn name(&self) -> &'static str {
        "teddy"
    }
    fn find(&self, haystack: &[u8], span: Span) -> Option<Span> {
        {
            let ac_span = crate::regex_engine::aho::Span {
                start: span.start,
                end: span.end,
            };
            self.searcher.find_in(haystack, ac_span).map(|m| Span {
                start: m.start(),
                end: m.end(),
            })
        }
    }
    fn prefix(&self, haystack: &[u8], span: Span) -> Option<Span> {
        {
            use crate::regex_engine::aho::automaton::Automaton;
            let input = crate::regex_engine::aho::Input::new(haystack)
                .anchored(crate::regex_engine::aho::Anchored::Yes)
                .span(span.start..span.end);
            self.anchored_ac
                .try_find(&input)
                .expect("aho-corasick DFA should never fail")
                .map(|m| Span {
                    start: m.start(),
                    end: m.end(),
                })
        }
    }
    fn memory_usage(&self) -> usize {
        {
            use crate::regex_engine::aho::automaton::Automaton;
            self.searcher.memory_usage() + self.anchored_ac.memory_usage()
        }
    }
    fn is_fast(&self) -> bool {
        { self.minimum_len >= 3 }
    }
}
