use alloc::ffi::CString;
use alloc::string::ToString;
use alloc::vec::Vec;
use core::mem;

use objc2::encode::{EncodeArguments, EncodeReturn, Encoding};

/// Computes the raw signature string of the object corresponding to the block
/// taking `A` as inputs and returning `R`.
///
/// Although this is currently implemented on a best-effort basis, this should
/// still serve as a good way to obtain what to fill in the encoding string
/// when implementing [`crate::ManualBlockEncoding`].
///
/// # Example
///
/// ```ignore
/// assert_eq!(block_signature_string::<(i32, f32), u8>(), "C16@?0i8f12");
/// ```
#[allow(unused)]
pub(crate) fn block_signature_string<A, R>() -> CString
where
    A: EncodeArguments,
    R: EncodeReturn,
{
    block_signature_string_inner(A::ENCODINGS, &R::ENCODING_RETURN)
}

#[allow(unused)]
fn block_signature_string_inner(args: &[Encoding], ret: &Encoding) -> CString {
    // TODO: alignment?
    let arg_sizes = args
        .iter()
        .map(Encoding::size)
        .map(Option::unwrap_or_default)
        .collect::<Vec<_>>();
    let args_size = arg_sizes.iter().sum::<usize>();

    // Take the hidden block parameter into account.
    let mut off = mem::size_of::<*const ()>();
    let mut res = ret.to_string();
    res.push_str(&(off + args_size).to_string());
    res.push_str("@?0");

    for (arg_enc, arg_size) in args.iter().zip(arg_sizes) {
        res.push_str(&arg_enc.to_string());
        res.push_str(&off.to_string());
        off += arg_size;
    }

    // UNWRAP: The above construction only uses controlled `ToString`
    // implementations that do not include nul bytes.
    CString::new(res).unwrap()
}

#[cfg(test)]
#[path = "../../../../tests/native/block2/unit/encoding.rs"]
mod tests;
