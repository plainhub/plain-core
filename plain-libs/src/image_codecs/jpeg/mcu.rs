use crate::image_codecs::jpeg::JpegDecoder;
use crate::image_codecs::jpeg::bitstream::BitStream;
use crate::image_codecs::jpeg::components::SampleRatios;
use crate::image_codecs::jpeg::decoder::MAX_COMPONENTS;
use crate::image_codecs::jpeg::errors::DecodeErrors;
use crate::image_codecs::jpeg::marker::Marker;
use crate::image_codecs::jpeg::mcu_prog::get_marker;
use crate::image_codecs::jpeg::misc::{calculate_padded_width, setup_component_params};
use crate::image_codecs::jpeg::worker::{color_convert, upsample};
use crate::image_codecs::zune::bytestream::ZByteReaderTrait;
use crate::image_codecs::zune::colorspace::ColorSpace;
use crate::image_codecs::zune::colorspace::ColorSpace::Luma;
use crate::image_codecs::zune::log::{error, trace, warn};
use core::cmp::min;
use std::vec::Vec;
use std::{format, vec};
pub const DCT_BLOCK: usize = 64;
impl<T: ZByteReaderTrait> JpegDecoder<T> {
    pub(crate) fn check_tables(&self) -> Result<(), DecodeErrors> {
        for component in &self.components {
            let _ = &self
                .dc_huffman_tables
                .get(component.dc_huff_table)
                .as_ref()
                .ok_or_else(|| {
                    DecodeErrors::HuffmanDecode(format!(
                        "No Huffman DC table for component {:?} ",
                        component.component_id
                    ))
                })?
                .as_ref()
                .ok_or_else(|| {
                    DecodeErrors::HuffmanDecode(format!(
                        "No DC table for component {:?}",
                        component.component_id
                    ))
                })?;
            let _ = &self
                .ac_huffman_tables
                .get(component.ac_huff_table)
                .as_ref()
                .ok_or_else(|| {
                    DecodeErrors::HuffmanDecode(format!(
                        "No Huffman AC table for component {:?} ",
                        component.component_id
                    ))
                })?
                .as_ref()
                .ok_or_else(|| {
                    DecodeErrors::HuffmanDecode(format!(
                        "No AC table for component {:?}",
                        component.component_id
                    ))
                })?;
        }
        Ok(())
    }
    #[inline(never)]
    pub(crate) fn decode_mcu_ycbcr_baseline(
        &mut self,
        pixels: &mut [u8],
    ) -> Result<(), DecodeErrors> {
        setup_component_params(self)?;
        self.check_tables()?;
        let (mut mcu_width, mut mcu_height);
        if self.is_interleaved {
            self.set_upsampling()?;
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
        if self.input_colorspace == ColorSpace::Luma && self.is_interleaved {
            warn!("Grayscale image with down-sampled component, resetting component details");
            self.reset_params();
            mcu_width = ((self.info.width + 7) / 8) as usize;
            mcu_height = ((self.info.height + 7) / 8) as usize;
        }
        let width = usize::from(self.info.width);
        let padded_width = calculate_padded_width(width, self.info.sample_ratio);
        let mut stream = BitStream::new();
        let mut tmp = [0_i32; DCT_BLOCK];
        let comp_len = self.components.len();
        for (pos, comp) in self.components.iter_mut().enumerate() {
            if min(
                self.options.jpeg_get_out_colorspace().num_components() - 1,
                pos,
            ) == pos
                || comp_len == 4
            {
                let len = comp.width_stride * comp.vertical_sample * 8;
                comp.needed = true;
                comp.raw_coeff = vec![0; len];
            } else {
                comp.needed = false;
            }
        }
        let all_components_in_first_scan = usize::from(self.num_scans) == self.components.len();
        let mut progressive_mcus: [Vec<i16>; 4] = core::array::from_fn(|_| vec![]);
        if !all_components_in_first_scan {
            for (component, mcu) in self.components.iter().zip(&mut progressive_mcus) {
                let len = mcu_width
                    * component.vertical_sample
                    * component.horizontal_sample
                    * mcu_height
                    * 64;
                *mcu = vec![0; len];
            }
        }
        let mut pixels_written = 0;
        let is_hv = usize::from(self.is_interleaved);
        let upsampler_scratch_size = is_hv
            * self
                .components
                .iter()
                .map(|x| x.width_stride)
                .max()
                .unwrap_or(0)
            * 8;
        let mut upsampler_scratch_space = vec![0; upsampler_scratch_size];
        'sos: loop {
            trace!(
                "Baseline decoding of components: {:?}",
                &self.z_order[..usize::from(self.num_scans)]
            );
            trace!("Decoding MCU width: {mcu_width}, height: {mcu_height}");
            for i in 0..mcu_height {
                if stream.overread_by > 0 {
                    pixels.get_mut(pixels_written..).map(|v| v.fill(128));
                    if self.options.strict_mode() {
                        return Err(DecodeErrors::FormatStatic("Premature end of buffer"));
                    };
                    error!("Premature end of buffer");
                    break;
                }
                let terminate = if all_components_in_first_scan {
                    self.decode_mcu_width::<false>(
                        mcu_width,
                        i,
                        &mut tmp,
                        &mut stream,
                        &mut progressive_mcus,
                    )?
                } else {
                    self.decode_mcu_width::<true>(
                        mcu_width,
                        i,
                        &mut tmp,
                        &mut stream,
                        &mut progressive_mcus,
                    )?
                };
                if all_components_in_first_scan {
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
                match terminate {
                    McuContinuation::Ok => {}
                    McuContinuation::AnotherSos if all_components_in_first_scan => {
                        warn!("More than one SOS despite already having all components");
                        return Ok(());
                    }
                    McuContinuation::AnotherSos => continue 'sos,
                    McuContinuation::InterScanMarker(marker) => {
                        if self.advance_to_next_sos(marker, &mut stream)? {
                            continue 'sos;
                        } else {
                            break;
                        }
                    }
                    McuContinuation::Terminate => {
                        warn!("Got terminate signal, will not process further");
                        pixels.get_mut(pixels_written..).map(|v| v.fill(128));
                        return Ok(());
                    }
                }
            }
            break;
        }
        if !all_components_in_first_scan {
            self.finish_baseline_decoding(&progressive_mcus, mcu_width, pixels)?;
        }
        if !stream.seen_eoi {
            let marker = get_marker(&mut self.stream, &mut stream);
            match marker {
                Ok(_m) => {
                    trace!("Found marker {:?}", _m);
                }
                Err(_) => {}
            }
        }
        trace!("Finished decoding image");
        Ok(())
    }
    pub(crate) fn finish_baseline_decoding(
        &mut self,
        block: &[Vec<i16>; MAX_COMPONENTS],
        _mcu_width: usize,
        pixels: &mut [u8],
    ) -> Result<(), DecodeErrors> {
        let mcu_height = self.mcu_y;
        let is_hv = usize::from(self.is_interleaved);
        let upsampler_scratch_size = is_hv * self.components[0].width_stride;
        let width = usize::from(self.info.width);
        let padded_width = calculate_padded_width(width, self.info.sample_ratio);
        let mut upsampler_scratch_space = vec![0; upsampler_scratch_size];
        for (pos, comp) in self.components.iter_mut().enumerate() {
            if min(
                self.options.jpeg_get_out_colorspace().num_components() - 1,
                pos,
            ) == pos
                || self.input_colorspace == ColorSpace::YCCK
                || self.input_colorspace == ColorSpace::CMYK
            {
                comp.needed = true;
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
                let step = block[position].len() / mcu_height;
                let slice = &block[position][i * step..][..step];
                let temp_channel = &mut component.raw_coeff;
                temp_channel[..step].copy_from_slice(slice);
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
        return Ok(());
    }
    fn decode_mcu_width<const PROGRESSIVE: bool>(
        &mut self,
        mcu_width: usize,
        mcu_height: usize,
        tmp: &mut [i32; 64],
        stream: &mut BitStream,
        progressive: &mut [Vec<i16>; 4],
    ) -> Result<McuContinuation, DecodeErrors> {
        let is_one_by_one = !self.scan_subsampled;
        if is_one_by_one {
            self.inner_decode_mcu_width::<PROGRESSIVE, false>(
                mcu_width,
                mcu_height,
                tmp,
                stream,
                progressive,
            )
        } else {
            self.inner_decode_mcu_width::<PROGRESSIVE, true>(
                mcu_width,
                mcu_height,
                tmp,
                stream,
                progressive,
            )
        }
    }
    fn inner_decode_mcu_width<const PROGRESSIVE: bool, const SAMPLED: bool>(
        &mut self,
        mcu_width: usize,
        mcu_height: usize,
        tmp: &mut [i32; 64],
        stream: &mut BitStream,
        progressive: &mut [Vec<i16>; 4],
    ) -> Result<McuContinuation, DecodeErrors> {
        let z_order = self.z_order;
        let z_scans = &z_order[..usize::from(self.num_scans)];
        let mut clobber_more_than_4x4 = true;
        let mut scan_du_width = if PROGRESSIVE {
            let k = z_scans[0];
            let comp = &self.components[k];
            (self.info.width as usize * comp.horizontal_sample + self.h_max * 8 - 1)
                / (self.h_max * 8)
        } else {
            mcu_width
        };
        if PROGRESSIVE && z_scans.len() > 1 {
            let min_du = z_scans
                .iter()
                .map(|&k| self.components[k].width_stride / 8)
                .min()
                .unwrap_or(0);
            scan_du_width = scan_du_width.min(min_du);
        }
        for j in 0..scan_du_width {
            for &k in z_scans {
                let component = &mut self.components[k];
                let dc_table = self.dc_huffman_tables[component.dc_huff_table % MAX_COMPONENTS]
                    .as_ref()
                    .ok_or(DecodeErrors::FormatStatic("DC table not found"))?;
                let ac_table = self.ac_huffman_tables[component.ac_huff_table % MAX_COMPONENTS]
                    .as_ref()
                    .ok_or(DecodeErrors::FormatStatic("AC table not found"))?;
                let qt_table = &component.quantization_table;
                let channel = if PROGRESSIVE {
                    let offset =
                        mcu_height * component.width_stride * 8 * component.vertical_sample;
                    if offset >= progressive[k].len() {
                        return Err(DecodeErrors::FormatStatic("Would panic on slice iteration"));
                    }
                    &mut progressive[k][offset..]
                } else {
                    &mut component.raw_coeff
                };
                let component_samples_needed = component.needed;
                let v_step = if SAMPLED && !PROGRESSIVE {
                    0..component.vertical_sample
                } else {
                    0..1
                };
                for v_samp in v_step {
                    let h_step = if SAMPLED && !PROGRESSIVE {
                        0..component.horizontal_sample
                    } else {
                        0..1
                    };
                    for h_samp in h_step {
                        let result = if component_samples_needed {
                            let clobber_len = if !clobber_more_than_4x4 { 32 } else { 64 };
                            tmp[..clobber_len].fill(0);
                            stream.decode_mcu_block(
                                &mut self.stream,
                                dc_table,
                                ac_table,
                                qt_table,
                                tmp,
                                &mut component.dc_pred,
                            )
                        } else {
                            stream.discard_mcu_block(&mut self.stream, dc_table, ac_table)
                        };
                        let len = if let Ok(len) = result {
                            len
                        } else {
                            return if self.options.strict_mode() {
                                Err(result.err().unwrap())
                            } else {
                                error!("{}", result.err().unwrap());
                                Ok(McuContinuation::Terminate)
                            };
                        };
                        if component_samples_needed {
                            clobber_more_than_4x4 = len > 10;
                            let idct_position = if PROGRESSIVE {
                                j * 8
                            } else {
                                let c2 = v_samp * 8;
                                let c3 = ((j * component.horizontal_sample) + h_samp) * 8;
                                component.width_stride * c2 + c3
                            };
                            let idct_pos = channel.get_mut(idct_position..).unwrap();
                            if len <= 1 {
                                (self.idct_1x1_func)(tmp, idct_pos, component.width_stride);
                            } else if len <= 10 {
                                (self.idct_4x4_func)(tmp, idct_pos, component.width_stride);
                            } else {
                                (self.idct_func)(tmp, idct_pos, component.width_stride);
                            }
                        }
                    }
                }
            }
            self.todo = self.todo.wrapping_sub(1);
            if self.todo == 0 {
                self.handle_rst_main(stream)?;
                continue;
            }
            if stream.marker.is_some() && stream.bits_left == 0 {
                break;
            }
        }
        self.check_stream_marker_after_mcu_width(stream)
    }
    fn check_stream_marker_after_mcu_width(
        &mut self,
        stream: &mut BitStream,
    ) -> Result<McuContinuation, DecodeErrors> {
        if let Some(m) = stream.marker {
            if m == Marker::EOI {
                stream.marker.take();
                trace!("Found EOI marker");
                stream.seen_eoi = true;
            } else if let Marker::RST(_) = m {
                if self.todo == 0 {
                    self.handle_rst(stream)?;
                }
            } else if let Marker::SOS = m {
                self.parse_marker_inner(m)?;
                stream.marker.take();
                stream.reset();
                trace!("Found SOS marker");
                return Ok(McuContinuation::AnotherSos);
            } else if matches!(m, Marker::DHT | Marker::DQT | Marker::DRI | Marker::COM)
                || matches!(m, Marker::APP(_))
            {
                stream.marker.take();
                trace!("Found inter-scan marker {:?}", m);
                return Ok(McuContinuation::InterScanMarker(m));
            } else {
                if self.options.strict_mode() {
                    return Err(DecodeErrors::Format(format!(
                        "Marker {m:?} found where not expected"
                    )));
                }
                error!(
                    "Marker `{:?}` Found within Huffman Stream, possibly corrupt jpeg",
                    m
                );
                self.parse_marker_inner(m)?;
                stream.marker.take();
                stream.reset();
                return Ok(McuContinuation::Terminate);
            }
        }
        Ok(McuContinuation::Ok)
    }
    fn advance_to_next_sos(
        &mut self,
        first_marker: Marker,
        stream: &mut BitStream,
    ) -> Result<bool, DecodeErrors> {
        const MAX_INTER_SCAN_MARKERS: usize = 64;
        self.parse_marker_inner(first_marker)?;
        stream.reset();
        for _ in 0..MAX_INTER_SCAN_MARKERS {
            let marker = get_marker(&mut self.stream, stream)?;
            match marker {
                Marker::SOS => {
                    self.parse_marker_inner(Marker::SOS)?;
                    stream.reset();
                    trace!("Found SOS marker, continuing decode");
                    return Ok(true);
                }
                Marker::EOI => {
                    stream.seen_eoi = true;
                    trace!("Found EOI marker");
                    return Ok(false);
                }
                Marker::DHT | Marker::DQT | Marker::DRI | Marker::COM => {
                    trace!("Parsing inter-scan marker {:?}", marker);
                    self.parse_marker_inner(marker)?;
                }
                Marker::APP(_) => {
                    trace!("Parsing inter-scan APP marker {:?}", marker);
                    self.parse_marker_inner(marker)?;
                }
                other => {
                    if self.options.strict_mode() {
                        return Err(DecodeErrors::Format(format!(
                            "Unexpected marker {:?} while scanning for SOS between scans",
                            other
                        )));
                    }
                    warn!("Skipping unexpected marker {:?} between scans", other);
                    let length = self.stream.get_u16_be_err()?;
                    if length >= 2 {
                        self.stream.skip((length - 2) as usize)?;
                    }
                }
            }
        }
        Err(DecodeErrors::FormatStatic(
            "Too many markers between scans (exceeded limit of 64)",
        ))
    }
    #[cold]
    pub(crate) fn handle_rst(&mut self, stream: &mut BitStream) -> Result<(), DecodeErrors> {
        self.todo = self.restart_interval;
        if let Some(marker) = stream.marker {
            match marker {
                Marker::RST(_) => {
                    stream.reset();
                    self.components.iter_mut().for_each(|x| x.dc_pred = 0);
                }
                Marker::EOI => {}
                Marker::SOS
                | Marker::DHT
                | Marker::DQT
                | Marker::DRI
                | Marker::COM
                | Marker::APP(_) => {}
                _ => {
                    if self.options.strict_mode() {
                        return Err(DecodeErrors::MCUError(format!(
                            "Unexpected marker {marker:?} at restart boundary"
                        )));
                    }
                    warn!("Unexpected marker {:?} at restart boundary", marker);
                }
            }
        }
        Ok(())
    }
    pub(crate) fn post_process(
        &mut self,
        pixels: &mut [u8],
        i: usize,
        mcu_height: usize,
        width: usize,
        padded_width: usize,
        pixels_written: &mut usize,
        upsampler_scratch_space: &mut [i16],
    ) -> Result<(), DecodeErrors> {
        let out_colorspace_components = self.options.jpeg_get_out_colorspace().num_components();
        let mut px = *pixels_written;
        let is_vertically_sampled = self
            .components
            .iter()
            .any(|c| c.sample_ratio == SampleRatios::HV || c.sample_ratio == SampleRatios::V);
        let mut comp_len = self.components.len();
        if out_colorspace_components < comp_len && self.options.jpeg_get_out_colorspace() == Luma {
            comp_len = out_colorspace_components;
        }
        let mut color_conv_function =
            |num_iters: usize, samples: [&[i16]; 4]| -> Result<(), DecodeErrors> {
                for (pos, output) in pixels[px..]
                    .chunks_exact_mut(width * out_colorspace_components)
                    .take(num_iters)
                    .enumerate()
                {
                    let mut raw_samples: [&[i16]; 4] = [&[], &[], &[], &[]];
                    for (j, samp) in raw_samples.iter_mut().enumerate().take(comp_len) {
                        let temp = &samples[j].get(pos * padded_width..(pos + 1) * padded_width);
                        if temp.is_none() {
                            return Err(DecodeErrors::FormatStatic("Missing samples"));
                        }
                        *samp = temp.unwrap();
                    }
                    color_convert(
                        &raw_samples,
                        self.color_convert_16,
                        self.input_colorspace,
                        self.options.jpeg_get_out_colorspace(),
                        output,
                        width,
                        padded_width,
                    )?;
                    px += width * out_colorspace_components;
                }
                Ok(())
            };
        let comps = &mut self.components[..];
        if self.is_interleaved && self.options.jpeg_get_out_colorspace() != ColorSpace::Luma {
            for comp in comps.iter_mut() {
                upsample(
                    comp,
                    mcu_height,
                    i,
                    upsampler_scratch_space,
                    is_vertically_sampled,
                )?;
            }
            if is_vertically_sampled {
                if i > 0 {
                    let mut samples: [&[i16]; 4] = [&[], &[], &[], &[]];
                    for (samp, component) in samples.iter_mut().zip(comps.iter()) {
                        *samp = &component.first_row_upsample_dest;
                    }
                    let _first_len = samples[0].len();
                    let num_iters = self.coeff * self.v_max;
                    color_conv_function(num_iters, samples)?;
                }
                for component in comps.iter_mut() {
                    if component.sample_ratio != SampleRatios::H {
                        let size = component.vertical_sample
                            * component.width_stride
                            * component.sample_ratio.sample();
                        let last_bytes =
                            component.raw_coeff.rchunks_exact_mut(size).next().unwrap();
                        component
                            .first_row_upsample_dest
                            .copy_from_slice(last_bytes);
                    }
                }
            }
            let mut samples: [&[i16]; 4] = [&[], &[], &[], &[]];
            for (samp, component) in samples.iter_mut().zip(comps.iter()) {
                *samp = if component.sample_ratio == SampleRatios::None {
                    &component.raw_coeff
                } else {
                    &component.upsample_dest
                };
            }
            let is_last_considered = is_vertically_sampled && (i != mcu_height.saturating_sub(1));
            let num_iters = (8 - usize::from(is_last_considered)) * self.coeff * self.v_max;
            color_conv_function(num_iters, samples)?;
        } else {
            let mut channels_ref: [&[i16]; MAX_COMPONENTS] = [&[]; MAX_COMPONENTS];
            self.components
                .iter()
                .enumerate()
                .for_each(|(pos, x)| channels_ref[pos] = &x.raw_coeff);
            if let SampleRatios::Generic(_, v) = self.info.sample_ratio {
                color_conv_function(8 * v * self.coeff, channels_ref)?;
            } else {
                color_conv_function(8 * self.coeff, channels_ref)?;
            }
        }
        *pixels_written = px;
        Ok(())
    }
}
enum McuContinuation {
    Ok,
    AnotherSos,
    InterScanMarker(Marker),
    Terminate,
}
