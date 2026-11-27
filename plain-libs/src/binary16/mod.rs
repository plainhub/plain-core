pub(crate) fn to_f32(value: u16) -> f32 {
    let sign = u32::from(value & 0x8000) << 16;
    let magnitude = value & 0x7fff;
    let bits = if magnitude < 0x0400 {
        (f32::from(magnitude) * (1.0 / 16_777_216.0)).to_bits()
    } else if magnitude < 0x7c00 {
        (u32::from(magnitude) << 13) + 0x3800_0000
    } else if magnitude == 0x7c00 {
        0x7f80_0000
    } else {
        0x7fc0_0000 | (u32::from(magnitude & 0x03ff) << 13)
    };
    f32::from_bits(sign | bits)
}

#[cfg(feature = "image-search-inference")]
pub(crate) const fn from_f32(value: f32) -> u16 {
    let x: u32 = value.to_bits();

    let sign = x & 0x8000_0000u32;
    let exp = x & 0x7F80_0000u32;
    let man = x & 0x007F_FFFFu32;

    if exp == 0x7F80_0000u32 {
        let nan_bit = if man == 0 { 0 } else { 0x0200u32 };
        return ((sign >> 16) | 0x7C00u32 | nan_bit | (man >> 13)) as u16;
    }

    let half_sign = sign >> 16;
    let unbiased_exp = ((exp >> 23) as i32) - 127;
    let half_exp = unbiased_exp + 15;

    if half_exp >= 0x1F {
        return (half_sign | 0x7C00u32) as u16;
    }

    if half_exp <= 0 {
        if 14 - half_exp > 24 {
            return half_sign as u16;
        }
        let man = man | 0x0080_0000u32;
        let mut half_man = man >> (14 - half_exp);
        let round_bit = 1 << (13 - half_exp);
        if (man & round_bit) != 0 && (man & (3 * round_bit - 1)) != 0 {
            half_man += 1;
        }
        return (half_sign | half_man) as u16;
    }

    let half_exp = (half_exp as u32) << 10;
    let half_man = man >> 13;
    let round_bit = 0x0000_1000u32;
    if (man & round_bit) != 0 && (man & (3 * round_bit - 1)) != 0 {
        ((half_sign | half_exp | half_man) + 1) as u16
    } else {
        (half_sign | half_exp | half_man) as u16
    }
}

#[cfg(feature = "image-search-inference")]
pub(crate) fn from_f64(value: f64) -> u16 {
    let bits = value.to_bits();
    let sign = ((bits >> 48) as u16) & 0x8000;
    let exponent = ((bits >> 52) & 0x7ff) as i32;
    let fraction = bits & ((1u64 << 52) - 1);
    if exponent == 0x7ff {
        return sign
            | 0x7c00
            | if fraction == 0 {
                0
            } else {
                0x0200 | (fraction >> 42) as u16
            };
    }
    let biased = exponent - 1023 + 15;
    if biased >= 31 {
        return sign | 0x7c00;
    }
    if biased < -10 {
        return sign;
    }
    let (base, mantissa, shift) = if biased <= 0 {
        (0, fraction | (1u64 << 52), 43 - biased as i64)
    } else {
        ((biased as u16) << 10, fraction, 42)
    };
    let kept = (mantissa >> shift) as u16;
    let remainder = mantissa & ((1u64 << shift) - 1);
    let halfway = 1u64 << (shift - 1);
    let round = u16::from(remainder > halfway || remainder == halfway && kept & 1 != 0);
    sign | ((base | kept) + round)
}

#[cfg(all(test, feature = "image-search-inference"))]
#[path = "../../../tests/plain-libs/unit/binary16.rs"]
mod tests;
