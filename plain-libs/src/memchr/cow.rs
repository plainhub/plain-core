use core::ops;
#[derive(Clone, Debug)]
pub struct CowBytes<'a>(Imp<'a>);
#[derive(Clone, Debug)]
enum Imp<'a> {
    Borrowed(&'a [u8]),
    Owned(alloc::boxed::Box<[u8]>),
}
impl<'a> ops::Deref for CowBytes<'a> {
    type Target = [u8];
    #[inline(always)]
    fn deref(&self) -> &[u8] {
        self.as_slice()
    }
}
impl<'a> CowBytes<'a> {
    #[inline(always)]
    pub(crate) fn new<B: ?Sized + AsRef<[u8]>>(bytes: &'a B) -> CowBytes<'a> {
        CowBytes(Imp::new(bytes.as_ref()))
    }
    #[inline(always)]
    pub(crate) fn new_owned(bytes: alloc::boxed::Box<[u8]>) -> CowBytes<'static> {
        CowBytes(Imp::Owned(bytes))
    }
    #[inline(always)]
    pub(crate) fn as_slice(&self) -> &[u8] {
        self.0.as_slice()
    }
    #[inline(always)]
    pub(crate) fn into_owned(self) -> CowBytes<'static> {
        match self.0 {
            Imp::Borrowed(b) => CowBytes::new_owned(alloc::boxed::Box::from(b)),
            Imp::Owned(b) => CowBytes::new_owned(b),
        }
    }
}
impl<'a> Imp<'a> {
    #[inline(always)]
    pub fn new(bytes: &'a [u8]) -> Imp<'a> {
        { Imp::Borrowed(bytes) }
    }
    #[inline(always)]
    pub fn as_slice(&self) -> &[u8] {
        {
            match self {
                Imp::Owned(x) => x,
                Imp::Borrowed(x) => x,
            }
        }
    }
}
