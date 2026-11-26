use super::*;

#[test]
fn concurrent_context_handoff_and_reopen_remain_usable() {
    let (sender, receiver) = std::sync::mpsc::sync_channel(0);
    let errors = std::thread::scope(|scope| {
        let releasing = scope.spawn(move || {
            for index in 0..20_000 {
                let context = receiver.recv().unwrap();
                for _ in 0..index % 256 {
                    std::hint::spin_loop();
                }
                drop(context);
            }
        });
        let mut errors = Vec::new();
        for _ in 0..20_000 {
            let context = Context::new();
            if let Err(error) = &context {
                errors.push(error.clone());
            }
            sender.send(context.ok()).unwrap();
        }
        releasing.join().unwrap();
        errors
    });
    assert!(
        errors.is_empty(),
        "Concurrent context failed: {:?}",
        errors.first()
    );
}

#[cfg(target_os = "macos")]
#[test]
fn packaged_macos_runtime_is_loaded_from_frameworks() {
    let root = crate::temporary::tempdir().unwrap();
    let contents = root.path().join("PlainApp.app/Contents");
    let executable = contents.join("MacOS");
    let frameworks = contents.join("Frameworks");
    std::fs::create_dir_all(&executable).unwrap();
    std::fs::create_dir_all(&frameworks).unwrap();
    let library = frameworks.join("libonnxruntime.dylib");
    std::fs::write(&library, b"library").unwrap();
    let located = packaged_library(&executable, "libonnxruntime.dylib").unwrap();
    assert_eq!(
        located.canonicalize().unwrap(),
        library.canonicalize().unwrap()
    );
}
