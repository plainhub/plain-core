macro_rules! opaque_future { ($(#[$m : meta]) * pub type $name : ident = $actual : ty ;) => { opaque_future ! { $(#[$m]) * pub type $name <> = $actual ; } } ; ($(#[$m : meta]) * pub type $name : ident <$($param : ident) ,*> = $actual : ty ;) => { pin_project_lite::pin_project ! { $(#[$m]) * pub struct $name <$($param) ,*> { #[pin] future : $actual , } } impl <$($param) ,*> $name <$($param) ,*> { pub (crate) fn new (future : $actual) -> Self { Self { future } } } impl <$($param) ,*> std::fmt::Debug for $name <$($param) ,*> { fn fmt (& self , f : & mut std::fmt::Formatter <'_ >) -> std::fmt::Result { f . debug_struct (stringify ! ($name)) . finish_non_exhaustive () } } impl <$($param) ,*> std::future::Future for $name <$($param) ,*> where $actual : std::future::Future , { type Output = <$actual as std::future::Future >::Output ; #[inline] fn poll (self : std::pin::Pin <& mut Self >, cx : & mut std::task::Context <'_ >,) -> std::task::Poll < Self::Output > { self . project () . future . poll (cx) } } } ; }

macro_rules! trace {
    ($($tt : tt) *) => {};
}
macro_rules! error {
    ($($tt : tt) *) => {};
}
