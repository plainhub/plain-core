use crate::regex_engine::automata::{
    hybrid::{
        dfa::{Cache, DFA, OverlappingState},
        id::LazyStateID,
    },
    util::{
        prefilter::Prefilter,
        search::{HalfMatch, Input, MatchError, Span},
    },
};
#[inline(never)]
pub(crate) fn find_fwd(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
) -> Result<Option<HalfMatch>, MatchError> {
    if input.is_done() {
        return Ok(None);
    }
    let pre = if input.get_anchored().is_anchored() {
        None
    } else {
        dfa.get_config().get_prefilter()
    };
    if pre.is_some() {
        if input.get_earliest() {
            find_fwd_imp(dfa, cache, input, pre, true)
        } else {
            find_fwd_imp(dfa, cache, input, pre, false)
        }
    } else {
        if input.get_earliest() {
            find_fwd_imp(dfa, cache, input, None, true)
        } else {
            find_fwd_imp(dfa, cache, input, None, false)
        }
    }
}
#[cfg_attr(all(), inline(always))]
fn find_fwd_imp(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
    pre: Option<&'_ Prefilter>,
    earliest: bool,
) -> Result<Option<HalfMatch>, MatchError> {
    let universal_start = dfa.get_nfa().look_set_prefix_any().is_empty();
    let mut mat = None;
    let mut sid = init_fwd(dfa, cache, input)?;
    let mut at = input.start();
    macro_rules! next_unchecked {
        ($ sid : expr , $ at : expr) => {{
            let byte = *input.haystack().get_unchecked($at);
            dfa.next_state_untagged_unchecked(cache, $sid, byte)
        }};
    }
    if let Some(ref pre) = pre {
        let span = Span::from(at..input.end());
        match pre.find(input.haystack(), span) {
            None => return Ok(mat),
            Some(ref span) => {
                at = span.start;
                if !universal_start {
                    sid = prefilter_restart(dfa, cache, &input, at)?;
                }
            }
        }
    }
    cache.search_start(at);
    while at < input.end() {
        if sid.is_tagged() {
            cache.search_update(at);
            sid = dfa
                .next_state(cache, sid, input.haystack()[at])
                .map_err(|_| gave_up(at))?;
        } else {
            let mut prev_sid = sid;
            while at < input.end() {
                prev_sid = unsafe { next_unchecked!(sid, at) };
                if prev_sid.is_tagged() || at + 3 >= input.end() {
                    core::mem::swap(&mut prev_sid, &mut sid);
                    break;
                }
                at += 1;
                sid = unsafe { next_unchecked!(prev_sid, at) };
                if sid.is_tagged() {
                    break;
                }
                at += 1;
                prev_sid = unsafe { next_unchecked!(sid, at) };
                if prev_sid.is_tagged() {
                    core::mem::swap(&mut prev_sid, &mut sid);
                    break;
                }
                at += 1;
                sid = unsafe { next_unchecked!(prev_sid, at) };
                if sid.is_tagged() {
                    break;
                }
                at += 1;
            }
            if sid.is_unknown() {
                cache.search_update(at);
                sid = dfa
                    .next_state(cache, prev_sid, input.haystack()[at])
                    .map_err(|_| gave_up(at))?;
            }
        }
        if sid.is_tagged() {
            if sid.is_start() {
                if let Some(ref pre) = pre {
                    if mat.is_none() {
                        let span = Span::from(at..input.end());
                        match pre.find(input.haystack(), span) {
                            None => {
                                cache.search_finish(span.end);
                                return Ok(mat);
                            }
                            Some(ref span) => {
                                if span.start > at {
                                    at = span.start;
                                    if !universal_start {
                                        sid = prefilter_restart(dfa, cache, &input, at)?;
                                    }
                                    continue;
                                }
                            }
                        }
                    }
                }
            } else if sid.is_match() {
                let pattern = dfa.match_pattern(cache, sid, 0);
                mat = Some(HalfMatch::new(pattern, at));
                if earliest {
                    cache.search_finish(at);
                    return Ok(mat);
                }
            } else if sid.is_dead() {
                cache.search_finish(at);
                return Ok(mat);
            } else if sid.is_quit() {
                cache.search_finish(at);
                return Err(MatchError::quit(input.haystack()[at], at));
            } else {
                debug_assert!(sid.is_unknown());
                unreachable!("sid being unknown is a bug");
            }
        }
        at += 1;
    }
    eoi_fwd(dfa, cache, input, &mut sid, &mut mat)?;
    cache.search_finish(input.end());
    Ok(mat)
}
#[inline(never)]
pub(crate) fn find_rev(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
) -> Result<Option<HalfMatch>, MatchError> {
    if input.is_done() {
        return Ok(None);
    }
    if input.get_earliest() {
        find_rev_imp(dfa, cache, input, true)
    } else {
        find_rev_imp(dfa, cache, input, false)
    }
}
#[cfg_attr(all(), inline(always))]
fn find_rev_imp(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
    earliest: bool,
) -> Result<Option<HalfMatch>, MatchError> {
    let mut mat = None;
    let mut sid = init_rev(dfa, cache, input)?;
    if input.start() == input.end() {
        eoi_rev(dfa, cache, input, &mut sid, &mut mat)?;
        return Ok(mat);
    }
    let mut at = input.end() - 1;
    macro_rules! next_unchecked {
        ($ sid : expr , $ at : expr) => {{
            let byte = *input.haystack().get_unchecked($at);
            dfa.next_state_untagged_unchecked(cache, $sid, byte)
        }};
    }
    cache.search_start(at);
    loop {
        if sid.is_tagged() {
            cache.search_update(at);
            sid = dfa
                .next_state(cache, sid, input.haystack()[at])
                .map_err(|_| gave_up(at))?;
        } else {
            let mut prev_sid = sid;
            while at >= input.start() {
                prev_sid = unsafe { next_unchecked!(sid, at) };
                if prev_sid.is_tagged() || at <= input.start().saturating_add(3) {
                    core::mem::swap(&mut prev_sid, &mut sid);
                    break;
                }
                at -= 1;
                sid = unsafe { next_unchecked!(prev_sid, at) };
                if sid.is_tagged() {
                    break;
                }
                at -= 1;
                prev_sid = unsafe { next_unchecked!(sid, at) };
                if prev_sid.is_tagged() {
                    core::mem::swap(&mut prev_sid, &mut sid);
                    break;
                }
                at -= 1;
                sid = unsafe { next_unchecked!(prev_sid, at) };
                if sid.is_tagged() {
                    break;
                }
                at -= 1;
            }
            if sid.is_unknown() {
                cache.search_update(at);
                sid = dfa
                    .next_state(cache, prev_sid, input.haystack()[at])
                    .map_err(|_| gave_up(at))?;
            }
        }
        if sid.is_tagged() {
            if sid.is_start() {
            } else if sid.is_match() {
                let pattern = dfa.match_pattern(cache, sid, 0);
                mat = Some(HalfMatch::new(pattern, at + 1));
                if earliest {
                    cache.search_finish(at);
                    return Ok(mat);
                }
            } else if sid.is_dead() {
                cache.search_finish(at);
                return Ok(mat);
            } else if sid.is_quit() {
                cache.search_finish(at);
                return Err(MatchError::quit(input.haystack()[at], at));
            } else {
                debug_assert!(sid.is_unknown());
                unreachable!("sid being unknown is a bug");
            }
        }
        if at == input.start() {
            break;
        }
        at -= 1;
    }
    cache.search_finish(input.start());
    eoi_rev(dfa, cache, input, &mut sid, &mut mat)?;
    Ok(mat)
}
#[inline(never)]
pub(crate) fn find_overlapping_fwd(
    dfa: &DFA,
    cache: &mut Cache,
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
        dfa.get_config().get_prefilter()
    };
    if pre.is_some() {
        find_overlapping_fwd_imp(dfa, cache, input, pre, state)
    } else {
        find_overlapping_fwd_imp(dfa, cache, input, None, state)
    }
}
#[cfg_attr(all(), inline(always))]
fn find_overlapping_fwd_imp(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
    pre: Option<&'_ Prefilter>,
    state: &mut OverlappingState,
) -> Result<(), MatchError> {
    let universal_start = dfa.get_nfa().look_set_prefix_any().is_empty();
    let mut sid = match state.id {
        None => {
            state.at = input.start();
            init_fwd(dfa, cache, input)?
        }
        Some(sid) => {
            if let Some(match_index) = state.next_match_index {
                let match_len = dfa.match_len(cache, sid);
                if match_index < match_len {
                    state.next_match_index = Some(match_index + 1);
                    let pattern = dfa.match_pattern(cache, sid, match_index);
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
    cache.search_start(state.at);
    while state.at < input.end() {
        sid = dfa
            .next_state(cache, sid, input.haystack()[state.at])
            .map_err(|_| gave_up(state.at))?;
        if sid.is_tagged() {
            state.id = Some(sid);
            if sid.is_start() {
                if let Some(ref pre) = pre {
                    let span = Span::from(state.at..input.end());
                    match pre.find(input.haystack(), span) {
                        None => return Ok(()),
                        Some(ref span) => {
                            if span.start > state.at {
                                state.at = span.start;
                                if !universal_start {
                                    sid = prefilter_restart(dfa, cache, &input, state.at)?;
                                }
                                continue;
                            }
                        }
                    }
                }
            } else if sid.is_match() {
                state.next_match_index = Some(1);
                let pattern = dfa.match_pattern(cache, sid, 0);
                state.mat = Some(HalfMatch::new(pattern, state.at));
                cache.search_finish(state.at);
                return Ok(());
            } else if sid.is_dead() {
                cache.search_finish(state.at);
                return Ok(());
            } else if sid.is_quit() {
                cache.search_finish(state.at);
                return Err(MatchError::quit(input.haystack()[state.at], state.at));
            } else {
                debug_assert!(sid.is_unknown());
                unreachable!("sid being unknown is a bug");
            }
        }
        state.at += 1;
        cache.search_update(state.at);
    }
    let result = eoi_fwd(dfa, cache, input, &mut sid, &mut state.mat);
    state.id = Some(sid);
    if state.mat.is_some() {
        state.next_match_index = Some(1);
    }
    cache.search_finish(input.end());
    result
}
#[inline(never)]
pub(crate) fn find_overlapping_rev(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
    state: &mut OverlappingState,
) -> Result<(), MatchError> {
    state.mat = None;
    if input.is_done() {
        return Ok(());
    }
    let mut sid = match state.id {
        None => {
            let sid = init_rev(dfa, cache, input)?;
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
                let match_len = dfa.match_len(cache, sid);
                if match_index < match_len {
                    state.next_match_index = Some(match_index + 1);
                    let pattern = dfa.match_pattern(cache, sid, match_index);
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
    cache.search_start(state.at);
    while !state.rev_eoi {
        sid = dfa
            .next_state(cache, sid, input.haystack()[state.at])
            .map_err(|_| gave_up(state.at))?;
        if sid.is_tagged() {
            state.id = Some(sid);
            if sid.is_start() {
            } else if sid.is_match() {
                state.next_match_index = Some(1);
                let pattern = dfa.match_pattern(cache, sid, 0);
                state.mat = Some(HalfMatch::new(pattern, state.at + 1));
                cache.search_finish(state.at);
                return Ok(());
            } else if sid.is_dead() {
                cache.search_finish(state.at);
                return Ok(());
            } else if sid.is_quit() {
                cache.search_finish(state.at);
                return Err(MatchError::quit(input.haystack()[state.at], state.at));
            } else {
                debug_assert!(sid.is_unknown());
                unreachable!("sid being unknown is a bug");
            }
        }
        if state.at == input.start() {
            break;
        }
        state.at -= 1;
        cache.search_update(state.at);
    }
    let result = eoi_rev(dfa, cache, input, &mut sid, &mut state.mat);
    state.rev_eoi = true;
    state.id = Some(sid);
    if state.mat.is_some() {
        state.next_match_index = Some(1);
    }
    cache.search_finish(input.start());
    result
}
#[cfg_attr(all(), inline(always))]
fn init_fwd(dfa: &DFA, cache: &mut Cache, input: &Input<'_>) -> Result<LazyStateID, MatchError> {
    let sid = dfa.start_state_forward(cache, input)?;
    debug_assert!(!sid.is_match());
    Ok(sid)
}
#[cfg_attr(all(), inline(always))]
fn init_rev(dfa: &DFA, cache: &mut Cache, input: &Input<'_>) -> Result<LazyStateID, MatchError> {
    let sid = dfa.start_state_reverse(cache, input)?;
    debug_assert!(!sid.is_match());
    Ok(sid)
}
#[cfg_attr(all(), inline(always))]
fn eoi_fwd(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
    sid: &mut LazyStateID,
    mat: &mut Option<HalfMatch>,
) -> Result<(), MatchError> {
    let sp = input.get_span();
    match input.haystack().get(sp.end) {
        Some(&b) => {
            *sid = dfa
                .next_state(cache, *sid, b)
                .map_err(|_| gave_up(sp.end))?;
            if sid.is_match() {
                let pattern = dfa.match_pattern(cache, *sid, 0);
                *mat = Some(HalfMatch::new(pattern, sp.end));
            } else if sid.is_quit() {
                return Err(MatchError::quit(b, sp.end));
            }
        }
        None => {
            *sid = dfa
                .next_eoi_state(cache, *sid)
                .map_err(|_| gave_up(input.haystack().len()))?;
            if sid.is_match() {
                let pattern = dfa.match_pattern(cache, *sid, 0);
                *mat = Some(HalfMatch::new(pattern, input.haystack().len()));
            }
            debug_assert!(!sid.is_quit());
        }
    }
    Ok(())
}
#[cfg_attr(all(), inline(always))]
fn eoi_rev(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
    sid: &mut LazyStateID,
    mat: &mut Option<HalfMatch>,
) -> Result<(), MatchError> {
    let sp = input.get_span();
    if sp.start > 0 {
        let byte = input.haystack()[sp.start - 1];
        *sid = dfa
            .next_state(cache, *sid, byte)
            .map_err(|_| gave_up(sp.start))?;
        if sid.is_match() {
            let pattern = dfa.match_pattern(cache, *sid, 0);
            *mat = Some(HalfMatch::new(pattern, sp.start));
        } else if sid.is_quit() {
            return Err(MatchError::quit(byte, sp.start - 1));
        }
    } else {
        *sid = dfa
            .next_eoi_state(cache, *sid)
            .map_err(|_| gave_up(sp.start))?;
        if sid.is_match() {
            let pattern = dfa.match_pattern(cache, *sid, 0);
            *mat = Some(HalfMatch::new(pattern, 0));
        }
        debug_assert!(!sid.is_quit());
    }
    Ok(())
}
#[cfg_attr(all(), inline(always))]
fn prefilter_restart(
    dfa: &DFA,
    cache: &mut Cache,
    input: &Input<'_>,
    at: usize,
) -> Result<LazyStateID, MatchError> {
    let mut input = input.clone();
    input.set_start(at);
    init_fwd(dfa, cache, &input)
}
#[cfg_attr(all(), inline(always))]
fn gave_up(offset: usize) -> MatchError {
    MatchError::gave_up(offset)
}
