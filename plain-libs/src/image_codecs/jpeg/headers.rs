use crate::image_codecs::jpeg::components::{Components, SampleRatios};
use crate::image_codecs::jpeg::decoder::{
    ExtendedXmpSegment, GainMapInfo, ICCChunk, JpegDecoder, MAX_COMPONENTS,
};
use crate::image_codecs::jpeg::errors::DecodeErrors;
use crate::image_codecs::jpeg::huffman::HuffmanTable;
use crate::image_codecs::jpeg::misc::{SOFMarkers, UN_ZIGZAG};
use crate::image_codecs::zune::bytestream::ZByteReaderTrait;
use crate::image_codecs::zune::colorspace::ColorSpace;
use crate::image_codecs::zune::log::{debug, trace, warn};
use core::cmp::max;
use std::format;
use std::string::ToString;
use std::vec::Vec;
pub(crate) fn parse_huffman<T: ZByteReaderTrait>(
    decoder: &mut JpegDecoder<T>,
) -> Result<(), DecodeErrors> {
    let mut dht_length = i32::from(decoder.stream.get_u16_be_err()?.checked_sub(2).ok_or(
        DecodeErrors::FormatStatic("Invalid Huffman length in image"),
    )?);
    while dht_length > 16 {
        let ht_info = decoder.stream.read_u8_err()?;
        let dc_or_ac = (ht_info >> 4) & 0xF;
        let index = (ht_info & 0xF) as usize;
        let mut num_symbols: [u8; 17] = [0; 17];
        if index >= MAX_COMPONENTS {
            return Err(DecodeErrors::HuffmanDecode(format!(
                "Invalid DHT index {index}, expected between 0 and 3"
            )));
        }
        if dc_or_ac > 1 {
            return Err(DecodeErrors::HuffmanDecode(format!(
                "Invalid DHT position {dc_or_ac}, should be 0 or 1"
            )));
        }
        decoder.stream.read_exact_bytes(&mut num_symbols[1..17])?;
        dht_length -= 1 + 16;
        let symbols_sum: i32 = num_symbols.iter().map(|f| i32::from(*f)).sum();
        if symbols_sum > 256 {
            return Err(DecodeErrors::FormatStatic(
                "Encountered Huffman table with excessive length in DHT",
            ));
        }
        if symbols_sum > dht_length {
            return Err(DecodeErrors::HuffmanDecode(format!(
                "Excessive Huffman table of length {symbols_sum} found when header length is {dht_length}"
            )));
        }
        dht_length -= symbols_sum;
        let mut symbols = [0; 256];
        decoder
            .stream
            .read_exact_bytes(&mut symbols[0..(symbols_sum as usize)])?;
        match dc_or_ac {
            0 => {
                decoder.dc_huffman_tables[index] = Some(HuffmanTable::new(
                    &num_symbols,
                    symbols,
                    true,
                    decoder.is_progressive,
                )?);
            }
            _ => {
                decoder.ac_huffman_tables[index] = Some(HuffmanTable::new(
                    &num_symbols,
                    symbols,
                    false,
                    decoder.is_progressive,
                )?);
            }
        }
    }
    if dht_length > 0 {
        return Err(DecodeErrors::FormatStatic("Bogus Huffman table definition"));
    }
    Ok(())
}
pub(crate) fn parse_dqt<T: ZByteReaderTrait>(img: &mut JpegDecoder<T>) -> Result<(), DecodeErrors> {
    let mut qt_length =
        img.stream
            .get_u16_be_err()?
            .checked_sub(2)
            .ok_or(DecodeErrors::FormatStatic(
                "Invalid DQT length. Length should be greater than 2",
            ))?;
    while qt_length > 0 {
        let qt_info = img.stream.read_u8_err()?;
        let precision = (qt_info >> 4) as usize;
        let table_position = (qt_info & 0x0f) as usize;
        let precision_value = 64 * (precision + 1);
        if (precision_value + 1) as u16 > qt_length {
            return Err(DecodeErrors::DqtError(format!(
                "Invalid QT table bytes left :{}. Too small to construct a valid qt table which should be {} long",
                qt_length,
                precision_value + 1
            )));
        }
        let dct_table = match precision {
            0 => {
                let mut qt_values = [0; 64];
                img.stream.read_exact_bytes(&mut qt_values)?;
                qt_length -= (precision_value as u16) + 1;
                un_zig_zag(&qt_values)
            }
            1 => {
                let mut qt_values = [0_u16; 64];
                for i in 0..64 {
                    qt_values[i] = img.stream.get_u16_be_err()?;
                }
                qt_length -= (precision_value as u16) + 1;
                un_zig_zag(&qt_values)
            }
            _ => {
                return Err(DecodeErrors::DqtError(format!(
                    "Expected QT precision value of either 0 or 1, found {precision:?}"
                )));
            }
        };
        if table_position >= MAX_COMPONENTS {
            return Err(DecodeErrors::DqtError(format!(
                "Too large table position for QT :{table_position}, expected between 0 and 3"
            )));
        }
        trace!("Assigning qt table {table_position} with precision {precision}");
        img.qt_tables[table_position] = Some(dct_table);
    }
    return Ok(());
}
pub(crate) fn parse_start_of_frame<T: ZByteReaderTrait>(
    sof: SOFMarkers,
    img: &mut JpegDecoder<T>,
) -> Result<(), DecodeErrors> {
    if img.seen_sof {
        return Err(DecodeErrors::SofError(
            "Two Start of Frame Markers".to_string(),
        ));
    }
    let length = img.stream.get_u16_be_err()?;
    let dt_precision = img.stream.read_u8_err()?;
    if dt_precision != 8 {
        return Err(DecodeErrors::SofError(format!(
            "The library can only parse 8-bit images, the image has {dt_precision} bits of precision"
        )));
    }
    img.info.set_density(dt_precision);
    let img_height = img.stream.get_u16_be_err()?;
    img.info.set_height(img_height);
    let img_width = img.stream.get_u16_be_err()?;
    img.info.set_width(img_width);
    trace!("Image width  :{}", img_width);
    trace!("Image height :{}", img_height);
    if usize::from(img_width) > img.options.max_width() {
        return Err(DecodeErrors::Format(format!(
            "Image width {} greater than width limit {}. If use `set_limits` if you want to support huge images",
            img_width,
            img.options.max_width()
        )));
    }
    if usize::from(img_height) > img.options.max_height() {
        return Err(DecodeErrors::Format(format!(
            "Image height {} greater than height limit {}. If use `set_limits` if you want to support huge images",
            img_height,
            img.options.max_height()
        )));
    }
    if img_width == 0 || img_height == 0 {
        return Err(DecodeErrors::ZeroError);
    }
    let num_components = img.stream.read_u8_err()?;
    if num_components == 0 {
        return Err(DecodeErrors::SofError(
            "Number of components cannot be zero.".to_string(),
        ));
    }
    let expected = 8 + 3 * u16::from(num_components);
    if length != expected {
        return Err(DecodeErrors::SofError(format!(
            "Length of start of frame differs from expected {expected},value is {length}"
        )));
    }
    trace!("Image components : {}", num_components);
    if num_components == 1 {
        img.input_colorspace = ColorSpace::Luma;
        debug!("Overriding default colorspace set to Luma");
    }
    if num_components == 4 && img.input_colorspace == ColorSpace::YCbCr {
        trace!("Input image has 4 components, defaulting to CMYK colorspace");
        img.input_colorspace = ColorSpace::CMYK;
    }
    img.info.components = num_components;
    let mut components = Vec::with_capacity(num_components as usize);
    let mut temp = [0; 3];
    for pos in 0..num_components {
        img.stream.read_exact_bytes(&mut temp)?;
        let component = Components::from(temp, pos)?;
        components.push(component);
    }
    img.seen_sof = true;
    img.info.set_sof_marker(sof);
    img.components = components;
    let mut h_max = 1;
    let mut v_max = 1;
    for comp in &img.components {
        h_max = max(h_max, comp.horizontal_sample);
        v_max = max(v_max, comp.vertical_sample);
    }
    img.info.sample_ratio = match (h_max, v_max) {
        (1, 1) => SampleRatios::None,
        (1, 2) => SampleRatios::V,
        (2, 1) => SampleRatios::H,
        (2, 2) => SampleRatios::HV,
        (hs, vs) => SampleRatios::Generic(hs, vs),
    };
    Ok(())
}
pub(crate) fn parse_sos<T: ZByteReaderTrait>(
    image: &mut JpegDecoder<T>,
) -> Result<(), DecodeErrors> {
    let ls = usize::from(image.stream.get_u16_be_err()?);
    let ns = image.stream.read_u8_err()?;
    let mut seen: [_; 5] = [-1; { MAX_COMPONENTS + 1 }];
    image.num_scans = ns;
    let smallest_size = 6 + 2 * usize::from(ns);
    if ls != smallest_size {
        return Err(DecodeErrors::SosError(format!(
            "Bad SOS length {ls},corrupt jpeg"
        )));
    }
    if !(1..5).contains(&ns) {
        return Err(DecodeErrors::SosError(format!(
            "Invalid number of components in start of scan {ns}, expected in range 1..5"
        )));
    }
    if image.info.components == 0 {
        return Err(DecodeErrors::FormatStatic(
            "Error decoding SOF Marker, Number of components cannot be zero.",
        ));
    }
    image.scan_subsampled = false;
    for i in 0..ns {
        let id = image.stream.read_u8_err()?;
        if seen.contains(&i32::from(id)) {
            return Err(DecodeErrors::SofError(format!(
                "Duplicate ID {id} seen twice in the same component"
            )));
        }
        seen[usize::from(i)] = i32::from(id);
        let y = image.stream.read_u8_err()?;
        let mut j = 0;
        while j < image.info.components {
            if image.components[j as usize].id == id {
                break;
            }
            j += 1;
        }
        if j == image.info.components {
            return Err(DecodeErrors::SofError(format!(
                "Invalid component id {}, expected one one of {:?}",
                id,
                image.components.iter().map(|c| c.id).collect::<Vec<_>>()
            )));
        }
        let component = &mut image.components[usize::from(j)];
        component.dc_huff_table = usize::from((y >> 4) & 0xF);
        component.ac_huff_table = usize::from(y & 0xF);
        image.z_order[i as usize] = j as usize;
        if component.vertical_sample != 1 || component.horizontal_sample != 1 {
            image.scan_subsampled = true;
        }
        trace!(
            "Assigned huffman tables {}/{} to component {j}, id={}",
            image.components[usize::from(j)].dc_huff_table,
            image.components[usize::from(j)].ac_huff_table,
            image.components[usize::from(j)].id,
        );
    }
    image.spec_start = image.stream.read_u8_err()?;
    image.spec_end = image.stream.read_u8_err()?;
    let bit_approx = image.stream.read_u8_err()?;
    image.succ_high = bit_approx >> 4;
    if image.spec_end > 63 {
        return Err(DecodeErrors::SosError(format!(
            "Invalid Se parameter {}, range should be 0-63",
            image.spec_end
        )));
    }
    if image.spec_start > 63 {
        return Err(DecodeErrors::SosError(format!(
            "Invalid Ss parameter {}, range should be 0-63",
            image.spec_start
        )));
    }
    if image.succ_high > 13 {
        return Err(DecodeErrors::SosError(format!(
            "Invalid Ah parameter {}, range should be 0-13",
            image.succ_low
        )));
    }
    image.succ_low = bit_approx & 0xF;
    if image.succ_low > 13 {
        return Err(DecodeErrors::SosError(format!(
            "Invalid Al parameter {}, range should be 0-13",
            image.succ_low
        )));
    }
    image.stream.skip(smallest_size.saturating_sub(ls))?;
    trace!(
        "Ss={}, Se={} Ah={} Al={}",
        image.spec_start, image.spec_end, image.succ_high, image.succ_low
    );
    Ok(())
}
pub(crate) fn parse_app13<T: ZByteReaderTrait>(
    decoder: &mut JpegDecoder<T>,
) -> Result<(), DecodeErrors> {
    const IPTC_PREFIX: &[u8] = b"Photoshop 3.0\0";
    let mut length = usize::from(decoder.stream.get_u16_be());
    if length < 2 {
        return Err(DecodeErrors::FormatStatic("Too small APP13 length"));
    }
    length -= 2;
    if length > IPTC_PREFIX.len() && decoder.stream.peek_at(0, IPTC_PREFIX.len())? == IPTC_PREFIX {
        decoder.stream.skip(IPTC_PREFIX.len())?;
        length -= IPTC_PREFIX.len();
        let iptc_bytes = decoder.stream.peek_at(0, length)?.to_vec();
        decoder.info.iptc_data = Some(iptc_bytes);
    }
    decoder.stream.skip(length)?;
    Ok(())
}
pub(crate) fn parse_app14<T: ZByteReaderTrait>(
    decoder: &mut JpegDecoder<T>,
) -> Result<(), DecodeErrors> {
    let mut length = usize::from(decoder.stream.get_u16_be());
    if length < 2 {
        return Err(DecodeErrors::FormatStatic("Too small APP14 length"));
    }
    if decoder.stream.peek_at(0, 5)? == b"Adobe" {
        if length < 14 {
            return Err(DecodeErrors::FormatStatic(
                "Too short of a length for App14 segment",
            ));
        }
        decoder.stream.skip(6)?;
        decoder.stream.skip(5)?;
        let transform = decoder.stream.read_u8();
        match transform {
            0 => decoder.input_colorspace = ColorSpace::CMYK,
            1 => decoder.input_colorspace = ColorSpace::YCbCr,
            2 => decoder.input_colorspace = ColorSpace::YCCK,
            _ => {
                return Err(DecodeErrors::Format(format!(
                    "Unknown Adobe colorspace {transform}"
                )));
            }
        }
        length = length.saturating_sub(14);
    } else {
        warn!("Not a valid Adobe APP14 Segment, skipping {} bytes", length);
        length = length.saturating_sub(2);
    }
    decoder.stream.skip(length)?;
    Ok(())
}
pub(crate) fn parse_app1<T: ZByteReaderTrait>(
    decoder: &mut JpegDecoder<T>,
) -> Result<(), DecodeErrors> {
    const XMP_NAMESPACE_PREFIX: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
    const EXTENDED_XMP_NAMESPACE_PREFIX: &[u8] = b"http://ns.adobe.com/xmp/extension/\0";
    const EXTENDED_XMP_GUID_SIZE: usize = 32;
    const EXTENDED_XMP_TOTAL_SIZE_SIZE: usize = 4;
    const EXTENDED_XMP_OFFSET_SIZE: usize = 4;
    const EXTENDED_XMP_HEADER_SIZE: usize =
        EXTENDED_XMP_GUID_SIZE + EXTENDED_XMP_TOTAL_SIZE_SIZE + EXTENDED_XMP_OFFSET_SIZE;
    let mut length = usize::from(decoder.stream.get_u16_be());
    if length < 2 {
        return Err(DecodeErrors::FormatStatic("Too small app1 length"));
    }
    length -= 2;
    if length > 6 && decoder.stream.peek_at(0, 6)? == b"Exif\x00\x00" {
        trace!("Exif segment present");
        decoder.stream.skip(6)?;
        length -= 6;
        let exif_bytes = decoder.stream.peek_at(0, length)?.to_vec();
        decoder.info.exif_data = Some(exif_bytes);
    } else if length > XMP_NAMESPACE_PREFIX.len()
        && decoder.stream.peek_at(0, XMP_NAMESPACE_PREFIX.len())? == XMP_NAMESPACE_PREFIX
    {
        trace!("XMP Data Present");
        decoder.stream.skip(XMP_NAMESPACE_PREFIX.len())?;
        length -= XMP_NAMESPACE_PREFIX.len();
        let xmp_data = decoder.stream.peek_at(0, length)?.to_vec();
        decoder.info.xmp_data = Some(xmp_data);
    } else if length > EXTENDED_XMP_NAMESPACE_PREFIX.len()
        && decoder
            .stream
            .peek_at(0, EXTENDED_XMP_NAMESPACE_PREFIX.len())?
            == EXTENDED_XMP_NAMESPACE_PREFIX
    {
        trace!("Extended XMP Data Present");
        decoder.stream.skip(EXTENDED_XMP_NAMESPACE_PREFIX.len())?;
        length -= EXTENDED_XMP_NAMESPACE_PREFIX.len();
        if length < EXTENDED_XMP_HEADER_SIZE {
            return Err(DecodeErrors::FormatStatic("Too small Extended XMP segment"));
        }
        let header = decoder.stream.peek_at(0, EXTENDED_XMP_HEADER_SIZE)?;
        let guid = header[0..EXTENDED_XMP_GUID_SIZE].to_vec();
        let total_size_start = EXTENDED_XMP_GUID_SIZE;
        let total_size_end = total_size_start + EXTENDED_XMP_TOTAL_SIZE_SIZE;
        let total_size =
            u32::from_be_bytes(header[total_size_start..total_size_end].try_into().unwrap());
        let offset_start = total_size_end;
        let offset_end = offset_start + EXTENDED_XMP_OFFSET_SIZE;
        let offset = u32::from_be_bytes(header[offset_start..offset_end].try_into().unwrap());
        let data = decoder
            .stream
            .peek_at(EXTENDED_XMP_HEADER_SIZE, length - EXTENDED_XMP_HEADER_SIZE)?
            .to_vec();
        decoder.extended_xmp_segments.push(ExtendedXmpSegment {
            offset,
            total_size,
            guid,
            data,
        });
    } else {
        warn!("Unknown format for APP1 tag, skipping");
    }
    decoder.stream.skip(length)?;
    Ok(())
}
pub(crate) fn parse_app2<T: ZByteReaderTrait>(
    decoder: &mut JpegDecoder<T>,
) -> Result<(), DecodeErrors> {
    static HDR_META: &[u8] = b"urn:iso:std:iso:ts:21496:-1\0";
    static MPF_DATA: &[u8] = b"MPF\0";
    let mut length = usize::from(decoder.stream.get_u16_be());
    if length < 2 {
        return Err(DecodeErrors::FormatStatic("Too small app2 segment"));
    }
    length -= 2;
    if length > 14 && decoder.stream.peek_at(0, 12)? == *b"ICC_PROFILE\0" {
        trace!("ICC Profile present");
        length -= 12;
        decoder.stream.skip(12)?;
        let seq_no = decoder.stream.read_u8();
        let num_markers = decoder.stream.read_u8();
        length -= 2;
        let data = decoder.stream.peek_at(0, length)?.to_vec();
        let icc_chunk = ICCChunk {
            seq_no,
            num_markers,
            data,
        };
        decoder.icc_data.push(icc_chunk);
    } else if length > HDR_META.len() && decoder.stream.peek_at(0, HDR_META.len())? == HDR_META {
        length = length.saturating_sub(HDR_META.len());
        decoder.stream.skip(HDR_META.len())?;
        trace!("Gain Map metadata found");
        match length {
            4 => {
                let _ = decoder.stream.get_u16_be();
                let _ = decoder.stream.get_u16_be();
                length -= 4;
                decoder
                    .info
                    .gain_map_info
                    .push(GainMapInfo { data: Vec::new() });
            }
            n if n > 4 => {
                let data = decoder.stream.peek_at(0, length)?.to_vec();
                length -= data.len();
                decoder.stream.skip(data.len())?;
                decoder.info.gain_map_info.push(GainMapInfo { data });
            }
            _ => {}
        }
    } else if length > MPF_DATA.len() && decoder.stream.peek_at(0, MPF_DATA.len())? == MPF_DATA {
        trace!("MPF Signature present");
        length = length.saturating_sub(MPF_DATA.len());
        decoder.stream.skip(MPF_DATA.len())?;
        decoder.info.multi_picture_information_offset = Some(decoder.stream.position()?);
        let data = decoder.stream.peek_at(0, length)?.to_vec();
        length -= data.len();
        decoder.stream.skip(data.len())?;
        decoder.info.multi_picture_information = Some(data);
    }
    decoder.stream.skip(length)?;
    Ok(())
}
fn un_zig_zag<T>(a: &[T]) -> [i32; 64]
where
    T: Default + Copy,
    i32: core::convert::From<T>,
{
    let mut output = [i32::default(); 64];
    for i in 0..64 {
        output[UN_ZIGZAG[i]] = i32::from(a[i]);
    }
    output
}
