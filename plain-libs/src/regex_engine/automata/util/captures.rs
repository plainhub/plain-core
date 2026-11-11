use crate::regex_engine::automata::util::{
    interpolate,
    primitives::{NonMaxUsize, PatternID, PatternIDError, PatternIDIter, SmallIndex},
    search::{Match, Span},
};
use std::{string::String, sync::Arc, vec, vec::Vec};
#[derive(Clone)]
pub struct Captures {
    group_info: GroupInfo,
    pid: Option<PatternID>,
    slots: Vec<Option<NonMaxUsize>>,
}
impl Captures {
    pub fn all(group_info: GroupInfo) -> Captures {
        let slots = group_info.slot_len();
        Captures {
            group_info,
            pid: None,
            slots: vec![None; slots],
        }
    }
    pub fn matches(group_info: GroupInfo) -> Captures {
        let slots = group_info.pattern_len().checked_mul(2).unwrap();
        Captures {
            group_info,
            pid: None,
            slots: vec![None; slots],
        }
    }
    pub fn empty(group_info: GroupInfo) -> Captures {
        Captures {
            group_info,
            pid: None,
            slots: vec![],
        }
    }
    #[inline]
    pub fn is_match(&self) -> bool {
        self.pid.is_some()
    }
    #[inline]
    pub fn pattern(&self) -> Option<PatternID> {
        self.pid
    }
    #[inline]
    pub fn get_match(&self) -> Option<Match> {
        Some(Match::new(self.pattern()?, self.get_group(0)?))
    }
    #[inline]
    pub fn get_group(&self, index: usize) -> Option<Span> {
        let pid = self.pattern()?;
        let (slot_start, slot_end) = if self.group_info().pattern_len() == 1 {
            (index.checked_mul(2)?, index.checked_mul(2)?.checked_add(1)?)
        } else {
            self.group_info().slots(pid, index)?
        };
        let start = self.slots.get(slot_start).copied()??;
        let end = self.slots.get(slot_end).copied()??;
        Some(Span {
            start: start.get(),
            end: end.get(),
        })
    }
    pub fn get_group_by_name(&self, name: &str) -> Option<Span> {
        let index = self.group_info().to_index(self.pattern()?, name)?;
        self.get_group(index)
    }
    pub fn iter(&self) -> CapturesPatternIter<'_> {
        let names = self
            .pattern()
            .map_or(GroupInfoPatternNames::empty().enumerate(), |pid| {
                self.group_info().pattern_names(pid).enumerate()
            });
        CapturesPatternIter { caps: self, names }
    }
    pub fn group_len(&self) -> usize {
        let pid = match self.pattern() {
            None => return 0,
            Some(pid) => pid,
        };
        self.group_info().group_len(pid)
    }
    pub fn group_info(&self) -> &GroupInfo {
        &self.group_info
    }

    pub fn interpolate_string_into(&self, haystack: &str, replacement: &str, dst: &mut String) {
        interpolate::string(
            replacement,
            |index, dst| {
                let span = match self.get_group(index) {
                    None => return,
                    Some(span) => span,
                };
                dst.push_str(&haystack[span]);
            },
            |name| self.group_info().to_index(self.pattern()?, name),
            dst,
        );
    }

    pub fn interpolate_bytes_into(&self, haystack: &[u8], replacement: &[u8], dst: &mut Vec<u8>) {
        interpolate::bytes(
            replacement,
            |index, dst| {
                let span = match self.get_group(index) {
                    None => return,
                    Some(span) => span,
                };
                dst.extend_from_slice(&haystack[span]);
            },
            |name| self.group_info().to_index(self.pattern()?, name),
            dst,
        );
    }
    pub fn extract<'h, const N: usize>(&self, haystack: &'h str) -> (&'h str, [&'h str; N]) {
        let mut matched = self.iter().flatten();
        let whole_match = &haystack[matched.next().expect("a match")];
        let group_matches = [0; N].map(|_| {
            let sp = matched.next().expect("too few matching groups");
            &haystack[sp]
        });
        (whole_match, group_matches)
    }
    pub fn extract_bytes<'h, const N: usize>(
        &self,
        haystack: &'h [u8],
    ) -> (&'h [u8], [&'h [u8]; N]) {
        let mut matched = self.iter().flatten();
        let whole_match = &haystack[matched.next().expect("a match")];
        let group_matches = [0; N].map(|_| {
            let sp = matched.next().expect("too few matching groups");
            &haystack[sp]
        });
        (whole_match, group_matches)
    }
}
impl Captures {
    #[inline]
    pub fn clear(&mut self) {
        self.pid = None;
        for slot in self.slots.iter_mut() {
            *slot = None;
        }
    }
    #[inline]
    pub fn set_pattern(&mut self, pid: Option<PatternID>) {
        self.pid = pid;
    }
    #[inline]
    pub fn slots(&self) -> &[Option<NonMaxUsize>] {
        &self.slots
    }
    #[inline]
    pub fn slots_mut(&mut self) -> &mut [Option<NonMaxUsize>] {
        &mut self.slots
    }
}
impl core::fmt::Debug for Captures {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        let mut dstruct = f.debug_struct("Captures");
        dstruct.field("pid", &self.pid);
        if let Some(pid) = self.pid {
            dstruct.field("spans", &CapturesDebugMap { pid, caps: self });
        }
        dstruct.finish()
    }
}
struct CapturesDebugMap<'a> {
    pid: PatternID,
    caps: &'a Captures,
}
impl<'a> core::fmt::Debug for CapturesDebugMap<'a> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        struct Key<'a>(usize, Option<&'a str>);
        impl<'a> core::fmt::Debug for Key<'a> {
            fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
                write!(f, "{}", self.0)?;
                if let Some(name) = self.1 {
                    write!(f, "/{name:?}")?;
                }
                Ok(())
            }
        }
        let mut map = f.debug_map();
        let names = self.caps.group_info().pattern_names(self.pid);
        for (group_index, maybe_name) in names.enumerate() {
            let key = Key(group_index, maybe_name);
            match self.caps.get_group(group_index) {
                None => map.entry(&key, &None::<()>),
                Some(span) => map.entry(&key, &span),
            };
        }
        map.finish()
    }
}
#[derive(Clone, Debug)]
pub struct CapturesPatternIter<'a> {
    caps: &'a Captures,
    names: core::iter::Enumerate<GroupInfoPatternNames<'a>>,
}
impl<'a> Iterator for CapturesPatternIter<'a> {
    type Item = Option<Span>;
    fn next(&mut self) -> Option<Option<Span>> {
        let (group_index, _) = self.names.next()?;
        Some(self.caps.get_group(group_index))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.names.size_hint()
    }
    fn count(self) -> usize {
        self.names.count()
    }
}
impl<'a> ExactSizeIterator for CapturesPatternIter<'a> {}
impl<'a> core::iter::FusedIterator for CapturesPatternIter<'a> {}
#[derive(Clone, Debug, Default)]
pub struct GroupInfo(Arc<GroupInfoInner>);
impl GroupInfo {
    pub fn new<P, G, N>(pattern_groups: P) -> Result<GroupInfo, GroupInfoError>
    where
        P: IntoIterator<Item = G>,
        G: IntoIterator<Item = Option<N>>,
        N: AsRef<str>,
    {
        let mut group_info = GroupInfoInner {
            slot_ranges: vec![],
            name_to_index: vec![],
            index_to_name: vec![],
            memory_extra: 0,
        };
        for (pattern_index, groups) in pattern_groups.into_iter().enumerate() {
            let pid = PatternID::new(pattern_index).map_err(GroupInfoError::too_many_patterns)?;
            let mut groups_iter = groups.into_iter().enumerate();
            match groups_iter.next() {
                None => return Err(GroupInfoError::missing_groups(pid)),
                Some((_, Some(_))) => return Err(GroupInfoError::first_must_be_unnamed(pid)),
                Some((_, None)) => {}
            }
            group_info.add_first_group(pid);
            for (group_index, maybe_name) in groups_iter {
                let group = SmallIndex::new(group_index)
                    .map_err(|_| GroupInfoError::too_many_groups(pid, group_index))?;
                group_info.add_explicit_group(pid, group, maybe_name)?;
            }
        }
        group_info.fixup_slot_ranges()?;
        group_info.slot_ranges.shrink_to_fit();
        group_info.name_to_index.shrink_to_fit();
        group_info.index_to_name.shrink_to_fit();
        Ok(GroupInfo(Arc::new(group_info)))
    }
    pub fn empty() -> GroupInfo {
        GroupInfo::new(core::iter::empty::<[Option<&str>; 0]>())
            .expect("empty group info is always valid")
    }
    #[inline]
    pub fn to_index(&self, pid: PatternID, name: &str) -> Option<usize> {
        let indices = self.0.name_to_index.get(pid.as_usize())?;
        indices.get(name).cloned().map(|i| i.as_usize())
    }

    #[inline]
    pub fn pattern_names(&self, pid: PatternID) -> GroupInfoPatternNames<'_> {
        GroupInfoPatternNames {
            it: self
                .0
                .index_to_name
                .get(pid.as_usize())
                .map(|indices| indices.iter())
                .unwrap_or([].iter()),
        }
    }

    #[inline]
    pub fn slots(&self, pid: PatternID, group_index: usize) -> Option<(usize, usize)> {
        self.slot(pid, group_index).map(|start| (start, start + 1))
    }
    #[inline]
    pub fn slot(&self, pid: PatternID, group_index: usize) -> Option<usize> {
        if group_index >= self.group_len(pid) {
            return None;
        }
        if group_index == 0 {
            Some(pid.as_usize() * 2)
        } else {
            let (start, _) = self.0.slot_ranges[pid];
            Some(start.as_usize() + ((group_index - 1) * 2))
        }
    }
    #[inline]
    pub fn pattern_len(&self) -> usize {
        self.0.pattern_len()
    }
    #[inline]
    pub fn group_len(&self, pid: PatternID) -> usize {
        self.0.group_len(pid)
    }

    #[inline]
    pub fn slot_len(&self) -> usize {
        self.0.small_slot_len().as_usize()
    }
    #[inline]
    pub fn implicit_slot_len(&self) -> usize {
        self.pattern_len() * 2
    }
    #[inline]
    pub fn explicit_slot_len(&self) -> usize {
        self.slot_len().saturating_sub(self.implicit_slot_len())
    }
    #[inline]
    pub fn memory_usage(&self) -> usize {
        use core::mem::size_of as s;
        s::<GroupInfoInner>()
            + self.0.slot_ranges.len() * s::<(SmallIndex, SmallIndex)>()
            + self.0.name_to_index.len() * s::<CaptureNameMap>()
            + self.0.index_to_name.len() * s::<Vec<Option<Arc<str>>>>()
            + self.0.memory_extra
    }
}
type CaptureNameMap = std::collections::HashMap<Arc<str>, SmallIndex>;
#[derive(Debug, Default)]
struct GroupInfoInner {
    slot_ranges: Vec<(SmallIndex, SmallIndex)>,
    name_to_index: Vec<CaptureNameMap>,
    index_to_name: Vec<Vec<Option<Arc<str>>>>,
    memory_extra: usize,
}
impl GroupInfoInner {
    fn add_first_group(&mut self, pid: PatternID) {
        assert_eq!(pid.as_usize(), self.slot_ranges.len());
        assert_eq!(pid.as_usize(), self.name_to_index.len());
        assert_eq!(pid.as_usize(), self.index_to_name.len());
        let slot_start = self.small_slot_len();
        self.slot_ranges.push((slot_start, slot_start));
        self.name_to_index.push(CaptureNameMap::new());
        self.index_to_name.push(vec![None]);
        self.memory_extra += core::mem::size_of::<Option<Arc<str>>>();
    }
    fn add_explicit_group<N: AsRef<str>>(
        &mut self,
        pid: PatternID,
        group: SmallIndex,
        maybe_name: Option<N>,
    ) -> Result<(), GroupInfoError> {
        let end = &mut self.slot_ranges[pid].1;
        *end = SmallIndex::new(end.as_usize() + 2)
            .map_err(|_| GroupInfoError::too_many_groups(pid, group.as_usize()))?;
        if let Some(name) = maybe_name {
            let name = Arc::<str>::from(name.as_ref());
            if self.name_to_index[pid].contains_key(&*name) {
                return Err(GroupInfoError::duplicate(pid, &name));
            }
            let len = name.len();
            self.name_to_index[pid].insert(Arc::clone(&name), group);
            self.index_to_name[pid].push(Some(name));
            self.memory_extra += 2 * (len + core::mem::size_of::<Option<Arc<str>>>());
            self.memory_extra += core::mem::size_of::<SmallIndex>();
        } else {
            self.index_to_name[pid].push(None);
            self.memory_extra += core::mem::size_of::<Option<Arc<str>>>();
        }
        assert_eq!(group.one_more(), self.group_len(pid));
        assert_eq!(group.one_more(), self.index_to_name[pid].len());
        Ok(())
    }
    fn fixup_slot_ranges(&mut self) -> Result<(), GroupInfoError> {
        use crate::regex_engine::automata::util::primitives::IteratorIndexExt;
        let offset = self.pattern_len().checked_mul(2).unwrap();
        for (pid, &mut (ref mut start, ref mut end)) in
            self.slot_ranges.iter_mut().with_pattern_ids()
        {
            let group_len = 1 + ((end.as_usize() - start.as_usize()) / 2);
            let new_end = match end.as_usize().checked_add(offset) {
                Some(new_end) => new_end,
                None => return Err(GroupInfoError::too_many_groups(pid, group_len)),
            };
            *end = SmallIndex::new(new_end)
                .map_err(|_| GroupInfoError::too_many_groups(pid, group_len))?;
            *start = SmallIndex::new(start.as_usize() + offset).unwrap();
        }
        Ok(())
    }
    fn pattern_len(&self) -> usize {
        self.slot_ranges.len()
    }
    fn group_len(&self, pid: PatternID) -> usize {
        let (start, end) = match self.slot_ranges.get(pid.as_usize()) {
            None => return 0,
            Some(range) => range,
        };
        1 + ((end.as_usize() - start.as_usize()) / 2)
    }
    fn small_slot_len(&self) -> SmallIndex {
        self.slot_ranges
            .last()
            .map_or(SmallIndex::ZERO, |&(_, end)| end)
    }
}
#[derive(Clone, Debug)]
pub struct GroupInfoError {
    kind: GroupInfoErrorKind,
}
#[derive(Clone, Debug)]
enum GroupInfoErrorKind {
    TooManyPatterns { err: PatternIDError },
    TooManyGroups { pattern: PatternID, minimum: usize },
    MissingGroups { pattern: PatternID },
    FirstMustBeUnnamed { pattern: PatternID },
    Duplicate { pattern: PatternID, name: String },
}
impl GroupInfoError {
    fn too_many_patterns(err: PatternIDError) -> GroupInfoError {
        GroupInfoError {
            kind: GroupInfoErrorKind::TooManyPatterns { err },
        }
    }
    fn too_many_groups(pattern: PatternID, minimum: usize) -> GroupInfoError {
        GroupInfoError {
            kind: GroupInfoErrorKind::TooManyGroups { pattern, minimum },
        }
    }
    fn missing_groups(pattern: PatternID) -> GroupInfoError {
        GroupInfoError {
            kind: GroupInfoErrorKind::MissingGroups { pattern },
        }
    }
    fn first_must_be_unnamed(pattern: PatternID) -> GroupInfoError {
        GroupInfoError {
            kind: GroupInfoErrorKind::FirstMustBeUnnamed { pattern },
        }
    }
    fn duplicate(pattern: PatternID, name: &str) -> GroupInfoError {
        GroupInfoError {
            kind: GroupInfoErrorKind::Duplicate {
                pattern,
                name: String::from(name),
            },
        }
    }
}
impl std::error::Error for GroupInfoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self.kind {
            GroupInfoErrorKind::TooManyPatterns { .. }
            | GroupInfoErrorKind::TooManyGroups { .. }
            | GroupInfoErrorKind::MissingGroups { .. }
            | GroupInfoErrorKind::FirstMustBeUnnamed { .. }
            | GroupInfoErrorKind::Duplicate { .. } => None,
        }
    }
}
impl core::fmt::Display for GroupInfoError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        use self::GroupInfoErrorKind::*;
        match self.kind {
            TooManyPatterns { ref err } => {
                write!(f, "too many patterns to build capture info: {err}")
            }
            TooManyGroups { pattern, minimum } => {
                write!(
                    f,
                    "too many capture groups (at least {}) were \
                     found for pattern {}",
                    minimum,
                    pattern.as_usize()
                )
            }
            MissingGroups { pattern } => write!(
                f,
                "no capturing groups found for pattern {} \
                 (either all patterns have zero groups or all patterns have \
                  at least one group)",
                pattern.as_usize(),
            ),
            FirstMustBeUnnamed { pattern } => write!(
                f,
                "first capture group (at index 0) for pattern {} has a name \
                 (it must be unnamed)",
                pattern.as_usize(),
            ),
            Duplicate { pattern, ref name } => write!(
                f,
                "duplicate capture group name '{}' found for pattern {}",
                name,
                pattern.as_usize(),
            ),
        }
    }
}
#[derive(Clone, Debug)]
pub struct GroupInfoPatternNames<'a> {
    it: core::slice::Iter<'a, Option<Arc<str>>>,
}
impl GroupInfoPatternNames<'static> {
    fn empty() -> GroupInfoPatternNames<'static> {
        GroupInfoPatternNames { it: [].iter() }
    }
}
impl<'a> Iterator for GroupInfoPatternNames<'a> {
    type Item = Option<&'a str>;
    fn next(&mut self) -> Option<Option<&'a str>> {
        self.it.next().map(|x| x.as_deref())
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.it.size_hint()
    }
    fn count(self) -> usize {
        self.it.count()
    }
}
impl<'a> ExactSizeIterator for GroupInfoPatternNames<'a> {}
impl<'a> core::iter::FusedIterator for GroupInfoPatternNames<'a> {}
#[derive(Debug)]
pub struct GroupInfoAllNames<'a> {
    group_info: &'a GroupInfo,
    pids: PatternIDIter,
    current_pid: Option<PatternID>,
    names: Option<core::iter::Enumerate<GroupInfoPatternNames<'a>>>,
}
impl<'a> Iterator for GroupInfoAllNames<'a> {
    type Item = (PatternID, usize, Option<&'a str>);
    fn next(&mut self) -> Option<(PatternID, usize, Option<&'a str>)> {
        if self.group_info.0.index_to_name.is_empty() {
            return None;
        }
        if self.current_pid.is_none() {
            self.current_pid = Some(self.pids.next()?);
        }
        let pid = self.current_pid.unwrap();
        if self.names.is_none() {
            self.names = Some(self.group_info.pattern_names(pid).enumerate());
        }
        let (group_index, name) = match self.names.as_mut().unwrap().next() {
            Some((group_index, name)) => (group_index, name),
            None => {
                self.current_pid = None;
                self.names = None;
                return self.next();
            }
        };
        Some((pid, group_index, name))
    }
}
