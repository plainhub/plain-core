use std::string::{String, ToString};
use std::vec::Vec;
pub fn simd_arms() -> String {
    { "SCALAR — built without the `asm` feature (no SIMD kernels compiled in)".to_string() }
}
const KNOB_CLASS: &[(&str, &str)] = &[
    (
        "RFF_ABL_RECON",
        "ABLATION — decoder OUTPUT IS WRONG while set",
    ),
    (
        "RFF_ABL_INTRA",
        "ABLATION — decoder OUTPUT IS WRONG while set",
    ),
    ("RFF_ABL_MC", "ABLATION — decoder OUTPUT IS WRONG while set"),
    (
        "RFF_ABL_DEBLOCK",
        "ABLATION — decoder OUTPUT IS WRONG while set",
    ),
    (
        "RFF_TQ_SCALAR",
        "pins ALL transform/quant dispatch to the scalar oracle arm",
    ),
    (
        "RS_H264_QPEL_COMPOSE",
        "=1 reverts the fused qpel kernels to the compose path",
    ),
    (
        "RS_H264_DEQUANT_AVX2",
        "opt-in AVX2 dequant (measured null; default scalar)",
    ),
    (
        "RFF_HPEL_AVX2",
        "=0 pins the pre-kernel hpel-build path (encoder ME)",
    ),
    ("RFF_HPEL_FUSED", "hpel builder A/B arm (encoder ME)"),
    ("RFF_HPEL_PAD", "hpel plane border sweep knob (encoder ME)"),
    (
        "RS_H264_BS_PACKED",
        "=0 disables the packed-bS fast arm (+3-7% win)",
    ),
    ("RS_H264_NO_MBKIND", "disables deblock kind-routing"),
    (
        "RS_H264_DEBLOCK_BRANCHY",
        "selects the branchy deblock fallback arm",
    ),
    ("RS_H264_BS_TWOPASS", "bS scan-predicate A/B arm"),
    ("RS_H264_BS_PRE", "bS precompute A/B arm"),
    (
        "RS_H264_NO_SKIPBAND",
        "=1 disables the mb_skip_run band coalescer",
    ),
    (
        "RS_H264_NO_RUNMV",
        "=1 disables the P_Skip run-theorem forced-(0,0) branch",
    ),
    ("RS_H264_NO_SKIPFP", "=1 disables the P_Skip fast paths"),
    (
        "RS_H264_NO_BSKIPFAST",
        "=1 disables the B_Skip zero-bi fast path",
    ),
    (
        "RS_H264_DOUBLE_RECON",
        "=1 runs every pixel reconstruction TWICE (ablation)",
    ),
    ("RS_H264_VERIFY_MBKIND", "adds per-MB verification work"),
    ("RS_H264_VERIFY_PACKED", "adds per-MB verification work"),
    ("RS_H264_NO_POOL", "disables DPB plane pooling"),
    (
        "RS_H264_FAT_SLICE",
        "=1 allocates B-only grids on every slice (A/B)",
    ),
    (
        "RS_H264_EDC",
        "=0 disables entropy-decoupled reconstruction",
    ),
    ("RS_H264_EDC_MT", "recon worker-thread arm (default inline)"),
    ("RS_H264_EDC_BOUND", "EDC queue bound override"),
    ("RS_H264_EDC_STATS", "EDC counter telemetry"),
    (
        "RS_H264_BATCH",
        "=0 per-MB job sends (latency mode; default row batches)",
    ),
    (
        "RS_H264_NORES",
        "=0 restores full-payload no-residual jobs (A/B)",
    ),
    ("RS_H264_ROWDB", "=0 disables row-deferred deblocking"),
    (
        "RS_H264_ROWHOOK_EAGER",
        "=1 restores the per-MB row-hook body (A/B)",
    ),
    ("RS_H264_ROW_PROGRESS", "frame-MT row progress publishing"),
    ("RS_H264_ROW_PUB", "frame-MT row publishing arm"),
    ("RS_H264_FRAME_THREADS", "frame-MT worker count"),
    (
        "RS_H264_DIRECT_MEMO",
        "=0 rewalks spatial-direct neighbours every 8x8",
    ),
    ("RS_H264_KIND_LOADS", "kind-routing load-count telemetry"),
    ("RS_H264_ROUTE_DUMP", "GATE-1 router telemetry dump"),
    ("RS_H264_PROF_SAMPLE", "profiler sampling divisor"),
];
pub fn active_knobs() -> Vec<(String, String, &'static str)> {
    let mut v: Vec<(String, String, &'static str)> = std::env::vars_os()
        .filter_map(|(k, val)| {
            let k = k.to_string_lossy().into_owned();
            if !(k.starts_with("RS_H264_") || k.starts_with("RFF_")) {
                return None;
            }
            let class = KNOB_CLASS
                .iter()
                .find(|(n, _)| *n == k)
                .map(|(_, c)| *c)
                .unwrap_or("unregistered knob — read its site before trusting any number");
            Some((k, val.to_string_lossy().into_owned(), class))
        })
        .collect();
    v.sort();
    v
}
