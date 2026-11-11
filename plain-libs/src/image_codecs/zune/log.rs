#[repr(usize)]
#[derive(Copy, Clone, Eq, PartialEq, Debug, Hash)]
pub enum Level {
    Error = 1,
    Warn,
    Info,
    Debug,
    Trace,
}
macro_rules! __log_enabled {
    ($ lvl : expr) => {{
        let _ = $lvl;
        false
    }};
}
macro_rules ! __error { ($ ($ arg : tt) +) => { { let _ = :: core :: format_args ! ($ ($ arg) +) ; } } ; }
macro_rules ! __warn { ($ ($ arg : tt) +) => { { let _ = :: core :: format_args ! ($ ($ arg) +) ; } } ; }
macro_rules ! __info { ($ ($ arg : tt) +) => { { let _ = :: core :: format_args ! ($ ($ arg) +) ; } } ; }
macro_rules ! __debug { ($ ($ arg : tt) +) => { { let _ = :: core :: format_args ! ($ ($ arg) +) ; } } ; }
macro_rules ! __trace { ($ ($ arg : tt) +) => { { let _ = :: core :: format_args ! ($ ($ arg) +) ; } } ; }

pub(crate) use {
    __debug as debug, __error as error, __info as info, __log_enabled as log_enabled,
    __trace as trace, __warn as warn,
};
