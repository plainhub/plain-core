use core::borrow::Borrow;

pub trait Equivalent<K: ?Sized> {
    fn equivalent(&self, key: &K) -> bool;
}
impl<Q: ?Sized, K: ?Sized> Equivalent<K> for Q
where
    Q: Eq,
    K: Borrow<Q>,
{
    #[inline]
    fn equivalent(&self, key: &K) -> bool {
        PartialEq::eq(self, key.borrow())
    }
}
