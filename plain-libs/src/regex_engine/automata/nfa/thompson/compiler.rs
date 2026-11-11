use crate::regex_engine::automata::{
    nfa::thompson::{
        builder::Builder,
        error::BuildError,
        literal_trie::LiteralTrie,
        map::{Utf8BoundedMap, Utf8SuffixKey, Utf8SuffixMap},
        nfa::{NFA, Transition},
        range_trie::RangeTrie,
    },
    util::{
        look::{Look, LookMatcher},
        primitives::{PatternID, StateID},
    },
};
use crate::regex_engine::syntax::{
    ParserBuilder,
    hir::{self, Hir},
    utf8::{Utf8Range, Utf8Sequences},
};
use core::{borrow::Borrow, cell::RefCell};
use std::{sync::Arc, vec, vec::Vec};
#[derive(Clone, Debug, Default)]
pub struct Config {
    utf8: Option<bool>,
    reverse: Option<bool>,
    nfa_size_limit: Option<Option<usize>>,
    shrink: Option<bool>,
    which_captures: Option<WhichCaptures>,
    look_matcher: Option<LookMatcher>,
}
impl Config {
    pub fn new() -> Config {
        Config::default()
    }
    pub fn utf8(mut self, yes: bool) -> Config {
        self.utf8 = Some(yes);
        self
    }
    pub fn reverse(mut self, yes: bool) -> Config {
        self.reverse = Some(yes);
        self
    }
    pub fn nfa_size_limit(mut self, bytes: Option<usize>) -> Config {
        self.nfa_size_limit = Some(bytes);
        self
    }
    pub fn shrink(mut self, yes: bool) -> Config {
        self.shrink = Some(yes);
        self
    }
    pub fn which_captures(mut self, which_captures: WhichCaptures) -> Config {
        self.which_captures = Some(which_captures);
        self
    }
    pub fn look_matcher(mut self, m: LookMatcher) -> Config {
        self.look_matcher = Some(m);
        self
    }
    pub fn get_utf8(&self) -> bool {
        self.utf8.unwrap_or(true)
    }
    pub fn get_reverse(&self) -> bool {
        self.reverse.unwrap_or(false)
    }
    pub fn get_nfa_size_limit(&self) -> Option<usize> {
        self.nfa_size_limit.unwrap_or(None)
    }
    pub fn get_shrink(&self) -> bool {
        self.shrink.unwrap_or(false)
    }
    pub fn get_which_captures(&self) -> WhichCaptures {
        self.which_captures.unwrap_or(WhichCaptures::All)
    }
    pub fn get_look_matcher(&self) -> LookMatcher {
        self.look_matcher.clone().unwrap_or(LookMatcher::default())
    }
    fn get_unanchored_prefix(&self) -> bool {
        { true }
    }
    pub(crate) fn overwrite(&self, o: Config) -> Config {
        Config {
            utf8: o.utf8.or(self.utf8),
            reverse: o.reverse.or(self.reverse),
            nfa_size_limit: o.nfa_size_limit.or(self.nfa_size_limit),
            shrink: o.shrink.or(self.shrink),
            which_captures: o.which_captures.or(self.which_captures),
            look_matcher: o.look_matcher.or_else(|| self.look_matcher.clone()),
        }
    }
}
#[derive(Clone, Copy, Debug)]
pub enum WhichCaptures {
    All,
    Implicit,
    None,
}
impl Default for WhichCaptures {
    fn default() -> WhichCaptures {
        WhichCaptures::All
    }
}
impl WhichCaptures {
    pub fn is_none(&self) -> bool {
        matches!(*self, WhichCaptures::None)
    }
    pub fn is_any(&self) -> bool {
        !self.is_none()
    }
}
#[derive(Clone, Debug)]
pub struct Compiler {
    parser: ParserBuilder,
    config: Config,
    builder: RefCell<Builder>,
    utf8_state: RefCell<Utf8State>,
    trie_state: RefCell<RangeTrie>,
    utf8_suffix: RefCell<Utf8SuffixMap>,
}
impl Compiler {
    pub fn new() -> Compiler {
        Compiler {
            parser: ParserBuilder::new(),
            config: Config::default(),
            builder: RefCell::new(Builder::new()),
            utf8_state: RefCell::new(Utf8State::new()),
            trie_state: RefCell::new(RangeTrie::new()),
            utf8_suffix: RefCell::new(Utf8SuffixMap::new(1000)),
        }
    }
    pub fn build(&self, pattern: &str) -> Result<NFA, BuildError> {
        self.build_many(&[pattern])
    }
    pub fn build_many<P: AsRef<str>>(&self, patterns: &[P]) -> Result<NFA, BuildError> {
        let mut hirs = vec![];
        for p in patterns {
            hirs.push(
                self.parser
                    .build()
                    .parse(p.as_ref())
                    .map_err(BuildError::syntax)?,
            );
            debug!("parsed: {:?}", p.as_ref());
        }
        self.build_many_from_hir(&hirs)
    }
    pub fn build_from_hir(&self, expr: &Hir) -> Result<NFA, BuildError> {
        self.build_many_from_hir(&[expr])
    }
    pub fn build_many_from_hir<H: Borrow<Hir>>(&self, exprs: &[H]) -> Result<NFA, BuildError> {
        self.compile(exprs)
    }
    pub fn configure(&mut self, config: Config) -> &mut Compiler {
        self.config = self.config.overwrite(config);
        self
    }
    pub fn syntax(
        &mut self,
        config: crate::regex_engine::automata::util::syntax::Config,
    ) -> &mut Compiler {
        config.apply(&mut self.parser);
        self
    }
}
impl Compiler {
    fn compile<H: Borrow<Hir>>(&self, exprs: &[H]) -> Result<NFA, BuildError> {
        if exprs.len() > PatternID::LIMIT {
            return Err(BuildError::too_many_patterns(exprs.len()));
        }
        if self.config.get_reverse() && self.config.get_which_captures().is_any() {
            return Err(BuildError::unsupported_captures());
        }
        self.builder.borrow_mut().clear();
        self.builder.borrow_mut().set_utf8(self.config.get_utf8());
        self.builder
            .borrow_mut()
            .set_reverse(self.config.get_reverse());
        self.builder
            .borrow_mut()
            .set_look_matcher(self.config.get_look_matcher());
        self.builder
            .borrow_mut()
            .set_size_limit(self.config.get_nfa_size_limit())?;
        let all_anchored = exprs.iter().all(|e| {
            let props = e.borrow().properties();
            if self.config.get_reverse() {
                props.look_set_suffix().contains(hir::Look::End)
            } else {
                props.look_set_prefix().contains(hir::Look::Start)
            }
        });
        let anchored = !self.config.get_unanchored_prefix() || all_anchored;
        let unanchored_prefix = if anchored {
            self.c_empty()?
        } else {
            self.c_at_least(&Hir::dot(hir::Dot::AnyByte), false, 0)?
        };
        let compiled = self.c_alt_iter(exprs.iter().map(|e| {
            let _ = self.start_pattern()?;
            let one = self.c_cap(0, None, e.borrow())?;
            let match_state_id = self.add_match()?;
            self.patch(one.end, match_state_id)?;
            let _ = self.finish_pattern(one.start)?;
            Ok(ThompsonRef {
                start: one.start,
                end: match_state_id,
            })
        }))?;
        self.patch(unanchored_prefix.end, compiled.start)?;
        let nfa = self
            .builder
            .borrow_mut()
            .build(compiled.start, unanchored_prefix.start)?;
        debug!("HIR-to-NFA compilation complete, config: {:?}", self.config);
        Ok(nfa)
    }
    fn c(&self, expr: &Hir) -> Result<ThompsonRef, BuildError> {
        use crate::regex_engine::syntax::hir::{Class, HirKind::*};
        match *expr.kind() {
            Empty => self.c_empty(),
            Literal(hir::Literal(ref bytes)) => self.c_literal(bytes),
            Class(Class::Bytes(ref c)) => self.c_byte_class(c),
            Class(Class::Unicode(ref c)) => self.c_unicode_class(c),
            Look(ref look) => self.c_look(look),
            Repetition(ref rep) => self.c_repetition(rep),
            Capture(ref c) => self.c_cap(c.index, c.name.as_deref(), &c.sub),
            Concat(ref es) => self.c_concat(es.iter().map(|e| self.c(e))),
            Alternation(ref es) => self.c_alt_slice(es),
        }
    }
    fn c_concat<I>(&self, mut it: I) -> Result<ThompsonRef, BuildError>
    where
        I: DoubleEndedIterator<Item = Result<ThompsonRef, BuildError>>,
    {
        let first = if self.is_reverse() {
            it.next_back()
        } else {
            it.next()
        };
        let ThompsonRef { start, mut end } = match first {
            Some(result) => result?,
            None => return self.c_empty(),
        };
        loop {
            let next = if self.is_reverse() {
                it.next_back()
            } else {
                it.next()
            };
            let compiled = match next {
                Some(result) => result?,
                None => break,
            };
            self.patch(end, compiled.start)?;
            end = compiled.end;
        }
        Ok(ThompsonRef { start, end })
    }
    fn c_alt_slice(&self, exprs: &[Hir]) -> Result<ThompsonRef, BuildError> {
        let literal_count = exprs
            .iter()
            .filter(|e| matches!(*e.kind(), hir::HirKind::Literal(hir::Literal(_))))
            .count();
        if literal_count <= 1 || literal_count < exprs.len() {
            return self.c_alt_iter(exprs.iter().map(|e| self.c(e)));
        }
        let mut trie = if self.is_reverse() {
            LiteralTrie::reverse()
        } else {
            LiteralTrie::forward()
        };
        for expr in exprs.iter() {
            let literal = match *expr.kind() {
                hir::HirKind::Literal(hir::Literal(ref bytes)) => bytes,
                _ => unreachable!(),
            };
            trie.add(literal)?;
        }
        trie.compile(&mut self.builder.borrow_mut())
    }
    fn c_alt_iter<I>(&self, mut it: I) -> Result<ThompsonRef, BuildError>
    where
        I: Iterator<Item = Result<ThompsonRef, BuildError>>,
    {
        let first = match it.next() {
            None => return self.c_fail(),
            Some(result) => result?,
        };
        let second = match it.next() {
            None => return Ok(first),
            Some(result) => result?,
        };
        let union = self.add_union()?;
        let end = self.add_empty()?;
        self.patch(union, first.start)?;
        self.patch(first.end, end)?;
        self.patch(union, second.start)?;
        self.patch(second.end, end)?;
        for result in it {
            let compiled = result?;
            self.patch(union, compiled.start)?;
            self.patch(compiled.end, end)?;
        }
        Ok(ThompsonRef { start: union, end })
    }
    fn c_cap(&self, index: u32, name: Option<&str>, expr: &Hir) -> Result<ThompsonRef, BuildError> {
        match self.config.get_which_captures() {
            WhichCaptures::None => return self.c(expr),
            WhichCaptures::Implicit if index > 0 => return self.c(expr),
            _ => {}
        }
        let start = self.add_capture_start(index, name)?;
        let inner = self.c(expr)?;
        let end = self.add_capture_end(index)?;
        self.patch(start, inner.start)?;
        self.patch(inner.end, end)?;
        Ok(ThompsonRef { start, end })
    }
    fn c_repetition(&self, rep: &hir::Repetition) -> Result<ThompsonRef, BuildError> {
        match (rep.min, rep.max) {
            (0, Some(1)) => self.c_zero_or_one(&rep.sub, rep.greedy),
            (min, None) => self.c_at_least(&rep.sub, rep.greedy, min),
            (min, Some(max)) if min == max => self.c_exactly(&rep.sub, min),
            (min, Some(max)) => self.c_bounded(&rep.sub, rep.greedy, min, max),
        }
    }
    fn c_bounded(
        &self,
        expr: &Hir,
        greedy: bool,
        min: u32,
        max: u32,
    ) -> Result<ThompsonRef, BuildError> {
        let prefix = self.c_exactly(expr, min)?;
        if min == max {
            return Ok(prefix);
        }
        let empty = self.add_empty()?;
        let mut prev_end = prefix.end;
        for _ in min..max {
            let union = if greedy {
                self.add_union()
            } else {
                self.add_union_reverse()
            }?;
            let compiled = self.c(expr)?;
            self.patch(prev_end, union)?;
            self.patch(union, compiled.start)?;
            self.patch(union, empty)?;
            prev_end = compiled.end;
        }
        self.patch(prev_end, empty)?;
        Ok(ThompsonRef {
            start: prefix.start,
            end: empty,
        })
    }
    fn c_at_least(&self, expr: &Hir, greedy: bool, n: u32) -> Result<ThompsonRef, BuildError> {
        if n == 0 {
            if expr.properties().minimum_len().map_or(false, |len| len > 0) {
                let union = if greedy {
                    self.add_union()
                } else {
                    self.add_union_reverse()
                }?;
                let compiled = self.c(expr)?;
                self.patch(union, compiled.start)?;
                self.patch(compiled.end, union)?;
                return Ok(ThompsonRef {
                    start: union,
                    end: union,
                });
            }
            let compiled = self.c(expr)?;
            let plus = if greedy {
                self.add_union()
            } else {
                self.add_union_reverse()
            }?;
            self.patch(compiled.end, plus)?;
            self.patch(plus, compiled.start)?;
            let question = if greedy {
                self.add_union()
            } else {
                self.add_union_reverse()
            }?;
            let empty = self.add_empty()?;
            self.patch(question, compiled.start)?;
            self.patch(question, empty)?;
            self.patch(plus, empty)?;
            Ok(ThompsonRef {
                start: question,
                end: empty,
            })
        } else if n == 1 {
            let compiled = self.c(expr)?;
            let union = if greedy {
                self.add_union()
            } else {
                self.add_union_reverse()
            }?;
            self.patch(compiled.end, union)?;
            self.patch(union, compiled.start)?;
            Ok(ThompsonRef {
                start: compiled.start,
                end: union,
            })
        } else {
            let prefix = self.c_exactly(expr, n - 1)?;
            let last = self.c(expr)?;
            let union = if greedy {
                self.add_union()
            } else {
                self.add_union_reverse()
            }?;
            self.patch(prefix.end, last.start)?;
            self.patch(last.end, union)?;
            self.patch(union, last.start)?;
            Ok(ThompsonRef {
                start: prefix.start,
                end: union,
            })
        }
    }
    fn c_zero_or_one(&self, expr: &Hir, greedy: bool) -> Result<ThompsonRef, BuildError> {
        let union = if greedy {
            self.add_union()
        } else {
            self.add_union_reverse()
        }?;
        let compiled = self.c(expr)?;
        let empty = self.add_empty()?;
        self.patch(union, compiled.start)?;
        self.patch(union, empty)?;
        self.patch(compiled.end, empty)?;
        Ok(ThompsonRef {
            start: union,
            end: empty,
        })
    }
    fn c_exactly(&self, expr: &Hir, n: u32) -> Result<ThompsonRef, BuildError> {
        let it = (0..n).map(|_| self.c(expr));
        self.c_concat(it)
    }
    fn c_byte_class(&self, cls: &hir::ClassBytes) -> Result<ThompsonRef, BuildError> {
        let end = self.add_empty()?;
        let mut trans = Vec::with_capacity(cls.ranges().len());
        for r in cls.iter() {
            trans.push(Transition {
                start: r.start(),
                end: r.end(),
                next: end,
            });
        }
        Ok(ThompsonRef {
            start: self.add_sparse(trans)?,
            end,
        })
    }
    fn c_unicode_class(&self, cls: &hir::ClassUnicode) -> Result<ThompsonRef, BuildError> {
        if cls.is_ascii() {
            let end = self.add_empty()?;
            let mut trans = Vec::with_capacity(cls.ranges().len());
            for r in cls.iter() {
                trans.push(Transition {
                    start: u8::try_from(u32::from(r.start())).unwrap(),
                    end: u8::try_from(u32::from(r.end())).unwrap(),
                    next: end,
                });
            }
            Ok(ThompsonRef {
                start: self.add_sparse(trans)?,
                end,
            })
        } else if self.is_reverse() {
            if !self.config.get_shrink() {
                self.c_unicode_class_reverse_with_suffix(cls)
            } else {
                let mut trie = self.trie_state.borrow_mut();
                trie.clear();
                for rng in cls.iter() {
                    for mut seq in Utf8Sequences::new(rng.start(), rng.end()) {
                        seq.reverse();
                        trie.insert(seq.as_slice());
                    }
                }
                let mut builder = self.builder.borrow_mut();
                let mut utf8_state = self.utf8_state.borrow_mut();
                let mut utf8c = Utf8Compiler::new(&mut *builder, &mut *utf8_state)?;
                trie.iter(|seq| {
                    utf8c.add(&seq)?;
                    Ok(())
                })?;
                utf8c.finish()
            }
        } else {
            let mut builder = self.builder.borrow_mut();
            let mut utf8_state = self.utf8_state.borrow_mut();
            let mut utf8c = Utf8Compiler::new(&mut *builder, &mut *utf8_state)?;
            for rng in cls.iter() {
                for seq in Utf8Sequences::new(rng.start(), rng.end()) {
                    utf8c.add(seq.as_slice())?;
                }
            }
            utf8c.finish()
        }
    }
    fn c_unicode_class_reverse_with_suffix(
        &self,
        cls: &hir::ClassUnicode,
    ) -> Result<ThompsonRef, BuildError> {
        let mut cache = self.utf8_suffix.borrow_mut();
        cache.clear();
        let union = self.add_union()?;
        let alt_end = self.add_empty()?;
        for urng in cls.iter() {
            for seq in Utf8Sequences::new(urng.start(), urng.end()) {
                let mut end = alt_end;
                for brng in seq.as_slice() {
                    let key = Utf8SuffixKey {
                        from: end,
                        start: brng.start,
                        end: brng.end,
                    };
                    let hash = cache.hash(&key);
                    if let Some(id) = cache.get(&key, hash) {
                        end = id;
                        continue;
                    }
                    let compiled = self.c_range(brng.start, brng.end)?;
                    self.patch(compiled.end, end)?;
                    end = compiled.start;
                    cache.set(key, hash, end);
                }
                self.patch(union, end)?;
            }
        }
        Ok(ThompsonRef {
            start: union,
            end: alt_end,
        })
    }
    fn c_look(&self, anchor: &hir::Look) -> Result<ThompsonRef, BuildError> {
        let look = match *anchor {
            hir::Look::Start => Look::Start,
            hir::Look::End => Look::End,
            hir::Look::StartLF => Look::StartLF,
            hir::Look::EndLF => Look::EndLF,
            hir::Look::StartCRLF => Look::StartCRLF,
            hir::Look::EndCRLF => Look::EndCRLF,
            hir::Look::WordAscii => Look::WordAscii,
            hir::Look::WordAsciiNegate => Look::WordAsciiNegate,
            hir::Look::WordUnicode => Look::WordUnicode,
            hir::Look::WordUnicodeNegate => Look::WordUnicodeNegate,
            hir::Look::WordStartAscii => Look::WordStartAscii,
            hir::Look::WordEndAscii => Look::WordEndAscii,
            hir::Look::WordStartUnicode => Look::WordStartUnicode,
            hir::Look::WordEndUnicode => Look::WordEndUnicode,
            hir::Look::WordStartHalfAscii => Look::WordStartHalfAscii,
            hir::Look::WordEndHalfAscii => Look::WordEndHalfAscii,
            hir::Look::WordStartHalfUnicode => Look::WordStartHalfUnicode,
            hir::Look::WordEndHalfUnicode => Look::WordEndHalfUnicode,
        };
        let id = self.add_look(look)?;
        Ok(ThompsonRef { start: id, end: id })
    }
    fn c_literal(&self, bytes: &[u8]) -> Result<ThompsonRef, BuildError> {
        self.c_concat(bytes.iter().copied().map(|b| self.c_range(b, b)))
    }
    fn c_range(&self, start: u8, end: u8) -> Result<ThompsonRef, BuildError> {
        let id = self.add_range(start, end)?;
        Ok(ThompsonRef { start: id, end: id })
    }
    fn c_empty(&self) -> Result<ThompsonRef, BuildError> {
        let id = self.add_empty()?;
        Ok(ThompsonRef { start: id, end: id })
    }
    fn c_fail(&self) -> Result<ThompsonRef, BuildError> {
        let id = self.add_fail()?;
        Ok(ThompsonRef { start: id, end: id })
    }
    fn patch(&self, from: StateID, to: StateID) -> Result<(), BuildError> {
        self.builder.borrow_mut().patch(from, to)
    }
    fn start_pattern(&self) -> Result<PatternID, BuildError> {
        self.builder.borrow_mut().start_pattern()
    }
    fn finish_pattern(&self, start_id: StateID) -> Result<PatternID, BuildError> {
        self.builder.borrow_mut().finish_pattern(start_id)
    }
    fn add_empty(&self) -> Result<StateID, BuildError> {
        self.builder.borrow_mut().add_empty()
    }
    fn add_range(&self, start: u8, end: u8) -> Result<StateID, BuildError> {
        self.builder.borrow_mut().add_range(Transition {
            start,
            end,
            next: StateID::ZERO,
        })
    }
    fn add_sparse(&self, ranges: Vec<Transition>) -> Result<StateID, BuildError> {
        self.builder.borrow_mut().add_sparse(ranges)
    }
    fn add_look(&self, mut look: Look) -> Result<StateID, BuildError> {
        if self.is_reverse() {
            look = look.reversed();
        }
        self.builder.borrow_mut().add_look(StateID::ZERO, look)
    }
    fn add_union(&self) -> Result<StateID, BuildError> {
        self.builder.borrow_mut().add_union(vec![])
    }
    fn add_union_reverse(&self) -> Result<StateID, BuildError> {
        self.builder.borrow_mut().add_union_reverse(vec![])
    }
    fn add_capture_start(
        &self,
        capture_index: u32,
        name: Option<&str>,
    ) -> Result<StateID, BuildError> {
        let name = name.map(Arc::from);
        self.builder
            .borrow_mut()
            .add_capture_start(StateID::ZERO, capture_index, name)
    }
    fn add_capture_end(&self, capture_index: u32) -> Result<StateID, BuildError> {
        self.builder
            .borrow_mut()
            .add_capture_end(StateID::ZERO, capture_index)
    }
    fn add_fail(&self) -> Result<StateID, BuildError> {
        self.builder.borrow_mut().add_fail()
    }
    fn add_match(&self) -> Result<StateID, BuildError> {
        self.builder.borrow_mut().add_match()
    }
    fn is_reverse(&self) -> bool {
        self.config.get_reverse()
    }
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct ThompsonRef {
    pub(crate) start: StateID,
    pub(crate) end: StateID,
}
#[derive(Debug)]
struct Utf8Compiler<'a> {
    builder: &'a mut Builder,
    state: &'a mut Utf8State,
    target: StateID,
}
#[derive(Clone, Debug)]
struct Utf8State {
    compiled: Utf8BoundedMap,
    uncompiled: Vec<Utf8Node>,
}
#[derive(Clone, Debug)]
struct Utf8Node {
    trans: Vec<Transition>,
    last: Option<Utf8LastTransition>,
}
#[derive(Clone, Debug)]
struct Utf8LastTransition {
    start: u8,
    end: u8,
}
impl Utf8State {
    fn new() -> Utf8State {
        Utf8State {
            compiled: Utf8BoundedMap::new(10_000),
            uncompiled: vec![],
        }
    }
    fn clear(&mut self) {
        self.compiled.clear();
        self.uncompiled.clear();
    }
}
impl<'a> Utf8Compiler<'a> {
    fn new(
        builder: &'a mut Builder,
        state: &'a mut Utf8State,
    ) -> Result<Utf8Compiler<'a>, BuildError> {
        let target = builder.add_empty()?;
        state.clear();
        let mut utf8c = Utf8Compiler {
            builder,
            state,
            target,
        };
        utf8c.add_empty();
        Ok(utf8c)
    }
    fn finish(&mut self) -> Result<ThompsonRef, BuildError> {
        self.compile_from(0)?;
        let node = self.pop_root();
        let start = self.compile(node)?;
        Ok(ThompsonRef {
            start,
            end: self.target,
        })
    }
    fn add(&mut self, ranges: &[Utf8Range]) -> Result<(), BuildError> {
        let prefix_len = ranges
            .iter()
            .zip(&self.state.uncompiled)
            .take_while(|&(range, node)| {
                node.last
                    .as_ref()
                    .map_or(false, |t| (t.start, t.end) == (range.start, range.end))
            })
            .count();
        assert!(prefix_len < ranges.len());
        self.compile_from(prefix_len)?;
        self.add_suffix(&ranges[prefix_len..]);
        Ok(())
    }
    fn compile_from(&mut self, from: usize) -> Result<(), BuildError> {
        let mut next = self.target;
        while from + 1 < self.state.uncompiled.len() {
            let node = self.pop_freeze(next);
            next = self.compile(node)?;
        }
        self.top_last_freeze(next);
        Ok(())
    }
    fn compile(&mut self, node: Vec<Transition>) -> Result<StateID, BuildError> {
        let hash = self.state.compiled.hash(&node);
        if let Some(id) = self.state.compiled.get(&node, hash) {
            return Ok(id);
        }
        let id = self.builder.add_sparse(node.clone())?;
        self.state.compiled.set(node, hash, id);
        Ok(id)
    }
    fn add_suffix(&mut self, ranges: &[Utf8Range]) {
        assert!(!ranges.is_empty());
        let last = self
            .state
            .uncompiled
            .len()
            .checked_sub(1)
            .expect("non-empty nodes");
        assert!(self.state.uncompiled[last].last.is_none());
        self.state.uncompiled[last].last = Some(Utf8LastTransition {
            start: ranges[0].start,
            end: ranges[0].end,
        });
        for r in &ranges[1..] {
            self.state.uncompiled.push(Utf8Node {
                trans: vec![],
                last: Some(Utf8LastTransition {
                    start: r.start,
                    end: r.end,
                }),
            });
        }
    }
    fn add_empty(&mut self) {
        self.state.uncompiled.push(Utf8Node {
            trans: vec![],
            last: None,
        });
    }
    fn pop_freeze(&mut self, next: StateID) -> Vec<Transition> {
        let mut uncompiled = self.state.uncompiled.pop().unwrap();
        uncompiled.set_last_transition(next);
        uncompiled.trans
    }
    fn pop_root(&mut self) -> Vec<Transition> {
        assert_eq!(self.state.uncompiled.len(), 1);
        assert!(self.state.uncompiled[0].last.is_none());
        self.state.uncompiled.pop().expect("non-empty nodes").trans
    }
    fn top_last_freeze(&mut self, next: StateID) {
        let last = self
            .state
            .uncompiled
            .len()
            .checked_sub(1)
            .expect("non-empty nodes");
        self.state.uncompiled[last].set_last_transition(next);
    }
}
impl Utf8Node {
    fn set_last_transition(&mut self, next: StateID) {
        if let Some(last) = self.last.take() {
            self.trans.push(Transition {
                start: last.start,
                end: last.end,
                next,
            });
        }
    }
}
