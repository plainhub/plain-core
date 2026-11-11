use super::internals::YearFlags;
use core::fmt;
#[derive(PartialEq, Eq, PartialOrd, Ord, Copy, Clone, Hash)]
pub struct IsoWeek {
    ywf: i32,
}
impl IsoWeek {
    pub(super) fn from_yof(year: i32, ordinal: u32, year_flags: YearFlags) -> Self {
        let rawweek = (ordinal + year_flags.isoweek_delta()) / 7;
        let (year, week) = if rawweek < 1 {
            let prevlastweek = YearFlags::from_year(year - 1).nisoweeks();
            (year - 1, prevlastweek)
        } else {
            let lastweek = year_flags.nisoweeks();
            if rawweek > lastweek {
                (year + 1, 1)
            } else {
                (year, rawweek)
            }
        };
        let flags = YearFlags::from_year(year);
        IsoWeek {
            ywf: (year << 10) | (week << 4) as i32 | i32::from(flags.0),
        }
    }
    #[inline]
    pub const fn year(&self) -> i32 {
        self.ywf >> 10
    }
    #[inline]
    pub const fn week(&self) -> u32 {
        ((self.ywf >> 4) & 0x3f) as u32
    }
}
impl fmt::Debug for IsoWeek {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let year = self.year();
        let week = self.week();
        if (0..=9999).contains(&year) {
            write!(f, "{year:04}-W{week:02}")
        } else {
            write!(f, "{year:+05}-W{week:02}")
        }
    }
}
