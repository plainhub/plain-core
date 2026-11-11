use crate::regex_engine::automata::{
    MatchError,
    dfa::{
        accel,
        automaton::{Automaton, OverlappingState},
    },
    util::{
        prefilter::Prefilter,
        primitives::StateID,
        search::{Anchored, HalfMatch, Input, Span},
    },
};
#[inline(never)]
pub fn find_fwd<A: Automaton + ?Sized>(
    dfa: &A,
    input: &Input<'_>,
) -> Result<Option<HalfMatch>, MatchError> {
    if input.is_done() {
        return Ok(None);
    }
    let pre = if input.get_anchored().is_anchored() {
        None
    } else {
        dfa.get_prefilter()
    };
    if pre.is_some() {
        if input.get_earliest() {
            find_fwd_imp(dfa, input, pre, true)
        } else {
            find_fwd_imp(dfa, input, pre, false)
        }
    } else {
        if input.get_earliest() {
            find_fwd_imp(dfa, input, None, true)
        } else {
            find_fwd_imp(dfa, input, None, false)
        }
    }
}
#[cfg_attr(all(), inline(always))]
fn find_fwd_imp<A: Automaton + ?Sized>(
    dfa: &A,
    input: &Input<'_>,
    pre: Option<&'_ Prefilter>,
    earliest: bool,
) -> Result<Option<HalfMatch>, MatchError> {
    let universal_start = dfa.universal_start_state(Anchored::No).is_some();
    let mut mat = None;
    let mut sid = init_fwd(dfa, input)?;
    let mut at = input.start();
    macro_rules! next_unchecked {
        ($ sid : expr , $ at : expr) => {{
            let byte = *input.haystack().get_unchecked($at);
            dfa.next_state_unchecked($sid, byte)
        }};
    }
    if let Some(ref pre) = pre {
        let span = Span::from(at..input.end());
        match pre.find(input.haystack(), span) {
            None => return Ok(mat),
            Some(ref span) => {
                at = span.start;
                if !universal_start {
                    sid = prefilter_restart(dfa, &input, at)?;
                }
            }
        }
    }
    while at < input.end() {
        let mut prev_sid;
        while at < input.end() {
            prev_sid = unsafe { next_unchecked!(sid, at) };
            if dfa.is_special_state(prev_sid) || at + 3 >= input.end() {
                core::mem::swap(&mut prev_sid, &mut sid);
                break;
            }
            at += 1;
            sid = unsafe { next_unchecked!(prev_sid, at) };
            if dfa.is_special_state(sid) {
                break;
            }
            at += 1;
            prev_sid = unsafe { next_unchecked!(sid, at) };
            if dfa.is_special_state(prev_sid) {
                core::mem::swap(&mut prev_sid, &mut sid);
                break;
            }
            at += 1;
            sid = unsafe { next_unchecked!(prev_sid, at) };
            if dfa.is_special_state(sid) {
                break;
            }
            at += 1;
        }
        if dfa.is_special_state(sid) {
            if dfa.is_start_state(sid) {
                if let Some(ref pre) = pre {
                    if mat.is_none() {
                        let span = Span::from(at..input.end());
                        match pre.find(input.haystack(), span) {
                            None => return Ok(mat),
                            Some(ref span) => {
                                if span.start > at {
                                    at = span.start;
                                    if !universal_start {
                                        sid = prefilter_restart(dfa, &input, at)?;
                                    }
                                    continue;
                                }
                            }
                        }
                    }
                } else if dfa.is_accel_state(sid) {
                    let needles = dfa.accelerator(sid);
                    at = accel::find_fwd(needles, input.haystack(), at + 1).unwrap_or(input.end());
                    continue;
                }
            } else if dfa.is_match_state(sid) {
                let pattern = dfa.match_pattern(sid, 0);
                mat = Some(HalfMatch::new(pattern, at));
                if earliest {
                    return Ok(mat);
                }
                if dfa.is_accel_state(sid) {
                    let needles = dfa.accelerator(sid);
                    at = accel::find_fwd(needles, input.haystack(), at + 1).unwrap_or(input.end());
                    continue;
                }
            } else if dfa.is_accel_state(sid) {
                let needs = dfa.accelerator(sid);
                at = accel::find_fwd(needs, input.haystack(), at + 1).unwrap_or(input.end());
                continue;
            } else if dfa.is_dead_state(sid) {
                return Ok(mat);
            } else {
                return Err(MatchError::quit(input.haystack()[at], at));
            }
        }
        at += 1;
    }
    eoi_fwd(dfa, input, &mut sid, &mut mat)?;
    Ok(mat)
}
#[inline(never)]
pub fn find_rev<A: Automaton + ?Sized>(
    dfa: &A,
    input: &Input<'_>,
) -> Result<Option<HalfMatch>, MatchError> {
    if input.is_done() {
        return Ok(None);
    }
    if input.get_earliest() {
        find_rev_imp(dfa, input, true)
    } else {
        find_rev_imp(dfa, input, false)
    }
}
#[cfg_attr(all(), inline(always))]
fn find_rev_imp<A: Automaton + ?Sized>(
    dfa: &A,
    input: &Input<'_>,
    earliest: bool,
) -> Result<Option<HalfMatch>, MatchError> {
    let mut mat = None;
    let mut sid = init_rev(dfa, input)?;
    if input.start() == input.end() {
        eoi_rev(dfa, input, &mut sid, &mut mat)?;
        return Ok(mat);
    }
    let mut at = input.end() - 1;
    macro_rules! next_unchecked {
        ($ sid : expr , $ at : expr) => {{
            let byte = *input.haystack().get_unchecked($at);
            dfa.next_state_unchecked($sid, byte)
        }};
    }
    loop {
        let mut prev_sid;
        while at >= input.start() {
            prev_sid = unsafe { next_unchecked!(sid, at) };
            if dfa.is_special_state(prev_sid) || at <= input.start().saturating_add(3) {
                core::mem::swap(&mut prev_sid, &mut sid);
                break;
            }
            at -= 1;
            sid = unsafe { next_unchecked!(prev_sid, at) };
            if dfa.is_special_state(sid) {
                break;
            }
            at -= 1;
            prev_sid = unsafe { next_unchecked!(sid, at) };
            if dfa.is_special_state(prev_sid) {
                core::mem::swap(&mut prev_sid, &mut sid);
                break;
            }
            at -= 1;
            sid = unsafe { next_unchecked!(prev_sid, at) };
            if dfa.is_special_state(sid) {
                break;
            }
            at -= 1;
        }
        if dfa.is_special_state(sid) {
            if dfa.is_start_state(sid) {
                if dfa.is_accel_state(sid) {
                    let needles = dfa.accelerator(sid);
                    at = accel::find_rev(needles, input.haystack(), at)
                        .map(|i| i + 1)
                        .unwrap_or(input.start());
                }
            } else if dfa.is_match_state(sid) {
                let pattern = dfa.match_pattern(sid, 0);
                mat = Some(HalfMatch::new(pattern, at + 1));
                if earliest {
                    return Ok(mat);
                }
                if dfa.is_accel_state(sid) {
                    let needles = dfa.accelerator(sid);
                    at = accel::find_rev(needles, input.haystack(), at)
                        .map(|i| i + 1)
                        .unwrap_or(input.start());
                }
            } else if dfa.is_accel_state(sid) {
                let needles = dfa.accelerator(sid);
                at = accel::find_rev(needles, input.haystack(), at)
                    .map(|i| i + 1)
                    .unwrap_or(input.start());
            } else if dfa.is_dead_state(sid) {
                return Ok(mat);
            } else {
                return Err(MatchError::quit(input.haystack()[at], at));
            }
        }
        if at == input.start() {
            break;
        }
        at -= 1;
    }
    eoi_rev(dfa, input, &mut sid, &mut mat)?;
    Ok(mat)
}
#[inline(never)]
pub fn find_overlapping_fwd<A: Automaton + ?Sized>(
    dfa: &A,
    input: &Input<'_>,
    state: &mut OverlappingState,
) -> Result<(), MatchError> {
    state.mat = None;
    if input.is_done() {
        return Ok(());
    }
    let pre = if input.get_anchored().is_anchored() {
        None
    } else {
        dfa.get_prefilter()
    };
    if pre.is_some() {
        find_overlapping_fwd_imp(dfa, input, pre, state)
    } else {
        find_overlapping_fwd_imp(dfa, input, None, state)
    }
}
#[cfg_attr(all(), inline(always))]
fn find_overlapping_fwd_imp<A: Automaton + ?Sized>(
    dfa: &A,
    input: &Input<'_>,
    pre: Option<&'_ Prefilter>,
    state: &mut OverlappingState,
) -> Result<(), MatchError> {
    let universal_start = dfa.universal_start_state(Anchored::No).is_some();
    let mut sid = match state.id {
        None => {
            state.at = input.start();
            init_fwd(dfa, input)?
        }
        Some(sid) => {
            if let Some(match_index) = state.next_match_index {
                let match_len = dfa.match_len(sid);
                if match_index < match_len {
                    state.next_match_index = Some(match_index + 1);
                    let pattern = dfa.match_pattern(sid, match_index);
                    state.mat = Some(HalfMatch::new(pattern, state.at));
                    return Ok(());
                }
            }
            state.at += 1;
            if state.at > input.end() {
                return Ok(());
            }
            sid
        }
    };
    while state.at < input.end() {
        sid = dfa.next_state(sid, input.haystack()[state.at]);
        if dfa.is_special_state(sid) {
            state.id = Some(sid);
            if dfa.is_start_state(sid) {
                if let Some(ref pre) = pre {
                    let span = Span::from(state.at..input.end());
                    match pre.find(input.haystack(), span) {
                        None => return Ok(()),
                        Some(ref span) => {
                            if span.start > state.at {
                                state.at = span.start;
                                if !universal_start {
                                    sid = prefilter_restart(dfa, &input, state.at)?;
                                }
                                continue;
                            }
                        }
                    }
                } else if dfa.is_accel_state(sid) {
                    let needles = dfa.accelerator(sid);
                    state.at = accel::find_fwd(needles, input.haystack(), state.at + 1)
                        .unwrap_or(input.end());
                    continue;
                }
            } else if dfa.is_match_state(sid) {
                state.next_match_index = Some(1);
                let pattern = dfa.match_pattern(sid, 0);
                state.mat = Some(HalfMatch::new(pattern, state.at));
                return Ok(());
            } else if dfa.is_accel_state(sid) {
                let needs = dfa.accelerator(sid);
                state.at =
                    accel::find_fwd(needs, input.haystack(), state.at + 1).unwrap_or(input.end());
                continue;
            } else if dfa.is_dead_state(sid) {
                return Ok(());
            } else {
                return Err(MatchError::quit(input.haystack()[state.at], state.at));
            }
        }
        state.at += 1;
    }
    let result = eoi_fwd(dfa, input, &mut sid, &mut state.mat);
    state.id = Some(sid);
    if state.mat.is_some() {
        state.next_match_index = Some(1);
    }
    result
}
#[inline(never)]
pub(crate) fn find_overlapping_rev<A: Automaton + ?Sized>(
    dfa: &A,
    input: &Input<'_>,
    state: &mut OverlappingState,
) -> Result<(), MatchError> {
    state.mat = None;
    if input.is_done() {
        return Ok(());
    }
    let mut sid = match state.id {
        None => {
            let sid = init_rev(dfa, input)?;
            state.id = Some(sid);
            if input.start() == input.end() {
                state.rev_eoi = true;
            } else {
                state.at = input.end() - 1;
            }
            sid
        }
        Some(sid) => {
            if let Some(match_index) = state.next_match_index {
                let match_len = dfa.match_len(sid);
                if match_index < match_len {
                    state.next_match_index = Some(match_index + 1);
                    let pattern = dfa.match_pattern(sid, match_index);
                    state.mat = Some(HalfMatch::new(pattern, state.at));
                    return Ok(());
                }
            }
            if state.rev_eoi {
                return Ok(());
            } else if state.at == input.start() {
                state.rev_eoi = true;
            } else {
                state.at -= 1;
            }
            sid
        }
    };
    while !state.rev_eoi {
        sid = dfa.next_state(sid, input.haystack()[state.at]);
        if dfa.is_special_state(sid) {
            state.id = Some(sid);
            if dfa.is_start_state(sid) {
                if dfa.is_accel_state(sid) {
                    let needles = dfa.accelerator(sid);
                    state.at = accel::find_rev(needles, input.haystack(), state.at)
                        .map(|i| i + 1)
                        .unwrap_or(input.start());
                }
            } else if dfa.is_match_state(sid) {
                state.next_match_index = Some(1);
                let pattern = dfa.match_pattern(sid, 0);
                state.mat = Some(HalfMatch::new(pattern, state.at + 1));
                return Ok(());
            } else if dfa.is_accel_state(sid) {
                let needles = dfa.accelerator(sid);
                state.at = accel::find_rev(needles, input.haystack(), state.at)
                    .map(|i| i + 1)
                    .unwrap_or(input.start());
            } else if dfa.is_dead_state(sid) {
                return Ok(());
            } else {
                return Err(MatchError::quit(input.haystack()[state.at], state.at));
            }
        }
        if state.at == input.start() {
            break;
        }
        state.at -= 1;
    }
    let result = eoi_rev(dfa, input, &mut sid, &mut state.mat);
    state.rev_eoi = true;
    state.id = Some(sid);
    if state.mat.is_some() {
        state.next_match_index = Some(1);
    }
    result
}
#[cfg_attr(all(), inline(always))]
fn init_fwd<A: Automaton + ?Sized>(dfa: &A, input: &Input<'_>) -> Result<StateID, MatchError> {
    let sid = dfa.start_state_forward(input)?;
    debug_assert!(!dfa.is_match_state(sid));
    Ok(sid)
}
#[cfg_attr(all(), inline(always))]
fn init_rev<A: Automaton + ?Sized>(dfa: &A, input: &Input<'_>) -> Result<StateID, MatchError> {
    let sid = dfa.start_state_reverse(input)?;
    debug_assert!(!dfa.is_match_state(sid));
    Ok(sid)
}
#[cfg_attr(all(), inline(always))]
fn eoi_fwd<A: Automaton + ?Sized>(
    dfa: &A,
    input: &Input<'_>,
    sid: &mut StateID,
    mat: &mut Option<HalfMatch>,
) -> Result<(), MatchError> {
    let sp = input.get_span();
    match input.haystack().get(sp.end) {
        Some(&b) => {
            *sid = dfa.next_state(*sid, b);
            if dfa.is_match_state(*sid) {
                let pattern = dfa.match_pattern(*sid, 0);
                *mat = Some(HalfMatch::new(pattern, sp.end));
            } else if dfa.is_quit_state(*sid) {
                return Err(MatchError::quit(b, sp.end));
            }
        }
        None => {
            *sid = dfa.next_eoi_state(*sid);
            if dfa.is_match_state(*sid) {
                let pattern = dfa.match_pattern(*sid, 0);
                *mat = Some(HalfMatch::new(pattern, input.haystack().len()));
            }
        }
    }
    Ok(())
}
#[cfg_attr(all(), inline(always))]
fn eoi_rev<A: Automaton + ?Sized>(
    dfa: &A,
    input: &Input<'_>,
    sid: &mut StateID,
    mat: &mut Option<HalfMatch>,
) -> Result<(), MatchError> {
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
    }
    Ok(())
}
#[cfg_attr(all(), inline(always))]
fn prefilter_restart<A: Automaton + ?Sized>(
    dfa: &A,
    input: &Input<'_>,
    at: usize,
) -> Result<StateID, MatchError> {
    let mut input = input.clone();
    input.set_start(at);
    init_fwd(dfa, &input)
}
