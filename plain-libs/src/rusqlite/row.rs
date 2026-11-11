use super::{Error, Result, Statement};
use crate::fallible_iterator::FallibleIterator;
use crate::fallible_streaming_iterator::FallibleStreamingIterator;
use crate::rusqlite::types::{FromSql, FromSqlError, ValueRef};

#[must_use = "Rows is lazy and will do nothing unless consumed"]
pub struct Rows<'stmt> {
    pub(crate) stmt: Option<&'stmt Statement<'stmt>>,
    row: Option<Row<'stmt>>,
}
impl<'stmt> Rows<'stmt> {
    #[inline]
    fn reset(&mut self) -> Result<()> {
        if let Some(stmt) = self.stmt.take() {
            stmt.reset()
        } else {
            Ok(())
        }
    }
    #[inline]
    pub fn next(&mut self) -> Result<Option<&Row<'stmt>>> {
        self.advance()?;
        Ok((*self).get())
    }
    #[inline]
    pub fn map<F, B>(self, f: F) -> Map<'stmt, F>
    where
        F: FnMut(&Row<'_>) -> Result<B>,
    {
        Map { rows: self, f }
    }
    #[inline]
    pub fn mapped<F, B>(self, f: F) -> MappedRows<'stmt, F>
    where
        F: FnMut(&Row<'_>) -> Result<B>,
    {
        MappedRows { rows: self, map: f }
    }
    #[inline]
    pub fn and_then<F, T, E>(self, f: F) -> AndThenRows<'stmt, F>
    where
        F: FnMut(&Row<'_>) -> Result<T, E>,
    {
        AndThenRows { rows: self, map: f }
    }
    #[must_use]
    pub fn as_ref(&self) -> Option<&Statement<'stmt>> {
        self.stmt
    }
}
impl<'stmt> Rows<'stmt> {
    #[inline]
    pub(crate) fn new(stmt: &'stmt Statement<'stmt>) -> Self {
        Rows {
            stmt: Some(stmt),
            row: None,
        }
    }
    #[inline]
    pub(crate) fn get_expected_row(&mut self) -> Result<&Row<'stmt>> {
        match self.next()? {
            Some(row) => Ok(row),
            None => Err(Error::QueryReturnedNoRows),
        }
    }
}
impl Drop for Rows<'_> {
    #[inline]
    fn drop(&mut self) {
        self.reset();
    }
}
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Map<'stmt, F> {
    rows: Rows<'stmt>,
    f: F,
}
impl<F, B> FallibleIterator for Map<'_, F>
where
    F: FnMut(&Row<'_>) -> Result<B>,
{
    type Error = Error;
    type Item = B;
    #[inline]
    fn next(&mut self) -> Result<Option<B>> {
        match self.rows.next()? {
            Some(v) => Ok(Some((self.f)(v)?)),
            None => Ok(None),
        }
    }
}
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct MappedRows<'stmt, F> {
    rows: Rows<'stmt>,
    map: F,
}
impl<T, F> Iterator for MappedRows<'_, F>
where
    F: FnMut(&Row<'_>) -> Result<T>,
{
    type Item = Result<T>;
    #[inline]
    fn next(&mut self) -> Option<Result<T>> {
        let map = &mut self.map;
        self.rows
            .next()
            .transpose()
            .map(|row_result| row_result.and_then(map))
    }
}
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct AndThenRows<'stmt, F> {
    rows: Rows<'stmt>,
    map: F,
}
impl<T, E, F> Iterator for AndThenRows<'_, F>
where
    E: From<Error>,
    F: FnMut(&Row<'_>) -> Result<T, E>,
{
    type Item = Result<T, E>;
    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let map = &mut self.map;
        self.rows
            .next()
            .transpose()
            .map(|row_result| row_result.map_err(E::from).and_then(map))
    }
}
impl<'stmt> FallibleStreamingIterator for Rows<'stmt> {
    type Error = Error;
    type Item = Row<'stmt>;
    #[inline]
    fn advance(&mut self) -> Result<()> {
        if let Some(stmt) = self.stmt {
            match stmt.step() {
                Ok(true) => {
                    self.row = Some(Row { stmt });
                    Ok(())
                }
                Ok(false) => {
                    let r = self.reset();
                    self.row = None;
                    r
                }
                Err(e) => {
                    let _ = self.reset();
                    self.row = None;
                    Err(e)
                }
            }
        } else {
            self.row = None;
            Ok(())
        }
    }
    #[inline]
    fn get(&self) -> Option<&Row<'stmt>> {
        self.row.as_ref()
    }
}
pub struct Row<'stmt> {
    pub(crate) stmt: &'stmt Statement<'stmt>,
}
impl Row<'_> {
    #[track_caller]
    pub fn get<I: RowIndex, T: FromSql>(&self, idx: I) -> Result<T> {
        let idx = idx.idx(self.stmt)?;
        let value = self.stmt.value_ref(idx);
        FromSql::column_result(value).map_err(|err| match err {
            FromSqlError::InvalidType => Error::InvalidColumnType(
                idx,
                self.stmt.column_name_unwrap(idx).into(),
                value.data_type(),
            ),
            FromSqlError::OutOfRange(i) => Error::IntegralValueOutOfRange(idx, i),
            FromSqlError::Utf8Error(err) => Error::Utf8Error(idx, err),
            FromSqlError::Other(err) => {
                Error::FromSqlConversionFailure(idx, value.data_type(), err)
            }
            FromSqlError::InvalidBlobSize { .. } => {
                Error::FromSqlConversionFailure(idx, value.data_type(), Box::new(err))
            }
        })
    }
    pub fn get_ref<I: RowIndex>(&self, idx: I) -> Result<ValueRef<'_>> {
        let idx = idx.idx(self.stmt)?;
        let val_ref = self.stmt.value_ref(idx);
        Ok(val_ref)
    }
}
impl<'stmt> AsRef<Statement<'stmt>> for Row<'stmt> {
    fn as_ref(&self) -> &Statement<'stmt> {
        self.stmt
    }
}
impl std::fmt::Debug for Row<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut dm = f.debug_map();
        for c in 0..self.stmt.column_count() {
            let name = self.stmt.column_name(c).expect("valid column index");
            dm.key(&name);
            let value = self.get_ref(c);
            match value {
                Ok(value) => {
                    let dt = value.data_type();
                    match value {
                        ValueRef::Null => {
                            dm.value(&(dt, ()));
                        }
                        ValueRef::Integer(i) => {
                            dm.value(&(dt, i));
                        }
                        ValueRef::Real(f) => {
                            dm.value(&(dt, f));
                        }
                        ValueRef::Text(s) => {
                            dm.value(&(dt, String::from_utf8_lossy(s)));
                        }
                        ValueRef::Blob(b) => {
                            dm.value(&(dt, b.len()));
                        }
                    }
                }
                Err(ref _err) => {
                    dm.value(&value);
                }
            }
        }
        dm.finish()
    }
}
mod sealed {
    pub trait Sealed {}
    impl Sealed for usize {}
    impl Sealed for &str {}
}
pub trait RowIndex: sealed::Sealed + Copy {
    fn idx(&self, stmt: &Statement<'_>) -> Result<usize>;
}
impl RowIndex for usize {
    #[inline]
    fn idx(&self, stmt: &Statement<'_>) -> Result<usize> {
        if *self >= stmt.column_count() {
            Err(Error::InvalidColumnIndex(*self))
        } else {
            Ok(*self)
        }
    }
}
impl RowIndex for &'_ str {
    #[inline]
    fn idx(&self, stmt: &Statement<'_>) -> Result<usize> {
        stmt.column_index(self)
    }
}
