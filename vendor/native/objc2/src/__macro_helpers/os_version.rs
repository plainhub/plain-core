//! Utilities for checking the runtime availability of APIs.
//!
//! TODO: Upstream some of this to `std`?
use core::cmp::Ordering;
use core::fmt;

#[cfg(target_vendor = "apple")]
mod apple;

/// The size of the fields here are limited by Mach-O's `LC_BUILD_VERSION`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OSVersion {
    // Shuffle the versions around a little so that OSVersion has the same bit
    // representation as the `u32` returned from `to_u32`, allowing
    // comparisons to compile down to just between two `u32`s.
    #[cfg(target_endian = "little")]
    pub patch: u8,
    #[cfg(target_endian = "little")]
    pub minor: u8,
    #[cfg(target_endian = "little")]
    pub major: u16,

    #[cfg(target_endian = "big")]
    pub major: u16,
    #[cfg(target_endian = "big")]
    pub minor: u8,
    #[cfg(target_endian = "big")]
    pub patch: u8,
}

#[track_caller]
const fn parse_usize(mut bytes: &[u8]) -> (usize, &[u8]) {
    // Ensure we have at least one digit (that is not just a period).
    let mut ret: usize = if let Some((&ascii, rest)) = bytes.split_first() {
        bytes = rest;

        match ascii {
            b'0'..=b'9' => (ascii - b'0') as usize,
            _ => panic!("found invalid digit when parsing version"),
        }
    } else {
        panic!("found empty version number part")
    };

    // Parse the remaining digits.
    while let Some((&ascii, rest)) = bytes.split_first() {
        let digit = match ascii {
            b'0'..=b'9' => ascii - b'0',
            _ => break,
        };

        bytes = rest;

        // This handles leading zeroes as well.
        match ret.checked_mul(10) {
            Some(val) => match val.checked_add(digit as _) {
                Some(val) => ret = val,
                None => panic!("version is too large"),
            },
            None => panic!("version is too large"),
        };
    }

    (ret, bytes)
}

impl OSVersion {
    const MIN: Self = Self {
        major: 0,
        minor: 0,
        patch: 0,
    };

    const MAX: Self = Self {
        major: u16::MAX,
        minor: u8::MAX,
        patch: u8::MAX,
    };

    /// Parse the version from a string at `const` time.
    #[track_caller]
    pub const fn from_str(version: &str) -> Self {
        Self::from_bytes(version.as_bytes())
    }

    #[track_caller]
    pub(crate) const fn from_bytes(bytes: &[u8]) -> Self {
        let (major, bytes) = parse_usize(bytes);
        if major > u16::MAX as usize {
            panic!("major version is too large");
        }
        let major = major as u16;

        let bytes = if let Some((period, bytes)) = bytes.split_first() {
            if *period != b'.' {
                panic!("expected period between major and minor version")
            }
            bytes
        } else {
            return Self {
                major,
                minor: 0,
                patch: 0,
            };
        };

        let (minor, bytes) = parse_usize(bytes);
        if minor > u8::MAX as usize {
            panic!("minor version is too large");
        }
        let minor = minor as u8;

        let bytes = if let Some((period, bytes)) = bytes.split_first() {
            if *period != b'.' {
                panic!("expected period after minor version")
            }
            bytes
        } else {
            return Self {
                major,
                minor,
                patch: 0,
            };
        };

        let (patch, bytes) = parse_usize(bytes);
        if patch > u8::MAX as usize {
            panic!("patch version is too large");
        }
        let patch = patch as u8;

        if !bytes.is_empty() {
            panic!("too many parts to version");
        }

        Self {
            major,
            minor,
            patch,
        }
    }

    /// Pack the version into a `u32`.
    ///
    /// This is used for faster comparisons.
    #[inline]
    pub const fn to_u32(self) -> u32 {
        // See comments in `OSVersion`, this should compile down to nothing.
        let (major, minor, patch) = (self.major as u32, self.minor as u32, self.patch as u32);
        (major << 16) | (minor << 8) | patch
    }

    /// Construct the version from a `u32`.
    #[inline]
    pub const fn from_u32(version: u32) -> Self {
        // See comments in `OSVersion`, this should compile down to nothing.
        let major = (version >> 16) as u16;
        let minor = (version >> 8) as u8;
        let patch = version as u8;
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl PartialEq for OSVersion {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.to_u32() == other.to_u32()
    }
}

impl PartialOrd for OSVersion {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.to_u32().partial_cmp(&other.to_u32())
    }
}

impl fmt::Debug for OSVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Same ordering on little and big endian.
        f.debug_struct("OSVersion")
            .field("major", &self.major)
            .field("minor", &self.minor)
            .field("patch", &self.patch)
            .finish()
    }
}

/// The combined availability.
///
/// This generally works closely together with the `available!` macro to make
/// syntax checking inside that easier.
#[derive(Clone, Copy, Debug)]
pub struct AvailableVersion {
    pub macos: OSVersion,
    pub ios: OSVersion,
    pub tvos: OSVersion,
    pub watchos: OSVersion,
    pub visionos: OSVersion,
    #[doc(hidden)]
    pub __others: OSVersion,
}

impl AvailableVersion {
    pub const MIN: Self = Self {
        macos: OSVersion::MIN,
        ios: OSVersion::MIN,
        tvos: OSVersion::MIN,
        watchos: OSVersion::MIN,
        visionos: OSVersion::MIN,
        __others: OSVersion::MIN,
    };

    pub const MAX: Self = Self {
        macos: OSVersion::MAX,
        ios: OSVersion::MAX,
        tvos: OSVersion::MAX,
        watchos: OSVersion::MAX,
        visionos: OSVersion::MAX,
        __others: OSVersion::MAX,
    };
}

#[inline]
pub fn is_available(version: AvailableVersion) -> bool {
    let version = if cfg!(target_os = "macos") {
        version.macos
    } else if cfg!(target_os = "ios") {
        version.ios
    } else if cfg!(target_os = "tvos") {
        version.tvos
    } else if cfg!(target_os = "watchos") {
        version.watchos
    } else if cfg!(target_os = "visionos") {
        version.visionos
    } else {
        version.__others
    };

    // In the special case that `version` was set to `OSVersion::MAX`, we
    // assume that there can never be an OS version that large, and hence we
    // want to avoid checking at all.
    //
    // This is useful for platforms where the version hasn't been specified.
    if version == OSVersion::MAX {
        return false;
    }

    #[cfg(target_vendor = "apple")]
    {
        // If the deployment target is high enough, the API is always available.
        //
        // This check should be optimized away at compile time.
        if version <= apple::DEPLOYMENT_TARGET {
            return true;
        }

        // Otherwise, compare against the version at runtime.
        version <= apple::current_version()
    }

    #[cfg(not(target_vendor = "apple"))]
    return true;
}

#[cfg(test)]
#[path = "../../../../../tests/native/objc2/unit/__macro_helpers/os_version.rs"]
mod tests;
