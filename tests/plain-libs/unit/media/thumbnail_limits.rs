use super::*;

#[test]
fn generation_reserves_a_quarter_of_cpu_capacity() {
    assert_eq!(
        [0, 1, 2, 3, 4, 8, 16, 32, 128].map(workers),
        [1, 1, 1, 2, 3, 6, 12, 24, 96]
    );
}

#[test]
fn memory_budget_preserves_capacity_for_other_services() {
    let gib = 1024 * 1024 * 1024;
    assert_eq!(memory_budget_mb(8 * gib), 1024);
    assert_eq!(memory_budget_mb(64 * gib), 8192);
    assert_eq!(memory_budget_mb(256 * gib), 16_384);
    assert_eq!(memory_budget_mb(512 * 1024 * 1024), 64);
    assert_eq!(memory_budget_mb(0), 1);
}
