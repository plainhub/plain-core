#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub enum RecursiveMode {
    Recursive,
    NonRecursive,
}
impl RecursiveMode {
    pub(crate) fn is_recursive(self) -> bool {
        self == Self::Recursive
    }
}
