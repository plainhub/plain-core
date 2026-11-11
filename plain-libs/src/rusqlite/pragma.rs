use crate::rusqlite::ffi;
use crate::rusqlite::types::{FromSql, ToSql, ToSqlOutput, ValueRef};
use crate::rusqlite::{Connection, Result, Row};
use std::ops::Deref;
pub struct Sql {
    buf: String,
}
impl Sql {
    pub fn new() -> Self {
        Self { buf: String::new() }
    }
    pub fn push_pragma(
        &mut self,
        _conn: &Connection,
        schema_name: Option<&str>,
        pragma_name: &str,
    ) -> Result<()> {
        self.push_keyword("PRAGMA")?;
        self.push_space();
        if let Some(schema_name) = schema_name {
            self.push_schema_name(schema_name);
            self.push_dot();
        }
        cfg_select! { any (any () , any () , not (any ())) => { if ! pragma_name . is_empty () && is_identifier (pragma_name) { self . buf . push_str (pragma_name) ; Ok (()) } else { Err (err ! (ffi::SQLITE_MISUSE , "Invalid pragma \"{pragma_name}\"")) } } _ => { const SQL : & str = "SELECT 1 FROM pragma_pragma_list WHERE name = ?" ; let mut stmt = cfg_select ! { all () => { _conn . prepare_cached (SQL) } _ => { _conn . prepare (SQL) } } ?; if stmt . exists ([pragma_name]) ? { self . buf . push_str (pragma_name) ; Ok (()) } else { Err (err ! (ffi::SQLITE_MISUSE , "Invalid pragma \"{pragma_name}\"")) } } }
    }
    pub fn push_keyword(&mut self, keyword: &str) -> Result<()> {
        if !keyword.is_empty() && is_keyword(keyword) {
            self.buf.push_str(keyword);
            Ok(())
        } else {
            Err(err!(ffi::SQLITE_MISUSE, "Invalid keyword \"{keyword}\""))
        }
    }
    pub fn push_schema_name(&mut self, schema_name: &str) {
        self.push_identifier(schema_name);
    }
    pub fn push_identifier(&mut self, s: &str) {
        if is_identifier(s) {
            self.buf.push_str(s);
        } else {
            self.wrap_and_escape(s, '"');
        }
    }
    pub fn push_value(&mut self, value: &dyn ToSql) -> Result<()> {
        let value = value.to_sql()?;
        let value = match value {
            ToSqlOutput::Borrowed(v) => v,
            ToSqlOutput::Owned(ref v) => ValueRef::from(v),
        };
        match value {
            ValueRef::Integer(i) => {
                self.push_int(i);
            }
            ValueRef::Real(r) => {
                self.push_real(r);
            }
            ValueRef::Text(s) => {
                let s = std::str::from_utf8(s)?;
                self.push_string_literal(s);
            }
            _ => {
                return Err(err!(ffi::SQLITE_MISUSE, "Unsupported value \"{value:?}\""));
            }
        }
        Ok(())
    }
    pub fn push_string_literal(&mut self, s: &str) {
        self.wrap_and_escape(s, '\'');
    }
    pub fn push_int(&mut self, i: i64) {
        self.buf.push_str(&i.to_string());
    }
    pub fn push_real(&mut self, f: f64) {
        self.buf.push_str(&f.to_string());
    }
    pub fn push_space(&mut self) {
        self.buf.push(' ');
    }
    pub fn push_dot(&mut self) {
        self.buf.push('.');
    }
    pub fn push_equal_sign(&mut self) {
        self.buf.push('=');
    }
    pub fn open_brace(&mut self) {
        self.buf.push('(');
    }
    pub fn close_brace(&mut self) {
        self.buf.push(')');
    }
    pub fn as_str(&self) -> &str {
        &self.buf
    }
    fn wrap_and_escape(&mut self, s: &str, quote: char) {
        self.buf.push(quote);
        let chars = s.chars();
        for ch in chars {
            if ch == quote {
                self.buf.push(ch);
            }
            self.buf.push(ch);
        }
        self.buf.push(quote);
    }
}
impl Deref for Sql {
    type Target = str;
    fn deref(&self) -> &str {
        self.as_str()
    }
}
impl Connection {
    pub fn pragma_query_value<T, F>(
        &self,
        schema_name: Option<&str>,
        pragma_name: &str,
        f: F,
    ) -> Result<T>
    where
        F: FnOnce(&Row<'_>) -> Result<T>,
    {
        let mut query = Sql::new();
        query.push_pragma(self, schema_name, pragma_name)?;
        self.query_row(&query, [], f)
    }

    pub fn pragma<F, V>(
        &self,
        schema_name: Option<&str>,
        pragma_name: &str,
        pragma_value: V,
        mut f: F,
    ) -> Result<()>
    where
        F: FnMut(&Row<'_>) -> Result<()>,
        V: ToSql,
    {
        let mut sql = Sql::new();
        sql.push_pragma(self, schema_name, pragma_name)?;
        sql.open_brace();
        sql.push_value(&pragma_value)?;
        sql.close_brace();
        let mut stmt = self.prepare(&sql)?;
        let mut rows = stmt.query([])?;
        while let Some(result_row) = rows.next()? {
            let row = result_row;
            f(row)?;
        }
        Ok(())
    }
    pub fn pragma_update<V>(
        &self,
        schema_name: Option<&str>,
        pragma_name: &str,
        pragma_value: V,
    ) -> Result<()>
    where
        V: ToSql,
    {
        let mut sql = Sql::new();
        sql.push_pragma(self, schema_name, pragma_name)?;
        sql.push_equal_sign();
        sql.push_value(&pragma_value)?;
        self.execute_batch(&sql)
    }
}
fn is_identifier(s: &str) -> bool {
    let chars = s.char_indices();
    for (i, ch) in chars {
        if i == 0 {
            if !is_identifier_start(ch) {
                return false;
            }
        } else if !is_identifier_continue(ch) {
            return false;
        }
    }
    true
}
fn is_identifier_start(c: char) -> bool {
    c.is_ascii_uppercase() || c == '_' || c.is_ascii_lowercase() || c > '\x7F'
}
fn is_identifier_continue(c: char) -> bool {
    c == '$'
        || c.is_ascii_digit()
        || c.is_ascii_uppercase()
        || c == '_'
        || c.is_ascii_lowercase()
        || c > '\x7F'
}
fn is_keyword(s: &str) -> bool {
    unsafe {
        ffi::sqlite3_keyword_check(
            s.as_ptr().cast::<std::ffi::c_char>(),
            s.len().try_into().unwrap(),
        ) != 0
    }
}
