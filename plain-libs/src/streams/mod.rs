#![forbid(unsafe_code)]

use futures_core::{Stream, stream::FusedStream};
use std::{
    future::{Future, poll_fn},
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

pub struct Yielder<Item> {
    output: Arc<Mutex<Option<Item>>>,
}

impl<Item> Yielder<Item> {
    pub async fn yield_item(&mut self, item: Item) {
        let mut item = Some(item);
        poll_fn(|_| match item.take() {
            Some(item) => {
                *self.output.lock().unwrap() = Some(item);
                Poll::Pending
            }
            None => Poll::Ready(()),
        })
        .await;
    }
}

pub fn stream_fn<F, Fut, Item>(func: F) -> impl Stream<Item = Item>
where
    F: FnOnce(Yielder<Item>) -> Fut,
    Fut: Future<Output = ()>,
{
    AsynkStrim {
        func: Some(func),
        future: None,
        output: Arc::new(Mutex::new(None)),
    }
}

struct AsynkStrim<F, Fut, Item> {
    func: Option<F>,
    future: Option<Pin<Box<Fut>>>,
    output: Arc<Mutex<Option<Item>>>,
}

// The producer stays pinned in its Box; the factory and output slot are never pinned.
impl<F, Fut, Item> Unpin for AsynkStrim<F, Fut, Item> {}

impl<F, Fut, Item> Stream for AsynkStrim<F, Fut, Item>
where
    F: FnOnce(Yielder<Item>) -> Fut,
    Fut: Future<Output = ()>,
{
    type Item = Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Item>> {
        let this = self.get_mut();
        if let Some(func) = this.func.take() {
            this.future = Some(Box::pin(func(Yielder {
                output: Arc::clone(&this.output),
            })));
        }
        let Some(future) = &mut this.future else {
            return Poll::Ready(None);
        };
        let completed = future.as_mut().poll(cx).is_ready();
        let item = this.output.lock().unwrap().take();
        if completed {
            this.future = None;
        }
        match item {
            Some(item) => Poll::Ready(Some(item)),
            None if completed => Poll::Ready(None),
            None => Poll::Pending,
        }
    }
}

impl<F, Fut, Item> FusedStream for AsynkStrim<F, Fut, Item>
where
    F: FnOnce(Yielder<Item>) -> Fut,
    Fut: Future<Output = ()>,
{
    fn is_terminated(&self) -> bool {
        self.func.is_none() && self.future.is_none()
    }
}

#[cfg(test)]
#[path = "../../../tests/plain-libs/unit/streams/lib.rs"]
mod tests;
