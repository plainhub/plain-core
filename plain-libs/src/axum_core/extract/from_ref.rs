pub trait FromRef<T> {
    fn from_ref(input: &T) -> Self;
}
impl<T> FromRef<T> for T
where
    T: Clone,
{
    fn from_ref(input: &T) -> Self {
        input.clone()
    }
}
