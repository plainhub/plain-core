//! # Supporting code for instance variables on defined classes.
//!
//! Adding instance variables to Objective-C classes is fairly simple, it can
//! be done using `ClassBuilder::add_ivar`.
//!
//! However, things become more complicated once we have to handle `Drop`,
//! deallocation and unwind safety; remember, `dealloc` may be called even on
//! newly, non-initialized instances.
//!
//! Note that Swift [doesn't handle this][swift-deinit-unsound], but that
//! doesn't mean we can simply stick our heads in the sand.
//!
//! Basically, instead of storing the ivars directly, we store it as the
//! following tagged enum:
//! ```
//! #[repr(u8)]
//! enum ActualIvar<T: objc2::DefinedClass> {
//!     Allocated = 0,
//!     PartialInit(T::Ivars),
//!     Finalized(T::Ivars),
//! }
//! ```
//!
//! For performance reasons, we unfortunately can't write it that cleanly: we
//! want the data and the drop flag as two separate ivars instead of combining
//! them into one, since that will give the layout algorithm in the
//! Objective-C runtime more information to work with, and it allows us to
//! selectively omit the drop flag or the data storage when either is not
//! needed.
//!
//! Ideally, we'd be able to somehow statically detect when the ivars have a
//! zero niche, which would allow us to know if the type is safe to drop when
//! zero-initialized:
//! ```ignore
//! None::<T::Ivars>.is_all_zeroes_bitpattern()
//! ```
//!
//! However, detecting if the `None` is all zeroes requires reading the bytes,
//! which is [unsound for types that may have padding][unsound-read-padding],
//! since that padding is uninitialized.
//!
//! So this is an optimization that we don't yet do, but that may be possible
//! in the future using something like `bytemuck::ZeroableInOption`.
//!
//! [swift-deinit-unsound]: https://github.com/apple/swift/issues/68734
//! [unsound-read-padding]: https://play.rust-lang.org/?version=stable&mode=debug&edition=2021&gist=ea068e8d9e55801aa9520ea914eb2822

use alloc::borrow::Cow;
use alloc::ffi::CString;
use alloc::format;
use core::ffi::CStr;
use core::mem;
use core::ptr::{self, NonNull};

use crate::encode::{Encode, Encoding};
use crate::runtime::{AnyClass, AnyObject, ClassBuilder, MessageReceiver, Sel};
use crate::{sel, ClassType, DefinedClass};

/// A type representing the drop flags that may be set for a type.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DropFlag {
    /// Set to zero to ensure that this is the default when created by the
    /// Objective-C runtime.
    ///
    /// Ivars are [documented][obj-init-zeroed] to be zero-initialized after
    /// allocation, and that has been true since at least [the Objective-C
    /// version shipped with Mac OS X 10.0][objc4-208-init].
    ///
    /// [obj-init-zeroed]: https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/ProgrammingWithObjectiveC/WorkingwithObjects/WorkingwithObjects.html#//apple_ref/doc/uid/TP40011210-CH4-SW7
    /// [objc4-208-init]: https://github.com/apple-oss-distributions/objc4/blob/objc4-208/runtime/objc-class.m#L367
    #[allow(dead_code)]
    Allocated = 0x00,
    /// Used when `mem::needs_drop::<T::Ivars>()`, or with debug assertions enabled.
    InitializedIvars = 0x0f,
    /// Used when `mem::needs_drop::<T>()`, or with debug assertions enabled.
    Finalized = 0xff,
}

// SAFETY: The DropFlag is #[repr(u8)]
unsafe impl Encode for DropFlag {
    const ENCODING: Encoding = u8::ENCODING;
}

pub trait DefinedIvarsHelper {
    const HAS_IVARS: bool;
    const HAS_DROP_FLAG: bool;
}

impl<T: DefinedClass> DefinedIvarsHelper for T {
    /// Only add ivar if we need the runtime to allocate memory for it.
    ///
    /// We can avoid doing so if the type is a zero-sized type (ZST), and the
    /// required alignment is less than the alignment of a pointer (objects
    /// are guaranteed to have at least that alignment themselves).
    const HAS_IVARS: bool = {
        mem::size_of::<T::Ivars>() > 0
            || mem::align_of::<T::Ivars>() > mem::align_of::<*mut AnyObject>()
    };
    /// Only add drop flag if the type or the ivars need it.
    ///
    /// `needs_drop::<T>` can reliably detect a direct implementation of
    /// `Drop`, since the type only includes `ManuallyDrop` or `PhantomData`
    /// fields.
    const HAS_DROP_FLAG: bool = mem::needs_drop::<T>() || mem::needs_drop::<T::Ivars>();
}

/// Helper function for getting a pointer to the instance variable.
///
/// # Safety
///
/// The pointer must be valid, and the instance variable offset (if it has
/// any) must have been initialized.
#[inline]
unsafe fn ptr_to_ivar<T: ?Sized + DefinedClass>(ptr: NonNull<T>) -> NonNull<T::Ivars> {
    // This is called even when there is no ivars, but that's fine, since in
    // that case the ivar is zero-sized, and the offset will be zero, so we
    // can still compute a valid pointer to the ivar.
    //
    // debug_assert!(T::HAS_IVARS);

    // SAFETY: That an instance variable with the given type exists at the
    // specified offset is ensured by `DefinedClass` trait implementor.
    unsafe { AnyObject::ivar_at_offset::<T::Ivars>(ptr.cast(), T::__ivars_offset()) }
}

/// Helper function for getting a pointer to the drop flag.
///
/// # Safety
///
/// The pointer must be valid and have an initialized drop flag.
#[inline]
unsafe fn ptr_to_drop_flag<T: DefinedClass>(ptr: NonNull<T>) -> *mut DropFlag {
    debug_assert!(T::HAS_DROP_FLAG, "type did not have drop flag");
    // SAFETY: That a drop flag exists at the specified offset is ensured
    // by caller.
    unsafe { AnyObject::ivar_at_offset::<DropFlag>(ptr.cast(), T::__drop_flag_offset()).as_ptr() }
}

pub(crate) fn setup_dealloc<T: DefinedClass>(builder: &mut ClassBuilder)
where
    T::Super: ClassType,
{
    // Add dealloc if the class or the ivars need dropping.
    if mem::needs_drop::<T>() || mem::needs_drop::<T::Ivars>() {
        let func: unsafe extern "C-unwind" fn(_, _) = dealloc::<T>;
        // SAFETY: The function signature is correct, and method contract is
        // upheld inside `dealloc`.
        unsafe { builder.add_method(sel!(dealloc), func) };
    } else {
        // Users should not rely on this omission, it is only an optimization.
    }
}

/// The `dealloc` Objective-C method.
///
/// See the following links for more details about `dealloc`:
/// - <https://clang.llvm.org/docs/AutomaticReferenceCounting.html#dealloc>
/// - <https://developer.apple.com/documentation/objectivec/nsobject/1571947-dealloc>
/// - <https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/MemoryMgmt/Articles/mmRules.html#//apple_ref/doc/uid/20000994-SW2>
unsafe extern "C-unwind" fn dealloc<T: DefinedClass>(this: NonNull<T>, cmd: Sel)
where
    T::Super: ClassType,
{
    /// Helper function for marking the cold path when branching.
    #[inline]
    #[cold]
    fn cold_path() {}

    // SAFETY: `dealloc` is only registered when there is a need for dropping,
    // and hence a need for a drop flag.
    let drop_flag = unsafe { *ptr_to_drop_flag(this) };

    if mem::needs_drop::<T>() {
        match drop_flag {
            // Don't deallocate the current instance if it has not been fully
            // initialized.
            //
            // Note that we still run the superclass deinitializer below.
            DropFlag::Allocated | DropFlag::InitializedIvars => cold_path(),
            // SAFETY: This is the `dealloc` method, so we know that the type
            // never needs to be deallocated again.
            //
            // Additionally, we know that the type was fully initialized, since
            // that's what the drop flag says.
            //
            // TODO: This can unwind, is it correct to just let that
            // propagate?
            DropFlag::Finalized => unsafe { ptr::drop_in_place(this.as_ptr()) },
        }
    }

    // Note: This should be done inside `.cxx_destruct`, since if a superclass
    // calls an overwritten method in its `dealloc`, it can access
    // deinitialized instance variables; but we can't do that without
    // generating statics, so we have to do it in `dealloc` for now.
    //
    // It is very important that we do this after the `Drop` of the class
    // itself above, though.
    //
    // Another possibility would be to read the contents of the ivars onto the
    // stack here, and only deinitialize after the superclass' `dealloc`, but
    // that would break the pinning guarantee that ivars otherwise have.
    if mem::needs_drop::<T::Ivars>() {
        match drop_flag {
            // Do nothing if the ivars have not been initialized.
            DropFlag::Allocated => cold_path(),
            DropFlag::InitializedIvars | DropFlag::Finalized => {
                // SAFETY: The instance variable is initialized, so it is
                // valid to drop here.
                //
                // TODO: This can unwind, is it correct to just let that
                // propagate?
                unsafe { ptr::drop_in_place(ptr_to_ivar(this).as_ptr()) };
            }
        }
    }

    // The superclass' "marker" that this stores is wrapped in `ManuallyDrop`,
    // we drop it by calling the superclass' `dealloc` method instead.
    //
    // Note: ARC does this automatically, which means most Objective-C code in
    // the wild don't contain this call; but we _are_ ARC, so we must do this.
    //
    // SAFETY: The argument and return types are correct, and we make sure to
    // only call this once.
    unsafe {
        MessageReceiver::send_super_message(
            this,
            <T as ClassType>::Super::class(),
            cmd, // Reuse the selector
            (),  // No arguments
        )
    }
}

#[inline]
pub(crate) fn ivar_drop_flag_names<T: DefinedClass>() -> (Cow<'static, CStr>, Cow<'static, CStr>) {
    if cfg!(any()) {
        // GNUStep does not support a subclass having an ivar with the
        // same name as a superclass, so let's use the class name as the
        // ivar name to ensure uniqueness.
        (
            CString::new(format!("{}_ivars", T::NAME)).unwrap().into(),
            CString::new(format!("{}_drop_flag", T::NAME))
                .unwrap()
                .into(),
        )
    } else {
        // SAFETY: The byte slices are NUL-terminated, and do not contain
        // interior NUL bytes.
        // TODO: Use `c"my_str"` syntax once in MSRV
        unsafe {
            (
                CStr::from_bytes_with_nul_unchecked(b"ivars\0").into(),
                CStr::from_bytes_with_nul_unchecked(b"drop_flag\0").into(),
            )
        }
    }
}

/// Register the ivars.
#[inline]
pub(crate) fn register_ivars<T: DefinedClass>(builder: &mut ClassBuilder, ivars_name: &CStr) {
    if T::HAS_IVARS {
        // TODO: Consider not adding a encoding - Swift doesn't do it.
        let ivar_encoding = Encoding::Array(
            mem::size_of::<T::Ivars>() as u64,
            match mem::align_of::<T::Ivars>() {
                1 => &u8::ENCODING,
                2 => &u16::ENCODING,
                4 => &u32::ENCODING,
                // The alignment of `u64` may not be 8 on all architectures
                8 if mem::align_of::<u64>() == 8 => &u64::ENCODING,
                alignment => panic!("unsupported alignment {alignment} for `{}::Ivars`", T::NAME),
            },
        );
        unsafe { builder.add_ivar_inner::<T::Ivars>(ivars_name, &ivar_encoding) };
    }
}

/// Register the drop flag ivar.
#[inline]
pub(crate) fn register_drop_flag<T: DefinedClass>(
    builder: &mut ClassBuilder,
    drop_flag_name: &CStr,
) {
    if T::HAS_DROP_FLAG {
        // TODO: Maybe we can reuse the drop flag when subclassing an already
        // defined class?
        builder.add_ivar::<DropFlag>(drop_flag_name);
    }
}

#[inline]
pub(crate) fn ivars_offset<T: DefinedClass>(cls: &AnyClass, ivars_name: &CStr) -> isize {
    if T::HAS_IVARS {
        // Monomorphized error handling
        // Intentionally not #[track_caller], we expect this error to never occur
        fn get_ivar_failed() -> ! {
            unreachable!("failed retrieving instance variable on newly defined class")
        }

        cls.instance_variable(ivars_name)
            .unwrap_or_else(|| get_ivar_failed())
            .offset()
    } else {
        // Fallback to an offset of zero.
        //
        // This is fine, since any reads here will only be via zero-sized
        // ivars, where the actual pointer doesn't matter.
        0
    }
}

#[inline]
pub(crate) fn drop_flag_offset<T: DefinedClass>(cls: &AnyClass, drop_flag_name: &CStr) -> isize {
    if T::HAS_DROP_FLAG {
        // Monomorphized error handling
        // Intentionally not #[track_caller], we expect this error to never occur
        fn get_drop_flag_failed() -> ! {
            unreachable!("failed retrieving drop flag instance variable on newly defined class")
        }

        cls.instance_variable(drop_flag_name)
            .unwrap_or_else(|| get_drop_flag_failed())
            .offset()
    } else {
        // Fall back to an offset of zero.
        //
        // This is fine, since the drop flag is never actually used in the
        // cases where it was not added.
        0
    }
}

/// # Safety
///
/// The pointer must be a valid, newly allocated instance.
#[inline]
#[track_caller]
pub(crate) unsafe fn initialize_ivars<T: DefinedClass>(ptr: NonNull<T>, val: T::Ivars) {
    // Debug assert the state of the drop flag
    if T::HAS_DROP_FLAG && cfg!(debug_assertions) {
        // SAFETY: Just checked that the drop flag is available.
        match unsafe { *ptr_to_drop_flag(ptr) } {
            DropFlag::Allocated => {
                // Allow initialization after allocation
            }
            DropFlag::InitializedIvars => {
                panic!("tried to initialize ivars after they were already initialized")
            }
            DropFlag::Finalized => {
                panic!("tried to initialize ivars on an already initialized object")
            }
        }
    }

    // SAFETY:
    // - Caller ensures the pointer is valid.
    // - The location is properly aligned by `ClassBuilder::add_ivar`.
    // - This write is done as part of initialization, so we know that the
    //   pointer is not shared elsewhere.
    unsafe { ptr_to_ivar(ptr).as_ptr().write(val) };

    // Write to drop flag that we've initialized the instance variables.
    //
    // Note: We intentionally only do this _after_ writing to the ivars,
    // for better unwind safety.
    if T::HAS_DROP_FLAG && (mem::needs_drop::<T::Ivars>() || cfg!(debug_assertions)) {
        // SAFETY: Just checked that the drop flag is available.
        unsafe { ptr_to_drop_flag(ptr).write(DropFlag::InitializedIvars) }
    }
}

/// # Safety
///
/// The pointer must be valid and finalized (i.e. all super initializers must
/// have been run).
#[inline]
#[track_caller]
pub(crate) unsafe fn set_finalized<T: DefinedClass>(ptr: NonNull<T>) {
    // Debug assert the state of the drop flag
    if T::HAS_DROP_FLAG && cfg!(debug_assertions) {
        // SAFETY: Just checked that the drop flag is available.
        match unsafe { *ptr_to_drop_flag(ptr) } {
            DropFlag::Allocated => {
                panic!("tried to finalize an object that was not yet fully initialized")
            }
            DropFlag::InitializedIvars => {
                // Allow finalizing after initialization
            }
            DropFlag::Finalized => {
                panic!("tried to finalize an already finalized object")
            }
        }
    }

    // Write to drop flag that we've fully initialized the class.
    if T::HAS_DROP_FLAG && (mem::needs_drop::<T>() || cfg!(debug_assertions)) {
        // SAFETY: Just checked that the drop flag is available.
        unsafe { ptr_to_drop_flag(ptr).write(DropFlag::Finalized) }
    }
}

/// # Safety
///
/// The pointer must be valid and the instance variables must be initialized.
#[inline]
#[track_caller]
pub(crate) unsafe fn get_initialized_ivar_ptr<T: DefinedClass>(
    ptr: NonNull<T>,
) -> NonNull<T::Ivars> {
    // Debug assert the state of the drop flag
    if T::HAS_DROP_FLAG && cfg!(debug_assertions) {
        // SAFETY: Just checked that the drop flag is available.
        match unsafe { *ptr_to_drop_flag(ptr) } {
            DropFlag::Allocated => {
                panic!("tried to access uninitialized instance variable")
            }
            DropFlag::InitializedIvars => {
                // Allow accessing even if not finalized, since we only set
                // that state _after_ it actually happens, while accesses may
                // be done by the superclass initializer in e.g. an
                // overwritten method.
            }
            DropFlag::Finalized => {
                // Allow accessing if finalized
            }
        }
    }

    // SAFETY: That the pointer is valid is ensured by caller.
    unsafe { ptr_to_ivar(ptr) }
}

#[cfg(test)]
#[path = "../../../../../tests/native/objc2/unit/__macro_helpers/defined_ivars.rs"]
mod tests;
