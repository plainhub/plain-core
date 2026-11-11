#![forbid(unsafe_code)]
use super::flush_block;
use crate::compression::backend::{
    DeflateFlush,
    deflate::{BlockState, DeflateStream, fill_window},
};
pub fn deflate_huff(stream: &mut DeflateStream, flush: DeflateFlush) -> BlockState {
    loop {
        if stream.state.lookahead == 0 {
            fill_window(stream);
            if stream.state.lookahead == 0 {
                match flush {
                    DeflateFlush::NoFlush => return BlockState::NeedMore,
                    _ => break,
                }
            }
        }
        let state = &mut stream.state;
        let lc = state.window.filled()[state.strstart];
        let bflush = state.tally_lit(lc);
        state.lookahead -= 1;
        state.strstart += 1;
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
