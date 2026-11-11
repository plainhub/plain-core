#![forbid(unsafe_code)]
use super::flush_block;
use crate::compression::backend::deflate::hash_calc::StandardHashCalc;
use crate::compression::backend::{
    DeflateFlush,
    deflate::{
        BlockState, DeflateStream, MIN_LOOKAHEAD, STD_MIN_MATCH, WANT_MIN_MATCH, fill_window,
    },
};
pub fn deflate_fast(stream: &mut DeflateStream, flush: DeflateFlush) -> BlockState {
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
        let lc = if state.lookahead >= WANT_MIN_MATCH {
            let val = u32::from_le_bytes(
                state.window.filled()[state.strstart..state.strstart + 4]
                    .try_into()
                    .unwrap(),
            );
            let hash_head = StandardHashCalc::quick_insert_value(state, state.strstart, val);
            let dist = state.strstart as isize - hash_head as isize;
            if dist <= state.max_dist() as isize && dist > 0 && hash_head != 0 {
                let mut match_len;
                (match_len, state.match_start) =
                    crate::compression::backend::deflate::longest_match::longest_match(
                        state, hash_head,
                    );
                if match_len >= WANT_MIN_MATCH {
                    let bflush = state.tally_dist(
                        state.strstart - state.match_start as usize,
                        match_len - STD_MIN_MATCH,
                    );
                    state.lookahead -= match_len;
                    if match_len <= state.max_insert_length() && state.lookahead >= WANT_MIN_MATCH {
                        match_len -= 1;
                        state.strstart += 1;
                        state.insert_string(state.strstart, match_len);
                        state.strstart += match_len;
                    } else {
                        state.strstart += match_len;
                        StandardHashCalc::quick_insert_string(
                            state,
                            state.strstart + 2 - STD_MIN_MATCH,
                        );
                    }
                    if bflush {
                        flush_block!(stream, false);
                    }
                    continue;
                }
            }
            val as u8
        } else {
            state.window.filled()[state.strstart]
        };
        let bflush = state.tally_lit(lc);
        state.lookahead -= 1;
        state.strstart += 1;
        if bflush {
            flush_block!(stream, false);
        }
    }
    stream.state.insert = if stream.state.strstart < (STD_MIN_MATCH - 1) {
        stream.state.strstart
    } else {
        STD_MIN_MATCH - 1
    };
    if flush == DeflateFlush::Finish {
        flush_block!(stream, true);
        return BlockState::FinishDone;
    }
    if !stream.state.sym_buf.is_empty() {
        flush_block!(stream, false);
    }
    BlockState::BlockDone
}
