use crate::regex_engine::automata::{
    nfa::thompson::{self, BuildError, Builder, compiler::ThompsonRef},
    util::primitives::{IteratorIndexExt, StateID},
};
use core::mem;
use std::{vec, vec::Vec};
#[derive(Clone)]
pub(crate) struct LiteralTrie {
    states: Vec<State>,
    rev: bool,
}
impl LiteralTrie {
    pub(crate) fn forward() -> LiteralTrie {
        let root = State::default();
        LiteralTrie {
            states: vec![root],
            rev: false,
        }
    }
    pub(crate) fn reverse() -> LiteralTrie {
        let root = State::default();
        LiteralTrie {
            states: vec![root],
            rev: true,
        }
    }
    pub(crate) fn add(&mut self, bytes: &[u8]) -> Result<(), BuildError> {
        let mut prev = StateID::ZERO;
        let mut it = bytes.iter().copied();
        while let Some(b) = if self.rev { it.next_back() } else { it.next() } {
            prev = self.get_or_add_state(prev, b)?;
        }
        self.states[prev].add_match();
        Ok(())
    }
    fn get_or_add_state(&mut self, from: StateID, byte: u8) -> Result<StateID, BuildError> {
        let active = self.states[from].active_chunk();
        match active.binary_search_by_key(&byte, |t| t.byte) {
            Ok(i) => Ok(active[i].next),
            Err(i) => {
                let next = StateID::new(self.states.len())
                    .map_err(|_| BuildError::too_many_states(self.states.len()))?;
                self.states.push(State::default());
                let i = self.states[from].active_chunk_start() + i;
                let t = Transition { byte, next };
                self.states[from].transitions.insert(i, t);
                Ok(next)
            }
        }
    }
    pub(crate) fn compile(&self, builder: &mut Builder) -> Result<ThompsonRef, BuildError> {
        let end = builder.add_empty()?;
        let mut stack = vec![];
        let mut f = Frame::new(&self.states[StateID::ZERO]);
        loop {
            if let Some(t) = f.transitions.next() {
                if self.states[t.next].is_leaf() {
                    f.sparse.push(thompson::Transition {
                        start: t.byte,
                        end: t.byte,
                        next: end,
                    });
                } else {
                    f.sparse.push(thompson::Transition {
                        start: t.byte,
                        end: t.byte,
                        next: StateID::ZERO,
                    });
                    stack.push(f);
                    f = Frame::new(&self.states[t.next]);
                }
                continue;
            }
            if !f.sparse.is_empty() {
                let chunk_id = if f.sparse.len() == 1 {
                    builder.add_range(f.sparse.pop().unwrap())?
                } else {
                    let sparse = mem::replace(&mut f.sparse, vec![]);
                    builder.add_sparse(sparse)?
                };
                f.union.push(chunk_id);
            }
            if let Some(chunk) = f.chunks.next() {
                f.union.push(end);
                f.transitions = chunk.iter();
                continue;
            }
            let start = builder.add_union(f.union)?;
            match stack.pop() {
                None => {
                    return Ok(ThompsonRef { start, end });
                }
                Some(mut parent) => {
                    parent.sparse.last_mut().unwrap().next = start;
                    f = parent;
                }
            }
        }
    }
}
impl core::fmt::Debug for LiteralTrie {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        writeln!(f, "LiteralTrie(")?;
        for (sid, state) in self.states.iter().with_state_ids() {
            writeln!(f, "{:06?}: {:?}", sid.as_usize(), state)?;
        }
        writeln!(f, ")")?;
        Ok(())
    }
}
#[derive(Debug)]
struct Frame<'a> {
    chunks: StateChunksIter<'a>,
    transitions: core::slice::Iter<'a, Transition>,
    union: Vec<StateID>,
    sparse: Vec<thompson::Transition>,
}
impl<'a> Frame<'a> {
    fn new(state: &'a State) -> Frame<'a> {
        let mut chunks = state.chunks();
        let chunk = chunks.next().unwrap();
        let transitions = chunk.iter();
        Frame {
            chunks,
            transitions,
            union: vec![],
            sparse: vec![],
        }
    }
}
#[derive(Clone, Default)]
struct State {
    transitions: Vec<Transition>,
    chunks: Vec<(usize, usize)>,
}
impl State {
    fn add_match(&mut self) {
        if self.transitions.is_empty() && !self.chunks.is_empty() {
            return;
        }
        let chunk_start = self.active_chunk_start();
        let chunk_end = self.transitions.len();
        self.chunks.push((chunk_start, chunk_end));
    }
    fn is_leaf(&self) -> bool {
        self.transitions.is_empty()
    }
    fn chunks(&self) -> StateChunksIter<'_> {
        StateChunksIter {
            transitions: &*self.transitions,
            chunks: self.chunks.iter(),
            active: Some(self.active_chunk()),
        }
    }
    fn active_chunk(&self) -> &[Transition] {
        let start = self.active_chunk_start();
        &self.transitions[start..]
    }
    fn active_chunk_start(&self) -> usize {
        self.chunks.last().map_or(0, |&(_, end)| end)
    }
}
impl core::fmt::Debug for State {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let mut spacing = " ";
        for (i, chunk) in self.chunks().enumerate() {
            if i > 0 {
                write!(f, "{spacing}MATCH")?;
            }
            spacing = "";
            for (j, t) in chunk.iter().enumerate() {
                spacing = " ";
                if j == 0 && i > 0 {
                    write!(f, " ")?;
                } else if j > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{t:?}")?;
            }
        }
        Ok(())
    }
}
#[derive(Debug)]
struct StateChunksIter<'a> {
    transitions: &'a [Transition],
    chunks: core::slice::Iter<'a, (usize, usize)>,
    active: Option<&'a [Transition]>,
}
impl<'a> Iterator for StateChunksIter<'a> {
    type Item = &'a [Transition];
    fn next(&mut self) -> Option<&'a [Transition]> {
        if let Some(&(start, end)) = self.chunks.next() {
            return Some(&self.transitions[start..end]);
        }
        if let Some(chunk) = self.active.take() {
            return Some(chunk);
        }
        None
    }
}
#[derive(Clone, Copy)]
struct Transition {
    byte: u8,
    next: StateID,
}
impl core::fmt::Debug for Transition {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(
            f,
            "{:?} => {}",
            crate::regex_engine::automata::util::escape::DebugByte(self.byte),
            self.next.as_usize()
        )
    }
}
