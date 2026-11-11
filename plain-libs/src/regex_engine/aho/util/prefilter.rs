use crate::regex_engine::aho::{
    packed,
    util::{
        alphabet::ByteSet,
        search::{Match, MatchKind, Span},
    },
};
use core::{
    cmp,
    fmt::Debug,
    panic::{RefUnwindSafe, UnwindSafe},
    u8,
};
use std::{sync::Arc, vec, vec::Vec};
#[derive(Clone, Debug)]
pub struct Prefilter {
    finder: Arc<dyn PrefilterI>,
    memory_usage: usize,
}
impl Prefilter {
    #[inline]
    pub fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        self.finder.find_in(haystack, span)
    }
    #[inline]
    pub(crate) fn memory_usage(&self) -> usize {
        self.memory_usage
    }
}
#[derive(Clone, Debug)]
pub enum Candidate {
    None,
    Match(Match),
    PossibleStartOfMatch(usize),
}
impl Candidate {
    pub fn into_option(self) -> Option<usize> {
        match self {
            Candidate::None => None,
            Candidate::Match(ref m) => Some(m.start()),
            Candidate::PossibleStartOfMatch(start) => Some(start),
        }
    }
}
trait PrefilterI: Send + Sync + RefUnwindSafe + UnwindSafe + Debug + 'static {
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate;
}
impl<P: PrefilterI + ?Sized> PrefilterI for Arc<P> {
    #[inline(always)]
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        (**self).find_in(haystack, span)
    }
}
#[derive(Debug)]
pub(crate) struct Builder {
    count: usize,
    ascii_case_insensitive: bool,
    start_bytes: StartBytesBuilder,
    rare_bytes: RareBytesBuilder,
    memmem: MemmemBuilder,
    packed: Option<packed::Builder>,
    enabled: bool,
}
impl Builder {
    pub(crate) fn new(kind: MatchKind) -> Builder {
        let pbuilder = kind
            .as_packed()
            .map(|kind| packed::Config::new().match_kind(kind).builder());
        Builder {
            count: 0,
            ascii_case_insensitive: false,
            start_bytes: StartBytesBuilder::new(),
            rare_bytes: RareBytesBuilder::new(),
            memmem: MemmemBuilder::default(),
            packed: pbuilder,
            enabled: true,
        }
    }
    pub(crate) fn ascii_case_insensitive(mut self, yes: bool) -> Builder {
        self.ascii_case_insensitive = yes;
        self.start_bytes = self.start_bytes.ascii_case_insensitive(yes);
        self.rare_bytes = self.rare_bytes.ascii_case_insensitive(yes);
        self
    }
    pub(crate) fn build(&self) -> Option<Prefilter> {
        if !self.enabled {
            debug!("prefilter not enabled, skipping");
            return None;
        }
        if !self.ascii_case_insensitive {
            if let Some(pre) = self.memmem.build() {
                debug!("using memmem prefilter");
                return Some(pre);
            }
        }
        let (packed, patlen, minlen) = if self.ascii_case_insensitive {
            (None, usize::MAX, 0)
        } else {
            let patlen = self.packed.as_ref().map_or(usize::MAX, |p| p.len());
            let minlen = self.packed.as_ref().map_or(0, |p| p.minimum_len());
            let packed = self.packed.as_ref().and_then(|b| b.build()).map(|s| {
                let memory_usage = s.memory_usage();
                debug!(
                    "built packed prefilter (len: {}, \
                         minimum pattern len: {}, memory usage: {}) \
                         for consideration",
                    patlen, minlen, memory_usage,
                );
                Prefilter {
                    finder: Arc::new(Packed(s)),
                    memory_usage,
                }
            });
            (packed, patlen, minlen)
        };
        match (self.start_bytes.build(), self.rare_bytes.build()) {
            (prestart @ Some(_), prerare @ Some(_)) => {
                debug!(
                    "both start (len={}, rank={}) and \
                     rare (len={}, rank={}) byte prefilters \
                     are available",
                    self.start_bytes.count,
                    self.start_bytes.rank_sum,
                    self.rare_bytes.count,
                    self.rare_bytes.rank_sum,
                );
                if patlen <= 16
                    && minlen >= 2
                    && self.start_bytes.count >= 3
                    && self.rare_bytes.count >= 3
                {
                    debug!(
                        "start and rare byte prefilters available, but \
                             they're probably slower than packed so using \
                             packed"
                    );
                    return packed;
                }
                let has_fewer_bytes = self.start_bytes.count < self.rare_bytes.count;
                let has_rarer_bytes = self.start_bytes.rank_sum <= self.rare_bytes.rank_sum + 50;
                if has_fewer_bytes {
                    debug!(
                        "using start byte prefilter because it has fewer
                         bytes to search for than the rare byte prefilter",
                    );
                    prestart
                } else if has_rarer_bytes {
                    debug!(
                        "using start byte prefilter because its byte \
                         frequency rank was determined to be \
                         \"good enough\" relative to the rare byte prefilter \
                         byte frequency rank",
                    );
                    prestart
                } else {
                    debug!("using rare byte prefilter");
                    prerare
                }
            }
            (prestart @ Some(_), None) => {
                if patlen <= 16 && minlen >= 2 && self.start_bytes.count >= 3 {
                    debug!(
                        "start byte prefilter available, but \
                         it's probably slower than packed so using \
                         packed"
                    );
                    return packed;
                }
                debug!(
                    "have start byte prefilter but not rare byte prefilter, \
                     so using start byte prefilter",
                );
                prestart
            }
            (None, prerare @ Some(_)) => {
                if patlen <= 16 && minlen >= 2 && self.rare_bytes.count >= 3 {
                    debug!(
                        "rare byte prefilter available, but \
                         it's probably slower than packed so using \
                         packed"
                    );
                    return packed;
                }
                debug!(
                    "have rare byte prefilter but not start byte prefilter, \
                     so using rare byte prefilter",
                );
                prerare
            }
            (None, None) if self.ascii_case_insensitive => {
                debug!(
                    "no start or rare byte prefilter and ASCII case \
                     insensitivity was enabled, so skipping prefilter",
                );
                None
            }
            (None, None) => {
                if packed.is_some() {
                    debug!("falling back to packed prefilter");
                } else {
                    debug!("no prefilter available");
                }
                packed
            }
        }
    }
    pub(crate) fn add(&mut self, bytes: &[u8]) {
        if bytes.is_empty() {
            self.enabled = false;
        }
        if !self.enabled {
            return;
        }
        self.count += 1;
        self.start_bytes.add(bytes);
        self.rare_bytes.add(bytes);
        self.memmem.add(bytes);
        if let Some(ref mut pbuilder) = self.packed {
            pbuilder.add(bytes);
        }
    }
}
#[derive(Clone, Debug)]
struct Packed(packed::Searcher);
impl PrefilterI for Packed {
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        self.0
            .find_in(haystack, span)
            .map_or(Candidate::None, Candidate::Match)
    }
}
#[derive(Debug, Default)]
struct MemmemBuilder {
    count: usize,
    one: Option<Vec<u8>>,
}
impl MemmemBuilder {
    fn build(&self) -> Option<Prefilter> {
        fn imp(builder: &MemmemBuilder) -> Option<Prefilter> {
            let pattern = builder.one.as_ref()?;
            assert_eq!(1, builder.count);
            let finder = Arc::new(Memmem(memchr::memmem::Finder::new(pattern).into_owned()));
            let memory_usage = pattern.len();
            Some(Prefilter {
                finder,
                memory_usage,
            })
        }
        imp(self)
    }
    fn add(&mut self, bytes: &[u8]) {
        self.count += 1;
        if self.count == 1 {
            self.one = Some(bytes.to_vec());
        } else {
            self.one = None;
        }
    }
}
#[derive(Clone, Debug)]
struct Memmem(memchr::memmem::Finder<'static>);
impl PrefilterI for Memmem {
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        use crate::regex_engine::aho::util::primitives::PatternID;
        self.0.find(&haystack[span]).map_or(Candidate::None, |i| {
            let start = span.start + i;
            let end = start + self.0.needle().len();
            Candidate::Match(Match::new(PatternID::ZERO, start..end))
        })
    }
}
#[derive(Clone, Debug)]
struct RareBytesBuilder {
    ascii_case_insensitive: bool,
    rare_set: ByteSet,
    byte_offsets: RareByteOffsets,
    available: bool,
    count: usize,
    rank_sum: u16,
}
#[derive(Clone, Copy)]
struct RareByteOffsets {
    set: [RareByteOffset; 256],
}
impl RareByteOffsets {
    pub(crate) fn empty() -> RareByteOffsets {
        RareByteOffsets {
            set: [RareByteOffset::default(); 256],
        }
    }
    pub(crate) fn set(&mut self, byte: u8, off: RareByteOffset) {
        self.set[byte as usize].max = cmp::max(self.set[byte as usize].max, off.max);
    }
}
impl core::fmt::Debug for RareByteOffsets {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut offsets = vec![];
        for off in self.set.iter() {
            if off.max > 0 {
                offsets.push(off);
            }
        }
        f.debug_struct("RareByteOffsets")
            .field("set", &offsets)
            .finish()
    }
}
#[derive(Clone, Copy, Debug)]
struct RareByteOffset {
    max: u8,
}
impl Default for RareByteOffset {
    fn default() -> RareByteOffset {
        RareByteOffset { max: 0 }
    }
}
impl RareByteOffset {
    fn new(max: usize) -> Option<RareByteOffset> {
        if max > u8::MAX as usize {
            None
        } else {
            Some(RareByteOffset { max: max as u8 })
        }
    }
}
impl RareBytesBuilder {
    fn new() -> RareBytesBuilder {
        RareBytesBuilder {
            ascii_case_insensitive: false,
            rare_set: ByteSet::empty(),
            byte_offsets: RareByteOffsets::empty(),
            available: true,
            count: 0,
            rank_sum: 0,
        }
    }
    fn ascii_case_insensitive(mut self, yes: bool) -> RareBytesBuilder {
        self.ascii_case_insensitive = yes;
        self
    }
    fn build(&self) -> Option<Prefilter> {
        fn imp(builder: &RareBytesBuilder) -> Option<Prefilter> {
            if !builder.available || builder.count > 3 {
                return None;
            }
            let (mut bytes, mut len) = ([0; 3], 0);
            for b in 0..=255 {
                if builder.rare_set.contains(b) {
                    bytes[len] = b;
                    len += 1;
                }
            }
            let finder: Arc<dyn PrefilterI> = match len {
                0 => return None,
                1 => Arc::new(RareBytesOne {
                    byte1: bytes[0],
                    offset: builder.byte_offsets.set[bytes[0] as usize],
                }),
                2 => Arc::new(RareBytesTwo {
                    offsets: builder.byte_offsets,
                    byte1: bytes[0],
                    byte2: bytes[1],
                }),
                3 => Arc::new(RareBytesThree {
                    offsets: builder.byte_offsets,
                    byte1: bytes[0],
                    byte2: bytes[1],
                    byte3: bytes[2],
                }),
                _ => unreachable!(),
            };
            Some(Prefilter {
                finder,
                memory_usage: 0,
            })
        }
        imp(self)
    }
    fn add(&mut self, bytes: &[u8]) {
        if !self.available {
            return;
        }
        if self.count > 3 {
            self.available = false;
            return;
        }
        if bytes.len() >= 256 {
            self.available = false;
            return;
        }
        let mut rarest = match bytes.first() {
            None => return,
            Some(&b) => (b, freq_rank(b)),
        };
        let mut found = false;
        for (pos, &b) in bytes.iter().enumerate() {
            self.set_offset(pos, b);
            if found {
                continue;
            }
            if self.rare_set.contains(b) {
                found = true;
                continue;
            }
            let rank = freq_rank(b);
            if rank < rarest.1 {
                rarest = (b, rank);
            }
        }
        if !found {
            self.add_rare_byte(rarest.0);
        }
    }
    fn set_offset(&mut self, pos: usize, byte: u8) {
        let offset = RareByteOffset::new(pos).unwrap();
        self.byte_offsets.set(byte, offset);
        if self.ascii_case_insensitive {
            self.byte_offsets.set(opposite_ascii_case(byte), offset);
        }
    }
    fn add_rare_byte(&mut self, byte: u8) {
        self.add_one_rare_byte(byte);
        if self.ascii_case_insensitive {
            self.add_one_rare_byte(opposite_ascii_case(byte));
        }
    }
    fn add_one_rare_byte(&mut self, byte: u8) {
        if !self.rare_set.contains(byte) {
            self.rare_set.add(byte);
            self.count += 1;
            self.rank_sum += freq_rank(byte) as u16;
        }
    }
}
#[derive(Clone, Debug)]
struct RareBytesOne {
    byte1: u8,
    offset: RareByteOffset,
}
impl PrefilterI for RareBytesOne {
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        memchr::memchr(self.byte1, &haystack[span])
            .map(|i| {
                let pos = span.start + i;
                cmp::max(span.start, pos.saturating_sub(usize::from(self.offset.max)))
            })
            .map_or(Candidate::None, Candidate::PossibleStartOfMatch)
    }
}
#[derive(Clone, Debug)]
struct RareBytesTwo {
    offsets: RareByteOffsets,
    byte1: u8,
    byte2: u8,
}
impl PrefilterI for RareBytesTwo {
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        memchr::memchr2(self.byte1, self.byte2, &haystack[span])
            .map(|i| {
                let pos = span.start + i;
                let offset = self.offsets.set[usize::from(haystack[pos])].max;
                cmp::max(span.start, pos.saturating_sub(usize::from(offset)))
            })
            .map_or(Candidate::None, Candidate::PossibleStartOfMatch)
    }
}
#[derive(Clone, Debug)]
struct RareBytesThree {
    offsets: RareByteOffsets,
    byte1: u8,
    byte2: u8,
    byte3: u8,
}
impl PrefilterI for RareBytesThree {
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        memchr::memchr3(self.byte1, self.byte2, self.byte3, &haystack[span])
            .map(|i| {
                let pos = span.start + i;
                let offset = self.offsets.set[usize::from(haystack[pos])].max;
                cmp::max(span.start, pos.saturating_sub(usize::from(offset)))
            })
            .map_or(Candidate::None, Candidate::PossibleStartOfMatch)
    }
}
#[derive(Clone, Debug)]
struct StartBytesBuilder {
    ascii_case_insensitive: bool,
    byteset: Vec<bool>,
    count: usize,
    rank_sum: u16,
}
impl StartBytesBuilder {
    fn new() -> StartBytesBuilder {
        StartBytesBuilder {
            ascii_case_insensitive: false,
            byteset: vec![false; 256],
            count: 0,
            rank_sum: 0,
        }
    }
    fn ascii_case_insensitive(mut self, yes: bool) -> StartBytesBuilder {
        self.ascii_case_insensitive = yes;
        self
    }
    fn build(&self) -> Option<Prefilter> {
        fn imp(builder: &StartBytesBuilder) -> Option<Prefilter> {
            if builder.count > 3 {
                return None;
            }
            let (mut bytes, mut len) = ([0; 3], 0);
            for b in 0..256 {
                if !builder.byteset[b] {
                    continue;
                }
                if b > 0x7F {
                    return None;
                }
                bytes[len] = b as u8;
                len += 1;
            }
            let finder: Arc<dyn PrefilterI> = match len {
                0 => return None,
                1 => Arc::new(StartBytesOne { byte1: bytes[0] }),
                2 => Arc::new(StartBytesTwo {
                    byte1: bytes[0],
                    byte2: bytes[1],
                }),
                3 => Arc::new(StartBytesThree {
                    byte1: bytes[0],
                    byte2: bytes[1],
                    byte3: bytes[2],
                }),
                _ => unreachable!(),
            };
            Some(Prefilter {
                finder,
                memory_usage: 0,
            })
        }
        imp(self)
    }
    fn add(&mut self, bytes: &[u8]) {
        if self.count > 3 {
            return;
        }
        if let Some(&byte) = bytes.first() {
            self.add_one_byte(byte);
            if self.ascii_case_insensitive {
                self.add_one_byte(opposite_ascii_case(byte));
            }
        }
    }
    fn add_one_byte(&mut self, byte: u8) {
        if !self.byteset[byte as usize] {
            self.byteset[byte as usize] = true;
            self.count += 1;
            self.rank_sum += freq_rank(byte) as u16;
        }
    }
}
#[derive(Clone, Debug)]
struct StartBytesOne {
    byte1: u8,
}
impl PrefilterI for StartBytesOne {
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        memchr::memchr(self.byte1, &haystack[span])
            .map(|i| span.start + i)
            .map_or(Candidate::None, Candidate::PossibleStartOfMatch)
    }
}
#[derive(Clone, Debug)]
struct StartBytesTwo {
    byte1: u8,
    byte2: u8,
}
impl PrefilterI for StartBytesTwo {
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        memchr::memchr2(self.byte1, self.byte2, &haystack[span])
            .map(|i| span.start + i)
            .map_or(Candidate::None, Candidate::PossibleStartOfMatch)
    }
}
#[derive(Clone, Debug)]
struct StartBytesThree {
    byte1: u8,
    byte2: u8,
    byte3: u8,
}
impl PrefilterI for StartBytesThree {
    fn find_in(&self, haystack: &[u8], span: Span) -> Candidate {
        memchr::memchr3(self.byte1, self.byte2, self.byte3, &haystack[span])
            .map(|i| span.start + i)
            .map_or(Candidate::None, Candidate::PossibleStartOfMatch)
    }
}
pub(crate) fn opposite_ascii_case(b: u8) -> u8 {
    if b'A' <= b && b <= b'Z' {
        b.to_ascii_lowercase()
    } else if b'a' <= b && b <= b'z' {
        b.to_ascii_uppercase()
    } else {
        b
    }
}
fn freq_rank(b: u8) -> u8 {
    use crate::regex_engine::aho::util::byte_frequencies::BYTE_FREQUENCIES;
    BYTE_FREQUENCIES[b as usize]
}
