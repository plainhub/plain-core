pub use core::borrow::Borrow;
pub use core::cell::UnsafeCell;
pub use core::cmp::{Eq, PartialEq};
pub use core::convert::AsRef;
pub use core::default::Default;
pub use core::ffi::CStr;
pub use core::fmt;
pub use core::hash::{Hash, Hasher};
pub use core::marker::{PhantomData, Sized};
pub use core::mem::{size_of, transmute, ManuallyDrop, MaybeUninit};
pub use core::ops::Deref;
pub use core::option::Option::{self, None, Some};
pub use core::primitive::{bool, isize, str, u8};
pub use core::{compile_error, concat, env, module_path, panic, stringify};
// TODO: Use `core::cell::LazyCell`
pub use std::sync::Once;

#[cfg(any())]
pub use core::os::darwin::objc as core_darwin_objc;

mod cache;
mod class;
mod common_selectors;
mod convert;
mod define_class;
pub(crate) mod defined_ivars;
mod image_info;
mod method_family;
mod module_info;
mod msg_send_retained;
mod null_error;
mod os_version;
mod retain_semantics;
mod sync_unsafe_cell;
mod writeback;

pub use self::cache::{CachedClass, CachedSel};
pub use self::class::{DoesNotImplDrop, MainThreadOnlyDoesNotImplSendSync, ValidThreadKind};
pub use self::common_selectors::{alloc_sel, dealloc_sel, init_sel, new_sel};
pub use self::convert::{ConvertArgument, ConvertArguments, ConvertReturn, TupleExtender};
pub use self::define_class::{
    class_c_name, define_class, ClassBuilderHelper, ClassProtocolMethodsBuilder,
    MaybeOptionRetained, MessageReceiveRetained, RetainedReturnValue, ThreadKindAutoTraits,
};
pub use self::defined_ivars::DefinedIvarsHelper;
pub use self::image_info::ImageInfo;
pub use self::method_family::{
    method_family, method_family_import, AllocFamily, AutoreleaseSelector, CopyFamily,
    DeallocSelector, InitFamily, MethodFamily, MutableCopyFamily, NewFamily, NoneFamily,
    ReleaseSelector, RetainSelector,
};
pub use self::module_info::ModuleInfo;
pub use self::msg_send_retained::{MsgSend, MsgSendError, MsgSendSuper, MsgSendSuperError};
pub use self::os_version::{is_available, AvailableVersion, OSVersion};
pub use self::retain_semantics::{
    KindDefined, KindSendMessage, KindSendMessageSuper, RetainSemantics,
};
pub use self::sync_unsafe_cell::SyncUnsafeCell;

/// Disallow using this passed in value in const and statics for forwards
/// compatibility (this function is not a `const` function).
#[inline]
pub fn disallow_in_static<T>(item: &'static T) -> &'static T {
    item
}

#[deprecated = "having the `impl` inside `extern_methods!` is deprecated, move it outside instead"]
pub const fn extern_methods_unsafe_impl() {}

#[cfg(test)]
#[path = "../../../../../tests/native/objc2/unit/__macro_helpers/mod.rs"]
mod tests;
