use super::*;

#[test]
fn apple_cpu_percent_preserves_idle_failure_and_counter_wrap_semantics() {
    assert_eq!(cpu_usage(None, Some([0; 4])), 0.0);
    assert_eq!(cpu_usage(Some([0; 4]), None), 0.0);
    assert_eq!(cpu_usage(Some([1; 4]), Some([1; 4])), 0.0);
    assert_eq!(cpu_usage(Some([0; 4]), Some([10, 10, 60, 20])), 40.0);
    assert_eq!(cpu_usage(Some([0; 4]), Some([0, 0, 100, 0])), 0.0);
    assert_eq!(
        cpu_usage(Some([u32::MAX - 4, 0, u32::MAX - 4, 0]), Some([5, 0, 5, 0])),
        50.0
    );
}

#[test]
fn apple_system_primitives_return_real_native_values_without_host_requests() {
    assert!(cpu_ticks().is_some());
    assert!(total_memory().unwrap() > 0);
    assert!(!kernel_version().unwrap().is_empty());
}
