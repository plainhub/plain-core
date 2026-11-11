use crate::image_codecs::jpeg::bitstream::BitStream;
use crate::image_codecs::jpeg::components::SampleRatios;
use crate::image_codecs::jpeg::decoder::{JpegDecoder, MAX_COMPONENTS};
use crate::image_codecs::jpeg::errors::DecodeErrors;
use crate::image_codecs::jpeg::headers::parse_sos;
use crate::image_codecs::jpeg::marker::Marker;
use crate::image_codecs::jpeg::mcu::DCT_BLOCK;
use crate::image_codecs::jpeg::misc::{calculate_padded_width, setup_component_params};
use crate::image_codecs::zune::bytestream::{ZByteReaderTrait, ZReader};
use crate::image_codecs::zune::colorspace::ColorSpace;
use crate::image_codecs::zune::log::{debug, error, warn};
use core::cmp::min;
use std::string::ToString;
use std::vec::Vec;
use std::{format, vec};
impl<T: ZByteReaderTrait> JpegDecoder<T> {
    #[inline(never)]
    pub(crate) fn decode_mcu_ycbcr_progressive(
        &mut self,
        pixels: &mut [u8],
    ) -> Result<(), DecodeErrors> {
        setup_component_params(self)?;
        let mut mcu_height;
        let mut block: [Vec<i16>; MAX_COMPONENTS] = [vec![], vec![], vec![], vec![]];
        let mut mcu_width;
        let mut seen_scans = 1;
        if self.input_colorspace == ColorSpace::Luma && self.is_interleaved {
            warn!("Grayscale image with down-sampled component, resetting component details");
            self.reset_params();
        }
        if self.is_interleaved {
            self.set_upsampling()?;
        }
        if self.is_interleaved {
            mcu_width = self.mcu_x;
            mcu_height = self.mcu_y;
        } else {
            mcu_width = (self.info.width as usize + 7) / 8;
            mcu_height = (self.info.height as usize + 7) / 8;
        }
        if self.is_interleaved
            && self.input_colorspace.num_components() > 1
            && self.options.jpeg_get_out_colorspace().num_components() == 1
            && (self.info.sample_ratio == SampleRatios::V
                || self.info.sample_ratio == SampleRatios::HV)
        {
            mcu_height *= self.v_max;
            mcu_height /= self.h_max;
            self.coeff = 2;
        }
        mcu_width *= 64;
        for i in 0..self.input_colorspace.num_components() {
            let comp = &self.components[i];
            let len = mcu_width * comp.vertical_sample * comp.horizontal_sample * mcu_height;
            block[i] = vec![0; len];
        }
        let mut stream = BitStream::new_progressive(self.succ_low, self.spec_start, self.spec_end);
        let result = self.parse_entropy_coded_data(&mut stream, &mut block);
        if result.is_err() {
            return if self.options.strict_mode() {
                Err(result.err().unwrap())
            } else {
                error!("{}", result.err().unwrap());
                return self.finish_progressive_decoding(&block, pixels);
            };
        }
        let mut marker = stream
            .marker
            .take()
            .ok_or(DecodeErrors::FormatStatic("Marker missing where expected"))?;
        'eoi: while marker != Marker::EOI {
            match marker {
                Marker::SOS => {
                    parse_sos(self)?;
                    stream.update_progressive_params(
                        self.succ_high,
                        self.succ_low,
                        self.spec_start,
                        self.spec_end,
                    );
                    let result = self.parse_entropy_coded_data(&mut stream, &mut block);
                    if result.is_err() {
                        return if self.options.strict_mode() {
                            Err(result.err().unwrap())
                        } else {
                            error!("{}", result.err().unwrap());
                            break 'eoi;
                        };
                    }
                    match get_marker(&mut self.stream, &mut stream) {
                        Ok(marker_n) => {
                            marker = marker_n;
                            seen_scans += 1;
                            if seen_scans > self.options.jpeg_get_max_scans() {
                                return Err(DecodeErrors::Format(format!(
                                    "Too many scans, exceeded limit of {}",
                                    self.options.jpeg_get_max_scans()
                                )));
                            }
                            stream.reset();
                            continue 'eoi;
                        }
                        Err(msg) => {
                            if self.options.strict_mode() {
                                return Err(msg);
                            }
                            error!("{:?}", msg);
                            break 'eoi;
                        }
                    }
                }
                Marker::RST(_n) => {
                    self.handle_rst(&mut stream)?;
                }
                _ => {
                    self.parse_marker_inner(marker)?;
                }
            }
            match get_marker(&mut self.stream, &mut stream) {
                Ok(marker_n) => {
                    marker = marker_n;
                }
                Err(e) => {
                    if self.options.strict_mode() {
                        return Err(e);
                    }
                    error!("{}", e);
                    break 'eoi;
                }
            }
        }
        self.finish_progressive_decoding(&block, pixels)
    }
    fn reset_prog_params(&mut self, stream: &mut BitStream) {
        stream.reset();
        self.components.iter_mut().for_each(|x| x.dc_pred = 0);
        self.todo = if self.restart_interval != 0 {
            self.restart_interval
        } else {
            usize::MAX
        };
    }
    fn parse_entropy_coded_data(
        &mut self,
        stream: &mut BitStream,
        buffer: &mut [Vec<i16>; MAX_COMPONENTS],
    ) -> Result<(), DecodeErrors> {
        self.reset_prog_params(stream);
        if usize::from(self.num_scans) > self.input_colorspace.num_components() {
            return Err(DecodeErrors::Format(format!(
                "Number of scans {} cannot be greater than number of components, {}",
                self.num_scans,
                self.input_colorspace.num_components()
            )));
        }
        if self.num_scans == 1 {
            if self.spec_end != 0 && self.spec_start == 0 {
                return Err(DecodeErrors::FormatStatic(
                    "Can't merge DC and AC corrupt jpeg",
                ));
            }
            let k = self.z_order[0];
            if k >= self.components.len() {
                return Err(DecodeErrors::Format(format!(
                    "Cannot find component {k}, corrupt image"
                )));
            }
            let component = &self.components[k];
            let mcu_width =
                (self.info.width as usize * component.horizontal_sample).div_ceil(self.h_max * 8);
            let mcu_height =
                (self.info.height as usize * component.vertical_sample).div_ceil(self.v_max * 8);
            for i in 0..mcu_height {
                for j in 0..mcu_width {
                    if self.spec_start != 0 && self.succ_high == 0 && stream.eob_run > 0 {
                        stream.eob_run -= 1;
                    } else {
                        let start = 64 * (j + i * (self.components[k].width_stride / 8));
                        let data: &mut [i16; 64] = buffer
                            .get_mut(k)
                            .unwrap()
                            .get_mut(start..start + 64)
                            .ok_or(DecodeErrors::FormatStatic("Slice to Small"))?
                            .try_into()
                            .unwrap();
                        if self.spec_start == 0 {
                            let pos = self.components[k].dc_huff_table & (MAX_COMPONENTS - 1);
                            let dc_table = self
                                .dc_huffman_tables
                                .get(pos)
                                .ok_or(DecodeErrors::FormatStatic(
                                    "No huffman table for DC component",
                                ))?
                                .as_ref()
                                .ok_or(DecodeErrors::FormatStatic(
                                    "Huffman table at index  {} not initialized",
                                ))?;
                            let dc_pred = &mut self.components[k].dc_pred;
                            if self.succ_high == 0 {
                                stream.decode_prog_dc_first(
                                    &mut self.stream,
                                    dc_table,
                                    &mut data[0],
                                    dc_pred,
                                )?;
                            } else {
                                stream.decode_prog_dc_refine(&mut self.stream, &mut data[0])?;
                            }
                        } else {
                            let pos = self.components[k].ac_huff_table;
                            let ac_table = self
                                .ac_huffman_tables
                                .get(pos)
                                .ok_or_else(|| {
                                    DecodeErrors::Format(format!(
                                        "No huffman table for component:{pos}"
                                    ))
                                })?
                                .as_ref()
                                .ok_or_else(|| {
                                    DecodeErrors::Format(format!(
                                        "Huffman table at index  {pos} not initialized"
                                    ))
                                })?;
                            if self.succ_high == 0 {
                                debug_assert!(stream.eob_run == 0, "EOB run is not zero");
                                stream.decode_mcu_ac_first(&mut self.stream, ac_table, data)?;
                            } else {
                                stream.decode_mcu_ac_refine(&mut self.stream, ac_table, data)?;
                            }
                        }
                    }
                    self.todo -= 1;
                    self.handle_rst_main(stream)?;
                }
            }
        } else {
            if self.spec_end != 0 {
                return Err(DecodeErrors::HuffmanDecode(
                    "Can't merge dc and AC corrupt jpeg".to_string(),
                ));
            }
            for k in 0..self.num_scans {
                let n = self.z_order[k as usize];
                if n >= self.components.len() {
                    return Err(DecodeErrors::Format(format!(
                        "Cannot find component {n}, corrupt image"
                    )));
                }
                let component = &mut self.components[n];
                let _ = self
                    .dc_huffman_tables
                    .get(component.dc_huff_table)
                    .ok_or_else(|| {
                        DecodeErrors::Format(format!(
                            "No huffman table for component:{}",
                            component.dc_huff_table
                        ))
                    })?
                    .as_ref()
                    .ok_or_else(|| {
                        DecodeErrors::Format(format!(
                            "Huffman table at index  {} not initialized",
                            component.dc_huff_table
                        ))
                    })?;
            }
            for i in 0..self.mcu_y {
                for j in 0..self.mcu_x {
                    for k in 0..self.num_scans {
                        let n = self.z_order[k as usize];
                        let component = &mut self.components[n];
                        let huff_table = self
                            .dc_huffman_tables
                            .get(component.dc_huff_table)
                            .ok_or(DecodeErrors::FormatStatic("No huffman table for component"))?
                            .as_ref()
                            .ok_or(DecodeErrors::FormatStatic(
                                "Huffman table at index not initialized",
                            ))?;
                        for v_samp in 0..component.vertical_sample {
                            for h_samp in 0..component.horizontal_sample {
                                let x2 = j * component.horizontal_sample + h_samp;
                                let y2 = i * component.vertical_sample + v_samp;
                                let position = 64 * (x2 + y2 * component.width_stride / 8);
                                let buf_n = &mut buffer[n];
                                let Some(data) = &mut buf_n.get_mut(position) else {
                                    return Err(DecodeErrors::FormatStatic("Invalid image"));
                                };
                                if self.succ_high == 0 {
                                    stream.decode_prog_dc_first(
                                        &mut self.stream,
                                        huff_table,
                                        data,
                                        &mut component.dc_pred,
                                    )?;
                                } else {
                                    stream.decode_prog_dc_refine(&mut self.stream, data)?;
                                }
                            }
                        }
                    }
                    self.todo -= 1;
                    self.handle_rst_main(stream)?;
                }
            }
        }
        return Ok(());
    }
    pub(crate) fn handle_rst_main(&mut self, stream: &mut BitStream) -> Result<(), DecodeErrors> {
        if self.todo == 0 {
            stream.refill(&mut self.stream)?;
        }
        if self.todo == 0
            && self.restart_interval != 0
            && stream.marker.is_none()
            && !stream.seen_eoi
        {
            let _start = self.stream.position()?;
            let marker = get_marker(&mut self.stream, stream);
            if let Ok(marker) = marker {
                let _end = self.stream.position()?;
                stream.marker = Some(marker);
                warn!(
                    "{} Extraneous bytes before marker {:?}",
                    _end - _start,
                    marker
                );
            } else {
                warn!("RST marker was not found, where expected, image may be garbled")
            }
        }
        if self.todo == 0 {
            self.handle_rst(stream)?
        }
        Ok(())
    }
    fn finish_progressive_decoding(
        &mut self,
        block: &[Vec<i16>; MAX_COMPONENTS],
        pixels: &mut [u8],
    ) -> Result<(), DecodeErrors> {
        let mcu_height = if self.is_interleaved {
            self.mcu_y
        } else {
            self.info.height.div_ceil(8) as usize
        };
        let is_hv = usize::from(self.is_interleaved);
        let upsampler_scratch_size = is_hv * self.components[0].width_stride;
        let width = usize::from(self.info.width);
        let padded_width = calculate_padded_width(width, self.info.sample_ratio);
        let mut upsampler_scratch_space = vec![0; upsampler_scratch_size];
        let mut tmp = [0_i32; DCT_BLOCK];
        for (pos, comp) in self.components.iter_mut().enumerate() {
            if min(
                self.options.jpeg_get_out_colorspace().num_components() - 1,
                pos,
            ) == pos
                || self.input_colorspace == ColorSpace::YCCK
                || self.input_colorspace == ColorSpace::CMYK
            {
                let len = comp.width_stride * comp.vertical_sample * 8;
                comp.needed = true;
                comp.raw_coeff = vec![0; len];
            } else {
                comp.needed = false;
            }
        }
        let mut pixels_written = 0;
        for i in 0..mcu_height {
            'component: for (position, component) in &mut self.components.iter_mut().enumerate() {
                if !component.needed {
                    continue 'component;
                }
                let qt_table = &component.quantization_table;
                let step = block[position].len() / mcu_height;
                let start = i * step;
                let slice = &block[position][start..start + step];
                let temp_channel = &mut component.raw_coeff;
                let mcu_x = component.width_stride / 8;
                for k in 0..component.vertical_sample {
                    for j in 0..mcu_x {
                        let width_stride = k * 8 * component.width_stride;
                        let start = j * 64 + width_stride;
                        let Some(qt_slice) = slice.get(start..start + 64) else {
                            return Err(DecodeErrors::FormatStatic(
                                "Invalid slice , would panic, invalid image",
                            ));
                        };
                        for ((x, out), qt_val) in
                            qt_slice.iter().zip(tmp.iter_mut()).zip(qt_table.iter())
                        {
                            *out = i32::from(*x) * qt_val;
                        }
                        let sl = &mut temp_channel[component.idct_pos..];
                        component.idct_pos += 8;
                        (self.idct_func)(&mut tmp, sl, component.width_stride);
                    }
                    component.idct_pos += 7 * component.width_stride;
                }
                component.idct_pos = 0;
            }
            self.post_process(
                pixels,
                i,
                mcu_height,
                width,
                padded_width,
                &mut pixels_written,
                &mut upsampler_scratch_space,
            )?;
        }
        debug!("Finished decoding image");
        return Ok(());
    }
    pub(crate) fn reset_params(&mut self) {
        self.h_max = 1;
        self.v_max = 1;
        self.info.sample_ratio = SampleRatios::None;
        self.is_interleaved = false;
        self.components[0].vertical_sample = 1;
        self.components[0].width_stride = (((self.info.width as usize) + 7) / 8) * 8;
        self.components[0].horizontal_sample = 1;
    }
}
pub fn get_marker<T>(
    reader: &mut ZReader<T>,
    stream: &mut BitStream,
) -> Result<Marker, DecodeErrors>
where
    T: ZByteReaderTrait,
{
    if let Some(marker) = stream.marker {
        stream.marker = None;
        return Ok(marker);
    }
    while !reader.eof()? {
        let marker = reader.read_u8_err()?;
        if marker == 255 {
            let mut r = reader.read_u8_err()?;
            while r == 0xFF {
                r = reader.read_u8_err()?;
            }
            if r != 0 {
                return Marker::from_u8(r)
                    .ok_or_else(|| DecodeErrors::Format(format!("Unknown marker 0xFF{r:X}")));
            }
        }
    }
    return Err(DecodeErrors::ExhaustedData);
}
