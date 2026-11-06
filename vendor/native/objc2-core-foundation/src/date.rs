use core::{cmp::Ordering, ptr};

use crate::CFDate;

impl CFDate {
    /// Create a `CFDate` from a [`SystemTime`].
    ///
    /// Nanosecond precision may be lost.
    ///
    /// [`SystemTime`]: std::time::SystemTime
    #[cfg(feature = "std")]
    pub fn from_system_time(time: &std::time::SystemTime) -> crate::CFRetained<Self> {
        let since_1970 = match time.duration_since(std::time::UNIX_EPOCH) {
            Ok(duration) => duration.as_secs_f64(),
            Err(err) => -err.duration().as_secs_f64(),
        } as core::ffi::c_double;

        let since_2001 = since_1970 - unsafe { crate::kCFAbsoluteTimeIntervalSince1970 };
        Self::new(None, since_2001).expect("failed creating CFDate")
    }

    /// Try to construct a [`SystemTime`] from the `CFDate`.
    ///
    /// Nanosecond precision may be lost.
    ///
    /// Returns `None` if the `CFDate` is too large to fit inside
    /// [`SystemTime`].
    ///
    /// [`SystemTime`]: std::time::SystemTime
    #[cfg(feature = "std")]
    #[allow(clippy::unnecessary_cast)]
    pub fn to_system_time(&self) -> Option<std::time::SystemTime> {
        let since_2001 = self.absolute_time();
        let since_1970 = (since_2001 + unsafe { crate::kCFAbsoluteTimeIntervalSince1970 }) as f64;

        std::time::UNIX_EPOCH.checked_add(std::time::Duration::try_from_secs_f64(since_1970).ok()?)
    }
}

impl PartialOrd for CFDate {
    #[inline]
    #[doc(alias = "CFDateCompare")]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for CFDate {
    #[inline]
    #[doc(alias = "CFDateCompare")]
    fn cmp(&self, other: &Self) -> Ordering {
        // Documented that one should pass NULL here.
        let context = ptr::null_mut();
        unsafe { self.compare(Some(other), context) }.into()
    }
}

#[cfg(test)]
#[path = "../../../../tests/native/objc2-core-foundation/unit/date_test.rs"]
mod test;
