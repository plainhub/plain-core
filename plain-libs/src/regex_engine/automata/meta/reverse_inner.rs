use crate::regex_engine::automata::{MatchKind, meta::prefix, util::prefilter::Prefilter};
use crate::regex_engine::syntax::hir::{
    self, Hir, HirKind,
    literal::{self, Literal},
};
use std::vec::Vec;
pub(super) fn has_no_earlier_match(concat_prefix: &Hir, literals: &[Literal]) -> bool {
    if literals.is_empty() || literals.iter().any(|lit| lit.is_empty()) {
        debug!(
            "reverse inner is not early return safe because \
                 no non-empty inner literals were found"
        );
        return false;
    }
    if literals.len() == 1 {
        let prefix_may_contain =
            prefix::hir_can_contain_literal(concat_prefix, literals[0].as_bytes());
        debug!(
            "reverse inner prefix can contain inner literals? \
             {prefix_may_contain}"
        );
        if !prefix_may_contain {
            return true;
        }
    }
    let fixed_length = prefix::hir_has_fixed_length(concat_prefix);
    debug!("reverse inner has fixed length prefix? {fixed_length}");
    if fixed_length {
        return true;
    }
    let class_separator = prefix::has_disjoint_class_separator(concat_prefix, &literals);
    debug!("reverse inner has disjoint class separator? {class_separator}");
    if class_separator {
        return true;
    }
    false
}
#[derive(Debug)]
pub(crate) struct InnerPrefilter {
    pub(crate) prefix: Hir,
    pub(crate) pre: Prefilter,
    pub(crate) literals: Vec<Literal>,
}
impl InnerPrefilter {
    pub(crate) fn new(hirs: &[&Hir]) -> Option<InnerPrefilter> {
        if hirs.len() != 1 {
            debug!(
                "skipping reverse inner optimization since it only \
                 supports 1 pattern, {} were given",
                hirs.len(),
            );
            return None;
        }
        let mut concat = match top_concat(hirs[0]) {
            Some(concat) => concat,
            None => {
                debug!(
                    "skipping reverse inner optimization because a top-level \
                     concatenation could not found",
                );
                return None;
            }
        };
        for i in 1..concat.len() {
            let hir = &concat[i];
            let (pre, lits) = match prefilter_with_literals(hir) {
                None => continue,
                Some(pre) => pre,
            };
            if !pre.is_fast() {
                debug!(
                    "skipping extracted inner prefilter because \
                     it probably isn't fast"
                );
                continue;
            }
            let concat_suffix = Hir::concat(concat.split_off(i));
            let concat_prefix = Hir::concat(concat);
            let (preinner, inner_literals) = match prefilter_with_literals(&concat_suffix) {
                None => (pre, lits),
                Some((pre2, lits2)) => {
                    if pre2.is_fast() {
                        (pre2, lits2)
                    } else {
                        (pre, lits)
                    }
                }
            };
            return Some(InnerPrefilter {
                prefix: concat_prefix,
                pre: preinner,
                literals: inner_literals,
            });
        }
        debug!(
            "skipping reverse inner optimization because a top-level \
             sub-expression with a fast prefilter could not be found"
        );
        None
    }
}
fn prefilter_with_literals(hir: &Hir) -> Option<(Prefilter, Vec<Literal>)> {
    let mut extractor = literal::Extractor::new();
    extractor.kind(literal::ExtractKind::Prefix);
    let mut prefixes = extractor.extract(hir);
    debug!(
        "inner prefixes (len={:?}) extracted before optimization: {:?}",
        prefixes.len(),
        prefixes
    );
    prefixes.make_inexact();
    prefixes.optimize_for_prefix_by_preference();
    debug!(
        "inner prefixes (len={:?}) extracted after optimization: {:?}",
        prefixes.len(),
        prefixes
    );
    let lits = prefixes.literals()?;
    let pre = Prefilter::new(MatchKind::LeftmostFirst, lits)?;
    Some((pre, lits.to_vec()))
}
fn top_concat(mut hir: &Hir) -> Option<Vec<Hir>> {
    loop {
        hir = match hir.kind() {
            HirKind::Empty
            | HirKind::Literal(_)
            | HirKind::Class(_)
            | HirKind::Look(_)
            | HirKind::Repetition(_)
            | HirKind::Alternation(_) => return None,
            HirKind::Capture(hir::Capture { sub, .. }) => sub,
            HirKind::Concat(subs) => {
                let concat = Hir::concat(subs.iter().map(|h| flatten(h)).collect());
                return match concat.into_kind() {
                    HirKind::Concat(xs) => Some(xs),
                    _ => return None,
                };
            }
        };
    }
}
fn flatten(hir: &Hir) -> Hir {
    match hir.kind() {
        HirKind::Empty => Hir::empty(),
        HirKind::Literal(hir::Literal(x)) => Hir::literal(x.clone()),
        HirKind::Class(x) => Hir::class(x.clone()),
        HirKind::Look(x) => Hir::look(x.clone()),
        HirKind::Repetition(x) => Hir::repetition(x.with(flatten(&x.sub))),
        HirKind::Capture(hir::Capture { sub, .. }) => flatten(sub),
        HirKind::Alternation(xs) => Hir::alternation(xs.iter().map(|x| flatten(x)).collect()),
        HirKind::Concat(xs) => Hir::concat(xs.iter().map(|x| flatten(x)).collect()),
    }
}
