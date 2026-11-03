use anyhow::{Result, ensure};
use std::sync::{LazyLock, OnceLock};

#[derive(Clone, Copy, Default)]
struct Code {
    bits: u16,
    len: u8,
}

struct Profile {
    header: Vec<u8>,
    quant: [Quantizer; 2],
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
        let mut header = vec![0xff, 0xd8];
        let mut divisors = [[0; 64]; 2];
        let mut codes = [[Code::default(); 256]; 4];
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
                        divisors[id].copy_from_slice(&rest[1..65]);
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
                                codes[index][usize::from(rest[offset])] = Code {
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
                header.extend_from_slice(&template[pos..pos + 2 + size]);
            }
            pos += 2 + size;
        }
        ensure!(
            seen_quant.into_iter().all(|v| v) && seen_codes.into_iter().all(|v| v),
            "Missing JPEG encoding tables"
        );
        Ok(Self {
            header,
            quant: [Quantizer::new(divisors[0])?, Quantizer::new(divisors[1])?],
            codes,
        })
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

// Lee's DCT-II decomposition: the odd outputs are adjacent sums of the half-size transform.
fn dct4(x: [f64; 4]) -> [f64; 4] {
    let a = x[0] + x[3];
    let b = x[1] + x[2];
    let c = (x[0] - x[3]) * 0.541196100146197;
    let d = (x[1] - x[2]) * 1.3065629648763766;
    let high = (c - d) * std::f64::consts::FRAC_1_SQRT_2;
    [
        a + b,
        c + d + high,
        (a - b) * std::f64::consts::FRAC_1_SQRT_2,
        high,
    ]
}

fn dct8(x: [f64; 8]) -> [f64; 8] {
    const SECANTS: [f64; 4] = [
        0.5097955791041592,
        0.6013448869350453,
        0.8999762231364156,
        2.5629154477415064,
    ];
    let even = dct4(std::array::from_fn(|j| x[j] + x[7 - j]));
    let odd = dct4(std::array::from_fn(|j| (x[j] - x[7 - j]) * SECANTS[j]));
    [
        even[0],
        odd[0] + odd[1],
        even[1],
        odd[1] + odd[2],
        even[2],
        odd[2] + odd[3],
        even[3],
        odd[3],
    ]
}

#[derive(Clone, Copy)]
struct Quantizer {
    divisors: [u8; 64],
    multipliers: [f64; 64],
}

impl Quantizer {
    fn new(divisors: [u8; 64]) -> Result<Self> {
        ensure!(!divisors.contains(&0), "Invalid JPEG quantization divisor");
        let zigzag = &*ZIGZAG;
        let multipliers = std::array::from_fn(|i| {
            let pos = zigzag[i];
            let scale = match (pos % 8 == 0, pos / 8 == 0) {
                (true, true) => 0.125,
                (false, false) => 0.25,
                _ => 0.25 * std::f64::consts::FRAC_1_SQRT_2,
            };
            scale / f64::from(divisors[i])
        });
        Ok(Self {
            divisors,
            multipliers,
        })
    }
}

fn transform(block: &[f64; 64], quant: &Quantizer) -> [i16; 64] {
    let rows: [[f64; 8]; 8] =
        std::array::from_fn(|y| dct8(std::array::from_fn(|x| block[y * 8 + x] - 128.0)));
    let columns: [[f64; 8]; 8] = std::array::from_fn(|x| dct8(std::array::from_fn(|y| rows[y][x])));
    let zigzag = &*ZIGZAG;
    std::array::from_fn(|i| {
        let pos = zigzag[i];
        let (x, y) = (pos % 8, pos / 8);
        let value = columns[x][y] * quant.multipliers[i];
        let magnitude = value.abs();
        let rounded = (magnitude + 0.5) as i16;
        // Retain the reference rounding at half-integers; reassociation changes the last few bits.
        if (magnitude - f64::from(rounded)).abs() >= 0.5 - 1e-10 {
            let basis = &*BASIS;
            let exact: f64 = (0..8)
                .map(|row| {
                    let horizontal: f64 = (0..8)
                        .map(|col| (block[row * 8 + col] - 128.0) * basis[x][col])
                        .sum();
                    horizontal * basis[y][row]
                })
                .sum();
            (exact / f64::from(quant.divisors[i])).round() as i16
        } else if value < 0.0 {
            -rounded
        } else {
            rounded
        }
    })
}

fn ycbcr(pixel: &[u8]) -> [u16; 3] {
    let r = f64::from(pixel[0]);
    let g = f64::from(pixel[1]);
    let b = f64::from(pixel[2]);
    [
        0.299 * r + 0.587 * g + 0.114 * b,
        128.0 - 0.168736 * r - 0.331264 * g + 0.5 * b,
        128.0 + 0.5 * r - 0.418688 * g - 0.081312 * b,
    ]
    .map(|v| ((v + 0.5) as u16).min(255))
}

fn sampling_blocks(rgb: &[u8], w: usize, h: usize, left: usize, top: usize) -> [[f64; 64]; 6] {
    let mut blocks = [[0.0; 64]; 6];
    for y in 0..8 {
        for x in 0..8 {
            let mut cb_sum = 0;
            let mut cr_sum = 0;
            for dy in 0..2 {
                let row = (top + 2 * y + dy).min(h - 1) * w;
                for dx in 0..2 {
                    let offset = 3 * (row + (left + 2 * x + dx).min(w - 1));
                    let [luma, cb, cr] = ycbcr(&rgb[offset..offset + 3]);
                    let block = (y / 4) * 2 + x / 4;
                    let index = ((2 * y + dy) % 8) * 8 + (2 * x + dx) % 8;
                    blocks[block][index] = f64::from(luma);
                    cb_sum += cb;
                    cr_sum += cr;
                }
            }
            blocks[4][8 * y + x] = f64::from((cb_sum + 2) / 4);
            blocks[5][8 * y + x] = f64::from((cr_sum + 2) / 4);
        }
    }
    blocks
}

pub(super) fn encode(rgb: &[u8], width: u16, height: u16, quality: u8) -> Result<Vec<u8>> {
    let (w, h) = (usize::from(width), usize::from(height));
    ensure!(rgb.len() == w * h * 3, "Invalid JPEG pixel buffer length");
    let profile = profile(quality)?;
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
    for top in (0..h).step_by(16) {
        for left in (0..w).step_by(16) {
            let blocks = sampling_blocks(rgb, w, h, left, top);
            for (block, component) in blocks.iter().zip([0, 0, 0, 0, 1, 2]) {
                scan.block(block, component, &mut previous[component], profile)?;
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
#[path = "../../../../../tests/plain-libs/unit/media/thumbnails/encode/jpeg.rs"]
mod tests;
