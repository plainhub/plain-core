#[cfg(feature = "inference")]
use std::{env, path::PathBuf, process::Command};
#[cfg(feature = "inference")]
fn main() {
    println!("cargo:rerun-if-changed=native/inference");
    for name in [
        "PLAIN_ONNX_RUNTIME_DIR",
        "ANDROID_NDK_HOME",
        "ANDROID_HOME",
        "PLAIN_ONNX_PYTHON",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    let target = env::var("TARGET").unwrap();
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let runtime = env::var_os("PLAIN_ONNX_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| out.join("onnxruntime"));
    let candidates = env::var("PLAIN_ONNX_PYTHON")
        .map(|name| vec![name])
        .unwrap_or_else(|_| {
            [
                "python3",
                "python3.14",
                "python3.13",
                "python3.12",
                "python3.11",
                "python3.10",
            ]
            .iter()
            .map(|name| name.to_string())
            .collect()
        });
    let python = candidates
        .into_iter()
        .find(|name| {
            Command::new(name)
                .args([
                    "-c",
                    "import sys; sys.exit(0 if sys.version_info >= (3, 10) else 1)",
                ])
                .status()
                .is_ok_and(|status| status.success())
        })
        .expect("Python 3.10 or newer is required to prepare ONNX Runtime");
    let mut prepare = Command::new(python);
    prepare
        .args([
            "native/inference/prepare_onnxruntime.py",
            "--target",
            &target,
            "--output",
        ])
        .arg(&runtime);
    assert!(
        prepare
            .status()
            .expect("Python 3 is required to prepare ONNX Runtime")
            .success(),
        "ONNX Runtime preparation failed"
    );
    for path in [
        runtime.join("runtime.json"),
        runtime.join("LICENSE"),
        runtime.join("ThirdPartyNotices.txt"),
        runtime.join(if target.contains("windows") {
            "onnxruntime.dll"
        } else if target.contains("apple-ios") {
            "libonnxruntime.a"
        } else if target.contains("apple") {
            "libonnxruntime.dylib"
        } else {
            "libonnxruntime.so"
        }),
    ] {
        println!("cargo:rerun-if-changed={}", path.display());
    }
    let mut native = cc::Build::new();
    native
        .file("native/inference/onnx_session.c")
        .include("native/inference/onnxruntime");
    if target.contains("apple-ios") {
        native.define("PLAIN_ONNX_STATIC", None);
        println!("cargo:rustc-link-search=native={}", runtime.display());
        println!("cargo:rustc-link-lib=static=onnxruntime");
        println!("cargo:rustc-link-lib=c++");
        println!("cargo:rustc-link-lib=framework=Foundation");
        println!("cargo:rustc-link-lib=framework=CoreML");
        println!("cargo:rustc-link-lib=framework=Accelerate");
    } else {
        println!(
            "cargo:rustc-env=PLAIN_ONNX_RUNTIME_LIBRARY={}",
            runtime
                .join(if target.contains("windows") {
                    "onnxruntime.dll"
                } else if target.contains("apple") {
                    "libonnxruntime.dylib"
                } else {
                    "libonnxruntime.so"
                })
                .display()
        );
        if target.contains("linux") {
            println!("cargo:rustc-link-lib=dl");
        }
    }
    native.compile("plain_onnx_session");
}

#[cfg(not(feature = "inference"))]
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
}
