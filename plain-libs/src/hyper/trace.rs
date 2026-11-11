macro_rules! debug { ($($arg : tt) +) => { #[cfg (any ())] { tracing::debug ! ($($arg) +) ; } } }
macro_rules! debug_span { ($($arg : tt) *) => { { #[cfg (any ())] { let _span = tracing::debug_span ! ($($arg) +) ; _span . entered () } } } }
macro_rules! error { ($($arg : tt) *) => { #[cfg (any ())] { tracing::error ! ($($arg) +) ; } } }
macro_rules! error_span { ($($arg : tt) *) => { { #[cfg (any ())] { let _span = tracing::error_span ! ($($arg) +) ; _span . entered () } } } }
macro_rules! info { ($($arg : tt) *) => { #[cfg (any ())] { tracing::info ! ($($arg) +) ; } } }
macro_rules! info_span { ($($arg : tt) *) => { { #[cfg (any ())] { let _span = tracing::info_span ! ($($arg) +) ; _span . entered () } } } }
macro_rules! trace { ($($arg : tt) *) => { #[cfg (any ())] { tracing::trace ! ($($arg) +) ; } } }
macro_rules! trace_span { ($($arg : tt) *) => { { #[cfg (any ())] { let _span = tracing::trace_span ! ($($arg) +) ; _span . entered () } } } }
macro_rules! span { ($($arg : tt) *) => { { #[cfg (any ())] { let _span = tracing::span ! ($($arg) +) ; _span . entered () } } } }
macro_rules! warn { ($($arg : tt) *) => { #[cfg (any ())] { tracing::warn ! ($($arg) +) ; } } }
macro_rules! warn_span { ($($arg : tt) *) => { { #[cfg (any ())] { let _span = tracing::warn_span ! ($($arg) +) ; _span . entered () } } } }
