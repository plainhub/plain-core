use super::scale::Bitmap;

const SAMPLE_EDGE: u32 = 32;

pub(super) fn from_bitmap(bitmap: &Bitmap) -> Vec<u8> {
    let (pixels, width, height) = bitmap.raw();
    let channels = bitmap.channels();
    let longest = width.max(height).max(SAMPLE_EDGE);
    let sample_width = (width * SAMPLE_EDGE).div_ceil(longest);
    let sample_height = (height * SAMPLE_EDGE).div_ceil(longest);
    let mut samples = Vec::with_capacity((sample_width * sample_height) as usize);
    for y in 0..sample_height {
        let sy = ((2 * y + 1) * height / (2 * sample_height)).min(height - 1);
        for x in 0..sample_width {
            let sx = ((2 * x + 1) * width / (2 * sample_width)).min(width - 1);
            let index = ((sy * width + sx) as usize) * channels;
            let pixel = &pixels[index..index + channels];
            samples.push([
                f64::from(pixel[0]) / 255.0,
                f64::from(pixel[1]) / 255.0,
                f64::from(pixel[2]) / 255.0,
                f64::from(pixel.get(3).copied().unwrap_or(255)) / 255.0,
            ]);
        }
    }
    encode(sample_width as usize, sample_height as usize, &mut samples)
}

fn encode(width: usize, height: usize, samples: &mut [[f64; 4]]) -> Vec<u8> {
    let alpha_sum = samples.iter().map(|pixel| pixel[3]).sum::<f64>();
    let has_alpha = alpha_sum < samples.len() as f64;
    let mut average = [0.0; 3];
    if has_alpha && alpha_sum > 0.0 {
        for &[r, g, b, a] in samples.iter() {
            average[0] += r * a;
            average[1] += g * a;
            average[2] += b * a;
        }
        average.iter_mut().for_each(|value| *value /= alpha_sum);
    }
    for [r, g, b, a] in samples.iter_mut() {
        let red = average[0] * (1.0 - *a) + *r * *a;
        let green = average[1] * (1.0 - *a) + *g * *a;
        let blue = average[2] * (1.0 - *a) + *b * *a;
        *r = (red + green + blue) / 3.0;
        *g = (red + green) / 2.0 - blue;
        *b = red - green;
    }

    let max_terms = if has_alpha { 5 } else { 7 };
    let longest = width.max(height);
    let terms_for = |length| (max_terms * length + longest / 2) / longest;
    let lx = terms_for(width).max(1);
    let ly = terms_for(height).max(1);
    let landscape = width > height;
    let bx = basis(width);
    let by = basis(height);
    let channel = |index: usize, nx: usize, ny: usize| {
        let mut dc = 0.0;
        let mut ac = Vec::new();
        let mut scale: f64 = 0.0;
        for (cy, vy) in by.iter().enumerate().take(ny) {
            for (cx, vx) in bx.iter().enumerate().take(nx) {
                if cx * ny + cy * nx >= nx * ny {
                    break;
                }
                let mut value = 0.0;
                for (row, y) in samples.chunks_exact(width).zip(vy) {
                    for (pixel, x) in row.iter().zip(vx) {
                        value += pixel[index] * x * y;
                    }
                }
                value /= samples.len() as f64;
                if cx == 0 && cy == 0 {
                    dc = value;
                } else {
                    ac.push(value);
                    scale = scale.max(value.abs());
                }
            }
        }
        (dc, scale, ac)
    };
    let l = channel(0, lx.max(3), ly.max(3));
    let p = channel(1, 3, 3);
    let q = channel(2, 3, 3);
    let a = has_alpha.then(|| channel(3, 5, 5));
    let quantize = |value: f64, max: f64| (value.clamp(0.0, 1.0) * max).round() as u32;
    let header24 = quantize(l.0, 63.0)
        | (quantize((p.0 + 1.0) / 2.0, 63.0) << 6)
        | (quantize((q.0 + 1.0) / 2.0, 63.0) << 12)
        | (quantize(l.1, 31.0) << 18)
        | (u32::from(has_alpha) << 23);
    let header16 = (if landscape { ly } else { lx }) as u32
        | (quantize(p.1, 63.0) << 3)
        | (quantize(q.1, 63.0) << 9)
        | (u32::from(landscape) << 15);
    let mut hash = header24.to_le_bytes()[..3].to_vec();
    hash.extend_from_slice(&header16.to_le_bytes()[..2]);
    if let Some((dc, scale, _)) = &a {
        hash.push((quantize(*dc, 15.0) | (quantize(*scale, 15.0) << 4)) as u8);
    }
    let coefficients = [l, p, q].into_iter().chain(a).flat_map(|(_, scale, ac)| {
        ac.into_iter().map(move |value| {
            if scale == 0.0 {
                0
            } else {
                quantize(0.5 + 0.5 / scale * value, 15.0) as u8
            }
        })
    });
    for (index, nibble) in coefficients.enumerate() {
        if index % 2 == 0 {
            hash.push(nibble);
        } else {
            *hash.last_mut().unwrap() |= nibble << 4;
        }
    }
    hash
}

fn basis(length: usize) -> [Vec<f64>; 7] {
    std::array::from_fn(|frequency| {
        (0..length)
            .map(|position| {
                (std::f64::consts::PI / length as f64 * frequency as f64 * (position as f64 + 0.5))
                    .cos()
            })
            .collect()
    })
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/media/thumbnails/thumb_hash.rs"]
mod tests;
