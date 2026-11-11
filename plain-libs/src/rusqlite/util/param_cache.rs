use super::SmallCString;
use std::cell::RefCell;
use std::collections::BTreeMap;
#[derive(Default, Clone, Debug)]
pub(crate) struct ParamIndexCache(RefCell<BTreeMap<SmallCString, usize>>);
impl ParamIndexCache {
    pub fn get_or_insert_with<F>(&self, s: &str, func: F) -> Option<usize>
    where
        F: FnOnce(&std::ffi::CStr) -> Option<usize>,
    {
        let mut cache = self.0.borrow_mut();
        if let Some(v) = cache.get(s) {
            return Some(*v);
        }
        let name = SmallCString::new(s).ok()?;
        let val = func(&name)?;
        cache.insert(name, val);
        Some(val)
    }
}
