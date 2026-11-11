use crate::h264::common::BitReader;
use crate::h264::decoder::DecodeError;
use std::borrow::ToOwned;
use std::boxed::Box;
use std::format;
use std::string::{String, ToString};
use std::vec;
use std::vec::Vec;
const HIGH_PROFILE_IDCS: &[u8] = &[100, 110, 122, 244, 44, 83, 86, 118, 128, 138, 139, 134, 135];
const MAX_FRAME_MBS: u64 = 36_864 * 4;
pub(crate) const DEFAULT_4X4_INTRA: [u8; 16] = [
    6, 13, 13, 20, 20, 20, 28, 28, 28, 28, 32, 32, 32, 37, 37, 42,
];
pub(crate) const DEFAULT_4X4_INTER: [u8; 16] = [
    10, 14, 14, 20, 20, 20, 24, 24, 24, 24, 27, 27, 27, 30, 30, 34,
];
pub(crate) const DEFAULT_8X8_INTRA: [u8; 64] = [
    6, 10, 10, 13, 11, 13, 16, 16, 16, 16, 18, 18, 18, 18, 18, 23, 23, 23, 23, 23, 23, 25, 25, 25,
    25, 25, 25, 25, 27, 27, 27, 27, 27, 27, 27, 27, 29, 29, 29, 29, 29, 29, 29, 31, 31, 31, 31, 31,
    31, 33, 33, 33, 33, 33, 36, 36, 36, 36, 38, 38, 38, 40, 40, 42,
];
pub(crate) const DEFAULT_8X8_INTER: [u8; 64] = [
    9, 13, 13, 15, 13, 15, 17, 17, 17, 17, 19, 19, 19, 19, 19, 21, 21, 21, 21, 21, 21, 22, 22, 22,
    22, 22, 22, 22, 24, 24, 24, 24, 24, 24, 24, 24, 25, 25, 25, 25, 25, 25, 25, 27, 27, 27, 27, 27,
    27, 28, 28, 28, 28, 28, 30, 30, 30, 30, 32, 32, 32, 33, 33, 35,
];
fn parse_scaling_list(r: &mut BitReader, out: &mut [u8], size: usize) -> Result<bool, DecodeError> {
    let mut last_scale = 8i32;
    let mut next_scale = 8i32;
    let mut use_default = false;
    for (j, slot) in out.iter_mut().enumerate().take(size) {
        if next_scale != 0 {
            let delta = r.read_se()?;
            next_scale = (last_scale + delta + 256).rem_euclid(256);
            if j == 0 && next_scale == 0 {
                use_default = true;
            }
        }
        let v = if next_scale == 0 {
            last_scale
        } else {
            next_scale
        };
        *slot = v as u8;
        last_scale = v;
    }
    Ok(use_default)
}
#[derive(Debug, Clone)]
pub struct Sps {
    pub profile_idc: u8,
    pub level_idc: u8,
    pub seq_parameter_set_id: u32,
    pub log2_max_frame_num: u32,
    pub pic_order_cnt_type: u32,
    pub log2_max_pic_order_cnt_lsb: u32,
    pub delta_pic_order_always_zero: bool,
    pub gaps_in_frame_num_allowed: bool,
    pub direct_8x8_inference: bool,
    pub max_num_ref_frames: u32,
    pub pic_width_in_mbs: usize,
    pub pic_height_in_mbs: usize,
    pub frame_crop_left: u32,
    pub frame_crop_right: u32,
    pub frame_crop_top: u32,
    pub frame_crop_bottom: u32,
    pub chroma_format_idc: u32,
    pub scaling_4x4: [[u8; 16]; 6],
    pub scaling_8x8: [[u8; 64]; 2],
    pub has_scaling: bool,
    pub transform_bypass: bool,
}
impl Sps {
    pub fn coded_width(&self) -> usize {
        self.pic_width_in_mbs * 16
    }
    pub fn coded_height(&self) -> usize {
        self.pic_height_in_mbs * 16
    }
    pub fn display_width(&self) -> usize {
        self.coded_width() - 2 * (self.frame_crop_left + self.frame_crop_right) as usize
    }
    pub fn display_height(&self) -> usize {
        self.coded_height() - 2 * (self.frame_crop_top + self.frame_crop_bottom) as usize
    }
    pub fn parse(rbsp: &[u8]) -> Result<Self, DecodeError> {
        let mut r = BitReader::new(rbsp);
        let profile_idc = r.read_bits(8)? as u8;
        let _constraints = r.read_bits(8)?;
        let level_idc = r.read_bits(8)? as u8;
        let seq_parameter_set_id = r.read_ue()?;
        let mut chroma_format_idc = 1u32;
        let mut transform_bypass = false;
        let mut scaling_4x4 = [[16u8; 16]; 6];
        let mut scaling_8x8 = [[16u8; 64]; 2];
        let mut has_scaling = false;
        if HIGH_PROFILE_IDCS.contains(&profile_idc) {
            chroma_format_idc = r.read_ue()?;
            if chroma_format_idc == 3 {
                let _separate_colour_plane = r.read_bit()?;
            }
            if chroma_format_idc != 1 {
                return Err(DecodeError::Unsupported("non-4:2:0 chroma"));
            }
            if r.read_ue()? != 0 || r.read_ue()? != 0 {
                return Err(DecodeError::Unsupported("bit depth > 8"));
            }
            transform_bypass = r.read_bit()?;
            if r.read_bit()? {
                has_scaling = true;
                for i in 0..8 {
                    let present = r.read_bit()?;
                    if i < 6 {
                        if present {
                            let dflt = parse_scaling_list(&mut r, &mut scaling_4x4[i], 16)?;
                            if dflt {
                                scaling_4x4[i] = if i < 3 {
                                    DEFAULT_4X4_INTRA
                                } else {
                                    DEFAULT_4X4_INTER
                                };
                            }
                        } else {
                            scaling_4x4[i] = match i {
                                0 => DEFAULT_4X4_INTRA,
                                3 => DEFAULT_4X4_INTER,
                                _ => *scaling_4x4.get(i - 1).unwrap_or(&DEFAULT_4X4_INTRA),
                            };
                        }
                    } else if present {
                        let dflt = parse_scaling_list(&mut r, &mut scaling_8x8[i - 6], 64)?;
                        if dflt {
                            scaling_8x8[i - 6] = if i == 6 {
                                DEFAULT_8X8_INTRA
                            } else {
                                DEFAULT_8X8_INTER
                            };
                        }
                    } else {
                        scaling_8x8[i - 6] = if i == 6 {
                            DEFAULT_8X8_INTRA
                        } else {
                            DEFAULT_8X8_INTER
                        };
                    }
                }
            }
        }
        let log2_max_frame_num = r.read_ue()? + 4;
        if log2_max_frame_num > 16 {
            return Err(DecodeError::Unsupported("invalid log2_max_frame_num"));
        }
        let pic_order_cnt_type = r.read_ue()?;
        let mut log2_max_pic_order_cnt_lsb = 0;
        let mut delta_pic_order_always_zero = false;
        if pic_order_cnt_type == 0 {
            log2_max_pic_order_cnt_lsb = r.read_ue()? + 4;
            if log2_max_pic_order_cnt_lsb > 16 {
                return Err(DecodeError::Unsupported(
                    "invalid log2_max_pic_order_cnt_lsb",
                ));
            }
        } else if pic_order_cnt_type == 1 {
            delta_pic_order_always_zero = r.read_bit()?;
            let _offset_for_non_ref_pic = r.read_se()?;
            let _offset_for_top_to_bottom = r.read_se()?;
            let n = r.read_ue()?;
            if n > 255 {
                return Err(DecodeError::Unsupported("oversized poc cycle"));
            }
            for _ in 0..n {
                let _offset = r.read_se()?;
            }
        } else if pic_order_cnt_type != 2 {
            return Err(DecodeError::Unsupported("invalid pic_order_cnt_type"));
        }
        let max_num_ref_frames = r.read_ue()?;
        let gaps_in_frame_num_allowed = r.read_bit()?;
        let pic_width_in_mbs = (r.read_ue()? as u64 + 1) as usize;
        let pic_height_in_mbs = (r.read_ue()? as u64 + 1) as usize;
        if (pic_width_in_mbs as u64) * (pic_height_in_mbs as u64) > MAX_FRAME_MBS {
            return Err(DecodeError::Unsupported("frame too large"));
        }
        let frame_mbs_only_flag = r.read_bit()?;
        if !frame_mbs_only_flag {
            return Err(DecodeError::Unsupported("interlace / field coding"));
        }
        let direct_8x8_inference = r.read_bit()?;
        let cropping = r.read_bit()?;
        let (mut cl, mut cr, mut ct, mut cb) = (0, 0, 0, 0);
        if cropping {
            cl = r.read_ue()?;
            cr = r.read_ue()?;
            ct = r.read_ue()?;
            cb = r.read_ue()?;
            if (cl + cr) as usize * 2 >= pic_width_in_mbs * 16
                || (ct + cb) as usize * 2 >= pic_height_in_mbs * 16
            {
                return Err(DecodeError::Unsupported("crop exceeds frame"));
            }
        }
        Ok(Self {
            profile_idc,
            level_idc,
            seq_parameter_set_id,
            log2_max_frame_num,
            pic_order_cnt_type,
            log2_max_pic_order_cnt_lsb,
            delta_pic_order_always_zero,
            gaps_in_frame_num_allowed,
            direct_8x8_inference,
            max_num_ref_frames,
            pic_width_in_mbs,
            pic_height_in_mbs,
            frame_crop_left: cl,
            frame_crop_right: cr,
            frame_crop_top: ct,
            frame_crop_bottom: cb,
            chroma_format_idc,
            scaling_4x4,
            scaling_8x8,
            has_scaling,
            transform_bypass,
        })
    }
}
#[derive(Debug, Clone)]
pub struct Pps {
    pub pic_parameter_set_id: u32,
    pub seq_parameter_set_id: u32,
    pub entropy_coding_mode_flag: bool,
    pub bottom_field_pic_order_present: bool,
    pub num_ref_idx_l0_default: u32,
    pub num_ref_idx_l1_default: u32,
    pub weighted_pred: bool,
    pub weighted_bipred_idc: u8,
    pub pic_init_qp: i32,
    pub chroma_qp_index_offset: i32,
    pub deblocking_filter_control_present_flag: bool,
    pub constrained_intra_pred_flag: bool,
    pub redundant_pic_cnt_present_flag: bool,
    pub transform_8x8_mode_flag: bool,
    pub second_chroma_qp_index_offset: i32,
    pub pic_scaling_matrix_present: bool,
    pub scaling_4x4: [[u8; 16]; 6],
    pub scaling_8x8: [[u8; 64]; 2],
    pub scaling_present_4x4: [bool; 6],
    pub scaling_present_8x8: [bool; 2],
}
impl Pps {
    pub fn parse(rbsp: &[u8]) -> Result<Self, DecodeError> {
        let mut r = BitReader::new(rbsp);
        let pic_parameter_set_id = r.read_ue()?;
        let seq_parameter_set_id = r.read_ue()?;
        let entropy_coding_mode_flag = r.read_bit()?;
        let bottom_field_pic_order_present = r.read_bit()?;
        let num_slice_groups_minus1 = r.read_ue()?;
        if num_slice_groups_minus1 != 0 {
            return Err(DecodeError::Unsupported("slice groups (FMO)"));
        }
        let num_ref_idx_l0_default = r.read_ue()? + 1;
        let num_ref_idx_l1_default = r.read_ue()? + 1;
        let weighted_pred = r.read_bit()?;
        let weighted_bipred_idc = r.read_bits(2)? as u8;
        let pic_init_qp = 26 + r.read_se()?;
        let _pic_init_qs = r.read_se()?;
        let chroma_qp_index_offset = r.read_se()?;
        let deblocking_filter_control_present_flag = r.read_bit()?;
        let constrained_intra_pred_flag = r.read_bit()?;
        let redundant_pic_cnt_present_flag = r.read_bit()?;
        let mut transform_8x8_mode_flag = false;
        let mut second_chroma_qp_index_offset = chroma_qp_index_offset;
        let mut pic_scaling_matrix_present = false;
        let mut scaling_4x4 = [[16u8; 16]; 6];
        let mut scaling_8x8 = [[16u8; 64]; 2];
        let mut scaling_present_4x4 = [false; 6];
        let mut scaling_present_8x8 = [false; 2];
        if r.more_rbsp_data() {
            transform_8x8_mode_flag = r.read_bit()?;
            if r.read_bit()? {
                pic_scaling_matrix_present = true;
                let n = 6 + if transform_8x8_mode_flag { 2 } else { 0 };
                for i in 0..n {
                    let present = r.read_bit()?;
                    if i < 6 {
                        scaling_present_4x4[i] = present;
                        if present {
                            let dflt = parse_scaling_list(&mut r, &mut scaling_4x4[i], 16)?;
                            if dflt {
                                scaling_4x4[i] = if i < 3 {
                                    DEFAULT_4X4_INTRA
                                } else {
                                    DEFAULT_4X4_INTER
                                };
                            }
                        }
                    } else {
                        scaling_present_8x8[i - 6] = present;
                        if present {
                            let dflt = parse_scaling_list(&mut r, &mut scaling_8x8[i - 6], 64)?;
                            if dflt {
                                scaling_8x8[i - 6] = if i == 6 {
                                    DEFAULT_8X8_INTRA
                                } else {
                                    DEFAULT_8X8_INTER
                                };
                            }
                        }
                    }
                }
            }
            second_chroma_qp_index_offset = r.read_se()?;
        }
        Ok(Self {
            pic_parameter_set_id,
            seq_parameter_set_id,
            entropy_coding_mode_flag,
            bottom_field_pic_order_present,
            num_ref_idx_l0_default,
            num_ref_idx_l1_default,
            weighted_pred,
            weighted_bipred_idc,
            pic_init_qp,
            chroma_qp_index_offset,
            deblocking_filter_control_present_flag,
            constrained_intra_pred_flag,
            redundant_pic_cnt_present_flag,
            transform_8x8_mode_flag,
            second_chroma_qp_index_offset,
            pic_scaling_matrix_present,
            scaling_4x4,
            scaling_8x8,
            scaling_present_4x4,
            scaling_present_8x8,
        })
    }
}
