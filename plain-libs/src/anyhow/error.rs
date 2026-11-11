use crate::anyhow::backtrace::Backtrace;
use crate::anyhow::chain::Chain;
use crate::anyhow::ptr::Mut;
use crate::anyhow::ptr::{Own, Ref};
use crate::anyhow::{Error, StdError};
use core::any::TypeId;
use core::fmt::{self, Debug, Display};
use core::mem::ManuallyDrop;
use core::ops::{Deref, DerefMut};
use core::panic::{RefUnwindSafe, UnwindSafe};
use core::ptr;
use core::ptr::NonNull;
use std::boxed::Box;
impl Error {
    #[cold]
    #[must_use]
    pub fn new<E>(error: E) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        let backtrace = backtrace_if_absent!(&error);
        Error::construct_from_std(error, backtrace)
    }
    #[cold]
    #[must_use]
    pub fn msg<M>(message: M) -> Self
    where
        M: Display + Debug + Send + Sync + 'static,
    {
        Error::construct_from_adhoc(message, backtrace!())
    }

    #[cold]
    pub(crate) fn construct_from_std<E>(error: E, backtrace: Option<Backtrace>) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        let vtable = &ErrorVTable {
            object_drop: object_drop::<E>,
            object_ref: object_ref::<E>,
            object_boxed: object_boxed::<E>,
            object_reallocate_boxed: object_reallocate_boxed::<E>,
            object_downcast: object_downcast::<E>,
            object_drop_rest: object_drop_front::<E>,
            object_backtrace: no_backtrace,
        };
        unsafe { Error::construct(error, vtable, backtrace) }
    }
    #[cold]
    pub(crate) fn construct_from_adhoc<M>(message: M, backtrace: Option<Backtrace>) -> Self
    where
        M: Display + Debug + Send + Sync + 'static,
    {
        use crate::anyhow::wrapper::MessageError;
        let error: MessageError<M> = MessageError(message);
        let vtable = &ErrorVTable {
            object_drop: object_drop::<MessageError<M>>,
            object_ref: object_ref::<MessageError<M>>,
            object_boxed: object_boxed::<MessageError<M>>,
            object_reallocate_boxed: object_reallocate_boxed::<MessageError<M>>,
            object_downcast: object_downcast::<M>,
            object_drop_rest: object_drop_front::<M>,
            object_backtrace: no_backtrace,
        };
        unsafe { Error::construct(error, vtable, backtrace) }
    }
    #[cold]
    pub(crate) fn construct_from_display<M>(message: M, backtrace: Option<Backtrace>) -> Self
    where
        M: Display + Send + Sync + 'static,
    {
        use crate::anyhow::wrapper::DisplayError;
        let error: DisplayError<M> = DisplayError(message);
        let vtable = &ErrorVTable {
            object_drop: object_drop::<DisplayError<M>>,
            object_ref: object_ref::<DisplayError<M>>,
            object_boxed: object_boxed::<DisplayError<M>>,
            object_reallocate_boxed: object_reallocate_boxed::<DisplayError<M>>,
            object_downcast: object_downcast::<M>,
            object_drop_rest: object_drop_front::<M>,
            object_backtrace: no_backtrace,
        };
        unsafe { Error::construct(error, vtable, backtrace) }
    }
    #[cold]
    pub(crate) fn construct_from_context<C, E>(
        context: C,
        error: E,
        backtrace: Option<Backtrace>,
    ) -> Self
    where
        C: Display + Send + Sync + 'static,
        E: StdError + Send + Sync + 'static,
    {
        let error: ContextError<C, E> = ContextError { context, error };
        let vtable = &ErrorVTable {
            object_drop: object_drop::<ContextError<C, E>>,
            object_ref: object_ref::<ContextError<C, E>>,
            object_boxed: object_boxed::<ContextError<C, E>>,
            object_reallocate_boxed: object_reallocate_boxed::<ContextError<C, E>>,
            object_downcast: context_downcast::<C, E>,
            object_drop_rest: context_drop_rest::<C, E>,
            object_backtrace: no_backtrace,
        };
        unsafe { Error::construct(error, vtable, backtrace) }
    }
    #[cold]
    pub(crate) fn construct_from_boxed(
        error: Box<dyn StdError + Send + Sync>,
        backtrace: Option<Backtrace>,
    ) -> Self {
        use crate::anyhow::wrapper::BoxedError;
        let error = BoxedError(error);
        let vtable = &ErrorVTable {
            object_drop: object_drop::<BoxedError>,
            object_ref: object_ref::<BoxedError>,
            object_boxed: object_boxed::<BoxedError>,
            object_reallocate_boxed: object_reallocate_boxed::<BoxedError>,
            object_downcast: object_downcast::<Box<dyn StdError + Send + Sync>>,
            object_drop_rest: object_drop_front::<Box<dyn StdError + Send + Sync>>,
            object_backtrace: no_backtrace,
        };
        unsafe { Error::construct(error, vtable, backtrace) }
    }
    #[cold]
    unsafe fn construct<E>(
        error: E,
        vtable: &'static ErrorVTable,
        backtrace: Option<Backtrace>,
    ) -> Self
    where
        E: StdError + Send + Sync + 'static,
    {
        let inner: Box<ErrorImpl<E>> = Box::new(ErrorImpl {
            vtable,
            backtrace,
            _object: error,
        });
        let inner = Own::new(inner).cast::<ErrorImpl>();
        Error { inner }
    }
    #[cold]
    #[must_use]
    pub fn context<C>(self, context: C) -> Self
    where
        C: Display + Send + Sync + 'static,
    {
        let error: ContextError<C, Error> = ContextError {
            context,
            error: self,
        };
        let vtable = &ErrorVTable {
            object_drop: object_drop::<ContextError<C, Error>>,
            object_ref: object_ref::<ContextError<C, Error>>,
            object_boxed: object_boxed::<ContextError<C, Error>>,
            object_reallocate_boxed: object_reallocate_boxed::<ContextError<C, Error>>,
            object_downcast: context_chain_downcast::<C>,
            object_drop_rest: context_chain_drop_rest::<C>,
            object_backtrace: context_backtrace::<C>,
        };
        let backtrace = None;
        unsafe { Error::construct(error, vtable, backtrace) }
    }
    pub fn backtrace(&self) -> &Backtrace {
        unsafe { ErrorImpl::backtrace(self.inner.by_ref()) }
    }
    #[cold]
    pub fn chain(&self) -> Chain {
        unsafe { ErrorImpl::chain(self.inner.by_ref()) }
    }

    pub fn is<E>(&self) -> bool
    where
        E: Display + Debug + Send + Sync + 'static,
    {
        self.downcast_ref::<E>().is_some()
    }
    pub fn downcast<E>(mut self) -> Result<E, Self>
    where
        E: Display + Debug + Send + Sync + 'static,
    {
        let target = TypeId::of::<E>();
        let inner = self.inner.by_mut();
        unsafe {
            let addr = match (vtable(inner.ptr).object_downcast)(inner.by_ref(), target) {
                Some(addr) => addr.by_mut().extend(),
                None => return Err(self),
            };
            let outer = ManuallyDrop::new(self);
            let error = addr.cast::<E>().read();
            (vtable(outer.inner.ptr).object_drop_rest)(outer.inner, target);
            Ok(error)
        }
    }
    pub fn downcast_ref<E>(&self) -> Option<&E>
    where
        E: Display + Debug + Send + Sync + 'static,
    {
        let target = TypeId::of::<E>();
        unsafe {
            let addr = (vtable(self.inner.ptr).object_downcast)(self.inner.by_ref(), target)?;
            Some(addr.cast::<E>().deref())
        }
    }

    #[must_use]
    pub fn into_boxed_dyn_error(self) -> Box<dyn StdError + Send + Sync + 'static> {
        let outer = ManuallyDrop::new(self);
        unsafe { (vtable(outer.inner.ptr).object_boxed)(outer.inner) }
    }
}
impl<E> From<E> for Error
where
    E: StdError + Send + Sync + 'static,
{
    #[cold]
    fn from(error: E) -> Self {
        let backtrace = backtrace_if_absent!(&error);
        Error::construct_from_std(error, backtrace)
    }
}
impl Deref for Error {
    type Target = dyn StdError + Send + Sync + 'static;
    fn deref(&self) -> &Self::Target {
        unsafe { ErrorImpl::error(self.inner.by_ref()) }
    }
}
impl DerefMut for Error {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { ErrorImpl::error_mut(self.inner.by_mut()) }
    }
}
impl Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        unsafe { ErrorImpl::display(self.inner.by_ref(), formatter) }
    }
}
impl Debug for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        unsafe { ErrorImpl::debug(self.inner.by_ref(), formatter) }
    }
}
impl Drop for Error {
    fn drop(&mut self) {
        unsafe {
            (vtable(self.inner.ptr).object_drop)(self.inner);
        }
    }
}
struct ErrorVTable {
    object_drop: unsafe fn(Own<ErrorImpl>),
    object_ref: unsafe fn(Ref<ErrorImpl>) -> Ref<dyn StdError + Send + Sync + 'static>,
    object_boxed: unsafe fn(Own<ErrorImpl>) -> Box<dyn StdError + Send + Sync + 'static>,
    object_reallocate_boxed: unsafe fn(Own<ErrorImpl>) -> Box<dyn StdError + Send + Sync + 'static>,
    object_downcast: unsafe fn(Ref<ErrorImpl>, TypeId) -> Option<Ref<()>>,
    object_drop_rest: unsafe fn(Own<ErrorImpl>, TypeId),
    object_backtrace: unsafe fn(Ref<ErrorImpl>) -> Option<&Backtrace>,
}
unsafe fn object_drop<E>(e: Own<ErrorImpl>) {
    unsafe {
        let unerased_own = e.cast::<ErrorImpl<E>>();
        drop(unsafe { unerased_own.boxed() });
    }
}
unsafe fn object_drop_front<E>(e: Own<ErrorImpl>, target: TypeId) {
    unsafe {
        let _ = target;
        let unerased_own = e.cast::<ErrorImpl<ManuallyDrop<E>>>();
        drop(unsafe { unerased_own.boxed() });
    }
}
unsafe fn object_ref<E>(e: Ref<ErrorImpl>) -> Ref<dyn StdError + Send + Sync + 'static>
where
    E: StdError + Send + Sync + 'static,
{
    unsafe {
        let unerased_ref = e.cast::<ErrorImpl<E>>();
        Ref::from_raw(unsafe {
            NonNull::new_unchecked(ptr::addr_of!((*unerased_ref.as_ptr())._object).cast_mut())
        })
    }
}
unsafe fn object_boxed<E>(e: Own<ErrorImpl>) -> Box<dyn StdError + Send + Sync + 'static>
where
    E: StdError + Send + Sync + 'static,
{
    unsafe {
        let unerased_own = e.cast::<ErrorImpl<E>>();
        unsafe { unerased_own.boxed() }
    }
}
unsafe fn object_reallocate_boxed<E>(e: Own<ErrorImpl>) -> Box<dyn StdError + Send + Sync + 'static>
where
    E: StdError + Send + Sync + 'static,
{
    unsafe {
        let unerased_own = e.cast::<ErrorImpl<E>>();
        Box::new(unsafe { unerased_own.boxed() }._object)
    }
}
unsafe fn object_downcast<E>(e: Ref<ErrorImpl>, target: TypeId) -> Option<Ref<()>>
where
    E: 'static,
{
    unsafe {
        if TypeId::of::<E>() == target {
            let unerased_ref = e.cast::<ErrorImpl<E>>();
            Some(
                Ref::from_raw(unsafe {
                    NonNull::new_unchecked(
                        ptr::addr_of!((*unerased_ref.as_ptr())._object).cast_mut(),
                    )
                })
                .cast::<()>(),
            )
        } else {
            None
        }
    }
}
fn no_backtrace(e: Ref<ErrorImpl>) -> Option<&Backtrace> {
    let _ = e;
    None
}
unsafe fn context_downcast<C, E>(e: Ref<ErrorImpl>, target: TypeId) -> Option<Ref<()>>
where
    C: 'static,
    E: 'static,
{
    unsafe {
        if TypeId::of::<C>() == target {
            let unerased_ref = e.cast::<ErrorImpl<ContextError<C, E>>>();
            let inner = unsafe { ptr::addr_of!((*unerased_ref.as_ptr())._object.context) };
            Some(Ref::from_raw(unsafe { NonNull::new_unchecked(inner.cast_mut()) }).cast::<()>())
        } else if TypeId::of::<E>() == target {
            let unerased_ref = e.cast::<ErrorImpl<ContextError<C, E>>>();
            let inner = unsafe { ptr::addr_of!((*unerased_ref.as_ptr())._object.error) };
            Some(Ref::from_raw(unsafe { NonNull::new_unchecked(inner.cast_mut()) }).cast::<()>())
        } else {
            None
        }
    }
}
unsafe fn context_drop_rest<C, E>(e: Own<ErrorImpl>, target: TypeId)
where
    C: 'static,
    E: 'static,
{
    unsafe {
        if TypeId::of::<C>() == target {
            let unerased_own = e.cast::<ErrorImpl<ContextError<ManuallyDrop<C>, E>>>();
            drop(unsafe { unerased_own.boxed() });
        } else {
            let unerased_own = e.cast::<ErrorImpl<ContextError<C, ManuallyDrop<E>>>>();
            drop(unsafe { unerased_own.boxed() });
        }
    }
}
unsafe fn context_chain_downcast<C>(e: Ref<ErrorImpl>, target: TypeId) -> Option<Ref<()>>
where
    C: 'static,
{
    unsafe {
        let unerased_ref = e.cast::<ErrorImpl<ContextError<C, Error>>>();
        if TypeId::of::<C>() == target {
            let inner = unsafe { ptr::addr_of!((*unerased_ref.as_ptr())._object.context) };
            Some(Ref::from_raw(unsafe { NonNull::new_unchecked(inner.cast_mut()) }).cast::<()>())
        } else {
            let unerased = unsafe { unerased_ref.deref() };
            let source = &unerased._object.error;
            unsafe { (vtable(source.inner.ptr).object_downcast)(source.inner.by_ref(), target) }
        }
    }
}
unsafe fn context_chain_drop_rest<C>(e: Own<ErrorImpl>, target: TypeId)
where
    C: 'static,
{
    unsafe {
        if TypeId::of::<C>() == target {
            let unerased_own = e.cast::<ErrorImpl<ContextError<ManuallyDrop<C>, Error>>>();
            drop(unsafe { unerased_own.boxed() });
        } else {
            let unerased_own = e.cast::<ErrorImpl<ContextError<C, ManuallyDrop<Error>>>>();
            let unerased = unsafe { unerased_own.boxed() };
            let inner = unerased._object.error.inner;
            drop(unerased);
            let vtable = unsafe { vtable(inner.ptr) };
            unsafe { (vtable.object_drop_rest)(inner, target) };
        }
    }
}
unsafe fn context_backtrace<C>(e: Ref<ErrorImpl>) -> Option<&Backtrace>
where
    C: 'static,
{
    unsafe {
        let unerased_ref = e.cast::<ErrorImpl<ContextError<C, Error>>>();
        let unerased = unsafe { unerased_ref.deref() };
        let backtrace = unsafe { ErrorImpl::backtrace(unerased._object.error.inner.by_ref()) };
        Some(backtrace)
    }
}
#[repr(C)]
pub(crate) struct ErrorImpl<E = ()> {
    vtable: &'static ErrorVTable,
    backtrace: Option<Backtrace>,
    _object: E,
}
unsafe fn vtable(p: NonNull<ErrorImpl>) -> &'static ErrorVTable {
    unsafe { *(p.as_ptr() as *const &'static ErrorVTable) }
}
#[repr(C)]
pub(crate) struct ContextError<C, E> {
    pub context: C,
    pub error: E,
}
impl<E> ErrorImpl<E> {
    fn erase(&self) -> Ref<ErrorImpl> {
        Ref::new(self).cast::<ErrorImpl>()
    }
}
impl ErrorImpl {
    pub(crate) unsafe fn error(this: Ref<Self>) -> &(dyn StdError + Send + Sync + 'static) {
        unsafe { (vtable(this.ptr).object_ref)(this).deref() }
    }
    pub(crate) unsafe fn error_mut(this: Mut<Self>) -> &mut (dyn StdError + Send + Sync + 'static) {
        unsafe {
            (vtable(this.ptr).object_ref)(this.by_ref())
                .by_mut()
                .deref_mut()
        }
    }
    pub(crate) unsafe fn backtrace(this: Ref<Self>) -> &Backtrace {
        unsafe { this.deref() }
            .backtrace
            .as_ref()
            .or_else(|| {
                return unsafe { (vtable(this.ptr).object_backtrace)(this) };
            })
            .expect("backtrace capture failed")
    }
    #[cold]
    pub(crate) unsafe fn chain(this: Ref<Self>) -> Chain {
        Chain::new(unsafe { Self::error(this) })
    }
}
impl<E> StdError for ErrorImpl<E>
where
    E: StdError,
{
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        unsafe { ErrorImpl::error(self.erase()).source() }
    }
}
impl<E> Debug for ErrorImpl<E>
where
    E: Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        unsafe { ErrorImpl::debug(self.erase(), formatter) }
    }
}
impl<E> Display for ErrorImpl<E>
where
    E: Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        unsafe { Display::fmt(ErrorImpl::error(self.erase()), formatter) }
    }
}
impl From<Error> for Box<dyn StdError + Send + Sync + 'static> {
    #[cold]
    fn from(error: Error) -> Self {
        error.into_boxed_dyn_error()
    }
}
impl From<Error> for Box<dyn StdError + Send + 'static> {
    #[cold]
    fn from(error: Error) -> Self {
        error.into_boxed_dyn_error()
    }
}
impl From<Error> for Box<dyn StdError + 'static> {
    #[cold]
    fn from(error: Error) -> Self {
        error.into_boxed_dyn_error()
    }
}
impl AsRef<dyn StdError + Send + Sync> for Error {
    fn as_ref(&self) -> &(dyn StdError + Send + Sync + 'static) {
        &**self
    }
}
impl AsRef<dyn StdError> for Error {
    fn as_ref(&self) -> &(dyn StdError + 'static) {
        &**self
    }
}
impl UnwindSafe for Error {}
impl RefUnwindSafe for Error {}
