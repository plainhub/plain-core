use crate::regex_engine::automata::util::primitives::StateID;
use crate::regex_engine::syntax::utf8::Utf8Range;
use core::{cell::RefCell, fmt, mem, ops::RangeInclusive};
use std::{format, string::String, vec, vec::Vec};
const FINAL: StateID = StateID::ZERO;
const ROOT: StateID = StateID::new_unchecked(1);
#[derive(Clone)]
pub struct RangeTrie {
    states: Vec<State>,
    free: Vec<State>,
    iter_stack: RefCell<Vec<NextIter>>,
    iter_ranges: RefCell<Vec<Utf8Range>>,
    dupe_stack: Vec<NextDupe>,
    insert_stack: Vec<NextInsert>,
}
#[derive(Clone)]
struct State {
    transitions: Vec<Transition>,
}
#[derive(Clone)]
struct Transition {
    range: Utf8Range,
    next_id: StateID,
}
impl RangeTrie {
    pub fn new() -> RangeTrie {
        let mut trie = RangeTrie {
            states: vec![],
            free: vec![],
            iter_stack: RefCell::new(vec![]),
            iter_ranges: RefCell::new(vec![]),
            dupe_stack: vec![],
            insert_stack: vec![],
        };
        trie.clear();
        trie
    }
    pub fn clear(&mut self) {
        self.free.append(&mut self.states);
        self.add_empty();
        self.add_empty();
    }
    pub fn iter<E, F: FnMut(&[Utf8Range]) -> Result<(), E>>(&self, mut f: F) -> Result<(), E> {
        let mut stack = self.iter_stack.borrow_mut();
        stack.clear();
        let mut ranges = self.iter_ranges.borrow_mut();
        ranges.clear();
        stack.push(NextIter {
            state_id: ROOT,
            tidx: 0,
        });
        while let Some(NextIter {
            mut state_id,
            mut tidx,
        }) = stack.pop()
        {
            loop {
                let state = self.state(state_id);
                if tidx >= state.transitions.len() {
                    ranges.pop();
                    break;
                }
                let t = &state.transitions[tidx];
                ranges.push(t.range);
                if t.next_id == FINAL {
                    f(&ranges)?;
                    ranges.pop();
                    tidx += 1;
                } else {
                    stack.push(NextIter {
                        state_id,
                        tidx: tidx + 1,
                    });
                    state_id = t.next_id;
                    tidx = 0;
                }
            }
        }
        Ok(())
    }
    pub fn insert(&mut self, ranges: &[Utf8Range]) {
        assert!(!ranges.is_empty());
        assert!(ranges.len() <= 4);
        let mut stack = core::mem::replace(&mut self.insert_stack, vec![]);
        stack.clear();
        stack.push(NextInsert::new(ROOT, ranges));
        while let Some(next) = stack.pop() {
            let (state_id, ranges) = (next.state_id(), next.ranges());
            assert!(!ranges.is_empty());
            let (mut new, rest) = (ranges[0], &ranges[1..]);
            let mut i = self.state(state_id).find(new);
            if i == self.state(state_id).transitions.len() {
                let next_id = NextInsert::push(self, &mut stack, rest);
                self.add_transition(state_id, new, next_id);
                continue;
            }
            'OUTER: loop {
                let old = self.state(state_id).transitions[i].clone();
                let split = match Split::new(old.range, new) {
                    Some(split) => split,
                    None => {
                        let next_id = NextInsert::push(self, &mut stack, rest);
                        self.add_transition_at(i, state_id, new, next_id);
                        continue;
                    }
                };
                let splits = split.as_slice();
                if splits.len() == 1 {
                    if !rest.is_empty() {
                        stack.push(NextInsert::new(old.next_id, rest));
                    }
                    break;
                }
                let mut first = true;
                let mut add_trans = |trie: &mut RangeTrie, pos, from, range, to| {
                    if first {
                        trie.set_transition_at(pos, from, range, to);
                        first = false;
                    } else {
                        trie.add_transition_at(pos, from, range, to);
                    }
                };
                for (j, &srange) in splits.iter().enumerate() {
                    match srange {
                        SplitRange::Old(r) => {
                            let dup_id = self.duplicate(old.next_id);
                            add_trans(self, i, state_id, r, dup_id);
                        }
                        SplitRange::New(r) => {
                            {
                                let trans = &self.state(state_id).transitions;
                                if j + 1 == splits.len()
                                    && i < trans.len()
                                    && intersects(r, trans[i].range)
                                {
                                    new = r;
                                    continue 'OUTER;
                                }
                            }
                            let next_id = NextInsert::push(self, &mut stack, rest);
                            add_trans(self, i, state_id, r, next_id);
                        }
                        SplitRange::Both(r) => {
                            if !rest.is_empty() {
                                stack.push(NextInsert::new(old.next_id, rest));
                            }
                            add_trans(self, i, state_id, r, old.next_id);
                        }
                    }
                    i += 1;
                }
                break;
            }
        }
        self.insert_stack = stack;
    }
    pub fn add_empty(&mut self) -> StateID {
        let id = match StateID::try_from(self.states.len()) {
            Ok(id) => id,
            Err(_) => {
                panic!("too many sequences added to range trie");
            }
        };
        if let Some(mut state) = self.free.pop() {
            state.clear();
            self.states.push(state);
        } else {
            self.states.push(State {
                transitions: vec![],
            });
        }
        id
    }
    fn duplicate(&mut self, old_id: StateID) -> StateID {
        if old_id == FINAL {
            return FINAL;
        }
        let mut stack = mem::replace(&mut self.dupe_stack, vec![]);
        stack.clear();
        let new_id = self.add_empty();
        stack.push(NextDupe { old_id, new_id });
        while let Some(NextDupe { old_id, new_id }) = stack.pop() {
            for i in 0..self.state(old_id).transitions.len() {
                let t = self.state(old_id).transitions[i].clone();
                if t.next_id == FINAL {
                    self.add_transition(new_id, t.range, FINAL);
                    continue;
                }
                let new_child_id = self.add_empty();
                self.add_transition(new_id, t.range, new_child_id);
                stack.push(NextDupe {
                    old_id: t.next_id,
                    new_id: new_child_id,
                });
            }
        }
        self.dupe_stack = stack;
        new_id
    }
    fn add_transition(&mut self, from_id: StateID, range: Utf8Range, next_id: StateID) {
        self.state_mut(from_id)
            .transitions
            .push(Transition { range, next_id });
    }
    fn add_transition_at(
        &mut self,
        i: usize,
        from_id: StateID,
        range: Utf8Range,
        next_id: StateID,
    ) {
        self.state_mut(from_id)
            .transitions
            .insert(i, Transition { range, next_id });
    }
    fn set_transition_at(
        &mut self,
        i: usize,
        from_id: StateID,
        range: Utf8Range,
        next_id: StateID,
    ) {
        self.state_mut(from_id).transitions[i] = Transition { range, next_id };
    }
    fn state(&self, id: StateID) -> &State {
        &self.states[id]
    }
    fn state_mut(&mut self, id: StateID) -> &mut State {
        &mut self.states[id]
    }
}
impl State {
    fn find(&self, range: Utf8Range) -> usize {
        fn binary_search<T, F>(xs: &[T], mut pred: F) -> usize
        where
            F: FnMut(&T) -> bool,
        {
            let (mut left, mut right) = (0, xs.len());
            while left < right {
                let mid = (left + right) / 2;
                if pred(&xs[mid]) {
                    right = mid;
                } else {
                    left = mid + 1;
                }
            }
            left
        }
        binary_search(&self.transitions, |t| range.start <= t.range.end)
    }
    fn clear(&mut self) {
        self.transitions.clear();
    }
}
#[derive(Clone, Debug)]
struct NextDupe {
    old_id: StateID,
    new_id: StateID,
}
#[derive(Clone, Debug)]
struct NextIter {
    state_id: StateID,
    tidx: usize,
}
#[derive(Clone, Debug)]
struct NextInsert {
    state_id: StateID,
    ranges: [Utf8Range; 4],
    len: u8,
}
impl NextInsert {
    fn new(state_id: StateID, ranges: &[Utf8Range]) -> NextInsert {
        let len = ranges.len();
        assert!(len > 0);
        assert!(len <= 4);
        let mut tmp = [Utf8Range { start: 0, end: 0 }; 4];
        tmp[..len].copy_from_slice(ranges);
        NextInsert {
            state_id,
            ranges: tmp,
            len: u8::try_from(len).unwrap(),
        }
    }
    fn push(trie: &mut RangeTrie, stack: &mut Vec<NextInsert>, ranges: &[Utf8Range]) -> StateID {
        if ranges.is_empty() {
            FINAL
        } else {
            let next_id = trie.add_empty();
            stack.push(NextInsert::new(next_id, ranges));
            next_id
        }
    }
    fn state_id(&self) -> StateID {
        self.state_id
    }
    fn ranges(&self) -> &[Utf8Range] {
        &self.ranges[..usize::try_from(self.len).unwrap()]
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
struct Split {
    partitions: [SplitRange; 3],
    len: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SplitRange {
    Old(Utf8Range),
    New(Utf8Range),
    Both(Utf8Range),
}
impl Split {
    fn new(o: Utf8Range, n: Utf8Range) -> Option<Split> {
        let range = |r: RangeInclusive<u8>| Utf8Range {
            start: *r.start(),
            end: *r.end(),
        };
        let old = |r| SplitRange::Old(range(r));
        let new = |r| SplitRange::New(range(r));
        let both = |r| SplitRange::Both(range(r));
        let (a, b, x, y) = (o.start, o.end, n.start, n.end);
        if b < x || y < a {
            None
        } else if a == x && b == y {
            Some(Split::parts1(both(a..=b)))
        } else if a == x && b < y {
            Some(Split::parts2(both(a..=b), new(b + 1..=y)))
        } else if b == y && a > x {
            Some(Split::parts2(new(x..=a - 1), both(a..=b)))
        } else if x == a && y < b {
            Some(Split::parts2(both(x..=y), old(y + 1..=b)))
        } else if y == b && x > a {
            Some(Split::parts2(old(a..=x - 1), both(x..=y)))
        } else if a > x && b < y {
            Some(Split::parts3(new(x..=a - 1), both(a..=b), new(b + 1..=y)))
        } else if x > a && y < b {
            Some(Split::parts3(old(a..=x - 1), both(x..=y), old(y + 1..=b)))
        } else if b == x && a < y {
            Some(Split::parts3(old(a..=b - 1), both(b..=b), new(b + 1..=y)))
        } else if y == a && x < b {
            Some(Split::parts3(new(x..=y - 1), both(y..=y), old(y + 1..=b)))
        } else if b > x && b < y {
            Some(Split::parts3(old(a..=x - 1), both(x..=b), new(b + 1..=y)))
        } else if y > a && y < b {
            Some(Split::parts3(new(x..=a - 1), both(a..=y), old(y + 1..=b)))
        } else {
            unreachable!()
        }
    }
    fn parts1(r1: SplitRange) -> Split {
        let nada = SplitRange::Old(Utf8Range { start: 0, end: 0 });
        Split {
            partitions: [r1, nada, nada],
            len: 1,
        }
    }
    fn parts2(r1: SplitRange, r2: SplitRange) -> Split {
        let nada = SplitRange::Old(Utf8Range { start: 0, end: 0 });
        Split {
            partitions: [r1, r2, nada],
            len: 2,
        }
    }
    fn parts3(r1: SplitRange, r2: SplitRange, r3: SplitRange) -> Split {
        Split {
            partitions: [r1, r2, r3],
            len: 3,
        }
    }
    fn as_slice(&self) -> &[SplitRange] {
        &self.partitions[..self.len]
    }
}
impl fmt::Debug for RangeTrie {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f)?;
        for (i, state) in self.states.iter().enumerate() {
            let status = if i == FINAL.as_usize() { '*' } else { ' ' };
            writeln!(f, "{status}{i:06}: {state:?}")?;
        }
        Ok(())
    }
}
impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let rs = self
            .transitions
            .iter()
            .map(|t| format!("{t:?}"))
            .collect::<Vec<String>>()
            .join(", ");
        write!(f, "{rs}")
    }
}
impl fmt::Debug for Transition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.range.start == self.range.end {
            write!(
                f,
                "{:02X} => {:02X}",
                self.range.start,
                self.next_id.as_usize(),
            )
        } else {
            write!(
                f,
                "{:02X}-{:02X} => {:02X}",
                self.range.start,
                self.range.end,
                self.next_id.as_usize(),
            )
        }
    }
}
fn intersects(r1: Utf8Range, r2: Utf8Range) -> bool {
    !(r1.end < r2.start || r2.end < r1.start)
}
