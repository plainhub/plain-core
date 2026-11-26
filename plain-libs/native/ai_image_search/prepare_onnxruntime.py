#!/usr/bin/env python3
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys


SOURCE = Path(__file__).resolve().with_name("onnxruntime")
SOURCE_LOCK = SOURCE.with_suffix(".lock")


def run(args):
    subprocess.run([str(arg) for arg in args], check=True)


def file_hash(path):
    with path.open("rb") as file:
        digest = hashlib.sha256()
        for chunk in iter(lambda: file.read(65536), b""):
            digest.update(chunk)
        return digest.hexdigest()


def source_hash(source):
    digest = hashlib.sha256()
    for path in sorted(source.rglob("*"), key=lambda path: path.relative_to(source).as_posix()):
        if path.is_symlink():
            raise RuntimeError("ONNX Runtime source must not contain symlinks")
        if path.is_file():
            name = path.relative_to(source).as_posix()
            digest.update(f"{name}\0{file_hash(path)}\n".encode())
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
    version = json.loads(SOURCE_LOCK.read_text())["version"]
    cache = Path(os.environ.get("PLAIN_ONNX_CACHE", Path.home() / ".cache/plain-onnxruntime")).resolve() / version
    cache.mkdir(parents=True, exist_ok=True)
    with acquire_lock(cache / ("prepare-" + args.target + ".lock")):
        prepare_runtime(args.target, args.output, cache)


def prepare_runtime(target, output, cache):
    source = json.loads(SOURCE_LOCK.read_text())
    recipe = file_hash(Path(__file__))
    identity = {
        "version": source["version"], "target": target, "source": True,
        "recipe": recipe, "source_lock": file_hash(SOURCE_LOCK),
    }
    ios = "apple-ios" in target
    if ios:
        identity["apple_sdk_headers"] = file_hash(Path(__file__).with_name("apple_sdk_headers.cmake"))
    name = "onnxruntime.dll" if "windows" in target else "libonnxruntime.a" if ios else "libonnxruntime.dylib" if "apple" in target else "libonnxruntime.so"
    output.mkdir(parents=True, exist_ok=True)
    destination = output / name
    receipt = output / "runtime.json"
    notices = ("LICENSE", "ThirdPartyNotices.txt")
    if destination.is_file() and receipt.is_file():
        recorded = json.loads(receipt.read_text())
        if recorded == {"identity": identity, "sha256": file_hash(destination)} and all((output / name).is_file() for name in notices):
            return

    if source_hash(SOURCE) != source["sha256"]:
        raise RuntimeError("ONNX Runtime source does not match onnxruntime.lock")
    build = cache / target / (identity["source_lock"][:12] + "-" + recipe[:12]) / "Release"
    configure = [
        "cmake", "-S", SOURCE / "cmake", "-B", build, "-DCMAKE_BUILD_TYPE=Release",
        "-DCMAKE_POLICY_VERSION_MINIMUM=3.5", "-DCMAKE_DISABLE_FIND_PACKAGE_Git=ON",
        f"-DPython_EXECUTABLE={sys.executable}",
        f"-Donnxruntime_CMAKE_DEPS_MIRROR_DIR={cache / 'mirror'}",
        "-Donnxruntime_BUILD_SHARED_LIB=ON", "-Donnxruntime_BUILD_UNIT_TESTS=OFF",
        "-Donnxruntime_USE_TELEMETRY=OFF", "-Donnxruntime_DISABLE_RTTI=OFF",
        "-Donnxruntime_ENABLE_CPU_FP16_OPS=OFF", "-Donnxruntime_USE_KLEIDIAI=ON", "-Donnxruntime_USE_SVE=ON",
    ]
    if "android" in target:
        abi = {"aarch64-linux-android": "arm64-v8a", "armv7-linux-androideabi": "armeabi-v7a",
               "i686-linux-android": "x86", "x86_64-linux-android": "x86_64"}[target]
        ndk = os.environ.get("ANDROID_NDK_HOME")
        if not ndk:
            raise RuntimeError("ANDROID_NDK_HOME is required to build ONNX Runtime for Android")
        configure += [
            "-DCMAKE_TOOLCHAIN_FILE=" + str(Path(ndk) / "build/cmake/android.toolchain.cmake"),
            "-DANDROID_ABI=" + abi, "-DANDROID_PLATFORM=android-26", "-DANDROID_MIN_SDK=26",
            "-DANDROID_USE_LEGACY_TOOLCHAIN_FILE=false", "-DANDROID_STL=c++_shared",
            "-Donnxruntime_USE_XNNPACK=ON",
            "-Donnxruntime_DISABLE_FLOAT4_TYPES=ON", "-Donnxruntime_DISABLE_FLOAT8_TYPES=ON",
        ]
    elif "apple" in target:
        configure += ["-Donnxruntime_USE_COREML=ON", "-DCMAKE_OSX_ARCHITECTURES=" + ("x86_64" if target.startswith("x86_64") else "arm64")]
        if ios:
            sdk = "iphonesimulator" if target.endswith("-sim") or target.startswith("x86_64") else "iphoneos"
            headers = Path(__file__).with_name("apple_sdk_headers.cmake").resolve()
            configure += [
                "-G", "Xcode", "-DCMAKE_SYSTEM_NAME=iOS", "-DCMAKE_OSX_SYSROOT=" + sdk,
                "-DCMAKE_OSX_DEPLOYMENT_TARGET=15.0", "-DPLATFORM_NAME=" + sdk,
                "-DCMAKE_TOOLCHAIN_FILE=" + str(SOURCE / "cmake/onnxruntime_ios.toolchain.cmake"),
                "-DCMAKE_PROJECT_TOP_LEVEL_INCLUDES=" + str(headers),
                "-Dprotobuf_BUILD_PROTOC_BINARIES=OFF", "-Donnxruntime_BUILD_APPLE_FRAMEWORK=ON",
                "-Donnxruntime_USE_XNNPACK=ON",
            ]
    elif "windows-msvc" in target and os.environ.get("CMAKE_GENERATOR", "Visual Studio").startswith("Visual Studio"):
        configure += ["-A", {"aarch64": "ARM64", "i686": "Win32", "x86_64": "x64"}[target.split("-")[0]]]

    run(configure)
    run(["cmake", "--build", build, "--config", "Release", "--target", "onnxruntime",
         "--parallel", os.environ.get("PLAIN_ONNX_BUILD_JOBS", "4")])
    if ios:
        libraries = list(build.glob("Release-*/static_framework/onnxruntime.framework/onnxruntime"))
    else:
        pattern = "libonnxruntime*.dylib" if "apple" in target else name + "*" if name.endswith(".so") else name
        libraries = [path for path in build.rglob(pattern) if path.is_file() and not path.is_symlink()]
    if len(libraries) != 1:
        raise RuntimeError("ONNX Runtime library missing or ambiguous after source build")
    shutil.copy2(libraries[0], destination)
    for name in notices:
        shutil.copy2(SOURCE / name, output / name)
    receipt.write_text(json.dumps({"identity": identity, "sha256": file_hash(destination)}))


if __name__ == "__main__":
    main()
