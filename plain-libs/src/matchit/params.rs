use std::iter;
use std::mem;
use std::slice;
#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Default, Copy, Clone)]
struct Param<'k, 'v> {
    key: &'k [u8],
    value: &'v [u8],
}
impl<'k, 'v> Param<'k, 'v> {
    fn key_str(&self) -> &'k str {
        std::str::from_utf8(self.key).unwrap()
    }
    fn value_str(&self) -> &'v str {
        std::str::from_utf8(self.value).unwrap()
    }
}
#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Clone)]
pub struct Params<'k, 'v> {
    kind: ParamsKind<'k, 'v>,
}
const SMALL: usize = 3;
#[derive(Debug, PartialEq, Eq, Ord, PartialOrd, Clone)]
enum ParamsKind<'k, 'v> {
    None,
    Small([Param<'k, 'v>; SMALL], usize),
    Large(Vec<Param<'k, 'v>>),
}
impl<'k, 'v> Params<'k, 'v> {
    pub(crate) fn new() -> Self {
        let kind = ParamsKind::None;
        Self { kind }
    }
    pub fn len(&self) -> usize {
        match &self.kind {
            ParamsKind::None => 0,
            ParamsKind::Small(_, len) => *len,
            ParamsKind::Large(vec) => vec.len(),
        }
    }
    pub(crate) fn truncate(&mut self, n: usize) {
        match &mut self.kind {
            ParamsKind::None => {}
            ParamsKind::Small(_, len) => {
                *len = n;
            }
            ParamsKind::Large(vec) => {
                vec.truncate(n);
            }
        }
    }
    pub fn get(&self, key: impl AsRef<str>) -> Option<&'v str> {
        let key = key.as_ref().as_bytes();
        match &self.kind {
            ParamsKind::None => None,
            ParamsKind::Small(arr, len) => arr
                .iter()
                .take(*len)
                .find(|param| param.key == key)
                .map(Param::value_str),
            ParamsKind::Large(vec) => vec
                .iter()
                .find(|param| param.key == key)
                .map(Param::value_str),
        }
    }
    pub fn iter(&self) -> ParamsIter<'_, 'k, 'v> {
        ParamsIter::new(self)
    }
    pub fn is_empty(&self) -> bool {
        match &self.kind {
            ParamsKind::None => true,
            ParamsKind::Small(_, len) => *len == 0,
            ParamsKind::Large(vec) => vec.is_empty(),
        }
    }
    pub(crate) fn push(&mut self, key: &'k [u8], value: &'v [u8]) {
        #[cold]
        fn drain_to_vec<T: Default>(len: usize, elem: T, arr: &mut [T; SMALL]) -> Vec<T> {
            let mut vec = Vec::with_capacity(len + 1);
            vec.extend(arr.iter_mut().map(mem::take));
            vec.push(elem);
            vec
        }
        let param = Param { key, value };
        match &mut self.kind {
            ParamsKind::None => {
                self.kind = ParamsKind::Small([param, Param::default(), Param::default()], 1);
            }
            ParamsKind::Small(arr, len) => {
                if *len == SMALL {
                    self.kind = ParamsKind::Large(drain_to_vec(*len, param, arr));
                    return;
                }
                arr[*len] = param;
                *len += 1;
            }
            ParamsKind::Large(vec) => vec.push(param),
        }
    }
    pub(crate) fn for_each_key_mut(&mut self, f: impl Fn((usize, &mut &'k [u8]))) {
        match &mut self.kind {
            ParamsKind::None => {}
            ParamsKind::Small(arr, len) => arr
                .iter_mut()
                .take(*len)
                .map(|param| &mut param.key)
                .enumerate()
                .for_each(f),
            ParamsKind::Large(vec) => vec
                .iter_mut()
                .map(|param| &mut param.key)
                .enumerate()
                .for_each(f),
        }
    }
}
pub struct ParamsIter<'ps, 'k, 'v> {
    kind: ParamsIterKind<'ps, 'k, 'v>,
}
impl<'ps, 'k, 'v> ParamsIter<'ps, 'k, 'v> {
    fn new(params: &'ps Params<'k, 'v>) -> Self {
        let kind = match &params.kind {
            ParamsKind::None => ParamsIterKind::None,
            ParamsKind::Small(arr, len) => ParamsIterKind::Small(arr.iter().take(*len)),
            ParamsKind::Large(vec) => ParamsIterKind::Large(vec.iter()),
        };
        Self { kind }
    }
}
enum ParamsIterKind<'ps, 'k, 'v> {
    None,
    Small(iter::Take<slice::Iter<'ps, Param<'k, 'v>>>),
    Large(slice::Iter<'ps, Param<'k, 'v>>),
}
impl<'ps, 'k, 'v> Iterator for ParamsIter<'ps, 'k, 'v> {
    type Item = (&'k str, &'v str);
    fn next(&mut self) -> Option<Self::Item> {
        match self.kind {
            ParamsIterKind::None => None,
            ParamsIterKind::Small(ref mut iter) => {
                iter.next().map(|p| (p.key_str(), p.value_str()))
            }
            ParamsIterKind::Large(ref mut iter) => {
                iter.next().map(|p| (p.key_str(), p.value_str()))
            }
        }
    }
}
