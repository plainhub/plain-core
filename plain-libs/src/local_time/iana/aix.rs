use std::env;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader};

pub(super) fn get_timezone_inner() -> Result<String, crate::local_time::iana::GetTimezoneError> {
    env::var("TZ").map_err(|_| crate::local_time::iana::GetTimezoneError::OsError)
}
