use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::vec::Vec;
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum DisposalMethod {
    Any = 0,
    Keep = 1,
    Background = 2,
    Previous = 3,
}
impl DisposalMethod {
    #[must_use]
    pub const fn from_u8(n: u8) -> Option<Self> {
        match n {
            0 => Some(Self::Any),
            1 => Some(Self::Keep),
            2 => Some(Self::Background),
            3 => Some(Self::Previous),
            _ => None,
        }
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum Block {
    Image = 0x2C,
    Extension = 0x21,
    Trailer = 0x3B,
}
impl Block {
    #[must_use]
    pub const fn from_u8(n: u8) -> Option<Self> {
        match n {
            0x2C => Some(Self::Image),
            0x21 => Some(Self::Extension),
            0x3B => Some(Self::Trailer),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AnyExtension(pub u8);
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum Extension {
    Text = 0x01,
    Control = 0xF9,
    Comment = 0xFE,
    Application = 0xFF,
}
impl AnyExtension {
    #[must_use]
    pub const fn into_known(self) -> Option<Extension> {
        Extension::from_u8(self.0)
    }
}
impl From<Extension> for AnyExtension {
    fn from(ext: Extension) -> Self {
        Self(ext as u8)
    }
}
impl Extension {
    #[must_use]
    pub const fn from_u8(n: u8) -> Option<Self> {
        match n {
            0x01 => Some(Self::Text),
            0xF9 => Some(Self::Control),
            0xFE => Some(Self::Comment),
            0xFF => Some(Self::Application),
            _ => None,
        }
    }
}
#[derive(Debug, Clone)]
pub struct Frame<'a> {
    pub delay: u16,
    pub dispose: DisposalMethod,
    pub transparent: Option<u8>,
    pub needs_user_input: bool,
    pub top: u16,
    pub left: u16,
    pub width: u16,
    pub height: u16,
    pub interlaced: bool,
    pub palette: Option<Vec<u8>>,
    pub buffer: Cow<'a, [u8]>,
}
impl Default for Frame<'_> {
    fn default() -> Self {
        Frame {
            delay: 0,
            dispose: DisposalMethod::Keep,
            transparent: None,
            needs_user_input: false,
            top: 0,
            left: 0,
            width: 0,
            height: 0,
            interlaced: false,
            palette: None,
            buffer: Cow::Borrowed(&[]),
        }
    }
}
impl Frame<'static> {
    #[track_caller]
    pub fn from_rgba_speed(width: u16, height: u16, pixels: &mut [u8], speed: i32) -> Self {
        assert_eq!(
            width as usize * height as usize * 4,
            pixels.len(),
            "Too much or too little pixel data for the given width and height to create a GIF Frame"
        );
        assert!(
            speed >= 1 && speed <= 30,
            "speed needs to be in the range [1, 30]"
        );
        let mut transparent: Option<[u8; 4]> = None;
        for pix in pixels.chunks_exact_mut(4) {
            if pix[3] != 0 {
                pix[3] = 0xFF;
                continue;
            }
            if let Some([r, g, b, a]) = transparent {
                pix[0] = r;
                pix[1] = g;
                pix[2] = b;
                pix[3] = a;
            } else {
                transparent = Some([pix[0], pix[1], pix[2], pix[3]]);
            }
        }
        let mut colors: BTreeSet<(u8, u8, u8, u8)> = BTreeSet::new();
        for pixel in pixels.chunks_exact(4) {
            if colors.insert((pixel[0], pixel[1], pixel[2], pixel[3])) && colors.len() > 256 {
                let nq = crate::image_codecs::quantize::NeuQuant::new(speed, 256, pixels);
                return Frame {
                    width,
                    height,
                    buffer: Cow::Owned(
                        pixels
                            .chunks_exact(4)
                            .map(|pix| nq.index_of(pix) as u8)
                            .collect(),
                    ),
                    palette: Some(nq.color_map_rgb()),
                    transparent: transparent.map(|t| nq.index_of(&t) as u8),
                    ..Frame::default()
                };
            }
        }
        let mut colors_vec: Vec<(u8, u8, u8, u8)> = colors.into_iter().collect();
        colors_vec.sort_unstable();
        let palette = colors_vec
            .iter()
            .flat_map(|&(r, g, b, _a)| [r, g, b])
            .collect();
        let colors_lookup: BTreeMap<(u8, u8, u8, u8), u8> =
            colors_vec.into_iter().zip(0..=255).collect();
        let index_of = |pixel: &[u8]| {
            colors_lookup
                .get(&(pixel[0], pixel[1], pixel[2], pixel[3]))
                .copied()
                .unwrap_or(0)
        };
        Frame {
            width,
            height,
            buffer: Cow::Owned(pixels.chunks_exact(4).map(index_of).collect()),
            palette: Some(palette),
            transparent: transparent.map(|t| index_of(&t)),
            ..Frame::default()
        }
    }

    #[must_use]
    #[track_caller]
    pub fn from_rgb_speed(width: u16, height: u16, pixels: &[u8], speed: i32) -> Self {
        assert_eq!(
            width as usize * height as usize * 3,
            pixels.len(),
            "Too much or too little pixel data for the given width and height to create a GIF Frame"
        );
        let mut vec: Vec<u8> = Vec::new();
        vec.try_reserve_exact(pixels.len() + width as usize * height as usize)
            .expect("OOM");
        for v in pixels.chunks_exact(3) {
            vec.extend_from_slice(&[v[0], v[1], v[2], 0xFF]);
        }
        Frame::from_rgba_speed(width, height, &mut vec, speed)
    }
    #[inline]
    pub(crate) fn take(&mut self) -> Self {
        Frame {
            delay: self.delay,
            dispose: self.dispose,
            transparent: self.transparent,
            needs_user_input: self.needs_user_input,
            top: self.top,
            left: self.left,
            width: self.width,
            height: self.height,
            interlaced: self.interlaced,
            palette: core::mem::take(&mut self.palette),
            buffer: core::mem::replace(&mut self.buffer, Cow::Borrowed(&[])),
        }
    }
}
