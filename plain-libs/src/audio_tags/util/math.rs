pub(crate) trait RoundedDivision<Rhs = Self> {
    type Output;

    fn div_round(self, rhs: Rhs) -> Self::Output;
}

macro_rules! unsigned_rounded_division {
	($($t:ty),*) => {
		$(
			impl RoundedDivision for $t {
				type Output = $t;

				fn div_round(self, rhs: Self) -> Self::Output {
					(self + (rhs >> 1)) / rhs
				}
			}
		)*
	};
}

unsigned_rounded_division!(u8, u16, u32, u64, u128, usize);

#[derive(Debug, Eq, PartialEq, Copy, Clone)]
pub(crate) struct F80 {
    signed: bool,
    // 15-bit exponent with a bias of 16383
    exponent: u16,
    fraction: u64,
}

impl F80 {
    pub fn from_be_bytes(bytes: [u8; 10]) -> Self {
        let signed = bytes[0] & 0x80 != 0;
        let exponent = (u16::from(bytes[0] & 0x7F) << 8) | u16::from(bytes[1]);

        let mut fraction_bytes = [0; 8];
        fraction_bytes.copy_from_slice(&bytes[2..]);
        let fraction = u64::from_be_bytes(fraction_bytes);

        Self {
            signed,
            exponent,
            fraction,
        }
    }

    pub fn as_f64(&self) -> f64 {
        // Apple® Apple Numerics Manual, Second Edition, Table 2-7:
        //
        // Biased exponent e  Integer i  Fraction f          Value v                     Class of v
        // 0 <= e <= 32766       1         (any)     v = (-1)^s * 2^(e-16383) * (1.f)   Normalized
        // 0 <= e <= 32766       0         f != 0    v = (-1)^s * 2^(e-16383) * (0.f)   Denormalized
        // 0 <= e <= 32766       0         f = 0     v = (-1)^s * 0                     Zero
        //    e = 32767        (any)       f = 0     v = (-1)^s * Infinity              Infinity
        //    e = 32767        (any)       f != 0    v is a NaN                         NaN

        let sign = if self.signed { 1 } else { 0 };

        // e = 32767
        if self.exponent == 32767 {
            if self.fraction == 0 {
                return f64::from_bits((sign << 63) | f64::INFINITY.to_bits());
            }

            return f64::from_bits((sign << 63) | f64::NAN.to_bits());
        }

        // 0 <= e <= 32766, i = 0, f = 0
        if self.fraction == 0 {
            return f64::from_bits(sign << 63);
        }

        // 0 <= e <= 32766, 0 <= i <= 1, f >= 0

        let fraction = self.fraction & 0x7FFF_FFFF_FFFF_FFFF;
        let exponent = self.exponent as i16 - 16383 + 1023;
        let bits = (sign << 63) | ((exponent as u64) << 52) | (fraction >> 11);

        f64::from_bits(bits)
    }
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/audio_tags/util/math/tests.rs"]
mod tests;
