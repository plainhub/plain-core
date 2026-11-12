use super::*;
use crate::tower::ServiceExt;
use std::{
    cell::Cell,
    future::{Ready, ready},
    pin::pin,
    rc::Rc,
    task::Waker,
};

type Payload = Box<[u8]>;
struct Probe {
    polls: Rc<Cell<usize>>,
    calls: Rc<Cell<usize>>,
    fail: bool,
}
impl Service<Payload> for Probe {
    type Response = Payload;
    type Error = &'static str;
    type Future = Ready<Result<Payload, &'static str>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.polls.set(self.polls.get() + 1);
        if self.fail {
            return Poll::Ready(Err("not ready"));
        }
        if self.polls.get() == 1 {
            cx.waker().wake_by_ref();
            Poll::Pending
        } else {
            Poll::Ready(Ok(()))
        }
    }
    fn call(&mut self, request: Payload) -> Self::Future {
        self.calls.set(self.calls.get() + 1);
        ready(Ok(request))
    }
}

#[test]
fn boxed_and_borrowed_services_preserve_readiness_and_payload_identity() {
    let polls = Rc::new(Cell::new(0));
    let calls = Rc::new(Cell::new(0));
    let mut service: Box<
        dyn Service<
                Payload,
                Response = Payload,
                Error = &'static str,
                Future = Ready<Result<Payload, &'static str>>,
            >,
    > = Box::new(Probe {
        polls: polls.clone(),
        calls: calls.clone(),
        fail: false,
    });
    let bytes = vec![7; 32].into_boxed_slice();
    let pointer = bytes.as_ptr();
    let mut future = pin!((&mut service).oneshot(bytes));
    let mut cx = Context::from_waker(Waker::noop());
    assert!(future.as_mut().poll(&mut cx).is_pending());
    assert_eq!(calls.get(), 0);
    let Poll::Ready(Ok(bytes)) = future.as_mut().poll(&mut cx) else {
        panic!("ready service must complete")
    };
    assert_eq!(bytes.as_ptr(), pointer);
    assert_eq!((polls.get(), calls.get()), (2, 1));
}

#[test]
fn readiness_failure_and_pending_cancellation_never_call_service() {
    for fail in [true, false] {
        let polls = Rc::new(Cell::new(0));
        let calls = Rc::new(Cell::new(0));
        let service = Probe {
            polls: polls.clone(),
            calls: calls.clone(),
            fail,
        };
        let mut cx = Context::from_waker(Waker::noop());
        {
            let mut future = pin!(service.oneshot(vec![0].into_boxed_slice()));
            let result = future.as_mut().poll(&mut cx);
            if fail {
                assert!(matches!(result, Poll::Ready(Err("not ready"))));
            } else {
                assert!(result.is_pending());
            }
        }
        assert_eq!((polls.get(), calls.get()), (1, 0));
    }
}
