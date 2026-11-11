pub(crate) use self::state::{State, StateBuilderEmpty, StateBuilderMatches, StateBuilderNFA};
use crate::regex_engine::automata::{
    nfa::thompson,
    util::{
        alphabet,
        look::{Look, LookSet},
        primitives::StateID,
        search::MatchKind,
        sparse_set::{SparseSet, SparseSets},
        start::Start,
        utf8,
    },
};
use std::vec::Vec;
mod state;
pub(crate) fn next(
    nfa: &thompson::NFA,
    match_kind: MatchKind,
    sparses: &mut SparseSets,
    stack: &mut Vec<StateID>,
    state: &State,
    unit: alphabet::Unit,
    empty_builder: StateBuilderEmpty,
) -> StateBuilderNFA {
    sparses.clear();
    let rev = nfa.is_reverse();
    let lookm = nfa.look_matcher();
    state.iter_nfa_state_ids(|nfa_id| {
        sparses.set1.insert(nfa_id);
    });
    if !state.look_need().is_empty() {
        let mut look_have = state.look_have();
        match unit.as_u8() {
            Some(b'\r') => {
                if !rev || !state.is_half_crlf() {
                    look_have = look_have.insert(Look::EndCRLF);
                }
            }
            Some(b'\n') => {
                if rev || !state.is_half_crlf() {
                    look_have = look_have.insert(Look::EndCRLF);
                }
            }
            Some(_) => {}
            None => {
                look_have = look_have
                    .insert(Look::End)
                    .insert(Look::EndLF)
                    .insert(Look::EndCRLF);
            }
        }
        if unit.is_byte(lookm.get_line_terminator()) {
            look_have = look_have.insert(Look::EndLF);
        }
        if state.is_half_crlf() && ((rev && !unit.is_byte(b'\r')) || (!rev && !unit.is_byte(b'\n')))
        {
            look_have = look_have.insert(Look::StartCRLF);
        }
        if state.is_from_word() == unit.is_word_byte() {
            look_have = look_have
                .insert(Look::WordAsciiNegate)
                .insert(Look::WordUnicodeNegate);
        } else {
            look_have = look_have.insert(Look::WordAscii).insert(Look::WordUnicode);
        }
        if !unit.is_word_byte() {
            look_have = look_have
                .insert(Look::WordEndHalfAscii)
                .insert(Look::WordEndHalfUnicode);
        }
        if state.is_from_word() && !unit.is_word_byte() {
            look_have = look_have
                .insert(Look::WordEndAscii)
                .insert(Look::WordEndUnicode);
        } else if !state.is_from_word() && unit.is_word_byte() {
            look_have = look_have
                .insert(Look::WordStartAscii)
                .insert(Look::WordStartUnicode);
        }
        if !look_have
            .subtract(state.look_have())
            .intersect(state.look_need())
            .is_empty()
        {
            for nfa_id in sparses.set1.iter() {
                epsilon_closure(nfa, nfa_id, look_have, stack, &mut sparses.set2);
            }
            sparses.swap();
            sparses.set2.clear();
        }
    }
    let mut builder = empty_builder.into_matches();
    if nfa.look_set_any().contains_anchor_line() && unit.is_byte(lookm.get_line_terminator()) {
        builder.set_look_have(|have| have.insert(Look::StartLF));
    }
    if nfa.look_set_any().contains_anchor_crlf()
        && ((rev && unit.is_byte(b'\r')) || (!rev && unit.is_byte(b'\n')))
    {
        builder.set_look_have(|have| have.insert(Look::StartCRLF));
    }
    if nfa.look_set_any().contains_word() && !unit.is_word_byte() {
        builder.set_look_have(|have| {
            have.insert(Look::WordStartHalfAscii)
                .insert(Look::WordStartHalfUnicode)
        });
    }
    for nfa_id in sparses.set1.iter() {
        match *nfa.state(nfa_id) {
            thompson::State::Union { .. }
            | thompson::State::BinaryUnion { .. }
            | thompson::State::Fail
            | thompson::State::Look { .. }
            | thompson::State::Capture { .. } => {}
            thompson::State::Match { pattern_id } => {
                builder.add_match_pattern_id(pattern_id);
                if !match_kind.continue_past_first_match() {
                    break;
                }
            }
            thompson::State::ByteRange { ref trans } => {
                if trans.matches_unit(unit) {
                    epsilon_closure(
                        nfa,
                        trans.next,
                        builder.look_have(),
                        stack,
                        &mut sparses.set2,
                    );
                }
            }
            thompson::State::Sparse(ref sparse) => {
                if let Some(next) = sparse.matches_unit(unit) {
                    epsilon_closure(nfa, next, builder.look_have(), stack, &mut sparses.set2);
                }
            }
            thompson::State::Dense(ref dense) => {
                if let Some(next) = dense.matches_unit(unit) {
                    epsilon_closure(nfa, next, builder.look_have(), stack, &mut sparses.set2);
                }
            }
        }
    }
    if !sparses.set2.is_empty() {
        if nfa.look_set_any().contains_word() && unit.is_word_byte() {
            builder.set_is_from_word();
        }
        if nfa.look_set_any().contains_anchor_crlf()
            && ((rev && unit.is_byte(b'\n')) || (!rev && unit.is_byte(b'\r')))
        {
            builder.set_is_half_crlf();
        }
    }
    let mut builder_nfa = builder.into_nfa();
    add_nfa_states(nfa, &sparses.set2, &mut builder_nfa);
    builder_nfa
}
pub(crate) fn epsilon_closure(
    nfa: &thompson::NFA,
    start_nfa_id: StateID,
    look_have: LookSet,
    stack: &mut Vec<StateID>,
    set: &mut SparseSet,
) {
    assert!(stack.is_empty());
    if !nfa.state(start_nfa_id).is_epsilon() {
        set.insert(start_nfa_id);
        return;
    }
    stack.push(start_nfa_id);
    while let Some(mut id) = stack.pop() {
        loop {
            if !set.insert(id) {
                break;
            }
            match *nfa.state(id) {
                thompson::State::ByteRange { .. }
                | thompson::State::Sparse { .. }
                | thompson::State::Dense { .. }
                | thompson::State::Fail
                | thompson::State::Match { .. } => break,
                thompson::State::Look { look, next } => {
                    if !look_have.contains(look) {
                        break;
                    }
                    id = next;
                }
                thompson::State::Union { ref alternates } => {
                    id = match alternates.get(0) {
                        None => break,
                        Some(&id) => id,
                    };
                    stack.extend(alternates[1..].iter().rev());
                }
                thompson::State::BinaryUnion { alt1, alt2 } => {
                    id = alt1;
                    stack.push(alt2);
                }
                thompson::State::Capture { next, .. } => {
                    id = next;
                }
            }
        }
    }
}
pub(crate) fn add_nfa_states(nfa: &thompson::NFA, set: &SparseSet, builder: &mut StateBuilderNFA) {
    for nfa_id in set.iter() {
        match *nfa.state(nfa_id) {
            thompson::State::ByteRange { .. } => {
                builder.add_nfa_state_id(nfa_id);
            }
            thompson::State::Sparse { .. } => {
                builder.add_nfa_state_id(nfa_id);
            }
            thompson::State::Dense { .. } => {
                builder.add_nfa_state_id(nfa_id);
            }
            thompson::State::Look { look, .. } => {
                builder.add_nfa_state_id(nfa_id);
                builder.set_look_need(|need| need.insert(look));
            }
            thompson::State::Union { .. } | thompson::State::BinaryUnion { .. } => {
                builder.add_nfa_state_id(nfa_id);
            }
            thompson::State::Capture { .. } => {}
            thompson::State::Fail => {
                builder.add_nfa_state_id(nfa_id);
            }
            thompson::State::Match { .. } => {
                builder.add_nfa_state_id(nfa_id);
            }
        }
    }
    if builder.look_need().is_empty() {
        builder.set_look_have(|_| LookSet::empty());
    }
}
pub(crate) fn set_lookbehind_from_start(
    nfa: &thompson::NFA,
    start: &Start,
    builder: &mut StateBuilderMatches,
) {
    let rev = nfa.is_reverse();
    let lineterm = nfa.look_matcher().get_line_terminator();
    let lookset = nfa.look_set_any();
    match *start {
        Start::NonWordByte => {
            if lookset.contains_word() {
                builder.set_look_have(|have| {
                    have.insert(Look::WordStartHalfAscii)
                        .insert(Look::WordStartHalfUnicode)
                });
            }
        }
        Start::WordByte => {
            if lookset.contains_word() {
                builder.set_is_from_word();
            }
        }
        Start::Text => {
            if lookset.contains_anchor_haystack() {
                builder.set_look_have(|have| have.insert(Look::Start));
            }
            if lookset.contains_anchor_line() {
                builder.set_look_have(|have| have.insert(Look::StartLF).insert(Look::StartCRLF));
            }
            if lookset.contains_word() {
                builder.set_look_have(|have| {
                    have.insert(Look::WordStartHalfAscii)
                        .insert(Look::WordStartHalfUnicode)
                });
            }
        }
        Start::LineLF => {
            if rev {
                if lookset.contains_anchor_crlf() {
                    builder.set_is_half_crlf();
                }
                if lookset.contains_anchor_line() {
                    builder.set_look_have(|have| have.insert(Look::StartLF));
                }
            } else {
                if lookset.contains_anchor_line() {
                    builder.set_look_have(|have| have.insert(Look::StartCRLF));
                }
            }
            if lookset.contains_anchor_line() && lineterm == b'\n' {
                builder.set_look_have(|have| have.insert(Look::StartLF));
            }
            if lookset.contains_word() {
                builder.set_look_have(|have| {
                    have.insert(Look::WordStartHalfAscii)
                        .insert(Look::WordStartHalfUnicode)
                });
            }
        }
        Start::LineCR => {
            if lookset.contains_anchor_crlf() {
                if rev {
                    builder.set_look_have(|have| have.insert(Look::StartCRLF));
                } else {
                    builder.set_is_half_crlf();
                }
            }
            if lookset.contains_anchor_line() && lineterm == b'\r' {
                builder.set_look_have(|have| have.insert(Look::StartLF));
            }
            if lookset.contains_word() {
                builder.set_look_have(|have| {
                    have.insert(Look::WordStartHalfAscii)
                        .insert(Look::WordStartHalfUnicode)
                });
            }
        }
        Start::CustomLineTerminator => {
            if lookset.contains_anchor_line() {
                builder.set_look_have(|have| have.insert(Look::StartLF));
            }
            if lookset.contains_word() {
                if utf8::is_word_byte(lineterm) {
                    builder.set_is_from_word();
                } else {
                    builder.set_look_have(|have| {
                        have.insert(Look::WordStartHalfAscii)
                            .insert(Look::WordStartHalfUnicode)
                    });
                }
            }
        }
    }
}
