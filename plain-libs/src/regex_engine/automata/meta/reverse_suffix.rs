use crate::regex_engine::automata::meta::prefix;
use crate::regex_engine::syntax::hir::{Hir, HirKind, Repetition, literal::Literal};
use std::vec::Vec;
pub(super) fn has_no_earlier_match(hirs: &[&Hir], suffix: &[u8]) -> bool {
    if hirs.len() != 1 || suffix.is_empty() {
        return false;
    }
    let hir = hirs[0];
    let Some(prefix) = strip_literal_suffix(hir, suffix) else {
        return false;
    };
    let fixed_length = prefix::hir_has_fixed_length(&prefix);
    debug!("reverse suffix has fixed length prefix? {fixed_length}");
    if fixed_length {
        return true;
    }
    let class_separator =
        prefix::has_disjoint_class_separator(&prefix, &[Literal::inexact(suffix)]);
    debug!("reverse suffix has disjoint class separator? {class_separator}");
    if class_separator {
        return true;
    }
    let internal = prefix::hir_can_contain_literal(&prefix, suffix);
    debug!("reverse suffix has internal suffix? {internal}");
    if !internal {
        return true;
    }
    false
}
fn strip_literal_suffix(hir: &Hir, suffix: &[u8]) -> Option<Hir> {
    let (prefix, suffix_cursor) = strip_literal_suffix_at(hir, suffix, suffix.len())?;
    if suffix_cursor == 0 {
        Some(prefix)
    } else {
        None
    }
}
fn strip_literal_suffix_at(hir: &Hir, suffix: &[u8], suffix_cursor: usize) -> Option<(Hir, usize)> {
    if suffix_cursor == 0 {
        return Some((hir.clone(), 0));
    }
    match hir.kind() {
        HirKind::Literal(lit) => {
            let bytes = &lit.0;
            let mut len = 0;
            while len < bytes.len()
                && len < suffix_cursor
                && bytes[bytes.len() - len - 1] == suffix[suffix_cursor - len - 1]
            {
                len += 1;
            }
            if len == 0 || (len < bytes.len() && len < suffix_cursor) {
                return None;
            }
            Some((
                Hir::literal(bytes[..bytes.len() - len].to_vec()),
                suffix_cursor - len,
            ))
        }
        HirKind::Capture(capture) => strip_literal_suffix_at(&capture.sub, suffix, suffix_cursor),
        HirKind::Concat(hirs) => {
            let mut prefix = hirs.to_vec();
            let mut cursor = suffix_cursor;
            for i in (0..prefix.len()).rev() {
                let (stripped, after) = strip_literal_suffix_at(&prefix[i], suffix, cursor)?;
                if after == cursor {
                    return None;
                }
                prefix[i] = stripped;
                cursor = after;
                if cursor == 0 {
                    break;
                }
            }
            Some((Hir::concat(prefix), cursor))
        }
        HirKind::Repetition(rep) => strip_repetition_literal_suffix(rep, suffix, suffix_cursor),
        _ => None,
    }
}
fn strip_repetition_literal_suffix(
    rep: &Repetition,
    suffix: &[u8],
    suffix_cursor: usize,
) -> Option<(Hir, usize)> {
    let mut min = rep.min;
    let mut max = rep.max;
    let mut cursor = suffix_cursor;
    let mut tail = Vec::new();
    while cursor > 0 {
        if min == 0 {
            return None;
        }
        let before = cursor;
        let (stripped, after) = strip_literal_suffix_at(&rep.sub, suffix, cursor)?;
        if after == before {
            return None;
        }
        min = min.saturating_sub(1);
        max = max.map(|max| max.saturating_sub(1));
        if !matches!(stripped.kind(), HirKind::Empty) {
            tail.push(stripped);
        }
        cursor = after;
    }
    let rest = Hir::repetition(Repetition {
        min,
        max,
        greedy: rep.greedy,
        sub: rep.sub.clone(),
    });
    let mut prefix = Vec::with_capacity(tail.len() + 1);
    if !matches!(rest.kind(), HirKind::Empty) {
        prefix.push(rest);
    }
    tail.reverse();
    prefix.extend(tail);
    Some((Hir::concat(prefix), 0))
}
