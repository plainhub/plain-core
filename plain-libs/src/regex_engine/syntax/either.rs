#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Either<Left, Right> {
    Left(Left),
    Right(Right),
}
