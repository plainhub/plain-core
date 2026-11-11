use crate::compression::backend::deflate::{
    MIN_LOOKAHEAD, Pos, STD_MAX_MATCH, STD_MIN_MATCH, State,
};
const EARLY_EXIT_TRIGGER_LEVEL: i8 = 5;
pub fn longest_match(
    state: &crate::compression::backend::deflate::State,
    cur_match: u16,
) -> (usize, u16) {
    longest_match_help::<false>(state, cur_match)
}
pub fn longest_match_slow(
    state: &crate::compression::backend::deflate::State,
    cur_match: u16,
) -> (usize, u16) {
    longest_match_help::<true>(state, cur_match)
}
fn longest_match_help<const SLOW: bool>(
    state: &crate::compression::backend::deflate::State,
    mut cur_match: u16,
) -> (usize, u16) {
    let mut match_start = state.match_start;
    let strstart = state.strstart;
    let wmask = state.w_mask();
    let window = state.window.filled();
    let scan = &window[strstart..];
    let prev = &state.prev.as_slice()[..=wmask];
    let mut limit: Pos;
    let limit_base: Pos;
    let early_exit: bool;
    let mut chain_length: u16;
    let mut best_len: usize;
    let lookahead = state.lookahead;
    let mut match_offset = 0;
    macro_rules! goto_next_in_chain {
        () => {
            chain_length -= 1;
            if chain_length > 0 {
                cur_match = prev[cur_match as usize & wmask];
                if cur_match > limit {
                    continue;
                }
            }
            return (best_len, match_start);
        };
    }
    assert_eq!(STD_MAX_MATCH, 258, "Code too clever");
    best_len = if state.prev_length > 0 {
        state.prev_length as usize
    } else {
        STD_MIN_MATCH - 1
    };
    let mut offset = best_len - 1;
    if best_len >= core::mem::size_of::<u32>() {
        offset -= 2;
        if best_len >= core::mem::size_of::<u64>() {
            offset -= 4;
        }
    }
    let mut mbase_start = window.as_ptr();
    let mut mbase_end = window[offset..].as_ptr();
    chain_length = state.max_chain_length;
    if best_len >= state.good_match as usize {
        chain_length >>= 2;
    }
    let nice_match = state.nice_match;
    limit = strstart.saturating_sub(state.max_dist()) as Pos;
    if SLOW {
        limit_base = limit;
        if best_len >= STD_MIN_MATCH {
            let mut pos: Pos;
            let Some([_cur_match, scan1, scan2, scanrest @ ..]) = scan.get(..best_len + 1) else {
                panic!("invalid scan");
            };
            let mut hash = 0;
            hash = state.update_hash(hash, *scan1 as u32);
            hash = state.update_hash(hash, *scan2 as u32);
            for (i, b) in scanrest.iter().enumerate() {
                hash = state.update_hash(hash, *b as u32);
                pos = state.head.as_slice()[hash as usize];
                if pos < cur_match {
                    match_offset = (i + 1) as Pos;
                    cur_match = pos;
                }
            }
            limit = limit_base + match_offset;
            if cur_match <= limit {
                return break_matching(state, best_len, match_start);
            }
            mbase_start = mbase_start.wrapping_sub(match_offset as usize);
            mbase_end = mbase_end.wrapping_sub(match_offset as usize);
        }
        early_exit = false;
    } else {
        limit_base = 0;
        early_exit = state.level < EARLY_EXIT_TRIGGER_LEVEL;
    }
    let scan_start = window[strstart..].as_ptr();
    let mut scan_end = window[strstart + offset..].as_ptr();
    assert!(
        strstart <= state.window_size.saturating_sub(MIN_LOOKAHEAD),
        "need lookahead"
    );
    loop {
        if cur_match as usize >= strstart {
            break;
        }
        #[inline(always)]
        unsafe fn memcmp_n_ptr<const N: usize>(src0: *const u8, src1: *const u8) -> bool {
            {
                unsafe {
                    let src0_cmp = core::ptr::read(src0 as *const [u8; N]);
                    let src1_cmp = core::ptr::read(src1 as *const [u8; N]);
                    src0_cmp == src1_cmp
                }
            }
        }
        #[inline(always)]
        unsafe fn is_match<const N: usize>(
            cur_match: u16,
            mbase_start: *const u8,
            mbase_end: *const u8,
            scan_start: *const u8,
            scan_end: *const u8,
        ) -> bool {
            {
                let be = mbase_end.wrapping_add(cur_match as usize);
                let bs = mbase_start.wrapping_add(cur_match as usize);
                unsafe { memcmp_n_ptr::<N>(be, scan_end) && memcmp_n_ptr::<N>(bs, scan_start) }
            }
        }
        let mut len = 0;
        unsafe {
            if best_len < core::mem::size_of::<u64>() {
                let scan_val = u64::from_ne_bytes(
                    core::slice::from_raw_parts(scan_start, 8)
                        .try_into()
                        .unwrap(),
                );
                loop {
                    let bs = mbase_start.wrapping_add(cur_match as usize);
                    let match_val =
                        u64::from_ne_bytes(core::slice::from_raw_parts(bs, 8).try_into().unwrap());
                    let cmp = scan_val ^ match_val;
                    if cmp == 0 {
                        break;
                    }
                    let cmp_len = cmp.to_le().trailing_zeros() as usize / 8;
                    if cmp_len > best_len {
                        len = cmp_len;
                        break;
                    }
                    goto_next_in_chain!();
                }
            } else {
                loop {
                    if is_match::<8>(cur_match, mbase_start, mbase_end, scan_start, scan_end) {
                        break;
                    }
                    goto_next_in_chain!();
                }
            }
        }
        if len == 0 {
            len = {
                let src1 = unsafe {
                    core::slice::from_raw_parts(
                        mbase_start.wrapping_add(cur_match as usize + 2),
                        256,
                    )
                };
                crate::compression::backend::deflate::compare256::compare256_slice(&scan[2..], src1)
                    + 2
            };
        }
        assert!(
            scan.as_ptr() as usize + len <= window.as_ptr() as usize + (state.window_size - 1),
            "wild scan"
        );
        if len > best_len {
            match_start = cur_match - match_offset;
            if len >= lookahead {
                return (lookahead, match_start);
            }
            best_len = len;
            if best_len >= nice_match as usize {
                return (best_len, match_start);
            }
            offset = best_len - 1;
            if best_len >= core::mem::size_of::<u32>() {
                offset -= 2;
                if best_len >= core::mem::size_of::<u64>() {
                    offset -= 4;
                }
            }
            scan_end = window[strstart + offset..].as_ptr();
            if SLOW && len > STD_MIN_MATCH && match_start as usize + len < strstart {
                let mut pos: Pos;
                cur_match -= match_offset;
                match_offset = 0;
                let mut next_pos = cur_match;
                for i in 0..=len - STD_MIN_MATCH {
                    pos = prev[(cur_match as usize + i) & wmask];
                    if pos < next_pos {
                        if pos <= limit_base + i as Pos {
                            return break_matching(state, best_len, match_start);
                        }
                        next_pos = pos;
                        match_offset = i as Pos;
                    }
                }
                cur_match = next_pos;
                let [scan0, scan1, scan2, ..] = scan[len - (STD_MIN_MATCH + 1)..] else {
                    panic!("index out of bounds");
                };
                let mut hash = 0;
                hash = state.update_hash(hash, scan0 as u32);
                hash = state.update_hash(hash, scan1 as u32);
                hash = state.update_hash(hash, scan2 as u32);
                pos = state.head.as_slice()[hash as usize];
                if pos < cur_match {
                    match_offset = (len - (STD_MIN_MATCH + 1)) as Pos;
                    if pos <= limit_base + match_offset {
                        return break_matching(state, best_len, match_start);
                    }
                    cur_match = pos;
                }
                limit = limit_base + match_offset;
                mbase_start = window.as_ptr().wrapping_sub(match_offset as usize);
                mbase_end = mbase_start.wrapping_add(offset);
                continue;
            }
            mbase_end = mbase_start.wrapping_add(offset);
        } else if !SLOW && early_exit {
            break;
        }
        goto_next_in_chain!();
    }
    (best_len, match_start)
}
fn break_matching(state: &State, best_len: usize, match_start: u16) -> (usize, u16) {
    (Ord::min(best_len, state.lookahead), match_start)
}
