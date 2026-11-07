use super::*;

#[test]
fn test_div_round() {
    #[derive(Debug)]
    struct TestEntry {
        lhs: u32,
        rhs: u32,
        result: u32,
    }

    #[rustfmt::skip]
		let tests = [
			TestEntry { lhs: 1, rhs: 1, result: 1 },
			TestEntry { lhs: 1, rhs: 2, result: 1 },
			TestEntry { lhs: 2, rhs: 2, result: 1 },
			TestEntry { lhs: 3, rhs: 2, result: 2 },
			TestEntry { lhs: 4, rhs: 2, result: 2 },
			TestEntry { lhs: 5, rhs: 2, result: 3 },

			// Should be rounded up to 1
			TestEntry { lhs: 800, rhs: 1500, result: 1 },
			TestEntry { lhs: 1500, rhs: 3000, result: 1 },

			// Shouldn't be rounded
			TestEntry { lhs: 0, rhs: 4000, result: 0 },
			TestEntry { lhs: 1500, rhs: 4000, result: 0 },
		];

    for test in &tests {
        let result = test.lhs.div_round(test.rhs);
        assert_eq!(result, test.result, "{}.div_round({})", test.lhs, test.rhs);
    }
}

#[test]
fn test_f80() {
    fn cmp_float_nearly_equal(a: f64, b: f64) -> bool {
        if a.is_infinite() && b.is_infinite() {
            return true;
        }

        if a.is_nan() && b.is_nan() {
            return true;
        }

        (a - b).abs() < f64::EPSILON
    }

    #[derive(Debug)]
    struct TestEntry {
        input: [u8; 10],
        output_f64: f64,
    }

    let tests = [
        TestEntry {
            input: [0; 10],
            output_f64: 0.0,
        },
        TestEntry {
            input: [0x7F, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0],
            output_f64: f64::INFINITY,
        },
        TestEntry {
            input: [0xFF, 0xFF, 0, 0, 0, 0, 0, 0, 0, 0],
            output_f64: f64::NEG_INFINITY,
        },
        TestEntry {
            input: [0x7F, 0xFF, 0x80, 0, 0, 0, 0, 0, 0, 0],
            output_f64: f64::NAN,
        },
        TestEntry {
            input: [0xFF, 0xFF, 0x80, 0, 0, 0, 0, 0, 0, 0],
            output_f64: -f64::NAN,
        },
        TestEntry {
            input: [0x3F, 0xFC, 0x80, 0, 0, 0, 0, 0, 0, 0],
            output_f64: 0.125,
        },
        TestEntry {
            input: [0x3F, 0xFF, 0x80, 0, 0, 0, 0, 0, 0, 0],
            output_f64: 1.0,
        },
        TestEntry {
            input: [0x40, 0x00, 0x80, 0, 0, 0, 0, 0, 0, 0],
            output_f64: 2.0,
        },
        TestEntry {
            input: [0x40, 0x00, 0xC0, 0, 0, 0, 0, 0, 0, 0],
            output_f64: 3.0,
        },
        TestEntry {
            input: [0x40, 0x0E, 0xBB, 0x80, 0, 0, 0, 0, 0, 0],
            output_f64: 48000.0,
        },
    ];

    for test in &tests {
        let f80 = F80::from_be_bytes(test.input);
        let f64 = f80.as_f64();
        assert!(
            cmp_float_nearly_equal(f64, test.output_f64),
            "F80::as_f64({f80:?}) == {f64} (expected {})",
            test.output_f64
        );
    }
}
