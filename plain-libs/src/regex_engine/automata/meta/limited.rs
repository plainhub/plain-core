use crate::regex_engine::automata::{
    HalfMatch, Input, MatchError,
    meta::error::{RetryError, RetryQuadraticError},
};
pub(crate) fn dfa_try_search_half_rev(
    dfa: &crate::regex_engine::automata::dfa::dense::DFA<std::vec::Vec<u32>>,
    input: &Input<'_>,
    min_start: usize,
) -> Result<Option<HalfMatch>, RetryError> {
    use crate::regex_engine::automata::dfa::Automaton;
    let mut mat = None;
    let mut sid = dfa.start_state_reverse(input)?;
    if input.start() == input.end() {
        dfa_eoi_rev(dfa, input, &mut sid, &mut mat)?;
        return Ok(mat);
    }
    let mut at = input.end() - 1;
    loop {
        sid = dfa.next_state(sid, input.haystack()[at]);
        if dfa.is_special_state(sid) {
            if dfa.is_match_state(sid) {
                let pattern = dfa.match_pattern(sid, 0);
                mat = Some(HalfMatch::new(pattern, at + 1));
            } else if dfa.is_dead_state(sid) {
                return Ok(mat);
            } else if dfa.is_quit_state(sid) {
                return Err(MatchError::quit(input.haystack()[at], at).into());
            }
        }
        if at == input.start() {
            break;
        }
        at -= 1;
        if at < min_start {
            trace!(
                "reached position {at} which is before the previous literal \
                 match, quitting to avoid quadratic behavior",
            );
            return Err(RetryError::Quadratic(RetryQuadraticError::new()));
        }
    }
    let was_dead = dfa.is_dead_state(sid);
    dfa_eoi_rev(dfa, input, &mut sid, &mut mat)?;
    if at == input.start() && mat.map_or(false, |m| m.offset() > input.start()) && !was_dead {
        trace!(
            "reached beginning of search at offset {at} without hitting \
             a dead state, quitting to avoid potential false positive match",
        );
        return Err(RetryError::Quadratic(RetryQuadraticError::new()));
    }
    Ok(mat)
}
pub(crate) fn hybrid_try_search_half_rev(
    dfa: &crate::regex_engine::automata::hybrid::dfa::DFA,
    cache: &mut crate::regex_engine::automata::hybrid::dfa::Cache,
    input: &Input<'_>,
    min_start: usize,
) -> Result<Option<HalfMatch>, RetryError> {
    let mut mat = None;
    let mut sid = dfa.start_state_reverse(cache, input)?;
    if input.start() == input.end() {
        hybrid_eoi_rev(dfa, cache, input, &mut sid, &mut mat)?;
        return Ok(mat);
    }
    let mut at = input.end() - 1;
    loop {
        sid = dfa
            .next_state(cache, sid, input.haystack()[at])
            .map_err(|_| MatchError::gave_up(at))?;
        if sid.is_tagged() {
            if sid.is_match() {
                let pattern = dfa.match_pattern(cache, sid, 0);
                mat = Some(HalfMatch::new(pattern, at + 1));
            } else if sid.is_dead() {
                return Ok(mat);
            } else if sid.is_quit() {
                return Err(MatchError::quit(input.haystack()[at], at).into());
            }
        }
        if at == input.start() {
            break;
        }
        at -= 1;
        if at < min_start {
            trace!(
                "reached position {at} which is before the previous literal \
                 match, quitting to avoid quadratic behavior",
            );
            return Err(RetryError::Quadratic(RetryQuadraticError::new()));
        }
    }
    let was_dead = sid.is_dead();
    hybrid_eoi_rev(dfa, cache, input, &mut sid, &mut mat)?;
    if at == input.start() && mat.map_or(false, |m| m.offset() > input.start()) && !was_dead {
        trace!(
            "reached beginning of search at offset {at} without hitting \
             a dead state, quitting to avoid potential false positive match",
        );
        return Err(RetryError::Quadratic(RetryQuadraticError::new()));
    }
    Ok(mat)
}
#[cfg_attr(all(), inline(always))]
fn dfa_eoi_rev(
    dfa: &crate::regex_engine::automata::dfa::dense::DFA<std::vec::Vec<u32>>,
    input: &Input<'_>,
    sid: &mut crate::regex_engine::automata::util::primitives::StateID,
    mat: &mut Option<HalfMatch>,
) -> Result<(), MatchError> {
    use crate::regex_engine::automata::dfa::Automaton;
    let sp = input.get_span();
    if sp.start > 0 {
        let byte = input.haystack()[sp.start - 1];
        *sid = dfa.next_state(*sid, byte);
        if dfa.is_match_state(*sid) {
            let pattern = dfa.match_pattern(*sid, 0);
            *mat = Some(HalfMatch::new(pattern, sp.start));
        } else if dfa.is_quit_state(*sid) {
            return Err(MatchError::quit(byte, sp.start - 1));
        }
    } else {
        *sid = dfa.next_eoi_state(*sid);
        if dfa.is_match_state(*sid) {
            let pattern = dfa.match_pattern(*sid, 0);
            *mat = Some(HalfMatch::new(pattern, 0));
        }
        debug_assert!(!dfa.is_quit_state(*sid));
    }
    Ok(())
}
#[cfg_attr(all(), inline(always))]
fn hybrid_eoi_rev(
    dfa: &crate::regex_engine::automata::hybrid::dfa::DFA,
    cache: &mut crate::regex_engine::automata::hybrid::dfa::Cache,
    input: &Input<'_>,
    sid: &mut crate::regex_engine::automata::hybrid::LazyStateID,
    mat: &mut Option<HalfMatch>,
) -> Result<(), MatchError> {
    let sp = input.get_span();
    if sp.start > 0 {
        let byte = input.haystack()[sp.start - 1];
        *sid = dfa
            .next_state(cache, *sid, byte)
            .map_err(|_| MatchError::gave_up(sp.start))?;
        if sid.is_match() {
            let pattern = dfa.match_pattern(cache, *sid, 0);
            *mat = Some(HalfMatch::new(pattern, sp.start));
        } else if sid.is_quit() {
            return Err(MatchError::quit(byte, sp.start - 1));
        }
    } else {
        *sid = dfa
            .next_eoi_state(cache, *sid)
            .map_err(|_| MatchError::gave_up(sp.start))?;
        if sid.is_match() {
            let pattern = dfa.match_pattern(cache, *sid, 0);
            *mat = Some(HalfMatch::new(pattern, 0));
        }
        debug_assert!(!sid.is_quit());
    }
    Ok(())
}
