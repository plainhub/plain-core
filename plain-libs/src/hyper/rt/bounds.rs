pub use self::h2_client::Http2ClientConnExec;
pub(crate) use self::h2_common::Http2UpgradedExec;
pub use self::h2_server::Http2ServerConnExec;
mod h2_common {
    use crate::hyper::proto::h2::upgrade::UpgradedSendStreamTask;
    use crate::hyper::rt::Executor;
    pub trait Http2UpgradedExec<B> {
        fn execute_upgrade(&self, fut: UpgradedSendStreamTask<B>);
    }
    impl<E, B> Http2UpgradedExec<B> for E
    where
        E: Executor<UpgradedSendStreamTask<B>>,
    {
        fn execute_upgrade(&self, fut: UpgradedSendStreamTask<B>) {
            self.execute(fut);
        }
    }
}
mod h2_client {
    use crate::hyper::rt::{Read, Write};
    use crate::hyper::{proto::h2::client::H2ClientFuture, rt::Executor};
    use std::{error::Error, future::Future};
    pub trait Http2ClientConnExec<B, T>:
        super::Http2UpgradedExec<B::Data> + sealed_client::Sealed<(B, T)> + Clone
    where
        B: crate::http_body::Body,
        B::Error: Into<Box<dyn Error + Send + Sync>>,
        T: Read + Write + Unpin,
    {
        fn execute_h2_future(&mut self, future: H2ClientFuture<B, T, Self>);
    }
    impl<E, B, T> Http2ClientConnExec<B, T> for E
    where
        E: Clone,
        E: Executor<H2ClientFuture<B, T, E>>,
        E: super::Http2UpgradedExec<B::Data>,
        B: crate::http_body::Body + 'static,
        B::Error: Into<Box<dyn Error + Send + Sync>>,
        H2ClientFuture<B, T, E>: Future<Output = ()>,
        T: Read + Write + Unpin,
    {
        fn execute_h2_future(&mut self, future: H2ClientFuture<B, T, E>) {
            self.execute(future);
        }
    }
    impl<E, B, T> sealed_client::Sealed<(B, T)> for E
    where
        E: Clone,
        E: Executor<H2ClientFuture<B, T, E>>,
        E: super::Http2UpgradedExec<B::Data>,
        B: crate::http_body::Body + 'static,
        B::Error: Into<Box<dyn Error + Send + Sync>>,
        H2ClientFuture<B, T, E>: Future<Output = ()>,
        T: Read + Write + Unpin,
    {
    }
    mod sealed_client {
        pub trait Sealed<X> {}
    }
}
mod h2_server {
    use crate::http_body::Body;
    use crate::hyper::{proto::h2::server::H2Stream, rt::Executor};
    use std::future::Future;
    pub trait Http2ServerConnExec<F, B: Body>:
        super::Http2UpgradedExec<B::Data> + sealed::Sealed<(F, B)> + Clone
    {
        fn execute_h2stream(&mut self, fut: H2Stream<F, B, Self>);
    }
    impl<E, F, B> Http2ServerConnExec<F, B> for E
    where
        E: Clone,
        E: Executor<H2Stream<F, B, E>>,
        E: super::Http2UpgradedExec<B::Data>,
        H2Stream<F, B, E>: Future<Output = ()>,
        B: Body,
    {
        fn execute_h2stream(&mut self, fut: H2Stream<F, B, E>) {
            self.execute(fut);
        }
    }
    impl<E, F, B> sealed::Sealed<(F, B)> for E
    where
        E: Clone,
        E: Executor<H2Stream<F, B, E>>,
        E: super::Http2UpgradedExec<B::Data>,
        H2Stream<F, B, E>: Future<Output = ()>,
        B: Body,
    {
    }
    mod sealed {
        pub trait Sealed<T> {}
    }
}
