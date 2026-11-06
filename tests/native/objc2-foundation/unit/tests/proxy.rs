#![cfg(any())]
use crate::NSProxy;
use objc2::ClassType;

#[test]
fn dummy() {
    let _cls = NSProxy::class();
}
