use crate::image::RgbaImage;
use crate::image::error::ImageResult;
use std::cmp::Ordering;
use std::time::Duration;
pub struct Frames<'a> {
    iterator: Box<dyn Iterator<Item = ImageResult<Frame>> + 'a>,
}
impl<'a> Frames<'a> {
    #[must_use]
    pub fn new(iterator: Box<dyn Iterator<Item = ImageResult<Frame>> + 'a>) -> Self {
        Frames { iterator }
    }
    pub fn collect_frames(self) -> ImageResult<Vec<Frame>> {
        self.collect()
    }
}
impl Iterator for Frames<'_> {
    type Item = ImageResult<Frame>;
    fn next(&mut self) -> Option<ImageResult<Frame>> {
        self.iterator.next()
    }
}
pub struct Frame {
    delay: Delay,
    left: u32,
    top: u32,
    buffer: RgbaImage,
}
impl Clone for Frame {
    fn clone(&self) -> Self {
        Self {
            delay: self.delay,
            left: self.left,
            top: self.top,
            buffer: self.buffer.clone(),
        }
    }
    fn clone_from(&mut self, source: &Self) {
        self.delay = source.delay;
        self.left = source.left;
        self.top = source.top;
        self.buffer.clone_from(&source.buffer);
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd)]
pub struct Delay {
    ratio: Ratio,
}
impl Frame {
    #[must_use]
    pub fn new(buffer: RgbaImage) -> Frame {
        Frame {
            delay: Delay::from_ratio(Ratio { numer: 0, denom: 1 }),
            left: 0,
            top: 0,
            buffer,
        }
    }
    #[must_use]
    pub fn from_parts(buffer: RgbaImage, left: u32, top: u32, delay: Delay) -> Frame {
        Frame {
            delay,
            left,
            top,
            buffer,
        }
    }
    #[must_use]
    pub fn delay(&self) -> Delay {
        self.delay
    }
    #[must_use]
    pub fn buffer(&self) -> &RgbaImage {
        &self.buffer
    }
    pub fn buffer_mut(&mut self) -> &mut RgbaImage {
        &mut self.buffer
    }
    #[must_use]
    pub fn into_buffer(self) -> RgbaImage {
        self.buffer
    }
    #[must_use]
    pub fn left(&self) -> u32 {
        self.left
    }
    #[must_use]
    pub fn top(&self) -> u32 {
        self.top
    }
}
impl Delay {
    #[must_use]
    pub fn from_numer_denom_ms(numerator: u32, denominator: u32) -> Self {
        Delay {
            ratio: Ratio::new(numerator, denominator),
        }
    }

    pub(crate) fn from_ratio(ratio: Ratio) -> Self {
        Delay { ratio }
    }
    pub(crate) fn into_ratio(self) -> Ratio {
        self.ratio
    }
    fn closest_bounded_fraction(denom_bound: u32, nom: u32, denom: u32) -> (u32, u32) {
        use std::cmp::Ordering::*;
        assert!(0 < denom);
        assert!(0 < denom_bound);
        assert!(nom < denom);
        fn compare_fraction((an, ad): (u64, u64), (bn, bd): (u64, u64)) -> Ordering {
            (an * bd).cmp(&(bn * ad))
        }
        fn abs_diff_nom((an, ad): (u64, u64), (bn, bd): (u64, u64)) -> u64 {
            let c0 = an * bd;
            let c1 = ad * bn;
            let d0 = c0.max(c1);
            let d1 = c0.min(c1);
            d0 - d1
        }
        let exact = (u64::from(nom), u64::from(denom));
        let mut lower = (0u64, 1u64);
        let mut upper = (1u64, 1u64);
        let mut guess = (u64::from(nom * 2 > denom), 1u64);
        loop {
            if compare_fraction(guess, exact) == Equal {
                break;
            }
            if u64::from(denom_bound) - lower.1 < upper.1 {
                break;
            }
            let next = (lower.0 + upper.0, lower.1 + upper.1);
            if compare_fraction(exact, next) == Less {
                upper = next;
            } else {
                lower = next;
            }
            let g_diff_nom = abs_diff_nom(guess, exact);
            let n_diff_nom = abs_diff_nom(next, exact);
            if match (n_diff_nom / next.1).cmp(&(g_diff_nom / guess.1)) {
                Less => true,
                Greater => false,
                Equal => {
                    compare_fraction(
                        (n_diff_nom % next.1, next.1),
                        (g_diff_nom % guess.1, guess.1),
                    ) == Less
                }
            } {
                guess = next;
            }
        }
        (guess.0 as u32, guess.1 as u32)
    }
}
impl From<Delay> for Duration {
    fn from(delay: Delay) -> Self {
        let ratio = delay.into_ratio();
        let ms = ratio.to_integer();
        let rest = ratio.numer % ratio.denom;
        let nanos = (u64::from(rest) * 1_000_000) / u64::from(ratio.denom);
        Duration::from_millis(ms.into()) + Duration::from_nanos(nanos)
    }
}
#[inline]
const fn gcd(mut a: u32, mut b: u32) -> u32 {
    while b != 0 {
        (a, b) = (b, a.rem_euclid(b));
    }
    a
}
#[derive(Copy, Clone, Debug)]
pub(crate) struct Ratio {
    numer: u32,
    denom: u32,
}
impl Ratio {
    #[inline]
    pub(crate) fn new(numerator: u32, denominator: u32) -> Self {
        assert_ne!(denominator, 0);
        let divisor = gcd(numerator, denominator);
        Self {
            numer: numerator / divisor,
            denom: denominator / divisor,
        }
    }
    #[inline]
    pub(crate) fn to_integer(self) -> u32 {
        self.numer / self.denom
    }
}
impl PartialEq for Ratio {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Ratio {}
impl PartialOrd for Ratio {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Ratio {
    fn cmp(&self, other: &Self) -> Ordering {
        let a: u32 = self.numer;
        let b: u32 = self.denom;
        let c: u32 = other.numer;
        let d: u32 = other.denom;
        (u64::from(a) * u64::from(d)).cmp(&(u64::from(c) * u64::from(b)))
    }
}
