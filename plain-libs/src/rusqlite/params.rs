use crate::rusqlite::{BindIndex, Result, Statement, ToSql};
mod sealed {
    pub trait Sealed {}
}
use sealed::Sealed;
pub trait Params: Sealed {
    fn __bind_in(self, stmt: &mut Statement<'_>) -> Result<()>;
}
impl Sealed for [&(dyn ToSql + Send + Sync); 0] {}
impl Params for [&(dyn ToSql + Send + Sync); 0] {
    #[inline]
    fn __bind_in(self, stmt: &mut Statement<'_>) -> Result<()> {
        stmt.ensure_parameter_count(0)
    }
}
impl Sealed for &[&dyn ToSql] {}
impl Params for &[&dyn ToSql] {
    #[inline]
    fn __bind_in(self, stmt: &mut Statement<'_>) -> Result<()> {
        stmt.bind_parameters(self)
    }
}
impl<S: BindIndex, T: ToSql> Sealed for &[(S, T)] {}
impl<S: BindIndex, T: ToSql> Params for &[(S, T)] {
    #[inline]
    fn __bind_in(self, stmt: &mut Statement<'_>) -> Result<()> {
        stmt.bind_parameters_named(self)
    }
}
impl Sealed for () {}
impl Params for () {
    #[inline]
    fn __bind_in(self, stmt: &mut Statement<'_>) -> Result<()> {
        stmt.ensure_parameter_count(0)
    }
}
impl<T: ToSql> Sealed for (T,) {}
impl<T: ToSql> Params for (T,) {
    #[inline]
    fn __bind_in(self, stmt: &mut Statement<'_>) -> Result<()> {
        stmt.ensure_parameter_count(1)?;
        stmt.raw_bind_parameter(1, self.0)?;
        Ok(())
    }
}
macro_rules! single_tuple_impl { ($count : literal : $(($field : tt $ftype : ident)) ,* $(,) ?) => { impl <$($ftype ,) *> Sealed for ($($ftype ,) *) where $($ftype : ToSql ,) * { } impl <$($ftype ,) *> Params for ($($ftype ,) *) where $($ftype : ToSql ,) * { fn __bind_in (self , stmt : & mut Statement <'_ >) -> Result < () > { stmt . ensure_parameter_count ($count) ?; $({ debug_assert ! ($field < $count) ; stmt . raw_bind_parameter ($field + 1 , self .$field) ?; }) + Ok (()) } } } }
single_tuple_impl ! (2 : (0 A) , (1 B));
single_tuple_impl ! (3 : (0 A) , (1 B) , (2 C));
single_tuple_impl ! (4 : (0 A) , (1 B) , (2 C) , (3 D));
single_tuple_impl ! (5 : (0 A) , (1 B) , (2 C) , (3 D) , (4 E));
single_tuple_impl ! (6 : (0 A) , (1 B) , (2 C) , (3 D) , (4 E) , (5 F));
single_tuple_impl ! (7 : (0 A) , (1 B) , (2 C) , (3 D) , (4 E) , (5 F) , (6 G));
single_tuple_impl ! (8 : (0 A) , (1 B) , (2 C) , (3 D) , (4 E) , (5 F) , (6 G) , (7 H));
single_tuple_impl ! (9 : (0 A) , (1 B) , (2 C) , (3 D) , (4 E) , (5 F) , (6 G) , (7 H) , (8 I));
single_tuple_impl ! (10 : (0 A) , (1 B) , (2 C) , (3 D) , (4 E) , (5 F) , (6 G) , (7 H) , (8 I) , (9 J));
single_tuple_impl ! (11 : (0 A) , (1 B) , (2 C) , (3 D) , (4 E) , (5 F) , (6 G) , (7 H) , (8 I) , (9 J) , (10 K));
single_tuple_impl ! (12 : (0 A) , (1 B) , (2 C) , (3 D) , (4 E) , (5 F) , (6 G) , (7 H) , (8 I) , (9 J) , (10 K) , (11 L));
single_tuple_impl ! (13 : (0 A) , (1 B) , (2 C) , (3 D) , (4 E) , (5 F) , (6 G) , (7 H) , (8 I) , (9 J) , (10 K) , (11 L) , (12 M));
single_tuple_impl ! (14 : (0 A) , (1 B) , (2 C) , (3 D) , (4 E) , (5 F) , (6 G) , (7 H) , (8 I) , (9 J) , (10 K) , (11 L) , (12 M) , (13 N));
single_tuple_impl ! (15 : (0 A) , (1 B) , (2 C) , (3 D) , (4 E) , (5 F) , (6 G) , (7 H) , (8 I) , (9 J) , (10 K) , (11 L) , (12 M) , (13 N) , (14 O));
single_tuple_impl ! (16 : (0 A) , (1 B) , (2 C) , (3 D) , (4 E) , (5 F) , (6 G) , (7 H) , (8 I) , (9 J) , (10 K) , (11 L) , (12 M) , (13 N) , (14 O) , (15 P));
macro_rules! impl_for_array_ref { ($($N : literal) +) => { $(impl < T : ToSql + ? Sized > Sealed for & [& T ; $N] { } impl < T : ToSql + ? Sized > Params for & [& T ; $N] { fn __bind_in (self , stmt : & mut Statement <'_ >) -> Result < () > { stmt . bind_parameters (self) } } impl < S : BindIndex , T : ToSql + ? Sized > Sealed for & [(S , & T) ; $N] { } impl < S : BindIndex , T : ToSql + ? Sized > Params for & [(S , & T) ; $N] { fn __bind_in (self , stmt : & mut Statement <'_ >) -> Result < () > { stmt . bind_parameters_named (self) } } impl < T : ToSql > Sealed for [T ; $N] { } impl < T : ToSql > Params for [T ; $N] { #[inline] fn __bind_in (self , stmt : & mut Statement <'_ >) -> Result < () > { stmt . bind_parameters (& self) } }) + } ; }
impl_for_array_ref ! (1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32);
#[derive(Clone, Debug)]
pub struct ParamsFromIter<I>(I);
#[inline]
pub fn params_from_iter<I>(iter: I) -> ParamsFromIter<I>
where
    I: IntoIterator,
    I::Item: ToSql,
{
    ParamsFromIter(iter)
}
impl<I> Sealed for ParamsFromIter<I>
where
    I: IntoIterator,
    I::Item: ToSql,
{
}
impl<I> Params for ParamsFromIter<I>
where
    I: IntoIterator,
    I::Item: ToSql,
{
    #[inline]
    fn __bind_in(self, stmt: &mut Statement<'_>) -> Result<()> {
        stmt.bind_parameters(self.0)
    }
}
