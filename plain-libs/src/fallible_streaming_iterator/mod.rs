pub trait FallibleStreamingIterator {
    type Item: ?Sized;
    type Error;
    fn advance(&mut self) -> Result<(), Self::Error>;
    fn get(&self) -> Option<&Self::Item>;
}
