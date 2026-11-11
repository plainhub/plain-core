use crate::regex_engine::syntax::hir::{Class, Hir, HirKind, literal::Literal};
pub(super) fn hir_can_contain_literal(hir: &Hir, lit: &[u8]) -> bool {
    lit.is_empty() || lit.iter().all(|&byte| hir_can_consume_byte(hir, byte))
}
pub(super) fn hir_has_fixed_length(hir: &Hir) -> bool {
    let props = hir.properties();
    props
        .minimum_len()
        .and_then(|min| props.maximum_len().map(|max| min == max))
        .unwrap_or(false)
}
pub(super) fn has_disjoint_class_separator(prefix: &Hir, literals: &[Literal]) -> bool {
    let hirs = match uncapture(prefix).kind() {
        HirKind::Concat(hirs) if hirs.len() >= 2 => hirs,
        _ => return false,
    };
    let Some(first) = hirs.iter().position(|hir| !hir_matches_empty_only(hir)) else {
        return false;
    };
    let last = hirs
        .iter()
        .rposition(|hir| !hir_matches_empty_only(hir))
        .unwrap();
    has_disjoint_class_separator_at(hirs, last, literals)
        || (first != last && has_disjoint_class_separator_at(hirs, first, literals))
}
fn has_disjoint_class_separator_at(hirs: &[Hir], separator: usize, literals: &[Literal]) -> bool {
    let Some(separator_class) = required_class(&hirs[separator]) else {
        return false;
    };
    class_is_disjoint_from_literals(separator_class, literals)
        && hirs
            .iter()
            .enumerate()
            .all(|(i, hir)| i == separator || hir_is_disjoint_from_class(hir, separator_class))
}
fn hir_matches_empty_only(hir: &Hir) -> bool {
    hir.properties().maximum_len() == Some(0)
}
fn required_class(hir: &Hir) -> Option<&Class> {
    let hir = uncapture(hir);
    match hir.kind() {
        HirKind::Class(cls) => Some(cls),
        HirKind::Repetition(rep) if rep.min > 0 => match uncapture(&rep.sub).kind() {
            HirKind::Class(cls) => Some(cls),
            _ => None,
        },
        _ => None,
    }
}
fn hir_is_disjoint_from_class(hir: &Hir, separator: &Class) -> bool {
    match hir.kind() {
        HirKind::Empty | HirKind::Look(_) => true,
        HirKind::Literal(lit) => class_is_disjoint_from_literal(separator, &lit.0),
        HirKind::Class(cls) => classes_are_disjoint(cls, separator),
        HirKind::Repetition(rep) => hir_is_disjoint_from_class(&rep.sub, separator),
        HirKind::Capture(capture) => hir_is_disjoint_from_class(&capture.sub, separator),
        HirKind::Concat(hirs) | HirKind::Alternation(hirs) => hirs
            .iter()
            .all(|hir| hir_is_disjoint_from_class(hir, separator)),
    }
}
fn uncapture(mut hir: &Hir) -> &Hir {
    while let HirKind::Capture(capture) = hir.kind() {
        hir = &capture.sub;
    }
    hir
}
fn classes_are_disjoint(left: &Class, right: &Class) -> bool {
    match (left, right) {
        (Class::Bytes(left), Class::Bytes(right)) => {
            let (mut intersection, other) = if left.ranges().len() <= right.ranges().len() {
                (left.clone(), right)
            } else {
                (right.clone(), left)
            };
            intersection.intersect(other);
            intersection.ranges().is_empty()
        }
        (Class::Unicode(left), Class::Unicode(right)) => {
            let (mut intersection, other) = if left.ranges().len() <= right.ranges().len() {
                (left.clone(), right)
            } else {
                (right.clone(), left)
            };
            intersection.intersect(other);
            intersection.ranges().is_empty()
        }
        _ => false,
    }
}
fn class_is_disjoint_from_literals(cls: &Class, literals: &[Literal]) -> bool {
    literals
        .iter()
        .all(|lit| class_is_disjoint_from_literal(cls, lit.as_bytes()))
}
fn class_is_disjoint_from_literal(cls: &Class, lit: &[u8]) -> bool {
    match cls {
        Class::Bytes(cls) => lit.iter().all(|&byte| !byte_class_contains(cls, byte)),
        Class::Unicode(cls) => core::str::from_utf8(lit).map_or(false, |lit| {
            lit.chars().all(|ch| !unicode_class_contains(cls, ch))
        }),
    }
}
fn hir_can_consume_byte(hir: &Hir, byte: u8) -> bool {
    match hir.kind() {
        HirKind::Empty | HirKind::Look(_) => false,
        HirKind::Literal(lit) => lit.0.contains(&byte),
        HirKind::Class(Class::Bytes(cls)) => byte_class_contains(cls, byte),
        HirKind::Class(Class::Unicode(cls)) => {
            if byte > 0x7F {
                return true;
            }
            let ch = char::from(byte);
            unicode_class_contains(cls, ch)
        }
        HirKind::Repetition(rep) => hir_can_consume_byte(&rep.sub, byte),
        HirKind::Capture(capture) => hir_can_consume_byte(&capture.sub, byte),
        HirKind::Concat(hirs) | HirKind::Alternation(hirs) => {
            hirs.iter().any(|hir| hir_can_consume_byte(hir, byte))
        }
    }
}
fn byte_class_contains(cls: &crate::regex_engine::syntax::hir::ClassBytes, byte: u8) -> bool {
    cls.ranges()
        .iter()
        .any(|range| range.start() <= byte && byte <= range.end())
}
fn unicode_class_contains(cls: &crate::regex_engine::syntax::hir::ClassUnicode, ch: char) -> bool {
    cls.ranges()
        .iter()
        .any(|range| range.start() <= ch && ch <= range.end())
}
