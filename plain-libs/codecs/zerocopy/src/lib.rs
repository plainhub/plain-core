// Copyright 2018 The Fuchsia Authors
//
// Licensed under the 2-Clause BSD License <LICENSE-BSD or
// https://opensource.org/license/bsd-2-clause>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

// After updating the following doc comment, make sure to run the following
// command to update `README.md` based on its contents:
//
//   ./generate-readme.sh > README.md

//! *<span style="font-size: 100%; color:grey;">Want to help improve zerocopy?
//! Fill out our [user survey][user-survey]!</span>*
//!
//! ***<span style="font-size: 140%">Fast, safe, <span
//! style="color:red;">compile error</span>. Pick two.</span>***
//!
//! Zerocopy makes zero-cost memory manipulation effortless. We write `unsafe`
//! so you don't have to.
//!
//! # Overview
//!
//! Zerocopy provides four core marker traits, each of which can be derived
//! (e.g., `#[derive(FromZeroes)]`):
//! - [`FromZeroes`] indicates that a sequence of zero bytes represents a valid
//!   instance of a type
//! - [`FromBytes`] indicates that a type may safely be converted from an
//!   arbitrary byte sequence
//! - [`AsBytes`] indicates that a type may safely be converted *to* a byte
//!   sequence
//! - [`Unaligned`] indicates that a type's alignment requirement is 1
//!
//! Types which implement a subset of these traits can then be converted to/from
//! byte sequences with little to no runtime overhead.
//!
//! Zerocopy also provides byte-order aware integer types that support these
//! conversions; see the [`byteorder`] module. These types are especially useful
//! for network parsing.
//!
//! [user-survey]: https://docs.google.com/forms/d/e/1FAIpQLSdzBNTN9tzwsmtyZxRFNL02K36IWCdHWW2ZBckyQS2xiO3i8Q/viewform?usp=published_options
//!
//! # Cargo Features
//!
//! - **`alloc`**
//!   By default, `zerocopy` is `no_std`. When the `alloc` feature is enabled,
//!   the `alloc` crate is added as a dependency, and some allocation-related
//!   functionality is added.
//!
//! - **`byteorder`** (enabled by default)
//!   Adds the [`byteorder`] module and a dependency on the `byteorder` crate.
//!   The `byteorder` module provides byte order-aware equivalents of the
//!   multi-byte primitive numerical types. Unlike their primitive equivalents,
//!   the types in this module have no alignment requirement and support byte
//!   order conversions. This can be useful in handling file formats, network
//!   packet layouts, etc which don't provide alignment guarantees and which may
//!   use a byte order different from that of the execution platform.
//!
//! - **`derive`**
//!   Provides derives for the core marker traits via the `zerocopy-derive`
//!   crate. These derives are re-exported from `zerocopy`, so it is not
//!   necessary to depend on `zerocopy-derive` directly.
//!
//!   However, you may experience better compile times if you instead directly
//!   depend on both `zerocopy` and `zerocopy-derive` in your `Cargo.toml`,
//!   since doing so will allow Rust to compile these crates in parallel. To do
//!   so, do *not* enable the `derive` feature, and list both dependencies in
//!   your `Cargo.toml` with the same leading non-zero version number; e.g:
//!
//!   ```toml
//!   [dependencies]
//!   zerocopy = "0.X"
//!   zerocopy-derive = "0.X"
//!   ```
//!
//! - **`simd`**
//!   When the `simd` feature is enabled, `FromZeroes`, `FromBytes`, and
//!   `AsBytes` impls are emitted for all stable SIMD types which exist on the
//!   target platform. Note that the layout of SIMD types is not yet stabilized,
//!   so these impls may be removed in the future if layout changes make them
//!   invalid. For more information, see the Unsafe Code Guidelines Reference
//!   page on the [layout of packed SIMD vectors][simd-layout].
//!
//! - **`simd-nightly`**
//!   Enables the `simd` feature and adds support for SIMD types which are only
//!   available on nightly. Since these types are unstable, support for any type
//!   may be removed at any point in the future.
//!
//! [simd-layout]: https://rust-lang.github.io/unsafe-code-guidelines/layout/packed-simd-vectors.html
//!
//! # Security Ethos
//!
//! Zerocopy is expressly designed for use in security-critical contexts. We
//! strive to ensure that that zerocopy code is sound under Rust's current
//! memory model, and *any future memory model*. We ensure this by:
//! - **...not 'guessing' about Rust's semantics.**
//!   We annotate `unsafe` code with a precise rationale for its soundness that
//!   cites a relevant section of Rust's official documentation. When Rust's
//!   documented semantics are unclear, we work with the Rust Operational
//!   Semantics Team to clarify Rust's documentation.
//! - **...rigorously testing our implementation.**
//!   We run tests using [Miri], ensuring that zerocopy is sound across a wide
//!   array of supported target platforms of varying endianness and pointer
//!   width, and across both current and experimental memory models of Rust.
//! - **...formally proving the correctness of our implementation.**
//!   We apply formal verification tools like [Kani][kani] to prove zerocopy's
//!   correctness.
//!
//! For more information, see our full [soundness policy].
//!
//! [Miri]: https://github.com/rust-lang/miri
//! [Kani]: https://github.com/model-checking/kani
//! [soundness policy]: https://github.com/google/zerocopy/blob/main/POLICIES.md#soundness
//!
//! # Relationship to Project Safe Transmute
//!
//! [Project Safe Transmute] is an official initiative of the Rust Project to
//! develop language-level support for safer transmutation. The Project consults
//! with crates like zerocopy to identify aspects of safer transmutation that
//! would benefit from compiler support, and has developed an [experimental,
//! compiler-supported analysis][mcp-transmutability] which determines whether,
//! for a given type, any value of that type may be soundly transmuted into
//! another type. Once this functionality is sufficiently mature, zerocopy
//! intends to replace its internal transmutability analysis (implemented by our
//! custom derives) with the compiler-supported one. This change will likely be
//! an implementation detail that is invisible to zerocopy's users.
//!
//! Project Safe Transmute will not replace the need for most of zerocopy's
//! higher-level abstractions. The experimental compiler analysis is a tool for
//! checking the soundness of `unsafe` code, not a tool to avoid writing
//! `unsafe` code altogether. For the foreseeable future, crates like zerocopy
//! will still be required in order to provide higher-level abstractions on top
//! of the building block provided by Project Safe Transmute.
//!
//! [Project Safe Transmute]: https://rust-lang.github.io/rfcs/2835-project-safe-transmute.html
//! [mcp-transmutability]: https://github.com/rust-lang/compiler-team/issues/411
//!
//! # MSRV
//!
//! See our [MSRV policy].
//!
//! [MSRV policy]: https://github.com/google/zerocopy/blob/main/POLICIES.md#msrv
//!
//! # Changelog
//!
//! Zerocopy uses [GitHub Releases].
//!
//! [GitHub Releases]: https://github.com/google/zerocopy/releases

// Sometimes we want to use lints which were added after our MSRV.
// `unknown_lints` is `warn` by default and we deny warnings in CI, so without
// this attribute, any unknown lint would cause a CI failure when testing with
// our MSRV.
//
// TODO(#1201): Remove `unexpected_cfgs`

#![deny(renamed_and_removed_lints)]
#![deny(
    anonymous_parameters,
    deprecated_in_future,
    late_bound_lifetime_arguments,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    path_statements,
    patterns_in_fns_without_body,
    rust_2018_idioms,
    trivial_numeric_casts,
    unreachable_pub,
    unsafe_op_in_unsafe_fn,
    unused_extern_crates,
    // We intentionally choose not to deny `unused_qualifications`. When items
    // are added to the prelude (e.g., `core::mem::size_of`), this has the
    // consequence of making some uses trigger this lint on the latest toolchain
    // (e.g., `mem::size_of`), but fixing it (e.g. by replacing with `size_of`)
    // does not work on older toolchains.
    //
    // We tested a more complicated fix in #1413, but ultimately decided that,
    // since this lint is just a minor style lint, the complexity isn't worth it
    // - it's fine to occasionally have unused qualifications slip through,
    // especially since these do not affect our user-facing API in any way.
    variant_size_differences
)]
#![cfg_attr(
    __INTERNAL_USE_ONLY_NIGHLTY_FEATURES_IN_TESTS,
    deny(fuzzy_provenance_casts, lossy_provenance_casts)
)]
#![deny(
    clippy::all,
    clippy::alloc_instead_of_core,
    clippy::arithmetic_side_effects,
    clippy::as_underscore,
    clippy::assertions_on_result_states,
    clippy::as_conversions,
    clippy::correctness,
    clippy::dbg_macro,
    clippy::decimal_literal_representation,
    clippy::get_unwrap,
    clippy::indexing_slicing,
    clippy::missing_inline_in_public_items,
    clippy::missing_safety_doc,
    clippy::obfuscated_if_else,
    clippy::perf,
    clippy::print_stdout,
    clippy::std_instead_of_core,
    clippy::style,
    clippy::suspicious,
    clippy::todo,
    clippy::undocumented_unsafe_blocks,
    clippy::unimplemented,
    clippy::unnested_or_patterns,
    clippy::unwrap_used,
    clippy::use_debug
)]
#![deny(
    rustdoc::bare_urls,
    rustdoc::broken_intra_doc_links,
    rustdoc::invalid_codeblock_attributes,
    rustdoc::invalid_html_tags,
    rustdoc::invalid_rust_codeblocks,
    rustdoc::missing_crate_level_docs,
    rustdoc::private_intra_doc_links
)]
// In test code, it makes sense to weight more heavily towards concise, readable
// code over correct or debuggable code.
#![cfg_attr(not(test), no_std)]
#![cfg_attr(
    all(
        feature = "simd-nightly",
        any(target_arch = "x86", target_arch = "x86_64")
    ),
    feature(stdarch_x86_avx512)
)]
#![cfg_attr(
    all(feature = "simd-nightly", target_arch = "arm"),
    feature(stdarch_arm_dsp, stdarch_arm_neon_intrinsics)
)]
#![cfg_attr(
    all(
        feature = "simd-nightly",
        any(target_arch = "powerpc", target_arch = "powerpc64")
    ),
    feature(stdarch_powerpc)
)]
#![cfg_attr(doc_cfg, feature(doc_cfg))]
#![cfg_attr(
    __INTERNAL_USE_ONLY_NIGHLTY_FEATURES_IN_TESTS,
    feature(layout_for_ptr, strict_provenance)
)]

// This is a hack to allow zerocopy-derive derives to work in this crate. They
// assume that zerocopy is linked as an extern crate, so they access items from
// it as `zerocopy::Xxx`. This makes that still work.
#[cfg(any(feature = "derive", test))]
extern crate self as zerocopy;

#[macro_use]
mod macros;

#[cfg(feature = "byteorder")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "byteorder")))]
pub mod byteorder;
#[doc(hidden)]
pub mod macro_util;
mod post_monomorphization_compile_fail_tests;
mod util;
// TODO(#252): If we make this pub, come up with a better name.
mod wrappers;

#[cfg(feature = "byteorder")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "byteorder")))]
pub use crate::byteorder::*;
pub use crate::wrappers::*;

#[cfg(any(feature = "derive", test))]
#[cfg_attr(doc_cfg, doc(cfg(feature = "derive")))]
pub use zerocopy_derive::Unaligned;

// `pub use` separately here so that we can mark it `#[doc(hidden)]`.
//
// TODO(#29): Remove this or add a doc comment.
#[cfg(any(feature = "derive", test))]
#[cfg_attr(doc_cfg, doc(cfg(feature = "derive")))]
#[doc(hidden)]
pub use zerocopy_derive::KnownLayout;

use core::{
    cell::{self, RefMut},
    cmp::Ordering,
    fmt::{self, Debug, Display, Formatter},
    hash::Hasher,
    marker::PhantomData,
    mem::{self, ManuallyDrop, MaybeUninit},
    num::{
        NonZeroI128, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI8, NonZeroIsize, NonZeroU128,
        NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU8, NonZeroUsize, Wrapping,
    },
    ops::{Deref, DerefMut},
    ptr::{self, NonNull},
    slice,
};

#[cfg(feature = "alloc")]
extern crate alloc;
#[cfg(feature = "alloc")]
use alloc::{boxed::Box, vec::Vec};

#[cfg(any(feature = "alloc", kani))]
use core::alloc::Layout;

// Used by `TryFromBytes::is_bit_valid`.
#[doc(hidden)]
pub use crate::util::ptr::Ptr;

// For each polyfill, as soon as the corresponding feature is stable, the
// polyfill import will be unused because method/function resolution will prefer
// the inherent method/function over a trait method/function. Thus, we suppress
// the `unused_imports` warning.
//
// See the documentation on `util::polyfills` for more information.

use crate::util::polyfills::NonNullExt as _;

/// The target pointer width, counted in bits.
const POINTER_WIDTH_BITS: usize = mem::size_of::<usize>() * 8;

/// The layout of a type which might be dynamically-sized.
///
/// `DstLayout` describes the layout of sized types, slice types, and "slice
/// DSTs" - ie, those that are known by the type system to have a trailing slice
/// (as distinguished from `dyn Trait` types - such types *might* have a
/// trailing slice type, but the type system isn't aware of it).
///
/// # Safety
///
/// Unlike [`core::alloc::Layout`], `DstLayout` is only used to describe full
/// Rust types - ie, those that satisfy the layout requirements outlined by
/// [the reference]. Callers may assume that an instance of `DstLayout`
/// satisfies any conditions imposed on Rust types by the reference.
///
/// If `layout: DstLayout` describes a type, `T`, then it is guaranteed that:
/// - `layout.align` is equal to `T`'s alignment
/// - If `layout.size_info` is `SizeInfo::Sized { size }`, then `T: Sized` and
///   `size_of::<T>() == size`
/// - If `layout.size_info` is `SizeInfo::SliceDst(slice_layout)`, then
///   - `T` is a slice DST
/// - The `size` of an instance of `T` with `elems` trailing slice elements is
///   equal to `slice_layout.offset + slice_layout.elem_size * elems` rounded up
///   to the nearest multiple of `layout.align`. Any bytes in the range
///   `[slice_layout.offset + slice_layout.elem_size * elems, size)` are padding
///   and must not be assumed to be initialized.
///
/// [the reference]: https://doc.rust-lang.org/reference/type-layout.html
#[doc(hidden)]
#[derive(Debug)]
#[cfg_attr(any(kani, test), derive(Copy, Clone, PartialEq, Eq))]
pub struct DstLayout {
    align: NonZeroUsize,
    size_info: SizeInfo,
}

#[derive(Debug)]
#[cfg_attr(any(kani, test), derive(Copy, Clone, PartialEq, Eq))]
enum SizeInfo<E = usize> {
    Sized { _size: usize },
    SliceDst(TrailingSliceLayout<E>),
}

#[derive(Debug)]
#[cfg_attr(any(kani, test), derive(Copy, Clone, PartialEq, Eq))]
struct TrailingSliceLayout<E = usize> {
    // The offset of the first byte of the trailing slice field. Note that this
    // is NOT the same as the minimum size of the type. For example, consider
    // the following type:
    //
    //   struct Foo {
    //       a: u16,
    //       b: u8,
    //       c: [u8],
    //   }
    //
    // In `Foo`, `c` is at byte offset 3. When `c.len() == 0`, `c` is followed
    // by a padding byte.
    _offset: usize,
    // The size of the element type of the trailing slice field.
    _elem_size: E,
}

impl SizeInfo {
    /// Attempts to create a `SizeInfo` from `Self` in which `elem_size` is a
    /// `NonZeroUsize`. If `elem_size` is 0, returns `None`.

    const fn try_to_nonzero_elem_size(&self) -> Option<SizeInfo<NonZeroUsize>> {
        Some(match *self {
            SizeInfo::Sized { _size } => SizeInfo::Sized { _size },
            SizeInfo::SliceDst(TrailingSliceLayout {
                _offset,
                _elem_size,
            }) => {
                if let Some(_elem_size) = NonZeroUsize::new(_elem_size) {
                    SizeInfo::SliceDst(TrailingSliceLayout {
                        _offset,
                        _elem_size,
                    })
                } else {
                    return None;
                }
            }
        })
    }
}

#[doc(hidden)]
#[derive(Copy, Clone, Debug)]

pub enum _CastType {
    _Prefix,
    _Suffix,
}

impl DstLayout {
    /// The minimum possible alignment of a type.
    const MIN_ALIGN: NonZeroUsize = match NonZeroUsize::new(1) {
        Some(min_align) => min_align,
        None => unreachable!(),
    };

    /// The maximum theoretic possible alignment of a type.
    ///
    /// For compatibility with future Rust versions, this is defined as the
    /// maximum power-of-two that fits into a `usize`. See also
    /// [`DstLayout::CURRENT_MAX_ALIGN`].
    const THEORETICAL_MAX_ALIGN: NonZeroUsize =
        match NonZeroUsize::new(1 << (POINTER_WIDTH_BITS - 1)) {
            Some(max_align) => max_align,
            None => unreachable!(),
        };

    /// The current, documented max alignment of a type \[1\].
    ///
    /// \[1\] Per <https://doc.rust-lang.org/reference/type-layout.html#the-alignment-modifiers>:
    ///
    ///   The alignment value must be a power of two from 1 up to
    ///   2<sup>29</sup>.
    #[cfg(not(kani))]
    const CURRENT_MAX_ALIGN: NonZeroUsize = match NonZeroUsize::new(1 << 28) {
        Some(max_align) => max_align,
        None => unreachable!(),
    };

    /// Constructs a `DstLayout` for a zero-sized type with `repr_align`
    /// alignment (or 1). If `repr_align` is provided, then it must be a power
    /// of two.
    ///
    /// # Panics
    ///
    /// This function panics if the supplied `repr_align` is not a power of two.
    ///
    /// # Safety
    ///
    /// Unsafe code may assume that the contract of this function is satisfied.
    #[doc(hidden)]
    #[inline]
    pub const fn new_zst(repr_align: Option<NonZeroUsize>) -> DstLayout {
        let align = match repr_align {
            Some(align) => align,
            None => Self::MIN_ALIGN,
        };

        assert!(align.is_power_of_two());

        DstLayout {
            align,
            size_info: SizeInfo::Sized { _size: 0 },
        }
    }

    /// Constructs a `DstLayout` which describes `T`.
    ///
    /// # Safety
    ///
    /// Unsafe code may assume that `DstLayout` is the correct layout for `T`.
    #[doc(hidden)]
    #[inline]
    pub const fn for_type<T>() -> DstLayout {
        // SAFETY: `align` is correct by construction. `T: Sized`, and so it is
        // sound to initialize `size_info` to `SizeInfo::Sized { size }`; the
        // `size` field is also correct by construction.
        DstLayout {
            align: match NonZeroUsize::new(mem::align_of::<T>()) {
                Some(align) => align,
                None => unreachable!(),
            },
            size_info: SizeInfo::Sized {
                _size: mem::size_of::<T>(),
            },
        }
    }

    /// Constructs a `DstLayout` which describes `[T]`.
    ///
    /// # Safety
    ///
    /// Unsafe code may assume that `DstLayout` is the correct layout for `[T]`.
    const fn for_slice<T>() -> DstLayout {
        // SAFETY: The alignment of a slice is equal to the alignment of its
        // element type, and so `align` is initialized correctly.
        //
        // Since this is just a slice type, there is no offset between the
        // beginning of the type and the beginning of the slice, so it is
        // correct to set `offset: 0`. The `elem_size` is correct by
        // construction. Since `[T]` is a (degenerate case of a) slice DST, it
        // is correct to initialize `size_info` to `SizeInfo::SliceDst`.
        DstLayout {
            align: match NonZeroUsize::new(mem::align_of::<T>()) {
                Some(align) => align,
                None => unreachable!(),
            },
            size_info: SizeInfo::SliceDst(TrailingSliceLayout {
                _offset: 0,
                _elem_size: mem::size_of::<T>(),
            }),
        }
    }

    /// Like `Layout::extend`, this creates a layout that describes a record
    /// whose layout consists of `self` followed by `next` that includes the
    /// necessary inter-field padding, but not any trailing padding.
    ///
    /// In order to match the layout of a `#[repr(C)]` struct, this method
    /// should be invoked for each field in declaration order. To add trailing
    /// padding, call `DstLayout::pad_to_align` after extending the layout for
    /// all fields. If `self` corresponds to a type marked with
    /// `repr(packed(N))`, then `repr_packed` should be set to `Some(N)`,
    /// otherwise `None`.
    ///
    /// This method cannot be used to match the layout of a record with the
    /// default representation, as that representation is mostly unspecified.
    ///
    /// # Safety
    ///
    /// If a (potentially hypothetical) valid `repr(C)` Rust type begins with
    /// fields whose layout are `self`, and those fields are immediately
    /// followed by a field whose layout is `field`, then unsafe code may rely
    /// on `self.extend(field, repr_packed)` producing a layout that correctly
    /// encompasses those two components.
    ///
    /// We make no guarantees to the behavior of this method if these fragments
    /// cannot appear in a valid Rust type (e.g., the concatenation of the
    /// layouts would lead to a size larger than `isize::MAX`).
    #[doc(hidden)]
    #[inline]
    pub const fn extend(self, field: DstLayout, repr_packed: Option<NonZeroUsize>) -> Self {
        use util::{core_layout::padding_needed_for, max, min};

        // If `repr_packed` is `None`, there are no alignment constraints, and
        // the value can be defaulted to `THEORETICAL_MAX_ALIGN`.
        let max_align = match repr_packed {
            Some(max_align) => max_align,
            None => Self::THEORETICAL_MAX_ALIGN,
        };

        assert!(max_align.is_power_of_two());

        // We use Kani to prove that this method is robust to future increases
        // in Rust's maximum allowed alignment. However, if such a change ever
        // actually occurs, we'd like to be notified via assertion failures.
        #[cfg(not(kani))]
        {
            debug_assert!(self.align.get() <= DstLayout::CURRENT_MAX_ALIGN.get());
            debug_assert!(field.align.get() <= DstLayout::CURRENT_MAX_ALIGN.get());
            if let Some(repr_packed) = repr_packed {
                debug_assert!(repr_packed.get() <= DstLayout::CURRENT_MAX_ALIGN.get());
            }
        }

        // The field's alignment is clamped by `repr_packed` (i.e., the
        // `repr(packed(N))` attribute, if any) [1].
        //
        // [1] Per https://doc.rust-lang.org/reference/type-layout.html#the-alignment-modifiers:
        //
        //   The alignments of each field, for the purpose of positioning
        //   fields, is the smaller of the specified alignment and the alignment
        //   of the field's type.
        let field_align = min(field.align, max_align);

        // The struct's alignment is the maximum of its previous alignment and
        // `field_align`.
        let align = max(self.align, field_align);

        let size_info = match self.size_info {
            // If the layout is already a DST, we panic; DSTs cannot be extended
            // with additional fields.
            SizeInfo::SliceDst(..) => panic!("Cannot extend a DST with additional fields."),

            SizeInfo::Sized {
                _size: preceding_size,
            } => {
                // Compute the minimum amount of inter-field padding needed to
                // satisfy the field's alignment, and offset of the trailing
                // field. [1]
                //
                // [1] Per https://doc.rust-lang.org/reference/type-layout.html#the-alignment-modifiers:
                //
                //   Inter-field padding is guaranteed to be the minimum
                //   required in order to satisfy each field's (possibly
                //   altered) alignment.
                let padding = padding_needed_for(preceding_size, field_align);

                // This will not panic (and is proven to not panic, with Kani)
                // if the layout components can correspond to a leading layout
                // fragment of a valid Rust type, but may panic otherwise (e.g.,
                // combining or aligning the components would create a size
                // exceeding `isize::MAX`).
                let offset = match preceding_size.checked_add(padding) {
                    Some(offset) => offset,
                    None => panic!("Adding padding to `self`'s size overflows `usize`."),
                };

                match field.size_info {
                    SizeInfo::Sized { _size: field_size } => {
                        // If the trailing field is sized, the resulting layout
                        // will be sized. Its size will be the sum of the
                        // preceeding layout, the size of the new field, and the
                        // size of inter-field padding between the two.
                        //
                        // This will not panic (and is proven with Kani to not
                        // panic) if the layout components can correspond to a
                        // leading layout fragment of a valid Rust type, but may
                        // panic otherwise (e.g., combining or aligning the
                        // components would create a size exceeding
                        // `usize::MAX`).
                        let size = match offset.checked_add(field_size) {
                            Some(size) => size,
                            None => panic!("`field` cannot be appended without the total size overflowing `usize`"),
                        };
                        SizeInfo::Sized { _size: size }
                    }
                    SizeInfo::SliceDst(TrailingSliceLayout {
                        _offset: trailing_offset,
                        _elem_size,
                    }) => {
                        // If the trailing field is dynamically sized, so too
                        // will the resulting layout. The offset of the trailing
                        // slice component is the sum of the offset of the
                        // trailing field and the trailing slice offset within
                        // that field.
                        //
                        // This will not panic (and is proven with Kani to not
                        // panic) if the layout components can correspond to a
                        // leading layout fragment of a valid Rust type, but may
                        // panic otherwise (e.g., combining or aligning the
                        // components would create a size exceeding
                        // `usize::MAX`).
                        let offset = match offset.checked_add(trailing_offset) {
                            Some(offset) => offset,
                            None => panic!("`field` cannot be appended without the total size overflowing `usize`"),
                        };
                        SizeInfo::SliceDst(TrailingSliceLayout {
                            _offset: offset,
                            _elem_size,
                        })
                    }
                }
            }
        };

        DstLayout { align, size_info }
    }

    /// Like `Layout::pad_to_align`, this routine rounds the size of this layout
    /// up to the nearest multiple of this type's alignment or `repr_packed`
    /// (whichever is less). This method leaves DST layouts unchanged, since the
    /// trailing padding of DSTs is computed at runtime.
    ///
    /// In order to match the layout of a `#[repr(C)]` struct, this method
    /// should be invoked after the invocations of [`DstLayout::extend`]. If
    /// `self` corresponds to a type marked with `repr(packed(N))`, then
    /// `repr_packed` should be set to `Some(N)`, otherwise `None`.
    ///
    /// This method cannot be used to match the layout of a record with the
    /// default representation, as that representation is mostly unspecified.
    ///
    /// # Safety
    ///
    /// If a (potentially hypothetical) valid `repr(C)` type begins with fields
    /// whose layout are `self` followed only by zero or more bytes of trailing
    /// padding (not included in `self`), then unsafe code may rely on
    /// `self.pad_to_align(repr_packed)` producing a layout that correctly
    /// encapsulates the layout of that type.
    ///
    /// We make no guarantees to the behavior of this method if `self` cannot
    /// appear in a valid Rust type (e.g., because the addition of trailing
    /// padding would lead to a size larger than `isize::MAX`).
    #[doc(hidden)]
    #[inline]
    pub const fn pad_to_align(self) -> Self {
        use util::core_layout::padding_needed_for;

        let size_info = match self.size_info {
            // For sized layouts, we add the minimum amount of trailing padding
            // needed to satisfy alignment.
            SizeInfo::Sized {
                _size: unpadded_size,
            } => {
                let padding = padding_needed_for(unpadded_size, self.align);
                let size = match unpadded_size.checked_add(padding) {
                    Some(size) => size,
                    None => panic!("Adding padding caused size to overflow `usize`."),
                };
                SizeInfo::Sized { _size: size }
            }
            // For DST layouts, trailing padding depends on the length of the
            // trailing DST and is computed at runtime. This does not alter the
            // offset or element size of the layout, so we leave `size_info`
            // unchanged.
            size_info @ SizeInfo::SliceDst(_) => size_info,
        };

        DstLayout {
            align: self.align,
            size_info,
        }
    }

    /// Validates that a cast is sound from a layout perspective.
    ///
    /// Validates that the size and alignment requirements of a type with the
    /// layout described in `self` would not be violated by performing a
    /// `cast_type` cast from a pointer with address `addr` which refers to a
    /// memory region of size `bytes_len`.
    ///
    /// If the cast is valid, `validate_cast_and_convert_metadata` returns
    /// `(elems, split_at)`. If `self` describes a dynamically-sized type, then
    /// `elems` is the maximum number of trailing slice elements for which a
    /// cast would be valid (for sized types, `elem` is meaningless and should
    /// be ignored). `split_at` is the index at which to split the memory region
    /// in order for the prefix (suffix) to contain the result of the cast, and
    /// in order for the remaining suffix (prefix) to contain the leftover
    /// bytes.
    ///
    /// There are three conditions under which a cast can fail:
    /// - The smallest possible value for the type is larger than the provided
    ///   memory region
    /// - A prefix cast is requested, and `addr` does not satisfy `self`'s
    ///   alignment requirement
    /// - A suffix cast is requested, and `addr + bytes_len` does not satisfy
    ///   `self`'s alignment requirement (as a consequence, since all instances
    ///   of the type are a multiple of its alignment, no size for the type will
    ///   result in a starting address which is properly aligned)
    ///
    /// # Safety
    ///
    /// The caller may assume that this implementation is correct, and may rely
    /// on that assumption for the soundness of their code. In particular, the
    /// caller may assume that, if `validate_cast_and_convert_metadata` returns
    /// `Some((elems, split_at))`, then:
    /// - A pointer to the type (for dynamically sized types, this includes
    ///   `elems` as its pointer metadata) describes an object of size `size <=
    ///   bytes_len`
    /// - If this is a prefix cast:
    ///   - `addr` satisfies `self`'s alignment
    ///   - `size == split_at`
    /// - If this is a suffix cast:
    ///   - `split_at == bytes_len - size`
    ///   - `addr + split_at` satisfies `self`'s alignment
    ///
    /// Note that this method does *not* ensure that a pointer constructed from
    /// its return values will be a valid pointer. In particular, this method
    /// does not reason about `isize` overflow, which is a requirement of many
    /// Rust pointer APIs, and may at some point be determined to be a validity
    /// invariant of pointer types themselves. This should never be a problem so
    /// long as the arguments to this method are derived from a known-valid
    /// pointer (e.g., one derived from a safe Rust reference), but it is
    /// nonetheless the caller's responsibility to justify that pointer
    /// arithmetic will not overflow based on a safety argument *other than* the
    /// mere fact that this method returned successfully.
    ///
    /// # Panics
    ///
    /// `validate_cast_and_convert_metadata` will panic if `self` describes a
    /// DST whose trailing slice element is zero-sized.
    ///
    /// If `addr + bytes_len` overflows `usize`,
    /// `validate_cast_and_convert_metadata` may panic, or it may return
    /// incorrect results. No guarantees are made about when
    /// `validate_cast_and_convert_metadata` will panic. The caller should not
    /// rely on `validate_cast_and_convert_metadata` panicking in any particular
    /// condition, even if `debug_assertions` are enabled.

    const fn validate_cast_and_convert_metadata(
        &self,
        addr: usize,
        bytes_len: usize,
        cast_type: _CastType,
    ) -> Option<(usize, usize)> {
        // `debug_assert!`, but with `#[allow(clippy::arithmetic_side_effects)]`.
        macro_rules! __debug_assert {
            ($e:expr $(, $msg:expr)?) => {
                debug_assert!({

                    let e = $e;
                    e
                } $(, $msg)?);
            };
        }

        // Note that, in practice, `self` is always a compile-time constant. We
        // do this check earlier than needed to ensure that we always panic as a
        // result of bugs in the program (such as calling this function on an
        // invalid type) instead of allowing this panic to be hidden if the cast
        // would have failed anyway for runtime reasons (such as a too-small
        // memory region).
        //
        // TODO(#67): Once our MSRV is 1.65, use let-else:
        // https://blog.rust-lang.org/2022/11/03/Rust-1.65.0.html#let-else-statements
        let size_info = match self.size_info.try_to_nonzero_elem_size() {
            Some(size_info) => size_info,
            None => panic!("attempted to cast to slice type with zero-sized element"),
        };

        // Precondition
        __debug_assert!(
            addr.checked_add(bytes_len).is_some(),
            "`addr` + `bytes_len` > usize::MAX"
        );

        // Alignment checks go in their own block to avoid introducing variables
        // into the top-level scope.
        {
            // We check alignment for `addr` (for prefix casts) or `addr +
            // bytes_len` (for suffix casts). For a prefix cast, the correctness
            // of this check is trivial - `addr` is the address the object will
            // live at.
            //
            // For a suffix cast, we know that all valid sizes for the type are
            // a multiple of the alignment (and by safety precondition, we know
            // `DstLayout` may only describe valid Rust types). Thus, a
            // validly-sized instance which lives at a validly-aligned address
            // must also end at a validly-aligned address. Thus, if the end
            // address for a suffix cast (`addr + bytes_len`) is not aligned,
            // then no valid start address will be aligned either.
            let offset = match cast_type {
                _CastType::_Prefix => 0,
                _CastType::_Suffix => bytes_len,
            };

            // Addition is guaranteed not to overflow because `offset <=
            // bytes_len`, and `addr + bytes_len <= usize::MAX` is a
            // precondition of this method. Modulus is guaranteed not to divide
            // by 0 because `align` is non-zero.

            if (addr + offset) % self.align.get() != 0 {
                return None;
            }
        }

        let (elems, self_bytes) = match size_info {
            SizeInfo::Sized { _size: size } => {
                if size > bytes_len {
                    return None;
                }
                (0, size)
            }
            SizeInfo::SliceDst(TrailingSliceLayout {
                _offset: offset,
                _elem_size: elem_size,
            }) => {
                // Calculate the maximum number of bytes that could be consumed
                // - any number of bytes larger than this will either not be a
                // multiple of the alignment, or will be larger than
                // `bytes_len`.
                let max_total_bytes =
                    util::round_down_to_next_multiple_of_alignment(bytes_len, self.align);
                // Calculate the maximum number of bytes that could be consumed
                // by the trailing slice.
                //
                // TODO(#67): Once our MSRV is 1.65, use let-else:
                // https://blog.rust-lang.org/2022/11/03/Rust-1.65.0.html#let-else-statements
                let max_slice_and_padding_bytes = match max_total_bytes.checked_sub(offset) {
                    Some(max) => max,
                    // `bytes_len` too small even for 0 trailing slice elements.
                    None => return None,
                };

                // Calculate the number of elements that fit in
                // `max_slice_and_padding_bytes`; any remaining bytes will be
                // considered padding.
                //
                // Guaranteed not to divide by zero: `elem_size` is non-zero.

                let elems = max_slice_and_padding_bytes / elem_size.get();
                // Guaranteed not to overflow on multiplication: `usize::MAX >=
                // max_slice_and_padding_bytes >= (max_slice_and_padding_bytes /
                // elem_size) * elem_size`.
                //
                // Guaranteed not to overflow on addition:
                // - max_slice_and_padding_bytes == max_total_bytes - offset
                // - elems * elem_size <= max_slice_and_padding_bytes == max_total_bytes - offset
                // - elems * elem_size + offset <= max_total_bytes <= usize::MAX

                let without_padding = offset + elems * elem_size.get();
                // `self_bytes` is equal to the offset bytes plus the bytes
                // consumed by the trailing slice plus any padding bytes
                // required to satisfy the alignment. Note that we have computed
                // the maximum number of trailing slice elements that could fit
                // in `self_bytes`, so any padding is guaranteed to be less than
                // the size of an extra element.
                //
                // Guaranteed not to overflow:
                // - By previous comment: without_padding == elems * elem_size +
                //   offset <= max_total_bytes
                // - By construction, `max_total_bytes` is a multiple of
                //   `self.align`.
                // - At most, adding padding needed to round `without_padding`
                //   up to the next multiple of the alignment will bring
                //   `self_bytes` up to `max_total_bytes`.

                let self_bytes = without_padding
                    + util::core_layout::padding_needed_for(without_padding, self.align);
                (elems, self_bytes)
            }
        };

        __debug_assert!(self_bytes <= bytes_len);

        let split_at = match cast_type {
            _CastType::_Prefix => self_bytes,
            // Guaranteed not to underflow:
            // - In the `Sized` branch, only returns `size` if `size <=
            //   bytes_len`.
            // - In the `SliceDst` branch, calculates `self_bytes <=
            //   max_toatl_bytes`, which is upper-bounded by `bytes_len`.
            _CastType::_Suffix => bytes_len - self_bytes,
        };

        Some((elems, split_at))
    }
}

/// A trait which carries information about a type's layout that is used by the
/// internals of this crate.
///
/// This trait is not meant for consumption by code outside of this crate. While
/// the normal semver stability guarantees apply with respect to which types
/// implement this trait and which trait implementations are implied by this
/// trait, no semver stability guarantees are made regarding its internals; they
/// may change at any time, and code which makes use of them may break.
///
/// # Safety
///
/// This trait does not convey any safety guarantees to code outside this crate.
#[doc(hidden)] // TODO: Remove this once KnownLayout is used by other APIs
pub unsafe trait KnownLayout {
    // The `Self: Sized` bound makes it so that `KnownLayout` can still be
    // object safe. It's not currently object safe thanks to `const LAYOUT`, and
    // it likely won't be in the future, but there's no reason not to be
    // forwards-compatible with object safety.
    #[doc(hidden)]
    fn only_derive_is_allowed_to_implement_this_trait()
    where
        Self: Sized;

    #[doc(hidden)]
    const LAYOUT: DstLayout;

    /// SAFETY: The returned pointer has the same address and provenance as
    /// `bytes`. If `Self` is a DST, the returned pointer's referent has `elems`
    /// elements in its trailing slice. If `Self` is sized, `elems` is ignored.
    #[doc(hidden)]
    fn raw_from_ptr_len(bytes: NonNull<u8>, elems: usize) -> NonNull<Self>;
}

// SAFETY: Delegates safety to `DstLayout::for_slice`.
unsafe impl<T: KnownLayout> KnownLayout for [T] {
    fn only_derive_is_allowed_to_implement_this_trait()
    where
        Self: Sized,
    {
    }
    const LAYOUT: DstLayout = DstLayout::for_slice::<T>();

    // SAFETY: `.cast` preserves address and provenance. The returned pointer
    // refers to an object with `elems` elements by construction.
    #[inline(always)]
    fn raw_from_ptr_len(data: NonNull<u8>, elems: usize) -> NonNull<Self> {
        // TODO(#67): Remove this allow. See NonNullExt for more details.

        NonNull::slice_from_raw_parts(data.cast::<T>(), elems)
    }
}

#[rustfmt::skip]
impl_known_layout!(
    (),
    u8, i8, u16, i16, u32, i32, u64, i64, u128, i128, usize, isize, f32, f64,
    bool, char,
    NonZeroU8, NonZeroI8, NonZeroU16, NonZeroI16, NonZeroU32, NonZeroI32,
    NonZeroU64, NonZeroI64, NonZeroU128, NonZeroI128, NonZeroUsize, NonZeroIsize
);
#[rustfmt::skip]
impl_known_layout!(
    T         => Option<T>,
    T: ?Sized => PhantomData<T>,
    T         => Wrapping<T>,
    T         => MaybeUninit<T>,
    T: ?Sized => *const T,
    T: ?Sized => *mut T,
);
impl_known_layout!(const N: usize, T => [T; N]);

safety_comment! {
    /// SAFETY:
    /// `str` and `ManuallyDrop<[T]>` [1] have the same representations as
    /// `[u8]` and `[T]` repsectively. `str` has different bit validity than
    /// `[u8]`, but that doesn't affect the soundness of this impl.
    ///
    /// [1] Per https://doc.rust-lang.org/nightly/core/mem/struct.ManuallyDrop.html:
    ///
    ///   `ManuallyDrop<T>` is guaranteed to have the same layout and bit
    ///   validity as `T`
    ///
    /// TODO(#429):
    /// -  Add quotes from docs.
    /// -  Once [1] (added in
    /// https://github.com/rust-lang/rust/pull/115522) is available on stable,
    /// quote the stable docs instead of the nightly docs.
    unsafe_impl_known_layout!(#[repr([u8])] str);
    unsafe_impl_known_layout!(T: ?Sized + KnownLayout => #[repr(T)] ManuallyDrop<T>);
}

/// Analyzes whether a type is [`FromZeroes`].
///
/// This derive analyzes, at compile time, whether the annotated type satisfies
/// the [safety conditions] of `FromZeroes` and implements `FromZeroes` if it is
/// sound to do so. This derive can be applied to structs, enums, and unions;
/// e.g.:
///
/// ```
/// # use zerocopy_derive::FromZeroes;
/// #[derive(FromZeroes)]
/// struct MyStruct {
/// # /*
///     ...
/// # */
/// }
///
/// #[derive(FromZeroes)]
/// #[repr(u8)]
/// enum MyEnum {
/// #   Variant0,
/// # /*
///     ...
/// # */
/// }
///
/// #[derive(FromZeroes)]
/// union MyUnion {
/// #   variant: u8,
/// # /*
///     ...
/// # */
/// }
/// ```
///
/// [safety conditions]: trait@FromZeroes#safety
///
/// # Analysis
///
/// *This section describes, roughly, the analysis performed by this derive to
/// determine whether it is sound to implement `FromZeroes` for a given type.
/// Unless you are modifying the implementation of this derive, or attempting to
/// manually implement `FromZeroes` for a type yourself, you don't need to read
/// this section.*
///
/// If a type has the following properties, then this derive can implement
/// `FromZeroes` for that type:
///
/// - If the type is a struct, all of its fields must be `FromZeroes`.
/// - If the type is an enum, it must be C-like (meaning that all variants have
///   no fields) and it must have a variant with a discriminant of `0`. See [the
///   reference] for a description of how discriminant values are chosen.
/// - The type must not contain any [`UnsafeCell`]s (this is required in order
///   for it to be sound to construct a `&[u8]` and a `&T` to the same region of
///   memory). The type may contain references or pointers to `UnsafeCell`s so
///   long as those values can themselves be initialized from zeroes
///   (`FromZeroes` is not currently implemented for, e.g.,
///   `Option<&UnsafeCell<_>>`, but it could be one day).
///
/// This analysis is subject to change. Unsafe code may *only* rely on the
/// documented [safety conditions] of `FromZeroes`, and must *not* rely on the
/// implementation details of this derive.
///
/// [the reference]: https://doc.rust-lang.org/reference/items/enumerations.html#custom-discriminant-values-for-fieldless-enumerations
/// [`UnsafeCell`]: core::cell::UnsafeCell
///
/// ## Why isn't an explicit representation required for structs?
///
/// Neither this derive, nor the [safety conditions] of `FromZeroes`, requires
/// that structs are marked with `#[repr(C)]`.
///
/// Per the [Rust reference](reference),
///
/// > The representation of a type can change the padding between fields, but
/// > does not change the layout of the fields themselves.
///
/// [reference]: https://doc.rust-lang.org/reference/type-layout.html#representations
///
/// Since the layout of structs only consists of padding bytes and field bytes,
/// a struct is soundly `FromZeroes` if:
/// 1. its padding is soundly `FromZeroes`, and
/// 2. its fields are soundly `FromZeroes`.
///
/// The answer to the first question is always yes: padding bytes do not have
/// any validity constraints. A [discussion] of this question in the Unsafe Code
/// Guidelines Working Group concluded that it would be virtually unimaginable
/// for future versions of rustc to add validity constraints to padding bytes.
///
/// [discussion]: https://github.com/rust-lang/unsafe-code-guidelines/issues/174
///
/// Whether a struct is soundly `FromZeroes` therefore solely depends on whether
/// its fields are `FromZeroes`.
// TODO(#146): Document why we don't require an enum to have an explicit `repr`
// attribute.
#[cfg(any(feature = "derive", test))]
#[cfg_attr(doc_cfg, doc(cfg(feature = "derive")))]
pub use zerocopy_derive::FromZeroes;

/// Types whose validity can be checked at runtime, allowing them to be
/// conditionally converted from byte slices.
///
/// WARNING: Do not implement this trait yourself! Instead, use
/// `#[derive(TryFromBytes)]`.
///
/// `TryFromBytes` types can safely be deserialized from an untrusted sequence
/// of bytes by performing a runtime check that the byte sequence contains a
/// valid instance of `Self`.
///
/// `TryFromBytes` is ignorant of byte order. For byte order-aware types, see
/// the [`byteorder`] module.
///
/// # What is a "valid instance"?
///
/// In Rust, each type has *bit validity*, which refers to the set of bit
/// patterns which may appear in an instance of that type. It is impossible for
/// safe Rust code to produce values which violate bit validity (ie, values
/// outside of the "valid" set of bit patterns). If `unsafe` code produces an
/// invalid value, this is considered [undefined behavior].
///
/// Rust's bit validity rules are currently being decided, which means that some
/// types have three classes of bit patterns: those which are definitely valid,
/// and whose validity is documented in the language; those which may or may not
/// be considered valid at some point in the future; and those which are
/// definitely invalid.
///
/// Zerocopy takes a conservative approach, and only considers a bit pattern to
/// be valid if its validity is a documenteed guarantee provided by the
/// language.
///
/// For most use cases, Rust's current guarantees align with programmers'
/// intuitions about what ought to be valid. As a result, zerocopy's
/// conservatism should not affect most users. One notable exception is unions,
/// whose bit validity is very up in the air; zerocopy does not permit
/// implementing `TryFromBytes` for any union type.
///
/// If you are negatively affected by lack of support for a particular type,
/// we encourage you to let us know by [filing an issue][github-repo].
///
/// # Safety
///
/// On its own, `T: TryFromBytes` does not make any guarantees about the layout
/// or representation of `T`. It merely provides the ability to perform a
/// validity check at runtime via methods like [`try_from_ref`].
///
/// Currently, it is not possible to stably implement `TryFromBytes` other than
/// by using `#[derive(TryFromBytes)]`. While there are `#[doc(hidden)]` items
/// on this trait that provide well-defined safety invariants, no stability
/// guarantees are made with respect to these items. In particular, future
/// releases of zerocopy may make backwards-breaking changes to these items,
/// including changes that only affect soundness, which may cause code which
/// uses those items to silently become unsound.
///
/// [undefined behavior]: https://raphlinus.github.io/programming/rust/2018/08/17/undefined-behavior.html
/// [github-repo]: https://github.com/google/zerocopy
/// [`try_from_ref`]: TryFromBytes::try_from_ref
// TODO(#5): Update `try_from_ref` doc link once it exists
#[doc(hidden)]
pub unsafe trait TryFromBytes {
    /// Does a given memory range contain a valid instance of `Self`?
    ///
    /// # Safety
    ///
    /// ## Preconditions
    ///
    /// The memory referenced by `candidate` may only be accessed via reads for
    /// the duration of this method call. This prohibits writes through mutable
    /// references and through [`UnsafeCell`]s. There may exist immutable
    /// references to the same memory which contain `UnsafeCell`s so long as:
    /// - Those `UnsafeCell`s exist at the same byte ranges as `UnsafeCell`s in
    ///   `Self`. This is a bidirectional property: `Self` may not contain
    ///   `UnsafeCell`s where other references to the same memory do not, and
    ///   vice-versa.
    /// - Those `UnsafeCell`s are never used to perform mutation for the
    ///   duration of this method call.
    ///
    /// The memory referenced by `candidate` may not be referenced by any
    /// mutable references even if these references are not used to perform
    /// mutation.
    ///
    /// `candidate` is not required to refer to a valid `Self`. However, it must
    /// satisfy the requirement that uninitialized bytes may only be present
    /// where it is possible for them to be present in `Self`. This is a dynamic
    /// property: if, at a particular byte offset, a valid enum discriminant is
    /// set, the subsequent bytes may only have uninitialized bytes as
    /// specificed by the corresponding enum.
    ///
    /// Formally, given `len = size_of_val_raw(candidate)`, at every byte
    /// offset, `b`, in the range `[0, len)`:
    /// - If, in all instances `s: Self` of length `len`, the byte at offset `b`
    ///   in `s` is initialized, then the byte at offset `b` within `*candidate`
    ///   must be initialized.
    /// - Let `c` be the contents of the byte range `[0, b)` in `*candidate`.
    ///   Let `S` be the subset of valid instances of `Self` of length `len`
    ///   which contain `c` in the offset range `[0, b)`. If, for all instances
    ///   of `s: Self` in `S`, the byte at offset `b` in `s` is initialized,
    ///   then the byte at offset `b` in `*candidate` must be initialized.
    ///
    ///   Pragmatically, this means that if `*candidate` is guaranteed to
    ///   contain an enum type at a particular offset, and the enum discriminant
    ///   stored in `*candidate` corresponds to a valid variant of that enum
    ///   type, then it is guaranteed that the appropriate bytes of `*candidate`
    ///   are initialized as defined by that variant's bit validity (although
    ///   note that the variant may contain another enum type, in which case the
    ///   same rules apply depending on the state of its discriminant, and so on
    ///   recursively).
    ///
    /// ## Postconditions
    ///
    /// Unsafe code may assume that, if `is_bit_valid(candidate)` returns true,
    /// `*candidate` contains a valid `Self`.
    ///
    /// # Panics
    ///
    /// `is_bit_valid` may panic. Callers are responsible for ensuring that any
    /// `unsafe` code remains sound even in the face of `is_bit_valid`
    /// panicking. (We support user-defined validation routines; so long as
    /// these routines are not required to be `unsafe`, there is no way to
    /// ensure that these do not generate panics.)
    ///
    /// [`UnsafeCell`]: core::cell::UnsafeCell
    #[doc(hidden)]
    unsafe fn is_bit_valid(candidate: Ptr<'_, Self>) -> bool;

    /// Attempts to interpret a byte slice as a `Self`.
    ///
    /// `try_from_ref` validates that `bytes` contains a valid `Self`, and that
    /// it satisfies `Self`'s alignment requirement. If it does, then `bytes` is
    /// reinterpreted as a `Self`.
    ///
    /// Note that Rust's bit validity rules are still being decided. As such,
    /// there exist types whose bit validity is ambiguous. See the
    /// `TryFromBytes` docs for a discussion of how these cases are handled.
    // TODO(#251): In a future in which we distinguish between `FromBytes` and
    // `RefFromBytes`, this requires `where Self: RefFromBytes` to disallow
    // interior mutability.
    #[inline]
    #[doc(hidden)] // TODO(#5): Finalize name before remove this attribute.
    fn try_from_ref(bytes: &[u8]) -> Option<&Self>
    where
        Self: KnownLayout,
    {
        let maybe_self = Ptr::from(bytes).try_cast_into_no_leftover::<Self>()?;

        // SAFETY:
        // - Since `bytes` is an immutable reference, we know that no mutable
        //   references exist to this memory region.
        // - Since `[u8]` contains no `UnsafeCell`s, we know there are no
        //   `&UnsafeCell` references to this memory region.
        // - Since we don't permit implementing `TryFromBytes` for types which
        //   contain `UnsafeCell`s, there are no `UnsafeCell`s in `Self`, and so
        //   the requirement that all references contain `UnsafeCell`s at the
        //   same offsets is trivially satisfied.
        // - All bytes of `bytes` are initialized.
        //
        // This call may panic. If that happens, it doesn't cause any soundness
        // issues, as we have not generated any invalid state which we need to
        // fix before returning.
        if unsafe { !Self::is_bit_valid(maybe_self) } {
            return None;
        }

        // SAFETY:
        // - Preconditions for `as_ref`:
        //   - `is_bit_valid` guarantees that `*maybe_self` contains a valid
        //     `Self`. Since `&[u8]` does not permit interior mutation, this
        //     cannot be invalidated after this method returns.
        //   - Since the argument and return types are immutable references,
        //     Rust will prevent the caller from producing any mutable
        //     references to the same memory region.
        //   - Since `Self` is not allowed to contain any `UnsafeCell`s and the
        //     same is true of `[u8]`, interior mutation is not possible. Thus,
        //     no mutation is possible. For the same reason, there is no
        //     mismatch between the two types in terms of which byte ranges are
        //     referenced as `UnsafeCell`s.
        // - Since interior mutation isn't possible within `Self`, there's no
        //   way for the returned reference to be used to modify the byte range,
        //   and thus there's no way for the returned reference to be used to
        //   write an invalid `[u8]` which would be observable via the original
        //   `&[u8]`.
        Some(unsafe { maybe_self.as_ref() })
    }
}

/// Types for which a sequence of bytes all set to zero represents a valid
/// instance of the type.
///
/// Any memory region of the appropriate length which is guaranteed to contain
/// only zero bytes can be viewed as any `FromZeroes` type with no runtime
/// overhead. This is useful whenever memory is known to be in a zeroed state,
/// such memory returned from some allocation routines.
///
/// # Implementation
///
/// **Do not implement this trait yourself!** Instead, use
/// [`#[derive(FromZeroes)]`][derive] (requires the `derive` Cargo feature);
/// e.g.:
///
/// ```
/// # use zerocopy_derive::FromZeroes;
/// #[derive(FromZeroes)]
/// struct MyStruct {
/// # /*
///     ...
/// # */
/// }
///
/// #[derive(FromZeroes)]
/// #[repr(u8)]
/// enum MyEnum {
/// #   Variant0,
/// # /*
///     ...
/// # */
/// }
///
/// #[derive(FromZeroes)]
/// union MyUnion {
/// #   variant: u8,
/// # /*
///     ...
/// # */
/// }
/// ```
///
/// This derive performs a sophisticated, compile-time safety analysis to
/// determine whether a type is `FromZeroes`.
///
/// # Safety
///
/// *This section describes what is required in order for `T: FromZeroes`, and
/// what unsafe code may assume of such types. If you don't plan on implementing
/// `FromZeroes` manually, and you don't plan on writing unsafe code that
/// operates on `FromZeroes` types, then you don't need to read this section.*
///
/// If `T: FromZeroes`, then unsafe code may assume that:
/// - It is sound to treat any initialized sequence of zero bytes of length
///   `size_of::<T>()` as a `T`.
/// - Given `b: &[u8]` where `b.len() == size_of::<T>()`, `b` is aligned to
///   `align_of::<T>()`, and `b` contains only zero bytes, it is sound to
///   construct a `t: &T` at the same address as `b`, and it is sound for both
///   `b` and `t` to be live at the same time.
///
/// If a type is marked as `FromZeroes` which violates this contract, it may
/// cause undefined behavior.
///
/// `#[derive(FromZeroes)]` only permits [types which satisfy these
/// requirements][derive-analysis].
///
#[cfg_attr(
    feature = "derive",
    doc = "[derive]: zerocopy_derive::FromZeroes",
    doc = "[derive-analysis]: zerocopy_derive::FromZeroes#analysis"
)]
#[cfg_attr(
    not(feature = "derive"),
    doc = concat!("[derive]: https://docs.rs/zerocopy/", env!("CARGO_PKG_VERSION"), "/zerocopy/derive.FromZeroes.html"),
    doc = concat!("[derive-analysis]: https://docs.rs/zerocopy/", env!("CARGO_PKG_VERSION"), "/zerocopy/derive.FromZeroes.html#analysis"),
)]
pub unsafe trait FromZeroes {
    // The `Self: Sized` bound makes it so that `FromZeroes` is still object
    // safe.
    #[doc(hidden)]
    fn only_derive_is_allowed_to_implement_this_trait()
    where
        Self: Sized;

    /// Overwrites `self` with zeroes.
    ///
    /// Sets every byte in `self` to 0. While this is similar to doing `*self =
    /// Self::new_zeroed()`, it differs in that `zero` does not semantically
    /// drop the current value and replace it with a new one - it simply
    /// modifies the bytes of the existing value.
    ///
    /// # Examples
    ///
    /// ```
    /// # use zerocopy::FromZeroes;
    /// # use zerocopy_derive::*;
    /// #
    /// #[derive(FromZeroes)]
    /// #[repr(C)]
    /// struct PacketHeader {
    ///     src_port: [u8; 2],
    ///     dst_port: [u8; 2],
    ///     length: [u8; 2],
    ///     checksum: [u8; 2],
    /// }
    ///
    /// let mut header = PacketHeader {
    ///     src_port: 100u16.to_be_bytes(),
    ///     dst_port: 200u16.to_be_bytes(),
    ///     length: 300u16.to_be_bytes(),
    ///     checksum: 400u16.to_be_bytes(),
    /// };
    ///
    /// header.zero();
    ///
    /// assert_eq!(header.src_port, [0, 0]);
    /// assert_eq!(header.dst_port, [0, 0]);
    /// assert_eq!(header.length, [0, 0]);
    /// assert_eq!(header.checksum, [0, 0]);
    /// ```
    #[inline(always)]
    fn zero(&mut self) {
        let slf: *mut Self = self;
        let len = mem::size_of_val(self);
        // SAFETY:
        // - `self` is guaranteed by the type system to be valid for writes of
        //   size `size_of_val(self)`.
        // - `u8`'s alignment is 1, and thus `self` is guaranteed to be aligned
        //   as required by `u8`.
        // - Since `Self: FromZeroes`, the all-zeroes instance is a valid
        //   instance of `Self.`
        //
        // TODO(#429): Add references to docs and quotes.
        unsafe { ptr::write_bytes(slf.cast::<u8>(), 0, len) };
    }

    /// Creates an instance of `Self` from zeroed bytes.
    ///
    /// # Examples
    ///
    /// ```
    /// # use zerocopy::FromZeroes;
    /// # use zerocopy_derive::*;
    /// #
    /// #[derive(FromZeroes)]
    /// #[repr(C)]
    /// struct PacketHeader {
    ///     src_port: [u8; 2],
    ///     dst_port: [u8; 2],
    ///     length: [u8; 2],
    ///     checksum: [u8; 2],
    /// }
    ///
    /// let header: PacketHeader = FromZeroes::new_zeroed();
    ///
    /// assert_eq!(header.src_port, [0, 0]);
    /// assert_eq!(header.dst_port, [0, 0]);
    /// assert_eq!(header.length, [0, 0]);
    /// assert_eq!(header.checksum, [0, 0]);
    /// ```
    #[inline(always)]
    fn new_zeroed() -> Self
    where
        Self: Sized,
    {
        // SAFETY: `FromZeroes` says that the all-zeroes bit pattern is legal.
        unsafe { mem::zeroed() }
    }

    /// Creates a `Box<Self>` from zeroed bytes.
    ///
    /// This function is useful for allocating large values on the heap and
    /// zero-initializing them, without ever creating a temporary instance of
    /// `Self` on the stack. For example, `<[u8; 1048576]>::new_box_zeroed()`
    /// will allocate `[u8; 1048576]` directly on the heap; it does not require
    /// storing `[u8; 1048576]` in a temporary variable on the stack.
    ///
    /// On systems that use a heap implementation that supports allocating from
    /// pre-zeroed memory, using `new_box_zeroed` (or related functions) may
    /// have performance benefits.
    ///
    /// Note that `Box<Self>` can be converted to `Arc<Self>` and other
    /// container types without reallocation.
    ///
    /// # Panics
    ///
    /// Panics if allocation of `size_of::<Self>()` bytes fails.
    #[cfg(feature = "alloc")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "alloc")))]
    #[inline]
    fn new_box_zeroed() -> Box<Self>
    where
        Self: Sized,
    {
        // If `T` is a ZST, then return a proper boxed instance of it. There is
        // no allocation, but `Box` does require a correct dangling pointer.
        let layout = Layout::new::<Self>();
        if layout.size() == 0 {
            return Box::new(Self::new_zeroed());
        }

        // TODO(#429): Add a "SAFETY" comment and remove this `allow`.

        let ptr = unsafe { alloc::alloc::alloc_zeroed(layout).cast::<Self>() };
        if ptr.is_null() {
            alloc::alloc::handle_alloc_error(layout);
        }
        // TODO(#429): Add a "SAFETY" comment and remove this `allow`.

        unsafe { Box::from_raw(ptr) }
    }

    /// Creates a `Box<[Self]>` (a boxed slice) from zeroed bytes.
    ///
    /// This function is useful for allocating large values of `[Self]` on the
    /// heap and zero-initializing them, without ever creating a temporary
    /// instance of `[Self; _]` on the stack. For example,
    /// `u8::new_box_slice_zeroed(1048576)` will allocate the slice directly on
    /// the heap; it does not require storing the slice on the stack.
    ///
    /// On systems that use a heap implementation that supports allocating from
    /// pre-zeroed memory, using `new_box_slice_zeroed` may have performance
    /// benefits.
    ///
    /// If `Self` is a zero-sized type, then this function will return a
    /// `Box<[Self]>` that has the correct `len`. Such a box cannot contain any
    /// actual information, but its `len()` property will report the correct
    /// value.
    ///
    /// # Panics
    ///
    /// * Panics if `size_of::<Self>() * len` overflows.
    /// * Panics if allocation of `size_of::<Self>() * len` bytes fails.
    #[cfg(feature = "alloc")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "alloc")))]
    #[inline]
    fn new_box_slice_zeroed(len: usize) -> Box<[Self]>
    where
        Self: Sized,
    {
        let size = mem::size_of::<Self>()
            .checked_mul(len)
            .expect("mem::size_of::<Self>() * len overflows `usize`");
        let align = mem::align_of::<Self>();
        // On stable Rust versions <= 1.64.0, `Layout::from_size_align` has a
        // bug in which sufficiently-large allocations (those which, when
        // rounded up to the alignment, overflow `isize`) are not rejected,
        // which can cause undefined behavior. See #64 for details.
        //
        // TODO(#67): Once our MSRV is > 1.64.0, remove this assertion.

        let max_alloc = (isize::MAX as usize).saturating_sub(align);
        assert!(size <= max_alloc);
        // TODO(https://github.com/rust-lang/rust/issues/55724): Use
        // `Layout::repeat` once it's stabilized.
        let layout =
            Layout::from_size_align(size, align).expect("total allocation size overflows `isize`");

        let ptr = if layout.size() != 0 {
            // TODO(#429): Add a "SAFETY" comment and remove this `allow`.

            let ptr = unsafe { alloc::alloc::alloc_zeroed(layout).cast::<Self>() };
            if ptr.is_null() {
                alloc::alloc::handle_alloc_error(layout);
            }
            ptr
        } else {
            // `Box<[T]>` does not allocate when `T` is zero-sized or when `len`
            // is zero, but it does require a non-null dangling pointer for its
            // allocation.
            NonNull::<Self>::dangling().as_ptr()
        };

        // TODO(#429): Add a "SAFETY" comment and remove this `allow`.

        unsafe { Box::from_raw(slice::from_raw_parts_mut(ptr, len)) }
    }

    /// Creates a `Vec<Self>` from zeroed bytes.
    ///
    /// This function is useful for allocating large values of `Vec`s and
    /// zero-initializing them, without ever creating a temporary instance of
    /// `[Self; _]` (or many temporary instances of `Self`) on the stack. For
    /// example, `u8::new_vec_zeroed(1048576)` will allocate directly on the
    /// heap; it does not require storing intermediate values on the stack.
    ///
    /// On systems that use a heap implementation that supports allocating from
    /// pre-zeroed memory, using `new_vec_zeroed` may have performance benefits.
    ///
    /// If `Self` is a zero-sized type, then this function will return a
    /// `Vec<Self>` that has the correct `len`. Such a `Vec` cannot contain any
    /// actual information, but its `len()` property will report the correct
    /// value.
    ///
    /// # Panics
    ///
    /// * Panics if `size_of::<Self>() * len` overflows.
    /// * Panics if allocation of `size_of::<Self>() * len` bytes fails.
    #[cfg(feature = "alloc")]
    #[cfg_attr(doc_cfg, doc(cfg(feature = "new_vec_zeroed")))]
    #[inline(always)]
    fn new_vec_zeroed(len: usize) -> Vec<Self>
    where
        Self: Sized,
    {
        Self::new_box_slice_zeroed(len).into()
    }
}

/// Analyzes whether a type is [`FromBytes`].
///
/// This derive analyzes, at compile time, whether the annotated type satisfies
/// the [safety conditions] of `FromBytes` and implements `FromBytes` if it is
/// sound to do so. This derive can be applied to structs, enums, and unions;
/// e.g.:
///
/// ```
/// # use zerocopy_derive::{FromBytes, FromZeroes};
/// #[derive(FromZeroes, FromBytes)]
/// struct MyStruct {
/// # /*
///     ...
/// # */
/// }
///
/// #[derive(FromZeroes, FromBytes)]
/// #[repr(u8)]
/// enum MyEnum {
/// #   V00, V01, V02, V03, V04, V05, V06, V07, V08, V09, V0A, V0B, V0C, V0D, V0E,
/// #   V0F, V10, V11, V12, V13, V14, V15, V16, V17, V18, V19, V1A, V1B, V1C, V1D,
/// #   V1E, V1F, V20, V21, V22, V23, V24, V25, V26, V27, V28, V29, V2A, V2B, V2C,
/// #   V2D, V2E, V2F, V30, V31, V32, V33, V34, V35, V36, V37, V38, V39, V3A, V3B,
/// #   V3C, V3D, V3E, V3F, V40, V41, V42, V43, V44, V45, V46, V47, V48, V49, V4A,
/// #   V4B, V4C, V4D, V4E, V4F, V50, V51, V52, V53, V54, V55, V56, V57, V58, V59,
/// #   V5A, V5B, V5C, V5D, V5E, V5F, V60, V61, V62, V63, V64, V65, V66, V67, V68,
/// #   V69, V6A, V6B, V6C, V6D, V6E, V6F, V70, V71, V72, V73, V74, V75, V76, V77,
/// #   V78, V79, V7A, V7B, V7C, V7D, V7E, V7F, V80, V81, V82, V83, V84, V85, V86,
/// #   V87, V88, V89, V8A, V8B, V8C, V8D, V8E, V8F, V90, V91, V92, V93, V94, V95,
/// #   V96, V97, V98, V99, V9A, V9B, V9C, V9D, V9E, V9F, VA0, VA1, VA2, VA3, VA4,
/// #   VA5, VA6, VA7, VA8, VA9, VAA, VAB, VAC, VAD, VAE, VAF, VB0, VB1, VB2, VB3,
/// #   VB4, VB5, VB6, VB7, VB8, VB9, VBA, VBB, VBC, VBD, VBE, VBF, VC0, VC1, VC2,
/// #   VC3, VC4, VC5, VC6, VC7, VC8, VC9, VCA, VCB, VCC, VCD, VCE, VCF, VD0, VD1,
/// #   VD2, VD3, VD4, VD5, VD6, VD7, VD8, VD9, VDA, VDB, VDC, VDD, VDE, VDF, VE0,
/// #   VE1, VE2, VE3, VE4, VE5, VE6, VE7, VE8, VE9, VEA, VEB, VEC, VED, VEE, VEF,
/// #   VF0, VF1, VF2, VF3, VF4, VF5, VF6, VF7, VF8, VF9, VFA, VFB, VFC, VFD, VFE,
/// #   VFF,
/// # /*
///     ...
/// # */
/// }
///
/// #[derive(FromZeroes, FromBytes)]
/// union MyUnion {
/// #   variant: u8,
/// # /*
///     ...
/// # */
/// }
/// ```
///
/// [safety conditions]: trait@FromBytes#safety
///
/// # Analysis
///
/// *This section describes, roughly, the analysis performed by this derive to
/// determine whether it is sound to implement `FromBytes` for a given type.
/// Unless you are modifying the implementation of this derive, or attempting to
/// manually implement `FromBytes` for a type yourself, you don't need to read
/// this section.*
///
/// If a type has the following properties, then this derive can implement
/// `FromBytes` for that type:
///
/// - If the type is a struct, all of its fields must be `FromBytes`.
/// - If the type is an enum:
///   - It must be a C-like enum (meaning that all variants have no fields).
///   - It must have a defined representation (`repr`s `C`, `u8`, `u16`, `u32`,
///     `u64`, `usize`, `i8`, `i16`, `i32`, `i64`, or `isize`).
///   - The maximum number of discriminants must be used (so that every possible
///     bit pattern is a valid one). Be very careful when using the `C`,
///     `usize`, or `isize` representations, as their size is
///     platform-dependent.
/// - The type must not contain any [`UnsafeCell`]s (this is required in order
///   for it to be sound to construct a `&[u8]` and a `&T` to the same region of
///   memory). The type may contain references or pointers to `UnsafeCell`s so
///   long as those values can themselves be initialized from zeroes
///   (`FromBytes` is not currently implemented for, e.g., `Option<*const
///   UnsafeCell<_>>`, but it could be one day).
///
/// [`UnsafeCell`]: core::cell::UnsafeCell
///
/// This analysis is subject to change. Unsafe code may *only* rely on the
/// documented [safety conditions] of `FromBytes`, and must *not* rely on the
/// implementation details of this derive.
///
/// ## Why isn't an explicit representation required for structs?
///
/// Neither this derive, nor the [safety conditions] of `FromBytes`, requires
/// that structs are marked with `#[repr(C)]`.
///
/// Per the [Rust reference](reference),
///
/// > The representation of a type can change the padding between fields, but
/// > does not change the layout of the fields themselves.
///
/// [reference]: https://doc.rust-lang.org/reference/type-layout.html#representations
///
/// Since the layout of structs only consists of padding bytes and field bytes,
/// a struct is soundly `FromBytes` if:
/// 1. its padding is soundly `FromBytes`, and
/// 2. its fields are soundly `FromBytes`.
///
/// The answer to the first question is always yes: padding bytes do not have
/// any validity constraints. A [discussion] of this question in the Unsafe Code
/// Guidelines Working Group concluded that it would be virtually unimaginable
/// for future versions of rustc to add validity constraints to padding bytes.
///
/// [discussion]: https://github.com/rust-lang/unsafe-code-guidelines/issues/174
///
/// Whether a struct is soundly `FromBytes` therefore solely depends on whether
/// its fields are `FromBytes`.
// TODO(#146): Document why we don't require an enum to have an explicit `repr`
// attribute.
#[cfg(any(feature = "derive", test))]
#[cfg_attr(doc_cfg, doc(cfg(feature = "derive")))]
pub use zerocopy_derive::FromBytes;

/// Types for which any bit pattern is valid.
///
/// Any memory region of the appropriate length which contains initialized bytes
/// can be viewed as any `FromBytes` type with no runtime overhead. This is
/// useful for efficiently parsing bytes as structured data.
///
/// # Implementation
///
/// **Do not implement this trait yourself!** Instead, use
/// [`#[derive(FromBytes)]`][derive] (requires the `derive` Cargo feature);
/// e.g.:
///
/// ```
/// # use zerocopy_derive::{FromBytes, FromZeroes};
/// #[derive(FromZeroes, FromBytes)]
/// struct MyStruct {
/// # /*
///     ...
/// # */
/// }
///
/// #[derive(FromZeroes, FromBytes)]
/// #[repr(u8)]
/// enum MyEnum {
/// #   V00, V01, V02, V03, V04, V05, V06, V07, V08, V09, V0A, V0B, V0C, V0D, V0E,
/// #   V0F, V10, V11, V12, V13, V14, V15, V16, V17, V18, V19, V1A, V1B, V1C, V1D,
/// #   V1E, V1F, V20, V21, V22, V23, V24, V25, V26, V27, V28, V29, V2A, V2B, V2C,
/// #   V2D, V2E, V2F, V30, V31, V32, V33, V34, V35, V36, V37, V38, V39, V3A, V3B,
/// #   V3C, V3D, V3E, V3F, V40, V41, V42, V43, V44, V45, V46, V47, V48, V49, V4A,
/// #   V4B, V4C, V4D, V4E, V4F, V50, V51, V52, V53, V54, V55, V56, V57, V58, V59,
/// #   V5A, V5B, V5C, V5D, V5E, V5F, V60, V61, V62, V63, V64, V65, V66, V67, V68,
/// #   V69, V6A, V6B, V6C, V6D, V6E, V6F, V70, V71, V72, V73, V74, V75, V76, V77,
/// #   V78, V79, V7A, V7B, V7C, V7D, V7E, V7F, V80, V81, V82, V83, V84, V85, V86,
/// #   V87, V88, V89, V8A, V8B, V8C, V8D, V8E, V8F, V90, V91, V92, V93, V94, V95,
/// #   V96, V97, V98, V99, V9A, V9B, V9C, V9D, V9E, V9F, VA0, VA1, VA2, VA3, VA4,
/// #   VA5, VA6, VA7, VA8, VA9, VAA, VAB, VAC, VAD, VAE, VAF, VB0, VB1, VB2, VB3,
/// #   VB4, VB5, VB6, VB7, VB8, VB9, VBA, VBB, VBC, VBD, VBE, VBF, VC0, VC1, VC2,
/// #   VC3, VC4, VC5, VC6, VC7, VC8, VC9, VCA, VCB, VCC, VCD, VCE, VCF, VD0, VD1,
/// #   VD2, VD3, VD4, VD5, VD6, VD7, VD8, VD9, VDA, VDB, VDC, VDD, VDE, VDF, VE0,
/// #   VE1, VE2, VE3, VE4, VE5, VE6, VE7, VE8, VE9, VEA, VEB, VEC, VED, VEE, VEF,
/// #   VF0, VF1, VF2, VF3, VF4, VF5, VF6, VF7, VF8, VF9, VFA, VFB, VFC, VFD, VFE,
/// #   VFF,
/// # /*
///     ...
/// # */
/// }
///
/// #[derive(FromZeroes, FromBytes)]
/// union MyUnion {
/// #   variant: u8,
/// # /*
///     ...
/// # */
/// }
/// ```
///
/// This derive performs a sophisticated, compile-time safety analysis to
/// determine whether a type is `FromBytes`.
///
/// # Safety
///
/// *This section describes what is required in order for `T: FromBytes`, and
/// what unsafe code may assume of such types. If you don't plan on implementing
/// `FromBytes` manually, and you don't plan on writing unsafe code that
/// operates on `FromBytes` types, then you don't need to read this section.*
///
/// If `T: FromBytes`, then unsafe code may assume that:
/// - It is sound to treat any initialized sequence of bytes of length
///   `size_of::<T>()` as a `T`.
/// - Given `b: &[u8]` where `b.len() == size_of::<T>()`, `b` is aligned to
///   `align_of::<T>()` it is sound to construct a `t: &T` at the same address
///   as `b`, and it is sound for both `b` and `t` to be live at the same time.
///
/// If a type is marked as `FromBytes` which violates this contract, it may
/// cause undefined behavior.
///
/// `#[derive(FromBytes)]` only permits [types which satisfy these
/// requirements][derive-analysis].
///
#[cfg_attr(
    feature = "derive",
    doc = "[derive]: zerocopy_derive::FromBytes",
    doc = "[derive-analysis]: zerocopy_derive::FromBytes#analysis"
)]
#[cfg_attr(
    not(feature = "derive"),
    doc = concat!("[derive]: https://docs.rs/zerocopy/", env!("CARGO_PKG_VERSION"), "/zerocopy/derive.FromBytes.html"),
    doc = concat!("[derive-analysis]: https://docs.rs/zerocopy/", env!("CARGO_PKG_VERSION"), "/zerocopy/derive.FromBytes.html#analysis"),
)]
pub unsafe trait FromBytes: FromZeroes {
    // The `Self: Sized` bound makes it so that `FromBytes` is still object
    // safe.
    #[doc(hidden)]
    fn only_derive_is_allowed_to_implement_this_trait()
    where
        Self: Sized;

    /// Interprets the given `bytes` as a `&Self` without copying.
    ///
    /// If `bytes.len() != size_of::<Self>()` or `bytes` is not aligned to
    /// `align_of::<Self>()`, this returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::FromBytes;
    /// # use zerocopy_derive::*;
    ///
    /// #[derive(FromZeroes, FromBytes)]
    /// #[repr(C)]
    /// struct PacketHeader {
    ///     src_port: [u8; 2],
    ///     dst_port: [u8; 2],
    ///     length: [u8; 2],
    ///     checksum: [u8; 2],
    /// }
    ///
    /// // These bytes encode a `PacketHeader`.
    /// let bytes = [0, 1, 2, 3, 4, 5, 6, 7].as_slice();
    ///
    /// let header = PacketHeader::ref_from(bytes).unwrap();
    ///
    /// assert_eq!(header.src_port, [0, 1]);
    /// assert_eq!(header.dst_port, [2, 3]);
    /// assert_eq!(header.length, [4, 5]);
    /// assert_eq!(header.checksum, [6, 7]);
    /// ```
    #[inline]
    fn ref_from(bytes: &[u8]) -> Option<&Self>
    where
        Self: Sized,
    {
        Ref::<&[u8], Self>::new(bytes).map(Ref::into_ref)
    }

    /// Interprets the prefix of the given `bytes` as a `&Self` without copying.
    ///
    /// `ref_from_prefix` returns a reference to the first `size_of::<Self>()`
    /// bytes of `bytes`. If `bytes.len() < size_of::<Self>()` or `bytes` is not
    /// aligned to `align_of::<Self>()`, this returns `None`.
    ///
    /// To also access the prefix bytes, use [`Ref::new_from_prefix`]. Then, use
    /// [`Ref::into_ref`] to get a `&Self` with the same lifetime.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::FromBytes;
    /// # use zerocopy_derive::*;
    ///
    /// #[derive(FromZeroes, FromBytes)]
    /// #[repr(C)]
    /// struct PacketHeader {
    ///     src_port: [u8; 2],
    ///     dst_port: [u8; 2],
    ///     length: [u8; 2],
    ///     checksum: [u8; 2],
    /// }
    ///
    /// // These are more bytes than are needed to encode a `PacketHeader`.
    /// let bytes = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9].as_slice();
    ///
    /// let header = PacketHeader::ref_from_prefix(bytes).unwrap();
    ///
    /// assert_eq!(header.src_port, [0, 1]);
    /// assert_eq!(header.dst_port, [2, 3]);
    /// assert_eq!(header.length, [4, 5]);
    /// assert_eq!(header.checksum, [6, 7]);
    /// ```
    #[inline]
    fn ref_from_prefix(bytes: &[u8]) -> Option<&Self>
    where
        Self: Sized,
    {
        Ref::<&[u8], Self>::new_from_prefix(bytes).map(|(r, _)| r.into_ref())
    }

    /// Interprets the suffix of the given `bytes` as a `&Self` without copying.
    ///
    /// `ref_from_suffix` returns a reference to the last `size_of::<Self>()`
    /// bytes of `bytes`. If `bytes.len() < size_of::<Self>()` or the suffix of
    /// `bytes` is not aligned to `align_of::<Self>()`, this returns `None`.
    ///
    /// To also access the suffix bytes, use [`Ref::new_from_suffix`]. Then, use
    /// [`Ref::into_ref`] to get a `&Self` with the same lifetime.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::FromBytes;
    /// # use zerocopy_derive::*;
    ///
    /// #[derive(FromZeroes, FromBytes)]
    /// #[repr(C)]
    /// struct PacketTrailer {
    ///     frame_check_sequence: [u8; 4],
    /// }
    ///
    /// // These are more bytes than are needed to encode a `PacketTrailer`.
    /// let bytes = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9].as_slice();
    ///
    /// let trailer = PacketTrailer::ref_from_suffix(bytes).unwrap();
    ///
    /// assert_eq!(trailer.frame_check_sequence, [6, 7, 8, 9]);
    /// ```
    #[inline]
    fn ref_from_suffix(bytes: &[u8]) -> Option<&Self>
    where
        Self: Sized,
    {
        Ref::<&[u8], Self>::new_from_suffix(bytes).map(|(_, r)| r.into_ref())
    }

    /// Interprets the given `bytes` as a `&mut Self` without copying.
    ///
    /// If `bytes.len() != size_of::<Self>()` or `bytes` is not aligned to
    /// `align_of::<Self>()`, this returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::FromBytes;
    /// # use zerocopy_derive::*;
    ///
    /// #[derive(AsBytes, FromZeroes, FromBytes)]
    /// #[repr(C)]
    /// struct PacketHeader {
    ///     src_port: [u8; 2],
    ///     dst_port: [u8; 2],
    ///     length: [u8; 2],
    ///     checksum: [u8; 2],
    /// }
    ///
    /// // These bytes encode a `PacketHeader`.
    /// let bytes = &mut [0, 1, 2, 3, 4, 5, 6, 7][..];
    ///
    /// let header = PacketHeader::mut_from(bytes).unwrap();
    ///
    /// assert_eq!(header.src_port, [0, 1]);
    /// assert_eq!(header.dst_port, [2, 3]);
    /// assert_eq!(header.length, [4, 5]);
    /// assert_eq!(header.checksum, [6, 7]);
    ///
    /// header.checksum = [0, 0];
    ///
    /// assert_eq!(bytes, [0, 1, 2, 3, 4, 5, 0, 0]);
    /// ```
    #[inline]
    fn mut_from(bytes: &mut [u8]) -> Option<&mut Self>
    where
        Self: Sized + AsBytes,
    {
        Ref::<&mut [u8], Self>::new(bytes).map(Ref::into_mut)
    }

    /// Interprets the prefix of the given `bytes` as a `&mut Self` without
    /// copying.
    ///
    /// `mut_from_prefix` returns a reference to the first `size_of::<Self>()`
    /// bytes of `bytes`. If `bytes.len() < size_of::<Self>()` or `bytes` is not
    /// aligned to `align_of::<Self>()`, this returns `None`.
    ///
    /// To also access the prefix bytes, use [`Ref::new_from_prefix`]. Then, use
    /// [`Ref::into_mut`] to get a `&mut Self` with the same lifetime.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::FromBytes;
    /// # use zerocopy_derive::*;
    ///
    /// #[derive(AsBytes, FromZeroes, FromBytes)]
    /// #[repr(C)]
    /// struct PacketHeader {
    ///     src_port: [u8; 2],
    ///     dst_port: [u8; 2],
    ///     length: [u8; 2],
    ///     checksum: [u8; 2],
    /// }
    ///
    /// // These are more bytes than are needed to encode a `PacketHeader`.
    /// let bytes = &mut [0, 1, 2, 3, 4, 5, 6, 7, 8, 9][..];
    ///
    /// let header = PacketHeader::mut_from_prefix(bytes).unwrap();
    ///
    /// assert_eq!(header.src_port, [0, 1]);
    /// assert_eq!(header.dst_port, [2, 3]);
    /// assert_eq!(header.length, [4, 5]);
    /// assert_eq!(header.checksum, [6, 7]);
    ///
    /// header.checksum = [0, 0];
    ///
    /// assert_eq!(bytes, [0, 1, 2, 3, 4, 5, 0, 0, 8, 9]);
    /// ```
    #[inline]
    fn mut_from_prefix(bytes: &mut [u8]) -> Option<&mut Self>
    where
        Self: Sized + AsBytes,
    {
        Ref::<&mut [u8], Self>::new_from_prefix(bytes).map(|(r, _)| r.into_mut())
    }

    /// Interprets the suffix of the given `bytes` as a `&mut Self` without copying.
    ///
    /// `mut_from_suffix` returns a reference to the last `size_of::<Self>()`
    /// bytes of `bytes`. If `bytes.len() < size_of::<Self>()` or the suffix of
    /// `bytes` is not aligned to `align_of::<Self>()`, this returns `None`.
    ///
    /// To also access the suffix bytes, use [`Ref::new_from_suffix`]. Then,
    /// use [`Ref::into_mut`] to get a `&mut Self` with the same lifetime.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::FromBytes;
    /// # use zerocopy_derive::*;
    ///
    /// #[derive(AsBytes, FromZeroes, FromBytes)]
    /// #[repr(C)]
    /// struct PacketTrailer {
    ///     frame_check_sequence: [u8; 4],
    /// }
    ///
    /// // These are more bytes than are needed to encode a `PacketTrailer`.
    /// let bytes = &mut [0, 1, 2, 3, 4, 5, 6, 7, 8, 9][..];
    ///
    /// let trailer = PacketTrailer::mut_from_suffix(bytes).unwrap();
    ///
    /// assert_eq!(trailer.frame_check_sequence, [6, 7, 8, 9]);
    ///
    /// trailer.frame_check_sequence = [0, 0, 0, 0];
    ///
    /// assert_eq!(bytes, [0, 1, 2, 3, 4, 5, 0, 0, 0, 0]);
    /// ```
    #[inline]
    fn mut_from_suffix(bytes: &mut [u8]) -> Option<&mut Self>
    where
        Self: Sized + AsBytes,
    {
        Ref::<&mut [u8], Self>::new_from_suffix(bytes).map(|(_, r)| r.into_mut())
    }

    /// Interprets the given `bytes` as a `&[Self]` without copying.
    ///
    /// If `bytes.len() % size_of::<Self>() != 0` or `bytes` is not aligned to
    /// `align_of::<Self>()`, this returns `None`.
    ///
    /// If you need to convert a specific number of slice elements, see
    /// [`slice_from_prefix`](FromBytes::slice_from_prefix) or
    /// [`slice_from_suffix`](FromBytes::slice_from_suffix).
    ///
    /// # Panics
    ///
    /// If `Self` is a zero-sized type.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::FromBytes;
    /// # use zerocopy_derive::*;
    ///
    /// # #[derive(Debug, PartialEq, Eq)]
    /// #[derive(FromZeroes, FromBytes)]
    /// #[repr(C)]
    /// struct Pixel {
    ///     r: u8,
    ///     g: u8,
    ///     b: u8,
    ///     a: u8,
    /// }
    ///
    /// // These bytes encode two `Pixel`s.
    /// let bytes = [0, 1, 2, 3, 4, 5, 6, 7].as_slice();
    ///
    /// let pixels = Pixel::slice_from(bytes).unwrap();
    ///
    /// assert_eq!(pixels, &[
    ///     Pixel { r: 0, g: 1, b: 2, a: 3 },
    ///     Pixel { r: 4, g: 5, b: 6, a: 7 },
    /// ]);
    /// ```
    #[inline]
    fn slice_from(bytes: &[u8]) -> Option<&[Self]>
    where
        Self: Sized,
    {
        Ref::<_, [Self]>::new_slice(bytes).map(|r| r.into_slice())
    }

    /// Interprets the prefix of the given `bytes` as a `&[Self]` with length
    /// equal to `count` without copying.
    ///
    /// This method verifies that `bytes.len() >= size_of::<T>() * count`
    /// and that `bytes` is aligned to `align_of::<T>()`. It consumes the
    /// first `size_of::<T>() * count` bytes from `bytes` to construct a
    /// `&[Self]`, and returns the remaining bytes to the caller. It also
    /// ensures that `sizeof::<T>() * count` does not overflow a `usize`.
    /// If any of the length, alignment, or overflow checks fail, it returns
    /// `None`.
    ///
    /// # Panics
    ///
    /// If `T` is a zero-sized type.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::FromBytes;
    /// # use zerocopy_derive::*;
    ///
    /// # #[derive(Debug, PartialEq, Eq)]
    /// #[derive(FromZeroes, FromBytes)]
    /// #[repr(C)]
    /// struct Pixel {
    ///     r: u8,
    ///     g: u8,
    ///     b: u8,
    ///     a: u8,
    /// }
    ///
    /// // These are more bytes than are needed to encode two `Pixel`s.
    /// let bytes = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9].as_slice();
    ///
    /// let (pixels, rest) = Pixel::slice_from_prefix(bytes, 2).unwrap();
    ///
    /// assert_eq!(pixels, &[
    ///     Pixel { r: 0, g: 1, b: 2, a: 3 },
    ///     Pixel { r: 4, g: 5, b: 6, a: 7 },
    /// ]);
    ///
    /// assert_eq!(rest, &[8, 9]);
    /// ```
    #[inline]
    fn slice_from_prefix(bytes: &[u8], count: usize) -> Option<(&[Self], &[u8])>
    where
        Self: Sized,
    {
        Ref::<_, [Self]>::new_slice_from_prefix(bytes, count).map(|(r, b)| (r.into_slice(), b))
    }

    /// Interprets the suffix of the given `bytes` as a `&[Self]` with length
    /// equal to `count` without copying.
    ///
    /// This method verifies that `bytes.len() >= size_of::<T>() * count`
    /// and that `bytes` is aligned to `align_of::<T>()`. It consumes the
    /// last `size_of::<T>() * count` bytes from `bytes` to construct a
    /// `&[Self]`, and returns the preceding bytes to the caller. It also
    /// ensures that `sizeof::<T>() * count` does not overflow a `usize`.
    /// If any of the length, alignment, or overflow checks fail, it returns
    /// `None`.
    ///
    /// # Panics
    ///
    /// If `T` is a zero-sized type.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::FromBytes;
    /// # use zerocopy_derive::*;
    ///
    /// # #[derive(Debug, PartialEq, Eq)]
    /// #[derive(FromZeroes, FromBytes)]
    /// #[repr(C)]
    /// struct Pixel {
    ///     r: u8,
    ///     g: u8,
    ///     b: u8,
    ///     a: u8,
    /// }
    ///
    /// // These are more bytes than are needed to encode two `Pixel`s.
    /// let bytes = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9].as_slice();
    ///
    /// let (rest, pixels) = Pixel::slice_from_suffix(bytes, 2).unwrap();
    ///
    /// assert_eq!(rest, &[0, 1]);
    ///
    /// assert_eq!(pixels, &[
    ///     Pixel { r: 2, g: 3, b: 4, a: 5 },
    ///     Pixel { r: 6, g: 7, b: 8, a: 9 },
    /// ]);
    /// ```
    #[inline]
    fn slice_from_suffix(bytes: &[u8], count: usize) -> Option<(&[u8], &[Self])>
    where
        Self: Sized,
    {
        Ref::<_, [Self]>::new_slice_from_suffix(bytes, count).map(|(b, r)| (b, r.into_slice()))
    }

    /// Interprets the given `bytes` as a `&mut [Self]` without copying.
    ///
    /// If `bytes.len() % size_of::<T>() != 0` or `bytes` is not aligned to
    /// `align_of::<T>()`, this returns `None`.
    ///
    /// If you need to convert a specific number of slice elements, see
    /// [`mut_slice_from_prefix`](FromBytes::mut_slice_from_prefix) or
    /// [`mut_slice_from_suffix`](FromBytes::mut_slice_from_suffix).
    ///
    /// # Panics
    ///
    /// If `T` is a zero-sized type.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::FromBytes;
    /// # use zerocopy_derive::*;
    ///
    /// # #[derive(Debug, PartialEq, Eq)]
    /// #[derive(AsBytes, FromZeroes, FromBytes)]
    /// #[repr(C)]
    /// struct Pixel {
    ///     r: u8,
    ///     g: u8,
    ///     b: u8,
    ///     a: u8,
    /// }
    ///
    /// // These bytes encode two `Pixel`s.
    /// let bytes = &mut [0, 1, 2, 3, 4, 5, 6, 7][..];
    ///
    /// let pixels = Pixel::mut_slice_from(bytes).unwrap();
    ///
    /// assert_eq!(pixels, &[
    ///     Pixel { r: 0, g: 1, b: 2, a: 3 },
    ///     Pixel { r: 4, g: 5, b: 6, a: 7 },
    /// ]);
    ///
    /// pixels[1] = Pixel { r: 0, g: 0, b: 0, a: 0 };
    ///
    /// assert_eq!(bytes, [0, 1, 2, 3, 0, 0, 0, 0]);
    /// ```
    #[inline]
    fn mut_slice_from(bytes: &mut [u8]) -> Option<&mut [Self]>
    where
        Self: Sized + AsBytes,
    {
        Ref::<_, [Self]>::new_slice(bytes).map(|r| r.into_mut_slice())
    }

    /// Interprets the prefix of the given `bytes` as a `&mut [Self]` with length
    /// equal to `count` without copying.
    ///
    /// This method verifies that `bytes.len() >= size_of::<T>() * count`
    /// and that `bytes` is aligned to `align_of::<T>()`. It consumes the
    /// first `size_of::<T>() * count` bytes from `bytes` to construct a
    /// `&[Self]`, and returns the remaining bytes to the caller. It also
    /// ensures that `sizeof::<T>() * count` does not overflow a `usize`.
    /// If any of the length, alignment, or overflow checks fail, it returns
    /// `None`.
    ///
    /// # Panics
    ///
    /// If `T` is a zero-sized type.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::FromBytes;
    /// # use zerocopy_derive::*;
    ///
    /// # #[derive(Debug, PartialEq, Eq)]
    /// #[derive(AsBytes, FromZeroes, FromBytes)]
    /// #[repr(C)]
    /// struct Pixel {
    ///     r: u8,
    ///     g: u8,
    ///     b: u8,
    ///     a: u8,
    /// }
    ///
    /// // These are more bytes than are needed to encode two `Pixel`s.
    /// let bytes = &mut [0, 1, 2, 3, 4, 5, 6, 7, 8, 9][..];
    ///
    /// let (pixels, rest) = Pixel::mut_slice_from_prefix(bytes, 2).unwrap();
    ///
    /// assert_eq!(pixels, &[
    ///     Pixel { r: 0, g: 1, b: 2, a: 3 },
    ///     Pixel { r: 4, g: 5, b: 6, a: 7 },
    /// ]);
    ///
    /// assert_eq!(rest, &[8, 9]);
    ///
    /// pixels[1] = Pixel { r: 0, g: 0, b: 0, a: 0 };
    ///
    /// assert_eq!(bytes, [0, 1, 2, 3, 0, 0, 0, 0, 8, 9]);
    /// ```
    #[inline]
    fn mut_slice_from_prefix(bytes: &mut [u8], count: usize) -> Option<(&mut [Self], &mut [u8])>
    where
        Self: Sized + AsBytes,
    {
        Ref::<_, [Self]>::new_slice_from_prefix(bytes, count).map(|(r, b)| (r.into_mut_slice(), b))
    }

    /// Interprets the suffix of the given `bytes` as a `&mut [Self]` with length
    /// equal to `count` without copying.
    ///
    /// This method verifies that `bytes.len() >= size_of::<T>() * count`
    /// and that `bytes` is aligned to `align_of::<T>()`. It consumes the
    /// last `size_of::<T>() * count` bytes from `bytes` to construct a
    /// `&[Self]`, and returns the preceding bytes to the caller. It also
    /// ensures that `sizeof::<T>() * count` does not overflow a `usize`.
    /// If any of the length, alignment, or overflow checks fail, it returns
    /// `None`.
    ///
    /// # Panics
    ///
    /// If `T` is a zero-sized type.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::FromBytes;
    /// # use zerocopy_derive::*;
    ///
    /// # #[derive(Debug, PartialEq, Eq)]
    /// #[derive(AsBytes, FromZeroes, FromBytes)]
    /// #[repr(C)]
    /// struct Pixel {
    ///     r: u8,
    ///     g: u8,
    ///     b: u8,
    ///     a: u8,
    /// }
    ///
    /// // These are more bytes than are needed to encode two `Pixel`s.
    /// let bytes = &mut [0, 1, 2, 3, 4, 5, 6, 7, 8, 9][..];
    ///
    /// let (rest, pixels) = Pixel::mut_slice_from_suffix(bytes, 2).unwrap();
    ///
    /// assert_eq!(rest, &[0, 1]);
    ///
    /// assert_eq!(pixels, &[
    ///     Pixel { r: 2, g: 3, b: 4, a: 5 },
    ///     Pixel { r: 6, g: 7, b: 8, a: 9 },
    /// ]);
    ///
    /// pixels[1] = Pixel { r: 0, g: 0, b: 0, a: 0 };
    ///
    /// assert_eq!(bytes, [0, 1, 2, 3, 4, 5, 0, 0, 0, 0]);
    /// ```
    #[inline]
    fn mut_slice_from_suffix(bytes: &mut [u8], count: usize) -> Option<(&mut [u8], &mut [Self])>
    where
        Self: Sized + AsBytes,
    {
        Ref::<_, [Self]>::new_slice_from_suffix(bytes, count).map(|(b, r)| (b, r.into_mut_slice()))
    }

    /// Reads a copy of `Self` from `bytes`.
    ///
    /// If `bytes.len() != size_of::<Self>()`, `read_from` returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::FromBytes;
    /// # use zerocopy_derive::*;
    ///
    /// #[derive(FromZeroes, FromBytes)]
    /// #[repr(C)]
    /// struct PacketHeader {
    ///     src_port: [u8; 2],
    ///     dst_port: [u8; 2],
    ///     length: [u8; 2],
    ///     checksum: [u8; 2],
    /// }
    ///
    /// // These bytes encode a `PacketHeader`.
    /// let bytes = [0, 1, 2, 3, 4, 5, 6, 7].as_slice();
    ///
    /// let header = PacketHeader::read_from(bytes).unwrap();
    ///
    /// assert_eq!(header.src_port, [0, 1]);
    /// assert_eq!(header.dst_port, [2, 3]);
    /// assert_eq!(header.length, [4, 5]);
    /// assert_eq!(header.checksum, [6, 7]);
    /// ```
    #[inline]
    fn read_from(bytes: &[u8]) -> Option<Self>
    where
        Self: Sized,
    {
        Ref::<_, Unalign<Self>>::new_unaligned(bytes).map(|r| r.read().into_inner())
    }

    /// Reads a copy of `Self` from the prefix of `bytes`.
    ///
    /// `read_from_prefix` reads a `Self` from the first `size_of::<Self>()`
    /// bytes of `bytes`. If `bytes.len() < size_of::<Self>()`, it returns
    /// `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::FromBytes;
    /// # use zerocopy_derive::*;
    ///
    /// #[derive(FromZeroes, FromBytes)]
    /// #[repr(C)]
    /// struct PacketHeader {
    ///     src_port: [u8; 2],
    ///     dst_port: [u8; 2],
    ///     length: [u8; 2],
    ///     checksum: [u8; 2],
    /// }
    ///
    /// // These are more bytes than are needed to encode a `PacketHeader`.
    /// let bytes = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9].as_slice();
    ///
    /// let header = PacketHeader::read_from_prefix(bytes).unwrap();
    ///
    /// assert_eq!(header.src_port, [0, 1]);
    /// assert_eq!(header.dst_port, [2, 3]);
    /// assert_eq!(header.length, [4, 5]);
    /// assert_eq!(header.checksum, [6, 7]);
    /// ```
    #[inline]
    fn read_from_prefix(bytes: &[u8]) -> Option<Self>
    where
        Self: Sized,
    {
        Ref::<_, Unalign<Self>>::new_unaligned_from_prefix(bytes)
            .map(|(r, _)| r.read().into_inner())
    }

    /// Reads a copy of `Self` from the suffix of `bytes`.
    ///
    /// `read_from_suffix` reads a `Self` from the last `size_of::<Self>()`
    /// bytes of `bytes`. If `bytes.len() < size_of::<Self>()`, it returns
    /// `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::FromBytes;
    /// # use zerocopy_derive::*;
    ///
    /// #[derive(FromZeroes, FromBytes)]
    /// #[repr(C)]
    /// struct PacketTrailer {
    ///     frame_check_sequence: [u8; 4],
    /// }
    ///
    /// // These are more bytes than are needed to encode a `PacketTrailer`.
    /// let bytes = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9].as_slice();
    ///
    /// let trailer = PacketTrailer::read_from_suffix(bytes).unwrap();
    ///
    /// assert_eq!(trailer.frame_check_sequence, [6, 7, 8, 9]);
    /// ```
    #[inline]
    fn read_from_suffix(bytes: &[u8]) -> Option<Self>
    where
        Self: Sized,
    {
        Ref::<_, Unalign<Self>>::new_unaligned_from_suffix(bytes)
            .map(|(_, r)| r.read().into_inner())
    }
}

/// Analyzes whether a type is [`AsBytes`].
///
/// This derive analyzes, at compile time, whether the annotated type satisfies
/// the [safety conditions] of `AsBytes` and implements `AsBytes` if it is
/// sound to do so. This derive can be applied to structs, enums, and unions;
/// e.g.:
///
/// ```
/// # use zerocopy_derive::{AsBytes};
/// #[derive(AsBytes)]
/// #[repr(C)]
/// struct MyStruct {
/// # /*
///     ...
/// # */
/// }
///
/// #[derive(AsBytes)]
/// #[repr(u8)]
/// enum MyEnum {
/// #   Variant,
/// # /*
///     ...
/// # */
/// }
///
/// #[derive(AsBytes)]
/// #[repr(C)]
/// union MyUnion {
/// #   variant: u8,
/// # /*
///     ...
/// # */
/// }
/// ```
///
/// [safety conditions]: trait@AsBytes#safety
///
/// # Error Messages
///
/// Due to the way that the custom derive for `AsBytes` is implemented, you may
/// get an error like this:
///
/// ```text
/// error[E0277]: the trait bound `HasPadding<Foo, true>: ShouldBe<false>` is not satisfied
///   --> lib.rs:23:10
///    |
///  1 | #[derive(AsBytes)]
///    |          ^^^^^^^ the trait `ShouldBe<false>` is not implemented for `HasPadding<Foo, true>`
///    |
///    = help: the trait `ShouldBe<VALUE>` is implemented for `HasPadding<T, VALUE>`
/// ```
///
/// This error indicates that the type being annotated has padding bytes, which
/// is illegal for `AsBytes` types. Consider reducing the alignment of some
/// fields by using types in the [`byteorder`] module, adding explicit struct
/// fields where those padding bytes would be, or using `#[repr(packed)]`. See
/// the Rust Reference's page on [type layout] for more information
/// about type layout and padding.
///
/// [type layout]: https://doc.rust-lang.org/reference/type-layout.html
///
/// # Analysis
///
/// *This section describes, roughly, the analysis performed by this derive to
/// determine whether it is sound to implement `AsBytes` for a given type.
/// Unless you are modifying the implementation of this derive, or attempting to
/// manually implement `AsBytes` for a type yourself, you don't need to read
/// this section.*
///
/// If a type has the following properties, then this derive can implement
/// `AsBytes` for that type:
///
/// - If the type is a struct:
///   - It must have a defined representation (`repr(C)`, `repr(transparent)`,
///     or `repr(packed)`).
///   - All of its fields must be `AsBytes`.
///   - Its layout must have no padding. This is always true for
///     `repr(transparent)` and `repr(packed)`. For `repr(C)`, see the layout
///     algorithm described in the [Rust Reference].
/// - If the type is an enum:
///   - It must be a C-like enum (meaning that all variants have no fields).
///   - It must have a defined representation (`repr`s `C`, `u8`, `u16`, `u32`,
///     `u64`, `usize`, `i8`, `i16`, `i32`, `i64`, or `isize`).
/// - The type must not contain any [`UnsafeCell`]s (this is required in order
///   for it to be sound to construct a `&[u8]` and a `&T` to the same region of
///   memory). The type may contain references or pointers to `UnsafeCell`s so
///   long as those values can themselves be initialized from zeroes (`AsBytes`
///   is not currently implemented for, e.g., `Option<&UnsafeCell<_>>`, but it
///   could be one day).
///
/// [`UnsafeCell`]: core::cell::UnsafeCell
///
/// This analysis is subject to change. Unsafe code may *only* rely on the
/// documented [safety conditions] of `FromBytes`, and must *not* rely on the
/// implementation details of this derive.
///
/// [Rust Reference]: https://doc.rust-lang.org/reference/type-layout.html
#[cfg(any(feature = "derive", test))]
#[cfg_attr(doc_cfg, doc(cfg(feature = "derive")))]
pub use zerocopy_derive::AsBytes;

/// Types that can be viewed as an immutable slice of initialized bytes.
///
/// Any `AsBytes` type can be viewed as a slice of initialized bytes of the same
/// size. This is useful for efficiently serializing structured data as raw
/// bytes.
///
/// # Implementation
///
/// **Do not implement this trait yourself!** Instead, use
/// [`#[derive(AsBytes)]`][derive] (requires the `derive` Cargo feature); e.g.:
///
/// ```
/// # use zerocopy_derive::AsBytes;
/// #[derive(AsBytes)]
/// #[repr(C)]
/// struct MyStruct {
/// # /*
///     ...
/// # */
/// }
///
/// #[derive(AsBytes)]
/// #[repr(u8)]
/// enum MyEnum {
/// #   Variant0,
/// # /*
///     ...
/// # */
/// }
///
/// #[derive(AsBytes)]
/// #[repr(C)]
/// union MyUnion {
/// #   variant: u8,
/// # /*
///     ...
/// # */
/// }
/// ```
///
/// This derive performs a sophisticated, compile-time safety analysis to
/// determine whether a type is `AsBytes`. See the [derive
/// documentation][derive] for guidance on how to interpret error messages
/// produced by the derive's analysis.
///
/// # Safety
///
/// *This section describes what is required in order for `T: AsBytes`, and
/// what unsafe code may assume of such types. If you don't plan on implementing
/// `AsBytes` manually, and you don't plan on writing unsafe code that
/// operates on `AsBytes` types, then you don't need to read this section.*
///
/// If `T: AsBytes`, then unsafe code may assume that:
/// - It is sound to treat any `t: T` as an immutable `[u8]` of length
///   `size_of_val(t)`.
/// - Given `t: &T`, it is sound to construct a `b: &[u8]` where `b.len() ==
///   size_of_val(t)` at the same address as `t`, and it is sound for both `b`
///   and `t` to be live at the same time.
///
/// If a type is marked as `AsBytes` which violates this contract, it may cause
/// undefined behavior.
///
/// `#[derive(AsBytes)]` only permits [types which satisfy these
/// requirements][derive-analysis].
///
#[cfg_attr(
    feature = "derive",
    doc = "[derive]: zerocopy_derive::AsBytes",
    doc = "[derive-analysis]: zerocopy_derive::AsBytes#analysis"
)]
#[cfg_attr(
    not(feature = "derive"),
    doc = concat!("[derive]: https://docs.rs/zerocopy/", env!("CARGO_PKG_VERSION"), "/zerocopy/derive.AsBytes.html"),
    doc = concat!("[derive-analysis]: https://docs.rs/zerocopy/", env!("CARGO_PKG_VERSION"), "/zerocopy/derive.AsBytes.html#analysis"),
)]
pub unsafe trait AsBytes {
    // The `Self: Sized` bound makes it so that this function doesn't prevent
    // `AsBytes` from being object safe. Note that other `AsBytes` methods
    // prevent object safety, but those provide a benefit in exchange for object
    // safety. If at some point we remove those methods, change their type
    // signatures, or move them out of this trait so that `AsBytes` is object
    // safe again, it's important that this function not prevent object safety.
    #[doc(hidden)]
    fn only_derive_is_allowed_to_implement_this_trait()
    where
        Self: Sized;

    /// Gets the bytes of this value.
    ///
    /// `as_bytes` provides access to the bytes of this value as an immutable
    /// byte slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::AsBytes;
    /// # use zerocopy_derive::*;
    ///
    /// #[derive(AsBytes)]
    /// #[repr(C)]
    /// struct PacketHeader {
    ///     src_port: [u8; 2],
    ///     dst_port: [u8; 2],
    ///     length: [u8; 2],
    ///     checksum: [u8; 2],
    /// }
    ///
    /// let header = PacketHeader {
    ///     src_port: [0, 1],
    ///     dst_port: [2, 3],
    ///     length: [4, 5],
    ///     checksum: [6, 7],
    /// };
    ///
    /// let bytes = header.as_bytes();
    ///
    /// assert_eq!(bytes, [0, 1, 2, 3, 4, 5, 6, 7]);
    /// ```
    #[inline(always)]
    fn as_bytes(&self) -> &[u8] {
        // Note that this method does not have a `Self: Sized` bound;
        // `size_of_val` works for unsized values too.
        let len = mem::size_of_val(self);
        let slf: *const Self = self;

        // SAFETY:
        // - `slf.cast::<u8>()` is valid for reads for `len *
        //   mem::size_of::<u8>()` many bytes because...
        //   - `slf` is the same pointer as `self`, and `self` is a reference
        //     which points to an object whose size is `len`. Thus...
        //     - The entire region of `len` bytes starting at `slf` is contained
        //       within a single allocation.
        //     - `slf` is non-null.
        //   - `slf` is trivially aligned to `align_of::<u8>() == 1`.
        // - `Self: AsBytes` ensures that all of the bytes of `slf` are
        //   initialized.
        // - Since `slf` is derived from `self`, and `self` is an immutable
        //   reference, the only other references to this memory region that
        //   could exist are other immutable references, and those don't allow
        //   mutation. `AsBytes` prohibits types which contain `UnsafeCell`s,
        //   which are the only types for which this rule wouldn't be sufficient.
        // - The total size of the resulting slice is no larger than
        //   `isize::MAX` because no allocation produced by safe code can be
        //   larger than `isize::MAX`.
        //
        // TODO(#429): Add references to docs and quotes.
        unsafe { slice::from_raw_parts(slf.cast::<u8>(), len) }
    }

    /// Gets the bytes of this value mutably.
    ///
    /// `as_bytes_mut` provides access to the bytes of this value as a mutable
    /// byte slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::AsBytes;
    /// # use zerocopy_derive::*;
    ///
    /// # #[derive(Eq, PartialEq, Debug)]
    /// #[derive(AsBytes, FromZeroes, FromBytes)]
    /// #[repr(C)]
    /// struct PacketHeader {
    ///     src_port: [u8; 2],
    ///     dst_port: [u8; 2],
    ///     length: [u8; 2],
    ///     checksum: [u8; 2],
    /// }
    ///
    /// let mut header = PacketHeader {
    ///     src_port: [0, 1],
    ///     dst_port: [2, 3],
    ///     length: [4, 5],
    ///     checksum: [6, 7],
    /// };
    ///
    /// let bytes = header.as_bytes_mut();
    ///
    /// assert_eq!(bytes, [0, 1, 2, 3, 4, 5, 6, 7]);
    ///
    /// bytes.reverse();
    ///
    /// assert_eq!(header, PacketHeader {
    ///     src_port: [7, 6],
    ///     dst_port: [5, 4],
    ///     length: [3, 2],
    ///     checksum: [1, 0],
    /// });
    /// ```
    #[inline(always)]
    fn as_bytes_mut(&mut self) -> &mut [u8]
    where
        Self: FromBytes,
    {
        // Note that this method does not have a `Self: Sized` bound;
        // `size_of_val` works for unsized values too.
        let len = mem::size_of_val(self);
        let slf: *mut Self = self;

        // SAFETY:
        // - `slf.cast::<u8>()` is valid for reads and writes for `len *
        //   mem::size_of::<u8>()` many bytes because...
        //   - `slf` is the same pointer as `self`, and `self` is a reference
        //     which points to an object whose size is `len`. Thus...
        //     - The entire region of `len` bytes starting at `slf` is contained
        //       within a single allocation.
        //     - `slf` is non-null.
        //   - `slf` is trivially aligned to `align_of::<u8>() == 1`.
        // - `Self: AsBytes` ensures that all of the bytes of `slf` are
        //   initialized.
        // - `Self: FromBytes` ensures that no write to this memory region
        //   could result in it containing an invalid `Self`.
        // - Since `slf` is derived from `self`, and `self` is a mutable
        //   reference, no other references to this memory region can exist.
        // - The total size of the resulting slice is no larger than
        //   `isize::MAX` because no allocation produced by safe code can be
        //   larger than `isize::MAX`.
        //
        // TODO(#429): Add references to docs and quotes.
        unsafe { slice::from_raw_parts_mut(slf.cast::<u8>(), len) }
    }

    /// Writes a copy of `self` to `bytes`.
    ///
    /// If `bytes.len() != size_of_val(self)`, `write_to` returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::AsBytes;
    /// # use zerocopy_derive::*;
    ///
    /// #[derive(AsBytes)]
    /// #[repr(C)]
    /// struct PacketHeader {
    ///     src_port: [u8; 2],
    ///     dst_port: [u8; 2],
    ///     length: [u8; 2],
    ///     checksum: [u8; 2],
    /// }
    ///
    /// let header = PacketHeader {
    ///     src_port: [0, 1],
    ///     dst_port: [2, 3],
    ///     length: [4, 5],
    ///     checksum: [6, 7],
    /// };
    ///
    /// let mut bytes = [0, 0, 0, 0, 0, 0, 0, 0];
    ///
    /// header.write_to(&mut bytes[..]);
    ///
    /// assert_eq!(bytes, [0, 1, 2, 3, 4, 5, 6, 7]);
    /// ```
    ///
    /// If too many or too few target bytes are provided, `write_to` returns
    /// `None` and leaves the target bytes unmodified:
    ///
    /// ```
    /// # use zerocopy::AsBytes;
    /// # let header = u128::MAX;
    /// let mut excessive_bytes = &mut [0u8; 128][..];
    ///
    /// let write_result = header.write_to(excessive_bytes);
    ///
    /// assert!(write_result.is_none());
    /// assert_eq!(excessive_bytes, [0u8; 128]);
    /// ```
    #[inline]
    fn write_to(&self, bytes: &mut [u8]) -> Option<()> {
        if bytes.len() != mem::size_of_val(self) {
            return None;
        }

        bytes.copy_from_slice(self.as_bytes());
        Some(())
    }

    /// Writes a copy of `self` to the prefix of `bytes`.
    ///
    /// `write_to_prefix` writes `self` to the first `size_of_val(self)` bytes
    /// of `bytes`. If `bytes.len() < size_of_val(self)`, it returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::AsBytes;
    /// # use zerocopy_derive::*;
    ///
    /// #[derive(AsBytes)]
    /// #[repr(C)]
    /// struct PacketHeader {
    ///     src_port: [u8; 2],
    ///     dst_port: [u8; 2],
    ///     length: [u8; 2],
    ///     checksum: [u8; 2],
    /// }
    ///
    /// let header = PacketHeader {
    ///     src_port: [0, 1],
    ///     dst_port: [2, 3],
    ///     length: [4, 5],
    ///     checksum: [6, 7],
    /// };
    ///
    /// let mut bytes = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    ///
    /// header.write_to_prefix(&mut bytes[..]);
    ///
    /// assert_eq!(bytes, [0, 1, 2, 3, 4, 5, 6, 7, 0, 0]);
    /// ```
    ///
    /// If insufficient target bytes are provided, `write_to_prefix` returns
    /// `None` and leaves the target bytes unmodified:
    ///
    /// ```
    /// # use zerocopy::AsBytes;
    /// # let header = u128::MAX;
    /// let mut insufficent_bytes = &mut [0, 0][..];
    ///
    /// let write_result = header.write_to_suffix(insufficent_bytes);
    ///
    /// assert!(write_result.is_none());
    /// assert_eq!(insufficent_bytes, [0, 0]);
    /// ```
    #[inline]
    fn write_to_prefix(&self, bytes: &mut [u8]) -> Option<()> {
        let size = mem::size_of_val(self);
        bytes.get_mut(..size)?.copy_from_slice(self.as_bytes());
        Some(())
    }

    /// Writes a copy of `self` to the suffix of `bytes`.
    ///
    /// `write_to_suffix` writes `self` to the last `size_of_val(self)` bytes of
    /// `bytes`. If `bytes.len() < size_of_val(self)`, it returns `None`.
    ///
    /// # Examples
    ///
    /// ```
    /// use zerocopy::AsBytes;
    /// # use zerocopy_derive::*;
    ///
    /// #[derive(AsBytes)]
    /// #[repr(C)]
    /// struct PacketHeader {
    ///     src_port: [u8; 2],
    ///     dst_port: [u8; 2],
    ///     length: [u8; 2],
    ///     checksum: [u8; 2],
    /// }
    ///
    /// let header = PacketHeader {
    ///     src_port: [0, 1],
    ///     dst_port: [2, 3],
    ///     length: [4, 5],
    ///     checksum: [6, 7],
    /// };
    ///
    /// let mut bytes = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
    ///
    /// header.write_to_suffix(&mut bytes[..]);
    ///
    /// assert_eq!(bytes, [0, 0, 0, 1, 2, 3, 4, 5, 6, 7]);
    ///
    /// let mut insufficent_bytes = &mut [0, 0][..];
    ///
    /// let write_result = header.write_to_suffix(insufficent_bytes);
    ///
    /// assert!(write_result.is_none());
    /// assert_eq!(insufficent_bytes, [0, 0]);
    /// ```
    ///
    /// If insufficient target bytes are provided, `write_to_suffix` returns
    /// `None` and leaves the target bytes unmodified:
    ///
    /// ```
    /// # use zerocopy::AsBytes;
    /// # let header = u128::MAX;
    /// let mut insufficent_bytes = &mut [0, 0][..];
    ///
    /// let write_result = header.write_to_suffix(insufficent_bytes);
    ///
    /// assert!(write_result.is_none());
    /// assert_eq!(insufficent_bytes, [0, 0]);
    /// ```
    #[inline]
    fn write_to_suffix(&self, bytes: &mut [u8]) -> Option<()> {
        let start = bytes.len().checked_sub(mem::size_of_val(self))?;
        bytes
            .get_mut(start..)
            .expect("`start` should be in-bounds of `bytes`")
            .copy_from_slice(self.as_bytes());
        Some(())
    }
}

/// Types with no alignment requirement.
///
/// WARNING: Do not implement this trait yourself! Instead, use
/// `#[derive(Unaligned)]` (requires the `derive` Cargo feature).
///
/// If `T: Unaligned`, then `align_of::<T>() == 1`.
///
/// # Safety
///
/// *This section describes what is required in order for `T: Unaligned`, and
/// what unsafe code may assume of such types. `#[derive(Unaligned)]` only
/// permits types which satisfy these requirements. If you don't plan on
/// implementing `Unaligned` manually, and you don't plan on writing unsafe code
/// that operates on `Unaligned` types, then you don't need to read this
/// section.*
///
/// If `T: Unaligned`, then unsafe code may assume that it is sound to produce a
/// reference to `T` at any memory location regardless of alignment. If a type
/// is marked as `Unaligned` which violates this contract, it may cause
/// undefined behavior.
pub unsafe trait Unaligned {
    // The `Self: Sized` bound makes it so that `Unaligned` is still object
    // safe.
    #[doc(hidden)]
    fn only_derive_is_allowed_to_implement_this_trait()
    where
        Self: Sized;
}

safety_comment! {
    /// SAFETY:
    /// Per the reference [1], "the unit tuple (`()`) ... is guaranteed as a
    /// zero-sized type to have a size of 0 and an alignment of 1."
    /// - `TryFromBytes` (with no validator), `FromZeroes`, `FromBytes`: There
    ///   is only one possible sequence of 0 bytes, and `()` is inhabited.
    /// - `AsBytes`: Since `()` has size 0, it contains no padding bytes.
    /// - `Unaligned`: `()` has alignment 1.
    ///
    /// [1] https://doc.rust-lang.org/reference/type-layout.html#tuple-layout
    unsafe_impl!((): TryFromBytes, FromZeroes, FromBytes, AsBytes, Unaligned);
    assert_unaligned!(());
}

safety_comment! {
    /// SAFETY:
    /// - `TryFromBytes` (with no validator), `FromZeroes`, `FromBytes`: all bit
    ///   patterns are valid for numeric types [1]
    /// - `AsBytes`: numeric types have no padding bytes [1]
    /// - `Unaligned` (`u8` and `i8` only): The reference [2] specifies the size
    ///   of `u8` and `i8` as 1 byte. We also know that:
    ///   - Alignment is >= 1 [3]
    ///   - Size is an integer multiple of alignment [4]
    ///   - The only value >= 1 for which 1 is an integer multiple is 1
    ///   Therefore, the only possible alignment for `u8` and `i8` is 1.
    ///
    /// [1] Per https://doc.rust-lang.org/beta/reference/types/numeric.html#bit-validity:
    ///
    ///     For every numeric type, `T`, the bit validity of `T` is equivalent to
    ///     the bit validity of `[u8; size_of::<T>()]`. An uninitialized byte is
    ///     not a valid `u8`.
    ///
    /// TODO(https://github.com/rust-lang/reference/pull/1392): Once this text
    /// is available on the Stable docs, cite those instead.
    ///
    /// [2] https://doc.rust-lang.org/reference/type-layout.html#primitive-data-layout
    ///
    /// [3] Per https://doc.rust-lang.org/reference/type-layout.html#size-and-alignment:
    ///
    ///     Alignment is measured in bytes, and must be at least 1.
    ///
    /// [4] Per https://doc.rust-lang.org/reference/type-layout.html#size-and-alignment:
    ///
    ///     The size of a value is always a multiple of its alignment.
    ///
    /// TODO(#278): Once we've updated the trait docs to refer to `u8`s rather
    /// than bits or bytes, update this comment, especially the reference to
    /// [1].
    unsafe_impl!(u8: TryFromBytes, FromZeroes, FromBytes, AsBytes, Unaligned);
    unsafe_impl!(i8: TryFromBytes, FromZeroes, FromBytes, AsBytes, Unaligned);
    assert_unaligned!(u8, i8);
    unsafe_impl!(u16: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(i16: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(u32: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(i32: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(u64: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(i64: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(u128: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(i128: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(usize: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(isize: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(f32: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(f64: TryFromBytes, FromZeroes, FromBytes, AsBytes);
}

safety_comment! {
    /// SAFETY:
    /// - `FromZeroes`: Valid since "[t]he value false has the bit pattern
    ///   0x00" [1].
    /// - `AsBytes`: Since "the boolean type has a size and alignment of 1 each"
    ///   and "The value false has the bit pattern 0x00 and the value true has
    ///   the bit pattern 0x01" [1]. Thus, the only byte of the bool is always
    ///   initialized.
    /// - `Unaligned`: Per the reference [1], "[a]n object with the boolean type
    ///   has a size and alignment of 1 each."
    ///
    /// [1] https://doc.rust-lang.org/reference/types/boolean.html
    unsafe_impl!(bool: FromZeroes, AsBytes, Unaligned);
    assert_unaligned!(bool);
    /// SAFETY:
    /// - The safety requirements for `unsafe_impl!` with an `is_bit_valid`
    ///   closure:
    ///   - Given `t: *mut bool` and `let r = *mut u8`, `r` refers to an object
    ///     of the same size as that referred to by `t`. This is true because
    ///     `bool` and `u8` have the same size (1 byte) [1].
    ///   - Since the closure takes a `&u8` argument, given a `Ptr<'a, bool>`
    ///     which satisfies the preconditions of
    ///     `TryFromBytes::<bool>::is_bit_valid`, it must be guaranteed that the
    ///     memory referenced by that `Ptr` always contains a valid `u8`. Since
    ///     `bool`'s single byte is always initialized, `is_bit_valid`'s
    ///     precondition requires that the same is true of its argument. Since
    ///     `u8`'s only bit validity invariant is that its single byte must be
    ///     initialized, this memory is guaranteed to contain a valid `u8`.
    ///   - The alignment of `bool` is equal to the alignment of `u8`. [1] [2]
    ///   - The impl must only return `true` for its argument if the original
    ///     `Ptr<bool>` refers to a valid `bool`. We only return true if the
    ///     `u8` value is 0 or 1, and both of these are valid values for `bool`.
    ///     [3]
    ///
    /// [1] Per https://doc.rust-lang.org/reference/type-layout.html#primitive-data-layout:
    ///
    ///   The size of most primitives is given in this table.
    ///
    ///   | Type      | `size_of::<Type>() ` |
    ///   |-----------|----------------------|
    ///   | `bool`    | 1                    |
    ///   | `u8`/`i8` | 1                    |
    ///
    /// [2] Per https://doc.rust-lang.org/reference/type-layout.html#size-and-alignment:
    ///
    ///   The size of a value is always a multiple of its alignment.
    ///
    /// [3] Per https://doc.rust-lang.org/reference/types/boolean.html:
    ///
    ///   The value false has the bit pattern 0x00 and the value true has the
    ///   bit pattern 0x01.
    unsafe_impl!(bool: TryFromBytes; |byte: &u8| *byte < 2);
}
safety_comment! {
    /// SAFETY:
    /// - `FromZeroes`: Per reference [1], "[a] value of type char is a Unicode
    ///   scalar value (i.e. a code point that is not a surrogate), represented
    ///   as a 32-bit unsigned word in the 0x0000 to 0xD7FF or 0xE000 to
    ///   0x10FFFF range" which contains 0x0000.
    /// - `AsBytes`: `char` is per reference [1] "represented as a 32-bit
    ///   unsigned word" (`u32`) which is `AsBytes`. Note that unlike `u32`, not
    ///   all bit patterns are valid for `char`.
    ///
    /// [1] https://doc.rust-lang.org/reference/types/textual.html
    unsafe_impl!(char: FromZeroes, AsBytes);
    /// SAFETY:
    /// - The safety requirements for `unsafe_impl!` with an `is_bit_valid`
    ///   closure:
    ///   - Given `t: *mut char` and `let r = *mut u32`, `r` refers to an object
    ///     of the same size as that referred to by `t`. This is true because
    ///     `char` and `u32` have the same size [1].
    ///   - Since the closure takes a `&u32` argument, given a `Ptr<'a, char>`
    ///     which satisfies the preconditions of
    ///     `TryFromBytes::<char>::is_bit_valid`, it must be guaranteed that the
    ///     memory referenced by that `Ptr` always contains a valid `u32`. Since
    ///     `char`'s bytes are always initialized [2], `is_bit_valid`'s
    ///     precondition requires that the same is true of its argument. Since
    ///     `u32`'s only bit validity invariant is that its bytes must be
    ///     initialized, this memory is guaranteed to contain a valid `u32`.
    ///   - The alignment of `char` is equal to the alignment of `u32`. [1]
    ///   - The impl must only return `true` for its argument if the original
    ///     `Ptr<char>` refers to a valid `char`. `char::from_u32` guarantees
    ///     that it returns `None` if its input is not a valid `char`. [3]
    ///
    /// [1] Per https://doc.rust-lang.org/nightly/reference/types/textual.html#layout-and-bit-validity:
    ///
    ///   `char` is guaranteed to have the same size and alignment as `u32` on
    ///   all platforms.
    ///
    /// [2] Per https://doc.rust-lang.org/core/primitive.char.html#method.from_u32:
    ///
    ///   Every byte of a `char` is guaranteed to be initialized.
    ///
    /// [3] Per https://doc.rust-lang.org/core/primitive.char.html#method.from_u32:
    ///
    ///   `from_u32()` will return `None` if the input is not a valid value for
    ///   a `char`.
    unsafe_impl!(char: TryFromBytes; |candidate: &u32| char::from_u32(*candidate).is_some());
}
safety_comment! {
    /// SAFETY:
    /// - `FromZeroes`, `AsBytes`, `Unaligned`: Per the reference [1], `str`
    ///   has the same layout as `[u8]`, and `[u8]` is `FromZeroes`, `AsBytes`,
    ///   and `Unaligned`.
    ///
    /// Note that we don't `assert_unaligned!(str)` because `assert_unaligned!`
    /// uses `align_of`, which only works for `Sized` types.
    ///
    /// TODO(#429): Add quotes from documentation.
    ///
    /// [1] https://doc.rust-lang.org/reference/type-layout.html#str-layout
    unsafe_impl!(str: FromZeroes, AsBytes, Unaligned);
    /// SAFETY:
    /// - The safety requirements for `unsafe_impl!` with an `is_bit_valid`
    ///   closure:
    ///   - Given `t: *mut str` and `let r = *mut [u8]`, `r` refers to an object
    ///     of the same size as that referred to by `t`. This is true because
    ///     `str` and `[u8]` have the same representation. [1]
    ///   - Since the closure takes a `&[u8]` argument, given a `Ptr<'a, str>`
    ///     which satisfies the preconditions of
    ///     `TryFromBytes::<str>::is_bit_valid`, it must be guaranteed that the
    ///     memory referenced by that `Ptr` always contains a valid `[u8]`.
    ///     Since `str`'s bytes are always initialized [1], `is_bit_valid`'s
    ///     precondition requires that the same is true of its argument. Since
    ///     `[u8]`'s only bit validity invariant is that its bytes must be
    ///     initialized, this memory is guaranteed to contain a valid `[u8]`.
    ///   - The alignment of `str` is equal to the alignment of `[u8]`. [1]
    ///   - The impl must only return `true` for its argument if the original
    ///     `Ptr<str>` refers to a valid `str`. `str::from_utf8` guarantees that
    ///     it returns `Err` if its input is not a valid `str`. [2]
    ///
    /// [1] Per https://doc.rust-lang.org/reference/types/textual.html:
    ///
    ///   A value of type `str` is represented the same was as `[u8]`.
    ///
    /// [2] Per https://doc.rust-lang.org/core/str/fn.from_utf8.html#errors:
    ///
    ///   Returns `Err` if the slice is not UTF-8.
    unsafe_impl!(str: TryFromBytes; |candidate: &[u8]| core::str::from_utf8(candidate).is_ok());
}

safety_comment! {
    // `NonZeroXxx` is `AsBytes`, but not `FromZeroes` or `FromBytes`.
    //
    /// SAFETY:
    /// - `AsBytes`: `NonZeroXxx` has the same layout as its associated
    ///    primitive. Since it is the same size, this guarantees it has no
    ///    padding - integers have no padding, and there's no room for padding
    ///    if it can represent all of the same values except 0.
    /// - `Unaligned`: `NonZeroU8` and `NonZeroI8` document that
    ///   `Option<NonZeroU8>` and `Option<NonZeroI8>` both have size 1. [1] [2]
    ///   This is worded in a way that makes it unclear whether it's meant as a
    ///   guarantee, but given the purpose of those types, it's virtually
    ///   unthinkable that that would ever change. `Option` cannot be smaller
    ///   than its contained type, which implies that, and `NonZeroX8` are of
    ///   size 1 or 0. `NonZeroX8` can represent multiple states, so they cannot
    ///   be 0 bytes, which means that they must be 1 byte. The only valid
    ///   alignment for a 1-byte type is 1.
    ///
    /// TODO(#429): Add quotes from documentation.
    ///
    /// [1] https://doc.rust-lang.org/stable/std/num/struct.NonZeroU8.html
    /// [2] https://doc.rust-lang.org/stable/std/num/struct.NonZeroI8.html
    /// TODO(https://github.com/rust-lang/rust/pull/104082): Cite documentation
    /// that layout is the same as primitive layout.
    unsafe_impl!(NonZeroU8: AsBytes, Unaligned);
    unsafe_impl!(NonZeroI8: AsBytes, Unaligned);
    assert_unaligned!(NonZeroU8, NonZeroI8);
    unsafe_impl!(NonZeroU16: AsBytes);
    unsafe_impl!(NonZeroI16: AsBytes);
    unsafe_impl!(NonZeroU32: AsBytes);
    unsafe_impl!(NonZeroI32: AsBytes);
    unsafe_impl!(NonZeroU64: AsBytes);
    unsafe_impl!(NonZeroI64: AsBytes);
    unsafe_impl!(NonZeroU128: AsBytes);
    unsafe_impl!(NonZeroI128: AsBytes);
    unsafe_impl!(NonZeroUsize: AsBytes);
    unsafe_impl!(NonZeroIsize: AsBytes);
    /// SAFETY:
    /// - The safety requirements for `unsafe_impl!` with an `is_bit_valid`
    ///   closure:
    ///   - Given `t: *mut NonZeroXxx` and `let r = *mut xxx`, `r` refers to an
    ///     object of the same size as that referred to by `t`. This is true
    ///     because `NonZeroXxx` and `xxx` have the same size. [1]
    ///   - Since the closure takes a `&xxx` argument, given a `Ptr<'a,
    ///     NonZeroXxx>` which satisfies the preconditions of
    ///     `TryFromBytes::<NonZeroXxx>::is_bit_valid`, it must be guaranteed
    ///     that the memory referenced by that `Ptr` always contains a valid
    ///     `xxx`. Since `NonZeroXxx`'s bytes are always initialized [1],
    ///     `is_bit_valid`'s precondition requires that the same is true of its
    ///     argument. Since `xxx`'s only bit validity invariant is that its
    ///     bytes must be initialized, this memory is guaranteed to contain a
    ///     valid `xxx`.
    ///   - The alignment of `NonZeroXxx` is equal to the alignment of `xxx`.
    ///     [1]
    ///   - The impl must only return `true` for its argument if the original
    ///     `Ptr<NonZeroXxx>` refers to a valid `NonZeroXxx`. The only `xxx`
    ///     which is not also a valid `NonZeroXxx` is 0. [1]
    ///
    /// [1] Per https://doc.rust-lang.org/core/num/struct.NonZeroU16.html:
    ///
    ///   `NonZeroU16` is guaranteed to have the same layout and bit validity as
    ///   `u16` with the exception that `0` is not a valid instance.
    unsafe_impl!(NonZeroU8: TryFromBytes; |n: &u8| *n != 0);
    unsafe_impl!(NonZeroI8: TryFromBytes; |n: &i8| *n != 0);
    unsafe_impl!(NonZeroU16: TryFromBytes; |n: &u16| *n != 0);
    unsafe_impl!(NonZeroI16: TryFromBytes; |n: &i16| *n != 0);
    unsafe_impl!(NonZeroU32: TryFromBytes; |n: &u32| *n != 0);
    unsafe_impl!(NonZeroI32: TryFromBytes; |n: &i32| *n != 0);
    unsafe_impl!(NonZeroU64: TryFromBytes; |n: &u64| *n != 0);
    unsafe_impl!(NonZeroI64: TryFromBytes; |n: &i64| *n != 0);
    unsafe_impl!(NonZeroU128: TryFromBytes; |n: &u128| *n != 0);
    unsafe_impl!(NonZeroI128: TryFromBytes; |n: &i128| *n != 0);
    unsafe_impl!(NonZeroUsize: TryFromBytes; |n: &usize| *n != 0);
    unsafe_impl!(NonZeroIsize: TryFromBytes; |n: &isize| *n != 0);
}
safety_comment! {
    /// SAFETY:
    /// - `TryFromBytes` (with no validator), `FromZeroes`, `FromBytes`,
    ///   `AsBytes`: The Rust compiler reuses `0` value to represent `None`, so
    ///   `size_of::<Option<NonZeroXxx>>() == size_of::<xxx>()`; see
    ///   `NonZeroXxx` documentation.
    /// - `Unaligned`: `NonZeroU8` and `NonZeroI8` document that
    ///   `Option<NonZeroU8>` and `Option<NonZeroI8>` both have size 1. [1] [2]
    ///   This is worded in a way that makes it unclear whether it's meant as a
    ///   guarantee, but given the purpose of those types, it's virtually
    ///   unthinkable that that would ever change. The only valid alignment for
    ///   a 1-byte type is 1.
    ///
    /// TODO(#429): Add quotes from documentation.
    ///
    /// [1] https://doc.rust-lang.org/stable/std/num/struct.NonZeroU8.html
    /// [2] https://doc.rust-lang.org/stable/std/num/struct.NonZeroI8.html
    ///
    /// TODO(https://github.com/rust-lang/rust/pull/104082): Cite documentation
    /// for layout guarantees.
    unsafe_impl!(Option<NonZeroU8>: TryFromBytes, FromZeroes, FromBytes, AsBytes, Unaligned);
    unsafe_impl!(Option<NonZeroI8>: TryFromBytes, FromZeroes, FromBytes, AsBytes, Unaligned);
    assert_unaligned!(Option<NonZeroU8>, Option<NonZeroI8>);
    unsafe_impl!(Option<NonZeroU16>: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(Option<NonZeroI16>: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(Option<NonZeroU32>: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(Option<NonZeroI32>: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(Option<NonZeroU64>: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(Option<NonZeroI64>: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(Option<NonZeroU128>: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(Option<NonZeroI128>: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(Option<NonZeroUsize>: TryFromBytes, FromZeroes, FromBytes, AsBytes);
    unsafe_impl!(Option<NonZeroIsize>: TryFromBytes, FromZeroes, FromBytes, AsBytes);
}

safety_comment! {
    /// SAFETY:
    /// The following types can be transmuted from `[0u8; size_of::<T>()]`. [1]
    /// None of them contain `UnsafeCell`s, and so they all soundly implement
    /// `FromZeroes`.
    ///
    /// [1] Per
    /// https://doc.rust-lang.org/nightly/core/option/index.html#representation:
    ///
    ///   Rust guarantees to optimize the following types `T` such that
    ///   [`Option<T>`] has the same size and alignment as `T`. In some of these
    ///   cases, Rust further guarantees that `transmute::<_, Option<T>>([0u8;
    ///   size_of::<T>()])` is sound and produces `Option::<T>::None`. These
    ///   cases are identified by the second column:
    ///
    ///   | `T`                   | `transmute::<_, Option<T>>([0u8; size_of::<T>()])` sound? |
    ///   |-----------------------|-----------------------------------------------------------|
    ///   | [`Box<U>`]            | when `U: Sized`                                           |
    ///   | `&U`                  | when `U: Sized`                                           |
    ///   | `&mut U`              | when `U: Sized`                                           |
    ///   | [`ptr::NonNull<U>`]   | when `U: Sized`                                           |
    ///   | `fn`, `extern "C" fn` | always                                                    |
    ///
    /// TODO(#429), TODO(https://github.com/rust-lang/rust/pull/115333): Cite
    /// the Stable docs once they're available.
    #[cfg(feature = "alloc")]
    unsafe_impl!(
        #[cfg_attr(doc_cfg, doc(cfg(feature = "alloc")))]
        T => FromZeroes for Option<Box<T>>
    );
    unsafe_impl!(T => FromZeroes for Option<&'_ T>);
    unsafe_impl!(T => FromZeroes for Option<&'_ mut T>);
    unsafe_impl!(T => FromZeroes for Option<NonNull<T>>);
    unsafe_impl_for_power_set!(A, B, C, D, E, F, G, H, I, J, K, L -> M => FromZeroes for opt_fn!(...));
    unsafe_impl_for_power_set!(A, B, C, D, E, F, G, H, I, J, K, L -> M => FromZeroes for opt_extern_c_fn!(...));
}

safety_comment! {
    /// SAFETY:
    /// Per reference [1]:
    /// "For all T, the following are guaranteed:
    /// size_of::<PhantomData<T>>() == 0
    /// align_of::<PhantomData<T>>() == 1".
    /// This gives:
    /// - `TryFromBytes` (with no validator), `FromZeroes`, `FromBytes`: There
    ///   is only one possible sequence of 0 bytes, and `PhantomData` is
    ///   inhabited.
    /// - `AsBytes`: Since `PhantomData` has size 0, it contains no padding
    ///   bytes.
    /// - `Unaligned`: Per the preceding reference, `PhantomData` has alignment
    ///   1.
    ///
    /// [1] https://doc.rust-lang.org/std/marker/struct.PhantomData.html#layout-1
    unsafe_impl!(T: ?Sized => TryFromBytes for PhantomData<T>);
    unsafe_impl!(T: ?Sized => FromZeroes for PhantomData<T>);
    unsafe_impl!(T: ?Sized => FromBytes for PhantomData<T>);
    unsafe_impl!(T: ?Sized => AsBytes for PhantomData<T>);
    unsafe_impl!(T: ?Sized => Unaligned for PhantomData<T>);
    assert_unaligned!(PhantomData<()>, PhantomData<u8>, PhantomData<u64>);
}
safety_comment! {
    /// SAFETY:
    /// `Wrapping<T>` is guaranteed by its docs [1] to have the same layout and
    /// bit validity as `T`. Also, `Wrapping<T>` is `#[repr(transparent)]`, and
    /// has a single field, which is `pub`. Per the reference [2], this means
    /// that the `#[repr(transparent)]` attribute is "considered part of the
    /// public ABI".
    ///
    /// - `TryFromBytes`: The safety requirements for `unsafe_impl!` with an
    ///   `is_bit_valid` closure:
    ///   - Given `t: *mut Wrapping<T>` and `let r = *mut T`, `r` refers to an
    ///     object of the same size as that referred to by `t`. This is true
    ///     because `Wrapping<T>` and `T` have the same layout
    ///   - The alignment of `Wrapping<T>` is equal to the alignment of `T`.
    ///   - The impl must only return `true` for its argument if the original
    ///     `Ptr<Wrapping<T>>` refers to a valid `Wrapping<T>`. Since
    ///     `Wrapping<T>` has the same bit validity as `T`, and since our impl
    ///     just calls `T::is_bit_valid`, our impl returns `true` exactly when
    ///     its argument contains a valid `Wrapping<T>`.
    /// - `FromBytes`: Since `Wrapping<T>` has the same bit validity as `T`, if
    ///   `T: FromBytes`, then all initialized byte sequences are valid
    ///   instances of `Wrapping<T>`. Similarly, if `T: FromBytes`, then
    ///   `Wrapping<T>` doesn't contain any `UnsafeCell`s. Thus, `impl FromBytes
    ///   for Wrapping<T> where T: FromBytes` is a sound impl.
    /// - `AsBytes`: Since `Wrapping<T>` has the same bit validity as `T`, if
    ///   `T: AsBytes`, then all valid instances of `Wrapping<T>` have all of
    ///   their bytes initialized. Similarly, if `T: AsBytes`, then
    ///   `Wrapping<T>` doesn't contain any `UnsafeCell`s. Thus, `impl AsBytes
    ///   for Wrapping<T> where T: AsBytes` is a valid impl.
    /// - `Unaligned`: Since `Wrapping<T>` has the same layout as `T`,
    ///   `Wrapping<T>` has alignment 1 exactly when `T` does.
    ///
    /// [1] Per https://doc.rust-lang.org/core/num/struct.NonZeroU16.html:
    ///
    ///   `NonZeroU16` is guaranteed to have the same layout and bit validity as
    ///   `u16` with the exception that `0` is not a valid instance.
    ///
    /// TODO(#429): Add quotes from documentation.
    ///
    /// [1] TODO(https://doc.rust-lang.org/nightly/core/num/struct.Wrapping.html#layout-1):
    /// Reference this documentation once it's available on stable.
    ///
    /// [2] https://doc.rust-lang.org/nomicon/other-reprs.html#reprtransparent
    unsafe_impl!(T: TryFromBytes => TryFromBytes for Wrapping<T>; |candidate: Ptr<T>| {
        // SAFETY:
        // - Since `T` and `Wrapping<T>` have the same layout and bit validity
        //   and contain the same fields, `T` contains `UnsafeCell`s exactly
        //   where `Wrapping<T>` does. Thus, all memory and `UnsafeCell`
        //   preconditions of `T::is_bit_valid` hold exactly when the same
        //   preconditions for `Wrapping<T>::is_bit_valid` hold.
        // - By the same token, since `candidate` is guaranteed to have its
        //   bytes initialized where there are always initialized bytes in
        //   `Wrapping<T>`, the same is true for `T`.
        unsafe { T::is_bit_valid(candidate) }
    });
    unsafe_impl!(T: FromZeroes => FromZeroes for Wrapping<T>);
    unsafe_impl!(T: FromBytes => FromBytes for Wrapping<T>);
    unsafe_impl!(T: AsBytes => AsBytes for Wrapping<T>);
    unsafe_impl!(T: Unaligned => Unaligned for Wrapping<T>);
    assert_unaligned!(Wrapping<()>, Wrapping<u8>);
}
safety_comment! {
    // `MaybeUninit<T>` is `FromZeroes` and `FromBytes`, but never `AsBytes`
    // since it may contain uninitialized bytes.
    //
    /// SAFETY:
    /// - `TryFromBytes` (with no validator), `FromZeroes`, `FromBytes`:
    ///   `MaybeUninit<T>` has no restrictions on its contents. Unfortunately,
    ///   in addition to bit validity, `TryFromBytes`, `FromZeroes` and
    ///   `FromBytes` also require that implementers contain no `UnsafeCell`s.
    ///   Thus, we require `T: Trait` in order to ensure that `T` - and thus
    ///   `MaybeUninit<T>` - contains to `UnsafeCell`s. Thus, requiring that `T`
    ///   implement each of these traits is sufficient.
    /// - `Unaligned`: "MaybeUninit<T> is guaranteed to have the same size,
    ///    alignment, and ABI as T" [1]
    ///
    /// [1] https://doc.rust-lang.org/stable/core/mem/union.MaybeUninit.html#layout-1
    ///
    /// TODO(https://github.com/google/zerocopy/issues/251): If we split
    /// `FromBytes` and `RefFromBytes`, or if we introduce a separate
    /// `NoCell`/`Freeze` trait, we can relax the trait bounds for `FromZeroes`
    /// and `FromBytes`.
    unsafe_impl!(T: TryFromBytes => TryFromBytes for MaybeUninit<T>);
    unsafe_impl!(T: FromZeroes => FromZeroes for MaybeUninit<T>);
    unsafe_impl!(T: FromBytes => FromBytes for MaybeUninit<T>);
    unsafe_impl!(T: Unaligned => Unaligned for MaybeUninit<T>);
    assert_unaligned!(MaybeUninit<()>, MaybeUninit<u8>);
}
safety_comment! {
    /// SAFETY:
    /// `ManuallyDrop` has the same layout and bit validity as `T` [1], and
    /// accessing the inner value is safe (meaning that it's unsound to leave
    /// the inner value uninitialized while exposing the `ManuallyDrop` to safe
    /// code).
    /// - `FromZeroes`, `FromBytes`: Since it has the same layout as `T`, any
    ///   valid `T` is a valid `ManuallyDrop<T>`. If `T: FromZeroes`, a sequence
    ///   of zero bytes is a valid `T`, and thus a valid `ManuallyDrop<T>`. If
    ///   `T: FromBytes`, any sequence of bytes is a valid `T`, and thus a valid
    ///   `ManuallyDrop<T>`.
    /// - `AsBytes`: Since it has the same layout as `T`, and since it's unsound
    ///   to let safe code access a `ManuallyDrop` whose inner value is
    ///   uninitialized, safe code can only ever access a `ManuallyDrop` whose
    ///   contents are a valid `T`. Since `T: AsBytes`, this means that safe
    ///   code can only ever access a `ManuallyDrop` with all initialized bytes.
    /// - `Unaligned`: `ManuallyDrop` has the same layout (and thus alignment)
    ///   as `T`, and `T: Unaligned` guarantees that that alignment is 1.
    ///
    ///   `ManuallyDrop<T>` is guaranteed to have the same layout and bit
    ///   validity as `T`
    ///
    /// [1] Per https://doc.rust-lang.org/nightly/core/mem/struct.ManuallyDrop.html:
    ///
    /// TODO(#429):
    /// - Add quotes from docs.
    /// - Once [1] (added in
    /// https://github.com/rust-lang/rust/pull/115522) is available on stable,
    /// quote the stable docs instead of the nightly docs.
    unsafe_impl!(T: ?Sized + FromZeroes => FromZeroes for ManuallyDrop<T>);
    unsafe_impl!(T: ?Sized + FromBytes => FromBytes for ManuallyDrop<T>);
    unsafe_impl!(T: ?Sized + AsBytes => AsBytes for ManuallyDrop<T>);
    unsafe_impl!(T: ?Sized + Unaligned => Unaligned for ManuallyDrop<T>);
    assert_unaligned!(ManuallyDrop<()>, ManuallyDrop<u8>);
}
safety_comment! {
    /// SAFETY:
    /// Per the reference [1]:
    ///
    ///   An array of `[T; N]` has a size of `size_of::<T>() * N` and the same
    ///   alignment of `T`. Arrays are laid out so that the zero-based `nth`
    ///   element of the array is offset from the start of the array by `n *
    ///   size_of::<T>()` bytes.
    ///
    ///   ...
    ///
    ///   Slices have the same layout as the section of the array they slice.
    ///
    /// In other words, the layout of a `[T]` or `[T; N]` is a sequence of `T`s
    /// laid out back-to-back with no bytes in between. Therefore, `[T]` or `[T;
    /// N]` are `TryFromBytes`, `FromZeroes`, `FromBytes`, and `AsBytes` if `T`
    /// is (respectively). Furthermore, since an array/slice has "the same
    /// alignment of `T`", `[T]` and `[T; N]` are `Unaligned` if `T` is.
    ///
    /// Note that we don't `assert_unaligned!` for slice types because
    /// `assert_unaligned!` uses `align_of`, which only works for `Sized` types.
    ///
    /// [1] https://doc.rust-lang.org/reference/type-layout.html#array-layout
    unsafe_impl!(const N: usize, T: FromZeroes => FromZeroes for [T; N]);
    unsafe_impl!(const N: usize, T: FromBytes => FromBytes for [T; N]);
    unsafe_impl!(const N: usize, T: AsBytes => AsBytes for [T; N]);
    unsafe_impl!(const N: usize, T: Unaligned => Unaligned for [T; N]);
    assert_unaligned!([(); 0], [(); 1], [u8; 0], [u8; 1]);
    unsafe_impl!(T: TryFromBytes => TryFromBytes for [T]; |c: Ptr<[T]>| {
        // SAFETY: Assuming the preconditions of `is_bit_valid` are satisfied,
        // so too will the postcondition: that, if `is_bit_valid(candidate)`
        // returns true, `*candidate` contains a valid `Self`. Per the reference
        // [1]:
        //
        //   An array of `[T; N]` has a size of `size_of::<T>() * N` and the
        //   same alignment of `T`. Arrays are laid out so that the zero-based
        //   `nth` element of the array is offset from the start of the array by
        //   `n * size_of::<T>()` bytes.
        //
        //   ...
        //
        //   Slices have the same layout as the section of the array they slice.
        //
        // In other words, the layout of a `[T] is a sequence of `T`s laid out
        // back-to-back with no bytes in between. If all elements in `candidate`
        // are `is_bit_valid`, so too is `candidate`.
        //
        // Note that any of the below calls may panic, but it would still be
        // sound even if it did. `is_bit_valid` does not promise that it will
        // not panic (in fact, it explicitly warns that it's a possibility), and
        // we have not violated any safety invariants that we must fix before
        // returning.
        c.iter().all(|elem|
            // SAFETY: We uphold the safety contract of `is_bit_valid(elem)`, by
            // precondition on the surrounding call to `is_bit_valid`. The
            // memory referenced by `elem` is contained entirely within `c`, and
            // satisfies the preconditions satisfied by `c`. By axiom, we assume
            // that `Iterator:all` does not invalidate these preconditions
            // (e.g., by writing to `elem`.) Since `elem` is derived from `c`,
            // it is only possible for uninitialized bytes to occur in `elem` at
            // the same bytes they occur within `c`.
            unsafe { <T as TryFromBytes>::is_bit_valid(elem) }
        )
    });
    unsafe_impl!(T: FromZeroes => FromZeroes for [T]);
    unsafe_impl!(T: FromBytes => FromBytes for [T]);
    unsafe_impl!(T: AsBytes => AsBytes for [T]);
    unsafe_impl!(T: Unaligned => Unaligned for [T]);
}
safety_comment! {
    /// SAFETY:
    /// - `FromZeroes`: For thin pointers (note that `T: Sized`), the zero
    ///   pointer is considered "null". [1] No operations which require
    ///   provenance are legal on null pointers, so this is not a footgun.
    ///
    /// NOTE(#170): Implementing `FromBytes` and `AsBytes` for raw pointers
    /// would be sound, but carries provenance footguns. We want to support
    /// `FromBytes` and `AsBytes` for raw pointers eventually, but we are
    /// holding off until we can figure out how to address those footguns.
    ///
    /// [1] TODO(https://github.com/rust-lang/rust/pull/116988): Cite the
    /// documentation once this PR lands.
    unsafe_impl!(T => FromZeroes for *const T);
    unsafe_impl!(T => FromZeroes for *mut T);
}

// SIMD support
//
// Per the Unsafe Code Guidelines Reference [1]:
//
//   Packed SIMD vector types are `repr(simd)` homogeneous tuple-structs
//   containing `N` elements of type `T` where `N` is a power-of-two and the
//   size and alignment requirements of `T` are equal:
//
//   ```rust
//   #[repr(simd)]
//   struct Vector<T, N>(T_0, ..., T_(N - 1));
//   ```
//
//   ...
//
//   The size of `Vector` is `N * size_of::<T>()` and its alignment is an
//   implementation-defined function of `T` and `N` greater than or equal to
//   `align_of::<T>()`.
//
//   ...
//
//   Vector elements are laid out in source field order, enabling random access
//   to vector elements by reinterpreting the vector as an array:
//
//   ```rust
//   union U {
//      vec: Vector<T, N>,
//      arr: [T; N]
//   }
//
//   assert_eq!(size_of::<Vector<T, N>>(), size_of::<[T; N]>());
//   assert!(align_of::<Vector<T, N>>() >= align_of::<[T; N]>());
//
//   unsafe {
//     let u = U { vec: Vector<T, N>(t_0, ..., t_(N - 1)) };
//
//     assert_eq!(u.vec.0, u.arr[0]);
//     // ...
//     assert_eq!(u.vec.(N - 1), u.arr[N - 1]);
//   }
//   ```
//
// Given this background, we can observe that:
// - The size and bit pattern requirements of a SIMD type are equivalent to the
//   equivalent array type. Thus, for any SIMD type whose primitive `T` is
//   `TryFromBytes`, `FromZeroes`, `FromBytes`, or `AsBytes`, that SIMD type is
//   also `TryFromBytes`, `FromZeroes`, `FromBytes`, or `AsBytes` respectively.
// - Since no upper bound is placed on the alignment, no SIMD type can be
//   guaranteed to be `Unaligned`.
//
// Also per [1]:
//
//   This chapter represents the consensus from issue #38. The statements in
//   here are not (yet) "guaranteed" not to change until an RFC ratifies them.
//
// See issue #38 [2]. While this behavior is not technically guaranteed, the
// likelihood that the behavior will change such that SIMD types are no longer
// `TryFromBytes`, `FromZeroes`, `FromBytes`, or `AsBytes` is next to zero, as
// that would defeat the entire purpose of SIMD types. Nonetheless, we put this
// behavior behind the `simd` Cargo feature, which requires consumers to opt
// into this stability hazard.
//
// [1] https://rust-lang.github.io/unsafe-code-guidelines/layout/packed-simd-vectors.html
// [2] https://github.com/rust-lang/unsafe-code-guidelines/issues/38
#[cfg(feature = "simd")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "simd")))]
mod simd {
    /// Defines a module which implements `TryFromBytes`, `FromZeroes`,
    /// `FromBytes`, and `AsBytes` for a set of types from a module in
    /// `core::arch`.
    ///
    /// `$arch` is both the name of the defined module and the name of the
    /// module in `core::arch`, and `$typ` is the list of items from that module
    /// to implement `FromZeroes`, `FromBytes`, and `AsBytes` for.
    // `allow(unused_macros)` is needed because some
    // target/feature combinations don't emit any impls
    // and thus don't use this macro.
    macro_rules! simd_arch_mod {
        (#[cfg $cfg:tt] $arch:ident, $mod:ident, $($typ:ident),*) => {
            #[cfg $cfg]
            #[cfg_attr(doc_cfg, doc(cfg $cfg))]
            mod $mod {
                use core::arch::$arch::{$($typ),*};

                use crate::*;
                impl_known_layout!($($typ),*);
                safety_comment! {
                    /// SAFETY:
                    /// See comment on module definition for justification.
                    $( unsafe_impl!($typ: TryFromBytes, FromZeroes, FromBytes, AsBytes); )*
                }
            }
        };
    }

    #[rustfmt::skip]
    const _: () = {
        simd_arch_mod!(
            #[cfg(target_arch = "x86")]
            x86, x86, __m128, __m128d, __m128i, __m256, __m256d, __m256i
        );
        simd_arch_mod!(
            #[cfg(all(feature = "simd-nightly", target_arch = "x86"))]
            x86, x86_nightly, __m512bh, __m512, __m512d, __m512i
        );
        simd_arch_mod!(
            #[cfg(target_arch = "x86_64")]
            x86_64, x86_64, __m128, __m128d, __m128i, __m256, __m256d, __m256i
        );
        simd_arch_mod!(
            #[cfg(all(feature = "simd-nightly", target_arch = "x86_64"))]
            x86_64, x86_64_nightly, __m512bh, __m512, __m512d, __m512i
        );
        simd_arch_mod!(
            #[cfg(target_arch = "wasm32")]
            wasm32, wasm32, v128
        );
        simd_arch_mod!(
            #[cfg(all(feature = "simd-nightly", target_arch = "powerpc"))]
            powerpc, powerpc, vector_bool_long, vector_double, vector_signed_long, vector_unsigned_long
        );
        simd_arch_mod!(
            #[cfg(all(feature = "simd-nightly", target_arch = "powerpc64"))]
            powerpc64, powerpc64, vector_bool_long, vector_double, vector_signed_long, vector_unsigned_long
        );
        simd_arch_mod!(
            #[cfg(target_arch = "aarch64")]
            aarch64, aarch64, float32x2_t, float32x4_t, float64x1_t, float64x2_t, int8x8_t, int8x8x2_t,
            int8x8x3_t, int8x8x4_t, int8x16_t, int8x16x2_t, int8x16x3_t, int8x16x4_t, int16x4_t,
            int16x8_t, int32x2_t, int32x4_t, int64x1_t, int64x2_t, poly8x8_t, poly8x8x2_t, poly8x8x3_t,
            poly8x8x4_t, poly8x16_t, poly8x16x2_t, poly8x16x3_t, poly8x16x4_t, poly16x4_t, poly16x8_t,
            poly64x1_t, poly64x2_t, uint8x8_t, uint8x8x2_t, uint8x8x3_t, uint8x8x4_t, uint8x16_t,
            uint8x16x2_t, uint8x16x3_t, uint8x16x4_t, uint16x4_t, uint16x8_t, uint32x2_t, uint32x4_t,
            uint64x1_t, uint64x2_t
        );
        simd_arch_mod!(
            #[cfg(all(feature = "simd-nightly", target_arch = "arm"))]
            arm, arm, int8x4_t, uint8x4_t
        );
    };
}

/// Safely transmutes a value of one type to a value of another type of the same
/// size.
///
/// The expression `$e` must have a concrete type, `T`, which implements
/// `AsBytes`. The `transmute!` expression must also have a concrete type, `U`
/// (`U` is inferred from the calling context), and `U` must implement
/// `FromBytes`.
///
/// Note that the `T` produced by the expression `$e` will *not* be dropped.
/// Semantically, its bits will be copied into a new value of type `U`, the
/// original `T` will be forgotten, and the value of type `U` will be returned.
///
/// # Examples
///
/// ```
/// # use zerocopy::transmute;
/// let one_dimensional: [u8; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
///
/// let two_dimensional: [[u8; 4]; 2] = transmute!(one_dimensional);
///
/// assert_eq!(two_dimensional, [[0, 1, 2, 3], [4, 5, 6, 7]]);
/// ```
#[macro_export]
macro_rules! transmute {
    ($e:expr) => {{
        // NOTE: This must be a macro (rather than a function with trait bounds)
        // because there's no way, in a generic context, to enforce that two
        // types have the same size. `core::mem::transmute` uses compiler magic
        // to enforce this so long as the types are concrete.

        let e = $e;
        if false {
            // This branch, though never taken, ensures that the type of `e` is
            // `AsBytes` and that the type of this macro invocation expression
            // is `FromBytes`.

            struct AssertIsAsBytes<T: $crate::AsBytes>(T);
            let _ = AssertIsAsBytes(e);

            struct AssertIsFromBytes<U: $crate::FromBytes>(U);

            let u = AssertIsFromBytes(loop {});
            u.0
        } else {
            // SAFETY: `core::mem::transmute` ensures that the type of `e` and
            // the type of this macro invocation expression have the same size.
            // We know this transmute is safe thanks to the `AsBytes` and
            // `FromBytes` bounds enforced by the `false` branch.
            //
            // We use this reexport of `core::mem::transmute` because we know it
            // will always be available for crates which are using the 2015
            // edition of Rust. By contrast, if we were to use
            // `std::mem::transmute`, this macro would not work for such crates
            // in `no_std` contexts, and if we were to use
            // `core::mem::transmute`, this macro would not work in `std`
            // contexts in which `core` was not manually imported. This is not a
            // problem for 2018 edition crates.
            unsafe {
                // Clippy: It's okay to transmute a type to itself.

                $crate::macro_util::core_reexport::mem::transmute(e)
            }
        }
    }}
}

/// Safely transmutes a mutable or immutable reference of one type to an
/// immutable reference of another type of the same size.
///
/// The expression `$e` must have a concrete type, `&T` or `&mut T`, where `T:
/// Sized + AsBytes`. The `transmute_ref!` expression must also have a concrete
/// type, `&U` (`U` is inferred from the calling context), where `U: Sized +
/// FromBytes`. It must be the case that `align_of::<T>() >= align_of::<U>()`.
///
/// The lifetime of the input type, `&T` or `&mut T`, must be the same as or
/// outlive the lifetime of the output type, `&U`.
///
/// # Examples
///
/// ```
/// # use zerocopy::transmute_ref;
/// let one_dimensional: [u8; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
///
/// let two_dimensional: &[[u8; 4]; 2] = transmute_ref!(&one_dimensional);
///
/// assert_eq!(two_dimensional, &[[0, 1, 2, 3], [4, 5, 6, 7]]);
/// ```
///
/// # Alignment increase error message
///
/// Because of limitations on macros, the error message generated when
/// `transmute_ref!` is used to transmute from a type of lower alignment to a
/// type of higher alignment is somewhat confusing. For example, the following
/// code:
///
/// ```compile_fail
/// const INCREASE_ALIGNMENT: &u16 = zerocopy::transmute_ref!(&[0u8; 2]);
/// ```
///
/// ...generates the following error:
///
/// ```text
/// error[E0512]: cannot transmute between types of different sizes, or dependently-sized types
///  --> src/lib.rs:1524:34
///   |
/// 5 | const INCREASE_ALIGNMENT: &u16 = zerocopy::transmute_ref!(&[0u8; 2]);
///   |                                  ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
///   |
///   = note: source type: `AlignOf<[u8; 2]>` (8 bits)
///   = note: target type: `MaxAlignsOf<[u8; 2], u16>` (16 bits)
///   = note: this error originates in the macro `$crate::assert_align_gt_eq` which comes from the expansion of the macro `transmute_ref` (in Nightly builds, run with -Z macro-backtrace for more info)
/// ```
///
/// This is saying that `max(align_of::<T>(), align_of::<U>()) !=
/// align_of::<T>()`, which is equivalent to `align_of::<T>() <
/// align_of::<U>()`.
#[macro_export]
macro_rules! transmute_ref {
    ($e:expr) => {{
        // NOTE: This must be a macro (rather than a function with trait bounds)
        // because there's no way, in a generic context, to enforce that two
        // types have the same size or alignment.

        // Ensure that the source type is a reference or a mutable reference
        // (note that mutable references are implicitly reborrowed here).
        let e: &_ = $e;


        if false {
            // This branch, though never taken, ensures that the type of `e` is
            // `&T` where `T: 't + Sized + AsBytes`, that the type of this macro
            // expression is `&U` where `U: 'u + Sized + FromBytes`, and that
            // `'t` outlives `'u`.

            struct AssertIsAsBytes<'a, T: ::core::marker::Sized + $crate::AsBytes>(&'a T);
            let _ = AssertIsAsBytes(e);

            struct AssertIsFromBytes<'a, U: ::core::marker::Sized + $crate::FromBytes>(&'a U);

            let u = AssertIsFromBytes(loop {});
            u.0
        } else if false {
            // This branch, though never taken, ensures that `size_of::<T>() ==
            // size_of::<U>()` and that that `align_of::<T>() >=
            // align_of::<U>()`.

            // `t` is inferred to have type `T` because it's assigned to `e` (of
            // type `&T`) as `&t`.
            let mut t = unreachable!();
            e = &t;

            // `u` is inferred to have type `U` because it's used as `&u` as the
            // value returned from this branch.
            let u;

            $crate::assert_size_eq!(t, u);
            $crate::assert_align_gt_eq!(t, u);

            &u
        } else {
            // SAFETY: For source type `Src` and destination type `Dst`:
            // - We know that `Src: AsBytes` and `Dst: FromBytes` thanks to the
            //   uses of `AssertIsAsBytes` and `AssertIsFromBytes` above.
            // - We know that `size_of::<Src>() == size_of::<Dst>()` thanks to
            //   the use of `assert_size_eq!` above.
            // - We know that `align_of::<Src>() >= align_of::<Dst>()` thanks to
            //   the use of `assert_align_gt_eq!` above.
            unsafe { $crate::macro_util::transmute_ref(e) }
        }
    }}
}

/// Safely transmutes a mutable reference of one type to an mutable reference of
/// another type of the same size.
///
/// The expression `$e` must have a concrete type, `&mut T`, where `T: Sized +
/// AsBytes`. The `transmute_mut!` expression must also have a concrete type,
/// `&mut U` (`U` is inferred from the calling context), where `U: Sized +
/// FromBytes`. It must be the case that `align_of::<T>() >= align_of::<U>()`.
///
/// The lifetime of the input type, `&mut T`, must be the same as or outlive the
/// lifetime of the output type, `&mut U`.
///
/// # Examples
///
/// ```
/// # use zerocopy::transmute_mut;
/// let mut one_dimensional: [u8; 8] = [0, 1, 2, 3, 4, 5, 6, 7];
///
/// let two_dimensional: &mut [[u8; 4]; 2] = transmute_mut!(&mut one_dimensional);
///
/// assert_eq!(two_dimensional, &[[0, 1, 2, 3], [4, 5, 6, 7]]);
///
/// two_dimensional.reverse();
///
/// assert_eq!(one_dimensional, [4, 5, 6, 7, 0, 1, 2, 3]);
/// ```
///
/// # Alignment increase error message
///
/// Because of limitations on macros, the error message generated when
/// `transmute_mut!` is used to transmute from a type of lower alignment to a
/// type of higher alignment is somewhat confusing. For example, the following
/// code:
///
/// ```compile_fail
/// const INCREASE_ALIGNMENT: &mut u16 = zerocopy::transmute_mut!(&mut [0u8; 2]);
/// ```
///
/// ...generates the following error:
///
/// ```text
/// error[E0512]: cannot transmute between types of different sizes, or dependently-sized types
///  --> src/lib.rs:1524:34
///   |
/// 5 | const INCREASE_ALIGNMENT: &mut u16 = zerocopy::transmute_mut!(&mut [0u8; 2]);
///   |                                      ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
///   |
///   = note: source type: `AlignOf<[u8; 2]>` (8 bits)
///   = note: target type: `MaxAlignsOf<[u8; 2], u16>` (16 bits)
///   = note: this error originates in the macro `$crate::assert_align_gt_eq` which comes from the expansion of the macro `transmute_mut` (in Nightly builds, run with -Z macro-backtrace for more info)
/// ```
///
/// This is saying that `max(align_of::<T>(), align_of::<U>()) !=
/// align_of::<T>()`, which is equivalent to `align_of::<T>() <
/// align_of::<U>()`.
#[macro_export]
macro_rules! transmute_mut {
    ($e:expr) => {{
        // NOTE: This must be a macro (rather than a function with trait bounds)
        // because there's no way, in a generic context, to enforce that two
        // types have the same size or alignment.

        // Ensure that the source type is a mutable reference.
        let e: &mut _ = $e;


        if false {
            // This branch, though never taken, ensures that the type of `e` is
            // `&mut T` where `T: 't + Sized + FromBytes + AsBytes`, that the
            // type of this macro expression is `&mut U` where `U: 'u + Sized +
            // FromBytes + AsBytes`.

            // We use immutable references here rather than mutable so that, if
            // this macro is used in a const context (in which, as of this
            // writing, mutable references are banned), the error message
            // appears to originate in the user's code rather than in the
            // internals of this macro.
            struct AssertSrcIsFromBytes<'a, T: ::core::marker::Sized + $crate::FromBytes>(&'a T);
            struct AssertSrcIsAsBytes<'a, T: ::core::marker::Sized + $crate::AsBytes>(&'a T);
            struct AssertDstIsFromBytes<'a, T: ::core::marker::Sized + $crate::FromBytes>(&'a T);
            struct AssertDstIsAsBytes<'a, T: ::core::marker::Sized + $crate::AsBytes>(&'a T);

            if true {
                let _ = AssertSrcIsFromBytes(&*e);
            } else {
                let _ = AssertSrcIsAsBytes(&*e);
            }

            if true {

                let u = AssertDstIsFromBytes(loop {});
                &mut *u.0
            } else {

                let u = AssertDstIsAsBytes(loop {});
                &mut *u.0
            }
        } else if false {
            // This branch, though never taken, ensures that `size_of::<T>() ==
            // size_of::<U>()` and that that `align_of::<T>() >=
            // align_of::<U>()`.

            // `t` is inferred to have type `T` because it's assigned to `e` (of
            // type `&mut T`) as `&mut t`.
            let mut t = unreachable!();
            e = &mut t;

            // `u` is inferred to have type `U` because it's used as `&mut u` as
            // the value returned from this branch.
            let u;

            $crate::assert_size_eq!(t, u);
            $crate::assert_align_gt_eq!(t, u);

            &mut u
        } else {
            // SAFETY: For source type `Src` and destination type `Dst`:
            // - We know that `Src: FromBytes + AsBytes` and `Dst: FromBytes +
            //   AsBytes` thanks to the uses of `AssertSrcIsFromBytes`,
            //   `AssertSrcIsAsBytes`, `AssertDstIsFromBytes`, and
            //   `AssertDstIsAsBytes` above.
            // - We know that `size_of::<Src>() == size_of::<Dst>()` thanks to
            //   the use of `assert_size_eq!` above.
            // - We know that `align_of::<Src>() >= align_of::<Dst>()` thanks to
            //   the use of `assert_align_gt_eq!` above.
            unsafe { $crate::macro_util::transmute_mut(e) }
        }
    }}
}

/// Includes a file and safely transmutes it to a value of an arbitrary type.
///
/// The file will be included as a byte array, `[u8; N]`, which will be
/// transmuted to another type, `T`. `T` is inferred from the calling context,
/// and must implement [`FromBytes`].
///
/// The file is located relative to the current file (similarly to how modules
/// are found). The provided path is interpreted in a platform-specific way at
/// compile time. So, for instance, an invocation with a Windows path containing
/// backslashes `\` would not compile correctly on Unix.
///
/// `include_value!` is ignorant of byte order. For byte order-aware types, see
/// the [`byteorder`] module.
///
/// # Examples
///
/// Assume there are two files in the same directory with the following
/// contents:
///
/// File `data` (no trailing newline):
///
/// ```text
/// abcd
/// ```
///
/// File `main.rs`:
///
/// ```rust
/// use zerocopy::include_value;
/// # macro_rules! include_value {
/// # ($file:expr) => { zerocopy::include_value!(concat!("../testdata/include_value/", $file)) };
/// # }
///
/// fn main() {
///     let as_u32: u32 = include_value!("data");
///     assert_eq!(as_u32, u32::from_ne_bytes([b'a', b'b', b'c', b'd']));
///     let as_i32: i32 = include_value!("data");
///     assert_eq!(as_i32, i32::from_ne_bytes([b'a', b'b', b'c', b'd']));
/// }
/// ```
#[doc(alias("include_bytes", "include_data", "include_type"))]
#[macro_export]
macro_rules! include_value {
    ($file:expr $(,)?) => {
        $crate::transmute!(*::core::include_bytes!($file))
    };
}

/// A typed reference derived from a byte slice.
///
/// A `Ref<B, T>` is a reference to a `T` which is stored in a byte slice, `B`.
/// Unlike a native reference (`&T` or `&mut T`), `Ref<B, T>` has the same
/// mutability as the byte slice it was constructed from (`B`).
///
/// # Examples
///
/// `Ref` can be used to treat a sequence of bytes as a structured type, and to
/// read and write the fields of that type as if the byte slice reference were
/// simply a reference to that type.
///
/// ```rust
/// # #[cfg(feature = "derive")] { // This example uses derives, and won't compile without them
/// use zerocopy::{AsBytes, ByteSlice, ByteSliceMut, FromBytes, FromZeroes, Ref, Unaligned};
///
/// #[derive(FromZeroes, FromBytes, AsBytes, Unaligned)]
/// #[repr(C)]
/// struct UdpHeader {
///     src_port: [u8; 2],
///     dst_port: [u8; 2],
///     length: [u8; 2],
///     checksum: [u8; 2],
/// }
///
/// struct UdpPacket<B> {
///     header: Ref<B, UdpHeader>,
///     body: B,
/// }
///
/// impl<B: ByteSlice> UdpPacket<B> {
///     pub fn parse(bytes: B) -> Option<UdpPacket<B>> {
///         let (header, body) = Ref::new_unaligned_from_prefix(bytes)?;
///         Some(UdpPacket { header, body })
///     }
///
///     pub fn get_src_port(&self) -> [u8; 2] {
///         self.header.src_port
///     }
/// }
///
/// impl<B: ByteSliceMut> UdpPacket<B> {
///     pub fn set_src_port(&mut self, src_port: [u8; 2]) {
///         self.header.src_port = src_port;
///     }
/// }
/// # }
/// ```
pub struct Ref<B, T: ?Sized>(B, PhantomData<T>);

/// Deprecated: prefer [`Ref`] instead.
#[deprecated(since = "0.7.0", note = "LayoutVerified has been renamed to Ref")]
#[doc(hidden)]
pub type LayoutVerified<B, T> = Ref<B, T>;

impl<B, T> Ref<B, T>
where
    B: ByteSlice,
{
    /// Constructs a new `Ref`.
    ///
    /// `new` verifies that `bytes.len() == size_of::<T>()` and that `bytes` is
    /// aligned to `align_of::<T>()`, and constructs a new `Ref`. If either of
    /// these checks fail, it returns `None`.
    #[inline]
    pub fn new(bytes: B) -> Option<Ref<B, T>> {
        if bytes.len() != mem::size_of::<T>() || !util::aligned_to::<_, T>(bytes.deref()) {
            return None;
        }
        Some(Ref(bytes, PhantomData))
    }

    /// Constructs a new `Ref` from the prefix of a byte slice.
    ///
    /// `new_from_prefix` verifies that `bytes.len() >= size_of::<T>()` and that
    /// `bytes` is aligned to `align_of::<T>()`. It consumes the first
    /// `size_of::<T>()` bytes from `bytes` to construct a `Ref`, and returns
    /// the remaining bytes to the caller. If either the length or alignment
    /// checks fail, it returns `None`.
    #[inline]
    pub fn new_from_prefix(bytes: B) -> Option<(Ref<B, T>, B)> {
        if bytes.len() < mem::size_of::<T>() || !util::aligned_to::<_, T>(bytes.deref()) {
            return None;
        }
        let (bytes, suffix) = bytes.split_at(mem::size_of::<T>());
        Some((Ref(bytes, PhantomData), suffix))
    }

    /// Constructs a new `Ref` from the suffix of a byte slice.
    ///
    /// `new_from_suffix` verifies that `bytes.len() >= size_of::<T>()` and that
    /// the last `size_of::<T>()` bytes of `bytes` are aligned to
    /// `align_of::<T>()`. It consumes the last `size_of::<T>()` bytes from
    /// `bytes` to construct a `Ref`, and returns the preceding bytes to the
    /// caller. If either the length or alignment checks fail, it returns
    /// `None`.
    #[inline]
    pub fn new_from_suffix(bytes: B) -> Option<(B, Ref<B, T>)> {
        let bytes_len = bytes.len();
        let split_at = bytes_len.checked_sub(mem::size_of::<T>())?;
        let (prefix, bytes) = bytes.split_at(split_at);
        if !util::aligned_to::<_, T>(bytes.deref()) {
            return None;
        }
        Some((prefix, Ref(bytes, PhantomData)))
    }
}

impl<B, T> Ref<B, [T]>
where
    B: ByteSlice,
{
    /// Constructs a new `Ref` of a slice type.
    ///
    /// `new_slice` verifies that `bytes.len()` is a multiple of
    /// `size_of::<T>()` and that `bytes` is aligned to `align_of::<T>()`, and
    /// constructs a new `Ref`. If either of these checks fail, it returns
    /// `None`.
    ///
    /// # Panics
    ///
    /// `new_slice` panics if `T` is a zero-sized type.
    #[inline]
    pub fn new_slice(bytes: B) -> Option<Ref<B, [T]>> {
        let remainder = bytes
            .len()
            .checked_rem(mem::size_of::<T>())
            .expect("Ref::new_slice called on a zero-sized type");
        if remainder != 0 || !util::aligned_to::<_, T>(bytes.deref()) {
            return None;
        }
        Some(Ref(bytes, PhantomData))
    }

    /// Constructs a new `Ref` of a slice type from the prefix of a byte slice.
    ///
    /// `new_slice_from_prefix` verifies that `bytes.len() >= size_of::<T>() *
    /// count` and that `bytes` is aligned to `align_of::<T>()`. It consumes the
    /// first `size_of::<T>() * count` bytes from `bytes` to construct a `Ref`,
    /// and returns the remaining bytes to the caller. It also ensures that
    /// `sizeof::<T>() * count` does not overflow a `usize`. If any of the
    /// length, alignment, or overflow checks fail, it returns `None`.
    ///
    /// # Panics
    ///
    /// `new_slice_from_prefix` panics if `T` is a zero-sized type.
    #[inline]
    pub fn new_slice_from_prefix(bytes: B, count: usize) -> Option<(Ref<B, [T]>, B)> {
        let expected_len = match mem::size_of::<T>().checked_mul(count) {
            Some(len) => len,
            None => return None,
        };
        if bytes.len() < expected_len {
            return None;
        }
        let (prefix, bytes) = bytes.split_at(expected_len);
        Self::new_slice(prefix).map(move |l| (l, bytes))
    }

    /// Constructs a new `Ref` of a slice type from the suffix of a byte slice.
    ///
    /// `new_slice_from_suffix` verifies that `bytes.len() >= size_of::<T>() *
    /// count` and that `bytes` is aligned to `align_of::<T>()`. It consumes the
    /// last `size_of::<T>() * count` bytes from `bytes` to construct a `Ref`,
    /// and returns the preceding bytes to the caller. It also ensures that
    /// `sizeof::<T>() * count` does not overflow a `usize`. If any of the
    /// length, alignment, or overflow checks fail, it returns `None`.
    ///
    /// # Panics
    ///
    /// `new_slice_from_suffix` panics if `T` is a zero-sized type.
    #[inline]
    pub fn new_slice_from_suffix(bytes: B, count: usize) -> Option<(B, Ref<B, [T]>)> {
        let expected_len = match mem::size_of::<T>().checked_mul(count) {
            Some(len) => len,
            None => return None,
        };
        let split_at = bytes.len().checked_sub(expected_len)?;
        let (bytes, suffix) = bytes.split_at(split_at);
        Self::new_slice(suffix).map(move |l| (bytes, l))
    }
}

fn map_zeroed<B: ByteSliceMut, T: ?Sized>(opt: Option<Ref<B, T>>) -> Option<Ref<B, T>> {
    match opt {
        Some(mut r) => {
            r.0.fill(0);
            Some(r)
        }
        None => None,
    }
}

fn map_prefix_tuple_zeroed<B: ByteSliceMut, T: ?Sized>(
    opt: Option<(Ref<B, T>, B)>,
) -> Option<(Ref<B, T>, B)> {
    match opt {
        Some((mut r, rest)) => {
            r.0.fill(0);
            Some((r, rest))
        }
        None => None,
    }
}

fn map_suffix_tuple_zeroed<B: ByteSliceMut, T: ?Sized>(
    opt: Option<(B, Ref<B, T>)>,
) -> Option<(B, Ref<B, T>)> {
    map_prefix_tuple_zeroed(opt.map(|(a, b)| (b, a))).map(|(a, b)| (b, a))
}

impl<B, T> Ref<B, T>
where
    B: ByteSliceMut,
{
    /// Constructs a new `Ref` after zeroing the bytes.
    ///
    /// `new_zeroed` verifies that `bytes.len() == size_of::<T>()` and that
    /// `bytes` is aligned to `align_of::<T>()`, and constructs a new `Ref`. If
    /// either of these checks fail, it returns `None`.
    ///
    /// If the checks succeed, then `bytes` will be initialized to zero. This
    /// can be useful when re-using buffers to ensure that sensitive data
    /// previously stored in the buffer is not leaked.
    #[inline(always)]
    pub fn new_zeroed(bytes: B) -> Option<Ref<B, T>> {
        map_zeroed(Self::new(bytes))
    }

    /// Constructs a new `Ref` from the prefix of a byte slice, zeroing the
    /// prefix.
    ///
    /// `new_from_prefix_zeroed` verifies that `bytes.len() >= size_of::<T>()`
    /// and that `bytes` is aligned to `align_of::<T>()`. It consumes the first
    /// `size_of::<T>()` bytes from `bytes` to construct a `Ref`, and returns
    /// the remaining bytes to the caller. If either the length or alignment
    /// checks fail, it returns `None`.
    ///
    /// If the checks succeed, then the prefix which is consumed will be
    /// initialized to zero. This can be useful when re-using buffers to ensure
    /// that sensitive data previously stored in the buffer is not leaked.
    #[inline(always)]
    pub fn new_from_prefix_zeroed(bytes: B) -> Option<(Ref<B, T>, B)> {
        map_prefix_tuple_zeroed(Self::new_from_prefix(bytes))
    }

    /// Constructs a new `Ref` from the suffix of a byte slice, zeroing the
    /// suffix.
    ///
    /// `new_from_suffix_zeroed` verifies that `bytes.len() >= size_of::<T>()`
    /// and that the last `size_of::<T>()` bytes of `bytes` are aligned to
    /// `align_of::<T>()`. It consumes the last `size_of::<T>()` bytes from
    /// `bytes` to construct a `Ref`, and returns the preceding bytes to the
    /// caller. If either the length or alignment checks fail, it returns
    /// `None`.
    ///
    /// If the checks succeed, then the suffix which is consumed will be
    /// initialized to zero. This can be useful when re-using buffers to ensure
    /// that sensitive data previously stored in the buffer is not leaked.
    #[inline(always)]
    pub fn new_from_suffix_zeroed(bytes: B) -> Option<(B, Ref<B, T>)> {
        map_suffix_tuple_zeroed(Self::new_from_suffix(bytes))
    }
}

impl<B, T> Ref<B, [T]>
where
    B: ByteSliceMut,
{
    /// Constructs a new `Ref` of a slice type after zeroing the bytes.
    ///
    /// `new_slice_zeroed` verifies that `bytes.len()` is a multiple of
    /// `size_of::<T>()` and that `bytes` is aligned to `align_of::<T>()`, and
    /// constructs a new `Ref`. If either of these checks fail, it returns
    /// `None`.
    ///
    /// If the checks succeed, then `bytes` will be initialized to zero. This
    /// can be useful when re-using buffers to ensure that sensitive data
    /// previously stored in the buffer is not leaked.
    ///
    /// # Panics
    ///
    /// `new_slice` panics if `T` is a zero-sized type.
    #[inline(always)]
    pub fn new_slice_zeroed(bytes: B) -> Option<Ref<B, [T]>> {
        map_zeroed(Self::new_slice(bytes))
    }

    /// Constructs a new `Ref` of a slice type from the prefix of a byte slice,
    /// after zeroing the bytes.
    ///
    /// `new_slice_from_prefix` verifies that `bytes.len() >= size_of::<T>() *
    /// count` and that `bytes` is aligned to `align_of::<T>()`. It consumes the
    /// first `size_of::<T>() * count` bytes from `bytes` to construct a `Ref`,
    /// and returns the remaining bytes to the caller. It also ensures that
    /// `sizeof::<T>() * count` does not overflow a `usize`. If any of the
    /// length, alignment, or overflow checks fail, it returns `None`.
    ///
    /// If the checks succeed, then the suffix which is consumed will be
    /// initialized to zero. This can be useful when re-using buffers to ensure
    /// that sensitive data previously stored in the buffer is not leaked.
    ///
    /// # Panics
    ///
    /// `new_slice_from_prefix_zeroed` panics if `T` is a zero-sized type.
    #[inline(always)]
    pub fn new_slice_from_prefix_zeroed(bytes: B, count: usize) -> Option<(Ref<B, [T]>, B)> {
        map_prefix_tuple_zeroed(Self::new_slice_from_prefix(bytes, count))
    }

    /// Constructs a new `Ref` of a slice type from the prefix of a byte slice,
    /// after zeroing the bytes.
    ///
    /// `new_slice_from_suffix` verifies that `bytes.len() >= size_of::<T>() *
    /// count` and that `bytes` is aligned to `align_of::<T>()`. It consumes the
    /// last `size_of::<T>() * count` bytes from `bytes` to construct a `Ref`,
    /// and returns the preceding bytes to the caller. It also ensures that
    /// `sizeof::<T>() * count` does not overflow a `usize`. If any of the
    /// length, alignment, or overflow checks fail, it returns `None`.
    ///
    /// If the checks succeed, then the consumed suffix will be initialized to
    /// zero. This can be useful when re-using buffers to ensure that sensitive
    /// data previously stored in the buffer is not leaked.
    ///
    /// # Panics
    ///
    /// `new_slice_from_suffix_zeroed` panics if `T` is a zero-sized type.
    #[inline(always)]
    pub fn new_slice_from_suffix_zeroed(bytes: B, count: usize) -> Option<(B, Ref<B, [T]>)> {
        map_suffix_tuple_zeroed(Self::new_slice_from_suffix(bytes, count))
    }
}

impl<B, T> Ref<B, T>
where
    B: ByteSlice,
    T: Unaligned,
{
    /// Constructs a new `Ref` for a type with no alignment requirement.
    ///
    /// `new_unaligned` verifies that `bytes.len() == size_of::<T>()` and
    /// constructs a new `Ref`. If the check fails, it returns `None`.
    #[inline(always)]
    pub fn new_unaligned(bytes: B) -> Option<Ref<B, T>> {
        Ref::new(bytes)
    }

    /// Constructs a new `Ref` from the prefix of a byte slice for a type with
    /// no alignment requirement.
    ///
    /// `new_unaligned_from_prefix` verifies that `bytes.len() >=
    /// size_of::<T>()`. It consumes the first `size_of::<T>()` bytes from
    /// `bytes` to construct a `Ref`, and returns the remaining bytes to the
    /// caller. If the length check fails, it returns `None`.
    #[inline(always)]
    pub fn new_unaligned_from_prefix(bytes: B) -> Option<(Ref<B, T>, B)> {
        Ref::new_from_prefix(bytes)
    }

    /// Constructs a new `Ref` from the suffix of a byte slice for a type with
    /// no alignment requirement.
    ///
    /// `new_unaligned_from_suffix` verifies that `bytes.len() >=
    /// size_of::<T>()`. It consumes the last `size_of::<T>()` bytes from
    /// `bytes` to construct a `Ref`, and returns the preceding bytes to the
    /// caller. If the length check fails, it returns `None`.
    #[inline(always)]
    pub fn new_unaligned_from_suffix(bytes: B) -> Option<(B, Ref<B, T>)> {
        Ref::new_from_suffix(bytes)
    }
}

impl<B, T> Ref<B, [T]>
where
    B: ByteSlice,
    T: Unaligned,
{
    /// Constructs a new `Ref` of a slice type with no alignment requirement.
    ///
    /// `new_slice_unaligned` verifies that `bytes.len()` is a multiple of
    /// `size_of::<T>()` and constructs a new `Ref`. If the check fails, it
    /// returns `None`.
    ///
    /// # Panics
    ///
    /// `new_slice` panics if `T` is a zero-sized type.
    #[inline(always)]
    pub fn new_slice_unaligned(bytes: B) -> Option<Ref<B, [T]>> {
        Ref::new_slice(bytes)
    }

    /// Constructs a new `Ref` of a slice type with no alignment requirement
    /// from the prefix of a byte slice.
    ///
    /// `new_slice_from_prefix` verifies that `bytes.len() >= size_of::<T>() *
    /// count`. It consumes the first `size_of::<T>() * count` bytes from
    /// `bytes` to construct a `Ref`, and returns the remaining bytes to the
    /// caller. It also ensures that `sizeof::<T>() * count` does not overflow a
    /// `usize`. If either the length, or overflow checks fail, it returns
    /// `None`.
    ///
    /// # Panics
    ///
    /// `new_slice_unaligned_from_prefix` panics if `T` is a zero-sized type.
    #[inline(always)]
    pub fn new_slice_unaligned_from_prefix(bytes: B, count: usize) -> Option<(Ref<B, [T]>, B)> {
        Ref::new_slice_from_prefix(bytes, count)
    }

    /// Constructs a new `Ref` of a slice type with no alignment requirement
    /// from the suffix of a byte slice.
    ///
    /// `new_slice_from_suffix` verifies that `bytes.len() >= size_of::<T>() *
    /// count`. It consumes the last `size_of::<T>() * count` bytes from `bytes`
    /// to construct a `Ref`, and returns the remaining bytes to the caller. It
    /// also ensures that `sizeof::<T>() * count` does not overflow a `usize`.
    /// If either the length, or overflow checks fail, it returns `None`.
    ///
    /// # Panics
    ///
    /// `new_slice_unaligned_from_suffix` panics if `T` is a zero-sized type.
    #[inline(always)]
    pub fn new_slice_unaligned_from_suffix(bytes: B, count: usize) -> Option<(B, Ref<B, [T]>)> {
        Ref::new_slice_from_suffix(bytes, count)
    }
}

impl<B, T> Ref<B, T>
where
    B: ByteSliceMut,
    T: Unaligned,
{
    /// Constructs a new `Ref` for a type with no alignment requirement, zeroing
    /// the bytes.
    ///
    /// `new_unaligned_zeroed` verifies that `bytes.len() == size_of::<T>()` and
    /// constructs a new `Ref`. If the check fails, it returns `None`.
    ///
    /// If the check succeeds, then `bytes` will be initialized to zero. This
    /// can be useful when re-using buffers to ensure that sensitive data
    /// previously stored in the buffer is not leaked.
    #[inline(always)]
    pub fn new_unaligned_zeroed(bytes: B) -> Option<Ref<B, T>> {
        map_zeroed(Self::new_unaligned(bytes))
    }

    /// Constructs a new `Ref` from the prefix of a byte slice for a type with
    /// no alignment requirement, zeroing the prefix.
    ///
    /// `new_unaligned_from_prefix_zeroed` verifies that `bytes.len() >=
    /// size_of::<T>()`. It consumes the first `size_of::<T>()` bytes from
    /// `bytes` to construct a `Ref`, and returns the remaining bytes to the
    /// caller. If the length check fails, it returns `None`.
    ///
    /// If the check succeeds, then the prefix which is consumed will be
    /// initialized to zero. This can be useful when re-using buffers to ensure
    /// that sensitive data previously stored in the buffer is not leaked.
    #[inline(always)]
    pub fn new_unaligned_from_prefix_zeroed(bytes: B) -> Option<(Ref<B, T>, B)> {
        map_prefix_tuple_zeroed(Self::new_unaligned_from_prefix(bytes))
    }

    /// Constructs a new `Ref` from the suffix of a byte slice for a type with
    /// no alignment requirement, zeroing the suffix.
    ///
    /// `new_unaligned_from_suffix_zeroed` verifies that `bytes.len() >=
    /// size_of::<T>()`. It consumes the last `size_of::<T>()` bytes from
    /// `bytes` to construct a `Ref`, and returns the preceding bytes to the
    /// caller. If the length check fails, it returns `None`.
    ///
    /// If the check succeeds, then the suffix which is consumed will be
    /// initialized to zero. This can be useful when re-using buffers to ensure
    /// that sensitive data previously stored in the buffer is not leaked.
    #[inline(always)]
    pub fn new_unaligned_from_suffix_zeroed(bytes: B) -> Option<(B, Ref<B, T>)> {
        map_suffix_tuple_zeroed(Self::new_unaligned_from_suffix(bytes))
    }
}

impl<B, T> Ref<B, [T]>
where
    B: ByteSliceMut,
    T: Unaligned,
{
    /// Constructs a new `Ref` for a slice type with no alignment requirement,
    /// zeroing the bytes.
    ///
    /// `new_slice_unaligned_zeroed` verifies that `bytes.len()` is a multiple
    /// of `size_of::<T>()` and constructs a new `Ref`. If the check fails, it
    /// returns `None`.
    ///
    /// If the check succeeds, then `bytes` will be initialized to zero. This
    /// can be useful when re-using buffers to ensure that sensitive data
    /// previously stored in the buffer is not leaked.
    ///
    /// # Panics
    ///
    /// `new_slice` panics if `T` is a zero-sized type.
    #[inline(always)]
    pub fn new_slice_unaligned_zeroed(bytes: B) -> Option<Ref<B, [T]>> {
        map_zeroed(Self::new_slice_unaligned(bytes))
    }

    /// Constructs a new `Ref` of a slice type with no alignment requirement
    /// from the prefix of a byte slice, after zeroing the bytes.
    ///
    /// `new_slice_from_prefix` verifies that `bytes.len() >= size_of::<T>() *
    /// count`. It consumes the first `size_of::<T>() * count` bytes from
    /// `bytes` to construct a `Ref`, and returns the remaining bytes to the
    /// caller. It also ensures that `sizeof::<T>() * count` does not overflow a
    /// `usize`. If either the length, or overflow checks fail, it returns
    /// `None`.
    ///
    /// If the checks succeed, then the prefix will be initialized to zero. This
    /// can be useful when re-using buffers to ensure that sensitive data
    /// previously stored in the buffer is not leaked.
    ///
    /// # Panics
    ///
    /// `new_slice_unaligned_from_prefix_zeroed` panics if `T` is a zero-sized
    /// type.
    #[inline(always)]
    pub fn new_slice_unaligned_from_prefix_zeroed(
        bytes: B,
        count: usize,
    ) -> Option<(Ref<B, [T]>, B)> {
        map_prefix_tuple_zeroed(Self::new_slice_unaligned_from_prefix(bytes, count))
    }

    /// Constructs a new `Ref` of a slice type with no alignment requirement
    /// from the suffix of a byte slice, after zeroing the bytes.
    ///
    /// `new_slice_from_suffix` verifies that `bytes.len() >= size_of::<T>() *
    /// count`. It consumes the last `size_of::<T>() * count` bytes from `bytes`
    /// to construct a `Ref`, and returns the remaining bytes to the caller. It
    /// also ensures that `sizeof::<T>() * count` does not overflow a `usize`.
    /// If either the length, or overflow checks fail, it returns `None`.
    ///
    /// If the checks succeed, then the suffix will be initialized to zero. This
    /// can be useful when re-using buffers to ensure that sensitive data
    /// previously stored in the buffer is not leaked.
    ///
    /// # Panics
    ///
    /// `new_slice_unaligned_from_suffix_zeroed` panics if `T` is a zero-sized
    /// type.
    #[inline(always)]
    pub fn new_slice_unaligned_from_suffix_zeroed(
        bytes: B,
        count: usize,
    ) -> Option<(B, Ref<B, [T]>)> {
        map_suffix_tuple_zeroed(Self::new_slice_unaligned_from_suffix(bytes, count))
    }
}

impl<'a, B, T> Ref<B, T>
where
    B: 'a + ByteSlice,
    T: FromBytes,
{
    /// Converts this `Ref` into a reference.
    ///
    /// `into_ref` consumes the `Ref`, and returns a reference to `T`.
    #[inline(always)]
    pub fn into_ref(self) -> &'a T {
        assert!(B::INTO_REF_INTO_MUT_ARE_SOUND);

        // SAFETY: According to the safety preconditions on
        // `ByteSlice::INTO_REF_INTO_MUT_ARE_SOUND`, the preceding assert
        // ensures that, given `B: 'a`, it is sound to drop `self` and still
        // access the underlying memory using reads for `'a`.
        unsafe { self.deref_helper() }
    }
}

impl<'a, B, T> Ref<B, T>
where
    B: 'a + ByteSliceMut,
    T: FromBytes + AsBytes,
{
    /// Converts this `Ref` into a mutable reference.
    ///
    /// `into_mut` consumes the `Ref`, and returns a mutable reference to `T`.
    #[inline(always)]
    pub fn into_mut(mut self) -> &'a mut T {
        assert!(B::INTO_REF_INTO_MUT_ARE_SOUND);

        // SAFETY: According to the safety preconditions on
        // `ByteSlice::INTO_REF_INTO_MUT_ARE_SOUND`, the preceding assert
        // ensures that, given `B: 'a + ByteSliceMut`, it is sound to drop
        // `self` and still access the underlying memory using both reads and
        // writes for `'a`.
        unsafe { self.deref_mut_helper() }
    }
}

impl<'a, B, T> Ref<B, [T]>
where
    B: 'a + ByteSlice,
    T: FromBytes,
{
    /// Converts this `Ref` into a slice reference.
    ///
    /// `into_slice` consumes the `Ref`, and returns a reference to `[T]`.
    #[inline(always)]
    pub fn into_slice(self) -> &'a [T] {
        assert!(B::INTO_REF_INTO_MUT_ARE_SOUND);

        // SAFETY: According to the safety preconditions on
        // `ByteSlice::INTO_REF_INTO_MUT_ARE_SOUND`, the preceding assert
        // ensures that, given `B: 'a`, it is sound to drop `self` and still
        // access the underlying memory using reads for `'a`.
        unsafe { self.deref_slice_helper() }
    }
}

impl<'a, B, T> Ref<B, [T]>
where
    B: 'a + ByteSliceMut,
    T: FromBytes + AsBytes,
{
    /// Converts this `Ref` into a mutable slice reference.
    ///
    /// `into_mut_slice` consumes the `Ref`, and returns a mutable reference to
    /// `[T]`.
    #[inline(always)]
    pub fn into_mut_slice(mut self) -> &'a mut [T] {
        assert!(B::INTO_REF_INTO_MUT_ARE_SOUND);

        // SAFETY: According to the safety preconditions on
        // `ByteSlice::INTO_REF_INTO_MUT_ARE_SOUND`, the preceding assert
        // ensures that, given `B: 'a + ByteSliceMut`, it is sound to drop
        // `self` and still access the underlying memory using both reads and
        // writes for `'a`.
        unsafe { self.deref_mut_slice_helper() }
    }
}

impl<B, T> Ref<B, T>
where
    B: ByteSlice,
    T: FromBytes,
{
    /// Creates an immutable reference to `T` with a specific lifetime.
    ///
    /// # Safety
    ///
    /// The type bounds on this method guarantee that it is safe to create an
    /// immutable reference to `T` from `self`. However, since the lifetime `'a`
    /// is not required to be shorter than the lifetime of the reference to
    /// `self`, the caller must guarantee that the lifetime `'a` is valid for
    /// this reference. In particular, the referent must exist for all of `'a`,
    /// and no mutable references to the same memory may be constructed during
    /// `'a`.
    unsafe fn deref_helper<'a>(&self) -> &'a T {
        // TODO(#429): Add a "SAFETY" comment and remove this `allow`.

        unsafe { &*self.0.as_ptr().cast::<T>() }
    }
}

impl<B, T> Ref<B, T>
where
    B: ByteSliceMut,
    T: FromBytes + AsBytes,
{
    /// Creates a mutable reference to `T` with a specific lifetime.
    ///
    /// # Safety
    ///
    /// The type bounds on this method guarantee that it is safe to create a
    /// mutable reference to `T` from `self`. However, since the lifetime `'a`
    /// is not required to be shorter than the lifetime of the reference to
    /// `self`, the caller must guarantee that the lifetime `'a` is valid for
    /// this reference. In particular, the referent must exist for all of `'a`,
    /// and no other references - mutable or immutable - to the same memory may
    /// be constructed during `'a`.
    unsafe fn deref_mut_helper<'a>(&mut self) -> &'a mut T {
        // TODO(#429): Add a "SAFETY" comment and remove this `allow`.

        unsafe { &mut *self.0.as_mut_ptr().cast::<T>() }
    }
}

impl<B, T> Ref<B, [T]>
where
    B: ByteSlice,
    T: FromBytes,
{
    /// Creates an immutable reference to `[T]` with a specific lifetime.
    ///
    /// # Safety
    ///
    /// `deref_slice_helper` has the same safety requirements as `deref_helper`.
    unsafe fn deref_slice_helper<'a>(&self) -> &'a [T] {
        let len = self.0.len();
        let elem_size = mem::size_of::<T>();
        debug_assert_ne!(elem_size, 0);
        // `Ref<_, [T]>` maintains the invariant that `size_of::<T>() > 0`.
        // Thus, neither the mod nor division operations here can panic.

        let elems = {
            debug_assert_eq!(len % elem_size, 0);
            len / elem_size
        };
        // TODO(#429): Add a "SAFETY" comment and remove this `allow`.

        unsafe { slice::from_raw_parts(self.0.as_ptr().cast::<T>(), elems) }
    }
}

impl<B, T> Ref<B, [T]>
where
    B: ByteSliceMut,
    T: FromBytes + AsBytes,
{
    /// Creates a mutable reference to `[T]` with a specific lifetime.
    ///
    /// # Safety
    ///
    /// `deref_mut_slice_helper` has the same safety requirements as
    /// `deref_mut_helper`.
    unsafe fn deref_mut_slice_helper<'a>(&mut self) -> &'a mut [T] {
        let len = self.0.len();
        let elem_size = mem::size_of::<T>();
        debug_assert_ne!(elem_size, 0);
        // `Ref<_, [T]>` maintains the invariant that `size_of::<T>() > 0`.
        // Thus, neither the mod nor division operations here can panic.

        let elems = {
            debug_assert_eq!(len % elem_size, 0);
            len / elem_size
        };
        // TODO(#429): Add a "SAFETY" comment and remove this `allow`.

        unsafe { slice::from_raw_parts_mut(self.0.as_mut_ptr().cast::<T>(), elems) }
    }
}

impl<B, T> Ref<B, T>
where
    B: ByteSlice,
    T: ?Sized,
{
    /// Gets the underlying bytes.
    #[inline]
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}

impl<B, T> Ref<B, T>
where
    B: ByteSliceMut,
    T: ?Sized,
{
    /// Gets the underlying bytes mutably.
    #[inline]
    pub fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

impl<B, T> Ref<B, T>
where
    B: ByteSlice,
    T: FromBytes,
{
    /// Reads a copy of `T`.
    #[inline]
    pub fn read(&self) -> T {
        // SAFETY: Because of the invariants on `Ref`, we know that `self.0` is
        // at least `size_of::<T>()` bytes long, and that it is at least as
        // aligned as `align_of::<T>()`. Because `T: FromBytes`, it is sound to
        // interpret these bytes as a `T`.
        unsafe { ptr::read(self.0.as_ptr().cast::<T>()) }
    }
}

impl<B, T> Ref<B, T>
where
    B: ByteSliceMut,
    T: AsBytes,
{
    /// Writes the bytes of `t` and then forgets `t`.
    #[inline]
    pub fn write(&mut self, t: T) {
        // SAFETY: Because of the invariants on `Ref`, we know that `self.0` is
        // at least `size_of::<T>()` bytes long, and that it is at least as
        // aligned as `align_of::<T>()`. Writing `t` to the buffer will allow
        // all of the bytes of `t` to be accessed as a `[u8]`, but because `T:
        // AsBytes`, we know this is sound.
        unsafe { ptr::write(self.0.as_mut_ptr().cast::<T>(), t) }
    }
}

impl<B, T> Deref for Ref<B, T>
where
    B: ByteSlice,
    T: FromBytes,
{
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        // SAFETY: This is sound because the lifetime of `self` is the same as
        // the lifetime of the return value, meaning that a) the returned
        // reference cannot outlive `self` and, b) no mutable methods on `self`
        // can be called during the lifetime of the returned reference. See the
        // documentation on `deref_helper` for what invariants we are required
        // to uphold.
        unsafe { self.deref_helper() }
    }
}

impl<B, T> DerefMut for Ref<B, T>
where
    B: ByteSliceMut,
    T: FromBytes + AsBytes,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: This is sound because the lifetime of `self` is the same as
        // the lifetime of the return value, meaning that a) the returned
        // reference cannot outlive `self` and, b) no other methods on `self`
        // can be called during the lifetime of the returned reference. See the
        // documentation on `deref_mut_helper` for what invariants we are
        // required to uphold.
        unsafe { self.deref_mut_helper() }
    }
}

impl<B, T> Deref for Ref<B, [T]>
where
    B: ByteSlice,
    T: FromBytes,
{
    type Target = [T];
    #[inline]
    fn deref(&self) -> &[T] {
        // SAFETY: This is sound because the lifetime of `self` is the same as
        // the lifetime of the return value, meaning that a) the returned
        // reference cannot outlive `self` and, b) no mutable methods on `self`
        // can be called during the lifetime of the returned reference. See the
        // documentation on `deref_slice_helper` for what invariants we are
        // required to uphold.
        unsafe { self.deref_slice_helper() }
    }
}

impl<B, T> DerefMut for Ref<B, [T]>
where
    B: ByteSliceMut,
    T: FromBytes + AsBytes,
{
    #[inline]
    fn deref_mut(&mut self) -> &mut [T] {
        // SAFETY: This is sound because the lifetime of `self` is the same as
        // the lifetime of the return value, meaning that a) the returned
        // reference cannot outlive `self` and, b) no other methods on `self`
        // can be called during the lifetime of the returned reference. See the
        // documentation on `deref_mut_slice_helper` for what invariants we are
        // required to uphold.
        unsafe { self.deref_mut_slice_helper() }
    }
}

impl<T, B> Display for Ref<B, T>
where
    B: ByteSlice,
    T: FromBytes + Display,
{
    #[inline]
    fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        let inner: &T = self;
        inner.fmt(fmt)
    }
}

impl<T, B> Display for Ref<B, [T]>
where
    B: ByteSlice,
    T: FromBytes,
    [T]: Display,
{
    #[inline]
    fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        let inner: &[T] = self;
        inner.fmt(fmt)
    }
}

impl<T, B> Debug for Ref<B, T>
where
    B: ByteSlice,
    T: FromBytes + Debug,
{
    #[inline]
    fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        let inner: &T = self;
        fmt.debug_tuple("Ref").field(&inner).finish()
    }
}

impl<T, B> Debug for Ref<B, [T]>
where
    B: ByteSlice,
    T: FromBytes + Debug,
{
    #[inline]
    fn fmt(&self, fmt: &mut Formatter<'_>) -> fmt::Result {
        let inner: &[T] = self;
        fmt.debug_tuple("Ref").field(&inner).finish()
    }
}

impl<T, B> Eq for Ref<B, T>
where
    B: ByteSlice,
    T: FromBytes + Eq,
{
}

impl<T, B> Eq for Ref<B, [T]>
where
    B: ByteSlice,
    T: FromBytes + Eq,
{
}

impl<T, B> PartialEq for Ref<B, T>
where
    B: ByteSlice,
    T: FromBytes + PartialEq,
{
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(other.deref())
    }
}

impl<T, B> PartialEq for Ref<B, [T]>
where
    B: ByteSlice,
    T: FromBytes + PartialEq,
{
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.deref().eq(other.deref())
    }
}

impl<T, B> Ord for Ref<B, T>
where
    B: ByteSlice,
    T: FromBytes + Ord,
{
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        let inner: &T = self;
        let other_inner: &T = other;
        inner.cmp(other_inner)
    }
}

impl<T, B> Ord for Ref<B, [T]>
where
    B: ByteSlice,
    T: FromBytes + Ord,
{
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        let inner: &[T] = self;
        let other_inner: &[T] = other;
        inner.cmp(other_inner)
    }
}

impl<T, B> PartialOrd for Ref<B, T>
where
    B: ByteSlice,
    T: FromBytes + PartialOrd,
{
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let inner: &T = self;
        let other_inner: &T = other;
        inner.partial_cmp(other_inner)
    }
}

impl<T, B> PartialOrd for Ref<B, [T]>
where
    B: ByteSlice,
    T: FromBytes + PartialOrd,
{
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        let inner: &[T] = self;
        let other_inner: &[T] = other;
        inner.partial_cmp(other_inner)
    }
}

mod sealed {
    pub trait ByteSliceSealed {}
}

// ByteSlice and ByteSliceMut abstract over [u8] references (&[u8], &mut [u8],
// Ref<[u8]>, RefMut<[u8]>, etc). We rely on various behaviors of these
// references such as that a given reference will never changes its length
// between calls to deref() or deref_mut(), and that split_at() works as
// expected. If ByteSlice or ByteSliceMut were not sealed, consumers could
// implement them in a way that violated these behaviors, and would break our
// unsafe code. Thus, we seal them and implement it only for known-good
// reference types. For the same reason, they're unsafe traits.

// TODO(fxbug.dev/99068)
/// A mutable or immutable reference to a byte slice.
///
/// `ByteSlice` abstracts over the mutability of a byte slice reference, and is
/// implemented for various special reference types such as `Ref<[u8]>` and
/// `RefMut<[u8]>`.
///
/// Note that, while it would be technically possible, `ByteSlice` is not
/// implemented for [`Vec<u8>`], as the only way to implement the [`split_at`]
/// method would involve reallocation, and `split_at` must be a very cheap
/// operation in order for the utilities in this crate to perform as designed.
///
/// [`split_at`]: crate::ByteSlice::split_at
// It may seem overkill to go to this length to ensure that this doc link never
// breaks. We do this because it simplifies CI - it means that generating docs
// always succeeds, so we don't need special logic to only generate docs under
// certain features.
#[cfg_attr(feature = "alloc", doc = "[`Vec<u8>`]: alloc::vec::Vec")]
#[cfg_attr(
    not(feature = "alloc"),
    doc = "[`Vec<u8>`]: https://doc.rust-lang.org/std/vec/struct.Vec.html"
)]
pub unsafe trait ByteSlice: Deref<Target = [u8]> + Sized + sealed::ByteSliceSealed {
    /// Are the [`Ref::into_ref`] and [`Ref::into_mut`] methods sound when used
    /// with `Self`? If not, evaluating this constant must panic at compile
    /// time.
    ///
    /// This exists to work around #716 on versions of zerocopy prior to 0.8.
    ///
    /// # Safety
    ///
    /// This may only be set to true if the following holds: Given the
    /// following:
    /// - `Self: 'a`
    /// - `bytes: Self`
    /// - `let ptr = bytes.as_ptr()`
    ///
    /// ...then:
    /// - Using `ptr` to read the memory previously addressed by `bytes` is
    ///   sound for `'a` even after `bytes` has been dropped.
    /// - If `Self: ByteSliceMut`, using `ptr` to write the memory previously
    ///   addressed by `bytes` is sound for `'a` even after `bytes` has been
    ///   dropped.
    #[doc(hidden)]
    const INTO_REF_INTO_MUT_ARE_SOUND: bool;

    /// Gets a raw pointer to the first byte in the slice.
    #[inline]
    fn as_ptr(&self) -> *const u8 {
        <[u8]>::as_ptr(self)
    }

    /// Splits the slice at the midpoint.
    ///
    /// `x.split_at(mid)` returns `x[..mid]` and `x[mid..]`.
    ///
    /// # Panics
    ///
    /// `x.split_at(mid)` panics if `mid > x.len()`.
    fn split_at(self, mid: usize) -> (Self, Self);
}

// TODO(fxbug.dev/99068)
/// A mutable reference to a byte slice.
///
/// `ByteSliceMut` abstracts over various ways of storing a mutable reference to
/// a byte slice, and is implemented for various special reference types such as
/// `RefMut<[u8]>`.
pub unsafe trait ByteSliceMut: ByteSlice + DerefMut {
    /// Gets a mutable raw pointer to the first byte in the slice.
    #[inline]
    fn as_mut_ptr(&mut self) -> *mut u8 {
        <[u8]>::as_mut_ptr(self)
    }
}

impl<'a> sealed::ByteSliceSealed for &'a [u8] {}
// TODO(#429): Add a "SAFETY" comment and remove this `allow`.

unsafe impl<'a> ByteSlice for &'a [u8] {
    // SAFETY: If `&'b [u8]: 'a`, then the underlying memory is treated as
    // borrowed immutably for `'a` even if the slice itself is dropped.
    const INTO_REF_INTO_MUT_ARE_SOUND: bool = true;

    #[inline]
    fn split_at(self, mid: usize) -> (Self, Self) {
        <[u8]>::split_at(self, mid)
    }
}

impl<'a> sealed::ByteSliceSealed for &'a mut [u8] {}
// TODO(#429): Add a "SAFETY" comment and remove this `allow`.

unsafe impl<'a> ByteSlice for &'a mut [u8] {
    // SAFETY: If `&'b mut [u8]: 'a`, then the underlying memory is treated as
    // borrowed mutably for `'a` even if the slice itself is dropped.
    const INTO_REF_INTO_MUT_ARE_SOUND: bool = true;

    #[inline]
    fn split_at(self, mid: usize) -> (Self, Self) {
        <[u8]>::split_at_mut(self, mid)
    }
}

impl<'a> sealed::ByteSliceSealed for cell::Ref<'a, [u8]> {}
// TODO(#429): Add a "SAFETY" comment and remove this `allow`.

unsafe impl<'a> ByteSlice for cell::Ref<'a, [u8]> {
    const INTO_REF_INTO_MUT_ARE_SOUND: bool = if !cfg!(doc) {
        panic!("Ref::into_ref and Ref::into_mut are unsound when used with core::cell::Ref; see https://github.com/google/zerocopy/issues/716")
    } else {
        // When compiling documentation, allow the evaluation of this constant
        // to succeed. This doesn't represent a soundness hole - it just delays
        // any error to runtime. The reason we need this is that, otherwise,
        // `rustdoc` will fail when trying to document this item.
        false
    };

    #[inline]
    fn split_at(self, mid: usize) -> (Self, Self) {
        cell::Ref::map_split(self, |slice| <[u8]>::split_at(slice, mid))
    }
}

impl<'a> sealed::ByteSliceSealed for RefMut<'a, [u8]> {}
// TODO(#429): Add a "SAFETY" comment and remove this `allow`.

unsafe impl<'a> ByteSlice for RefMut<'a, [u8]> {
    const INTO_REF_INTO_MUT_ARE_SOUND: bool = if !cfg!(doc) {
        panic!("Ref::into_ref and Ref::into_mut are unsound when used with core::cell::RefMut; see https://github.com/google/zerocopy/issues/716")
    } else {
        // When compiling documentation, allow the evaluation of this constant
        // to succeed. This doesn't represent a soundness hole - it just delays
        // any error to runtime. The reason we need this is that, otherwise,
        // `rustdoc` will fail when trying to document this item.
        false
    };

    #[inline]
    fn split_at(self, mid: usize) -> (Self, Self) {
        RefMut::map_split(self, |slice| <[u8]>::split_at_mut(slice, mid))
    }
}

// TODO(#429): Add a "SAFETY" comment and remove this `allow`.

unsafe impl<'a> ByteSliceMut for &'a mut [u8] {}

// TODO(#429): Add a "SAFETY" comment and remove this `allow`.

unsafe impl<'a> ByteSliceMut for RefMut<'a, [u8]> {}

#[cfg(feature = "alloc")]
#[cfg_attr(doc_cfg, doc(cfg(feature = "alloc")))]
mod alloc_support {
    use alloc::vec::Vec;

    use super::*;

    /// Extends a `Vec<T>` by pushing `additional` new items onto the end of the
    /// vector. The new items are initialized with zeroes.
    ///
    /// # Panics
    ///
    /// Panics if `Vec::reserve(additional)` fails to reserve enough memory.
    #[inline(always)]
    pub fn extend_vec_zeroed<T: FromZeroes>(v: &mut Vec<T>, additional: usize) {
        insert_vec_zeroed(v, v.len(), additional);
    }

    /// Inserts `additional` new items into `Vec<T>` at `position`.
    /// The new items are initialized with zeroes.
    ///
    /// # Panics
    ///
    /// * Panics if `position > v.len()`.
    /// * Panics if `Vec::reserve(additional)` fails to reserve enough memory.
    #[inline]
    pub fn insert_vec_zeroed<T: FromZeroes>(v: &mut Vec<T>, position: usize, additional: usize) {
        assert!(position <= v.len());
        v.reserve(additional);
        // SAFETY: The `reserve` call guarantees that these cannot overflow:
        // * `ptr.add(position)`
        // * `position + additional`
        // * `v.len() + additional`
        //
        // `v.len() - position` cannot overflow because we asserted that
        // `position <= v.len()`.
        unsafe {
            // This is a potentially overlapping copy.
            let ptr = v.as_mut_ptr();

            ptr.add(position)
                .copy_to(ptr.add(position + additional), v.len() - position);
            ptr.add(position).write_bytes(0, additional);

            v.set_len(v.len() + additional);
        }
    }
}

#[cfg(feature = "alloc")]
#[doc(inline)]
pub use alloc_support::*;

#[cfg(kani)]
mod proofs {
    use super::*;

    impl kani::Arbitrary for DstLayout {
        fn any() -> Self {
            let align: NonZeroUsize = kani::any();
            let size_info: SizeInfo = kani::any();

            kani::assume(align.is_power_of_two());
            kani::assume(align < DstLayout::THEORETICAL_MAX_ALIGN);

            // For testing purposes, we most care about instantiations of
            // `DstLayout` that can correspond to actual Rust types. We use
            // `Layout` to verify that our `DstLayout` satisfies the validity
            // conditions of Rust layouts.
            kani::assume(
                match size_info {
                    SizeInfo::Sized { _size } => Layout::from_size_align(_size, align.get()),
                    SizeInfo::SliceDst(TrailingSliceLayout {
                        _offset,
                        _elem_size,
                    }) => {
                        // `SliceDst`` cannot encode an exact size, but we know
                        // it is at least `_offset` bytes.
                        Layout::from_size_align(_offset, align.get())
                    }
                }
                .is_ok(),
            );

            Self {
                align: align,
                size_info: size_info,
            }
        }
    }

    impl kani::Arbitrary for SizeInfo {
        fn any() -> Self {
            let is_sized: bool = kani::any();

            match is_sized {
                true => {
                    let size: usize = kani::any();

                    kani::assume(size <= isize::MAX as _);

                    SizeInfo::Sized { _size: size }
                }
                false => SizeInfo::SliceDst(kani::any()),
            }
        }
    }

    impl kani::Arbitrary for TrailingSliceLayout {
        fn any() -> Self {
            let elem_size: usize = kani::any();
            let offset: usize = kani::any();

            kani::assume(elem_size < isize::MAX as _);
            kani::assume(offset < isize::MAX as _);

            TrailingSliceLayout {
                _elem_size: elem_size,
                _offset: offset,
            }
        }
    }

    #[kani::proof]
    fn prove_dst_layout_extend() {
        use crate::util::{core_layout::padding_needed_for, max, min};

        let base: DstLayout = kani::any();
        let field: DstLayout = kani::any();
        let packed: Option<NonZeroUsize> = kani::any();

        if let Some(max_align) = packed {
            kani::assume(max_align.is_power_of_two());
            kani::assume(base.align <= max_align);
        }

        // The base can only be extended if it's sized.
        kani::assume(matches!(base.size_info, SizeInfo::Sized { .. }));
        let base_size = if let SizeInfo::Sized { _size: size } = base.size_info {
            size
        } else {
            unreachable!();
        };

        // Under the above conditions, `DstLayout::extend` will not panic.
        let composite = base.extend(field, packed);

        // The field's alignment is clamped by `max_align` (i.e., the
        // `packed` attribute, if any) [1].
        //
        // [1] Per https://doc.rust-lang.org/reference/type-layout.html#the-alignment-modifiers:
        //
        //   The alignments of each field, for the purpose of positioning
        //   fields, is the smaller of the specified alignment and the
        //   alignment of the field's type.
        let field_align = min(
            field.align,
            packed.unwrap_or(DstLayout::THEORETICAL_MAX_ALIGN),
        );

        // The struct's alignment is the maximum of its previous alignment and
        // `field_align`.
        assert_eq!(composite.align, max(base.align, field_align));

        // Compute the minimum amount of inter-field padding needed to
        // satisfy the field's alignment, and offset of the trailing field.
        // [1]
        //
        // [1] Per https://doc.rust-lang.org/reference/type-layout.html#the-alignment-modifiers:
        //
        //   Inter-field padding is guaranteed to be the minimum required in
        //   order to satisfy each field's (possibly altered) alignment.
        let padding = padding_needed_for(base_size, field_align);
        let offset = base_size + padding;

        // For testing purposes, we'll also construct `alloc::Layout`
        // stand-ins for `DstLayout`, and show that `extend` behaves
        // comparably on both types.
        let base_analog = Layout::from_size_align(base_size, base.align.get()).unwrap();

        match field.size_info {
            SizeInfo::Sized { _size: field_size } => {
                if let SizeInfo::Sized {
                    _size: composite_size,
                } = composite.size_info
                {
                    // If the trailing field is sized, the resulting layout
                    // will be sized. Its size will be the sum of the
                    // preceeding layout, the size of the new field, and the
                    // size of inter-field padding between the two.
                    assert_eq!(composite_size, offset + field_size);

                    let field_analog =
                        Layout::from_size_align(field_size, field_align.get()).unwrap();

                    if let Ok((actual_composite, actual_offset)) = base_analog.extend(field_analog)
                    {
                        assert_eq!(actual_offset, offset);
                        assert_eq!(actual_composite.size(), composite_size);
                        assert_eq!(actual_composite.align(), composite.align.get());
                    } else {
                        // An error here reflects that composite of `base`
                        // and `field` cannot correspond to a real Rust type
                        // fragment, because such a fragment would violate
                        // the basic invariants of a valid Rust layout. At
                        // the time of writing, `DstLayout` is a little more
                        // permissive than `Layout`, so we don't assert
                        // anything in this branch (e.g., unreachability).
                    }
                } else {
                    panic!("The composite of two sized layouts must be sized.")
                }
            }
            SizeInfo::SliceDst(TrailingSliceLayout {
                _offset: field_offset,
                _elem_size: field_elem_size,
            }) => {
                if let SizeInfo::SliceDst(TrailingSliceLayout {
                    _offset: composite_offset,
                    _elem_size: composite_elem_size,
                }) = composite.size_info
                {
                    // The offset of the trailing slice component is the sum
                    // of the offset of the trailing field and the trailing
                    // slice offset within that field.
                    assert_eq!(composite_offset, offset + field_offset);
                    // The elem size is unchanged.
                    assert_eq!(composite_elem_size, field_elem_size);

                    let field_analog =
                        Layout::from_size_align(field_offset, field_align.get()).unwrap();

                    if let Ok((actual_composite, actual_offset)) = base_analog.extend(field_analog)
                    {
                        assert_eq!(actual_offset, offset);
                        assert_eq!(actual_composite.size(), composite_offset);
                        assert_eq!(actual_composite.align(), composite.align.get());
                    } else {
                        // An error here reflects that composite of `base`
                        // and `field` cannot correspond to a real Rust type
                        // fragment, because such a fragment would violate
                        // the basic invariants of a valid Rust layout. At
                        // the time of writing, `DstLayout` is a little more
                        // permissive than `Layout`, so we don't assert
                        // anything in this branch (e.g., unreachability).
                    }
                } else {
                    panic!("The extension of a layout with a DST must result in a DST.")
                }
            }
        }
    }

    #[kani::proof]
    #[kani::should_panic]
    fn prove_dst_layout_extend_dst_panics() {
        let base: DstLayout = kani::any();
        let field: DstLayout = kani::any();
        let packed: Option<NonZeroUsize> = kani::any();

        if let Some(max_align) = packed {
            kani::assume(max_align.is_power_of_two());
            kani::assume(base.align <= max_align);
        }

        kani::assume(matches!(base.size_info, SizeInfo::SliceDst(..)));

        let _ = base.extend(field, packed);
    }

    #[kani::proof]
    fn prove_dst_layout_pad_to_align() {
        use crate::util::core_layout::padding_needed_for;

        let layout: DstLayout = kani::any();

        let padded: DstLayout = layout.pad_to_align();

        // Calling `pad_to_align` does not alter the `DstLayout`'s alignment.
        assert_eq!(padded.align, layout.align);

        if let SizeInfo::Sized {
            _size: unpadded_size,
        } = layout.size_info
        {
            if let SizeInfo::Sized { _size: padded_size } = padded.size_info {
                // If the layout is sized, it will remain sized after padding is
                // added. Its sum will be its unpadded size and the size of the
                // trailing padding needed to satisfy its alignment
                // requirements.
                let padding = padding_needed_for(unpadded_size, layout.align);
                assert_eq!(padded_size, unpadded_size + padding);

                // Prove that calling `DstLayout::pad_to_align` behaves
                // identically to `Layout::pad_to_align`.
                let layout_analog =
                    Layout::from_size_align(unpadded_size, layout.align.get()).unwrap();
                let padded_analog = layout_analog.pad_to_align();
                assert_eq!(padded_analog.align(), layout.align.get());
                assert_eq!(padded_analog.size(), padded_size);
            } else {
                panic!("The padding of a sized layout must result in a sized layout.")
            }
        } else {
            // If the layout is a DST, padding cannot be statically added.
            assert_eq!(padded.size_info, layout.size_info);
        }
    }
}

#[cfg(test)]
#[path = "../tests/unit/lib.rs"]
mod tests;
