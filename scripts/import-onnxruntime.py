#!/usr/bin/env python3
import argparse
import json
from pathlib import Path, PurePosixPath
import re
import runpy
import subprocess
import tarfile


PROVIDERS = {"cpu", "coreml", "xnnpack", "shared", "shared_library", "pch"}
C_API_DEPENDENCIES = {
    "cann/cann_provider_options.h", "cuda/cuda_provider_options.h", "dnnl/dnnl_provider_options.h",
    "nv_tensorrt_rtx/nv_provider_options.h", "openvino/openvino_provider_factory.h",
    "tensorrt/tensorrt_provider_options.h", "utils/ort_graph_to_proto.h",
}
PROVIDER_INTERFACES = {
    "cann/cann_provider_factory.h", "cann/cann_provider_factory_creator.h",
    "cuda/cuda_provider_factory.h", "cuda/cuda_provider_factory_creator.h",
    "dnnl/dnnl_provider_factory.h", "dnnl/dnnl_provider_factory_creator.h",
    "migraphx/migraphx_provider_factory.h", "migraphx/migraphx_provider_factory_creator.h",
    "nv_tensorrt_rtx/nv_provider_factory.h", "nv_tensorrt_rtx/nv_provider_factory_creator.h",
    "openvino/openvino_provider_factory_creator.h", "qnn/qnn_provider_factory_creator.h",
    "tensorrt/tensorrt_provider_factory.h", "tensorrt/tensorrt_provider_factory_creator.h",
    "vitisai/vitisai_provider_factory_creator.h", "webgpu/webgpu_provider_options.h",
}
CMAKE_EXTERNAL = {
    "abseil-cpp.cmake", "abseil-cpp.natvis", "eigen.cmake", "helper_functions.cmake",
    "onnx_protobuf.natvis", "onnxruntime_external_deps.cmake", "protobuf_function.cmake",
    "wil.cmake", "xnnpack.cmake",
}
CMAKE_PATCHES = {"coremltools", "cpuinfo", "date", "eigen", "flatbuffers", "gsl", "onnx", "protobuf", "xnnpack"}
CONTACT_EMAIL = re.compile(rb"[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}")


def retained(path):
    parts = path.parts
    if path.as_posix() in {
        "LICENSE", "ThirdPartyNotices.txt", "VERSION_NUMBER", "tools/ci_build/gen_def.py",
        "tools/ci_build/github/apple/framework_info.json.template",
    }:
        return True
    if parts[0] == "cmake":
        if parts[1] == "external":
            return len(parts) == 3 and parts[2] in CMAKE_EXTERNAL
        if parts[1] == "patches":
            return parts[2] in CMAKE_PATCHES
        if parts[1:3] == ("vcpkg-ports", "abseil"):
            return path.suffix == ".patch"
        if len(parts) != 2:
            return False
        if path.name.startswith("onnxruntime_providers_"):
            return path.stem.removeprefix("onnxruntime_providers_") in PROVIDERS
        return path.name in {
            "CMakeLists.txt", "EnableVisualStudioCodeAnalysis.props", "Info.plist.in", "Sdl.ruleset",
            "adjust_global_compile_flags.cmake", "arm64x.cmake", "deps.txt", "empty.c",
            "detect_onnxruntime_target_platform.cmake", "libonnxruntime.pc.cmake.in",
            "onnxruntime.cmake", "onnxruntime_common.cmake", "onnxruntime_config.h.in",
            "onnxruntime_flatbuffers.cmake", "onnxruntime_framework.cmake", "onnxruntime_framework.natvis",
            "onnxruntime_graph.cmake", "onnxruntime_ios.toolchain.cmake",
            "onnxruntime_language_standard_versions.cmake", "onnxruntime_lora.cmake",
            "onnxruntime_mlas.cmake", "onnxruntime_optimizer.cmake", "onnxruntime_providers.cmake",
            "onnxruntime_session.cmake", "onnxruntime_telemetry_tenant_token.h.in", "onnxruntime_util.cmake",
            "onnxruntime_xctest.cmake", "resolve_onnxruntime_libraries.cmake", "onnxruntime_unity_exclusions.cmake",
        }
    if parts[:2] == ("include", "onnxruntime"):
        return (parts[:4] != ("include", "onnxruntime", "core", "providers") or len(parts) == 5
                or parts[4] in PROVIDERS or "/".join(parts[4:]) in C_API_DEPENDENCIES)
    if parts[:2] == ("onnxruntime", "core"):
        return (parts[2] != "providers" or len(parts) == 4 or parts[3] in PROVIDERS
                or "/".join(parts[3:]) in PROVIDER_INTERFACES)
    if parts[0] == "model_package":
        return parts[1] in {"CMakeLists.txt", "include", "src"}
    return (parts[:3] == ("onnxruntime", "contrib_ops", "cpu") or parts[:2] == ("onnxruntime", "lora")
            or path.as_posix() in {"onnxruntime/contrib_ops/cuda/bert/gated_delta_net_plan.h",
                                   "onnxruntime/contrib_ops/cuda/moe/qmoe_row_tiling.h"})


def import_source(checkout, revision, output):
    git = ["git", "-C", checkout]
    commit = subprocess.check_output([*git, "rev-parse", "--verify", revision + "^{commit}"], text=True).strip()
    if commit != revision:
        raise ValueError("Use the full upstream commit ID")
    build = Path(__file__).resolve().parents[1] / "plain-libs/native/ai_image_search/prepare_onnxruntime.py"
    source_hash = runpy.run_path(str(build))["source_hash"]
    output.mkdir(parents=True)
    with subprocess.Popen([*git, "archive", commit], stdout=subprocess.PIPE) as archive:
        with tarfile.open(fileobj=archive.stdout, mode="r|") as packaged:
            for item in packaged:
                path = PurePosixPath(item.name)
                if not item.isfile() or not retained(path):
                    continue
                destination = output / path
                destination.parent.mkdir(parents=True, exist_ok=True)
                with packaged.extractfile(item) as source, destination.open("xb") as file:
                    file.write(CONTACT_EMAIL.sub(b"[contact omitted]", source.read()))
        if archive.wait():
            raise RuntimeError("Failed to export the pinned ONNX Runtime source")
    source = {
        "commit": commit,
        "version": (output / "VERSION_NUMBER").read_text().strip(),
        "sha256": source_hash(output),
    }
    output.with_suffix(".lock").write_text(json.dumps(source, indent=2) + "\n", newline="\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("checkout", type=Path)
    parser.add_argument("revision")
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    import_source(args.checkout, args.revision, args.output)
