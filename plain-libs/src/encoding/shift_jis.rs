use super::in_inclusive_range;
use super::*;
use crate::encoding::data::*;
use crate::encoding::handles::*;
use crate::encoding::variant::*;
pub struct ShiftJisDecoder {
    lead: Option<u8>,
}
impl ShiftJisDecoder {
    pub fn new() -> VariantDecoder {
        VariantDecoder::ShiftJis(ShiftJisDecoder { lead: None })
    }

    fn plus_one_if_lead(&self, byte_length: usize) -> Option<usize> {
        byte_length.checked_add(match self.lead {
            None => 0,
            Some(_) => 1,
        })
    }
    pub fn max_utf8_buffer_length_without_replacement(&self, byte_length: usize) -> Option<usize> {
        self.max_utf8_buffer_length(byte_length)
    }
    pub fn max_utf8_buffer_length(&self, byte_length: usize) -> Option<usize> {
        checked_mul(3, self.plus_one_if_lead(byte_length))
    }
    ascii_compatible_two_byte_decoder_functions ! (lead = { let mut non_ascii_minus_offset = non_ascii . wrapping_sub (0x81) ; if non_ascii_minus_offset > (0x9F - 0x81) { let non_ascii_minus_range_start = non_ascii . wrapping_sub (0xE0) ; if non_ascii_minus_range_start > (0xFC - 0xE0) { let non_ascii_minus_half_with_katakana_start = non_ascii . wrapping_sub (0xA1) ; if non_ascii_minus_half_with_katakana_start > (0xDF - 0xA1) { if non_ascii == 0x80 { handle . write_mid_bmp (0x80) ; continue 'outermost ; } return (DecoderResult :: Malformed (1 , 0) , source . consumed () , handle . written ()) ; } handle . write_upper_bmp (0xFF61 + u16 :: from (non_ascii_minus_half_with_katakana_start)) ; continue 'outermost ; } non_ascii_minus_offset = non_ascii - 0xC1 ; } non_ascii_minus_offset } , trail = { let trail_minus_hiragana = byte . wrapping_sub (0x9F) ; if lead_minus_offset == 0x01 && trail_minus_hiragana < 0x53 { handle . write_upper_bmp (0x3041 + u16 :: from (trail_minus_hiragana)) } else { let mut trail_minus_offset = byte . wrapping_sub (0x40) ; if trail_minus_offset > (0x7E - 0x40) { let trail_minus_range_start = byte . wrapping_sub (0x80) ; if trail_minus_range_start > (0xFC - 0x80) { if byte < 0x80 { return (DecoderResult :: Malformed (1 , 0) , unread_handle_trail . unread () , handle . written ()) ; } return (DecoderResult :: Malformed (2 , 0) , unread_handle_trail . consumed () , handle . written ()) ; } trail_minus_offset = byte - 0x41 ; } if lead_minus_offset == 0x02 && trail_minus_offset < 0x56 { handle . write_upper_bmp (0x30A1 + u16 :: from (trail_minus_offset)) } else { let pointer = lead_minus_offset as usize * 188usize + trail_minus_offset as usize ; let level1_pointer = pointer . wrapping_sub (1410) ; if level1_pointer < JIS0208_LEVEL1_KANJI . len () { handle . write_upper_bmp (JIS0208_LEVEL1_KANJI [level1_pointer]) } else { let level2_pointer = pointer . wrapping_sub (4418) ; if level2_pointer < JIS0208_LEVEL2_AND_ADDITIONAL_KANJI . len () { handle . write_upper_bmp (JIS0208_LEVEL2_AND_ADDITIONAL_KANJI [level2_pointer]) } else { let upper_ibm_pointer = pointer . wrapping_sub (10744) ; if upper_ibm_pointer < IBM_KANJI . len () { handle . write_upper_bmp (IBM_KANJI [upper_ibm_pointer]) } else { let lower_ibm_pointer = pointer . wrapping_sub (8272) ; if lower_ibm_pointer < IBM_KANJI . len () { handle . write_upper_bmp (IBM_KANJI [lower_ibm_pointer]) } else if in_inclusive_range (pointer , 8836 , 10715) { handle . write_upper_bmp ((0xE000 - 8836 + pointer) as u16) } else if let Some (bmp) = jis0208_symbol_decode (pointer) { handle . write_bmp_excl_ascii (bmp) } else if let Some (bmp) = jis0208_range_decode (pointer) { handle . write_bmp_excl_ascii (bmp) } else { if byte < 0x80 { return (DecoderResult :: Malformed (1 , 0) , unread_handle_trail . unread () , handle . written ()) ; } return (DecoderResult :: Malformed (2 , 0) , unread_handle_trail . consumed () , handle . written ()) ; } } } } } } } , self = self , non_ascii = non_ascii , byte = byte , lead_minus_offset = lead_minus_offset , unread_handle_trail = unread_handle_trail , source = source , handle = handle , outermost = 'outermost , copy_ascii = copy_ascii_from_check_space_bmp , destination_check = check_space_bmp , ascii_punctuation = false);
}
