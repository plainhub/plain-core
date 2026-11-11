use crate::chrono::{IsoWeek, Month, Weekday};
pub trait Datelike: Sized {
    fn year(&self) -> i32;
    #[inline]
    fn year_ce(&self) -> (bool, u32) {
        let year = self.year();
        if year < 1 {
            (false, (1 - year) as u32)
        } else {
            (true, year as u32)
        }
    }
    #[inline]
    fn quarter(&self) -> u32 {
        (self.month() - 1).div_euclid(3) + 1
    }
    fn month(&self) -> u32;
    fn month0(&self) -> u32;
    fn day(&self) -> u32;
    fn day0(&self) -> u32;
    fn ordinal(&self) -> u32;
    fn ordinal0(&self) -> u32;
    fn weekday(&self) -> Weekday;
    fn iso_week(&self) -> IsoWeek;
    fn with_year(&self, year: i32) -> Option<Self>;
    fn with_month(&self, month: u32) -> Option<Self>;
    fn with_month0(&self, month0: u32) -> Option<Self>;
    fn with_day(&self, day: u32) -> Option<Self>;
    fn with_day0(&self, day0: u32) -> Option<Self>;
    fn with_ordinal(&self, ordinal: u32) -> Option<Self>;
    fn with_ordinal0(&self, ordinal0: u32) -> Option<Self>;
    fn num_days_from_ce(&self) -> i32 {
        let mut year = self.year() - 1;
        let mut ndays = 0;
        if year < 0 {
            let excess = 1 + (-year) / 400;
            year += excess * 400;
            ndays -= excess * 146_097;
        }
        let div_100 = year / 100;
        ndays += ((year * 1461) >> 2) - div_100 + (div_100 >> 2);
        ndays + self.ordinal() as i32
    }
    fn num_days_in_month(&self) -> u8 {
        use num_traits::FromPrimitive;
        let month = Month::from_u32(self.month()).unwrap();
        month.num_days(self.year()).unwrap()
    }
}
pub trait Timelike: Sized {
    fn hour(&self) -> u32;
    #[inline]
    fn hour12(&self) -> (bool, u32) {
        let hour = self.hour();
        let mut hour12 = hour % 12;
        if hour12 == 0 {
            hour12 = 12;
        }
        (hour >= 12, hour12)
    }
    fn minute(&self) -> u32;
    fn second(&self) -> u32;
    fn nanosecond(&self) -> u32;
    fn with_hour(&self, hour: u32) -> Option<Self>;
    fn with_minute(&self, min: u32) -> Option<Self>;
    fn with_second(&self, sec: u32) -> Option<Self>;
    fn with_nanosecond(&self, nano: u32) -> Option<Self>;
    #[inline]
    fn num_seconds_from_midnight(&self) -> u32 {
        self.hour() * 3600 + self.minute() * 60 + self.second()
    }
}
