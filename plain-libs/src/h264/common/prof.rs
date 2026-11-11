use std::vec;
#[derive(Clone, Copy)]
pub enum Stage {
    Entropy = 0,
    IntraPred = 1,
    InterMc = 2,
    Reconstruct = 3,
    Deblock = 4,
    Dequant = 5,
    Scatter = 6,
    PredBuf = 7,
    MvGrid = 8,
    Neighbors = 9,
    SkipRecon = 10,
    Finalize = 11,
    Syntax = 12,
    DpbClone = 13,
    EncSource = 14,
    EncSkip = 15,
    EncMe = 16,
    EncIntraCost = 17,
    EncInterCode = 18,
    EncIntraCode = 19,
    EncFinal = 20,
    EncTq = 21,
    EncWrite = 22,
    EncFree = 23,
    EncPrep = 24,
    EncNal = 25,
    EncMbLoop = 26,
    EncMvPred = 27,
    EncEmit = 28,
    EncBs = 29,
    EncAq = 30,
    MeHpel = 31,
    MeCost = 32,
    MeHpelBuild = 33,
    MeDiamond = 34,
    MeSubpel = 35,
    MeRescue = 36,
    Total = 37,
    DecMbP = 38,
    DecMbB = 39,
    DecMbI = 40,
    DecBDirect = 41,
    DecBMc = 42,
    DecSetup = 43,
    DecBDeriv = 44,
    DecBSet = 45,
    DecBWeights = 46,
    DecBLuma = 47,
    DecBChroma = 48,
    DecBBlend = 49,
    EntCbf = 50,
    EntSig = 51,
    EntLvl = 52,
    CavTok = 53,
    CavLvl = 54,
    CavRun = 55,
    DecStateCache = 56,
    DecResidAdd = 57,
    DecMcStage = 58,
    DebPack = 59,
    DebDerive = 60,
    DecNalSplit = 61,
    DecRbsp = 62,
    DecSliceAlloc = 63,
    DecMbLoop = 64,
    DecRowHook = 65,
    BSkipHot = 66,
    BSkipCold = 67,
    BSpanRecon = 68,
    BTypeParse = 69,
    BFillCache = 70,
    BMvdParse = 71,
    BResid = 72,
}
pub const N: usize = 73;
mod imp {
    use super::{N, Stage};
    pub struct Guard;
    #[inline(always)]
    pub fn scope(_s: Stage) -> Guard {
        Guard
    }
    #[inline(always)]
    pub fn reset() {}
    #[inline(always)]
    pub fn dump() {}
    #[inline(always)]
    pub fn snapshot() -> [(f64, u64); N] {
        [(0.0, 0); N]
    }
    #[inline(always)]
    pub fn name(_i: usize) -> &'static str {
        ""
    }
}
pub use imp::{Guard, dump, name, reset, scope, snapshot};
