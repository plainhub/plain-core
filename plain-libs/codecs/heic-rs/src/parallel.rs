use alloc::vec::Vec;
use crate::error::Result;
pub(crate) type Threads=Option<usize>;
pub(crate) fn try_map<T,U,F>(items:&[T],_threads:Threads,f:F)->Result<Vec<U>> where T:Sync,U:Send,F:Fn(&T)->Result<U>+Send+Sync {items.iter().map(f).collect()}
pub(crate) fn for_each_band<F>(out:&mut[u8],_row_bytes:usize,_rows_per_band:usize,_threads:Threads,f:F) where F:Fn(usize,&mut[u8])+Send+Sync {f(0,out)}
