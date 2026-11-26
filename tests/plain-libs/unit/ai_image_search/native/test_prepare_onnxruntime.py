import importlib.util
import json
import os
import shutil
from pathlib import Path
import sys
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).resolve().parents[5] / 'plain-libs/native/ai_image_search/prepare_onnxruntime.py'
spec = importlib.util.spec_from_file_location('prepare_runtime', SOURCE)
prepare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prepare)


class RuntimePreparationTest(unittest.TestCase):
    def test_cold_build_configures_the_project_owned_source_without_cloning(self):
        with tempfile.TemporaryDirectory(prefix='plain-ort-cold-') as temporary:
            root = Path(temporary)
            output = root / 'runtime'
            commands = []

            def run(command, **kwargs):
                command = [str(argument) for argument in command]
                self.assertEqual(command[0], 'cmake')
                commands.append(command)
                if '--build' in command:
                    build = Path(command[command.index('--build') + 1])
                    build.mkdir(parents=True, exist_ok=True)
                    (build / 'libonnxruntime.so').write_bytes(b'fixture-runtime')

            environment = {'PLAIN_ONNX_CACHE': str(root / 'cache')}
            argv = [str(SOURCE), '--target', 'x86_64-unknown-linux-gnu', '--output', str(output)]
            with patch.dict(os.environ, environment), patch.object(sys, 'argv', argv), patch.object(prepare, 'run', side_effect=run):
                prepare.main()

            self.assertEqual(len(commands), 2)
            configure = commands[0]
            self.assertEqual(Path(configure[configure.index('-S') + 1]), SOURCE.with_name('onnxruntime') / 'cmake')
            self.assertEqual((output / 'libonnxruntime.so').read_bytes(), b'fixture-runtime')

    def test_refuses_modified_local_source_before_running_the_build(self):
        with tempfile.TemporaryDirectory(prefix='plain-ort-invalid-source-') as temporary:
            root = Path(temporary)
            source = root / 'onnxruntime'
            source.mkdir()
            (source / 'VERSION_NUMBER').write_text('1.30.0')
            source_lock = source.with_suffix('.lock')
            source_lock.write_text(json.dumps({'version': '1.30.0', 'sha256': prepare.source_hash(source)}))
            (source / 'VERSION_NUMBER').write_text('changed')
            output = root / 'runtime'
            environment = {'PLAIN_ONNX_CACHE': str(root / 'cache')}
            argv = [str(SOURCE), '--target', 'x86_64-unknown-linux-gnu', '--output', str(output)]

            with patch.dict(os.environ, environment), patch.object(sys, 'argv', argv), patch.object(prepare, 'SOURCE', source), \
                    patch.object(prepare, 'SOURCE_LOCK', source_lock), patch.object(prepare, 'run') as build:
                with self.assertRaisesRegex(RuntimeError, 'source does not match'):
                    prepare.main()
                build.assert_not_called()
            self.assertFalse((output / 'libonnxruntime.so').exists())

    @unittest.skipUnless(shutil.which('cmake'), 'CMake is required for the Apple header search-path regression')
    def test_apple_sdk_headers_use_sysroot_without_changing_other_includes_or_libraries(self):
        with tempfile.TemporaryDirectory(prefix='plain-apple-headers-') as temporary:
            root = Path(temporary)
            sdk = root / 'Simulator.sdk/usr/include'
            custom = root / 'custom'
            sdk.mkdir(parents=True)
            custom.mkdir()
            (root / 'CMakeLists.txt').write_text(f"""cmake_minimum_required(VERSION 3.24)
project(HeaderFixture NONE)
set(APPLE TRUE)
add_library(Iconv::Iconv INTERFACE IMPORTED)
set_property(TARGET Iconv::Iconv PROPERTY INTERFACE_INCLUDE_DIRECTORIES "{sdk};{custom}")
set_property(TARGET Iconv::Iconv PROPERTY INTERFACE_LINK_LIBRARIES "fixture-iconv")
function(capture)
  get_target_property(includes Iconv::Iconv INTERFACE_INCLUDE_DIRECTORIES)
  get_target_property(libraries Iconv::Iconv INTERFACE_LINK_LIBRARIES)
  file(WRITE "${{CMAKE_BINARY_DIR}}/result.txt" "${{includes}}\\n${{libraries}}")
endfunction()
cmake_language(DEFER CALL capture)
""")
            for enabled in (False, True):
                build = root / str(enabled)
                args = ['cmake', '-S', str(root), '-B', str(build)]
                if enabled:
                    args.append('-DCMAKE_PROJECT_TOP_LEVEL_INCLUDES=' + str(SOURCE.with_name('apple_sdk_headers.cmake')))
                subprocess.run(args, check=True, capture_output=True, text=True)
                includes, libraries = (build / 'result.txt').read_text().splitlines()
                self.assertEqual(includes, str(custom) if enabled else f'{sdk};{custom}')
                self.assertEqual(libraries, 'fixture-iconv')

    def verify_runtime(self, target, name, expected, forbidden):
        with tempfile.TemporaryDirectory(prefix='plain-ort-test-') as temporary:
            root = Path(temporary)
            output = root / 'runtime'
            commands = []

            def run(command):
                command = [str(arg) for arg in command]
                commands.append(command)
                if '--build' not in command:
                    return
                directory = Path(command[command.index('--build') + 1])
                if 'apple-ios' in target:
                    library = directory / 'Release-iphoneos/static_framework/onnxruntime.framework/onnxruntime'
                else:
                    library = directory / name
                library.parent.mkdir(parents=True, exist_ok=True)
                library.write_bytes(b'fixture-runtime')

            environment = {'PLAIN_ONNX_CACHE': str(root / 'cache'), 'ANDROID_NDK_HOME': str(root / 'ndk')}
            argv = [str(SOURCE), '--target', target, '--output', str(output)]
            with patch.dict(os.environ, environment), patch.object(sys, 'argv', argv), patch.object(prepare, 'run', side_effect=run):
                prepare.main()
                self.assertEqual(len(commands), 2)
                self.assertIn('-Donnxruntime_USE_TELEMETRY=OFF', commands[0])
                self.assertIn('-Donnxruntime_ENABLE_CPU_FP16_OPS=OFF', commands[0])
                self.assertIn('-Donnxruntime_USE_KLEIDIAI=ON', commands[0])
                self.assertIn('-Donnxruntime_USE_SVE=ON', commands[0])
                for flag in expected:
                    self.assertIn(flag, commands[0])
                for flag in forbidden:
                    self.assertNotIn(flag, commands[0])
                self.assertEqual((output / name).read_bytes(), b'fixture-runtime')
                prepare.main()
                self.assertEqual(len(commands), 2)
                (output / name).unlink()
                prepare.main()
                self.assertEqual(len(commands), 4)

    def test_build_without_ai_image_search_does_not_prepare_native_runtime(self):
        with tempfile.TemporaryDirectory(prefix='plain-no-ai-image-search-') as temporary:
            root = Path(temporary)
            (root / 'src').mkdir()
            (root / 'src/main.rs').write_text('fn main() {}')
            (root / 'Cargo.toml').write_text('[package]\nname = "no-ai-image-search-fixture"\nversion = "0.0.0"\nedition = "2024"\n[features]\nai-image-search = []\n')
            shutil.copy2(SOURCE.parents[2] / 'build.rs', root / 'build.rs')
            runtime = root / 'runtime'
            environment = dict(os.environ, PLAIN_ONNX_RUNTIME_DIR=str(runtime), CARGO_TARGET_DIR=str(root / 'target'))
            result = subprocess.run(['cargo', 'build', '--offline', '--quiet'], cwd=root, env=environment, text=True, capture_output=True)
            self.assertEqual(result.returncode, 0, result.stderr[-4000:])
            self.assertFalse(runtime.exists())

    def test_cargo_repairs_removed_generated_runtime_without_rebuilding_unchanged_inputs(self):
        with tempfile.TemporaryDirectory(prefix='plain-ort-cargo-') as temporary:
            root = Path(temporary)
            (root / 'src').mkdir()
            (root / 'native/ai_image_search/onnxruntime').mkdir(parents=True)
            (root / 'src/main.rs').write_text('fn main() {}')
            (root / 'Cargo.toml').write_text('[package]\nname = "runtime-artifact-fixture"\nversion = "0.0.0"\nedition = "2024"\n[features]\ndefault = ["onnx-runtime"]\nonnx-runtime = []\n[build-dependencies]\ncc = "1"\n')
            shutil.copy2(SOURCE.parents[2] / 'build.rs', root / 'build.rs')
            (root / 'native/ai_image_search/onnx_session.c').write_text('int fixture(void) { return 1; }')
            (root / 'native/ai_image_search/prepare_onnxruntime.py').write_text(
                'import argparse\nfrom pathlib import Path\np=argparse.ArgumentParser()\n'
                'p.add_argument("--target")\np.add_argument("--output", type=Path)\na=p.parse_args()\n'
                'a.output.mkdir(parents=True, exist_ok=True)\n'
                'counter=a.output / "calls"\ncounter.write_text(str(int(counter.read_text()) + 1) if counter.exists() else "1")\n'
                'for name in ["runtime.json", "LICENSE", "ThirdPartyNotices.txt", "onnxruntime.dll", "libonnxruntime.dylib", "libonnxruntime.so", "libonnxruntime.a"]:\n'
                ' if not (a.output / name).exists(): (a.output / name).write_text("fixture")\n')
            runtime = root / 'runtime'
            environment = dict(os.environ, PLAIN_ONNX_RUNTIME_DIR=str(runtime), CARGO_TARGET_DIR=str(root / 'target'))
            command = ['cargo', 'build', '--offline', '--quiet', '--manifest-path', str(root / 'Cargo.toml')]
            def build():
                result = subprocess.run(command, cwd=root, env=environment, text=True, capture_output=True)
                self.assertEqual(result.returncode, 0, result.stderr[-4000:])
            build()
            self.assertEqual((runtime / 'calls').read_text(), '1')
            build()
            self.assertEqual((runtime / 'calls').read_text(), '2')
            build()
            self.assertEqual((runtime / 'calls').read_text(), '2')
            name = 'onnxruntime.dll' if os.name == 'nt' else 'libonnxruntime.dylib' if sys.platform == 'darwin' else 'libonnxruntime.so'
            (runtime / name).unlink()
            build()
            self.assertEqual((runtime / 'calls').read_text(), '3')
            self.assertEqual((runtime / name).read_text(), 'fixture')

    def test_native_context_disables_telemetry_and_rejects_disable_failure(self):
        with tempfile.TemporaryDirectory(prefix='plain-ort-context-') as temporary:
            executable = Path(temporary) / ('context-test.exe' if os.name == 'nt' else 'context-test')
            fixture = Path(__file__).with_name('onnx_context.c')
            compiler = shutil.which('cc') or shutil.which('clang') or shutil.which('cl')
            self.assertIsNotNone(compiler, 'Native runtime tests require the configured C compiler')
            if Path(compiler).stem.lower() == 'cl':
                command = [compiler, '/std:c11', '/DPLAIN_ONNX_STATIC', '/I' + str(SOURCE.parent / 'onnxruntime/include/onnxruntime/core/session'), str(fixture), '/Fe' + str(executable)]
            else:
                command = [compiler, '-std=c11', '-DPLAIN_ONNX_STATIC', '-I', str(SOURCE.parent / 'onnxruntime/include/onnxruntime/core/session'), str(fixture), '-o', str(executable)]
            subprocess.run(command, cwd=temporary, check=True, capture_output=True)
            subprocess.run([str(executable)], check=True, capture_output=True)

    @unittest.skipIf(os.name == 'nt', 'POSIX dynamic loader adapter')
    def test_dynamic_library_is_released_after_environment_and_on_initialization_failure(self):
        with tempfile.TemporaryDirectory(prefix='plain-ort-library-') as temporary:
            executable = Path(temporary) / 'library-test'
            compiler = shutil.which('cc') or shutil.which('clang')
            self.assertIsNotNone(compiler)
            subprocess.run([compiler, '-std=c11', '-I', str(SOURCE.parent / 'onnxruntime/include/onnxruntime/core/session'),
                            str(Path(__file__).with_name('onnx_library.c')), '-o', str(executable)],
                           cwd=temporary, check=True, capture_output=True)
            subprocess.run([str(executable)], check=True, capture_output=True)

    def test_desktop_runtime_disables_telemetry_without_adding_providers(self):
        for target, name in [('aarch64-apple-darwin', 'libonnxruntime.dylib'),
                             ('x86_64-unknown-linux-gnu', 'libonnxruntime.so'),
                             ('x86_64-pc-windows-msvc', 'onnxruntime.dll')]:
            with self.subTest(target=target):
                self.verify_runtime(target, name, ['-Donnxruntime_USE_COREML=ON'] if 'apple' in target else [], ['-Donnxruntime_USE_XNNPACK=ON', '-Donnxruntime_USE_WEBGPU=ON'])

    def test_mobile_runtime_keeps_existing_platform_providers(self):
        android = ['-Donnxruntime_USE_XNNPACK=ON', '-Donnxruntime_DISABLE_FLOAT4_TYPES=ON', '-Donnxruntime_DISABLE_FLOAT8_TYPES=ON']
        self.verify_runtime('aarch64-linux-android', 'libonnxruntime.so', [*android, '-DANDROID_ABI=arm64-v8a'], [])
        self.verify_runtime('armv7-linux-androideabi', 'libonnxruntime.so', [*android, '-DANDROID_ABI=armeabi-v7a'], [])
        self.verify_runtime('aarch64-apple-ios', 'libonnxruntime.a', ['-Donnxruntime_USE_COREML=ON', '-Donnxruntime_USE_XNNPACK=ON', '-DCMAKE_SYSTEM_NAME=iOS'], [])


if __name__ == '__main__':
    unittest.main()
