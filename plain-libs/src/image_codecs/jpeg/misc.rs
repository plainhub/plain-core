use crate::image_codecs::jpeg::JpegDecoder;
use crate::image_codecs::jpeg::components::{ComponentID, SampleRatios};
use crate::image_codecs::jpeg::errors::DecodeErrors;
use crate::image_codecs::jpeg::huffman::HuffmanTable;
use crate::image_codecs::zune::bytestream::ZByteReaderTrait;
use crate::image_codecs::zune::colorspace::ColorSpace;
use crate::image_codecs::zune::log::{trace, warn};
use core::cmp::max;
use core::fmt;
use core::num::NonZeroU32;
use std::format;
pub const START_OF_FRAME_BASE: u16 = 0xffc0;
pub const START_OF_FRAME_EXT_SEQ: u16 = 0xffc1;
pub const START_OF_FRAME_PROG_DCT: u16 = 0xffc2;
pub const START_OF_FRAME_LOS_SEQ: u16 = 0xffc3;
pub const START_OF_FRAME_EXT_AR: u16 = 0xffc9;
pub const START_OF_FRAME_PROG_DCT_AR: u16 = 0xffca;
pub const START_OF_FRAME_LOS_SEQ_AR: u16 = 0xffcb;
# [rustfmt :: skip] pub const UN_ZIGZAG : [usize ; 64 + 16] = [0 , 1 , 8 , 16 , 9 , 2 , 3 , 10 , 17 , 24 , 32 , 25 , 18 , 11 , 4 , 5 , 12 , 19 , 26 , 33 , 40 , 48 , 41 , 34 , 27 , 20 , 13 , 6 , 7 , 14 , 21 , 28 , 35 , 42 , 49 , 56 , 57 , 50 , 43 , 36 , 29 , 22 , 15 , 23 , 30 , 37 , 44 , 51 , 58 , 59 , 52 , 45 , 38 , 31 , 39 , 46 , 53 , 60 , 61 , 54 , 47 , 55 , 62 , 63 , 63 , 63 , 63 , 63 , 63 , 63 , 63 , 63 , 63 , 63 , 63 , 63 , 63 , 63 , 63 , 63] ;
#[repr(align(16))]
#[derive(Clone)]
pub struct Aligned16<T: ?Sized>(pub T);
impl<T> Default for Aligned16<T>
where
    T: Default,
{
    fn default() -> Self {
        Aligned16(T::default())
    }
}
#[repr(align(32))]
#[derive(Clone)]
pub struct Aligned32<T: ?Sized>(pub T);
impl<T> Default for Aligned32<T>
where
    T: Default,
{
    fn default() -> Self {
        Aligned32(T::default())
    }
}
#[derive(Eq, PartialEq, Copy, Clone)]
pub enum SOFMarkers {
    BaselineDct,
    ExtendedSequentialHuffman,
    ProgressiveDctHuffman,
    LosslessHuffman,
    ExtendedSequentialDctArithmetic,
    ProgressiveDctArithmetic,
    LosslessArithmetic,
}
impl Default for SOFMarkers {
    fn default() -> Self {
        Self::BaselineDct
    }
}
impl SOFMarkers {
    pub fn is_lossless(self) -> bool {
        matches!(self, Self::LosslessHuffman | Self::LosslessArithmetic)
    }
    pub fn is_progressive(self) -> bool {
        matches!(
            self,
            Self::ProgressiveDctHuffman | Self::ProgressiveDctArithmetic
        )
    }
    pub fn from_int(int: u16) -> Option<SOFMarkers> {
        match int {
            START_OF_FRAME_BASE => Some(Self::BaselineDct),
            START_OF_FRAME_PROG_DCT => Some(Self::ProgressiveDctHuffman),
            START_OF_FRAME_PROG_DCT_AR => Some(Self::ProgressiveDctArithmetic),
            START_OF_FRAME_LOS_SEQ => Some(Self::LosslessHuffman),
            START_OF_FRAME_LOS_SEQ_AR => Some(Self::LosslessArithmetic),
            START_OF_FRAME_EXT_SEQ => Some(Self::ExtendedSequentialHuffman),
            START_OF_FRAME_EXT_AR => Some(Self::ExtendedSequentialDctArithmetic),
            _ => None,
        }
    }
}
impl fmt::Debug for SOFMarkers {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match &self {
            Self::BaselineDct => write!(f, "Baseline DCT"),
            Self::ExtendedSequentialHuffman => {
                write!(f, "Extended sequential DCT, Huffman Coding")
            }
            Self::ProgressiveDctHuffman => write!(f, "Progressive DCT,Huffman Encoding"),
            Self::LosslessHuffman => write!(f, "Lossless (sequential) Huffman encoding"),
            Self::ExtendedSequentialDctArithmetic => {
                write!(f, "Extended sequential DCT, arithmetic coding")
            }
            Self::ProgressiveDctArithmetic => write!(f, "Progressive DCT, arithmetic coding"),
            Self::LosslessArithmetic => write!(f, "Lossless (sequential) arithmetic coding"),
        }
    }
}
pub(crate) fn setup_component_params<T: ZByteReaderTrait>(
    img: &mut JpegDecoder<T>,
) -> Result<(), DecodeErrors> {
    let img_width = img.width();
    let img_height = img.height();
    if img.components.len() == 3 && img.input_colorspace == ColorSpace::CMYK {
        img.input_colorspace = ColorSpace::RGB;
    }
    for component in &mut img.components {
        img.h_max = max(img.h_max, component.horizontal_sample);
        img.v_max = max(img.v_max, component.vertical_sample);
        img.mcu_width = img.h_max * 8;
        img.mcu_height = img.v_max * 8;
        img.mcu_x = usize::from(img.info.width).div_ceil(img.mcu_width);
        img.mcu_y = usize::from(img.info.height).div_ceil(img.mcu_height);
        if img.h_max != 1 || img.v_max != 1 {
            img.is_interleaved = true;
        }
        let qt_table = *img.qt_tables[component.quantization_table_number as usize]
            .as_ref()
            .ok_or_else(|| {
                DecodeErrors::DqtError(format!(
                    "No quantization table for component {:?}",
                    component.component_id
                ))
            })?;
        let x = (usize::from(img_width) * component.horizontal_sample + img.h_max - 1) / img.h_max;
        let y = (usize::from(img_height) * component.horizontal_sample + img.h_max - 1) / img.v_max;
        component.x = x;
        component.w2 = img.mcu_x * component.horizontal_sample * 8;
        component.y = y;
        component.quantization_table = qt_table;
        component.width_stride *= img.mcu_x * 8;
    }
    {
        let mut handle_that_annoying_bug = false;
        if let Some(y_component) = img
            .components
            .iter()
            .find(|c| c.component_id == ComponentID::Y)
        {
            if y_component.horizontal_sample == 2 || y_component.vertical_sample == 2 {
                handle_that_annoying_bug = true;
            }
        }
        if handle_that_annoying_bug {
            for comp in &mut img.components {
                if (comp.component_id != ComponentID::Y)
                    && (comp.horizontal_sample != 1 || comp.vertical_sample != 1)
                {
                    comp.fix_an_annoying_bug = 2;
                }
            }
        }
    }
    if img.is_mjpeg {
        fill_default_mjpeg_tables(
            img.is_progressive,
            &mut img.dc_huffman_tables,
            &mut img.ac_huffman_tables,
        );
    }
    if img.input_colorspace.num_components() > img.components.len() {
        if img.input_colorspace == ColorSpace::YCCK {
            warn!("Treating YCCK colorspace as YCbCr as component length does not match");
            img.input_colorspace = ColorSpace::YCbCr
        } else {
            if !img.options.strict_mode() {
                warn!(
                    "Expected {} number of components but found {}",
                    img.input_colorspace.num_components(),
                    img.components.len()
                );
                warn!("Defaulting to multisample to decode");
                if img.components.len() > 0 {
                    img.input_colorspace = ColorSpace::MultiBand(
                        NonZeroU32::new(img.components.len() as u32).unwrap(),
                    );
                }
            } else {
                let msg = format!(
                    "Expected {} number of components but found {}",
                    img.input_colorspace.num_components(),
                    img.components.len()
                );
                return Err(DecodeErrors::Format(msg));
            }
        }
    }
    Ok(())
}
pub fn calculate_padded_width(actual_width: usize, sub_sample: SampleRatios) -> usize {
    match sub_sample {
        SampleRatios::None | SampleRatios::V => ((actual_width + 7) / 8) * 8,
        SampleRatios::H | SampleRatios::HV => ((actual_width + 15) / 16) * 16,
        SampleRatios::Generic(h, _) => {
            ((actual_width + ((h * 8).saturating_sub(1))) / (h * 8)) * (h * 8)
        }
    }
}
pub fn fill_default_mjpeg_tables(
    is_progressive: bool,
    dc_huffman_tables: &mut [Option<HuffmanTable>],
    ac_huffman_tables: &mut [Option<HuffmanTable>],
) {
    trace!("Filling with default mjpeg tables");
    if dc_huffman_tables[0].is_none() {
        dc_huffman_tables[0] = Some(
            HuffmanTable::new_unfilled(
                &[
                    0x00, 0x00, 0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00,
                    0x00, 0x00, 0x00, 0x00,
                ],
                &[
                    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
                ],
                true,
                is_progressive,
            )
            .unwrap(),
        );
    }
    if dc_huffman_tables[1].is_none() {
        dc_huffman_tables[1] = Some(
            HuffmanTable::new_unfilled(
                &[
                    0x00, 0x00, 0x03, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00,
                    0x00, 0x00, 0x00, 0x00,
                ],
                &[
                    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
                ],
                true,
                is_progressive,
            )
            .unwrap(),
        );
    }
    if ac_huffman_tables[0].is_none() {
        ac_huffman_tables[0] = Some(
            HuffmanTable::new_unfilled(
                &[
                    0x00, 0x00, 0x02, 0x01, 0x03, 0x03, 0x02, 0x04, 0x03, 0x05, 0x05, 0x04, 0x04,
                    0x00, 0x00, 0x01, 0x7D,
                ],
                &[
                    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13,
                    0x51, 0x61, 0x07, 0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08, 0x23, 0x42,
                    0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0, 0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0A,
                    0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2A, 0x34, 0x35,
                    0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4A,
                    0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67,
                    0x68, 0x69, 0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84,
                    0x85, 0x86, 0x87, 0x88, 0x89, 0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98,
                    0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3,
                    0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5, 0xC6, 0xC7,
                    0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1,
                    0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4,
                    0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA,
                ],
                false,
                is_progressive,
            )
            .unwrap(),
        );
    }
    if ac_huffman_tables[1].is_none() {
        ac_huffman_tables[1] = Some(
            HuffmanTable::new_unfilled(
                &[
                    0x00, 0x00, 0x02, 0x01, 0x02, 0x04, 0x04, 0x03, 0x04, 0x07, 0x05, 0x04, 0x04,
                    0x00, 0x01, 0x02, 0x77,
                ],
                &[
                    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51,
                    0x07, 0x61, 0x71, 0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xA1, 0xB1,
                    0xC1, 0x09, 0x23, 0x33, 0x52, 0xF0, 0x15, 0x62, 0x72, 0xD1, 0x0A, 0x16, 0x24,
                    0x34, 0xE1, 0x25, 0xF1, 0x17, 0x18, 0x19, 0x1A, 0x26, 0x27, 0x28, 0x29, 0x2A,
                    0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
                    0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66,
                    0x67, 0x68, 0x69, 0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x82,
                    0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89, 0x8A, 0x92, 0x93, 0x94, 0x95, 0x96,
                    0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7, 0xA8, 0xA9, 0xAA,
                    0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5,
                    0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9,
                    0xDA, 0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF2, 0xF3, 0xF4,
                    0xF5, 0xF6, 0xF7, 0xF8, 0xF9, 0xFA,
                ],
                false,
                is_progressive,
            )
            .unwrap(),
        );
    }
}
