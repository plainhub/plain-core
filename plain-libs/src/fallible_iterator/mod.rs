pub trait FallibleIterator {
    type Item;
    type Error;
    fn next(&mut self) -> Result<Option<Self::Item>, Self::Error>;
}
