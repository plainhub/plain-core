use std::{path::Path, process::Command};

#[test]
fn resolver_poll_keeps_its_codegen_boundary() {
    let temp = plain_libs::temporary::tempdir().unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/resolver_utils/object.rs");
    let fixture =
        include_str!("object_codegen.rs").replace("\"RESOLVER_SOURCE\"", &format!("{source:?}"));
    let input = temp.path().join("probe.rs");
    let output = temp.path().join("probe.ll");
    std::fs::write(&input, fixture).unwrap();
    let result = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
        .args([
            "--edition=2024",
            "--crate-type=lib",
            "--crate-name=probe",
            "--emit=llvm-ir",
            "-O",
            "-Ccodegen-units=1",
        ])
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
    let ir = std::fs::read_to_string(output).unwrap();
    let lines: Vec<_> = ir.lines().collect();
    let poll = lines.windows(3).find(|lines| {
        lines[0].starts_with("; probe::subject::resolve_field_async::<")
            && lines[0].ends_with("::{closure#0}")
            && lines[2].starts_with("define ")
    });
    let poll = poll.expect("the resolver poll body must remain a separate generated function");
    assert!(
        poll[1].split_whitespace().any(|word| word == "noinline"),
        "{}",
        poll[1]
    );
}
