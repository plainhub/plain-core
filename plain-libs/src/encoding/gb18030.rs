use super::*;
use crate::encoding::data::*;
use crate::encoding::handles::*;
use crate::encoding::variant::*;
enum Gb18030Pending {
    None,
    One(u8),
    Two(u8, u8),
    Three(u8, u8, u8),
}
impl Gb18030Pending {
    fn is_none(&self) -> bool {
        matches!(*self, Gb18030Pending::None)
    }
    fn count(&self) -> usize {
        match *self {
            Gb18030Pending::None => 0,
            Gb18030Pending::One(_) => 1,
            Gb18030Pending::Two(_, _) => 2,
            Gb18030Pending::Three(_, _, _) => 3,
        }
    }
}
pub struct Gb18030Decoder {
    first: Option<u8>,
    second: Option<u8>,
    third: Option<u8>,
    pending: Gb18030Pending,
    pending_ascii: Option<u8>,
}
impl Gb18030Decoder {
    pub fn new() -> VariantDecoder {
        VariantDecoder::Gb18030(Gb18030Decoder {
            first: None,
            second: None,
            third: None,
            pending: Gb18030Pending::None,
            pending_ascii: None,
        })
    }

    fn extra_from_state(&self, byte_length: usize) -> Option<usize> {
        byte_length.checked_add(
            self.pending.count()
                + match self.first {
                    None => 0,
                    Some(_) => 1,
                }
                + match self.second {
                    None => 0,
                    Some(_) => 1,
                }
                + match self.third {
                    None => 0,
                    Some(_) => 1,
                }
                + match self.pending_ascii {
                    None => 0,
                    Some(_) => 1,
                },
        )
    }
    pub fn max_utf8_buffer_length_without_replacement(&self, byte_length: usize) -> Option<usize> {
        self.max_utf8_buffer_length(byte_length)
    }
    pub fn max_utf8_buffer_length(&self, byte_length: usize) -> Option<usize> {
        checked_add(1, checked_mul(3, self.extra_from_state(byte_length)))
    }
    gb18030_decoder_functions ! ({ let non_ascii_minus_offset = non_ascii . wrapping_sub (0x81) ; if non_ascii_minus_offset > (0xFE - 0x81) { if non_ascii == 0x80 { handle . write_upper_bmp (0x20ACu16) ; continue 'outermost ; } return (DecoderResult :: Malformed (1 , 0) , source . consumed () , handle . written ()) ; } non_ascii_minus_offset } , { if first_minus_offset >= 0x20 { let trail_minus_offset = second . wrapping_sub (0xA1) ; if trail_minus_offset <= (0xFE - 0xA1) { let hanzi_lead = first_minus_offset . wrapping_sub (0x2F) ; if hanzi_lead < (0x77 - 0x2F) { let hanzi_pointer = mul_94 (hanzi_lead) + trail_minus_offset as usize ; let upper_bmp = GB2312_HANZI [hanzi_pointer] ; handle . write_upper_bmp (upper_bmp) } else if first_minus_offset == 0x20 { let bmp = GB2312_SYMBOLS [trail_minus_offset as usize] ; handle . write_bmp_excl_ascii (bmp) } else if first_minus_offset == 0x25 && ((trail_minus_offset . wrapping_sub (63) as usize) < GB2312_SYMBOLS_AFTER_GREEK . len ()) { handle . write_bmp_excl_ascii (GB2312_SYMBOLS_AFTER_GREEK [trail_minus_offset . wrapping_sub (63) as usize]) } else if first_minus_offset == 0x27 && (trail_minus_offset as usize) < GB2312_PINYIN . len () { handle . write_bmp_excl_ascii (GB2312_PINYIN [trail_minus_offset as usize]) } else if first_minus_offset > 0x76 { let pua = (0xE234 + mul_94 (first_minus_offset - 0x77) + trail_minus_offset as usize) as u16 ; handle . write_upper_bmp (pua) } else { let bmp = gb2312_other_decode ((mul_94 (first_minus_offset - 0x21) + (trail_minus_offset as usize)) as u16) ; handle . write_bmp_excl_ascii (bmp) } } else { let mut trail_minus_offset = second . wrapping_sub (0x40) ; if trail_minus_offset > (0x7E - 0x40) { let trail_minus_range_start = second . wrapping_sub (0x80) ; if trail_minus_range_start > (0xA0 - 0x80) { if second < 0x80 { return (DecoderResult :: Malformed (1 , 0) , unread_handle_second . unread () , handle . written ()) ; } return (DecoderResult :: Malformed (2 , 0) , unread_handle_second . consumed () , handle . written ()) ; } trail_minus_offset = second - 0x41 ; } let left_lead = first_minus_offset - 0x20 ; let left_pointer = left_lead as usize * (190 - 94) + trail_minus_offset as usize ; let gbk_left_ideograph_pointer = left_pointer . wrapping_sub ((0x29 - 0x20) * (190 - 94)) ; if gbk_left_ideograph_pointer < (((0x7D - 0x29) * (190 - 94)) - 5) { let upper_bmp = gbk_left_ideograph_decode (gbk_left_ideograph_pointer as u16) ; handle . write_upper_bmp (upper_bmp) } else if left_pointer < ((0x29 - 0x20) * (190 - 94)) { let bmp = gbk_other_decode (left_pointer as u16) ; handle . write_bmp_excl_ascii (bmp) } else { let bottom_pointer = left_pointer - (((0x7D - 0x20) * (190 - 94)) - 5) ; let upper_bmp = GBK_BOTTOM [bottom_pointer] ; handle . write_upper_bmp (upper_bmp) } } } else { let mut trail_minus_offset = second . wrapping_sub (0x40) ; if trail_minus_offset > (0x7E - 0x40) { let trail_minus_range_start = second . wrapping_sub (0x80) ; if trail_minus_range_start > (0xFE - 0x80) { if second < 0x80 { return (DecoderResult :: Malformed (1 , 0) , unread_handle_second . unread () , handle . written ()) ; } return (DecoderResult :: Malformed (2 , 0) , unread_handle_second . consumed () , handle . written ()) ; } trail_minus_offset = second - 0x41 ; } let pointer = first_minus_offset as usize * 190usize + trail_minus_offset as usize ; let upper_bmp = gbk_top_ideograph_decode (pointer as u16) ; handle . write_upper_bmp (upper_bmp) } } , { let third_minus_offset = third . wrapping_sub (0x81) ; if third_minus_offset > (0xFE - 0x81) { self . pending_ascii = Some (second_minus_offset + 0x30) ; return (DecoderResult :: Malformed (1 , 1) , unread_handle_third . unread () , handle . written ()) ; } third_minus_offset } , { let fourth_minus_offset = fourth . wrapping_sub (0x30) ; if fourth_minus_offset > (0x39 - 0x30) { self . pending_ascii = Some (second_minus_offset + 0x30) ; self . pending = Gb18030Pending :: One (third_minus_offset) ; return (DecoderResult :: Malformed (1 , 2) , unread_handle_fourth . unread () , handle . written ()) ; } let pointer = (first_minus_offset as usize * (10 * 126 * 10)) + (second_minus_offset as usize * (10 * 126)) + (third_minus_offset as usize * 10) + fourth_minus_offset as usize ; if pointer <= 39419 { if pointer == 7457 { handle . write_upper_bmp (0xE7C7) } else { handle . write_bmp_excl_ascii (gb18030_range_decode (pointer as u16)) } } else if (189_000 ..= 1_237_575) . contains (& pointer) { handle . write_astral ((pointer - (189_000usize - 0x1_0000usize)) as u32) } else { return (DecoderResult :: Malformed (4 , 0) , unread_handle_fourth . consumed () , handle . written ()) ; } } , self , non_ascii , first_minus_offset , second , second_minus_offset , unread_handle_second , third , third_minus_offset , unread_handle_third , fourth , fourth_minus_offset , unread_handle_fourth , source , handle , 'outermost);
}
