#![forbid(unsafe_code)]
use super::flush_block;
use crate::compression::backend::{
    DeflateFlush,
    deflate::{
        BlockState, DeflateStream, MIN_LOOKAHEAD, STD_MAX_MATCH, STD_MIN_MATCH,
        compare256::compare256_rle_slice, fill_window,
    },
};
pub fn deflate_rle(stream: &mut DeflateStream, flush: DeflateFlush) -> BlockState {
    let mut match_len = 0;
    let mut bflush;
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
        if state.lookahead >= STD_MIN_MATCH && state.strstart > 0 {
            let scan = &state.window.filled()[state.strstart - 1..][..3 + 256];
            {
                if scan[0] == scan[1] && scan[1] == scan[2] {
                    match_len = compare256_rle_slice(scan[0], &scan[3..]) + 2;
                    match_len = Ord::min(match_len, state.lookahead);
                    match_len = Ord::min(match_len, STD_MAX_MATCH);
                }
            }
            assert!(
                state.strstart - 1 + match_len < state.window_size,
                "wild scan"
            );
        }
        if match_len >= STD_MIN_MATCH {
            bflush = state.tally_dist(1, match_len - STD_MIN_MATCH);
            state.lookahead -= match_len;
            state.strstart += match_len;
            match_len = 0;
        } else {
            let lc = state.window.filled()[state.strstart];
            bflush = state.tally_lit(lc);
            state.lookahead -= 1;
            state.strstart += 1;
        }
        if bflush {
            flush_block!(stream, false);
        }
    }
    stream.state.insert = 0;
    if flush == DeflateFlush::Finish {
        flush_block!(stream, true);
        return BlockState::FinishDone;
    }
    if !stream.state.sym_buf.is_empty() {
        flush_block!(stream, false);
    }
    BlockState::BlockDone
}
