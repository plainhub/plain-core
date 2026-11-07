use super::{CpuUsage, CpuValues};

#[test]
fn global_usage_uses_two_samples_and_preserves_zero_and_reset_handling() {
    let mut usage = CpuUsage::default();
    usage.set(100, 0, 0, 100, 0, 0, 0, 0, 0, 0);
    assert_eq!(usage.usage(), 50.0);
    usage.set(110, 0, 10, 180, 0, 0, 0, 0, 0, 0);
    assert_eq!(usage.usage(), 20.0);
    usage.set(110, 0, 10, 180, 0, 0, 0, 0, 0, 0);
    assert_eq!(usage.usage(), 0.0);
    usage.set(0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
    assert_eq!(usage.usage(), 0.0);
}

#[test]
fn global_counters_preserve_guest_and_io_wait_accounting_and_saturate() {
    let mut values = CpuValues::default();
    values.set(100, 20, 30, 40, 10, 5, 5, 5, 10, 5);
    assert_eq!(values.work_time(), 145);
    assert_eq!(values.total_time(), 215);
    values.set(
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        u64::MAX,
        0,
        0,
    );
    assert_eq!(values.work_time(), u64::MAX);
    assert_eq!(values.total_time(), u64::MAX);
}
