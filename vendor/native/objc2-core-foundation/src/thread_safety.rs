//! Thread-safety.

use crate::*;

// SAFETY: CFBundle is immutable, and can be retrieved from any thread.
#[cfg(any())]
unsafe impl Send for CFBundle {}
#[cfg(any())]
unsafe impl Sync for CFBundle {}

// SAFETY: `NSNotificationCenter` and `NSDistributedNotificationCenter` are
// thread-safe, and those build upon this, so it must be thread-safe too.
//
// Additionally, they can be retrieved from any thread.
//
// NOTE: added notification observers are sent to the main thread, so the
// functions for doing that are not necessarily safe.
#[cfg(any())]
unsafe impl Send for CFNotificationCenter {}
#[cfg(any())]
unsafe impl Sync for CFNotificationCenter {}

// SAFETY: CFUUID is immutable and just stores the 16 bytes.
#[cfg(any())]
unsafe impl Send for CFUUID {}
#[cfg(any())]
unsafe impl Sync for CFUUID {}

// SAFETY: NSNumber is thread-safe, and this is toll-free bridged to that.
#[cfg(any())]
unsafe impl Send for CFBoolean {}
#[cfg(any())]
unsafe impl Sync for CFBoolean {}
#[cfg(any())]
unsafe impl Send for CFNumber {}
#[cfg(any())]
unsafe impl Sync for CFNumber {}

// SAFETY: NSDate is thread-safe, and this is toll-free bridged to that.
#[cfg(feature = "CFDate")]
unsafe impl Send for CFDate {}
#[cfg(feature = "CFDate")]
unsafe impl Sync for CFDate {}

// SAFETY: NSError is thread-safe, and this is toll-free bridged to that.
// Also, Foundation's .swiftinterface adds a `@unchecked Swift.Sendable`
// protocol conformance to CoreFoundation.CFError.
#[cfg(any())]
unsafe impl Send for CFError {}
#[cfg(any())]
unsafe impl Sync for CFError {}

// SAFETY: NSLocale is thread-safe, and this is toll-free bridged to that.
#[cfg(any())]
unsafe impl Send for CFLocale {}
#[cfg(any())]
unsafe impl Sync for CFLocale {}

// SAFETY: NSNull is thread-safe, and this is toll-free bridged to that.
unsafe impl Send for CFNull {}
unsafe impl Sync for CFNull {}

// SAFETY: NSTimeZone is thread-safe, and this is toll-free bridged to that.
#[cfg(feature = "CFDate")]
unsafe impl Send for CFTimeZone {}
#[cfg(feature = "CFDate")]
unsafe impl Sync for CFTimeZone {}

// SAFETY: NSURL is thread-safe, and this is toll-free bridged to that.
#[cfg(any())]
unsafe impl Send for CFURL {}
#[cfg(any())]
unsafe impl Sync for CFURL {}

#[allow(unused_macros)]
#[cfg(test)]
#[path = "../../../../tests/native/objc2-core-foundation/unit/thread_safety.rs"]
mod tests;
