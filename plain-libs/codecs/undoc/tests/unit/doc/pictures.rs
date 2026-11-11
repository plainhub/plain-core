use super::*;

/// A `PICF` + shape container (with alternative text) + blip store entry holding `png`.
pub(crate) fn picf_with_png(png: &[u8], alt: Option<&str>) -> Vec<u8> {
    let mut shape = Vec::new();
    if let Some(alt) = alt {
        let text: Vec<u8> = alt
            .encode_utf16()
            .chain([0])
            .flat_map(|u| u.to_le_bytes())
            .collect();
        let mut fopt = Vec::new();
        fopt.extend((PROP_DESCRIPTION | 0x8000).to_le_bytes());
        fopt.extend((text.len() as u32).to_le_bytes());
        fopt.extend(&text);
        shape.extend((((1u16) << 4) | 0x3).to_le_bytes());
        shape.extend(RT_FOPT.to_le_bytes());
        shape.extend((fopt.len() as u32).to_le_bytes());
        shape.extend(fopt);
    }
    let mut sp = (0x000Fu16).to_le_bytes().to_vec();
    sp.extend(RT_SP_CONTAINER.to_le_bytes());
    sp.extend((shape.len() as u32).to_le_bytes());
    sp.extend(shape);

    let mut blip = (0x6E0u16 << 4).to_le_bytes().to_vec();
    blip.extend(0xF01Eu16.to_le_bytes());
    blip.extend(((16 + 1 + png.len()) as u32).to_le_bytes());
    blip.extend([0xAB; 16]);
    blip.push(0xFF);
    blip.extend(png);
    let mut fbse = (((6u16) << 4) | 0x2).to_le_bytes().to_vec();
    fbse.extend(RT_FBSE.to_le_bytes());
    fbse.extend(((36 + blip.len()) as u32).to_le_bytes());
    fbse.extend([0u8; 36]);
    fbse.extend(blip);

    let mut out = Vec::new();
    let total = 68 + sp.len() + fbse.len();
    out.extend((total as u32).to_le_bytes());
    out.extend(68u16.to_le_bytes());
    out.extend(0x64u16.to_le_bytes());
    out.extend([0u8; 60]);
    out.extend(sp);
    out.extend(fbse);
    out
}

#[test]
fn a_png_blip_and_its_alternative_text_are_read() {
    let png = b"\x89PNG\r\n\x1a\nfake";
    let mut data = vec![0u8; 10];
    data.extend(picf_with_png(png, Some("A chart of sales")));
    let picture = read(&data, 10).expect("a picture");
    assert_eq!(picture.extension, "png");
    assert_eq!(picture.data, png);
    assert_eq!(picture.alt_text.as_deref(), Some("A chart of sales"));
}

#[test]
fn a_picf_past_the_stream_is_none() {
    let data = picf_with_png(b"x", None);
    assert!(read(&data[..data.len() - 1], 0).is_none());
}

#[test]
fn a_compressed_emf_blip_is_inflated() {
    use std::io::Write;
    let emf = b" EMF metafile bytes".repeat(4);
    let mut z = plain_libs::compression::write::ZlibEncoder::new(Vec::new(), plain_libs::compression::Compression::default());
    z.write_all(&emf).unwrap();
    let packed = z.finish().unwrap();

    let mut body = vec![0xCD; 16];
    let mut header = vec![0u8; 34];
    header[0..4].copy_from_slice(&(emf.len() as u32).to_le_bytes());
    header[28..32].copy_from_slice(&(packed.len() as u32).to_le_bytes());
    header[33] = 0xFE;
    body.extend(header);
    body.extend(&packed);
    let mut data = (0x3D4u16 << 4).to_le_bytes().to_vec();
    data.extend(0xF01Au16.to_le_bytes());
    data.extend((body.len() as u32).to_le_bytes());
    data.extend(body);
    let blip = record(&data, 0, data.len()).unwrap();
    assert_eq!(metafile(&data, &blip, "emf"), Some((emf, "emf")));
}

#[test]
fn a_dib_becomes_a_bmp_file() {
    // 40-byte header, 1x1, 24 bits, no palette.
    let mut dib = vec![0u8; 40];
    dib[0] = 40;
    dib[14] = 24;
    dib.extend([1, 2, 3, 0]);
    let bmp = bmp_from_dib(&dib).unwrap();
    assert_eq!(&bmp[..2], b"BM");
    assert_eq!(u32_at(&bmp, 10), Some(54));
    assert_eq!(bmp.len(), 14 + dib.len());
}
