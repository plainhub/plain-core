// Local-time conversion adapted from chrono 0.4.45 (SOURCE.json).
use crate::chrono::{
    DateTime, FixedOffset, MappedLocalTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc,
};

#[cfg(unix)]
mod iana;
#[cfg(any(test, target_os = "android", target_env = "ohos"))]
mod tz_data;
#[cfg(unix)]
mod tz_info;
#[cfg(unix)]
mod unix;
#[cfg(unix)]
use unix as platform;
#[cfg(windows)]
mod win_bindings;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
use std::cmp::Ordering;
#[cfg(windows)]
use windows as platform;

#[derive(Clone, Copy, Debug)]
pub struct Local;

impl Local {
    pub fn now() -> DateTime<Self> {
        Utc::now().with_timezone(&Self)
    }
}

impl TimeZone for Local {
    type Offset = FixedOffset;

    fn from_offset(_: &FixedOffset) -> Self {
        Self
    }

    fn offset_from_local_date(&self, local: &NaiveDate) -> MappedLocalTime<FixedOffset> {
        self.offset_from_local_datetime(&local.and_time(NaiveTime::MIN))
    }

    fn offset_from_local_datetime(&self, local: &NaiveDateTime) -> MappedLocalTime<FixedOffset> {
        platform::offset_from_local_datetime(local)
    }

    fn offset_from_utc_date(&self, utc: &NaiveDate) -> FixedOffset {
        self.offset_from_utc_datetime(&utc.and_time(NaiveTime::MIN))
    }

    fn offset_from_utc_datetime(&self, utc: &NaiveDateTime) -> FixedOffset {
        platform::offset_from_utc_datetime(utc).unwrap()
    }
}

#[cfg(not(any(unix, windows)))]
mod platform {
    use super::*;

    pub(super) fn offset_from_utc_datetime(_: &NaiveDateTime) -> MappedLocalTime<FixedOffset> {
        MappedLocalTime::Single(FixedOffset::east_opt(0).unwrap())
    }

    pub(super) use offset_from_utc_datetime as offset_from_local_datetime;
}

#[cfg(windows)]
fn shift_seconds(date: NaiveDateTime, seconds: i32) -> (i64, u32) {
    let utc = date.and_utc();
    (
        utc.timestamp() + i64::from(seconds),
        utc.timestamp_subsec_nanos(),
    )
}

#[cfg(windows)]
#[derive(Copy, Clone, Eq, PartialEq)]
struct Transition {
    transition_utc: (i64, u32),
    offset_before: FixedOffset,
    offset_after: FixedOffset,
}

#[cfg(windows)]
impl Transition {
    fn new(
        transition_local: NaiveDateTime,
        offset_before: FixedOffset,
        offset_after: FixedOffset,
    ) -> Transition {
        // It is no problem if the transition time in UTC falls a couple of hours inside the buffer
        // space around the `NaiveDateTime` range (although it is very theoretical to have a
        // transition at midnight around `NaiveDate::(MIN|MAX)`.
        let transition_utc = shift_seconds(transition_local, -offset_before.local_minus_utc());
        Transition {
            transition_utc,
            offset_before,
            offset_after,
        }
    }
}

#[cfg(windows)]
impl PartialOrd for Transition {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(windows)]
impl Ord for Transition {
    fn cmp(&self, other: &Self) -> Ordering {
        self.transition_utc.cmp(&other.transition_utc)
    }
}

// Calculate the time in UTC given a local time and transitions.
// `transitions` must be sorted.
#[cfg(windows)]
fn lookup_with_dst_transitions(
    transitions: &[Transition],
    dt: NaiveDateTime,
) -> MappedLocalTime<FixedOffset> {
    let dt = shift_seconds(dt, 0);
    for t in transitions.iter() {
        // A transition can result in the wall clock time going forward (creating a gap) or going
        // backward (creating a fold). We are interested in the earliest and latest wall time of the
        // transition, as this are the times between which `dt` does may not exist or is ambiguous.
        //
        // It is no problem if the transition times falls a couple of hours inside the buffer
        // space around the `NaiveDateTime` range (although it is very theoretical to have a
        // transition at midnight around `NaiveDate::(MIN|MAX)`.
        let (offset_min, offset_max) =
            match t.offset_after.local_minus_utc() > t.offset_before.local_minus_utc() {
                true => (t.offset_before, t.offset_after),
                false => (t.offset_after, t.offset_before),
            };
        let wall_earliest = (
            t.transition_utc.0 + i64::from(offset_min.local_minus_utc()),
            t.transition_utc.1,
        );
        let wall_latest = (
            t.transition_utc.0 + i64::from(offset_max.local_minus_utc()),
            t.transition_utc.1,
        );

        if dt < wall_earliest {
            return MappedLocalTime::Single(t.offset_before);
        } else if dt <= wall_latest {
            return match t
                .offset_after
                .local_minus_utc()
                .cmp(&t.offset_before.local_minus_utc())
            {
                Ordering::Equal => MappedLocalTime::Single(t.offset_before),
                Ordering::Less => MappedLocalTime::Ambiguous(t.offset_before, t.offset_after),
                Ordering::Greater => {
                    if dt == wall_earliest {
                        MappedLocalTime::Single(t.offset_before)
                    } else if dt == wall_latest {
                        MappedLocalTime::Single(t.offset_after)
                    } else {
                        MappedLocalTime::None
                    }
                }
            };
        }
    }
    MappedLocalTime::Single(transitions.last().unwrap().offset_after)
}
