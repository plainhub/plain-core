use super::*;
use crate::encoding::handles::*;
use crate::encoding::variant::*;
impl Utf16Decoder {
    pub fn new(big_endian: bool) -> VariantDecoder {
        VariantDecoder::Utf16(Utf16Decoder {
            lead_surrogate: 0,
            lead_byte: None,
            be: big_endian,
            pending_bmp: false,
        })
    }
    pub fn additional_from_state(&self) -> usize {
        1 + if self.lead_byte.is_some() { 1 } else { 0 }
            + if self.lead_surrogate == 0 { 0 } else { 2 }
    }
    pub fn max_utf8_buffer_length_without_replacement(&self, byte_length: usize) -> Option<usize> {
        checked_add(
            1,
            checked_mul(
                3,
                checked_div(byte_length.checked_add(self.additional_from_state()), 2),
            ),
        )
    }
    pub fn max_utf8_buffer_length(&self, byte_length: usize) -> Option<usize> {
        checked_add(
            1,
            checked_mul(
                3,
                checked_div(byte_length.checked_add(self.additional_from_state()), 2),
            ),
        )
    }
    decoder_functions!(
        preamble = {
            if self.pending_bmp {
                match dest.check_space_bmp() {
                    Space::Full(_) => {
                        return (DecoderResult::OutputFull, 0, 0);
                    }
                    Space::Available(destination_handle) => {
                        destination_handle.write_bmp(self.lead_surrogate);
                        self.pending_bmp = false;
                        self.lead_surrogate = 0;
                    }
                }
            }
        },
        loop_preamble = {
            if self.lead_byte.is_none()
                && self.lead_surrogate == 0
                && let Some((read, written)) = if self.be {
                    dest.copy_utf16_from::<BigEndian>(&mut source)
                } else {
                    dest.copy_utf16_from::<LittleEndian>(&mut source)
                }
            {
                return (DecoderResult::Malformed(2, 0), read, written);
            }
        },
        eof = {
            debug_assert!(!self.pending_bmp);
            if self.lead_surrogate != 0 || self.lead_byte.is_some() {
                match dest.check_space_bmp() {
                    Space::Full(_) => {
                        return (DecoderResult::OutputFull, 0, 0);
                    }
                    Space::Available(_) => {
                        if self.lead_surrogate != 0 {
                            self.lead_surrogate = 0;
                            match self.lead_byte {
                                None => {
                                    return (
                                        DecoderResult::Malformed(2, 0),
                                        src_consumed,
                                        dest.written(),
                                    );
                                }
                                Some(_) => {
                                    self.lead_byte = None;
                                    return (
                                        DecoderResult::Malformed(3, 0),
                                        src_consumed,
                                        dest.written(),
                                    );
                                }
                            }
                        }
                        debug_assert!(self.lead_byte.is_some());
                        self.lead_byte = None;
                        return (DecoderResult::Malformed(1, 0), src_consumed, dest.written());
                    }
                }
            }
        },
        body = {
            match self.lead_byte {
                None => {
                    self.lead_byte = Some(b);
                    continue;
                }
                Some(lead) => {
                    self.lead_byte = None;
                    let code_unit = if self.be {
                        u16::from(lead) << 8 | u16::from(b)
                    } else {
                        u16::from(b) << 8 | u16::from(lead)
                    };
                    let high_bits = code_unit & 0xFC00u16;
                    if high_bits == 0xD800u16 {
                        if self.lead_surrogate != 0 {
                            self.lead_surrogate = code_unit;
                            return (
                                DecoderResult::Malformed(2, 2),
                                unread_handle.consumed(),
                                destination_handle.written(),
                            );
                        }
                        self.lead_surrogate = code_unit;
                        continue;
                    }
                    if high_bits == 0xDC00u16 {
                        if self.lead_surrogate == 0 {
                            return (
                                DecoderResult::Malformed(2, 0),
                                unread_handle.consumed(),
                                destination_handle.written(),
                            );
                        }
                        destination_handle.write_surrogate_pair(self.lead_surrogate, code_unit);
                        self.lead_surrogate = 0;
                        continue;
                    }
                    if self.lead_surrogate != 0 {
                        self.lead_surrogate = code_unit;
                        self.pending_bmp = true;
                        return (
                            DecoderResult::Malformed(2, 2),
                            unread_handle.consumed(),
                            destination_handle.written(),
                        );
                    }
                    destination_handle.write_bmp(code_unit);
                    continue;
                }
            }
        },
        self = self,
        src_consumed = src_consumed,
        dest = dest,
        source = source,
        byte = b,
        destination_handle = destination_handle,
        unread_handle = unread_handle,
        destination_check = check_space_astral
    );
}

pub struct Utf16Decoder {
    lead_surrogate: u16,
    lead_byte: Option<u8>,
    be: bool,
    pending_bmp: bool,
}
