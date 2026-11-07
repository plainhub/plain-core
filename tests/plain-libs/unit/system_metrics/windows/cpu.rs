use super::*;
use windows::Win32::System::Performance::PDH_FMT_COUNTERVALUE_0;

#[test]
fn cpu_brand_reads_only_the_three_brand_leaves() {
    let mut leaves = Vec::new();
    let brand = cpu_brand(|leaf| {
        leaves.push(leaf);
        match leaf {
            0x80000000 => [u32::MAX, 0, 0, 0],
            0x80000002 => [
                u32::from_le_bytes(*b"CPU "),
                u32::from_le_bytes(*b"Name"),
                0,
                0,
            ],
            0x80000003 | 0x80000004 => [0; 4],
            _ => panic!("Unexpected CPUID leaf"),
        }
    });
    assert_eq!(brand, "CPU Name");
    assert_eq!(leaves, [0x80000000, 0x80000002, 0x80000003, 0x80000004]);
}

#[test]
fn cpu_brand_does_not_query_unsupported_leaves() {
    let mut leaves = Vec::new();
    assert_eq!(
        cpu_brand(|leaf| {
            leaves.push(leaf);
            [0; 4]
        }),
        ""
    );
    assert_eq!(leaves, [0x80000000]);
}

#[test]
fn formatted_counter_requires_successful_data_status() {
    for status in [0, 1, 0xC0000BBA, u32::MAX] {
        let value = PDH_FMT_COUNTERVALUE {
            CStatus: status,
            Anonymous: PDH_FMT_COUNTERVALUE_0 { doubleValue: 73.25 },
        };
        let expected = if status <= 1 { Some(73.25) } else { None };
        assert_eq!(unsafe { counter_value(&value) }, expected);
    }
}
