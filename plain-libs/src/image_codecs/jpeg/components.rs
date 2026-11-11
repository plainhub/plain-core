use crate::image_codecs::jpeg::decoder::MAX_COMPONENTS;
use crate::image_codecs::jpeg::errors::DecodeErrors;
use crate::image_codecs::jpeg::upsampler::upsample_no_op;
use crate::image_codecs::zune::log::trace;
use std::string::ToString;
use std::vec::Vec;
use std::{format, vec};
const MAX_SAMP_FACTOR: usize = 4;
pub type UpSampler = fn(
    input: &[i16],
    in_near: &[i16],
    in_far: &[i16],
    scratch_space: &mut [i16],
    output: &mut [i16],
);
#[derive(Clone)]
pub(crate) struct Components {
    pub component_id: ComponentID,
    pub vertical_sample: usize,
    pub horizontal_sample: usize,
    pub dc_huff_table: usize,
    pub ac_huff_table: usize,
    pub quantization_table_number: u8,
    pub quantization_table: [i32; 64],
    pub dc_pred: i32,
    pub up_sampler: UpSampler,
    pub width_stride: usize,
    pub id: u8,
    pub needed: bool,
    pub raw_coeff: Vec<i16>,
    pub upsample_dest: Vec<i16>,
    pub row_up: Vec<i16>,
    pub row: Vec<i16>,
    pub first_row_upsample_dest: Vec<i16>,
    pub idct_pos: usize,
    pub x: usize,
    pub w2: usize,
    pub y: usize,
    pub sample_ratio: SampleRatios,
    pub fix_an_annoying_bug: usize,
}
impl Components {
    #[inline]
    pub fn from(a: [u8; 3], pos: u8) -> Result<Components, DecodeErrors> {
        let id = match pos {
            0 => ComponentID::Y,
            1 => ComponentID::Cb,
            2 => ComponentID::Cr,
            3 => ComponentID::Q,
            _ => {
                return Err(DecodeErrors::Format(format!(
                    "Unknown component id found,{pos}, expected value between 1 and 4"
                )));
            }
        };
        let horizontal_sample = (a[1] >> 4) as usize;
        let vertical_sample = (a[1] & 0x0f) as usize;
        if horizontal_sample > MAX_SAMP_FACTOR {
            return Err(DecodeErrors::Format(format!(
                "Bogus Horizontal Sampling Factor {horizontal_sample}"
            )));
        }
        if vertical_sample > MAX_SAMP_FACTOR {
            return Err(DecodeErrors::Format(format!(
                "Bogus Vertical Sampling Factor {vertical_sample}"
            )));
        }
        let quantization_table_number = a[2];
        if usize::from(quantization_table_number) >= MAX_COMPONENTS {
            return Err(DecodeErrors::Format(format!(
                "Too large quantization number :{quantization_table_number}, expected value between 0 and {MAX_COMPONENTS}"
            )));
        }
        if !horizontal_sample.is_power_of_two() {
            return Err(DecodeErrors::Format(format!(
                "Horizontal sample is not a power of two({horizontal_sample}) cannot decode"
            )));
        }
        if vertical_sample == 0 {
            return Err(DecodeErrors::Format("Vertical sample is zero".to_string()));
        }
        trace!(
            "Component ID:{:?} \tHS:{} VS:{} QT:{}",
            id, horizontal_sample, vertical_sample, quantization_table_number
        );
        Ok(Components {
            component_id: id,
            vertical_sample,
            horizontal_sample,
            quantization_table_number,
            first_row_upsample_dest: vec![],
            dc_huff_table: 0,
            ac_huff_table: 0,
            quantization_table: [0; 64],
            dc_pred: 0,
            up_sampler: upsample_no_op,
            width_stride: horizontal_sample,
            id: a[0],
            needed: true,
            raw_coeff: vec![],
            upsample_dest: vec![],
            row_up: vec![],
            row: vec![],
            idct_pos: 0,
            x: 0,
            y: 0,
            w2: 0,
            sample_ratio: SampleRatios::None,
            fix_an_annoying_bug: 1,
        })
    }
    pub fn setup_upsample_scanline(&mut self) {
        self.row = vec![0; self.width_stride * self.vertical_sample];
        self.row_up = vec![0; self.width_stride * self.vertical_sample];
        self.first_row_upsample_dest =
            vec![128; self.vertical_sample * self.width_stride * self.sample_ratio.sample()];
        self.upsample_dest =
            vec![0; self.width_stride * self.sample_ratio.sample() * self.fix_an_annoying_bug * 8];
    }
}
#[derive(Copy, Debug, Clone, PartialEq, Eq)]
pub enum ComponentID {
    Y,
    Cb,
    Cr,
    Q,
}
#[derive(Copy, Debug, Clone, PartialEq, Eq, Default)]
pub enum SampleRatios {
    HV,
    V,
    H,
    Generic(usize, usize),
    #[default]
    None,
}
impl SampleRatios {
    pub fn sample(self) -> usize {
        match self {
            SampleRatios::HV => 4,
            SampleRatios::V | SampleRatios::H => 2,
            SampleRatios::Generic(a, b) => a * b,
            SampleRatios::None => 1,
        }
    }
}
