use {
    crate::foldhash::fast::RandomState,
    core::hash::{BuildHasher, Hasher},
};
#[derive(Clone, Debug, Default)]
pub struct DefaultHashBuilder {
    inner: RandomState,
}
impl BuildHasher for DefaultHashBuilder {
    type Hasher = DefaultHasher;
    #[inline(always)]
    fn build_hasher(&self) -> Self::Hasher {
        DefaultHasher {
            inner: self.inner.build_hasher(),
        }
    }
}
#[derive(Clone)]
pub struct DefaultHasher {
    inner: <RandomState as BuildHasher>::Hasher,
}
macro_rules! forward_writes { ($($write : ident ($ty : ty) ,) *) => { $(#[inline (always)] fn $write (& mut self , arg : $ty) { self . inner .$write (arg) ; }) * } }
impl Hasher for DefaultHasher {
    forward_writes! { write (& [u8]) , write_u8 (u8) , write_u16 (u16) , write_u32 (u32) , write_u64 (u64) , write_u128 (u128) , write_usize (usize) , write_i8 (i8) , write_i16 (i16) , write_i32 (i32) , write_i64 (i64) , write_i128 (i128) , write_isize (isize) , }
    #[inline(always)]
    fn finish(&self) -> u64 {
        self.inner.finish()
    }
}
