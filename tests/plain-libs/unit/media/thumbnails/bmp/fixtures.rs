use std::io::{self, BufRead, Cursor, Read, Seek, SeekFrom};

pub const FORMATS: &[(&str, bool)] = &[
    ("rgb24.bmp", false),
    ("rgb24-top-down.bmp", false),
    ("rgb32.bmp", false),
    ("rgb88832.bmp", false),
    ("rgb565.bmp", false),
    ("rgb555.bmp", false),
    ("palette1.bmp", false),
    ("palette2.bmp", false),
    ("palette4.bmp", false),
    ("palette8.bmp", false),
    ("core24.bmp", false),
    ("rle4.bmp", false),
    ("rle8.bmp", false),
    ("rle4-commands.bmp", false),
    ("rle8-commands.bmp", false),
    ("rle8-delta.bmp", false),
    ("alpha16.bmp", true),
    ("alpha32.bmp", true),
    ("alpha32-v108.bmp", true),
    ("alpha32-v124.bmp", true),
];

pub fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../plain-rs/testdata/bmp")
            .join(name),
    )
    .unwrap()
}

pub fn expanded(name: &str, width: u32, height: u32, top_down: bool) -> Vec<u8> {
    let mut bytes = fixture(name);
    let offset = u32::from_le_bytes(bytes[10..14].try_into().unwrap()) as usize;
    let channels = u16::from_le_bytes(bytes[28..30].try_into().unwrap()) as usize / 8;
    let stride = (width as usize * channels + 3) & !3;
    bytes.resize(offset + stride * height as usize, 0);
    let size = bytes.len() as u32;
    bytes[2..6].copy_from_slice(&size.to_le_bytes());
    bytes[18..22].copy_from_slice(&(width as i32).to_le_bytes());
    bytes[22..26].copy_from_slice(
        &(if top_down {
            -(height as i32)
        } else {
            height as i32
        })
        .to_le_bytes(),
    );
    bytes[34..38].copy_from_slice(&((stride * height as usize) as u32).to_le_bytes());
    for y in 0..height as usize {
        for x in 0..width as usize {
            let start = offset + y * stride + x * channels;
            bytes[start..start + 3].copy_from_slice(&[
                (x * 37 + y * 5) as u8,
                (x * 11 + y * 23) as u8,
                (x * 3 + y * 13) as u8,
            ]);
            if channels == 4 {
                bytes[start + 3] = (x * 19 + y * 7) as u8;
            }
        }
    }
    bytes
}

pub struct Counted {
    pub input: Cursor<Vec<u8>>,
    pub reads: usize,
    pub bytes: usize,
    pub largest_read: usize,
}
impl Read for Counted {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        self.reads += 1;
        self.largest_read = self.largest_read.max(bytes.len());
        let count = self.input.read(bytes)?;
        self.bytes += count;
        Ok(count)
    }
}
impl BufRead for Counted {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.input.fill_buf()
    }
    fn consume(&mut self, count: usize) {
        self.input.consume(count)
    }
}
impl Seek for Counted {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        self.input.seek(position)
    }
}
