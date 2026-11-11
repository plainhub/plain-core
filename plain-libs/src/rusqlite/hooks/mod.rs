use crate::rusqlite::ffi;
use crate::rusqlite::wal::CheckpointMode;
use crate::rusqlite::{Connection, InnerConnection, Result};
use std::ffi::{CStr, c_char, c_int, c_void};
use std::panic::catch_unwind;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
#[non_exhaustive]
pub enum Action {
    UNKNOWN = -1,
    SQLITE_DELETE = ffi::SQLITE_DELETE,
    SQLITE_INSERT = ffi::SQLITE_INSERT,
    SQLITE_UPDATE = ffi::SQLITE_UPDATE,
}
impl From<i32> for Action {
    #[inline]
    fn from(code: i32) -> Self {
        match code {
            ffi::SQLITE_DELETE => Self::SQLITE_DELETE,
            ffi::SQLITE_INSERT => Self::SQLITE_INSERT,
            ffi::SQLITE_UPDATE => Self::SQLITE_UPDATE,
            _ => Self::UNKNOWN,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthContext<'c> {
    pub action: AuthAction<'c>,
    pub database_name: Option<&'c str>,
    pub accessor: Option<&'c str>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum AuthAction<'c> {
    Unknown {
        code: i32,
        arg1: Option<&'c str>,
        arg2: Option<&'c str>,
    },
    CreateIndex {
        index_name: &'c str,
        table_name: &'c str,
    },
    CreateTable {
        table_name: &'c str,
    },
    CreateTempIndex {
        index_name: &'c str,
        table_name: &'c str,
    },
    CreateTempTable {
        table_name: &'c str,
    },
    CreateTempTrigger {
        trigger_name: &'c str,
        table_name: &'c str,
    },
    CreateTempView {
        view_name: &'c str,
    },
    CreateTrigger {
        trigger_name: &'c str,
        table_name: &'c str,
    },
    CreateView {
        view_name: &'c str,
    },
    Delete {
        table_name: &'c str,
    },
    DropIndex {
        index_name: &'c str,
        table_name: &'c str,
    },
    DropTable {
        table_name: &'c str,
    },
    DropTempIndex {
        index_name: &'c str,
        table_name: &'c str,
    },
    DropTempTable {
        table_name: &'c str,
    },
    DropTempTrigger {
        trigger_name: &'c str,
        table_name: &'c str,
    },
    DropTempView {
        view_name: &'c str,
    },
    DropTrigger {
        trigger_name: &'c str,
        table_name: &'c str,
    },
    DropView {
        view_name: &'c str,
    },
    Insert {
        table_name: &'c str,
    },
    Pragma {
        pragma_name: &'c str,
        pragma_value: Option<&'c str>,
    },
    Read {
        table_name: &'c str,
        column_name: &'c str,
    },
    Select,
    Transaction {
        operation: TransactionOperation,
    },
    Update {
        table_name: &'c str,
        column_name: &'c str,
    },
    Attach {
        filename: &'c str,
    },
    Detach {
        database_name: &'c str,
    },
    AlterTable {
        database_name: &'c str,
        table_name: &'c str,
    },
    Reindex {
        index_name: &'c str,
    },
    Analyze {
        table_name: &'c str,
    },
    CreateVtable {
        table_name: &'c str,
        module_name: &'c str,
    },
    DropVtable {
        table_name: &'c str,
        module_name: &'c str,
    },
    Function {
        function_name: &'c str,
    },
    Savepoint {
        operation: TransactionOperation,
        savepoint_name: &'c str,
    },
    Recursive,
}
impl<'c> AuthAction<'c> {
    fn from_raw(code: i32, arg1: Option<&'c str>, arg2: Option<&'c str>) -> Self {
        match (code, arg1, arg2) {
            (ffi::SQLITE_CREATE_INDEX, Some(index_name), Some(table_name)) => Self::CreateIndex {
                index_name,
                table_name,
            },
            (ffi::SQLITE_CREATE_TABLE, Some(table_name), _) => Self::CreateTable { table_name },
            (ffi::SQLITE_CREATE_TEMP_INDEX, Some(index_name), Some(table_name)) => {
                Self::CreateTempIndex {
                    index_name,
                    table_name,
                }
            }
            (ffi::SQLITE_CREATE_TEMP_TABLE, Some(table_name), _) => {
                Self::CreateTempTable { table_name }
            }
            (ffi::SQLITE_CREATE_TEMP_TRIGGER, Some(trigger_name), Some(table_name)) => {
                Self::CreateTempTrigger {
                    trigger_name,
                    table_name,
                }
            }
            (ffi::SQLITE_CREATE_TEMP_VIEW, Some(view_name), _) => {
                Self::CreateTempView { view_name }
            }
            (ffi::SQLITE_CREATE_TRIGGER, Some(trigger_name), Some(table_name)) => {
                Self::CreateTrigger {
                    trigger_name,
                    table_name,
                }
            }
            (ffi::SQLITE_CREATE_VIEW, Some(view_name), _) => Self::CreateView { view_name },
            (ffi::SQLITE_DELETE, Some(table_name), None) => Self::Delete { table_name },
            (ffi::SQLITE_DROP_INDEX, Some(index_name), Some(table_name)) => Self::DropIndex {
                index_name,
                table_name,
            },
            (ffi::SQLITE_DROP_TABLE, Some(table_name), _) => Self::DropTable { table_name },
            (ffi::SQLITE_DROP_TEMP_INDEX, Some(index_name), Some(table_name)) => {
                Self::DropTempIndex {
                    index_name,
                    table_name,
                }
            }
            (ffi::SQLITE_DROP_TEMP_TABLE, Some(table_name), _) => {
                Self::DropTempTable { table_name }
            }
            (ffi::SQLITE_DROP_TEMP_TRIGGER, Some(trigger_name), Some(table_name)) => {
                Self::DropTempTrigger {
                    trigger_name,
                    table_name,
                }
            }
            (ffi::SQLITE_DROP_TEMP_VIEW, Some(view_name), _) => Self::DropTempView { view_name },
            (ffi::SQLITE_DROP_TRIGGER, Some(trigger_name), Some(table_name)) => Self::DropTrigger {
                trigger_name,
                table_name,
            },
            (ffi::SQLITE_DROP_VIEW, Some(view_name), _) => Self::DropView { view_name },
            (ffi::SQLITE_INSERT, Some(table_name), _) => Self::Insert { table_name },
            (ffi::SQLITE_PRAGMA, Some(pragma_name), pragma_value) => Self::Pragma {
                pragma_name,
                pragma_value,
            },
            (ffi::SQLITE_READ, Some(table_name), Some(column_name)) => Self::Read {
                table_name,
                column_name,
            },
            (ffi::SQLITE_SELECT, ..) => Self::Select,
            (ffi::SQLITE_TRANSACTION, Some(operation_str), _) => Self::Transaction {
                operation: TransactionOperation::from_str(operation_str),
            },
            (ffi::SQLITE_UPDATE, Some(table_name), Some(column_name)) => Self::Update {
                table_name,
                column_name,
            },
            (ffi::SQLITE_ATTACH, Some(filename), _) => Self::Attach { filename },
            (ffi::SQLITE_DETACH, Some(database_name), _) => Self::Detach { database_name },
            (ffi::SQLITE_ALTER_TABLE, Some(database_name), Some(table_name)) => Self::AlterTable {
                database_name,
                table_name,
            },
            (ffi::SQLITE_REINDEX, Some(index_name), _) => Self::Reindex { index_name },
            (ffi::SQLITE_ANALYZE, Some(table_name), _) => Self::Analyze { table_name },
            (ffi::SQLITE_CREATE_VTABLE, Some(table_name), Some(module_name)) => {
                Self::CreateVtable {
                    table_name,
                    module_name,
                }
            }
            (ffi::SQLITE_DROP_VTABLE, Some(table_name), Some(module_name)) => Self::DropVtable {
                table_name,
                module_name,
            },
            (ffi::SQLITE_FUNCTION, _, Some(function_name)) => Self::Function { function_name },
            (ffi::SQLITE_SAVEPOINT, Some(operation_str), Some(savepoint_name)) => Self::Savepoint {
                operation: TransactionOperation::from_str(operation_str),
                savepoint_name,
            },
            (ffi::SQLITE_RECURSIVE, ..) => Self::Recursive,
            (code, arg1, arg2) => Self::Unknown { code, arg1, arg2 },
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum TransactionOperation {
    Unknown,
    Begin,
    Release,
    Rollback,
}
impl TransactionOperation {
    fn from_str(op_str: &str) -> Self {
        match op_str {
            "BEGIN" => Self::Begin,
            "RELEASE" => Self::Release,
            "ROLLBACK" => Self::Rollback,
            _ => Self::Unknown,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum Authorization {
    Allow,
    Ignore,
    Deny,
}
impl Authorization {
    fn into_raw(self) -> c_int {
        match self {
            Self::Allow => ffi::SQLITE_OK,
            Self::Ignore => ffi::SQLITE_IGNORE,
            Self::Deny => ffi::SQLITE_DENY,
        }
    }
}
impl Connection {
    #[inline]
    pub fn commit_hook<F>(&mut self, hook: Option<F>) -> Result<()>
    where
        F: FnMut() -> bool + Send + 'static,
    {
        self.db.borrow_mut().commit_hook(hook)
    }
    #[inline]
    pub fn rollback_hook<F>(&mut self, hook: Option<F>) -> Result<()>
    where
        F: FnMut() + Send + 'static,
    {
        self.db.borrow_mut().rollback_hook(hook)
    }
    #[inline]
    pub fn update_hook<F>(&mut self, hook: Option<F>) -> Result<()>
    where
        F: FnMut(Action, &str, &str, i64) + Send + 'static,
    {
        self.db.borrow_mut().update_hook(hook)
    }

    pub fn progress_handler<F>(&mut self, num_ops: c_int, handler: Option<F>) -> Result<()>
    where
        F: FnMut() -> bool + Send + 'static,
    {
        self.db.borrow_mut().progress_handler(num_ops, handler)
    }
    #[inline]
    pub fn authorizer<F>(&mut self, hook: Option<F>) -> Result<()>
    where
        F: for<'r> FnMut(AuthContext<'r>) -> Authorization + Send + 'static,
    {
        self.db.borrow_mut().authorizer(hook)
    }
}
pub struct Wal {
    db: InnerConnection,
    db_name: *const c_char,
}
impl Wal {
    pub fn checkpoint(&mut self) -> Result<()> {
        self.checkpoint_v2(CheckpointMode::default())?;
        Ok(())
    }
    pub fn checkpoint_v2(&mut self, mode: CheckpointMode) -> Result<(c_int, c_int)> {
        self.db
            .wal_checkpoint_v2(Some(unsafe { CStr::from_ptr(self.db_name) }), mode)
    }
    #[must_use]
    pub fn name(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.db_name) }
    }
}
impl InnerConnection {
    fn commit_hook<F>(&mut self, hook: Option<F>) -> Result<()>
    where
        F: FnMut() -> bool + Send + 'static,
    {
        unsafe extern "C" fn call_boxed_closure<F>(p_arg: *mut c_void) -> c_int
        where
            F: FnMut() -> bool,
        {
            unsafe {
                unsafe {
                    let r = catch_unwind(|| {
                        let boxed_hook: *mut F = p_arg.cast::<F>();
                        (*boxed_hook)()
                    });
                    c_int::from(r.unwrap_or_default())
                }
            }
        }
        let x = hook.as_ref().map(|_| call_boxed_closure::<F> as _);
        self.set_clientdata(c"sqlite3_commit_hook", hook, |db, bh| unsafe {
            ffi::sqlite3_commit_hook(db, x, bh);
            ffi::SQLITE_OK
        })?;
        Ok(())
    }
    fn rollback_hook<F>(&mut self, hook: Option<F>) -> Result<()>
    where
        F: FnMut() + Send + 'static,
    {
        unsafe extern "C" fn call_boxed_closure<F>(p_arg: *mut c_void)
        where
            F: FnMut(),
        {
            unsafe {
                unsafe {
                    drop(catch_unwind(|| {
                        let boxed_hook: *mut F = p_arg.cast::<F>();
                        (*boxed_hook)();
                    }));
                }
            }
        }
        let x = hook.as_ref().map(|_| call_boxed_closure::<F> as _);
        self.set_clientdata(c"sqlite3_rollback_hook", hook, |db, bh| unsafe {
            ffi::sqlite3_rollback_hook(db, x, bh);
            ffi::SQLITE_OK
        })?;
        Ok(())
    }
    fn update_hook<F>(&mut self, hook: Option<F>) -> Result<()>
    where
        F: FnMut(Action, &str, &str, i64) + Send + 'static,
    {
        unsafe extern "C" fn call_boxed_closure<F>(
            p_arg: *mut c_void,
            action_code: c_int,
            p_db_name: *const c_char,
            p_table_name: *const c_char,
            row_id: i64,
        ) where
            F: FnMut(Action, &str, &str, i64),
        {
            unsafe {
                let action = Action::from(action_code);
                unsafe {
                    drop(catch_unwind(|| {
                        let boxed_hook: *mut F = p_arg.cast::<F>();
                        (*boxed_hook)(
                            action,
                            expect_utf8(p_db_name, "database name"),
                            expect_utf8(p_table_name, "table name"),
                            row_id,
                        );
                    }));
                }
            }
        }
        let x = hook.as_ref().map(|_| call_boxed_closure::<F> as _);
        self.set_clientdata(c"sqlite3_update_hook", hook, |db, bh| unsafe {
            ffi::sqlite3_update_hook(db, x, bh);
            ffi::SQLITE_OK
        })?;
        Ok(())
    }
    fn progress_handler<F>(&mut self, num_ops: c_int, handler: Option<F>) -> Result<()>
    where
        F: FnMut() -> bool + Send + 'static,
    {
        unsafe extern "C" fn call_boxed_closure<F>(p_arg: *mut c_void) -> c_int
        where
            F: FnMut() -> bool,
        {
            unsafe {
                unsafe {
                    let r = catch_unwind(|| {
                        let boxed_handler: *mut F = p_arg.cast::<F>();
                        (*boxed_handler)()
                    });
                    c_int::from(r.unwrap_or_default())
                }
            }
        }
        let x = handler.as_ref().map(|_| call_boxed_closure::<F> as _);
        self.set_clientdata(c"sqlite3_progress_handler", handler, |db, bh| unsafe {
            ffi::sqlite3_progress_handler(db, num_ops, x, bh);
            ffi::SQLITE_OK
        })?;
        Ok(())
    }
    fn authorizer<'c, F>(&'c mut self, authorizer: Option<F>) -> Result<()>
    where
        F: for<'r> FnMut(AuthContext<'r>) -> Authorization + Send + 'static,
    {
        unsafe extern "C" fn call_boxed_closure<'c, F>(
            p_arg: *mut c_void,
            action_code: c_int,
            param1: *const c_char,
            param2: *const c_char,
            db_name: *const c_char,
            trigger_or_view_name: *const c_char,
        ) -> c_int
        where
            F: FnMut(AuthContext<'c>) -> Authorization + Send + 'static,
        {
            unsafe {
                unsafe {
                    catch_unwind(|| {
                        let action = AuthAction::from_raw(
                            action_code,
                            expect_optional_utf8(param1, "authorizer param 1"),
                            expect_optional_utf8(param2, "authorizer param 2"),
                        );
                        let auth_ctx = AuthContext {
                            action,
                            database_name: expect_optional_utf8(db_name, "database name"),
                            accessor: expect_optional_utf8(
                                trigger_or_view_name,
                                "accessor (inner-most trigger or view)",
                            ),
                        };
                        let boxed_hook: *mut F = p_arg.cast::<F>();
                        (*boxed_hook)(auth_ctx)
                    })
                    .map_or_else(|_| ffi::SQLITE_ERROR, Authorization::into_raw)
                }
            }
        }
        let x_auth = authorizer
            .as_ref()
            .map(|_| call_boxed_closure::<'c, F> as _);
        self.set_clientdata(c"sqlite3_set_authorizer", authorizer, |db, bh| unsafe {
            ffi::sqlite3_set_authorizer(db, x_auth, bh)
        })?;
        Ok(())
    }
}
unsafe fn expect_utf8<'a>(p_str: *const c_char, description: &'static str) -> &'a str {
    unsafe {
        unsafe {
            expect_optional_utf8(p_str, description)
                .unwrap_or_else(|| panic!("received empty {description}"))
        }
    }
}
unsafe fn expect_optional_utf8<'a>(
    p_str: *const c_char,
    description: &'static str,
) -> Option<&'a str> {
    unsafe {
        if p_str.is_null() {
            return None;
        }
        unsafe {
            CStr::from_ptr(p_str)
                .to_str()
                .unwrap_or_else(|_| panic!("received non-utf8 string as {description}"))
                .into()
        }
    }
}
