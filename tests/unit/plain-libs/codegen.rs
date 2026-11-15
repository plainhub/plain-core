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
    let libraries = linked_libraries(dependencies, &fingerprints, expected);
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

fn linked_libraries(
    dependencies: &std::path::Path,
    fingerprints: &std::path::Path,
    expected: u64,
) -> Vec<std::path::PathBuf> {
    let expected = expected
        .to_le_bytes()
        .map(|byte| format!("{byte:02x}"))
        .join("");
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
            fingerprint == expected
        })
        .collect();
    libraries
}

#[test]
fn library_selection_ignores_incomplete_unrelated_cargo_fingerprints() {
    let directory = plain_libs::temporary::tempdir().unwrap();
    let dependencies = directory.path().join("deps");
    let fingerprints = directory.path().join(".fingerprint");
    std::fs::create_dir(&dependencies).unwrap();
    for (hash, fingerprint) in [
        ("1111111111111111", ""),
        ("2222222222222222", "0807060504030201"),
        ("3333333333333333", "0000000000000000"),
    ] {
        std::fs::write(dependencies.join(format!("libplain_libs-{hash}.rlib")), []).unwrap();
        let entry = fingerprints.join(format!("plain-libs-{hash}"));
        std::fs::create_dir_all(&entry).unwrap();
        std::fs::write(entry.join("lib-plain_libs"), fingerprint).unwrap();
    }
    assert_eq!(
        linked_libraries(&dependencies, &fingerprints, 0x0102030405060708),
        vec![dependencies.join("libplain_libs-2222222222222222.rlib")]
    );
}
