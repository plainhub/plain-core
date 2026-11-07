use super::*;

#[test]
fn option_snapshots_survive_subsequent_updates() {
    let original = global_options();
    apply_global_options(GlobalOptions::new().allocation_limit(64));
    let snapshot = global_options();
    apply_global_options(GlobalOptions::new().allocation_limit(128));
    assert_eq!(snapshot.allocation_limit, 64);
    assert_eq!(global_options().allocation_limit, 128);
    apply_global_options(original);
}

#[test]
fn updates_are_local_to_the_current_thread() {
    let original = global_options();
    apply_global_options(GlobalOptions::new().allocation_limit(64));
    let child = std::thread::spawn(|| {
        let default = global_options();
        apply_global_options(GlobalOptions::new().allocation_limit(128));
        (default.allocation_limit, global_options().allocation_limit)
    })
    .join()
    .unwrap();
    assert_eq!(child, (GlobalOptions::DEFAULT_ALLOCATION_LIMIT, 128));
    assert_eq!(global_options().allocation_limit, 64);
    apply_global_options(original);
}
