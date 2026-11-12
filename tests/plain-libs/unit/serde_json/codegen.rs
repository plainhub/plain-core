use std::{fs, path::PathBuf, process::Command};

struct ProbeDir(PathBuf);
impl Drop for ProbeDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn external_calls(ir: &str, function: &str) -> Vec<String> {
    let lines: Vec<_> = ir.lines().collect();
    let mut name = function.to_owned();
    let mut seen = std::collections::HashSet::new();
    let start = loop {
        assert!(seen.insert(name.clone()), "cyclic LLVM function alias");
        if let Some(start) = lines
            .iter()
            .position(|line| line.starts_with("define ") && line.contains(&name))
        {
            break start;
        }
        let alias = lines
            .iter()
            .find(|line| {
                line.starts_with('@')
                    && line.contains(" alias ")
                    && line.split('=').next().unwrap().contains(&name)
            })
            .expect("probe function or its alias must be present in generated LLVM IR");
        name = alias
            .rsplit_once("ptr @")
            .expect("LLVM alias must name a function")
            .1
            .split(|c: char| c.is_whitespace() || c == ',')
            .next()
            .unwrap()
            .to_owned();
    };
    lines[start + 1..]
        .iter()
        .take_while(|line| line.trim() != "}")
        .filter(|line| {
            !line.trim_start().starts_with(';')
                && (line.contains("call ") || line.contains("invoke "))
                && !line.contains("@llvm.")
        })
        .map(|line| line.trim().to_owned())
        .collect()
}

#[test]
#[ignore = "Requires PLAIN_LIBS_CODEGEN_RLIB and PLAIN_LIBS_CODEGEN_DEPS from a production library build"]
fn numeric_constructors_inline_across_the_library_boundary() {
    let library = std::env::var_os("PLAIN_LIBS_CODEGEN_RLIB").expect("production rlib is required");
    let dependencies =
        std::env::var_os("PLAIN_LIBS_CODEGEN_DEPS").expect("dependency directory is required");
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = ProbeDir(std::env::temp_dir().join(format!(
        "plain-json-codegen-{}-{unique}",
        std::process::id()
    )));
    fs::create_dir(&directory.0).unwrap();
    let source = directory.0.join("probe.rs");
    let output = directory.0.join("probe.ll");
    fs::write(
        &source,
        r#"
extern crate plain_libs;
use plain_libs::serde_json::{Number, Value};
#[unsafe(no_mangle)]
pub fn construct_unsigned(value: u64) -> Number { value.into() }
#[unsafe(no_mangle)]
pub fn construct_signed(value: i64) -> Number { value.into() }
#[unsafe(no_mangle)]
pub fn construct_float(value: f64) -> Option<Number> { Number::from_f64(value) }
#[unsafe(no_mangle)]
pub fn duration_seconds(value: &plain_libs::chrono::TimeDelta) -> i64 { value.num_seconds() }
#[unsafe(no_mangle)]
pub fn uuid_bytes(value: &plain_libs::utils::uuid::Uuid) -> &[u8; 16] { value.as_bytes() }
#[unsafe(no_mangle)]
pub fn method_safe(value: &plain_libs::http::Method) -> bool { value.is_safe() }
#[unsafe(no_mangle)]
pub fn parse_array(input: &[u8]) -> Value { plain_libs::serde_json::from_slice(input).unwrap() }
"#,
    )
    .unwrap();
    let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let result = Command::new(compiler)
        .args([
            "--crate-type=lib",
            "--edition=2024",
            "--emit=llvm-ir",
            "-C",
            "opt-level=3",
            "-C",
            "codegen-units=1",
        ])
        .arg(&source)
        .arg("--extern")
        .arg(format!("plain_libs={}", PathBuf::from(library).display()))
        .arg("-L")
        .arg(format!(
            "dependency={}",
            PathBuf::from(dependencies).display()
        ))
        .arg("-o")
        .arg(&output)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let ir = fs::read_to_string(output).unwrap();
    let suffix_calls: Vec<_> = ir
        .lines()
        .filter(|line| {
            !line.trim_start().starts_with(';')
                && (line.contains("call ") || line.contains("invoke "))
                && line.contains("serde_json")
                && (line.contains("parse_number")
                    || line.contains("parse_integer")
                    || line.contains("append_decimal_digit"))
        })
        .collect();
    assert!(
        suffix_calls.is_empty(),
        "numeric suffix dispatch must inline: {suffix_calls:?}"
    );

    for function in [
        "construct_unsigned",
        "construct_signed",
        "construct_float",
        "duration_seconds",
        "uuid_bytes",
        "method_safe",
    ] {
        let calls = external_calls(&ir, function);
        assert!(
            calls.is_empty(),
            "{function} must be inlined, got {calls:?}"
        );
    }
}
