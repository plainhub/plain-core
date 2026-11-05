// Copyright 2019 The Fuchsia Authors
//
// Licensed under a BSD-style license <LICENSE-BSD>, Apache License, Version 2.0
// <LICENSE-APACHE or https://www.apache.org/licenses/LICENSE-2.0>, or the MIT
// license <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your option.
// This file may not be copied, modified, or distributed except according to
// those terms.

//! Byte order-aware numeric primitives.
//!
//! This module contains equivalents of the native multi-byte integer types with
//! no alignment requirement and supporting byte order conversions.
//!
//! For each native multi-byte integer type - `u16`, `i16`, `u32`, etc - and
//! floating point type - `f32` and `f64` - an equivalent type is defined by
//! this module - [`U16`], [`I16`], [`U32`], [`F64`], etc. Unlike their native
//! counterparts, these types have alignment 1, and take a type parameter
//! specifying the byte order in which the bytes are stored in memory. Each type
//! implements the [`FromBytes`], [`AsBytes`], and [`Unaligned`] traits.
//!
//! These two properties, taken together, make these types useful for defining
//! data structures whose memory layout matches a wire format such as that of a
//! network protocol or a file format. Such formats often have multi-byte values
//! at offsets that do not respect the alignment requirements of the equivalent
//! native types, and stored in a byte order not necessarily the same as that of
//! the target platform.
//!
//! Type aliases are provided for common byte orders in the [`big_endian`],
//! [`little_endian`], [`network_endian`], and [`native_endian`] submodules.
//!
//! # Example
//!
//! One use of these types is for representing network packet formats, such as
//! UDP:
//!
//! ```rust,edition2021
//! # #[cfg(feature = "derive")] { // This example uses derives, and won't compile without them
//! use zerocopy::{AsBytes, ByteSlice, FromBytes, FromZeroes, Ref, Unaligned};
//! use zerocopy::byteorder::network_endian::U16;
//!
//! #[derive(FromZeroes, FromBytes, AsBytes, Unaligned)]
//! #[repr(C)]
//! struct UdpHeader {
//!     src_port: U16,
//!     dst_port: U16,
//!     length: U16,
//!     checksum: U16,
//! }
//!
//! struct UdpPacket<B: ByteSlice> {
//!     header: Ref<B, UdpHeader>,
//!     body: B,
//! }
//!
//! impl<B: ByteSlice> UdpPacket<B> {
//!     fn parse(bytes: B) -> Option<UdpPacket<B>> {
//!         let (header, body) = Ref::new_from_prefix(bytes)?;
//!         Some(UdpPacket { header, body })
//!     }
//!
//!     fn src_port(&self) -> u16 {
//!         self.header.src_port.get()
//!     }
//!
//!     // more getters...
//! }
//! # }
//! ```

use core::{
    convert::{TryFrom, TryInto},
    fmt::{self, Binary, Debug, Display, Formatter, LowerHex, Octal, UpperHex},
    marker::PhantomData,
    num::TryFromIntError,
};

// We don't reexport `WriteBytesExt` or `ReadBytesExt` because those are only
// available with the `std` feature enabled, and zerocopy is `no_std` by
// default.
pub use ::byteorder::{BigEndian, ByteOrder, LittleEndian, NativeEndian, NetworkEndian, BE, LE};

use super::*;

macro_rules! impl_fmt_trait {
    ($name:ident, $native:ident, $trait:ident) => {
        impl<O: ByteOrder> $trait for $name<O> {
            #[inline(always)]
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                $trait::fmt(&self.get(), f)
            }
        }
    };
}

macro_rules! impl_fmt_traits {
    ($name:ident, $native:ident, "floating point number") => {
        impl_fmt_trait!($name, $native, Display);
    };
    ($name:ident, $native:ident, "unsigned integer") => {
        impl_fmt_traits!($name, $native, @all_types);
    };
    ($name:ident, $native:ident, "signed integer") => {
        impl_fmt_traits!($name, $native, @all_types);
    };
    ($name:ident, $native:ident, @all_types) => {
        impl_fmt_trait!($name, $native, Display);
        impl_fmt_trait!($name, $native, Octal);
        impl_fmt_trait!($name, $native, LowerHex);
        impl_fmt_trait!($name, $native, UpperHex);
        impl_fmt_trait!($name, $native, Binary);
    };
}

macro_rules! impl_ops_traits {
    ($name:ident, $native:ident, "floating point number") => {
        impl_ops_traits!($name, $native, @all_types);
        impl_ops_traits!($name, $native, @signed_integer_floating_point);
    };
    ($name:ident, $native:ident, "unsigned integer") => {
        impl_ops_traits!($name, $native, @signed_unsigned_integer);
        impl_ops_traits!($name, $native, @all_types);
    };
    ($name:ident, $native:ident, "signed integer") => {
        impl_ops_traits!($name, $native, @signed_unsigned_integer);
        impl_ops_traits!($name, $native, @signed_integer_floating_point);
        impl_ops_traits!($name, $native, @all_types);
    };
    ($name:ident, $native:ident, @signed_unsigned_integer) => {
        impl_ops_traits!(@without_byteorder_swap $name, $native, BitAnd, bitand, BitAndAssign, bitand_assign);
        impl_ops_traits!(@without_byteorder_swap $name, $native, BitOr, bitor, BitOrAssign, bitor_assign);
        impl_ops_traits!(@without_byteorder_swap $name, $native, BitXor, bitxor, BitXorAssign, bitxor_assign);
        impl_ops_traits!(@with_byteorder_swap $name, $native, Shl, shl, ShlAssign, shl_assign);
        impl_ops_traits!(@with_byteorder_swap $name, $native, Shr, shr, ShrAssign, shr_assign);

        impl<O> core::ops::Not for $name<O> {
            type Output = $name<O>;

            #[inline(always)]
            fn not(self) -> $name<O> {
                 let self_native = $native::from_ne_bytes(self.0);
                 $name((!self_native).to_ne_bytes(), PhantomData)
            }
        }
    };
    ($name:ident, $native:ident, @signed_integer_floating_point) => {
        impl<O: ByteOrder> core::ops::Neg for $name<O> {
            type Output = $name<O>;

            #[inline(always)]
            fn neg(self) -> $name<O> {
                let self_native: $native = self.get();

                $name::<O>::new(-self_native)
            }
        }
    };
    ($name:ident, $native:ident, @all_types) => {
        impl_ops_traits!(@with_byteorder_swap $name, $native, Add, add, AddAssign, add_assign);
        impl_ops_traits!(@with_byteorder_swap $name, $native, Div, div, DivAssign, div_assign);
        impl_ops_traits!(@with_byteorder_swap $name, $native, Mul, mul, MulAssign, mul_assign);
        impl_ops_traits!(@with_byteorder_swap $name, $native, Rem, rem, RemAssign, rem_assign);
        impl_ops_traits!(@with_byteorder_swap $name, $native, Sub, sub, SubAssign, sub_assign);
    };
    (@with_byteorder_swap $name:ident, $native:ident, $trait:ident, $method:ident, $trait_assign:ident, $method_assign:ident) => {
        impl<O: ByteOrder> core::ops::$trait for $name<O> {
            type Output = $name<O>;

            #[inline(always)]
            fn $method(self, rhs: $name<O>) -> $name<O> {
                let self_native: $native = self.get();
                let rhs_native: $native = rhs.get();
                let result_native = core::ops::$trait::$method(self_native, rhs_native);
                $name::<O>::new(result_native)
            }
        }

        impl<O: ByteOrder> core::ops::$trait_assign for $name<O> {
            #[inline(always)]
            fn $method_assign(&mut self, rhs: $name<O>) {
                *self = core::ops::$trait::$method(*self, rhs);
            }
        }
    };
    // Implement traits in terms of the same trait on the native type, but
    // without performing a byte order swap. This only works for bitwise
    // operations like `&`, `|`, etc.
    (@without_byteorder_swap $name:ident, $native:ident, $trait:ident, $method:ident, $trait_assign:ident, $method_assign:ident) => {
        impl<O: ByteOrder> core::ops::$trait for $name<O> {
            type Output = $name<O>;

            #[inline(always)]
            fn $method(self, rhs: $name<O>) -> $name<O> {
                let self_native = $native::from_ne_bytes(self.0);
                let rhs_native = $native::from_ne_bytes(rhs.0);
                let result_native = core::ops::$trait::$method(self_native, rhs_native);
                $name(result_native.to_ne_bytes(), PhantomData)
            }
        }

        impl<O: ByteOrder> core::ops::$trait_assign for $name<O> {
            #[inline(always)]
            fn $method_assign(&mut self, rhs: $name<O>) {
                *self = core::ops::$trait::$method(*self, rhs);
            }
        }
    };
}

macro_rules! doc_comment {
    ($x:expr, $($tt:tt)*) => {
        #[doc = $x]
        $($tt)*
    };
}

macro_rules! define_max_value_constant {
    ($name:ident, $bytes:expr, "unsigned integer") => {
        /// The maximum value.
        ///
        /// This constant should be preferred to constructing a new value using
        /// `new`, as `new` may perform an endianness swap depending on the
        /// endianness `O` and the endianness of the platform.
        pub const MAX_VALUE: $name<O> = $name([0xFFu8; $bytes], PhantomData);
    };
    // We don't provide maximum and minimum value constants for signed values
    // and floats because there's no way to do it generically - it would require
    // a different value depending on the value of the `ByteOrder` type
    // parameter. Currently, one workaround would be to provide implementations
    // for concrete implementations of that trait. In the long term, if we are
    // ever able to make the `new` constructor a const fn, we could use that
    // instead.
    ($name:ident, $bytes:expr, "signed integer") => {};
    ($name:ident, $bytes:expr, "floating point number") => {};
}

macro_rules! define_type {
    ($article:ident,
        $name:ident,
        $native:ident,
        $bits:expr,
        $bytes:expr,
        $read_method:ident,
        $write_method:ident,
        $number_kind:tt,
        [$($larger_native:ty),*],
        [$($larger_native_try:ty),*],
        [$($larger_byteorder:ident),*],
        [$($larger_byteorder_try:ident),*]) => {
        doc_comment! {
            concat!("A ", stringify!($bits), "-bit ", $number_kind,
            " stored in a given byte order.

`", stringify!($name), "` is like the native `", stringify!($native), "` type with
two major differences: First, it has no alignment requirement (its alignment is 1).
Second, the endianness of its memory layout is given by the type parameter `O`,
which can be any type which implements [`ByteOrder`]. In particular, this refers
to [`BigEndian`], [`LittleEndian`], [`NativeEndian`], and [`NetworkEndian`].

", stringify!($article), " `", stringify!($name), "` can be constructed using
the [`new`] method, and its contained value can be obtained as a native
`",stringify!($native), "` using the [`get`] method, or updated in place with
the [`set`] method. In all cases, if the endianness `O` is not the same as the
endianness of the current platform, an endianness swap will be performed in
order to uphold the invariants that a) the layout of `", stringify!($name), "`
has endianness `O` and that, b) the layout of `", stringify!($native), "` has
the platform's native endianness.

`", stringify!($name), "` implements [`FromBytes`], [`AsBytes`], and [`Unaligned`],
making it useful for parsing and serialization. See the module documentation for an
example of how it can be used for parsing UDP packets.

[`new`]: crate::byteorder::", stringify!($name), "::new
[`get`]: crate::byteorder::", stringify!($name), "::get
[`set`]: crate::byteorder::", stringify!($name), "::set
[`FromBytes`]: crate::FromBytes
[`AsBytes`]: crate::AsBytes
[`Unaligned`]: crate::Unaligned"),
            #[derive(Copy, Clone, Eq, PartialEq, Hash)]
            #[cfg_attr(any(feature = "derive", test), derive(KnownLayout, FromZeroes, FromBytes, AsBytes, Unaligned))]
            #[repr(transparent)]
            pub struct $name<O>([u8; $bytes], PhantomData<O>);
        }

        #[cfg(not(any(feature = "derive", test)))]
        impl_known_layout!(O => $name<O>);

        safety_comment! {
            /// SAFETY:
            /// `$name<O>` is `repr(transparent)`, and so it has the same layout
            /// as its only non-zero field, which is a `u8` array. `u8` arrays
            /// are `FromZeroes`, `FromBytes`, `AsBytes`, and `Unaligned`.
            impl_or_verify!(O => FromZeroes for $name<O>);
            impl_or_verify!(O => FromBytes for $name<O>);
            impl_or_verify!(O => AsBytes for $name<O>);
            impl_or_verify!(O => Unaligned for $name<O>);
        }

        impl<O> Default for $name<O> {
            #[inline(always)]
            fn default() -> $name<O> {
                $name::ZERO
            }
        }

        impl<O> $name<O> {
            /// The value zero.
            ///
            /// This constant should be preferred to constructing a new value
            /// using `new`, as `new` may perform an endianness swap depending
            /// on the endianness and platform.
            pub const ZERO: $name<O> = $name([0u8; $bytes], PhantomData);

            define_max_value_constant!($name, $bytes, $number_kind);

            /// Constructs a new value from bytes which are already in the
            /// endianness `O`.
            #[inline(always)]
            pub const fn from_bytes(bytes: [u8; $bytes]) -> $name<O> {
                $name(bytes, PhantomData)
            }
        }

        impl<O: ByteOrder> $name<O> {
            // TODO(joshlf): Make these const fns if the `ByteOrder` methods
            // ever become const fns.

            /// Constructs a new value, possibly performing an endianness swap
            /// to guarantee that the returned value has endianness `O`.
            #[inline(always)]
            pub fn new(n: $native) -> $name<O> {
                let mut out = $name::default();
                O::$write_method(&mut out.0[..], n);
                out
            }

            /// Returns the value as a primitive type, possibly performing an
            /// endianness swap to guarantee that the return value has the
            /// endianness of the native platform.
            #[inline(always)]
            pub fn get(self) -> $native {
                O::$read_method(&self.0[..])
            }

            /// Updates the value in place as a primitive type, possibly
            /// performing an endianness swap to guarantee that the stored value
            /// has the endianness `O`.
            #[inline(always)]
            pub fn set(&mut self, n: $native) {
                O::$write_method(&mut self.0[..], n);
            }
        }

        // The reasoning behind which traits to implement here is to only
        // implement traits which won't cause inference issues. Notably,
        // comparison traits like PartialEq and PartialOrd tend to cause
        // inference issues.

        impl<O: ByteOrder> From<$name<O>> for [u8; $bytes] {
            #[inline(always)]
            fn from(x: $name<O>) -> [u8; $bytes] {
                x.0
            }
        }

        impl<O: ByteOrder> From<[u8; $bytes]> for $name<O> {
            #[inline(always)]
            fn from(bytes: [u8; $bytes]) -> $name<O> {
                $name(bytes, PhantomData)
            }
        }

        impl<O: ByteOrder> From<$name<O>> for $native {
            #[inline(always)]
            fn from(x: $name<O>) -> $native {
                x.get()
            }
        }

        impl<O: ByteOrder> From<$native> for $name<O> {
            #[inline(always)]
            fn from(x: $native) -> $name<O> {
                $name::new(x)
            }
        }

        $(
            impl<O: ByteOrder> From<$name<O>> for $larger_native {
                #[inline(always)]
                fn from(x: $name<O>) -> $larger_native {
                    x.get().into()
                }
            }
        )*

        $(
            impl<O: ByteOrder> TryFrom<$larger_native_try> for $name<O> {
                type Error = TryFromIntError;
                #[inline(always)]
                fn try_from(x: $larger_native_try) -> Result<$name<O>, TryFromIntError> {
                    $native::try_from(x).map($name::new)
                }
            }
        )*

        $(
            impl<O: ByteOrder, P: ByteOrder> From<$name<O>> for $larger_byteorder<P> {
                #[inline(always)]
                fn from(x: $name<O>) -> $larger_byteorder<P> {
                    $larger_byteorder::new(x.get().into())
                }
            }
        )*

        $(
            impl<O: ByteOrder, P: ByteOrder> TryFrom<$larger_byteorder_try<P>> for $name<O> {
                type Error = TryFromIntError;
                #[inline(always)]
                fn try_from(x: $larger_byteorder_try<P>) -> Result<$name<O>, TryFromIntError> {
                    x.get().try_into().map($name::new)
                }
            }
        )*

        impl<O: ByteOrder> AsRef<[u8; $bytes]> for $name<O> {
            #[inline(always)]
            fn as_ref(&self) -> &[u8; $bytes] {
                &self.0
            }
        }

        impl<O: ByteOrder> AsMut<[u8; $bytes]> for $name<O> {
            #[inline(always)]
            fn as_mut(&mut self) -> &mut [u8; $bytes] {
                &mut self.0
            }
        }

        impl<O: ByteOrder> PartialEq<$name<O>> for [u8; $bytes] {
            #[inline(always)]
            fn eq(&self, other: &$name<O>) -> bool {
                self.eq(&other.0)
            }
        }

        impl<O: ByteOrder> PartialEq<[u8; $bytes]> for $name<O> {
            #[inline(always)]
            fn eq(&self, other: &[u8; $bytes]) -> bool {
                self.0.eq(other)
            }
        }

        impl_fmt_traits!($name, $native, $number_kind);
        impl_ops_traits!($name, $native, $number_kind);

        impl<O: ByteOrder> Debug for $name<O> {
            #[inline]
            fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
                // This results in a format like "U16(42)".
                f.debug_tuple(stringify!($name)).field(&self.get()).finish()
            }
        }
    };
}

define_type!(
    A,
    U16,
    u16,
    16,
    2,
    read_u16,
    write_u16,
    "unsigned integer",
    [u32, u64, u128, usize],
    [u32, u64, u128, usize],
    [U32, U64, U128],
    [U32, U64, U128]
);
define_type!(
    A,
    U32,
    u32,
    32,
    4,
    read_u32,
    write_u32,
    "unsigned integer",
    [u64, u128],
    [u64, u128],
    [U64, U128],
    [U64, U128]
);
define_type!(
    A,
    U64,
    u64,
    64,
    8,
    read_u64,
    write_u64,
    "unsigned integer",
    [u128],
    [u128],
    [U128],
    [U128]
);
define_type!(
    A,
    U128,
    u128,
    128,
    16,
    read_u128,
    write_u128,
    "unsigned integer",
    [],
    [],
    [],
    []
);
define_type!(
    An,
    I16,
    i16,
    16,
    2,
    read_i16,
    write_i16,
    "signed integer",
    [i32, i64, i128, isize],
    [i32, i64, i128, isize],
    [I32, I64, I128],
    [I32, I64, I128]
);
define_type!(
    An,
    I32,
    i32,
    32,
    4,
    read_i32,
    write_i32,
    "signed integer",
    [i64, i128],
    [i64, i128],
    [I64, I128],
    [I64, I128]
);
define_type!(
    An,
    I64,
    i64,
    64,
    8,
    read_i64,
    write_i64,
    "signed integer",
    [i128],
    [i128],
    [I128],
    [I128]
);
define_type!(
    An,
    I128,
    i128,
    128,
    16,
    read_i128,
    write_i128,
    "signed integer",
    [],
    [],
    [],
    []
);
define_type!(
    An,
    F32,
    f32,
    32,
    4,
    read_f32,
    write_f32,
    "floating point number",
    [f64],
    [],
    [F64],
    []
);
define_type!(
    An,
    F64,
    f64,
    64,
    8,
    read_f64,
    write_f64,
    "floating point number",
    [],
    [],
    [],
    []
);

macro_rules! module {
    ($name:ident, $trait:ident, $endianness_str:expr) => {
        /// Numeric primitives stored in
        #[doc = $endianness_str]
        /// byte order.
        pub mod $name {
            use byteorder::$trait;

            module!(@ty U16,  $trait, "16-bit unsigned integer", $endianness_str);
            module!(@ty U32,  $trait, "32-bit unsigned integer", $endianness_str);
            module!(@ty U64,  $trait, "64-bit unsigned integer", $endianness_str);
            module!(@ty U128, $trait, "128-bit unsigned integer", $endianness_str);
            module!(@ty I16,  $trait, "16-bit signed integer", $endianness_str);
            module!(@ty I32,  $trait, "32-bit signed integer", $endianness_str);
            module!(@ty I64,  $trait, "64-bit signed integer", $endianness_str);
            module!(@ty I128, $trait, "128-bit signed integer", $endianness_str);
            module!(@ty F32,  $trait, "32-bit floating point number", $endianness_str);
            module!(@ty F64,  $trait, "64-bit floating point number", $endianness_str);
        }
    };
    (@ty $ty:ident, $trait:ident, $desc_str:expr, $endianness_str:expr) => {
        /// A
        #[doc = $desc_str]
        /// stored in
        #[doc = $endianness_str]
        /// byte order.
        pub type $ty = crate::byteorder::$ty<$trait>;
    };
}

module!(big_endian, BigEndian, "big-endian");
module!(little_endian, LittleEndian, "little-endian");
module!(network_endian, NetworkEndian, "network-endian");
module!(native_endian, NativeEndian, "native-endian");
