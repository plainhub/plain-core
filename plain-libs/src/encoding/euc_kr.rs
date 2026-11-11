use super::*;
use crate::encoding::data::*;
use crate::encoding::handles::*;
use crate::encoding::variant::*;
pub struct EucKrDecoder {
    lead: Option<u8>,
}
impl EucKrDecoder {
    pub fn new() -> VariantDecoder {
        VariantDecoder::EucKr(EucKrDecoder { lead: None })
    }

    fn plus_one_if_lead(&self, byte_length: usize) -> Option<usize> {
        byte_length.checked_add(match self.lead {
            None => 0,
            Some(_) => 1,
        })
    }
    pub fn max_utf8_buffer_length_without_replacement(&self, byte_length: usize) -> Option<usize> {
        let len = self.plus_one_if_lead(byte_length);
        checked_add(2, checked_add_opt(len, checked_div(checked_add(1, len), 2)))
    }
    pub fn max_utf8_buffer_length(&self, byte_length: usize) -> Option<usize> {
        checked_mul(3, self.plus_one_if_lead(byte_length))
    }
    ascii_compatible_two_byte_decoder_functions ! (lead = { let non_ascii_minus_offset = non_ascii . wrapping_sub (0x81) ; if non_ascii_minus_offset > (0xFE - 0x81) { return (DecoderResult :: Malformed (1 , 0) , source . consumed () , handle . written ()) ; } non_ascii_minus_offset } , trail = { if lead_minus_offset >= 0x20 { let trail_minus_offset = byte . wrapping_sub (0xA1) ; if trail_minus_offset <= (0xFE - 0xA1) { let ksx_pointer = mul_94 (lead_minus_offset - 0x20) + trail_minus_offset as usize ; let hangul_pointer = ksx_pointer . wrapping_sub ((0x2F - 0x20) * 94) ; if hangul_pointer < KSX1001_HANGUL . len () { let upper_bmp = KSX1001_HANGUL [hangul_pointer] ; handle . write_upper_bmp (upper_bmp) } else if ksx_pointer < KSX1001_SYMBOLS . len () { let bmp = KSX1001_SYMBOLS [ksx_pointer] ; handle . write_bmp_excl_ascii (bmp) } else { let hanja_pointer = ksx_pointer . wrapping_sub ((0x49 - 0x20) * 94) ; if hanja_pointer < KSX1001_HANJA . len () { let upper_bmp = KSX1001_HANJA [hanja_pointer] ; handle . write_upper_bmp (upper_bmp) } else if (lead_minus_offset == 0x27) && ((trail_minus_offset as usize) < KSX1001_UPPERCASE . len ()) { let mid_bmp = KSX1001_UPPERCASE [trail_minus_offset as usize] ; if mid_bmp == 0 { return (DecoderResult :: Malformed (2 , 0) , unread_handle_trail . consumed () , handle . written ()) ; } handle . write_mid_bmp (mid_bmp) } else if (lead_minus_offset == 0x28) && ((trail_minus_offset as usize) < KSX1001_LOWERCASE . len ()) { let mid_bmp = KSX1001_LOWERCASE [trail_minus_offset as usize] ; handle . write_mid_bmp (mid_bmp) } else if (lead_minus_offset == 0x25) && ((trail_minus_offset as usize) < KSX1001_BOX . len ()) { let upper_bmp = KSX1001_BOX [trail_minus_offset as usize] ; handle . write_upper_bmp (upper_bmp) } else { let other_pointer = ksx_pointer . wrapping_sub (2 * 94) ; if other_pointer < 0x039F { let bmp = ksx1001_other_decode (other_pointer as u16) ; if bmp < 0x80 { return (DecoderResult :: Malformed (2 , 0) , unread_handle_trail . consumed () , handle . written ()) ; } handle . write_bmp_excl_ascii (bmp) } else { return (DecoderResult :: Malformed (2 , 0) , unread_handle_trail . consumed () , handle . written ()) ; } } } } else { let left_lead = lead_minus_offset - 0x20 ; let left_trail = if byte . wrapping_sub (0x40 + 0x41) < (0x60 - 0x40) { byte - (12 + 0x41) } else if byte . wrapping_sub (0x20 + 0x41) < (0x3A - 0x20) { byte - (6 + 0x41) } else if byte . wrapping_sub (0x41) < 0x1A { byte - 0x41 } else { if byte < 0x80 { return (DecoderResult :: Malformed (1 , 0) , unread_handle_trail . unread () , handle . written ()) ; } return (DecoderResult :: Malformed (2 , 0) , unread_handle_trail . consumed () , handle . written ()) ; } ; let left_pointer = ((left_lead as usize) * (190 - 94 - 12)) + left_trail as usize ; if left_pointer < (0x45 - 0x20) * (190 - 94 - 12) + 0x12 { let upper_bmp = cp949_left_hangul_decode (left_pointer as u16) ; handle . write_upper_bmp (upper_bmp) } else { if byte < 0x80 { return (DecoderResult :: Malformed (1 , 0) , unread_handle_trail . unread () , handle . written ()) ; } return (DecoderResult :: Malformed (2 , 0) , unread_handle_trail . consumed () , handle . written ()) ; } } } else { let top_trail = if byte . wrapping_sub (0x40 + 0x41) < (0xBE - 0x40) { byte - (12 + 0x41) } else if byte . wrapping_sub (0x20 + 0x41) < (0x3A - 0x20) { byte - (6 + 0x41) } else if byte . wrapping_sub (0x41) < 0x1A { byte - 0x41 } else { if byte < 0x80 { return (DecoderResult :: Malformed (1 , 0) , unread_handle_trail . unread () , handle . written ()) ; } return (DecoderResult :: Malformed (2 , 0) , unread_handle_trail . consumed () , handle . written ()) ; } ; let top_pointer = ((lead_minus_offset as usize) * (190 - 12)) + top_trail as usize ; let upper_bmp = cp949_top_hangul_decode (top_pointer as u16) ; handle . write_upper_bmp (upper_bmp) } } , self = self , non_ascii = non_ascii , byte = byte , lead_minus_offset = lead_minus_offset , unread_handle_trail = unread_handle_trail , source = source , handle = handle , outermost = 'outermost , copy_ascii = copy_ascii_from_check_space_bmp , destination_check = check_space_bmp , ascii_punctuation = true);
}
