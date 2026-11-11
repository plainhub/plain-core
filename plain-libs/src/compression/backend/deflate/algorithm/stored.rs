use crate::compression::backend::{
    DeflateFlush,
    deflate::{
        BlockState, DeflateStream, MAX_STORED, flush_pending, read_buf_window, zng_tr_stored_block,
    },
};
pub fn deflate_stored(stream: &mut DeflateStream, flush: DeflateFlush) -> BlockState {
    let min_block = Ord::min(
        stream.state.bit_writer.pending.capacity() - 5,
        stream.state.w_size,
    );
    let mut have;
    let mut last = false;
    let mut used = stream.avail_in;
    loop {
        let mut len = MAX_STORED;
        have = ((stream.state.bit_writer.bits_valid + 42) / 8) as usize;
        if stream.avail_out < have as u32 {
            break;
        }
        let left = stream.state.strstart as isize - stream.state.block_start;
        let left = Ord::max(0, left) as usize;
        have = stream.avail_out as usize - have;
        if len > left + stream.avail_in as usize {
            len = left + stream.avail_in as usize;
        }
        len = Ord::min(len, have);
        if len < min_block
            && ((len == 0 && flush != DeflateFlush::Finish)
                || flush == DeflateFlush::NoFlush
                || len != left + stream.avail_in as usize)
        {
            break;
        }
        last = flush == DeflateFlush::Finish && len == left + stream.avail_in as usize;
        zng_tr_stored_block(stream.state, 0..0, last);
        stream.state.bit_writer.pending.rewind(4);
        stream
            .state
            .bit_writer
            .pending
            .extend(&(len as u16).to_le_bytes());
        stream
            .state
            .bit_writer
            .pending
            .extend(&(!len as u16).to_le_bytes());
        flush_pending(stream);
        stream.state.bit_writer.cmpr_bits_add(len << 3);
        stream.state.bit_writer.sent_bits_add(len << 3);
        if left > 0 {
            let left = Ord::min(left, len);
            let src = &stream.state.window.filled()[stream.state.block_start as usize..];
            unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), stream.next_out, left) };
            stream.next_out = stream.next_out.wrapping_add(left);
            stream.avail_out = stream.avail_out.wrapping_sub(left as _);
            stream.total_out = stream.total_out.wrapping_add(left as _);
            stream.state.block_start += left as isize;
            len -= left;
        }
        if len > 0 {
            read_buf_direct_copy(stream, len);
        }
        if last {
            break;
        }
    }
    used -= stream.avail_in;
    if used > 0 {
        let state = &mut stream.state;
        if used as usize >= state.w_size {
            state.matches = 2;
            let src = stream.next_in.wrapping_sub(state.w_size);
            unsafe { state.window.copy_and_initialize(0..state.w_size, src) };
            state.strstart = state.w_size;
            state.insert = state.strstart;
        } else {
            if state.window_size - state.strstart <= used as usize {
                state.strstart -= state.w_size;
                let copy = Ord::min(state.strstart, state.window.filled().len() - state.w_size);
                state
                    .window
                    .filled_mut()
                    .copy_within(state.w_size..state.w_size + copy, 0);
                if state.matches < 2 {
                    state.matches += 1;
                }
                state.insert = Ord::min(state.insert, state.strstart);
            }
            let src = stream.next_in.wrapping_sub(used as usize);
            let dst = state.strstart..state.strstart + used as usize;
            unsafe { state.window.copy_and_initialize(dst, src) };
            state.strstart += used as usize;
            state.insert += Ord::min(used as usize, state.w_size - state.insert);
        }
        state.block_start = state.strstart as isize;
    }
    if last {
        stream.state.bit_writer.bits_used = 8;
        return BlockState::FinishDone;
    }
    if flush != DeflateFlush::NoFlush
        && flush != DeflateFlush::Finish
        && stream.avail_in == 0
        && stream.state.strstart as isize == stream.state.block_start
    {
        return BlockState::BlockDone;
    }
    let mut have = stream.state.window_size - stream.state.strstart;
    if stream.avail_in as usize > have && stream.state.block_start >= stream.state.w_size as isize {
        let state = &mut stream.state;
        state.block_start -= state.w_size as isize;
        state.strstart -= state.w_size;
        let copy = Ord::min(state.strstart, state.window.filled().len() - state.w_size);
        state
            .window
            .filled_mut()
            .copy_within(state.w_size..state.w_size + copy, 0);
        if state.matches < 2 {
            state.matches += 1;
        }
        have += state.w_size;
        state.insert = Ord::min(state.insert, state.strstart);
    }
    let have = Ord::min(have, stream.avail_in as usize);
    if have > 0 {
        read_buf_window(stream, stream.state.strstart, have);
        let state = &mut stream.state;
        state.strstart += have;
        state.insert += Ord::min(have, state.w_size - state.insert);
    }
    let state = &mut stream.state;
    let have = ((state.bit_writer.bits_valid + 42) >> 3) as usize;
    let have = Ord::min(state.bit_writer.pending.capacity() - have, MAX_STORED);
    let min_block = Ord::min(have, state.w_size);
    let left = state.strstart as isize - state.block_start;
    if left >= min_block as isize
        || ((left > 0 || flush == DeflateFlush::Finish)
            && flush != DeflateFlush::NoFlush
            && stream.avail_in == 0
            && left <= have as isize)
    {
        let len = Ord::min(left as usize, have);
        last = flush == DeflateFlush::Finish && stream.avail_in == 0 && len == (left as usize);
        let range = state.block_start as usize..state.block_start as usize + len;
        zng_tr_stored_block(state, range, last);
        state.block_start += len as isize;
        flush_pending(stream);
    }
    if last {
        stream.state.bit_writer.bits_used = 8;
        BlockState::FinishStarted
    } else {
        BlockState::NeedMore
    }
}
fn read_buf_direct_copy(stream: &mut DeflateStream, size: usize) -> usize {
    let len = Ord::min(stream.avail_in as usize, size);
    let output = stream.next_out;
    if len == 0 {
        return 0;
    }
    stream.avail_in -= len as u32;
    if stream.state.wrap == 2 {
        unsafe { core::ptr::copy_nonoverlapping(stream.next_in, output, len) }
        let data = unsafe { core::slice::from_raw_parts(output, len) };
        stream.state.crc_fold.fold(data, 0);
    } else if stream.state.wrap == 1 {
        unsafe { core::ptr::copy_nonoverlapping(stream.next_in, output, len) }
        let data = unsafe { core::slice::from_raw_parts(output, len) };
        stream.adler = crate::checksum::adler32::adler32(stream.adler as u32, data) as _;
    } else {
        unsafe { core::ptr::copy_nonoverlapping(stream.next_in, output, len) }
    }
    stream.next_in = stream.next_in.wrapping_add(len);
    stream.total_in += len as crate::compression::backend::c_api::ZSize;
    stream.next_out = stream.next_out.wrapping_add(len as _);
    stream.avail_out = stream.avail_out.wrapping_sub(len as _);
    stream.total_out = stream.total_out.wrapping_add(len as _);
    len
}
