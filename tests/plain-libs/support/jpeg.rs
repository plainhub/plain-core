fn segment(out: &mut Vec<u8>, marker: u8, data: &[u8]) {
    out.extend_from_slice(&[0xff, marker]);
    out.extend_from_slice(&u16::try_from(data.len() + 2).unwrap().to_be_bytes());
    out.extend_from_slice(data);
}

pub fn constant(progressive: bool, components: u8) -> Vec<u8> {
    let mut out = vec![0xff, 0xd8];
    let mut quant = vec![0];
    quant.extend([1; 64]);
    segment(&mut out, 0xdb, &quant);
    let mut frame = vec![8, 0, 8, 0, 8, components];
    for id in 1..=components {
        frame.extend([id, 0x11, 0]);
    }
    segment(&mut out, if progressive { 0xc2 } else { 0xc0 }, &frame);
    let mut tables = Vec::new();
    for class in [0, 0x10] {
        tables.extend([class, 1]);
        tables.extend([0; 15]);
        tables.push(0);
    }
    segment(&mut out, 0xc4, &tables);
    if components == 4 {
        segment(&mut out, 0xee, b"Adobe\0\x64\0\0\0\0\0");
    }
    let mut scan = vec![components];
    for id in 1..=components {
        scan.extend([id, 0]);
    }
    scan.extend([0, if progressive { 0 } else { 63 }, 0]);
    segment(&mut out, 0xda, &scan);
    let used = usize::from(components) * if progressive { 1 } else { 2 };
    out.push(if used == 8 { 0 } else { (1 << (8 - used)) - 1 });
    if progressive {
        segment(&mut out, 0xda, &[1, 1, 0, 1, 63, 0]);
        out.push(0x7f);
    }
    out.extend([0xff, 0xd9]);
    out
}

pub fn lossless(precision: u8) -> Vec<u8> {
    let mut out = vec![0xff, 0xd8];
    segment(&mut out, 0xc3, &[precision, 0, 3, 0, 4, 1, 1, 0x11, 0]);
    let mut table = vec![0, 1];
    table.extend([0; 15]);
    table.push(0);
    segment(&mut out, 0xc4, &table);
    segment(&mut out, 0xda, &[1, 1, 0, 1, 0, 0]);
    out.extend([0, 0x0f, 0xff, 0xd9]);
    out
}
