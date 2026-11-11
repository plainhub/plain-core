use super::*;
use crate::encoding::data::*;
use crate::encoding::handles::*;
use crate::encoding::variant::*;
pub struct Big5Decoder {
    lead: Option<u8>,
}
impl Big5Decoder {
    pub fn new() -> VariantDecoder {
        VariantDecoder::Big5(Big5Decoder { lead: None })
    }

    fn plus_one_if_lead(&self, byte_length: usize) -> Option<usize> {
        byte_length.checked_add(match self.lead {
            None => 0,
            Some(_) => 1,
        })
    }
    pub fn max_utf8_buffer_length_without_replacement(&self, byte_length: usize) -> Option<usize> {
        checked_add(2, checked_mul(2, self.plus_one_if_lead(byte_length)))
    }
    pub fn max_utf8_buffer_length(&self, byte_length: usize) -> Option<usize> {
        checked_add(3, checked_mul(3, self.plus_one_if_lead(byte_length)))
    }
    ascii_compatible_two_byte_decoder_functions ! (lead = { let non_ascii_minus_offset = non_ascii . wrapping_sub (0x81) ; if non_ascii_minus_offset > (0xFE - 0x81) { return (DecoderResult :: Malformed (1 , 0) , source . consumed () , handle . written ()) ; } non_ascii_minus_offset } , trail = { let mut trail_minus_offset = byte . wrapping_sub (0x40) ; if trail_minus_offset > (0x7E - 0x40) { let trail_minus_range_start = byte . wrapping_sub (0xA1) ; if trail_minus_range_start > (0xFE - 0xA1) { if byte < 0x80 { return (DecoderResult :: Malformed (1 , 0) , unread_handle_trail . unread () , handle . written ()) ; } return (DecoderResult :: Malformed (2 , 0) , unread_handle_trail . consumed () , handle . written ()) ; } trail_minus_offset = byte - 0x62 ; } let pointer = lead_minus_offset as usize * 157usize + trail_minus_offset as usize ; let rebased_pointer = pointer . wrapping_sub (942) ; let low_bits = big5_low_bits (rebased_pointer) ; if low_bits == 0 { match pointer { 1133 => { handle . write_big5_combination (0x00CAu16 , 0x0304u16) } 1135 => { handle . write_big5_combination (0x00CAu16 , 0x030Cu16) } 1164 => { handle . write_big5_combination (0x00EAu16 , 0x0304u16) } 1166 => { handle . write_big5_combination (0x00EAu16 , 0x030Cu16) } _ => { if byte < 0x80 { return (DecoderResult :: Malformed (1 , 0) , unread_handle_trail . unread () , handle . written ()) ; } return (DecoderResult :: Malformed (2 , 0) , unread_handle_trail . consumed () , handle . written ()) ; } } } else if big5_is_astral (rebased_pointer) { handle . write_astral (u32 :: from (low_bits) | 0x20000u32) } else { handle . write_bmp_excl_ascii (low_bits) } } , self = self , non_ascii = non_ascii , byte = byte , lead_minus_offset = lead_minus_offset , unread_handle_trail = unread_handle_trail , source = source , handle = handle , outermost = 'outermost , copy_ascii = copy_ascii_from_check_space_astral , destination_check = check_space_astral , ascii_punctuation = false);
}
