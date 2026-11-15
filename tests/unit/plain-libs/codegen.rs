pub fn compile_consumer(fixture: &str, sources: &[&str]) -> String {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let input = directory.path().join("consumer.rs");
    let output = directory.path().join("consumer.ll");
    let owner = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::write(&input, fixture).unwrap();
    let executable = std::env::current_exe().unwrap();
    let dependencies = executable.parent().unwrap();
    let (target, test_hash) = executable
        .file_stem()
        .unwrap()
        .to_str()
        .unwrap()
        .rsplit_once('-')
        .unwrap();
    let fingerprints = dependencies.parent().unwrap().join(".fingerprint");
    let test: plain_libs::serde_json::Value = plain_libs::serde_json::from_slice(
        &std::fs::read(fingerprints.join(format!(
            "plain-libs-{test_hash}/test-integration-test-{target}.json"
        )))
        .unwrap(),
    )
    .unwrap();
    let expected = test["deps"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row[1] == "plain_libs")
        .unwrap()[3]
        .as_u64()
        .unwrap();
    // Cargo fingerprints identify the exact library linked into this test.
    let libraries: Vec<_> = std::fs::read_dir(dependencies)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            let name = path.file_name().unwrap().to_string_lossy();
            let Some(hash) = name
                .strip_prefix("libplain_libs-")
                .and_then(|name| name.strip_suffix(".rlib"))
            else {
                return false;
            };
            let fingerprint = std::fs::read_to_string(
                fingerprints.join(format!("plain-libs-{hash}/lib-plain_libs")),
            )
            .unwrap();
            let bytes: Vec<_> = fingerprint
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
                .collect();
            u64::from_le_bytes(bytes.try_into().unwrap()) == expected
        })
        .collect();
    assert_eq!(libraries.len(), 1, "the exact Cargo library is required");
    let library = &libraries[0];
    let changed = sources
        .iter()
        .copied()
        .map(|name| {
            std::fs::metadata(owner.join(name))
                .unwrap()
                .modified()
                .unwrap()
        })
        .max()
        .unwrap();
    assert!(
        std::fs::metadata(library).unwrap().modified().unwrap() >= changed,
        "library source changed after compilation"
    );
    let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let result = std::process::Command::new(&compiler)
        .args([
            "--edition=2024",
            "--crate-type=lib",
            "--crate-name=consumer",
            "--emit=llvm-ir",
            "-O",
            "-Ccodegen-units=1",
            "--extern",
        ])
        .arg(format!("plain_libs={}", library.display()))
        .arg("-L")
        .arg(format!("dependency={}", dependencies.display()))
        .arg(&input)
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    std::fs::read_to_string(output).unwrap()
}
