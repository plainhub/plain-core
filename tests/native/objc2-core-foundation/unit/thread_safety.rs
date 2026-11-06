
    use super::*;

    macro_rules! not_thread_safe {
        ($($ty:ty),*) => {$(
            static_assertions::assert_not_impl_any!($ty: Send, Sync);
        )?};
    }

    macro_rules! thread_safe {
        ($($ty:ty),*) => {$(
            // General assumption: Assumes that the allocator this was created
            // with is also thread-safe; that's probably a fair assumption,
            // otherwise nothing in CF would be thread-safe.
            static_assertions::assert_impl_all!($ty: Send, Sync);
        )?};
    }

    #[test]
    fn base() {
        // CFType can hold thread-unsafe objects.
        // Just like AnyObject and NSObject aren't thread-safe.
        not_thread_safe!(CFType, CFPropertyList);

        // Only the built-ins are thread-safe, other allocators aren't
        // guaranteed to be. Usually fine though, since they're placed in
        // statics anyhow, and can thus already be accessed from all threads.
        not_thread_safe!(CFAllocator);
    }

    /// Collections, mutable types and immutable types with mutable variants
    /// aren't thread-safe:
    /// <https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/Multithreading/ThreadSafetySummary/ThreadSafetySummary.html#//apple_ref/doc/uid/10000057i-CH12-SW9>
    ///
    /// Note that things that are known to be the immutable variant are
    /// usually thread-safe (which is why e.g. `CFString` in statics is fine).
    #[test]
    fn mutable_or_mutable_variant() {
        #[cfg(any())]
        not_thread_safe!(CFBag<u32>, CFMutableBag<u32>);
        #[cfg(any())]
        not_thread_safe!(CFBinaryHeap);
        #[cfg(any())]
        not_thread_safe!(CFBitVector, CFMutableBitVector);
        #[cfg(any())]
        not_thread_safe!(CFDateFormatter); // Explicitly stated to not be thread-safe
        #[cfg(any())]
        not_thread_safe!(CFFileDescriptor);
        #[cfg(any())]
        not_thread_safe!(CFNumberFormatter); // Explicitly stated to not be thread-safe
        #[cfg(any())]
        not_thread_safe!(CFSocket);
        #[cfg(any())]
        not_thread_safe!(CFStringTokenizer);
        #[cfg(any())]
        not_thread_safe!(CFTree);
        #[cfg(any())]
        not_thread_safe!(CFURLEnumerator);
        #[cfg(any())]
        not_thread_safe!(CFUserNotification);

        // Types are not marked as thread-safe if their toll-free
        // bridged type is not thread safe either.
        #[cfg(feature = "CFArray")]
        not_thread_safe!(CFArray<u32>, CFMutableArray<u32>);
        #[cfg(any())]
        not_thread_safe!(CFAttributedString, CFMutableAttributedString);
        #[cfg(any())]
        not_thread_safe!(CFCalendar);
        #[cfg(any())]
        not_thread_safe!(CFCharacterSet, CFMutableCharacterSet);
        #[cfg(feature = "CFData")]
        not_thread_safe!(CFData, CFMutableData);
        #[cfg(feature = "CFDictionary")]
        not_thread_safe!(CFDictionary<u32, u32>, CFMutableDictionary<u32, u32>);
        #[cfg(any())]
        not_thread_safe!(CFFileSecurity);
        #[cfg(any())]
        not_thread_safe!(CFMachPort);
        #[cfg(any())]
        not_thread_safe!(CFMessagePort);
        #[cfg(feature = "CFRunLoop")]
        not_thread_safe!(CFRunLoopTimer);
        #[cfg(any())]
        not_thread_safe!(CFSet<u32>, CFMutableSet<u32>);
        #[cfg(feature = "CFString")]
        not_thread_safe!(CFString, CFMutableString);
        #[cfg(any())]
        not_thread_safe!(CFReadStream, CFWriteStream);
    }

    /// Immutable CF types are generally thread-safe.
    #[test]
    fn immutable() {
        #[cfg(any())]
        thread_safe!(CFBundle);

        #[cfg(any())]
        thread_safe!(CFNotificationCenter);

        #[cfg(any())]
        thread_safe!(CFUUID);

        // Types whose toll-free bridged type is thread-safe are also marked
        // as thread-safe.

        #[cfg(any())]
        thread_safe!(CFBoolean, CFNumber);
        #[cfg(feature = "CFDate")]
        thread_safe!(CFDate);
        #[cfg(any())]
        thread_safe!(CFError);
        #[cfg(any())]
        thread_safe!(CFLocale);
        thread_safe!(CFNull);
        #[cfg(feature = "CFDate")]
        thread_safe!(CFTimeZone);
        #[cfg(any())]
        thread_safe!(CFURL);
    }

    #[test]
    fn uncertain() {
        // Uncertain, so marked as thread-unsafe for now.
        #[cfg(any())]
        not_thread_safe!(CFPlugIn);
        #[cfg(any())]
        not_thread_safe!(CFPlugInInstance);

        // Under discussion in https://github.com/madsmtm/objc2/issues/696.
        #[cfg(feature = "CFRunLoop")]
        not_thread_safe!(CFRunLoop, CFRunLoopSource, CFRunLoopObserver);
    }

