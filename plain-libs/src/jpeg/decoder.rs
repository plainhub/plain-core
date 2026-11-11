use crate::jpeg::error::{Error, Result, UnsupportedFeature};
use crate::jpeg::huffman::{HuffmanDecoder, HuffmanTable, fill_default_mjpeg_tables};
use crate::jpeg::marker::Marker;
use crate::jpeg::parser::{
    AdobeColorTransform, AppData, CodingProcess, Component, Dimensions, EntropyCoding, FrameInfo,
    ScanInfo, parse_app, parse_com, parse_dht, parse_dqt, parse_dri, parse_sof, parse_sos,
};
use crate::jpeg::read_u8;
use crate::jpeg::upsampler::Upsampler;
use crate::jpeg::worker::{PreferWorkerKind, RowData, Worker, WorkerScope, compute_image_parallel};
use core::cmp;
use core::mem;
use core::ops::Range;
use std::borrow::ToOwned;
use std::io::Read;
use std::sync::Arc;
use std::vec::Vec;
use std::{format, vec};
pub const MAX_COMPONENTS: usize = 4;
mod lossless;
use self::lossless::compute_image_lossless;
# [rustfmt :: skip] static UNZIGZAG : [u8 ; 64] = [0 , 1 , 8 , 16 , 9 , 2 , 3 , 10 , 17 , 24 , 32 , 25 , 18 , 11 , 4 , 5 , 12 , 19 , 26 , 33 , 40 , 48 , 41 , 34 , 27 , 20 , 13 , 6 , 7 , 14 , 21 , 28 , 35 , 42 , 49 , 56 , 57 , 50 , 43 , 36 , 29 , 22 , 15 , 23 , 30 , 37 , 44 , 51 , 58 , 59 , 52 , 45 , 38 , 31 , 39 , 46 , 53 , 60 , 61 , 54 , 47 , 55 , 62 , 63 ,] ;
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PixelFormat {
    L8,
    L16,
    RGB24,
    CMYK32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ImageInfo {
    pub width: u16,
    pub height: u16,
    pub pixel_format: PixelFormat,
    pub coding_process: CodingProcess,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ColorTransform {
    Unknown,
    Grayscale,
    RGB,
    YCbCr,
    CMYK,
    YCCK,
    JcsBgYcc,
    JcsBgRgb,
}
pub struct Decoder<R> {
    reader: R,
    frame: Option<FrameInfo>,
    dc_huffman_tables: Vec<Option<HuffmanTable>>,
    ac_huffman_tables: Vec<Option<HuffmanTable>>,
    quantization_tables: [Option<Arc<[u16; 64]>>; 4],
    restart_interval: u16,
    adobe_color_transform: Option<AdobeColorTransform>,
    is_jfif: bool,
    is_mjpeg: bool,
    coefficients: Vec<Vec<i16>>,
    coefficients_finished: [u64; MAX_COMPONENTS],
}
impl<R: Read> Decoder<R> {
    pub fn new(reader: R) -> Decoder<R> {
        Decoder {
            reader,
            frame: None,
            dc_huffman_tables: vec![None, None, None, None],
            ac_huffman_tables: vec![None, None, None, None],
            quantization_tables: [None, None, None, None],
            restart_interval: 0,
            adobe_color_transform: None,
            is_jfif: false,
            is_mjpeg: false,
            coefficients: Vec::new(),
            coefficients_finished: [0; MAX_COMPONENTS],
        }
    }
    pub fn info(&self) -> Option<ImageInfo> {
        match self.frame {
            Some(ref frame) => {
                let pixel_format = match frame.components.len() {
                    1 => match frame.precision {
                        2..=8 => PixelFormat::L8,
                        9..=16 => PixelFormat::L16,
                        _ => panic!(),
                    },
                    3 => PixelFormat::RGB24,
                    4 => PixelFormat::CMYK32,
                    _ => panic!(),
                };
                Some(ImageInfo {
                    width: frame.output_size.width,
                    height: frame.output_size.height,
                    pixel_format,
                    coding_process: frame.coding_process,
                })
            }
            None => None,
        }
    }
    fn select_worker(frame: &FrameInfo, worker_preference: PreferWorkerKind) -> PreferWorkerKind {
        const PARALLELISM_THRESHOLD: u64 = 128 * 128;
        match worker_preference {
            PreferWorkerKind::Immediate => PreferWorkerKind::Immediate,
            PreferWorkerKind::Multithreaded => {
                let width: u64 = frame.output_size.width.into();
                let height: u64 = frame.output_size.width.into();
                if width * height > PARALLELISM_THRESHOLD {
                    PreferWorkerKind::Multithreaded
                } else {
                    PreferWorkerKind::Immediate
                }
            }
        }
    }
    pub fn read_info(&mut self) -> Result<()> {
        WorkerScope::with(|worker| self.decode_internal(true, worker)).map(|_| ())
    }
    pub fn scale(&mut self, requested_width: u16, requested_height: u16) -> Result<(u16, u16)> {
        self.read_info()?;
        let frame = self.frame.as_mut().unwrap();
        let idct_size = crate::jpeg::idct::choose_idct_size(
            frame.image_size,
            Dimensions {
                width: requested_width,
                height: requested_height,
            },
        );
        frame.update_idct_size(idct_size)?;
        Ok((frame.output_size.width, frame.output_size.height))
    }
    pub fn decode(&mut self) -> Result<Vec<u8>> {
        WorkerScope::with(|worker| self.decode_internal(false, worker))
    }
    fn decode_internal(
        &mut self,
        stop_after_metadata: bool,
        worker_scope: &WorkerScope,
    ) -> Result<Vec<u8>> {
        if stop_after_metadata && self.frame.is_some() {
            return Ok(Vec::new());
        } else if self.frame.is_none()
            && (read_u8(&mut self.reader)? != 0xFF
                || Marker::from_u8(read_u8(&mut self.reader)?) != Some(Marker::SOI))
        {
            return Err(Error::Format(
                "first two bytes are not an SOI marker".to_owned(),
            ));
        }
        let mut previous_marker = Marker::SOI;
        let mut pending_marker = None;
        let mut scans_processed = 0;
        let mut planes = vec![
            Vec::<u8>::new();
            self.frame
                .as_ref()
                .map_or(0, |frame| frame.components.len())
        ];
        let mut planes_u16 = vec![
            Vec::<u16>::new();
            self.frame
                .as_ref()
                .map_or(0, |frame| frame.components.len())
        ];
        loop {
            let marker = match pending_marker.take() {
                Some(m) => m,
                None => self.read_marker()?,
            };
            match marker {
                Marker::SOF(..) => {
                    if self.frame.is_some() {
                        return Err(Error::Unsupported(UnsupportedFeature::Hierarchical));
                    }
                    let frame = parse_sof(&mut self.reader, marker)?;
                    let component_count = frame.components.len();
                    if frame.is_differential {
                        return Err(Error::Unsupported(UnsupportedFeature::Hierarchical));
                    }
                    if frame.entropy_coding == EntropyCoding::Arithmetic {
                        return Err(Error::Unsupported(
                            UnsupportedFeature::ArithmeticEntropyCoding,
                        ));
                    }
                    if frame.precision != 8 && frame.coding_process != CodingProcess::Lossless {
                        return Err(Error::Unsupported(UnsupportedFeature::SamplePrecision(
                            frame.precision,
                        )));
                    }
                    if !(2..=16).contains(&frame.precision) {
                        return Err(Error::Unsupported(UnsupportedFeature::SamplePrecision(
                            frame.precision,
                        )));
                    }
                    if component_count != 1 && component_count != 3 && component_count != 4 {
                        return Err(Error::Unsupported(UnsupportedFeature::ComponentCount(
                            component_count as u8,
                        )));
                    }
                    let _ = Upsampler::new(
                        &frame.components,
                        frame.image_size.width,
                        frame.image_size.height,
                    )?;
                    self.frame = Some(frame);
                    if stop_after_metadata {
                        return Ok(Vec::new());
                    }
                    planes = vec![Vec::new(); component_count];
                    planes_u16 = vec![Vec::new(); component_count];
                }
                Marker::SOS => {
                    if self.frame.is_none() {
                        return Err(Error::Format("scan encountered before frame".to_owned()));
                    }
                    let frame = self.frame.clone().unwrap();
                    let scan = parse_sos(&mut self.reader, &frame)?;
                    if frame.coding_process == CodingProcess::DctProgressive
                        && self.coefficients.is_empty()
                    {
                        self.coefficients = frame
                            .components
                            .iter()
                            .map(|c| {
                                let block_count =
                                    c.block_size.width as usize * c.block_size.height as usize;
                                vec![0; block_count * 64]
                            })
                            .collect();
                    }
                    if frame.coding_process == CodingProcess::Lossless {
                        let (marker, data) = self.decode_scan_lossless(&frame, &scan)?;
                        for (i, plane) in data
                            .into_iter()
                            .enumerate()
                            .filter(|(_, plane)| !plane.is_empty())
                        {
                            planes_u16[i] = plane;
                        }
                        pending_marker = marker;
                    } else {
                        let mut finished = [false; MAX_COMPONENTS];
                        if scan.successive_approximation_low == 0 {
                            for (&i, component_finished) in
                                scan.component_indices.iter().zip(&mut finished)
                            {
                                if self.coefficients_finished[i] == !0 {
                                    continue;
                                }
                                for j in scan.spectral_selection.clone() {
                                    self.coefficients_finished[i] |= 1 << j;
                                }
                                if self.coefficients_finished[i] == !0 {
                                    *component_finished = true;
                                }
                            }
                        }
                        let preference =
                            Self::select_worker(&frame, PreferWorkerKind::Multithreaded);
                        let (marker, data) = worker_scope
                            .get_or_init_worker(preference, |worker| {
                                self.decode_scan(&frame, &scan, worker, &finished)
                            })?;
                        if let Some(data) = data {
                            for (i, plane) in data
                                .into_iter()
                                .enumerate()
                                .filter(|(_, plane)| !plane.is_empty())
                            {
                                if self.coefficients_finished[i] == !0 {
                                    planes[i] = plane;
                                }
                            }
                        }
                        pending_marker = marker;
                    }
                    scans_processed += 1;
                }
                Marker::DQT => {
                    let tables = parse_dqt(&mut self.reader)?;
                    for (i, &table) in tables.iter().enumerate() {
                        if let Some(table) = table {
                            let mut unzigzagged_table = [0u16; 64];
                            for j in 0..64 {
                                unzigzagged_table[UNZIGZAG[j] as usize] = table[j];
                            }
                            self.quantization_tables[i] = Some(Arc::new(unzigzagged_table));
                        }
                    }
                }
                Marker::DHT => {
                    let is_baseline = self.frame.as_ref().map(|frame| frame.is_baseline);
                    let (dc_tables, ac_tables) = parse_dht(&mut self.reader, is_baseline)?;
                    let current_dc_tables = mem::take(&mut self.dc_huffman_tables);
                    self.dc_huffman_tables = dc_tables
                        .into_iter()
                        .zip(current_dc_tables)
                        .map(|(a, b)| a.or(b))
                        .collect();
                    let current_ac_tables = mem::take(&mut self.ac_huffman_tables);
                    self.ac_huffman_tables = ac_tables
                        .into_iter()
                        .zip(current_ac_tables)
                        .map(|(a, b)| a.or(b))
                        .collect();
                }
                Marker::DAC => {
                    return Err(Error::Unsupported(
                        UnsupportedFeature::ArithmeticEntropyCoding,
                    ));
                }
                Marker::DRI => self.restart_interval = parse_dri(&mut self.reader)?,
                Marker::COM => {
                    let _comment = parse_com(&mut self.reader)?;
                }
                Marker::APP(..) => {
                    if let Some(data) = parse_app(&mut self.reader, marker)? {
                        match data {
                            AppData::Adobe(color_transform) => {
                                self.adobe_color_transform = Some(color_transform)
                            }
                            AppData::Jfif => {
                                self.is_jfif = true;
                            }
                            AppData::Avi1 => self.is_mjpeg = true,
                        }
                    }
                }
                Marker::RST(..) => {
                    if previous_marker != Marker::SOS {
                        return Err(Error::Format(
                            "RST found outside of entropy-coded data".to_owned(),
                        ));
                    }
                }
                Marker::DNL => {
                    if previous_marker != Marker::SOS || scans_processed != 1 {
                        return Err(Error::Format(
                            "DNL is only allowed immediately after the first scan".to_owned(),
                        ));
                    }
                    return Err(Error::Unsupported(UnsupportedFeature::DNL));
                }
                Marker::DHP | Marker::EXP => {
                    return Err(Error::Unsupported(UnsupportedFeature::Hierarchical));
                }
                Marker::EOI => break,
                _ => {
                    return Err(Error::Format(format!(
                        "{:?} marker found where not allowed",
                        marker
                    )));
                }
            }
            previous_marker = marker;
        }
        if self.frame.is_none() {
            return Err(Error::Format(
                "end of image encountered before frame".to_owned(),
            ));
        }
        let frame = self.frame.as_ref().unwrap();
        let preference = Self::select_worker(frame, PreferWorkerKind::Multithreaded);
        worker_scope.get_or_init_worker(preference, |worker| {
            self.decode_planes(worker, planes, planes_u16)
        })
    }
    fn decode_planes(
        &mut self,
        worker: &mut dyn Worker,
        mut planes: Vec<Vec<u8>>,
        planes_u16: Vec<Vec<u16>>,
    ) -> Result<Vec<u8>> {
        if self.frame.is_none() {
            return Err(Error::Format(
                "end of image encountered before frame".to_owned(),
            ));
        }
        let frame = self.frame.as_ref().unwrap();
        if frame
            .components
            .len()
            .checked_mul(frame.output_size.width.into())
            .and_then(|m| m.checked_mul(frame.output_size.height.into()))
            .is_none()
        {
            return Err(Error::Format(
                "size of decoded image exceeds maximum allowed size".to_owned(),
            ));
        }
        if frame.coding_process == CodingProcess::DctProgressive
            && self.coefficients.len() == frame.components.len()
        {
            for (i, component) in frame.components.iter().enumerate() {
                if self.coefficients_finished[i] == !0 {
                    continue;
                }
                let quantization_table =
                    match self.quantization_tables[component.quantization_table_index].clone() {
                        Some(quantization_table) => quantization_table,
                        None => continue,
                    };
                let row_data = RowData {
                    index: i,
                    component: component.clone(),
                    quantization_table,
                };
                worker.start(row_data)?;
                let coefficients_per_mcu_row = usize::from(component.block_size.width)
                    * usize::from(component.vertical_sampling_factor)
                    * 64;
                let mut tasks = (0..frame.mcu_size.height).map(|mcu_y| {
                    let offset = usize::from(mcu_y) * coefficients_per_mcu_row;
                    let row_coefficients =
                        self.coefficients[i][offset..offset + coefficients_per_mcu_row].to_vec();
                    (i, row_coefficients)
                });
                worker.append_rows(&mut tasks)?;
                planes[i] = worker.get_result(i)?;
            }
        }
        if frame.coding_process == CodingProcess::Lossless {
            compute_image_lossless(frame, planes_u16)
        } else {
            compute_image(
                &frame.components,
                planes,
                frame.output_size,
                self.determine_color_transform(),
            )
        }
    }
    fn determine_color_transform(&self) -> ColorTransform {
        let frame = self.frame.as_ref().unwrap();
        if frame.components.len() == 1 {
            return ColorTransform::Grayscale;
        }
        if frame.components.len() == 3 {
            match (
                frame.components[0].identifier,
                frame.components[1].identifier,
                frame.components[2].identifier,
            ) {
                (1, 2, 3) => {
                    return ColorTransform::YCbCr;
                }
                (1, 34, 35) => {
                    return ColorTransform::JcsBgYcc;
                }
                (82, 71, 66) => {
                    return ColorTransform::RGB;
                }
                (114, 103, 98) => {
                    return ColorTransform::JcsBgRgb;
                }
                _ => {}
            }
            if self.is_jfif {
                return ColorTransform::YCbCr;
            }
        }
        if let Some(colour_transform) = self.adobe_color_transform {
            match colour_transform {
                AdobeColorTransform::Unknown => {
                    if frame.components.len() == 3 {
                        return ColorTransform::RGB;
                    } else if frame.components.len() == 4 {
                        return ColorTransform::CMYK;
                    }
                }
                AdobeColorTransform::YCbCr => {
                    return ColorTransform::YCbCr;
                }
                AdobeColorTransform::YCCK => {
                    return ColorTransform::YCCK;
                }
            }
        } else if frame.components.len() == 4 {
            return ColorTransform::CMYK;
        }
        if frame.components.len() == 4 {
            ColorTransform::YCCK
        } else if frame.components.len() == 3 {
            ColorTransform::YCbCr
        } else {
            ColorTransform::Unknown
        }
    }
    fn read_marker(&mut self) -> Result<Marker> {
        loop {
            while read_u8(&mut self.reader)? != 0xFF {}
            let mut byte = read_u8(&mut self.reader)?;
            while byte == 0xFF {
                byte = read_u8(&mut self.reader)?;
            }
            if byte != 0x00 && byte != 0xFF {
                return Ok(Marker::from_u8(byte).unwrap());
            }
        }
    }
    fn decode_scan(
        &mut self,
        frame: &FrameInfo,
        scan: &ScanInfo,
        worker: &mut dyn Worker,
        finished: &[bool; MAX_COMPONENTS],
    ) -> Result<(Option<Marker>, Option<Vec<Vec<u8>>>)> {
        assert!(scan.component_indices.len() <= MAX_COMPONENTS);
        let components: Vec<Component> = scan
            .component_indices
            .iter()
            .map(|&i| frame.components[i].clone())
            .collect();
        if components
            .iter()
            .any(|component| self.quantization_tables[component.quantization_table_index].is_none())
        {
            return Err(Error::Format("use of unset quantization table".to_owned()));
        }
        if self.is_mjpeg {
            fill_default_mjpeg_tables(
                scan,
                &mut self.dc_huffman_tables,
                &mut self.ac_huffman_tables,
            );
        }
        if scan.spectral_selection.start == 0
            && scan
                .dc_table_indices
                .iter()
                .any(|&i| self.dc_huffman_tables[i].is_none())
        {
            return Err(Error::Format(
                "scan makes use of unset dc huffman table".to_owned(),
            ));
        }
        if scan.spectral_selection.end > 1
            && scan
                .ac_table_indices
                .iter()
                .any(|&i| self.ac_huffman_tables[i].is_none())
        {
            return Err(Error::Format(
                "scan makes use of unset ac huffman table".to_owned(),
            ));
        }
        for (i, component) in components.iter().enumerate() {
            if finished[i] {
                let row_data = RowData {
                    index: i,
                    component: component.clone(),
                    quantization_table: self.quantization_tables
                        [component.quantization_table_index]
                        .clone()
                        .unwrap(),
                };
                worker.start(row_data)?;
            }
        }
        let is_progressive = frame.coding_process == CodingProcess::DctProgressive;
        let is_interleaved = components.len() > 1;
        let mut dummy_block = [0i16; 64];
        let mut huffman = HuffmanDecoder::new();
        let mut dc_predictors = [0i16; MAX_COMPONENTS];
        let mut mcus_left_until_restart = self.restart_interval;
        let mut expected_rst_num = 0;
        let mut eob_run = 0;
        let mut mcu_row_coefficients = vec![vec![]; components.len()];
        if !is_progressive {
            for (i, component) in components.iter().enumerate().filter(|&(i, _)| finished[i]) {
                let coefficients_per_mcu_row = component.block_size.width as usize
                    * component.vertical_sampling_factor as usize
                    * 64;
                mcu_row_coefficients[i] = vec![0i16; coefficients_per_mcu_row];
            }
        }
        let (mcu_horizontal_samples, mcu_vertical_samples) = if is_interleaved {
            let horizontal = components
                .iter()
                .map(|component| component.horizontal_sampling_factor as u16)
                .collect::<Vec<_>>();
            let vertical = components
                .iter()
                .map(|component| component.vertical_sampling_factor as u16)
                .collect::<Vec<_>>();
            (horizontal, vertical)
        } else {
            (vec![1], vec![1])
        };
        let (max_mcu_x, max_mcu_y) = if is_interleaved {
            (frame.mcu_size.width, frame.mcu_size.height)
        } else {
            (
                components[0].block_size.width,
                components[0].block_size.height,
            )
        };
        for mcu_y in 0..max_mcu_y {
            if mcu_y * 8 >= frame.image_size.height {
                break;
            }
            for mcu_x in 0..max_mcu_x {
                if mcu_x * 8 >= frame.image_size.width {
                    break;
                }
                if self.restart_interval > 0 {
                    if mcus_left_until_restart == 0 {
                        match huffman.take_marker(&mut self.reader)? {
                            Some(Marker::RST(n)) => {
                                if n != expected_rst_num {
                                    return Err(Error::Format(format!(
                                        "found RST{} where RST{} was expected",
                                        n, expected_rst_num
                                    )));
                                }
                                huffman.reset();
                                dc_predictors = [0i16; MAX_COMPONENTS];
                                eob_run = 0;
                                expected_rst_num = (expected_rst_num + 1) % 8;
                                mcus_left_until_restart = self.restart_interval;
                            }
                            Some(marker) => {
                                return Err(Error::Format(format!(
                                    "found marker {:?} inside scan where RST{} was expected",
                                    marker, expected_rst_num
                                )));
                            }
                            None => {
                                return Err(Error::Format(format!(
                                    "no marker found where RST{} was expected",
                                    expected_rst_num
                                )));
                            }
                        }
                    }
                    mcus_left_until_restart -= 1;
                }
                for (i, component) in components.iter().enumerate() {
                    for v_pos in 0..mcu_vertical_samples[i] {
                        for h_pos in 0..mcu_horizontal_samples[i] {
                            let coefficients = if is_progressive {
                                let block_y = (mcu_y * mcu_vertical_samples[i] + v_pos) as usize;
                                let block_x = (mcu_x * mcu_horizontal_samples[i] + h_pos) as usize;
                                let block_offset =
                                    (block_y * component.block_size.width as usize + block_x) * 64;
                                &mut self.coefficients[scan.component_indices[i]]
                                    [block_offset..block_offset + 64]
                            } else if finished[i] {
                                let mcu_batch_current_row = if is_interleaved {
                                    0
                                } else {
                                    mcu_y % component.vertical_sampling_factor as u16
                                };
                                let block_y = (mcu_batch_current_row * mcu_vertical_samples[i]
                                    + v_pos) as usize;
                                let block_x = (mcu_x * mcu_horizontal_samples[i] + h_pos) as usize;
                                let block_offset =
                                    (block_y * component.block_size.width as usize + block_x) * 64;
                                &mut mcu_row_coefficients[i][block_offset..block_offset + 64]
                            } else {
                                &mut dummy_block[..64]
                            }
                            .try_into()
                            .unwrap();
                            if scan.successive_approximation_high == 0 {
                                decode_block(
                                    &mut self.reader,
                                    coefficients,
                                    &mut huffman,
                                    self.dc_huffman_tables[scan.dc_table_indices[i]].as_ref(),
                                    self.ac_huffman_tables[scan.ac_table_indices[i]].as_ref(),
                                    scan.spectral_selection.clone(),
                                    scan.successive_approximation_low,
                                    &mut eob_run,
                                    &mut dc_predictors[i],
                                )?;
                            } else {
                                decode_block_successive_approximation(
                                    &mut self.reader,
                                    coefficients,
                                    &mut huffman,
                                    self.ac_huffman_tables[scan.ac_table_indices[i]].as_ref(),
                                    scan.spectral_selection.clone(),
                                    scan.successive_approximation_low,
                                    &mut eob_run,
                                )?;
                            }
                        }
                    }
                }
            }
            for (i, component) in components.iter().enumerate() {
                if finished[i] {
                    if !is_interleaved
                        && (mcu_y + 1) * 8 < frame.image_size.height
                        && (mcu_y + 1) % component.vertical_sampling_factor as u16 > 0
                    {
                        continue;
                    }
                    let coefficients_per_mcu_row = component.block_size.width as usize
                        * component.vertical_sampling_factor as usize
                        * 64;
                    let row_coefficients = if is_progressive {
                        let worker_mcu_y = if is_interleaved {
                            mcu_y
                        } else {
                            mcu_y / component.vertical_sampling_factor as u16
                        };
                        let offset = worker_mcu_y as usize * coefficients_per_mcu_row;
                        self.coefficients[scan.component_indices[i]]
                            [offset..offset + coefficients_per_mcu_row]
                            .to_vec()
                    } else {
                        mem::replace(
                            &mut mcu_row_coefficients[i],
                            vec![0i16; coefficients_per_mcu_row],
                        )
                    };
                    worker.append_row((i, row_coefficients))?;
                }
            }
        }
        let mut marker = huffman.take_marker(&mut self.reader)?;
        while let Some(Marker::RST(_)) = marker {
            marker = self.read_marker().ok();
        }
        if finished.iter().any(|&c| c) {
            let mut data = vec![Vec::new(); frame.components.len()];
            for (i, &component_index) in scan.component_indices.iter().enumerate() {
                if finished[i] {
                    data[component_index] = worker.get_result(i)?;
                }
            }
            Ok((marker, Some(data)))
        } else {
            Ok((marker, None))
        }
    }
}
fn decode_block<R: Read>(
    reader: &mut R,
    coefficients: &mut [i16; 64],
    huffman: &mut HuffmanDecoder,
    dc_table: Option<&HuffmanTable>,
    ac_table: Option<&HuffmanTable>,
    spectral_selection: Range<u8>,
    successive_approximation_low: u8,
    eob_run: &mut u16,
    dc_predictor: &mut i16,
) -> Result<()> {
    debug_assert_eq!(coefficients.len(), 64);
    if spectral_selection.start == 0 {
        let value = huffman.decode(reader, dc_table.unwrap())?;
        let diff = match value {
            0 => 0,
            1..=11 => huffman.receive_extend(reader, value)?,
            _ => {
                return Err(Error::Format(
                    "invalid DC difference magnitude category".to_owned(),
                ));
            }
        };
        *dc_predictor = dc_predictor.wrapping_add(diff);
        coefficients[0] = *dc_predictor << successive_approximation_low;
    }
    let mut index = cmp::max(spectral_selection.start, 1);
    if index < spectral_selection.end && *eob_run > 0 {
        *eob_run -= 1;
        return Ok(());
    }
    while index < spectral_selection.end {
        if let Some((value, run)) = huffman.decode_fast_ac(reader, ac_table.unwrap())? {
            index += run;
            if index >= spectral_selection.end {
                break;
            }
            coefficients[UNZIGZAG[index as usize] as usize] = value << successive_approximation_low;
            index += 1;
        } else {
            let byte = huffman.decode(reader, ac_table.unwrap())?;
            let r = byte >> 4;
            let s = byte & 0x0f;
            if s == 0 {
                match r {
                    15 => index += 16,
                    _ => {
                        *eob_run = (1 << r) - 1;
                        if r > 0 {
                            *eob_run += huffman.get_bits(reader, r)?;
                        }
                        break;
                    }
                }
            } else {
                index += r;
                if index >= spectral_selection.end {
                    break;
                }
                coefficients[UNZIGZAG[index as usize] as usize] =
                    huffman.receive_extend(reader, s)? << successive_approximation_low;
                index += 1;
            }
        }
    }
    Ok(())
}
fn decode_block_successive_approximation<R: Read>(
    reader: &mut R,
    coefficients: &mut [i16; 64],
    huffman: &mut HuffmanDecoder,
    ac_table: Option<&HuffmanTable>,
    spectral_selection: Range<u8>,
    successive_approximation_low: u8,
    eob_run: &mut u16,
) -> Result<()> {
    debug_assert_eq!(coefficients.len(), 64);
    let bit = 1 << successive_approximation_low;
    if spectral_selection.start == 0 {
        if huffman.get_bits(reader, 1)? == 1 {
            coefficients[0] |= bit;
        }
    } else {
        if *eob_run > 0 {
            *eob_run -= 1;
            refine_non_zeroes(reader, coefficients, huffman, spectral_selection, 64, bit)?;
            return Ok(());
        }
        let mut index = spectral_selection.start;
        while index < spectral_selection.end {
            let byte = huffman.decode(reader, ac_table.unwrap())?;
            let r = byte >> 4;
            let s = byte & 0x0f;
            let mut zero_run_length = r;
            let mut value = 0;
            match s {
                0 => match r {
                    15 => {}
                    _ => {
                        *eob_run = (1 << r) - 1;
                        if r > 0 {
                            *eob_run += huffman.get_bits(reader, r)?;
                        }
                        zero_run_length = 64;
                    }
                },
                1 => {
                    if huffman.get_bits(reader, 1)? == 1 {
                        value = bit;
                    } else {
                        value = -bit;
                    }
                }
                _ => return Err(Error::Format("unexpected huffman code".to_owned())),
            }
            let range = Range {
                start: index,
                end: spectral_selection.end,
            };
            index = refine_non_zeroes(reader, coefficients, huffman, range, zero_run_length, bit)?;
            if value != 0 {
                coefficients[UNZIGZAG[index as usize] as usize] = value;
            }
            index += 1;
        }
    }
    Ok(())
}
fn refine_non_zeroes<R: Read>(
    reader: &mut R,
    coefficients: &mut [i16; 64],
    huffman: &mut HuffmanDecoder,
    range: Range<u8>,
    zrl: u8,
    bit: i16,
) -> Result<u8> {
    debug_assert_eq!(coefficients.len(), 64);
    let last = range.end - 1;
    let mut zero_run_length = zrl;
    for i in range {
        let index = UNZIGZAG[i as usize] as usize;
        let coefficient = &mut coefficients[index];
        if *coefficient == 0 {
            if zero_run_length == 0 {
                return Ok(i);
            }
            zero_run_length -= 1;
        } else if huffman.get_bits(reader, 1)? == 1 && *coefficient & bit == 0 {
            if *coefficient > 0 {
                *coefficient = coefficient
                    .checked_add(bit)
                    .ok_or_else(|| Error::Format("Coefficient overflow".to_owned()))?;
            } else {
                *coefficient = coefficient
                    .checked_sub(bit)
                    .ok_or_else(|| Error::Format("Coefficient overflow".to_owned()))?;
            }
        }
    }
    Ok(last)
}
fn compute_image(
    components: &[Component],
    mut data: Vec<Vec<u8>>,
    output_size: Dimensions,
    color_transform: ColorTransform,
) -> Result<Vec<u8>> {
    if data.is_empty() || data.iter().any(Vec::is_empty) {
        return Err(Error::Format("not all components have data".to_owned()));
    }
    if components.len() == 1 {
        let component = &components[0];
        let mut decoded: Vec<u8> = data.remove(0);
        let width = component.size.width as usize;
        let height = component.size.height as usize;
        let size = width * height;
        let line_stride = component.block_size.width as usize * component.dct_scale;
        if usize::from(output_size.width) != line_stride {
            for y in 1..height {
                let destination_idx = y * width;
                let source_idx = y * line_stride;
                let end = source_idx + width;
                decoded.copy_within(source_idx..end, destination_idx);
            }
        }
        decoded.resize(size, 0);
        Ok(decoded)
    } else {
        compute_image_parallel(components, data, output_size, color_transform)
    }
}
pub(crate) fn choose_color_convert_func(
    component_count: usize,
    color_transform: ColorTransform,
) -> Result<fn(&[Vec<u8>], &mut [u8])> {
    match component_count {
        3 => match color_transform {
            ColorTransform::Grayscale => Err(Error::Format(
                "Invalid number of channels (3) for Grayscale data".to_string(),
            )),
            ColorTransform::RGB => Ok(color_convert_line_rgb),
            ColorTransform::YCbCr => Ok(color_convert_line_ycbcr),
            ColorTransform::CMYK => Err(Error::Format(
                "Invalid number of channels (3) for CMYK data".to_string(),
            )),
            ColorTransform::YCCK => Err(Error::Format(
                "Invalid number of channels (3) for YCCK data".to_string(),
            )),
            ColorTransform::JcsBgYcc => Err(Error::Unsupported(
                UnsupportedFeature::ColorTransform(ColorTransform::JcsBgYcc),
            )),
            ColorTransform::JcsBgRgb => Err(Error::Unsupported(
                UnsupportedFeature::ColorTransform(ColorTransform::JcsBgRgb),
            )),
            ColorTransform::Unknown => Err(Error::Format("Unknown colour transform".to_string())),
        },
        4 => match color_transform {
            ColorTransform::Grayscale => Err(Error::Format(
                "Invalid number of channels (4) for Grayscale data".to_string(),
            )),
            ColorTransform::RGB => Err(Error::Format(
                "Invalid number of channels (4) for RGB data".to_string(),
            )),
            ColorTransform::YCbCr => Err(Error::Format(
                "Invalid number of channels (4) for YCbCr data".to_string(),
            )),
            ColorTransform::CMYK => Ok(color_convert_line_cmyk),
            ColorTransform::YCCK => Ok(color_convert_line_ycck),
            ColorTransform::JcsBgYcc => Err(Error::Unsupported(
                UnsupportedFeature::ColorTransform(ColorTransform::JcsBgYcc),
            )),
            ColorTransform::JcsBgRgb => Err(Error::Unsupported(
                UnsupportedFeature::ColorTransform(ColorTransform::JcsBgRgb),
            )),
            ColorTransform::Unknown => Err(Error::Format("Unknown colour transform".to_string())),
        },
        _ => panic!(),
    }
}
fn color_convert_line_rgb(data: &[Vec<u8>], output: &mut [u8]) {
    assert!(data.len() == 3, "wrong number of components for rgb");
    let [r, g, b]: &[Vec<u8>; 3] = data.try_into().unwrap();
    for (((chunk, r), g), b) in output
        .chunks_exact_mut(3)
        .zip(r.iter())
        .zip(g.iter())
        .zip(b.iter())
    {
        chunk[0] = *r;
        chunk[1] = *g;
        chunk[2] = *b;
    }
}
fn color_convert_line_ycbcr(data: &[Vec<u8>], output: &mut [u8]) {
    assert!(data.len() == 3, "wrong number of components for ycbcr");
    let [y, cb, cr]: &[_; 3] = data.try_into().unwrap();
    let arch_specific_pixels = {
        if let Some(ycbcr) = crate::jpeg::arch::get_color_convert_line_ycbcr() {
            unsafe { ycbcr(y, cb, cr, output) }
        } else {
            0
        }
    };
    for (((chunk, y), cb), cr) in output
        .chunks_exact_mut(3)
        .zip(y.iter())
        .zip(cb.iter())
        .zip(cr.iter())
        .skip(arch_specific_pixels)
    {
        let (r, g, b) = ycbcr_to_rgb(*y, *cb, *cr);
        chunk[0] = r;
        chunk[1] = g;
        chunk[2] = b;
    }
}
fn color_convert_line_ycck(data: &[Vec<u8>], output: &mut [u8]) {
    assert!(data.len() == 4, "wrong number of components for ycck");
    let [c, m, y, k]: &[Vec<u8>; 4] = data.try_into().unwrap();
    for ((((chunk, c), m), y), k) in output
        .chunks_exact_mut(4)
        .zip(c.iter())
        .zip(m.iter())
        .zip(y.iter())
        .zip(k.iter())
    {
        let (r, g, b) = ycbcr_to_rgb(*c, *m, *y);
        chunk[0] = r;
        chunk[1] = g;
        chunk[2] = b;
        chunk[3] = 255 - *k;
    }
}
fn color_convert_line_cmyk(data: &[Vec<u8>], output: &mut [u8]) {
    assert!(data.len() == 4, "wrong number of components for cmyk");
    let [c, m, y, k]: &[Vec<u8>; 4] = data.try_into().unwrap();
    for ((((chunk, c), m), y), k) in output
        .chunks_exact_mut(4)
        .zip(c.iter())
        .zip(m.iter())
        .zip(y.iter())
        .zip(k.iter())
    {
        chunk[0] = 255 - c;
        chunk[1] = 255 - m;
        chunk[2] = 255 - y;
        chunk[3] = 255 - k;
    }
}

const FIXED_POINT_OFFSET: i32 = 20;
const HALF: i32 = 1 << (FIXED_POINT_OFFSET - 1);
#[inline]
pub(crate) fn ycbcr_to_rgb(y: u8, cb: u8, cr: u8) -> (u8, u8, u8) {
    let y = y as i32 * (1 << FIXED_POINT_OFFSET) + HALF;
    let cb = cb as i32 - 128;
    let cr = cr as i32 - 128;
    let r = clamp_fixed_point(y + stbi_f2f(1.40200) * cr);
    let g = clamp_fixed_point(y - stbi_f2f(0.34414) * cb - stbi_f2f(0.71414) * cr);
    let b = clamp_fixed_point(y + stbi_f2f(1.77200) * cb);
    (r, g, b)
}
fn stbi_f2f(x: f32) -> i32 {
    (x * ((1 << FIXED_POINT_OFFSET) as f32) + 0.5) as i32
}
fn clamp_fixed_point(value: i32) -> u8 {
    (value >> FIXED_POINT_OFFSET).min(255).max(0) as u8
}
