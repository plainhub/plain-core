use regex_automata::Anchored;
use regex_automata::Input;
use regex_automata::meta::Regex;
use regex_automata::util::look::LookMatcher;
use regex_automata::util::primitives::NonMaxUsize;
use std::collections::BTreeSet;

use super::Assertion;
use super::Error;
use super::Result;
use super::codepoint_len;
use super::error::RuntimeError;
use super::prev_codepoint_ix;

pub(crate) const OPTION_SKIPPED_EMPTY_MATCH: u32 = 1 << 1;

// TODO: make configurable
const MAX_STACK: usize = 1_000_000;

#[derive(Debug, Clone)]
pub enum Insn {
    End,

    Any,

    AnyNoNL,

    Assertion(Assertion),

    Lit(String), // should be cow?

    Split(usize, usize),

    Jmp(usize),

    Save(usize),

    Save0(usize),

    Restore(usize),

    RepeatGr {
        lo: usize,

        hi: usize,

        next: usize,

        repeat: usize,
    },

    RepeatNg {
        lo: usize,

        hi: usize,

        next: usize,

        repeat: usize,
    },

    RepeatEpsilonGr {
        lo: usize,

        next: usize,

        repeat: usize,

        check: usize,
    },

    RepeatEpsilonNg {
        lo: usize,

        next: usize,

        repeat: usize,

        check: usize,
    },

    FailNegativeLookAround,

    GoBack(usize),

    Backref(usize),

    BeginAtomic,

    EndAtomic,

    Delegate {
        inner: Regex,

        start_group: usize,

        end_group: usize,
    },

    ContinueFromPreviousMatchEnd,

    BackrefExistsCondition(usize),
}

#[derive(Debug, Clone)]
pub struct Prog {
    pub body: Vec<Insn>,
    n_saves: usize,
}

impl Prog {
    pub(crate) fn new(body: Vec<Insn>, n_saves: usize) -> Prog {
        Prog { body, n_saves }
    }
}

#[derive(Debug)]
struct Branch {
    pc: usize,
    ix: usize,
    nsave: usize,
}

#[derive(Debug)]
struct Save {
    slot: usize,
    value: usize,
}

struct State {
    saves: Vec<usize>,

    stack: Vec<Branch>,

    oldsave: Vec<Save>,

    nsave: usize,
    explicit_sp: usize,

    max_stack: usize,
}

// Each element in the stack conceptually represents the entire state
// of the machine: the pc (index into prog), the index into the
// string, and the entire vector of saves. However, copying the save
// vector on every push/pop would be inefficient, so instead we use a
// copy-on-write approach for each slot within the save vector. The
// top `nsave` elements in `oldsave` represent the delta from the
// current machine state to the top of stack.

impl State {
    fn new(n_saves: usize, max_stack: usize) -> State {
        State {
            saves: vec![usize::MAX; n_saves],
            stack: Vec::new(),
            oldsave: Vec::new(),
            nsave: 0,
            explicit_sp: n_saves,
            max_stack,
        }
    }

    // push a backtrack branch
    fn push(&mut self, pc: usize, ix: usize) -> Result<()> {
        if self.stack.len() < self.max_stack {
            let nsave = self.nsave;
            self.stack.push(Branch { pc, ix, nsave });
            self.nsave = 0;
            Ok(())
        } else {
            Err(Error::RuntimeError(RuntimeError::StackOverflow))
        }
    }

    // pop a backtrack branch
    fn pop(&mut self) -> (usize, usize) {
        for _ in 0..self.nsave {
            let Save { slot, value } = self.oldsave.pop().unwrap();
            self.saves[slot] = value;
        }
        let Branch { pc, ix, nsave } = self.stack.pop().unwrap();
        self.nsave = nsave;
        (pc, ix)
    }

    fn save(&mut self, slot: usize, val: usize) {
        for i in 0..self.nsave {
            // could avoid this iteration with some overhead; worth it?
            if self.oldsave[self.oldsave.len() - i - 1].slot == slot {
                // already saved, just update
                self.saves[slot] = val;
                return;
            }
        }
        self.oldsave.push(Save {
            slot,
            value: self.saves[slot],
        });
        self.nsave += 1;
        self.saves[slot] = val;
    }

    fn get(&self, slot: usize) -> usize {
        self.saves[slot]
    }

    // push a value onto the explicit stack; note: the entire contents of
    // the explicit stack is saved and restored on backtrack.
    fn stack_push(&mut self, val: usize) {
        if self.saves.len() == self.explicit_sp {
            self.saves.push(self.explicit_sp + 1);
        }
        let explicit_sp = self.explicit_sp;
        let sp = self.get(explicit_sp);
        if self.saves.len() == sp {
            self.saves.push(val);
        } else {
            self.save(sp, val);
        }
        self.save(explicit_sp, sp + 1);
    }

    // pop a value from the explicit stack
    fn stack_pop(&mut self) -> usize {
        let explicit_sp = self.explicit_sp;
        let sp = self.get(explicit_sp) - 1;
        let result = self.get(sp);
        self.save(explicit_sp, sp);
        result
    }

    fn backtrack_count(&self) -> usize {
        self.stack.len()
    }

    fn backtrack_cut(&mut self, count: usize) {
        if self.stack.len() == count {
            // no backtrack branches to discard, all good
            return;
        }
        // start and end indexes of old saves for the branch we're cutting to
        let (oldsave_start, oldsave_end) = {
            let mut end = self.oldsave.len() - self.nsave;
            for &Branch { nsave, .. } in &self.stack[count + 1..] {
                end -= nsave;
            }
            let start = end - self.stack[count].nsave;
            (start, end)
        };
        let mut saved = BTreeSet::new();
        // keep all the old saves of our branch (they're all for different slots)
        for &Save { slot, .. } in &self.oldsave[oldsave_start..oldsave_end] {
            saved.insert(slot);
        }
        let mut oldsave_ix = oldsave_end;
        // for other old saves, keep them only if they're for a slot that we haven't saved yet
        for ix in oldsave_end..self.oldsave.len() {
            let Save { slot, .. } = self.oldsave[ix];
            let new_slot = saved.insert(slot);
            if new_slot {
                // put the save we want to keep (ix) after the ones we already have (oldsave_ix)
                // note that it's fine if the indexes are the same (then swapping is a no-op)
                self.oldsave.swap(oldsave_ix, ix);
                oldsave_ix += 1;
            }
        }
        self.stack.truncate(count);
        self.oldsave.truncate(oldsave_ix);
        self.nsave = oldsave_ix - oldsave_start;
    }
}

fn codepoint_len_at(s: &str, ix: usize) -> usize {
    codepoint_len(s.as_bytes()[ix])
}

#[inline]
fn matches_literal(s: &str, ix: usize, end: usize, literal: &str) -> bool {
    // Compare as bytes because the literal might be a single byte char whereas ix
    // points to a multibyte char. Comparing with str would result in an error like
    // "byte index N is not a char boundary".
    end <= s.len() && &s.as_bytes()[ix..end] == literal.as_bytes()
}

pub(crate) fn run(
    prog: &Prog,
    s: &str,
    pos: usize,
    option_flags: u32,
) -> Result<Option<Vec<usize>>> {
    let mut state = State::new(prog.n_saves, MAX_STACK);
    let mut inner_slots: Vec<Option<NonMaxUsize>> = Vec::new();
    let look_matcher = LookMatcher::new();
    let mut backtrack_count = 0;
    let mut pc = 0;
    let mut ix = pos;
    loop {
        // break from this loop to fail, causes stack to pop
        'fail: loop {
            match prog.body[pc] {
                Insn::End => {
                    // save of end position into slot 1 is now done
                    // with an explicit group; we might want to
                    // optimize that.
                    //state.saves[1] = ix;
                    if let Some(&slot1) = state.saves.get(1) {
                        // With some features like keep out (\K), the match start can be after
                        // the match end. Cap the start to <= end.
                        if state.get(0) > slot1 {
                            state.save(0, slot1);
                        }
                    }
                    return Ok(Some(state.saves));
                }
                Insn::Any => {
                    if ix < s.len() {
                        ix += codepoint_len_at(s, ix);
                    } else {
                        break 'fail;
                    }
                }
                Insn::AnyNoNL => {
                    if ix < s.len() && s.as_bytes()[ix] != b'\n' {
                        ix += codepoint_len_at(s, ix);
                    } else {
                        break 'fail;
                    }
                }
                Insn::Lit(ref val) => {
                    let ix_end = ix + val.len();
                    if !matches_literal(s, ix, ix_end, val) {
                        break 'fail;
                    }
                    ix = ix_end
                }
                Insn::Assertion(assertion) => {
                    if !match assertion {
                        Assertion::StartText => look_matcher.is_start(s.as_bytes(), ix),
                        Assertion::EndText => look_matcher.is_end(s.as_bytes(), ix),
                        Assertion::StartLine { crlf: false } => {
                            look_matcher.is_start_lf(s.as_bytes(), ix)
                        }
                        Assertion::StartLine { crlf: true } => {
                            look_matcher.is_start_crlf(s.as_bytes(), ix)
                        }
                        Assertion::EndLine { crlf: false } => {
                            look_matcher.is_end_lf(s.as_bytes(), ix)
                        }
                        Assertion::EndLine { crlf: true } => {
                            look_matcher.is_end_crlf(s.as_bytes(), ix)
                        }
                        Assertion::LeftWordBoundary => look_matcher
                            .is_word_start_unicode(s.as_bytes(), ix)
                            .unwrap(),
                        Assertion::RightWordBoundary => {
                            look_matcher.is_word_end_unicode(s.as_bytes(), ix).unwrap()
                        }
                        Assertion::WordBoundary => {
                            look_matcher.is_word_unicode(s.as_bytes(), ix).unwrap()
                        }
                        Assertion::NotWordBoundary => look_matcher
                            .is_word_unicode_negate(s.as_bytes(), ix)
                            .unwrap(),
                    } {
                        break 'fail;
                    }
                }
                Insn::Split(x, y) => {
                    state.push(y, ix)?;
                    pc = x;
                    continue;
                }
                Insn::Jmp(target) => {
                    pc = target;
                    continue;
                }
                Insn::Save(slot) => state.save(slot, ix),
                Insn::Save0(slot) => state.save(slot, 0),
                Insn::Restore(slot) => ix = state.get(slot),
                Insn::RepeatGr {
                    lo,
                    hi,
                    next,
                    repeat,
                } => {
                    let repcount = state.get(repeat);
                    if repcount == hi {
                        pc = next;
                        continue;
                    }
                    state.save(repeat, repcount + 1);
                    if repcount >= lo {
                        state.push(next, ix)?;
                    }
                }
                Insn::RepeatNg {
                    lo,
                    hi,
                    next,
                    repeat,
                } => {
                    let repcount = state.get(repeat);
                    if repcount == hi {
                        pc = next;
                        continue;
                    }
                    state.save(repeat, repcount + 1);
                    if repcount >= lo {
                        state.push(pc + 1, ix)?;
                        pc = next;
                        continue;
                    }
                }
                Insn::RepeatEpsilonGr {
                    lo,
                    next,
                    repeat,
                    check,
                } => {
                    let repcount = state.get(repeat);
                    if repcount > lo && state.get(check) == ix {
                        // prevent zero-length match on repeat
                        break 'fail;
                    }
                    state.save(repeat, repcount + 1);
                    if repcount >= lo {
                        state.save(check, ix);
                        state.push(next, ix)?;
                    }
                }
                Insn::RepeatEpsilonNg {
                    lo,
                    next,
                    repeat,
                    check,
                } => {
                    let repcount = state.get(repeat);
                    if repcount > lo && state.get(check) == ix {
                        // prevent zero-length match on repeat
                        break 'fail;
                    }
                    state.save(repeat, repcount + 1);
                    if repcount >= lo {
                        state.save(check, ix);
                        state.push(pc + 1, ix)?;
                        pc = next;
                        continue;
                    }
                }
                Insn::GoBack(count) => {
                    for _ in 0..count {
                        if ix == 0 {
                            break 'fail;
                        }
                        ix = prev_codepoint_ix(s, ix);
                    }
                }
                Insn::FailNegativeLookAround => {
                    // Reaching this instruction means that the body of the
                    // look-around matched. Because it's a *negative* look-around,
                    // that means the look-around itself should fail (not match).
                    // But before, we need to discard all the states that have
                    // been pushed with the look-around, because we don't want to
                    // explore them.
                    loop {
                        let (popped_pc, _) = state.pop();
                        if popped_pc == pc + 1 {
                            // We've reached the state that would jump us to
                            // after the look-around (in case the look-around
                            // succeeded). That means we popped enough states.
                            break;
                        }
                    }
                    break 'fail;
                }
                Insn::Backref(slot) => {
                    let lo = state.get(slot);
                    if lo == usize::MAX {
                        // Referenced group hasn't matched, so the backref doesn't match either
                        break 'fail;
                    }
                    let hi = state.get(slot + 1);
                    if hi == usize::MAX {
                        // Referenced group hasn't matched, so the backref doesn't match either
                        break 'fail;
                    }
                    let ref_text = &s[lo..hi];
                    let ix_end = ix + ref_text.len();
                    if !matches_literal(s, ix, ix_end, ref_text) {
                        break 'fail;
                    }
                    ix = ix_end;
                }
                Insn::BackrefExistsCondition(group) => {
                    let lo = state.get(group * 2);
                    if lo == usize::MAX {
                        // Referenced group hasn't matched, so the backref doesn't match either
                        break 'fail;
                    }
                }
                Insn::BeginAtomic => {
                    let count = state.backtrack_count();
                    state.stack_push(count);
                }
                Insn::EndAtomic => {
                    let count = state.stack_pop();
                    state.backtrack_cut(count);
                }
                Insn::Delegate {
                    ref inner,
                    start_group,
                    end_group,
                } => {
                    let input = Input::new(s).span(ix..s.len()).anchored(Anchored::Yes);
                    if start_group == end_group {
                        // No groups, so we can use faster methods
                        match inner.search_half(&input) {
                            Some(m) => ix = m.offset(),
                            _ => break 'fail,
                        }
                    } else {
                        inner_slots.resize((end_group - start_group + 1) * 2, None);
                        if inner.search_slots(&input, &mut inner_slots).is_some() {
                            for i in 0..(end_group - start_group) {
                                let slot = (start_group + i) * 2;
                                if let Some(start) = inner_slots[(i + 1) * 2] {
                                    let end = inner_slots[(i + 1) * 2 + 1].unwrap();
                                    state.save(slot, start.get());
                                    state.save(slot + 1, end.get());
                                } else {
                                    state.save(slot, usize::MAX);
                                    state.save(slot + 1, usize::MAX);
                                }
                            }
                            ix = inner_slots[1].unwrap().get();
                        } else {
                            break 'fail;
                        }
                    }
                }
                Insn::ContinueFromPreviousMatchEnd => {
                    if ix > pos || option_flags & OPTION_SKIPPED_EMPTY_MATCH != 0 {
                        break 'fail;
                    }
                }
            }
            pc += 1;
        }
        // "break 'fail" goes here
        if state.stack.is_empty() {
            return Ok(None);
        }

        backtrack_count += 1;
        if backtrack_count > 1_000_000 {
            return Err(Error::RuntimeError(RuntimeError::BacktrackLimitExceeded));
        }

        let (newpc, newix) = state.pop();
        pc = newpc;
        ix = newix;
    }
}
