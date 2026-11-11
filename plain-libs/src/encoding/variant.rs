use super::*;
use big5::*;
use euc_kr::*;
use gb18030::*;
use shift_jis::*;
use single_byte::*;
use utf_8::*;
use utf_16::*;
pub enum VariantDecoder {
    SingleByte(SingleByteDecoder),
    Utf8(Utf8Decoder),
    Gb18030(Gb18030Decoder),
    Big5(Big5Decoder),
    ShiftJis(ShiftJisDecoder),
    EucKr(EucKrDecoder),
    Utf16(Utf16Decoder),
}
impl VariantDecoder {
    pub fn max_utf8_buffer_length_without_replacement(&self, byte_length: usize) -> Option<usize> {
        match *self {
            VariantDecoder::SingleByte(ref v) => {
                v.max_utf8_buffer_length_without_replacement(byte_length)
            }
            VariantDecoder::Utf8(ref v) => {
                v.max_utf8_buffer_length_without_replacement(byte_length)
            }
            VariantDecoder::Gb18030(ref v) => {
                v.max_utf8_buffer_length_without_replacement(byte_length)
            }
            VariantDecoder::Big5(ref v) => {
                v.max_utf8_buffer_length_without_replacement(byte_length)
            }
            VariantDecoder::ShiftJis(ref v) => {
                v.max_utf8_buffer_length_without_replacement(byte_length)
            }
            VariantDecoder::EucKr(ref v) => {
                v.max_utf8_buffer_length_without_replacement(byte_length)
            }
            VariantDecoder::Utf16(ref v) => {
                v.max_utf8_buffer_length_without_replacement(byte_length)
            }
        }
    }
    pub fn max_utf8_buffer_length(&self, byte_length: usize) -> Option<usize> {
        match *self {
            VariantDecoder::SingleByte(ref v) => v.max_utf8_buffer_length(byte_length),
            VariantDecoder::Utf8(ref v) => v.max_utf8_buffer_length(byte_length),
            VariantDecoder::Gb18030(ref v) => v.max_utf8_buffer_length(byte_length),
            VariantDecoder::Big5(ref v) => v.max_utf8_buffer_length(byte_length),
            VariantDecoder::ShiftJis(ref v) => v.max_utf8_buffer_length(byte_length),
            VariantDecoder::EucKr(ref v) => v.max_utf8_buffer_length(byte_length),
            VariantDecoder::Utf16(ref v) => v.max_utf8_buffer_length(byte_length),
        }
    }
    pub fn decode_to_utf8_raw(
        &mut self,
        src: &[u8],
        dst: &mut [u8],
        last: bool,
    ) -> (DecoderResult, usize, usize) {
        match *self {
            VariantDecoder::SingleByte(ref mut v) => v.decode_to_utf8_raw(src, dst, last),
            VariantDecoder::Utf8(ref mut v) => v.decode_to_utf8_raw(src, dst, last),
            VariantDecoder::Gb18030(ref mut v) => v.decode_to_utf8_raw(src, dst, last),
            VariantDecoder::Big5(ref mut v) => v.decode_to_utf8_raw(src, dst, last),
            VariantDecoder::ShiftJis(ref mut v) => v.decode_to_utf8_raw(src, dst, last),
            VariantDecoder::EucKr(ref mut v) => v.decode_to_utf8_raw(src, dst, last),
            VariantDecoder::Utf16(ref mut v) => v.decode_to_utf8_raw(src, dst, last),
        }
    }
}
pub enum VariantEncoding {
    SingleByte(&'static [u16; 128]),
    Utf8,
    Gbk,
    Big5,
    ShiftJis,
    EucKr,
    Utf16Be,
    Utf16Le,
}
impl VariantEncoding {
    pub fn new_variant_decoder(&self) -> VariantDecoder {
        match *self {
            VariantEncoding::SingleByte(table) => SingleByteDecoder::new(table),
            VariantEncoding::Utf8 => Utf8Decoder::new(),
            VariantEncoding::Gbk => Gb18030Decoder::new(),
            VariantEncoding::Big5 => Big5Decoder::new(),
            VariantEncoding::ShiftJis => ShiftJisDecoder::new(),
            VariantEncoding::EucKr => EucKrDecoder::new(),
            VariantEncoding::Utf16Be => Utf16Decoder::new(true),
            VariantEncoding::Utf16Le => Utf16Decoder::new(false),
        }
    }
}
