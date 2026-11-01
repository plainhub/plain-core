#!/usr/bin/env python3
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

VERSION = "1.30.0"
SOURCE_COMMIT = "f2c39fe2f838cf35ce7da92824f5a5e3ee6e88a7"



def run(args, **kwargs):
    subprocess.run([str(a) for a in args], check=True, **kwargs)




def file_hash(path):
    digest = hashlib.sha256()
    with path.open("rb") as data:
        for chunk in iter(lambda: data.read(65536), b""):
            digest.update(chunk)
    return digest.hexdigest()


def acquire_lock(path):
    lock = path.open("a+b")
    if os.name == "nt":
        import msvcrt
        if lock.tell() == 0:
            lock.write(b"0")
            lock.flush()
        lock.seek(0)
        msvcrt.locking(lock.fileno(), msvcrt.LK_LOCK, 1)
    else:
        import fcntl
        fcntl.flock(lock.fileno(), fcntl.LOCK_EX)
    return lock


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--target", required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    target = args.target
    cache = Path(os.environ.get("PLAIN_ONNX_CACHE", Path.home() / ".cache/plain-onnxruntime")) / VERSION
    cache.mkdir(parents=True, exist_ok=True)
    with acquire_lock(cache / ("prepare-" + target + ".lock")):
        prepare_runtime(target, args.output, cache)


def prepare_runtime(target, output, cache):
    output.mkdir(parents=True, exist_ok=True)
    name = "onnxruntime.dll" if "windows" in target else "libonnxruntime.a" if "apple-ios" in target else "libonnxruntime.dylib" if "apple" in target else "libonnxruntime.so"
    destination = output / name
    stamp = output / "runtime.json"
    identity = {"version": VERSION, "target": target, "source": True, "recipe": file_hash(Path(__file__))}
    if destination.is_file() and stamp.is_file():
        recorded = json.loads(stamp.read_text())
        if recorded.get("identity") == identity and recorded.get("sha256") == file_hash(destination) and all((output / name).is_file() for name in ("LICENSE", "ThirdPartyNotices.txt")):
            return
    checkout = cache / "source"
    checkout_lock = acquire_lock(cache / "checkout.lock")
    try:
        if not checkout.exists():
            run(["git", "clone", "--depth", "1", "--branch", "v" + VERSION,
                 "--recurse-submodules", "--shallow-submodules", "https://github.com/microsoft/onnxruntime.git", checkout])
    finally:
        checkout_lock.close()
    commit = subprocess.check_output(["git", "-C", str(checkout), "rev-parse", "HEAD"], text=True).strip()
    if commit != SOURCE_COMMIT:
        raise RuntimeError("ONNX Runtime source revision mismatch")
    for legal_name in ("LICENSE", "ThirdPartyNotices.txt"):
        shutil.copy2(checkout / legal_name, output / legal_name)
    build = cache / target
    cmd = [sys.executable, checkout / "tools/ci_build/build.py", "--build_dir", build,
           "--config", "Release", "--update", "--build", "--skip_tests",
           "--parallel", os.environ.get("PLAIN_ONNX_BUILD_JOBS", "4"),
           "--cmake_extra_defines", "onnxruntime_BUILD_UNIT_TESTS=OFF", "onnxruntime_USE_TELEMETRY=OFF", "CMAKE_POLICY_VERSION_MINIMUM=3.5"]
    if "android" in target:
        abi = {"aarch64-linux-android": "arm64-v8a", "armv7-linux-androideabi": "armeabi-v7a",
               "i686-linux-android": "x86", "x86_64-linux-android": "x86_64"}[target]
        sdk = os.environ.get("ANDROID_HOME") or os.environ.get("ANDROID_SDK_ROOT")
        ndk = os.environ.get("ANDROID_NDK_HOME")
        if not sdk or not ndk:
            raise RuntimeError("ANDROID_HOME and ANDROID_NDK_HOME are required for the source build")
        cmd += ["--android", "--android_sdk_path", sdk, "--android_ndk_path", ndk,
                "--android_abi", abi, "--android_api", "26", "--build_shared_lib", "--android_cpp_shared", "--use_xnnpack"]
    elif "apple-ios" in target:
        sdk = "iphonesimulator" if target.endswith("-sim") or target.startswith("x86_64") else "iphoneos"
        cmd += ["--ios", "--cmake_generator", "Xcode", "--apple_sysroot", sdk, "--osx_arch", "x86_64" if target.startswith("x86_64") else "arm64",
                "--apple_deploy_target", "15.0", "--build_apple_framework", "--use_coreml", "--use_xnnpack"]
    else:
        cmd += ["--build_shared_lib"]
        if "apple" in target:
            cmd += ["--use_coreml", "--osx_arch", "x86_64" if target.startswith("x86_64") else "arm64"]
    cmd += ["--skip_submodule_sync"]
    run(cmd)
    if "apple-ios" in target:
        archives = sorted((build / "Release").rglob("static_framework/onnxruntime.framework/onnxruntime"))
        if len(archives) != 1:
            raise RuntimeError("ONNX Runtime static framework missing or ambiguous")
        shutil.copy2(archives[0], destination)
    else:
        pattern = "libonnxruntime*.dylib" if "apple" in target else name + "*" if name.endswith(".so") else name
        libraries = sorted(p for p in (build / "Release").rglob(pattern) if p.is_file() and not p.is_symlink())
        if len(libraries) != 1:
            raise RuntimeError("ONNX Runtime library missing or ambiguous after source build")
        shutil.copy2(libraries[0], destination)
    digest = file_hash(destination)
    stamp.write_text(json.dumps({"identity": identity, "sha256": digest}))


if __name__ == "__main__":
    main()
