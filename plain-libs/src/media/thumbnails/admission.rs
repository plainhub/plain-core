use std::sync::OnceLock;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// One memory-permit unit = 64 KiB of estimated decoded bitmap.
pub const MEM_UNIT: u64 = 64 * 1024;

/// Hard ceiling on decoded source pixels, any format (decompression-bomb
/// guard). Scaled JPEG decodes are priced by their *scaled* output, so
/// huge-but-shrinking JPEGs stay allowed; this guards full decodes.
pub const MAX_SOURCE_PIXELS: u64 = 300_000_000;

pub struct Admission {
    cpu: std::sync::Arc<Semaphore>,
    workers: usize,
    mem: std::sync::Arc<Semaphore>,
    /// Total memory budget in permit units (the semaphore alone cannot tell
    /// "fully used" from "configured total").
    mem_total: u64,
}

impl Admission {
    pub fn new(cpu_permits: usize, mem_budget_mb: u32) -> Self {
        let total_units = units_for(u64::from(mem_budget_mb.max(1)) * 1024 * 1024);
        Admission {
            cpu: std::sync::Arc::new(Semaphore::new(cpu_permits.max(1))),
            workers: cpu_permits.max(1),
            mem: std::sync::Arc::new(Semaphore::new(total_units.min(u32::MAX as u64) as usize)),
            mem_total: total_units,
        }
    }

    pub fn workers(&self) -> usize {
        self.workers
    }

    pub fn mem_budget_mb(&self) -> u32 {
        (self.mem_total * MEM_UNIT / (1024 * 1024)) as u32
    }

    /// Acquire CPU + memory permits for a job whose decoded bitmap is
    /// estimated at `est_bytes`. Rejects a job that could never fit the
    /// budget on its own; otherwise queues fairly behind other holders.
    pub async fn acquire(&self, est_bytes: u64) -> anyhow::Result<Permits> {
        let units = units_for(est_bytes) as usize;
        if units as u64 > self.mem_total && units > 0 {
            anyhow::bail!(
                "thumbnail decode estimate {est_bytes} exceeds memory budget {}",
                self.mem_budget_mb()
            );
        }
        // CPU first, then memory: memory holders never wait on CPU, so this
        // ordering cannot deadlock.
        let cpu = self
            .cpu
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| anyhow::anyhow!("cpu semaphore closed"))?;
        let mem = if units > 0 {
            Some(
                self.mem
                    .clone()
                    .acquire_many_owned(units as u32)
                    .await
                    .map_err(|_| anyhow::anyhow!("mem semaphore closed"))?,
            )
        } else {
            None
        };
        Ok(Permits {
            _cpu: cpu,
            _mem: mem,
        })
    }
}

fn units_for(est_bytes: u64) -> u64 {
    est_bytes.div_ceil(MEM_UNIT)
}

/// Per-job RAII permits.
pub struct Permits {
    _cpu: OwnedSemaphorePermit,
    _mem: Option<OwnedSemaphorePermit>,
}

static GLOBAL: OnceLock<Admission> = OnceLock::new();

fn build(raw: i64) -> Admission {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let cpu_permits = crate::media::thumbnail_limits::workers(cpus);
    // Missing/invalid key (get_int → 0) falls back to the 2 GB default.
    let mb = if raw <= 0 { 2048 } else { raw.clamp(1, 16_384) } as u32;
    Admission::new(cpu_permits, mb)
}

pub fn initialize(mem_budget_mb: i64) {
    let _ = GLOBAL.set(build(mem_budget_mb));
}

pub fn initialize_with_memory(sample: impl FnOnce() -> u64) {
    GLOBAL.get_or_init(|| {
        build(i64::from(crate::media::thumbnail_limits::memory_budget_mb(
            sample(),
        )))
    });
}

pub fn global() -> &'static Admission {
    GLOBAL.get_or_init(|| build(0))
}

#[cfg(test)]
#[path = "../../../../tests/plain-libs/unit/media/thumbnails/admission.rs"]
mod tests;
