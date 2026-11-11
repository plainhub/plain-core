use super::{FixedOffset, MappedLocalTime, Offset, TimeZone};
use crate::chrono::naive::{NaiveDate, NaiveDateTime};
use crate::chrono::{Date, DateTime};
use core::fmt;
#[cfg(all(
    all(),
    not(all(
        target_arch = "wasm32",
        all(),
        not(any(target_os = "emscripten", target_os = "wasi", target_os = "linux"))
    ))
))]
use std::time::{SystemTime, UNIX_EPOCH};
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(all(any(), all()), derive(arbitrary::Arbitrary))]
pub struct Utc;
impl Utc {
    #[must_use]
    pub fn today() -> Date<Utc> {
        Utc::now().date()
    }
    #[cfg(not(all(
        target_arch = "wasm32",
        all(),
        not(any(target_os = "emscripten", target_os = "wasi", target_os = "linux"))
    )))]
    #[must_use]
    pub fn now() -> DateTime<Utc> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before Unix epoch");
        DateTime::from_timestamp(now.as_secs() as i64, now.subsec_nanos()).unwrap()
    }
    #[cfg(all(
        target_arch = "wasm32",
        all(),
        not(any(target_os = "emscripten", target_os = "wasi", target_os = "linux"))
    ))]
    #[must_use]
    pub fn now() -> DateTime<Utc> {
        let now = js_sys::Date::new_0();
        DateTime::<Utc>::from(now)
    }
}
impl TimeZone for Utc {
    type Offset = Utc;
    fn from_offset(_state: &Utc) -> Utc {
        Utc
    }
    fn offset_from_local_date(&self, _local: &NaiveDate) -> MappedLocalTime<Utc> {
        MappedLocalTime::Single(Utc)
    }
    fn offset_from_local_datetime(&self, _local: &NaiveDateTime) -> MappedLocalTime<Utc> {
        MappedLocalTime::Single(Utc)
    }
    fn offset_from_utc_date(&self, _utc: &NaiveDate) -> Utc {
        Utc
    }
    fn offset_from_utc_datetime(&self, _utc: &NaiveDateTime) -> Utc {
        Utc
    }
}
impl Offset for Utc {
    fn fix(&self) -> FixedOffset {
        FixedOffset::east_opt(0).unwrap()
    }
}
impl fmt::Debug for Utc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Z")
    }
}
impl fmt::Display for Utc {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "UTC")
    }
}
