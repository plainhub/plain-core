use crate::image_codecs::jpeg::color_convert::choose_ycbcr_to_rgb_convert_func;
use crate::image_codecs::jpeg::components::{Components, SampleRatios};
use crate::image_codecs::jpeg::errors::{DecodeErrors, UnsupportedSchemes};
use crate::image_codecs::jpeg::headers::{
    parse_app1, parse_app2, parse_app13, parse_app14, parse_dqt, parse_huffman, parse_sos,
    parse_start_of_frame,
};
use crate::image_codecs::jpeg::huffman::HuffmanTable;
use crate::image_codecs::jpeg::idct::{
    choose_idct_1x1_func, choose_idct_4x4_func, choose_idct_func,
};
use crate::image_codecs::jpeg::marker::Marker;
use crate::image_codecs::jpeg::misc::SOFMarkers;
use crate::image_codecs::jpeg::upsampler::{
    choose_horizontal_samp_function, choose_hv_samp_function, choose_v_samp_function,
    generic_sampler, upsample_no_op,
};
use crate::image_codecs::zune::bytestream::{ZByteReaderTrait, ZReader};
use crate::image_codecs::zune::colorspace::ColorSpace;
use crate::image_codecs::zune::log::{error, trace, warn};
use crate::image_codecs::zune::options::DecoderOptions;
use std::string::ToString;
use std::vec::Vec;
use std::{format, vec};
pub(crate) const MAX_COMPONENTS: usize = 4;
pub(crate) const MAX_DIMENSIONS: usize = 1 << 27;
pub type ColorConvert16Ptr = fn(&[i16; 16], &[i16; 16], &[i16; 16], &mut [u8], &mut usize);
pub type IDCTPtr = fn(&mut [i32; 64], &mut [i16], usize);
pub(crate) struct ICCChunk {
    pub(crate) seq_no: u8,
    pub(crate) num_markers: u8,
    pub(crate) data: Vec<u8>,
}
pub struct JpegDecoder<T> {
    pub(crate) info: ImageInfo,
    pub(crate) qt_tables: [Option<[i32; 64]>; MAX_COMPONENTS],
    pub(crate) dc_huffman_tables: [Option<HuffmanTable>; MAX_COMPONENTS],
    pub(crate) ac_huffman_tables: [Option<HuffmanTable>; MAX_COMPONENTS],
    pub(crate) components: Vec<Components>,
    pub(crate) h_max: usize,
    pub(crate) v_max: usize,
    pub(crate) mcu_width: usize,
    pub(crate) mcu_height: usize,
    pub(crate) mcu_x: usize,
    pub(crate) mcu_y: usize,
    pub(crate) is_interleaved: bool,
    pub(crate) input_colorspace: ColorSpace,
    pub(crate) is_progressive: bool,
    pub(crate) spec_start: u8,
    pub(crate) spec_end: u8,
    pub(crate) succ_high: u8,
    pub(crate) succ_low: u8,
    pub(crate) num_scans: u8,
    pub(crate) scan_subsampled: bool,
    pub(crate) idct_func: IDCTPtr,
    pub(crate) idct_4x4_func: IDCTPtr,
    pub(crate) idct_1x1_func: IDCTPtr,
    pub(crate) color_convert_16: ColorConvert16Ptr,
    pub(crate) z_order: [usize; MAX_COMPONENTS],
    pub(crate) restart_interval: usize,
    pub(crate) todo: usize,
    pub(crate) options: DecoderOptions,
    pub(crate) stream: ZReader<T>,
    pub(crate) headers_decoded: bool,
    pub(crate) seen_sof: bool,
    pub(crate) icc_data: Vec<ICCChunk>,
    pub(crate) is_mjpeg: bool,
    pub(crate) coeff: usize,
    pub(crate) extended_xmp_segments: Vec<ExtendedXmpSegment>,
}
impl<T> JpegDecoder<T>
where
    T: ZByteReaderTrait,
{
    fn default(options: DecoderOptions, buffer: T) -> Self {
        let color_convert = choose_ycbcr_to_rgb_convert_func(ColorSpace::RGB, &options).unwrap();
        JpegDecoder {
            info: ImageInfo::default(),
            qt_tables: [None, None, None, None],
            dc_huffman_tables: [None, None, None, None],
            ac_huffman_tables: [None, None, None, None],
            components: vec![],
            h_max: 1,
            v_max: 1,
            mcu_height: 0,
            mcu_width: 0,
            mcu_x: 0,
            mcu_y: 0,
            is_interleaved: false,
            is_progressive: false,
            spec_start: 0,
            spec_end: 0,
            succ_high: 0,
            succ_low: 0,
            num_scans: 0,
            scan_subsampled: false,
            idct_func: choose_idct_func(&options),
            idct_4x4_func: choose_idct_4x4_func(&options),
            idct_1x1_func: choose_idct_1x1_func(&options),
            color_convert_16: color_convert,
            input_colorspace: ColorSpace::YCbCr,
            z_order: [0; MAX_COMPONENTS],
            restart_interval: 0,
            todo: 0x7fff_ffff,
            options: options,
            stream: ZReader::new(buffer),
            headers_decoded: false,
            seen_sof: false,
            icc_data: vec![],
            is_mjpeg: false,
            coeff: 1,
            extended_xmp_segments: vec![],
        }
    }
    pub fn decode(&mut self) -> Result<Vec<u8>, DecodeErrors> {
        self.decode_headers()?;
        let size = self.output_buffer_size().unwrap();
        let mut out = vec![0; size];
        self.decode_into(&mut out)?;
        Ok(out)
    }
    #[must_use]
    pub fn new(stream: T) -> JpegDecoder<T> {
        JpegDecoder::default(DecoderOptions::default(), stream)
    }
    #[must_use]
    pub fn info(&self) -> Option<ImageInfo> {
        if !self.headers_decoded {
            return None;
        }
        return Some(self.info.clone());
    }
    #[must_use]
    pub fn output_buffer_size(&self) -> Option<usize> {
        return if self.headers_decoded {
            Some(
                usize::from(self.width())
                    .checked_mul(usize::from(self.height()))?
                    .checked_mul(self.options.jpeg_get_out_colorspace().num_components())?,
            )
        } else {
            None
        };
    }
    #[must_use]
    pub const fn options(&self) -> &DecoderOptions {
        &self.options
    }
    #[must_use]
    pub fn input_colorspace(&self) -> Option<ColorSpace> {
        return if self.headers_decoded {
            Some(self.input_colorspace)
        } else {
            None
        };
    }
    pub fn set_options(&mut self, options: DecoderOptions) {
        self.options = options;
    }
    fn reassemble_extended_xmp(&mut self) {
        if self.extended_xmp_segments.is_empty() {
            return;
        }
        self.extended_xmp_segments
            .sort_by(|a, b| a.offset.cmp(&b.offset));
        let guid = &self.extended_xmp_segments[0].guid;
        let total_size = self.extended_xmp_segments[0].total_size;
        for segment in &self.extended_xmp_segments {
            if &segment.guid != guid || segment.total_size != total_size {
                error!("Inconsistent Extended XMP segments");
                self.extended_xmp_segments.clear();
                return;
            }
        }
        let mut rolling_offset = 0;
        let mut complete = true;
        for segment in &self.extended_xmp_segments {
            if segment.offset != rolling_offset {
                complete = false;
                break;
            }
            rolling_offset += segment.data.len() as u32;
        }
        if complete && rolling_offset == total_size {
            let mut result = Vec::with_capacity(total_size as usize);
            for segment in &self.extended_xmp_segments {
                result.extend_from_slice(&segment.data);
            }
            self.info.extended_xmp = Some(result);
            self.info.extended_xmp_guid = Some(guid.clone());
            self.extended_xmp_segments.clear();
        } else if rolling_offset > total_size {
            error!("Extended XMP overflow");
            self.extended_xmp_segments.clear();
        }
    }
    fn decode_headers_internal(&mut self) -> Result<(), DecodeErrors> {
        if self.headers_decoded {
            trace!("Headers decoded!");
            return Ok(());
        }
        let out_colorspace = self.options.jpeg_get_out_colorspace();
        if matches!(
            out_colorspace,
            ColorSpace::BGR | ColorSpace::BGRA | ColorSpace::RGB | ColorSpace::RGBA
        ) {
            self.color_convert_16 = choose_ycbcr_to_rgb_convert_func(
                self.options.jpeg_get_out_colorspace(),
                &self.options,
            )
            .unwrap();
        }
        let magic_bytes = self.stream.get_u16_be_err()?;
        let mut last_byte = 0;
        let mut bytes_before_marker = 0;
        if magic_bytes != 0xffd8 {
            return Err(DecodeErrors::IllegalMagicBytes(magic_bytes));
        }
        loop {
            let mut m = self.stream.read_u8_err()?;
            if (m == 0xFF || m == 0) && last_byte == 0xFF {
                while m == 0xFF || m == 0x0 {
                    last_byte = m;
                    m = self.stream.read_u8_err()?;
                }
            }
            if last_byte == 0xFF {
                let marker = Marker::from_u8(m);
                if let Some(n) = marker {
                    if bytes_before_marker > 3 {
                        if self.options.strict_mode() {
                            return Err(DecodeErrors::FormatStatic(
                                "[strict-mode]: Extra bytes between headers",
                            ));
                        }
                        error!(
                            "Extra bytes {} before marker 0xFF{:X}",
                            bytes_before_marker - 3,
                            m
                        );
                    }
                    bytes_before_marker = 0;
                    self.parse_marker_inner(n)?;
                    if !self.extended_xmp_segments.is_empty() {
                        self.reassemble_extended_xmp();
                    }
                    if n == Marker::SOS {
                        self.headers_decoded = true;
                        trace!("Input colorspace {:?}", self.input_colorspace);
                        let mut is_rgb = self.components.len() == 3;
                        let chars = ['R', 'G', 'B'];
                        for (comp, single_char) in self.components.iter().zip(chars.iter()) {
                            is_rgb &= comp.id == (*single_char) as u8
                        }
                        if is_rgb {
                            self.input_colorspace = ColorSpace::RGB;
                        }
                        return Ok(());
                    }
                } else {
                    bytes_before_marker = 0;
                    warn!("Marker 0xFF{:X} not known", m);
                    let length = self.stream.get_u16_be_err()?;
                    if length < 2 {
                        return Err(DecodeErrors::Format(format!(
                            "Found a marker with invalid length : {length}"
                        )));
                    }
                    warn!("Skipping {} bytes", length - 2);
                    self.stream.skip((length - 2) as usize)?;
                }
            }
            last_byte = m;
            bytes_before_marker += 1;
        }
    }
    pub(crate) fn parse_marker_inner(&mut self, m: Marker) -> Result<(), DecodeErrors> {
        match m {
            Marker::SOF(0..=2) => {
                let marker = {
                    if m == Marker::SOF(0) || m == Marker::SOF(1) {
                        SOFMarkers::BaselineDct
                    } else {
                        self.is_progressive = true;
                        SOFMarkers::ProgressiveDctHuffman
                    }
                };
                trace!("Image encoding scheme =`{:?}`", marker);
                parse_start_of_frame(marker, self)?;
            }
            Marker::SOF(v) => {
                let feature = UnsupportedSchemes::from_int(v);
                if let Some(feature) = feature {
                    return Err(DecodeErrors::Unsupported(feature));
                }
                return Err(DecodeErrors::Format("Unsupported image format".to_string()));
            }
            Marker::APP(0) => {
                let mut length = self.stream.get_u16_be_err()?;
                if length < 2 {
                    return Err(DecodeErrors::Format(format!(
                        "Found a marker with invalid length:{length}\n"
                    )));
                }
                if length > 5 {
                    let mut buffer = [0u8; 5];
                    self.stream.read_exact_bytes(&mut buffer)?;
                    if &buffer == b"AVI1\0" {
                        self.is_mjpeg = true;
                    }
                    length -= 5;
                }
                self.stream.skip(length.saturating_sub(2) as usize)?;
            }
            Marker::APP(1) => {
                parse_app1(self)?;
            }
            Marker::APP(2) => {
                parse_app2(self)?;
            }
            Marker::DQT => {
                parse_dqt(self)?;
            }
            Marker::DHT => {
                parse_huffman(self)?;
            }
            Marker::SOS => {
                parse_sos(self)?;
            }
            Marker::EOI => return Err(DecodeErrors::FormatStatic("Premature End of image")),
            Marker::DAC | Marker::DNL => {
                return Err(DecodeErrors::Format(format!(
                    "Parsing of the following header `{m:?}` is not supported,\
                                cannot continue"
                )));
            }
            Marker::DRI => {
                if self.stream.get_u16_be_err()? != 4 {
                    return Err(DecodeErrors::Format(
                        "Bad DRI length, Corrupt JPEG".to_string(),
                    ));
                }
                self.restart_interval = usize::from(self.stream.get_u16_be_err()?);
                trace!("DRI marker present ({})", self.restart_interval);
                self.todo = self.restart_interval;
            }
            Marker::APP(14) => {
                parse_app14(self)?;
            }
            Marker::APP(13) => {
                parse_app13(self)?;
            }
            _ => {
                warn!(
                    "Capabilities for processing marker \"{:?}\" not implemented",
                    m
                );
                let length = self.stream.get_u16_be_err()?;
                if length < 2 {
                    return Err(DecodeErrors::Format(format!(
                        "Found a marker with invalid length:{length}\n"
                    )));
                }
                warn!("Skipping {} bytes", length - 2);
                self.stream.skip((length - 2) as usize)?;
            }
        }
        Ok(())
    }
    #[must_use]
    pub fn icc_profile(&self) -> Option<Vec<u8>> {
        let mut marker_present: [Option<&ICCChunk>; 256] = [None; 256];
        if !self.headers_decoded {
            return None;
        }
        let num_markers = self.icc_data.len();
        if num_markers == 0 || num_markers >= 255 {
            return None;
        }
        for chunk in &self.icc_data {
            if usize::from(chunk.num_markers) != num_markers {
                return None;
            }
            if chunk.seq_no == 0 {
                warn!("Zero sequence number in ICC, corrupt ICC chunk");
                return None;
            }
            if marker_present[usize::from(chunk.seq_no)].is_some() {
                warn!("Duplicate sequence number in ICC, corrupt chunk");
                return None;
            }
            marker_present[usize::from(chunk.seq_no)] = Some(chunk);
        }
        let mut data = Vec::with_capacity(1000);
        for chunk in marker_present.get(1..=num_markers).unwrap() {
            if let Some(ch) = chunk {
                data.extend_from_slice(&ch.data);
            } else {
                warn!("Missing icc sequence number, corrupt ICC chunk ");
                return None;
            }
        }
        Some(data)
    }
    #[must_use]
    pub fn exif(&self) -> Option<&Vec<u8>> {
        return self.info.exif_data.as_ref();
    }
    pub fn xmp(&self) -> Option<&Vec<u8>> {
        return self.info.xmp_data.as_ref();
    }
    #[must_use]
    pub fn iptc(&self) -> Option<&Vec<u8>> {
        return self.info.iptc_data.as_ref();
    }
    #[must_use]
    pub fn output_colorspace(&self) -> Option<ColorSpace> {
        return if self.headers_decoded {
            Some(self.options.jpeg_get_out_colorspace())
        } else {
            None
        };
    }
    pub fn decode_into(&mut self, out: &mut [u8]) -> Result<(), DecodeErrors> {
        self.decode_headers_internal()?;
        let expected_size = self.output_buffer_size().unwrap();
        if out.len() < expected_size {
            return Err(DecodeErrors::TooSmallOutput(expected_size, out.len()));
        }
        let out_len = core::cmp::min(out.len(), expected_size);
        let out = &mut out[0..out_len];
        if self.is_progressive {
            self.decode_mcu_ycbcr_progressive(out)
        } else {
            self.decode_mcu_ycbcr_baseline(out)
        }
    }
    pub fn decode_headers(&mut self) -> Result<(), DecodeErrors> {
        self.decode_headers_internal()?;
        Ok(())
    }
    #[must_use]
    pub fn new_with_options(buf: T, options: DecoderOptions) -> JpegDecoder<T> {
        JpegDecoder::default(options, buf)
    }
    pub(crate) fn set_upsampling(&mut self) -> Result<(), DecodeErrors> {
        if self.h_max == self.v_max && self.h_max == 1 {
            return Ok(());
        }
        for comp in &mut self.components {
            let hs = self.h_max / comp.horizontal_sample;
            let vs = self.v_max / comp.vertical_sample;
            let samp_factor = match (hs, vs) {
                (1, 1) => {
                    comp.sample_ratio = SampleRatios::None;
                    upsample_no_op
                }
                (2, 1) => {
                    comp.sample_ratio = SampleRatios::H;
                    choose_horizontal_samp_function(&self.options)
                }
                (1, 2) => {
                    comp.sample_ratio = SampleRatios::V;
                    choose_v_samp_function(&self.options)
                }
                (2, 2) => {
                    comp.sample_ratio = SampleRatios::HV;
                    choose_hv_samp_function(&self.options)
                }
                (hs, vs) => {
                    comp.sample_ratio = SampleRatios::Generic(hs, vs);
                    generic_sampler()
                }
            };
            comp.setup_upsample_scanline();
            comp.up_sampler = samp_factor;
        }
        return Ok(());
    }
    #[must_use]
    pub(crate) fn width(&self) -> u16 {
        self.info.width
    }
    #[must_use]
    pub(crate) fn height(&self) -> u16 {
        self.info.height
    }
    #[must_use]
    pub const fn dimensions(&self) -> Option<(usize, usize)> {
        return if self.headers_decoded {
            Some((self.info.width as usize, self.info.height as usize))
        } else {
            None
        };
    }
}
#[derive(Default, Clone, Eq, PartialEq, Debug)]
pub struct GainMapInfo {
    pub data: Vec<u8>,
}
#[derive(Default, Clone, Eq, PartialEq, Debug)]
pub(crate) struct ExtendedXmpSegment {
    pub(crate) offset: u32,
    pub(crate) total_size: u32,
    pub(crate) guid: Vec<u8>,
    pub(crate) data: Vec<u8>,
}
#[derive(Default, Clone, Eq, PartialEq)]
pub struct ImageInfo {
    pub width: u16,
    pub height: u16,
    pub pixel_density: u8,
    pub sof: SOFMarkers,
    pub x_density: u16,
    pub y_density: u16,
    pub components: u8,
    pub gain_map_info: Vec<GainMapInfo>,
    pub multi_picture_information: Option<Vec<u8>>,
    pub exif_data: Option<Vec<u8>>,
    pub xmp_data: Option<Vec<u8>>,
    pub iptc_data: Option<Vec<u8>>,
    pub extended_xmp: Option<Vec<u8>>,
    pub extended_xmp_guid: Option<Vec<u8>>,
    pub sample_ratio: SampleRatios,
    pub multi_picture_information_offset: Option<u64>,
}
impl ImageInfo {
    pub(crate) fn set_width(&mut self, width: u16) {
        self.width = width;
    }
    pub(crate) fn set_height(&mut self, height: u16) {
        self.height = height;
    }
    pub(crate) fn set_density(&mut self, density: u8) {
        self.pixel_density = density;
    }
    pub(crate) fn set_sof_marker(&mut self, marker: SOFMarkers) {
        self.sof = marker;
    }
}
