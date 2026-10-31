pub fn now_iso() -> String {
    now_iso_millis()
}

pub fn now_iso_millis() -> String {
    let dur = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    unix_millis_to_iso8601(dur.as_millis() as u64)
}

pub fn now_millis() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

pub fn iso_from_unix_millis(millis: i64) -> String {
    unix_millis_to_iso8601(millis.max(0) as u64)
}

fn is_leap(y: u64) -> bool {
    y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400))
}

fn unix_millis_to_iso8601(millis: u64) -> String {
    let (date, hh, mm, ss) = split_unix_secs(millis / 1000);
    let ms = millis % 1000;
    format!("{date}T{hh:02}:{mm:02}:{ss:02}.{ms:03}Z")
}

fn split_unix_secs(secs: u64) -> (String, u64, u64, u64) {
    let ss = secs % 60;
    let t = secs / 60;
    let mm = t % 60;
    let t = t / 60;
    let hh = t % 24;
    let mut days = t / 24;
    let mut year = 1970u64;
    loop {
        let dy = if is_leap(year) { 366u64 } else { 365u64 };
        if days < dy {
            break;
        }
        days -= dy;
        year += 1;
    }
    let md: [u64; 12] = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1u64;
    for &dm in md.iter() {
        if days < dm {
            break;
        }
        days -= dm;
        month += 1;
    }
    (
        format!("{:04}-{:02}-{:02}", year, month, days + 1),
        hh,
        mm,
        ss,
    )
}

#[cfg(any(feature = "chat", feature = "library"))]
pub fn normalize(value: &str) -> rusqlite::Result<String> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|instant| {
            instant
                .with_timezone(&chrono::Utc)
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
        })
        .map_err(|error| rusqlite::Error::ToSqlConversionFailure(Box::new(error)))
}
