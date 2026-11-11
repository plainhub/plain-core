mod immediate;
#[cfg(not(target_arch = "wasm32"))]
mod multithreaded;
use crate::jpeg::decoder::{ColorTransform, choose_color_convert_func};
use crate::jpeg::error::Result;
use crate::jpeg::parser::{Component, Dimensions};
use crate::jpeg::upsampler::Upsampler;
use core::cell::RefCell;
use std::sync::Arc;
use std::vec::Vec;
pub struct RowData {
    pub index: usize,
    pub component: Component,
    pub quantization_table: Arc<[u16; 64]>,
}
pub trait Worker {
    fn start(&mut self, row_data: RowData) -> Result<()>;
    fn append_row(&mut self, row: (usize, Vec<i16>)) -> Result<()>;
    fn get_result(&mut self, index: usize) -> Result<Vec<u8>>;
    fn append_rows(&mut self, row: &mut dyn Iterator<Item = (usize, Vec<i16>)>) -> Result<()> {
        for item in row {
            self.append_row(item)?;
        }
        Ok(())
    }
}
pub enum PreferWorkerKind {
    Immediate,
    Multithreaded,
}
#[derive(Default)]
pub struct WorkerScope {
    inner: core::cell::RefCell<Option<WorkerScopeInner>>,
}
enum WorkerScopeInner {
    #[cfg(not(any(target_arch = "wasm32")))]
    Multithreaded(multithreaded::MpscWorker),
    Immediate(immediate::ImmediateWorker),
}
impl WorkerScope {
    pub fn with<T>(with: impl FnOnce(&Self) -> T) -> T {
        with(&WorkerScope {
            inner: RefCell::default(),
        })
    }
    pub fn get_or_init_worker<T>(
        &self,
        prefer: PreferWorkerKind,
        f: impl FnOnce(&mut dyn Worker) -> T,
    ) -> T {
        let mut inner = self.inner.borrow_mut();
        let inner = inner.get_or_insert_with(move || match prefer {
            #[cfg(not(any(target_arch = "wasm32")))]
            PreferWorkerKind::Multithreaded => WorkerScopeInner::Multithreaded(Default::default()),
            _ => WorkerScopeInner::Immediate(Default::default()),
        });
        f(match &mut *inner {
            #[cfg(not(any(target_arch = "wasm32")))]
            WorkerScopeInner::Multithreaded(worker) => worker,
            WorkerScopeInner::Immediate(worker) => worker,
        })
    }
}
pub fn compute_image_parallel(
    components: &[Component],
    data: Vec<Vec<u8>>,
    output_size: Dimensions,
    color_transform: ColorTransform,
) -> Result<Vec<u8>> {
    {
        let color_convert_func = choose_color_convert_func(components.len(), color_transform)?;
        let upsampler = Upsampler::new(components, output_size.width, output_size.height)?;
        let line_size = output_size.width as usize * components.len();
        let mut image = vec![0u8; line_size * output_size.height as usize];
        for (row, line) in image.chunks_mut(line_size).enumerate() {
            upsampler.upsample_and_interleave_row(
                &data,
                row,
                output_size.width as usize,
                line,
                color_convert_func,
            );
        }
        Ok(image)
    }
}
