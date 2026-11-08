use super::*;
use chrono::{DateTime, MappedLocalTime};

#[test]
fn transition_search_skips_only_transitions_strictly_before_wall_time() {
    let standard = LocalTimeType::new(0, false, None).unwrap();
    let daylight = LocalTimeType::new(3600, true, None).unwrap();
    let zone = TimeZone::new(
        vec![Transition::new(10_000, 1), Transition::new(20_000, 0)],
        vec![standard, daylight],
        vec![],
        None,
    )
    .unwrap();
    assert_eq!(zone.as_ref().first_candidate(13_600, zone.max_offset), 0);
    assert_eq!(zone.as_ref().first_candidate(13_601, zone.max_offset), 1);
    assert_eq!(zone.as_ref().first_candidate(23_600, zone.max_offset), 1);
    assert_eq!(zone.as_ref().first_candidate(23_601, zone.max_offset), 2);
    let wall = |time| DateTime::from_timestamp(time, 0).unwrap().naive_utc();
    assert_eq!(
        zone.find_local_time_type_from_local(wall(11_000)).unwrap(),
        MappedLocalTime::None
    );
    assert_eq!(
        zone.find_local_time_type_from_local(wall(21_000)).unwrap(),
        MappedLocalTime::Ambiguous(standard, daylight)
    );
}
