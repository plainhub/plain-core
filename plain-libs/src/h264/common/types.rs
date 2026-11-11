use std::vec;
use std::vec::Vec;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    ConstrainedBaseline,
    Baseline,
    Main,
    High,
    Other(u8),
}
impl Profile {
    pub fn profile_idc(self) -> u8 {
        match self {
            Profile::ConstrainedBaseline | Profile::Baseline => 66,
            Profile::Main => 77,
            Profile::High => 100,
            Profile::Other(v) => v,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChromaFormat {
    Monochrome,
    Yuv420,
}
impl ChromaFormat {
    pub fn idc(self) -> u8 {
        match self {
            ChromaFormat::Monochrome => 0,
            ChromaFormat::Yuv420 => 1,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct YuvFrame {
    pub width: usize,
    pub height: usize,
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
}
#[derive(Clone, Copy, Debug)]
pub struct YuvPlanes<'a> {
    pub width: usize,
    pub height: usize,
    pub y: &'a [u8],
    pub u: &'a [u8],
    pub v: &'a [u8],
    pub stride_y: usize,
    pub stride_c: usize,
}
impl<'a> YuvPlanes<'a> {
    pub fn new(
        width: usize,
        height: usize,
        y: &'a [u8],
        u: &'a [u8],
        v: &'a [u8],
        stride_y: usize,
        stride_c: usize,
    ) -> Option<Self> {
        if width == 0 || height == 0 || width % 2 != 0 || height % 2 != 0 {
            return None;
        }
        let (cw, ch) = (width / 2, height / 2);
        if stride_y < width || stride_c < cw {
            return None;
        }
        if y.len() < stride_y * (height - 1) + width
            || u.len() < stride_c * (ch - 1) + cw
            || v.len() < stride_c * (ch - 1) + cw
        {
            return None;
        }
        Some(Self {
            width,
            height,
            y,
            u,
            v,
            stride_y,
            stride_c,
        })
    }
    pub fn tight(
        width: usize,
        height: usize,
        y: &'a [u8],
        u: &'a [u8],
        v: &'a [u8],
    ) -> Option<Self> {
        Self::new(width, height, y, u, v, width, width / 2)
    }
    pub fn chroma_width(&self) -> usize {
        self.width / 2
    }
    pub fn chroma_height(&self) -> usize {
        self.height / 2
    }
    pub fn is_tight(&self) -> bool {
        self.stride_y == self.width && self.stride_c == self.chroma_width()
    }
    pub fn is_valid(&self) -> bool {
        self.is_tight()
            && self.y.len() == self.width * self.height
            && self.u.len() == self.chroma_width() * self.chroma_height()
            && self.v.len() == self.chroma_width() * self.chroma_height()
    }

    pub fn copy_into(&self, dst: &mut YuvFrame) {
        if dst.width != self.width || dst.height != self.height || !dst.is_valid() {
            *dst = YuvFrame::black(self.width, self.height);
        }
        let (w, h, cw, ch) = (
            self.width,
            self.height,
            self.chroma_width(),
            self.chroma_height(),
        );
        for r in 0..h {
            dst.y[r * w..(r + 1) * w].copy_from_slice(&self.y[r * self.stride_y..][..w]);
        }
        for r in 0..ch {
            dst.u[r * cw..(r + 1) * cw].copy_from_slice(&self.u[r * self.stride_c..][..cw]);
            dst.v[r * cw..(r + 1) * cw].copy_from_slice(&self.v[r * self.stride_c..][..cw]);
        }
    }
}
impl YuvFrame {
    pub fn black(width: usize, height: usize) -> Self {
        assert!(width % 2 == 0 && height % 2 == 0, "dimensions must be even");
        let cw = width / 2;
        let ch = height / 2;
        Self {
            width,
            height,
            y: vec![0; width * height],
            u: vec![128; cw * ch],
            v: vec![128; cw * ch],
        }
    }
    pub fn chroma_width(&self) -> usize {
        self.width / 2
    }
    pub fn chroma_height(&self) -> usize {
        self.height / 2
    }
    pub fn is_valid(&self) -> bool {
        self.y.len() == self.width * self.height
            && self.u.len() == self.chroma_width() * self.chroma_height()
            && self.v.len() == self.chroma_width() * self.chroma_height()
    }
}
