#![forbid(unsafe_code)]
mod compress;
mod decompress;
mod huffman;
mod tables;
pub use compress::{Compressor, StoredOnlyCompressor, compress_to_vec};
pub use decompress::{
    BoundedDecompressionError, DecompressionError, Decompressor, decompress_to_vec,
    decompress_to_vec_bounded,
};

const fn compute_codes<const NSYMS: usize>(lengths: &[u8; NSYMS]) -> Option<[u16; NSYMS]> {
    let mut codes = [0u16; NSYMS];
    let mut code = 0u32;
    let mut len = 1;
    while len <= 16 {
        let mut i = 0;
        while i < lengths.len() {
            if lengths[i] == len {
                codes[i] = (code as u16).reverse_bits() >> (16 - len);
                code += 1;
            }
            i += 1;
        }
        code <<= 1;
        len += 1;
    }
    if code == 2 << 16 { Some(codes) } else { None }
}
