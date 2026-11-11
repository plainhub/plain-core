use core::arch::aarch64 as arch;
#[derive(Clone)]
pub struct State {
    state: u32,
}
impl State {
    pub fn new(state: u32) -> Option<Self> {
        if std::arch::is_aarch64_feature_detected!("crc") {
            Some(Self { state })
        } else {
            None
        }
    }
    pub fn update(&mut self, buf: &[u8]) {
        self.state = unsafe { calculate(self.state, buf) }
    }
    pub fn finalize(self) -> u32 {
        self.state
    }
    pub fn reset(&mut self) {
        self.state = 0;
    }
}
const MIN_TRIPLE_QUADS: usize = 192;
#[target_feature(enable = "crc")]
pub unsafe fn calculate(crc: u32, data: &[u8]) -> u32 {
    unsafe {
        let (pre_quad, quads, post_quad) = data.align_to::<u64>();
        let mut r = crc;
        r = crc_bytes(r, pre_quad);
        if quads.len() >= MIN_TRIPLE_QUADS {
            let n3 = quads.len() / 3;
            let block_len = (n3 * 8) as u64;
            let a = &quads[0..n3];
            let b = &quads[n3..2 * n3];
            let c = &quads[2 * n3..3 * n3];
            let mut c0 = !r;
            let mut c1 = !0u32;
            let mut c2 = !0u32;
            let mut chunks_a = a.chunks_exact(4);
            let mut chunks_b = b.chunks_exact(4);
            let mut chunks_c = c.chunks_exact(4);
            for ((qa, qb), qc) in (&mut chunks_a).zip(&mut chunks_b).zip(&mut chunks_c) {
                for k in 0..4 {
                    c0 = arch::__crc32d(c0, qa[k]);
                    c1 = arch::__crc32d(c1, qb[k]);
                    c2 = arch::__crc32d(c2, qc[k]);
                }
            }
            for ((&qa, &qb), &qc) in chunks_a
                .remainder()
                .iter()
                .zip(chunks_b.remainder())
                .zip(chunks_c.remainder())
            {
                c0 = arch::__crc32d(c0, qa);
                c1 = arch::__crc32d(c1, qb);
                c2 = arch::__crc32d(c2, qc);
            }
            r = crate::checksum::crc32::combine::combine(!c0, !c1, block_len);
            r = crate::checksum::crc32::combine::combine(r, !c2, block_len);
            r = crc_quads(r, &quads[3 * n3..]);
        } else {
            r = crc_quads(r, quads);
        }
        crc_bytes(r, post_quad)
    }
}
#[target_feature(enable = "crc")]
unsafe fn crc_quads(crc: u32, quads: &[u64]) -> u32 {
    {
        let mut c = !crc;
        let mut quad_iter = quads.chunks_exact(8);
        for chunk in &mut quad_iter {
            c = arch::__crc32d(c, chunk[0]);
            c = arch::__crc32d(c, chunk[1]);
            c = arch::__crc32d(c, chunk[2]);
            c = arch::__crc32d(c, chunk[3]);
            c = arch::__crc32d(c, chunk[4]);
            c = arch::__crc32d(c, chunk[5]);
            c = arch::__crc32d(c, chunk[6]);
            c = arch::__crc32d(c, chunk[7]);
        }
        c = quad_iter
            .remainder()
            .iter()
            .fold(c, |acc, &q| arch::__crc32d(acc, q));
        !c
    }
}
#[target_feature(enable = "crc")]
unsafe fn crc_bytes(crc: u32, bytes: &[u8]) -> u32 {
    {
        let c = bytes.iter().fold(!crc, |acc, &b| arch::__crc32b(acc, b));
        !c
    }
}
