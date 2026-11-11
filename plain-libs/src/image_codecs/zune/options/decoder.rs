use crate::image_codecs::zune::bit_depth::ByteEndian;
use crate::image_codecs::zune::colorspace::ColorSpace;
fn decoder_error_tolerance_mode() -> DecoderFlags {
    fast_options()
}
fn fast_options() -> DecoderFlags {
    DecoderFlags {
        inflate_confirm_adler: false,
        png_confirm_crc: false,
        jpg_error_on_non_conformance: false,
        zune_use_unsafe: true,
        zune_use_neon: true,
        zune_use_avx: true,
        zune_use_avx2: true,
        zune_use_sse2: true,
        zune_use_sse3: true,
        zune_use_sse41: true,
        png_add_alpha_channel: false,
        png_strip_16_bit_to_8_bit: false,
        png_decode_animated: true,
        jxl_decode_animated: true,
        hvec_use_videotoolbox: true,
    }
}
fn cmd_options() -> DecoderFlags {
    DecoderFlags {
        inflate_confirm_adler: false,
        png_confirm_crc: false,
        jpg_error_on_non_conformance: false,
        zune_use_unsafe: true,
        zune_use_neon: true,
        zune_use_avx: true,
        zune_use_avx2: true,
        zune_use_sse2: true,
        zune_use_sse3: true,
        zune_use_sse41: true,
        png_add_alpha_channel: false,
        png_strip_16_bit_to_8_bit: false,
        png_decode_animated: true,
        jxl_decode_animated: true,
        hvec_use_videotoolbox: true,
    }
}
#[derive(Copy, Debug, Clone, Default)]
pub struct DecoderFlags {
    inflate_confirm_adler: bool,
    png_confirm_crc: bool,
    jpg_error_on_non_conformance: bool,
    zune_use_unsafe: bool,
    zune_use_sse2: bool,
    zune_use_sse3: bool,
    zune_use_sse41: bool,
    zune_use_avx: bool,
    zune_use_avx2: bool,
    png_add_alpha_channel: bool,
    zune_use_neon: bool,
    png_strip_16_bit_to_8_bit: bool,
    png_decode_animated: bool,
    jxl_decode_animated: bool,
    hvec_use_videotoolbox: bool,
}
#[derive(Debug, Copy, Clone)]
pub struct DecoderOptions {
    max_width: usize,
    max_height: usize,
    out_colorspace: ColorSpace,
    max_scans: usize,
    deflate_limit: usize,
    flags: DecoderFlags,
    endianness: ByteEndian,
    hevc_max_mdat_size: usize,
    num_threads: u8,
}
impl DecoderOptions {}
impl DecoderOptions {
    pub const fn max_width(&self) -> usize {
        self.max_width
    }
    pub const fn max_height(&self) -> usize {
        self.max_height
    }
    pub fn strict_mode(&self) -> bool {
        self.flags.jpg_error_on_non_conformance
            | self.flags.png_confirm_crc
            | self.flags.inflate_confirm_adler
    }

    pub fn set_max_width(mut self, width: usize) -> Self {
        self.max_width = width;
        self
    }
    pub fn set_max_height(mut self, height: usize) -> Self {
        self.max_height = height;
        self
    }

    fn set_decoder_flags(mut self, flags: DecoderFlags) -> Self {
        self.flags = flags;
        self
    }
    pub fn set_strict_mode(mut self, yes: bool) -> Self {
        self.flags.jpg_error_on_non_conformance = yes;
        self.flags.png_confirm_crc = yes;
        self.flags.inflate_confirm_adler = yes;
        self
    }

    pub const fn num_threads(&self) -> u8 {
        self.num_threads
    }
}
impl DecoderOptions {
    pub const fn png_decode_animated(&self) -> bool {
        self.flags.png_decode_animated
    }
}
impl DecoderOptions {
    pub const fn jpeg_get_max_scans(&self) -> usize {
        self.max_scans
    }

    pub const fn jpeg_get_out_colorspace(&self) -> ColorSpace {
        self.out_colorspace
    }
    #[must_use]
    pub fn jpeg_set_out_colorspace(mut self, colorspace: ColorSpace) -> Self {
        self.out_colorspace = colorspace;
        self
    }
}
impl DecoderOptions {
    pub fn use_avx2(&self) -> bool {
        let opt = self.flags.zune_use_avx2 | self.flags.zune_use_unsafe;
        if !opt {
            return false;
        }
        #[cfg(any(target_arch = "x86_64", target_arch = "x86"))]
        {
            {
                if is_x86_feature_detected!("avx2") {
                    return true;
                }
            }
        }
        false
    }
    pub fn use_neon(&self) -> bool {
        let opt = self.flags.zune_use_neon | self.flags.zune_use_unsafe;
        if !opt {
            return false;
        }
        #[cfg(target_arch = "aarch64")]
        {
            return true;
        }
        false
    }
}
impl DecoderOptions {
    pub const fn jxl_decode_animated(&self) -> bool {
        self.flags.jxl_decode_animated
    }
}
impl DecoderOptions {
    pub const fn hevc_max_mdat_size(&self) -> usize {
        self.hevc_max_mdat_size
    }
}
impl Default for DecoderOptions {
    fn default() -> Self {
        Self {
            out_colorspace: ColorSpace::RGB,
            max_width: 1 << 14,
            max_height: 1 << 14,
            max_scans: 100,
            deflate_limit: 1 << 30,
            flags: decoder_error_tolerance_mode(),
            hevc_max_mdat_size: 1 << 24,
            num_threads: 4,
            endianness: ByteEndian::BE,
        }
    }
}
