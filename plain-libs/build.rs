#[cfg(feature = "ai-image-search")]
use std::{
    env,
    path::{Path, PathBuf},
    process::Command,
};
#[cfg(feature = "ai-image-search")]
fn main() {
    println!("cargo:rerun-if-changed=native/ai_image_search");
    for name in [
        "PLAIN_ONNX_RUNTIME_DIR",
        "PLAIN_ONNX_RUNTIME_SHA256",
        "PLAIN_ONNX_RUNTIME_MANIFEST",
        "ANDROID_NDK_HOME",
        "ANDROID_HOME",
        "PLAIN_ONNX_PYTHON",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
    }
    let target = env::var("TARGET").unwrap();
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let configured_runtime = env::var_os("PLAIN_ONNX_RUNTIME_DIR");
    let runtime = configured_runtime
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| out.join("onnxruntime"));
    let library_name = if target.contains("windows") {
        "onnxruntime.dll"
    } else if target.contains("apple-ios") {
        "libonnxruntime.a"
    } else if target.contains("apple") {
        "libonnxruntime.dylib"
    } else {
        "libonnxruntime.so"
    };
    let library = runtime.join(library_name);
    let manifest = env::var_os("PLAIN_ONNX_RUNTIME_MANIFEST").map(PathBuf::from);
    let explicit_digest = env::var_os("PLAIN_ONNX_RUNTIME_SHA256").is_some();
    let files = [
        "runtime.json",
        "LICENSE",
        "ThirdPartyNotices.txt",
        library_name,
    ];
    if manifest.is_some() || explicit_digest {
        assert!(
            manifest.is_none() || !explicit_digest,
            "Use either the ONNX Runtime manifest or an explicit SHA-256"
        );
        assert!(
            !explicit_digest || configured_runtime.is_some(),
            "PLAIN_ONNX_RUNTIME_DIR is required for a prepared ONNX Runtime"
        );
        let mut verify = Command::new("cmake");
        verify
            .args(["-P", "native/ai_image_search/verify_onnxruntime.cmake"])
            .env("PLAIN_ONNX_RUNTIME_TARGET", &target)
            .env("PLAIN_ONNX_RUNTIME_LIBRARY", &library);
        if let Some(manifest) = manifest {
            assert!(
                manifest.is_absolute() && runtime.is_absolute(),
                "ONNX Runtime manifest and output paths must be absolute"
            );
            assert!(
                manifest.metadata().is_ok_and(|metadata| metadata.is_file()),
                "ONNX Runtime manifest must be a file"
            );
            let source = manifest
                .parent()
                .expect("ONNX Runtime manifest parent")
                .join(&target);
            println!("cargo:rerun-if-changed={}", manifest.display());
            for name in files {
                let path = source.join(name);
                assert!(
                    path.symlink_metadata()
                        .is_ok_and(|metadata| metadata.is_file()),
                    "Prepared ONNX Runtime requires a regular {name} for {target}"
                );
                println!("cargo:rerun-if-changed={}", path.display());
            }
            verify
                .env("PLAIN_ONNX_RUNTIME_LIBRARY", source.join(library_name))
                .env("PLAIN_ONNX_RUNTIME_OUTPUT_DIR", &runtime);
        }
        assert!(
            verify
                .status()
                .expect("CMake is required to verify the prepared ONNX Runtime")
                .success(),
            "Prepared ONNX Runtime verification failed"
        );
    } else {
        prepare_runtime(&target, &runtime);
    }
    for name in files {
        println!("cargo:rerun-if-changed={}", runtime.join(name).display());
    }
    let mut native = cc::Build::new();
    native
        .file("native/ai_image_search/onnx_session.c")
        .include("native/ai_image_search/onnxruntime");
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
            library.display()
        );
        if target.contains("linux") {
            println!("cargo:rustc-link-lib=dl");
        }
    }
    native.compile("plain_onnx_session");
}

#[cfg(feature = "ai-image-search")]
fn prepare_runtime(target: &str, runtime: &Path) {
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
            "native/ai_image_search/prepare_onnxruntime.py",
            "--target",
            target,
            "--output",
        ])
        .arg(runtime);
    assert!(
        prepare
            .status()
            .expect("Python 3 is required to prepare ONNX Runtime")
            .success(),
        "ONNX Runtime preparation failed"
    );
}

#[cfg(not(feature = "ai-image-search"))]
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
}
