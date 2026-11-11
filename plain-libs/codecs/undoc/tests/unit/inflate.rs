use super::*;

fn read_all(data: impl Read, compressed: u64) -> io::Result<usize> {
    InflateGuard::new(data, compressed).read_to_end(&mut Vec::new())
}

#[test]
fn small_output_is_never_refused() {
    let grace = InflateLimits::POI.grace_size;
    assert_eq!(
        read_all(io::repeat(0).take(grace), 1).unwrap(),
        grace as usize
    );
}

#[test]
fn other_limits_move_both_bounds() {
    let limits = InflateLimits {
        grace_size: 1 << 20,
        max_expansion: 600,
        max_size: 8 << 20,
    };
    let read = |len: u64, compressed: u64| {
        InflateGuard::with_limits(io::repeat(0).take(len), compressed, limits)
            .read_to_end(&mut Vec::new())
    };
    // 500-fold past the grace: within 600.
    assert!(read(5 << 20, (5 << 20) / 500).is_ok());
    // 1000-fold: refused.
    assert!(is_inflation_limit(
        &read(5 << 20, (5 << 20) / 1000).unwrap_err()
    ));
    // Under the grace size any ratio passes; past the maximum nothing does.
    assert!(read(1 << 20, 1).is_ok());
    assert!(is_inflation_limit(&read(9 << 20, 9 << 20).unwrap_err()));
}

#[test]
fn check_applies_the_rule_without_a_reader() {
    let poi = InflateLimits::default();
    assert!(poi.check(1, poi.grace_size).is_ok());
    let err = poi.check(1, poi.grace_size + 1).unwrap_err();
    assert!(is_inflation_limit(&err));
    assert!(poi.allows(u64::MAX, poi.max_size));
    assert!(!poi.allows(u64::MAX, poi.max_size + 1));
}

#[test]
fn output_past_a_hundredfold_is_a_bomb() {
    let err = read_all(io::repeat(0).take(10 << 20), 10 * 1024).unwrap_err();
    assert!(is_inflation_limit(&err), "{err}");
    assert!(err.to_string().contains("decompression bomb"), "{err}");
}

#[test]
fn ordinary_compression_passes() {
    // 20-fold: markup compresses about this well.
    assert_eq!(
        read_all(io::repeat(b'x').take(4 << 20), (4 << 20) / 20).unwrap(),
        4 << 20
    );
}

#[test]
fn an_io_error_underneath_is_not_the_limit() {
    struct Failing;
    impl Read for Failing {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("disk"))
        }
    }
    let err = read_all(Failing, 10).unwrap_err();
    assert!(!is_inflation_limit(&err));
}

#[test]
fn inflation_limits_example_keeps_expected_output() {
    use crate::inflate::{InflateGuard, is_inflation_limit};
    use std::io::Read;

    let bomb = std::io::repeat(0).take(1 << 20);
    let err = InflateGuard::new(bomb, 1024)
        .read_to_end(&mut Vec::new())
        .unwrap_err();
    assert!(is_inflation_limit(&err));

    let text = std::io::repeat(b'a').take(1 << 20);
    assert_eq!(
        InflateGuard::new(text, 64 * 1024)
            .read_to_end(&mut Vec::new())
            .unwrap(),
        1 << 20
    );
}
