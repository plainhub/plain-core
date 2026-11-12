use std::{fs, path::PathBuf, process::Command};

struct ProbeDir(PathBuf);
impl Drop for ProbeDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub(crate) fn function_body(ir: &str, function: &str) -> Vec<String> {
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
        .map(|line| line.trim().to_owned())
        .collect()
}

pub(crate) fn external_calls(ir: &str, function: &str) -> Vec<String> {
    function_body(ir, function)
        .into_iter()
        .filter(|line| {
            !line.trim_start().starts_with(';')
                && (line.contains("call ") || line.contains("invoke "))
                && !line.contains("@llvm.")
        })
        .map(|line| line.trim().to_owned())
        .collect()
}

pub(crate) fn compile_probe(source_text: &str) -> String {
    let library = std::env::var_os("PLAIN_LIBS_CODEGEN_RLIB").expect("production rlib is required");
    let dependencies =
        std::env::var_os("PLAIN_LIBS_CODEGEN_DEPS").expect("dependency directory is required");
    compile_probe_externs(
        source_text,
        &[("plain_libs", PathBuf::from(library))],
        PathBuf::from(dependencies),
    )
}

pub(crate) fn compile_probe_externs(
    source_text: &str,
    libraries: &[(&str, PathBuf)],
    dependencies: PathBuf,
) -> String {
    static NEXT_PROBE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let sequence = NEXT_PROBE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = ProbeDir(std::env::temp_dir().join(format!(
        "plain-libs-codegen-{}-{unique}-{sequence}",
        std::process::id()
    )));
    fs::create_dir(&directory.0).unwrap();
    let source = directory.0.join("probe.rs");
    let output = directory.0.join("probe.ll");
    fs::write(&source, source_text).unwrap();
    let compiler = std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let mut command = Command::new(compiler);
    command
        .args([
            "--crate-type=lib",
            "--edition=2024",
            "--emit=llvm-ir",
            "-C",
            "opt-level=3",
            "-C",
            "codegen-units=1",
            "-C",
            "debug-assertions=yes",
        ])
        .arg(&source)
        .arg("-L")
        .arg(format!(
            "dependency={}",
            PathBuf::from(dependencies).display()
        ))
        .arg("-o")
        .arg(&output);
    for (name, library) in libraries {
        command
            .arg("--extern")
            .arg(format!("{name}={}", library.display()));
    }
    let result = command.output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    fs::read_to_string(output).unwrap()
}
