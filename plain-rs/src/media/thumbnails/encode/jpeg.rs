use anyhow::{Result, ensure};
use std::sync::{LazyLock, OnceLock};

#[derive(Clone, Copy, Default)]
struct Code {
    bits: u16,
    len: u8,
}

struct Profile {
    header: Vec<u8>,
    quant: [[u8; 64]; 2],
    codes: [[Code; 256]; 4],
}

static PROFILES: [OnceLock<Profile>; 89] = [const { OnceLock::new() }; 89];
static ZIGZAG: LazyLock<[usize; 64]> = LazyLock::new(|| {
    let mut order = [0; 64];
    let mut i = 0;
    for diagonal in 0..15usize {
        let start = diagonal.saturating_sub(7);
        let end = diagonal.min(7);
        for step in start..=end {
            let row = if diagonal % 2 == 0 {
                end + start - step
            } else {
                step
            };
            order[i] = row * 8 + diagonal - row;
            i += 1;
        }
    }
    order
});
static BASIS: LazyLock<[[f64; 8]; 8]> = LazyLock::new(|| {
    std::array::from_fn(|k| {
        std::array::from_fn(|x| {
            if k == 0 {
                1.0 / 8.0f64.sqrt()
            } else {
                0.5 * (((2 * x + 1) * k) as f64 * std::f64::consts::PI / 16.0).cos()
            }
        })
    })
});

fn profile(quality: u8) -> Result<&'static Profile> {
    let slot = &PROFILES[usize::from(quality - 1)];
    if let Some(profile) = slot.get() {
        return Ok(profile);
    }
    let mut template = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut template, quality).encode(
        &[0, 0, 0],
        1,
        1,
        image::ExtendedColorType::Rgb8,
    )?;
    let value = Profile::from_template(&template)?;
    let _ = slot.set(value);
    Ok(slot.get().unwrap())
}

impl Profile {
    fn from_template(template: &[u8]) -> Result<Self> {
        let mut profile = Self {
            header: vec![0xff, 0xd8],
            quant: [[0; 64]; 2],
            codes: [[Code::default(); 256]; 4],
        };
        let mut seen_quant = [false; 2];
        let mut seen_codes = [false; 4];
        let mut pos = 2;
        while pos + 4 <= template.len() {
            ensure!(template[pos] == 0xff, "Invalid JPEG table marker");
            let marker = template[pos + 1];
            let size = usize::from(u16::from_be_bytes([template[pos + 2], template[pos + 3]]));
            ensure!(
                size >= 2 && pos + 2 + size <= template.len(),
                "Invalid JPEG table length"
            );
            let payload = &template[pos + 4..pos + 2 + size];
            match marker {
                0xda => break,
                0xdb => {
                    let mut rest = payload;
                    while !rest.is_empty() {
                        let id = usize::from(rest[0]);
                        ensure!(
                            id < 2 && rest.len() >= 65,
                            "Unsupported JPEG quantization table"
                        );
                        profile.quant[id].copy_from_slice(&rest[1..65]);
                        seen_quant[id] = true;
                        rest = &rest[65..];
                    }
                }
                0xc4 => {
                    let mut rest = payload;
                    while !rest.is_empty() {
                        ensure!(rest.len() >= 17, "Invalid JPEG Huffman table");
                        let id = usize::from(rest[0] & 15);
                        let class = usize::from(rest[0] >> 4);
                        ensure!(id < 2 && class < 2, "Unsupported JPEG Huffman table");
                        let index = id * 2 + class;
                        let counts = &rest[1..17];
                        let count = counts.iter().map(|&n| usize::from(n)).sum::<usize>();
                        ensure!(rest.len() >= 17 + count, "Truncated JPEG Huffman table");
                        let mut code = 0u32;
                        let mut offset = 17;
                        for (bits, &n) in counts.iter().enumerate() {
                            for _ in 0..n {
                                ensure!(code < (1 << (bits + 1)), "Invalid JPEG Huffman code");
                                profile.codes[index][usize::from(rest[offset])] = Code {
                                    bits: code as u16,
                                    len: (bits + 1) as u8,
                                };
                                code += 1;
                                offset += 1;
                            }
                            code <<= 1;
                        }
                        seen_codes[index] = true;
                        rest = &rest[17 + count..];
                    }
                }
                _ => {}
            }
            if marker == 0xdb || marker == 0xc4 || (0xe0..=0xef).contains(&marker) {
                profile
                    .header
                    .extend_from_slice(&template[pos..pos + 2 + size]);
            }
            pos += 2 + size;
        }
        ensure!(
            seen_quant.into_iter().all(|v| v) && seen_codes.into_iter().all(|v| v),
            "Missing JPEG encoding tables"
        );
        ensure!(
            profile.quant.iter().flatten().all(|&v| v != 0),
            "Invalid JPEG quantization divisor"
        );
        Ok(profile)
    }
}

struct Scan {
    bytes: Vec<u8>,
    pending: u32,
    len: u8,
}

impl Scan {
    fn bits(&mut self, bits: u16, len: u8) {
        self.pending = (self.pending << len) | u32::from(bits);
        self.len += len;
        while self.len >= 8 {
            self.len -= 8;
            let byte = (self.pending >> self.len) as u8;
            self.bytes.push(byte);
            if byte == 0xff {
                self.bytes.push(0);
            }
        }
        self.pending &= (1 << self.len) - 1;
    }

    fn symbol(&mut self, table: &[Code; 256], symbol: u8) -> Result<()> {
        let code = table[usize::from(symbol)];
        ensure!(code.len != 0, "Missing JPEG coefficient code");
        self.bits(code.bits, code.len);
        Ok(())
    }

    fn value(&mut self, value: i16) {
        let len = category(value);
        if len != 0 {
            let bits = if value < 0 {
                i32::from(value) + (1 << len) - 1
            } else {
                i32::from(value)
            };
            self.bits(bits as u16, len);
        }
    }

    fn block(
        &mut self,
        block: &[f64; 64],
        component: usize,
        previous: &mut i16,
        profile: &Profile,
    ) -> Result<()> {
        let table = usize::from(component != 0);
        let coefficients = transform(block, &profile.quant[table]);
        let difference = coefficients[0] - *previous;
        *previous = coefficients[0];
        self.symbol(&profile.codes[table * 2], category(difference))?;
        self.value(difference);
        let mut zeros = 0;
        for &coefficient in &coefficients[1..] {
            if coefficient == 0 {
                zeros += 1;
                continue;
            }
            while zeros >= 16 {
                self.symbol(&profile.codes[table * 2 + 1], 0xf0)?;
                zeros -= 16;
            }
            let len = category(coefficient);
            ensure!(len <= 10, "JPEG coefficient out of range");
            self.symbol(&profile.codes[table * 2 + 1], (zeros << 4) | len)?;
            self.value(coefficient);
            zeros = 0;
        }
        if zeros != 0 {
            self.symbol(&profile.codes[table * 2 + 1], 0)?;
        }
        Ok(())
    }
}

fn category(value: i16) -> u8 {
    (u16::BITS - value.unsigned_abs().leading_zeros()) as u8
}

fn transform(block: &[f64; 64], quant: &[u8; 64]) -> [i16; 64] {
    let mut rows = [[0.0; 8]; 8];
    for y in 0..8 {
        for k in 0..8 {
            rows[y][k] = (0..8)
                .map(|x| (block[y * 8 + x] - 128.0) * BASIS[k][x])
                .sum();
        }
    }
    std::array::from_fn(|index| {
        let natural = ZIGZAG[index];
        let value = (0..8)
            .map(|y| rows[y][natural % 8] * BASIS[natural / 8][y])
            .sum::<f64>();
        (value / f64::from(quant[index])).round() as i16
    })
}

fn pixel(rgb: &[u8], width: usize, height: usize, x: usize, y: usize, component: usize) -> f64 {
    let offset = (y.min(height - 1) * width + x.min(width - 1)) * 3;
    let r = f64::from(rgb[offset]);
    let g = f64::from(rgb[offset + 1]);
    let b = f64::from(rgb[offset + 2]);
    match component {
        0 => 0.299 * r + 0.587 * g + 0.114 * b,
        1 => 128.0 - 0.168736 * r - 0.331264 * g + 0.5 * b,
        _ => 128.0 + 0.5 * r - 0.418688 * g - 0.081312 * b,
    }
    .round()
    .clamp(0.0, 255.0)
}

pub(super) fn encode(rgb: &[u8], width: u16, height: u16, quality: u8) -> Result<Vec<u8>> {
    let (w, h) = (usize::from(width), usize::from(height));
    ensure!(rgb.len() == w * h * 3, "Invalid JPEG pixel buffer length");
    let profile = profile(quality)?;
    let sampling = 2;
    let mut scan = Scan {
        bytes: profile.header.clone(),
        pending: 0,
        len: 0,
    };
    scan.bytes.extend_from_slice(&[0xff, 0xc0, 0, 17, 8]);
    scan.bytes.extend_from_slice(&height.to_be_bytes());
    scan.bytes.extend_from_slice(&width.to_be_bytes());
    scan.bytes
        .extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    scan.bytes
        .extend_from_slice(&[0xff, 0xda, 0, 12, 3, 1, 0, 2, 0x11, 3, 0x11, 0, 63, 0]);
    let mut previous = [0; 3];
    for top in (0..h).step_by(8 * sampling) {
        for left in (0..w).step_by(8 * sampling) {
            for (component, previous) in previous.iter_mut().enumerate() {
                let blocks = if component == 0 { sampling } else { 1 };
                let step = if component == 0 { 1 } else { sampling };
                for by in 0..blocks {
                    for bx in 0..blocks {
                        let block = std::array::from_fn(|i| {
                            let x = left + bx * 8 + (i % 8) * step;
                            let y = top + by * 8 + (i / 8) * step;
                            let mut sum = 0.0;
                            for dy in 0..step {
                                for dx in 0..step {
                                    sum += pixel(rgb, w, h, x + dx, y + dy, component);
                                }
                            }
                            (sum / (step * step) as f64).round()
                        });
                        scan.block(&block, component, previous, profile)?;
                    }
                }
            }
        }
    }
    if scan.len != 0 {
        scan.bits((1 << (8 - scan.len)) - 1, 8 - scan.len);
    }
    scan.bytes.extend_from_slice(&[0xff, 0xd9]);
    Ok(scan.bytes)
}

#[cfg(test)]
#[path = "../../../../tests/unit/media/thumbnails/encode/jpeg.rs"]
mod tests;
