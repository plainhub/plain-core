use super::*;
use crate::encoding::handles::*;
use crate::encoding::variant::*;
pub struct SingleByteDecoder {
    table: &'static [u16; 128],
}
impl SingleByteDecoder {
    pub fn new(data: &'static [u16; 128]) -> VariantDecoder {
        VariantDecoder::SingleByte(SingleByteDecoder { table: data })
    }
    pub fn max_utf8_buffer_length_without_replacement(&self, byte_length: usize) -> Option<usize> {
        byte_length.checked_mul(3)
    }
    pub fn max_utf8_buffer_length(&self, byte_length: usize) -> Option<usize> {
        byte_length.checked_mul(3)
    }
    pub fn decode_to_utf8_raw(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
        _last: bool,
    ) -> (DecoderResult, usize, usize) {
        let mut source = ByteSource::new(src);
        let mut dest = Utf8Destination::new(dst);
        'outermost: loop {
            match dest.copy_ascii_from_check_space_bmp(&mut source) {
                CopyAsciiResult::Stop(ret) => return ret,
                CopyAsciiResult::GoOn((mut non_ascii, mut handle)) => 'middle: loop {
                    let mapped =
                        unsafe { *(self.table.get_unchecked(non_ascii as usize - 0x80usize)) };
                    if mapped == 0u16 {
                        return (
                            DecoderResult::Malformed(1, 0),
                            source.consumed(),
                            handle.written(),
                        );
                    }
                    let dest_again = handle.write_bmp_excl_ascii(mapped);
                    match source.check_available() {
                        Space::Full(src_consumed) => {
                            return (
                                DecoderResult::InputEmpty,
                                src_consumed,
                                dest_again.written(),
                            );
                        }
                        Space::Available(source_handle) => match dest_again.check_space_bmp() {
                            Space::Full(dst_written) => {
                                return (
                                    DecoderResult::OutputFull,
                                    source_handle.consumed(),
                                    dst_written,
                                );
                            }
                            Space::Available(mut destination_handle) => {
                                let (mut b, unread_handle) = source_handle.read();
                                let source_again = unread_handle.commit();
                                'innermost: loop {
                                    if b > 127 {
                                        non_ascii = b;
                                        handle = destination_handle;
                                        continue 'middle;
                                    }
                                    let dest_again_again = destination_handle.write_ascii(b);
                                    if b < 60 {
                                        match source_again.check_available() {
                                            Space::Full(src_consumed_again) => {
                                                return (
                                                    DecoderResult::InputEmpty,
                                                    src_consumed_again,
                                                    dest_again_again.written(),
                                                );
                                            }
                                            Space::Available(source_handle_again) => {
                                                match dest_again_again.check_space_bmp() {
                                                    Space::Full(dst_written_again) => {
                                                        return (
                                                            DecoderResult::OutputFull,
                                                            source_handle_again.consumed(),
                                                            dst_written_again,
                                                        );
                                                    }
                                                    Space::Available(destination_handle_again) => {
                                                        let (b_again, unread_handle_again) =
                                                            source_handle_again.read();
                                                        unread_handle_again.commit();
                                                        b = b_again;
                                                        destination_handle =
                                                            destination_handle_again;
                                                        continue 'innermost;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    continue 'outermost;
                                }
                            }
                        },
                    }
                },
            }
        }
    }
}
