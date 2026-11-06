use core::ptr;

use crate::encode::{Encode, RefEncode};
use crate::rc::Retained;
use crate::runtime::{AnyClass, MessageReceiver, Sel};
use crate::ClassType;

use super::null_error::encountered_error;
use super::{
    ConvertArguments, KindSendMessage, KindSendMessageSuper, RetainSemantics, TupleExtender,
};

//
// MsgSend
//

pub trait MsgSend<Receiver, Return> {
    #[track_caller]
    unsafe fn send_message<A: ConvertArguments>(receiver: Receiver, sel: Sel, args: A) -> Return;
}

impl<Receiver, Return, MethodFamily> MsgSend<Receiver, Return> for MethodFamily
where
    MethodFamily: RetainSemantics<Receiver, Return, KindSendMessage>,
{
    #[inline]
    unsafe fn send_message<A: ConvertArguments>(receiver: Receiver, sel: Sel, args: A) -> Return {
        let ptr = Self::prepare_message_send(receiver).__as_raw_receiver();

        // SAFETY: The writeback helper is not leaked (it is dropped at the
        // end of this scope).
        let (args, _helper) = unsafe { A::__into_arguments(args) };

        // SAFETY: Upheld by caller.
        let ret = unsafe { MessageReceiver::send_message(ptr, sel, args) };

        // SAFETY: The pointers are valid (or, in the case of the receiver
        // pointer, at least valid when the message send is not `init`).
        unsafe { Self::convert_message_return(ret, ptr, sel) }
    }
}

//
// MsgSendSuper
//

pub trait MsgSendSuper<Receiver, Return> {
    type Inner: ?Sized + RefEncode;

    #[track_caller]
    unsafe fn send_super_message<A: ConvertArguments>(
        receiver: Receiver,
        superclass: &AnyClass,
        sel: Sel,
        args: A,
    ) -> Return;

    #[inline]
    #[track_caller]
    unsafe fn send_super_message_static<A: ConvertArguments>(
        receiver: Receiver,
        sel: Sel,
        args: A,
    ) -> Return
    where
        Self::Inner: ClassType,
        <Self::Inner as ClassType>::Super: ClassType,
    {
        unsafe {
            Self::send_super_message(
                receiver,
                <Self::Inner as ClassType>::Super::class(),
                sel,
                args,
            )
        }
    }
}

impl<Receiver, Return, MethodFamily> MsgSendSuper<Receiver, Return> for MethodFamily
where
    MethodFamily: RetainSemantics<Receiver, Return, KindSendMessageSuper>,
{
    type Inner = <<MethodFamily as RetainSemantics<Receiver, Return, KindSendMessageSuper>>::ReceiverInner as MessageReceiver>::__Inner;

    #[inline]
    unsafe fn send_super_message<A: ConvertArguments>(
        receiver: Receiver,
        superclass: &AnyClass,
        sel: Sel,
        args: A,
    ) -> Return {
        let ptr = Self::prepare_message_send(receiver).__as_raw_receiver();

        // SAFETY: The writeback helper is not leaked (it is dropped at the
        // end of this scope).
        let (args, _helper) = unsafe { A::__into_arguments(args) };

        // SAFETY: Upheld by caller.
        let ret = unsafe { MessageReceiver::send_super_message(ptr, superclass, sel, args) };

        // SAFETY: The pointers are valid (or, in the case of the receiver
        // pointer, at least valid when the message send is not `init`).
        unsafe { Self::convert_message_return(ret, ptr, sel) }
    }
}

//
// MsgSendError
//

pub trait MsgSendError<Receiver, Return> {
    /// Add an extra error argument to the argument list, call `send_message`
    /// with that, and return an error if one occurred.
    #[track_caller]
    unsafe fn send_message_error<A, E>(
        receiver: Receiver,
        sel: Sel,
        args: A,
    ) -> Result<Return, Retained<E>>
    where
        *mut *mut E: Encode,
        A: TupleExtender<*mut *mut E>,
        <A as TupleExtender<*mut *mut E>>::PlusOneArgument: ConvertArguments,
        E: ClassType;
}

// `Option<Retained<T>>` return.
impl<Receiver, Return, MethodFamily> MsgSendError<Receiver, Retained<Return>> for MethodFamily
where
    MethodFamily: MsgSend<Receiver, Option<Retained<Return>>>,
{
    #[inline]
    unsafe fn send_message_error<A, E>(
        receiver: Receiver,
        sel: Sel,
        args: A,
    ) -> Result<Retained<Return>, Retained<E>>
    where
        *mut *mut E: Encode,
        A: TupleExtender<*mut *mut E>,
        <A as TupleExtender<*mut *mut E>>::PlusOneArgument: ConvertArguments,
        E: ClassType,
    {
        let mut err: *mut E = ptr::null_mut();
        let args = args.add_argument(&mut err);
        let ret = unsafe { Self::send_message(receiver, sel, args) };
        // As per the Cocoa documentation:
        // > Success or failure is indicated by the return value of the
        // > method. Although Cocoa methods that indirectly return error
        // > objects in the Cocoa error domain are guaranteed to return such
        // > objects if the method indicates failure by directly returning
        // > `nil` or `NO`, you should always check that the return value is
        // > `nil` or `NO` before attempting to do anything with the `NSError`
        // > object.
        if let Some(ret) = ret {
            // In this case, the error is likely not created. If it is, it is
            // autoreleased anyhow, so it would be a waste to retain and
            // release it here.
            Ok(ret)
        } else {
            // In this case, the error has very likely been created, but has
            // been autoreleased (as is common for "out parameters", see
            // `src/__macro_helpers/writeback.rs`). Hence we need to retain it
            // if we want it to live across autorelease pools.
            //
            // SAFETY: The message send is guaranteed to populate the error
            // object, or leave it as NULL. The error is shared, and all
            // holders of the error know this, so is safe to retain.
            Err(unsafe { encountered_error(err) })
        }
    }
}

// Bool return.
impl<Receiver, MethodFamily> MsgSendError<Receiver, ()> for MethodFamily
where
    MethodFamily: MsgSend<Receiver, bool>,
{
    #[inline]
    unsafe fn send_message_error<A, E>(
        receiver: Receiver,
        sel: Sel,
        args: A,
    ) -> Result<(), Retained<E>>
    where
        *mut *mut E: Encode,
        A: TupleExtender<*mut *mut E>,
        <A as TupleExtender<*mut *mut E>>::PlusOneArgument: ConvertArguments,
        E: ClassType,
    {
        let mut err: *mut E = ptr::null_mut();
        let args = args.add_argument(&mut err);
        let ret = unsafe { Self::send_message(receiver, sel, args) };
        if ret {
            Ok(())
        } else {
            Err(unsafe { encountered_error(err) })
        }
    }
}

//
// MsgSendSuperError
//

pub trait MsgSendSuperError<Receiver, Return> {
    type Inner: ?Sized + RefEncode;

    #[track_caller]
    unsafe fn send_super_message_error<A, E>(
        receiver: Receiver,
        superclass: &AnyClass,
        sel: Sel,
        args: A,
    ) -> Result<Return, Retained<E>>
    where
        *mut *mut E: Encode,
        A: TupleExtender<*mut *mut E>,
        <A as TupleExtender<*mut *mut E>>::PlusOneArgument: ConvertArguments,
        E: ClassType;

    #[track_caller]
    #[inline]
    unsafe fn send_super_message_static_error<A, E>(
        receiver: Receiver,
        sel: Sel,
        args: A,
    ) -> Result<Return, Retained<E>>
    where
        Self::Inner: ClassType,
        <Self::Inner as ClassType>::Super: ClassType,
        *mut *mut E: Encode,
        A: TupleExtender<*mut *mut E>,
        <A as TupleExtender<*mut *mut E>>::PlusOneArgument: ConvertArguments,
        E: ClassType,
    {
        unsafe {
            Self::send_super_message_error(
                receiver,
                <Self::Inner as ClassType>::Super::class(),
                sel,
                args,
            )
        }
    }
}

// `Option<Retained<T>>` return.
impl<Receiver, Return, MethodFamily> MsgSendSuperError<Receiver, Retained<Return>> for MethodFamily
where
    MethodFamily: MsgSendSuper<Receiver, Option<Retained<Return>>>,
{
    type Inner = <MethodFamily as MsgSendSuper<Receiver, Option<Retained<Return>>>>::Inner;

    #[inline]
    unsafe fn send_super_message_error<A, E>(
        receiver: Receiver,
        superclass: &AnyClass,
        sel: Sel,
        args: A,
    ) -> Result<Retained<Return>, Retained<E>>
    where
        *mut *mut E: Encode,
        A: TupleExtender<*mut *mut E>,
        <A as TupleExtender<*mut *mut E>>::PlusOneArgument: ConvertArguments,
        E: ClassType,
    {
        let mut err: *mut E = ptr::null_mut();
        let args = args.add_argument(&mut err);
        // SAFETY: See `send_message_error`
        let ret = unsafe { Self::send_super_message(receiver, superclass, sel, args) };
        if let Some(ret) = ret {
            Ok(ret)
        } else {
            // SAFETY: See `send_message_error`
            Err(unsafe { encountered_error(err) })
        }
    }
}

// Bool return.
impl<Receiver, MethodFamily> MsgSendSuperError<Receiver, ()> for MethodFamily
where
    MethodFamily: MsgSendSuper<Receiver, bool>,
{
    type Inner = <MethodFamily as MsgSendSuper<Receiver, bool>>::Inner;

    #[inline]
    unsafe fn send_super_message_error<A, E>(
        receiver: Receiver,
        superclass: &AnyClass,
        sel: Sel,
        args: A,
    ) -> Result<(), Retained<E>>
    where
        *mut *mut E: Encode,
        A: TupleExtender<*mut *mut E>,
        <A as TupleExtender<*mut *mut E>>::PlusOneArgument: ConvertArguments,
        E: ClassType,
    {
        let mut err: *mut E = ptr::null_mut();
        let args = args.add_argument(&mut err);
        // SAFETY: See `send_message_error`
        let ret = unsafe { Self::send_super_message(receiver, superclass, sel, args) };
        if ret {
            Ok(())
        } else {
            // SAFETY: See `send_message_error`
            Err(unsafe { encountered_error(err) })
        }
    }
}

#[cfg(test)]
#[path = "../../../../../tests/native/objc2/unit/__macro_helpers/msg_send_retained.rs"]
mod tests;
