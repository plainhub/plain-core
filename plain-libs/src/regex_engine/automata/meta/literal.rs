use crate::regex_engine::automata::{meta::regex::RegexInfo, util::search::MatchKind};
use crate::regex_engine::syntax::hir::Hir;
use std::{vec, vec::Vec};
pub(crate) fn alternation_literals(info: &RegexInfo, hirs: &[&Hir]) -> Option<Vec<Vec<u8>>> {
    use crate::regex_engine::syntax::hir::{HirKind, Literal};
    if !cfg!(all()) {
        return None;
    }
    if hirs.len() != 1
        || !info.props()[0].look_set().is_empty()
        || info.props()[0].explicit_captures_len() > 0
        || !info.props()[0].is_alternation_literal()
        || info.config().get_match_kind() != MatchKind::LeftmostFirst
    {
        return None;
    }
    let hir = &hirs[0];
    let alts = match *hir.kind() {
        HirKind::Alternation(ref alts) => alts,
        _ => return None,
    };
    let mut lits = vec![];
    for alt in alts {
        let mut lit = vec![];
        match *alt.kind() {
            HirKind::Literal(Literal(ref bytes)) => lit.extend_from_slice(bytes),
            HirKind::Concat(ref exprs) => {
                for e in exprs {
                    match *e.kind() {
                        HirKind::Literal(Literal(ref bytes)) => {
                            lit.extend_from_slice(bytes);
                        }
                        _ => unreachable!("expected literal, got {e:?}"),
                    }
                }
            }
            _ => unreachable!("expected literal or concat, got {alt:?}"),
        }
        lits.push(lit);
    }
    if lits.len() < 3000 {
        debug!("skipping Aho-Corasick because there are too few literals");
        return None;
    }
    Some(lits)
}
