#![forbid(unsafe_code)]
use super::flush_block;
use crate::compression::backend::{
    DeflateFlush,
    deflate::{
        BlockState, DeflateStream, MIN_LOOKAHEAD, STD_MIN_MATCH, Strategy, WANT_MIN_MATCH,
        fill_window, flush_block_only,
    },
};
pub fn deflate_slow(stream: &mut DeflateStream, flush: DeflateFlush) -> BlockState {
    let mut hash_head;
    let mut bflush;
    let mut dist;
    let mut match_len;
    let use_longest_match_slow = stream.state.max_chain_length > 1024;
    let valid_distance_range = 1..=stream.state.max_dist() as isize;
    let mut match_available = stream.state.match_available;
    loop {
        if stream.state.lookahead < MIN_LOOKAHEAD {
            fill_window(stream);
            if stream.state.lookahead < MIN_LOOKAHEAD && flush == DeflateFlush::NoFlush {
                return BlockState::NeedMore;
            }
            if stream.state.lookahead == 0 {
                break;
            }
        }
        let state = &mut stream.state;
        hash_head = if state.lookahead >= WANT_MIN_MATCH {
            state.quick_insert_string(state.strstart)
        } else {
            0
        };
        state.prev_match = state.match_start;
        match_len = STD_MIN_MATCH - 1;
        dist = state.strstart as isize - hash_head as isize;
        if valid_distance_range.contains(&dist)
            && state.prev_length < state.max_lazy_match
            && hash_head != 0
        {
            (match_len, state.match_start) = if use_longest_match_slow {
                crate::compression::backend::deflate::longest_match::longest_match_slow(
                    state, hash_head,
                )
            } else {
                crate::compression::backend::deflate::longest_match::longest_match(state, hash_head)
            };
            if match_len <= 5 && (state.strategy == Strategy::Filtered) {
                match_len = STD_MIN_MATCH - 1;
            }
        }
        if state.prev_length as usize >= STD_MIN_MATCH && match_len <= state.prev_length as usize {
            let max_insert = state.strstart + state.lookahead - STD_MIN_MATCH;
            bflush = state.tally_dist(
                state.strstart - 1 - state.prev_match as usize,
                state.prev_length as usize - STD_MIN_MATCH,
            );
            state.prev_length -= 1;
            state.lookahead -= state.prev_length as usize;
            let mov_fwd = state.prev_length as usize - 1;
            if max_insert > state.strstart {
                let insert_cnt = Ord::min(mov_fwd, max_insert - state.strstart);
                state.insert_string(state.strstart + 1, insert_cnt);
            }
            state.prev_length = 0;
            state.match_available = false;
            match_available = false;
            state.strstart += mov_fwd + 1;
            if bflush {
                flush_block!(stream, false);
            }
        } else if match_available {
            let lc = state.window.filled()[state.strstart - 1];
            bflush = state.tally_lit(lc);
            if bflush {
                flush_block_only(stream, false);
            }
            stream.state.prev_length = match_len as u16;
            stream.state.strstart += 1;
            stream.state.lookahead -= 1;
            if stream.avail_out == 0 {
                return BlockState::NeedMore;
            }
        } else {
            state.prev_length = match_len as u16;
            state.match_available = true;
            match_available = true;
            state.strstart += 1;
            state.lookahead -= 1;
        }
    }
    assert_ne!(flush, DeflateFlush::NoFlush, "no flush?");
    let state = &mut stream.state;
    if state.match_available {
        let lc = state.window.filled()[state.strstart - 1];
        let _ = state.tally_lit(lc);
        state.match_available = false;
    }
    state.insert = Ord::min(state.strstart, STD_MIN_MATCH - 1);
    if flush == DeflateFlush::Finish {
        flush_block!(stream, true);
        return BlockState::FinishDone;
    }
    if !stream.state.sym_buf.is_empty() {
        flush_block!(stream, false);
    }
    BlockState::BlockDone
}
