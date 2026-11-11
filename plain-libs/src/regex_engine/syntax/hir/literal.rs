use crate::regex_engine::syntax::hir::{self, Hir};
use core::{cmp, mem, num::NonZeroUsize};
use std::{vec, vec::Vec};
#[derive(Clone, Debug)]
pub struct Extractor {
    kind: ExtractKind,
    limit_class: usize,
    limit_repeat: usize,
    limit_literal_len: usize,
    limit_total: usize,
}
impl Extractor {
    pub fn new() -> Extractor {
        Extractor {
            kind: ExtractKind::Prefix,
            limit_class: 10,
            limit_repeat: 10,
            limit_literal_len: 100,
            limit_total: 250,
        }
    }
    pub fn extract(&self, hir: &Hir) -> Seq {
        use crate::regex_engine::syntax::hir::HirKind::*;
        match *hir.kind() {
            Empty | Look(_) => Seq::singleton(self::Literal::exact(vec![])),
            Literal(hir::Literal(ref bytes)) => {
                let mut seq = Seq::singleton(self::Literal::exact(bytes.to_vec()));
                self.enforce_literal_len(&mut seq);
                seq
            }
            Class(hir::Class::Unicode(ref cls)) => self.extract_class_unicode(cls),
            Class(hir::Class::Bytes(ref cls)) => self.extract_class_bytes(cls),
            Repetition(ref rep) => self.extract_repetition(rep),
            Capture(hir::Capture { ref sub, .. }) => self.extract(sub),
            Concat(ref hirs) => match self.kind {
                ExtractKind::Prefix => self.extract_concat(hirs.iter()),
                ExtractKind::Suffix => self.extract_concat(hirs.iter().rev()),
            },
            Alternation(ref hirs) => self.extract_alternation(hirs.iter()),
        }
    }
    pub fn kind(&mut self, kind: ExtractKind) -> &mut Extractor {
        self.kind = kind;
        self
    }
    pub fn limit_class(&mut self, limit: usize) -> &mut Extractor {
        self.limit_class = limit;
        self
    }
    pub fn limit_repeat(&mut self, limit: usize) -> &mut Extractor {
        self.limit_repeat = limit;
        self
    }
    pub fn limit_literal_len(&mut self, limit: usize) -> &mut Extractor {
        self.limit_literal_len = limit;
        self
    }
    pub fn limit_total(&mut self, limit: usize) -> &mut Extractor {
        self.limit_total = limit;
        self
    }
    fn extract_concat<'a, I: Iterator<Item = &'a Hir>>(&self, it: I) -> Seq {
        let mut seq = Seq::singleton(self::Literal::exact(vec![]));
        for hir in it {
            if seq.is_inexact() {
                break;
            }
            seq = self.cross(seq, &mut self.extract(hir));
        }
        seq
    }
    fn extract_alternation<'a, I: Iterator<Item = &'a Hir>>(&self, it: I) -> Seq {
        let mut seq = Seq::empty();
        for hir in it {
            if !seq.is_finite() {
                break;
            }
            seq = self.union(seq, &mut self.extract(hir));
        }
        seq
    }
    fn extract_repetition(&self, rep: &hir::Repetition) -> Seq {
        let mut subseq = self.extract(&rep.sub);
        match *rep {
            hir::Repetition {
                min: 0,
                max,
                greedy,
                ..
            } => {
                if max != Some(1) {
                    subseq.make_inexact();
                }
                let mut empty = Seq::singleton(Literal::exact(vec![]));
                if !greedy {
                    mem::swap(&mut subseq, &mut empty);
                }
                self.union(subseq, &mut empty)
            }
            hir::Repetition {
                min,
                max: Some(max),
                ..
            } if min == max => {
                assert!(min > 0);
                let limit = u32::try_from(self.limit_repeat).unwrap_or(u32::MAX);
                let mut seq = Seq::singleton(Literal::exact(vec![]));
                for _ in 0..cmp::min(min, limit) {
                    if seq.is_inexact() {
                        break;
                    }
                    seq = self.cross(seq, &mut subseq.clone());
                }
                if usize::try_from(min).is_err() || min > limit {
                    seq.make_inexact();
                }
                seq
            }
            hir::Repetition { min, .. } => {
                assert!(min > 0);
                let limit = u32::try_from(self.limit_repeat).unwrap_or(u32::MAX);
                let mut seq = Seq::singleton(Literal::exact(vec![]));
                for _ in 0..cmp::min(min, limit) {
                    if seq.is_inexact() {
                        break;
                    }
                    seq = self.cross(seq, &mut subseq.clone());
                }
                seq.make_inexact();
                seq
            }
        }
    }
    fn extract_class_unicode(&self, cls: &hir::ClassUnicode) -> Seq {
        if self.class_over_limit_unicode(cls) {
            return Seq::infinite();
        }
        let mut seq = Seq::empty();
        for r in cls.iter() {
            for ch in r.start()..=r.end() {
                seq.push(Literal::from(ch));
            }
        }
        self.enforce_literal_len(&mut seq);
        seq
    }
    fn extract_class_bytes(&self, cls: &hir::ClassBytes) -> Seq {
        if self.class_over_limit_bytes(cls) {
            return Seq::infinite();
        }
        let mut seq = Seq::empty();
        for r in cls.iter() {
            for b in r.start()..=r.end() {
                seq.push(Literal::from(b));
            }
        }
        self.enforce_literal_len(&mut seq);
        seq
    }
    fn class_over_limit_unicode(&self, cls: &hir::ClassUnicode) -> bool {
        let mut count = 0;
        for r in cls.iter() {
            if count > self.limit_class {
                return true;
            }
            count += r.len();
        }
        count > self.limit_class
    }
    fn class_over_limit_bytes(&self, cls: &hir::ClassBytes) -> bool {
        let mut count = 0;
        for r in cls.iter() {
            if count > self.limit_class {
                return true;
            }
            count += r.len();
        }
        count > self.limit_class
    }
    fn cross(&self, mut seq1: Seq, seq2: &mut Seq) -> Seq {
        if seq1
            .max_cross_len(seq2)
            .map_or(false, |len| len > self.limit_total)
        {
            seq2.make_infinite();
        }
        if let ExtractKind::Suffix = self.kind {
            seq1.cross_reverse(seq2);
        } else {
            seq1.cross_forward(seq2);
        }
        assert!(seq1.len().map_or(true, |x| x <= self.limit_total));
        self.enforce_literal_len(&mut seq1);
        seq1
    }
    fn union(&self, mut seq1: Seq, seq2: &mut Seq) -> Seq {
        if seq1
            .max_union_len(seq2)
            .map_or(false, |len| len > self.limit_total)
        {
            match self.kind {
                ExtractKind::Prefix => {
                    seq1.keep_first_bytes(4);
                    seq2.keep_first_bytes(4);
                }
                ExtractKind::Suffix => {
                    seq1.keep_last_bytes(4);
                    seq2.keep_last_bytes(4);
                }
            }
            seq1.dedup();
            seq2.dedup();
            if seq1
                .max_union_len(seq2)
                .map_or(false, |len| len > self.limit_total)
            {
                seq2.make_infinite();
            }
        }
        seq1.union(seq2);
        assert!(seq1.len().map_or(true, |x| x <= self.limit_total));
        seq1
    }
    fn enforce_literal_len(&self, seq: &mut Seq) {
        let len = self.limit_literal_len;
        match self.kind {
            ExtractKind::Prefix => seq.keep_first_bytes(len),
            ExtractKind::Suffix => seq.keep_last_bytes(len),
        }
    }
}
impl Default for Extractor {
    fn default() -> Extractor {
        Extractor::new()
    }
}
#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum ExtractKind {
    Prefix,
    Suffix,
}
impl ExtractKind {
    pub fn is_prefix(&self) -> bool {
        matches!(*self, ExtractKind::Prefix)
    }
}
impl Default for ExtractKind {
    fn default() -> ExtractKind {
        ExtractKind::Prefix
    }
}
#[derive(Clone, Eq, PartialEq)]
pub struct Seq {
    literals: Option<Vec<Literal>>,
}
impl Seq {
    #[inline]
    pub fn empty() -> Seq {
        Seq {
            literals: Some(vec![]),
        }
    }
    #[inline]
    pub fn infinite() -> Seq {
        Seq { literals: None }
    }
    #[inline]
    pub fn singleton(lit: Literal) -> Seq {
        Seq {
            literals: Some(vec![lit]),
        }
    }
    #[inline]
    pub fn new<I, B>(it: I) -> Seq
    where
        I: IntoIterator<Item = B>,
        B: AsRef<[u8]>,
    {
        it.into_iter().map(|b| Literal::exact(b.as_ref())).collect()
    }
    #[inline]
    pub fn literals(&self) -> Option<&[Literal]> {
        self.literals.as_deref()
    }
    #[inline]
    pub fn push(&mut self, lit: Literal) {
        let lits = match self.literals {
            None => return,
            Some(ref mut lits) => lits,
        };
        if lits.last().map_or(false, |m| m == &lit) {
            return;
        }
        lits.push(lit);
    }
    #[inline]
    pub fn make_inexact(&mut self) {
        let lits = match self.literals {
            None => return,
            Some(ref mut lits) => lits,
        };
        for lit in lits.iter_mut() {
            lit.make_inexact();
        }
    }
    #[inline]
    pub fn make_infinite(&mut self) {
        self.literals = None;
    }
    #[inline]
    pub fn cross_forward(&mut self, other: &mut Seq) {
        let (lits1, lits2) = match self.cross_preamble(other) {
            None => return,
            Some((lits1, lits2)) => (lits1, lits2),
        };
        let newcap = lits1.len().saturating_mul(lits2.len());
        for selflit in mem::replace(lits1, Vec::with_capacity(newcap)) {
            if !selflit.is_exact() {
                lits1.push(selflit);
                continue;
            }
            for otherlit in lits2.iter() {
                let mut newlit = Literal::exact(Vec::with_capacity(selflit.len() + otherlit.len()));
                newlit.extend(&selflit);
                newlit.extend(&otherlit);
                if !otherlit.is_exact() {
                    newlit.make_inexact();
                }
                lits1.push(newlit);
            }
        }
        lits2.drain(..);
        self.dedup();
    }
    #[inline]
    pub fn cross_reverse(&mut self, other: &mut Seq) {
        let (lits1, lits2) = match self.cross_preamble(other) {
            None => return,
            Some((lits1, lits2)) => (lits1, lits2),
        };
        let newcap = lits1.len().saturating_mul(lits2.len());
        let selflits = mem::replace(lits1, Vec::with_capacity(newcap));
        for (i, otherlit) in lits2.drain(..).enumerate() {
            for selflit in selflits.iter() {
                if !selflit.is_exact() {
                    if i == 0 {
                        lits1.push(selflit.clone());
                    }
                    continue;
                }
                let mut newlit = Literal::exact(Vec::with_capacity(otherlit.len() + selflit.len()));
                newlit.extend(&otherlit);
                newlit.extend(&selflit);
                if !otherlit.is_exact() {
                    newlit.make_inexact();
                }
                lits1.push(newlit);
            }
        }
        self.dedup();
    }
    fn cross_preamble<'a>(
        &'a mut self,
        other: &'a mut Seq,
    ) -> Option<(&'a mut Vec<Literal>, &'a mut Vec<Literal>)> {
        let lits2 = match other.literals {
            None => {
                if self.min_literal_len() == Some(0) {
                    *self = Seq::infinite();
                } else {
                    self.make_inexact();
                }
                return None;
            }
            Some(ref mut lits) => lits,
        };
        let lits1 = match self.literals {
            None => {
                lits2.drain(..);
                return None;
            }
            Some(ref mut lits) => lits,
        };
        Some((lits1, lits2))
    }
    #[inline]
    pub fn union(&mut self, other: &mut Seq) {
        let lits2 = match other.literals {
            None => {
                self.make_infinite();
                return;
            }
            Some(ref mut lits) => lits.drain(..),
        };
        let lits1 = match self.literals {
            None => return,
            Some(ref mut lits) => lits,
        };
        lits1.extend(lits2);
        self.dedup();
    }

    #[inline]
    pub fn dedup(&mut self) {
        if let Some(ref mut lits) = self.literals {
            lits.dedup_by(|lit1, lit2| {
                if lit1.as_bytes() != lit2.as_bytes() {
                    return false;
                }
                if lit1.is_exact() != lit2.is_exact() {
                    lit1.make_inexact();
                    lit2.make_inexact();
                }
                true
            });
        }
    }
    #[inline]
    pub fn sort(&mut self) {
        if let Some(ref mut lits) = self.literals {
            lits.sort();
        }
    }

    #[inline]
    pub fn keep_first_bytes(&mut self, len: usize) {
        if let Some(ref mut lits) = self.literals {
            for m in lits.iter_mut() {
                m.keep_first_bytes(len);
            }
        }
    }
    #[inline]
    pub fn keep_last_bytes(&mut self, len: usize) {
        if let Some(ref mut lits) = self.literals {
            for m in lits.iter_mut() {
                m.keep_last_bytes(len);
            }
        }
    }
    #[inline]
    pub fn is_finite(&self) -> bool {
        self.literals.is_some()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == Some(0)
    }
    #[inline]
    pub fn len(&self) -> Option<usize> {
        self.literals.as_ref().map(|lits| lits.len())
    }
    #[inline]
    pub fn is_exact(&self) -> bool {
        self.literals()
            .map_or(false, |lits| lits.iter().all(|x| x.is_exact()))
    }
    #[inline]
    pub fn is_inexact(&self) -> bool {
        self.literals()
            .map_or(true, |lits| lits.iter().all(|x| !x.is_exact()))
    }
    #[inline]
    pub fn max_union_len(&self, other: &Seq) -> Option<usize> {
        let len1 = self.len()?;
        let len2 = other.len()?;
        Some(len1.saturating_add(len2))
    }
    #[inline]
    pub fn max_cross_len(&self, other: &Seq) -> Option<usize> {
        let len1 = self.len()?;
        let len2 = other.len()?;
        Some(len1.saturating_mul(len2))
    }
    #[inline]
    pub fn min_literal_len(&self) -> Option<usize> {
        self.literals.as_ref()?.iter().map(|x| x.len()).min()
    }

    #[inline]
    pub fn longest_common_prefix(&self) -> Option<&[u8]> {
        let lits = match self.literals {
            None => return None,
            Some(ref lits) => lits,
        };
        if lits.len() == 0 {
            return None;
        }
        let base = lits[0].as_bytes();
        let mut len = base.len();
        for m in lits.iter().skip(1) {
            len = m
                .as_bytes()
                .iter()
                .zip(base[..len].iter())
                .take_while(|&(a, b)| a == b)
                .count();
            if len == 0 {
                return Some(&[]);
            }
        }
        Some(&base[..len])
    }
    #[inline]
    pub fn longest_common_suffix(&self) -> Option<&[u8]> {
        let lits = match self.literals {
            None => return None,
            Some(ref lits) => lits,
        };
        if lits.len() == 0 {
            return None;
        }
        let base = lits[0].as_bytes();
        let mut len = base.len();
        for m in lits.iter().skip(1) {
            len = m
                .as_bytes()
                .iter()
                .rev()
                .zip(base[base.len() - len..].iter().rev())
                .take_while(|&(a, b)| a == b)
                .count();
            if len == 0 {
                return Some(&[]);
            }
        }
        Some(&base[base.len() - len..])
    }
    #[inline]
    pub fn optimize_for_prefix_by_preference(&mut self) {
        self.optimize_by_preference(true);
    }
    #[inline]
    pub fn optimize_for_suffix_by_preference(&mut self) {
        self.optimize_by_preference(false);
    }
    fn optimize_by_preference(&mut self, prefix: bool) {
        let origlen = match self.len() {
            None => return,
            Some(len) => len,
        };
        if self.min_literal_len().map_or(false, |len| len == 0) {
            self.make_infinite();
            return;
        }
        if prefix {
            if let Some(ref mut lits) = self.literals {
                PreferenceTrie::minimize(lits, true);
            }
        }
        let fix = if prefix {
            self.longest_common_prefix()
        } else {
            self.longest_common_suffix()
        };
        if let Some(fix) = fix {
            if prefix && origlen > 1 && fix.len() >= 1 && fix.len() <= 3 && rank(fix[0]) < 200 {
                self.keep_first_bytes(1);
                self.dedup();
                return;
            }
            let isfast = self.is_exact() && self.len().map_or(false, |len| len <= 16);
            let usefix = fix.len() > 4 || (fix.len() > 1 && !isfast);
            if usefix {
                if prefix {
                    self.keep_first_bytes(fix.len());
                } else {
                    self.keep_last_bytes(fix.len());
                }
                self.dedup();
                assert_eq!(Some(1), self.len());
            }
        }
        let exact: Option<Seq> = if self.is_exact() {
            Some(self.clone())
        } else {
            None
        };
        const ATTEMPTS: [(usize, usize); 5] = [(5, 10), (4, 10), (3, 64), (2, 64), (1, 10)];
        for (keep, limit) in ATTEMPTS {
            let len = match self.len() {
                None => break,
                Some(len) => len,
            };
            if len <= limit {
                break;
            }
            if prefix {
                self.keep_first_bytes(keep);
            } else {
                self.keep_last_bytes(keep);
            }
            if prefix {
                if let Some(ref mut lits) = self.literals {
                    PreferenceTrie::minimize(lits, true);
                }
            }
        }
        if let Some(lits) = self.literals() {
            if lits.iter().any(|lit| lit.is_poisonous()) {
                self.make_infinite();
            }
        }
        if let Some(exact) = exact {
            if !self.is_finite() {
                *self = exact;
                return;
            }
            if self.min_literal_len().map_or(true, |len| len <= 2) {
                *self = exact;
                return;
            }
            if self.len().map_or(true, |len| len > 64) {
                *self = exact;
                return;
            }
        }
    }
}
impl core::fmt::Debug for Seq {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "Seq")?;
        if let Some(lits) = self.literals() {
            f.debug_list().entries(lits.iter()).finish()
        } else {
            write!(f, "[∞]")
        }
    }
}
impl FromIterator<Literal> for Seq {
    fn from_iter<T: IntoIterator<Item = Literal>>(it: T) -> Seq {
        let mut seq = Seq::empty();
        for literal in it {
            seq.push(literal);
        }
        seq
    }
}
#[derive(Clone, Eq, PartialEq, PartialOrd, Ord)]
pub struct Literal {
    bytes: Vec<u8>,
    exact: bool,
}
impl Literal {
    #[inline]
    pub fn exact<B: Into<Vec<u8>>>(bytes: B) -> Literal {
        Literal {
            bytes: bytes.into(),
            exact: true,
        }
    }
    #[inline]
    pub fn inexact<B: Into<Vec<u8>>>(bytes: B) -> Literal {
        Literal {
            bytes: bytes.into(),
            exact: false,
        }
    }
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
    #[inline]
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
    #[inline]
    pub fn len(&self) -> usize {
        self.as_bytes().len()
    }
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
    #[inline]
    pub fn is_exact(&self) -> bool {
        self.exact
    }
    #[inline]
    pub fn make_inexact(&mut self) {
        self.exact = false;
    }
    #[inline]
    pub fn reverse(&mut self) {
        self.bytes.reverse();
    }
    #[inline]
    pub fn extend(&mut self, lit: &Literal) {
        if !self.is_exact() {
            return;
        }
        self.bytes.extend_from_slice(&lit.bytes);
    }
    #[inline]
    pub fn keep_first_bytes(&mut self, len: usize) {
        if len >= self.len() {
            return;
        }
        self.make_inexact();
        self.bytes.truncate(len);
    }
    #[inline]
    pub fn keep_last_bytes(&mut self, len: usize) {
        if len >= self.len() {
            return;
        }
        self.make_inexact();
        self.bytes.drain(..self.len() - len);
    }
    fn is_poisonous(&self) -> bool {
        self.is_empty() || (self.len() == 1 && rank(self.as_bytes()[0]) >= 250)
    }
}
impl From<u8> for Literal {
    fn from(byte: u8) -> Literal {
        Literal::exact(vec![byte])
    }
}
impl From<char> for Literal {
    fn from(ch: char) -> Literal {
        use std::string::ToString;
        Literal::exact(ch.encode_utf8(&mut [0; 4]).to_string())
    }
}
impl AsRef<[u8]> for Literal {
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}
impl core::fmt::Debug for Literal {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let tag = if self.exact { "E" } else { "I" };
        f.debug_tuple(tag)
            .field(&crate::regex_engine::syntax::debug::Bytes(self.as_bytes()))
            .finish()
    }
}
#[derive(Debug)]
struct PreferenceTrie {
    states: Vec<State>,
    matches: Vec<Option<NonZeroUsize>>,
    next_literal_index: usize,
}
#[derive(Debug, Default)]
struct State {
    trans: Vec<(u8, usize)>,
}
impl PreferenceTrie {
    fn minimize(literals: &mut Vec<Literal>, keep_exact: bool) {
        let mut trie = PreferenceTrie {
            states: vec![],
            matches: vec![],
            next_literal_index: 1,
        };
        let mut make_inexact = vec![];
        literals.retain_mut(|lit| match trie.insert(lit.as_bytes()) {
            Ok(_) => true,
            Err(i) => {
                if !keep_exact {
                    make_inexact.push(i.checked_sub(1).unwrap());
                }
                false
            }
        });
        for i in make_inexact {
            literals[i].make_inexact();
        }
    }
    fn insert(&mut self, bytes: &[u8]) -> Result<usize, usize> {
        let mut prev = self.root();
        if let Some(idx) = self.matches[prev] {
            return Err(idx.get());
        }
        for &b in bytes.iter() {
            match self.states[prev].trans.binary_search_by_key(&b, |t| t.0) {
                Ok(i) => {
                    prev = self.states[prev].trans[i].1;
                    if let Some(idx) = self.matches[prev] {
                        return Err(idx.get());
                    }
                }
                Err(i) => {
                    let next = self.create_state();
                    self.states[prev].trans.insert(i, (b, next));
                    prev = next;
                }
            }
        }
        let idx = self.next_literal_index;
        self.next_literal_index += 1;
        self.matches[prev] = NonZeroUsize::new(idx);
        Ok(idx)
    }
    fn root(&mut self) -> usize {
        if !self.states.is_empty() {
            0
        } else {
            self.create_state()
        }
    }
    fn create_state(&mut self) -> usize {
        let id = self.states.len();
        self.states.push(State::default());
        self.matches.push(None);
        id
    }
}
pub fn rank(byte: u8) -> u8 {
    crate::regex_engine::syntax::rank::BYTE_FREQUENCIES[usize::from(byte)]
}
