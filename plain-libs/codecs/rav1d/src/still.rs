use crate::include::common::bitdepth::{BitDepth8, BitDepth16};
use crate::include::dav1d::data::Rav1dData;
use crate::include::dav1d::dav1d::Rav1dSettings;
use crate::include::dav1d::headers::Rav1dPixelLayout;
use crate::include::dav1d::picture::Rav1dPicture;
use crate::src::c_arc::CArc;
use crate::src::c_box::CBox;
use crate::src::error::{Rav1dError, Rav1dResult};
use crate::src::internal::Rav1dContext;
use crate::src::lib::{rav1d_close, rav1d_get_picture, rav1d_open, rav1d_send_data};
use std::sync::Arc;

pub struct Picture {
    pub width: u32,
    pub height: u32,
    pub bit_depth: u8,
    pub chroma: u8,
    pub matrix: u16,
    pub primaries: u16,
    pub transfer: u16,
    pub full_range: bool,
    pub planes: [Vec<u16>; 3],
}

struct Decoder(Option<Arc<Rav1dContext>>);
impl Drop for Decoder {
    fn drop(&mut self) {
        if let Some(context) = self.0.take() {
            rav1d_close(context);
        }
    }
}

pub fn decode(bytes: Vec<u8>, max_pixels: u32) -> Rav1dResult<Picture> {
    let settings = Rav1dSettings {
        n_threads: 1,
        max_frame_delay: 1,
        frame_size_limit: max_pixels,
        strict_std_compliance: true,
        logger: None,
        ..Default::default()
    };
    let decoder = Decoder(Some(rav1d_open(&settings)?));
    let context = decoder.0.as_ref().unwrap();
    let mut packet = Rav1dData::from(CArc::wrap(CBox::from_box(bytes.into_boxed_slice()))?);
    let mut frame = Rav1dPicture::default();
    while !packet.as_ref().is_empty() {
        match rav1d_send_data(context, &mut packet) {
            Ok(()) => {},
            Err(Rav1dError::EAGAIN) => {
                rav1d_get_picture(context, &mut frame)?;
                break;
            }
            Err(error) => return Err(error),
        }
    }
    if frame.data.is_none() {
        rav1d_get_picture(context, &mut frame)?;
    }
    let width = frame.p.w as u32;
    let height = frame.p.h as u32;
    let data = frame.data.as_ref().ok_or(Rav1dError::EINVAL)?;
    let sequence = frame.seq_hdr.as_ref().ok_or(Rav1dError::EINVAL)?;
    let sequence = &sequence.rav1d;
    let layout = frame.p.layout;
    let mut planes: [Vec<u16>; 3] = Default::default();
    for (plane, output) in planes.iter_mut().enumerate() {
        if plane != 0 && layout == Rav1dPixelLayout::I400 {
            continue;
        }
        let xs = usize::from(plane != 0 && matches!(layout, Rav1dPixelLayout::I420 | Rav1dPixelLayout::I422));
        let ys = usize::from(plane != 0 && layout == Rav1dPixelLayout::I420);
        let w = (width as usize).div_ceil(1 << xs);
        let h = (height as usize).div_ceil(1 << ys);
        output.reserve_exact(w * h);
        let component = &data.data[plane];
        if frame.p.bpc == 8 {
            let stride = frame.stride[usize::from(plane != 0)] as usize;
            for y in 0..h {
                output.extend(component.slice::<BitDepth8, _>(y * stride..y * stride + w).iter().map(|v| u16::from(*v)));
            }
        } else {
            let stride = frame.stride[usize::from(plane != 0)] as usize / 2;
            for y in 0..h {
                output.extend_from_slice(&component.slice::<BitDepth16, _>(y * stride..y * stride + w));
            }
        }
    }
    Ok(Picture {width, height, bit_depth:frame.p.bpc, chroma:layout as u8, matrix:u16::from(sequence.mtrx.0), primaries:u16::from(sequence.pri.0), transfer:u16::from(sequence.trc.0), full_range:sequence.color_range != 0, planes})
}
