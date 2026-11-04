import importlib.util
import os
import shutil
import shlex
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
    @unittest.skipIf(os.name == 'nt', 'POSIX Git fsmonitor fixture')
    def test_source_validation_does_not_execute_checkout_git_hooks(self):
        with tempfile.TemporaryDirectory(prefix='plain-ort-git-') as temporary:
            root = Path(temporary)
            checkout = root / 'source'
            checkout.mkdir()
            git = ['git', '-c', 'core.fsmonitor=false', '-c', 'core.hooksPath=' + os.devnull, '-C', str(checkout)]
            subprocess.run([*git, 'init', '--quiet'], check=True, capture_output=True)
            for name in ['LICENSE', 'ThirdPartyNotices.txt']:
                (checkout / name).write_text('fixture')
            subprocess.run([*git, 'add', '.'], check=True, capture_output=True)
            tree = subprocess.check_output([*git, 'write-tree'], text=True).strip()
            commit = f'tree {tree}\nauthor Fixture <fixture> 0 +0000\ncommitter Fixture <fixture> 0 +0000\n\nFixture\n'
            revision = subprocess.check_output([*git, 'hash-object', '-w', '-t', 'commit', '--stdin'], input=commit, text=True).strip()
            subprocess.run([*git, 'update-ref', 'HEAD', revision], check=True, capture_output=True)
            marker = root / 'unverified-hook-executed'
            hook = root / 'monitor'
            hook.write_text('#!/bin/sh\nprintf 1 > ' + shlex.quote(str(marker)) + "\nprintf 'fixture-token\\0'\n")
            hook.chmod(0o700)
            subprocess.run([*git, 'config', 'core.fsmonitor', str(hook)], check=True, capture_output=True)
            with patch.object(prepare, 'SOURCE_COMMIT', revision), \
                    patch.object(prepare, 'run', side_effect=RuntimeError('Verified build reached')) as build:
                with self.assertRaisesRegex(RuntimeError, 'Verified build reached'):
                    prepare.prepare_runtime('x86_64-unknown-linux-gnu', root / 'output', root)
                build.assert_called_once()
            self.assertFalse(marker.exists(), 'Source validation executed an unverified repository hook')

    def test_refuses_modified_sources_and_unpinned_submodules_before_running_the_build(self):
        for dirty, submodules in [(' M tools/ci_build/build.py\n', ''), ('?? tools/injected.py\n', ''),
                                  ('', '+abc external/component\n'), ('', '-abc external/component\n')]:
            with self.subTest(dirty=dirty, submodules=submodules), tempfile.TemporaryDirectory(prefix='plain-ort-source-') as temporary:
                root = Path(temporary)
                (root / 'source').mkdir()
                for legal in ['LICENSE', 'ThirdPartyNotices.txt']:
                    (root / 'source' / legal).write_text('fixture')
                output = root / 'output'
                output.mkdir()
                library = output / 'libonnxruntime.so'
                library.write_bytes(b'preserved-runtime')

                def git_output(command, **kwargs):
                    if 'rev-parse' in command:
                        return prepare.SOURCE_COMMIT + '\n'
                    return submodules if 'submodule' in command else dirty

                with patch.object(prepare.subprocess, 'check_output', side_effect=git_output), \
                        patch.object(prepare, 'run', side_effect=AssertionError('Unverified source build executed')) as build:
                    with self.assertRaisesRegex(RuntimeError, 'modified|submodule'):
                        prepare.prepare_runtime('x86_64-unknown-linux-gnu', output, root)
                    build.assert_not_called()
                self.assertEqual(library.read_bytes(), b'preserved-runtime')

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
            checkout = root / 'cache' / prepare.VERSION / 'source'
            checkout.mkdir(parents=True)
            for legal in ['LICENSE', 'ThirdPartyNotices.txt']:
                (checkout / legal).write_text('fixture')
            output = root / 'runtime'
            commands = []

            def run(command, **kwargs):
                command = [str(arg) for arg in command]
                commands.append(command)
                directory = Path(command[command.index('--build_dir') + 1]) / 'Release'
                if 'apple-ios' in target:
                    library = directory / 'static_framework/onnxruntime.framework/onnxruntime'
                else:
                    library = directory / (f"libonnxruntime.{prepare.VERSION}.dylib" if "apple" in target else name)
                library.parent.mkdir(parents=True, exist_ok=True)
                library.write_bytes(b'fixture-runtime')

            environment = {'PLAIN_ONNX_CACHE': str(root / 'cache'), 'ANDROID_HOME': str(root / 'sdk'),
                           'ANDROID_NDK_HOME': str(root / 'ndk')}
            with patch.dict(os.environ, environment), patch.object(sys, 'argv', [str(SOURCE), '--target', target, '--output', str(output)]), \
                    patch.object(prepare, 'run', side_effect=run), \
                    patch.object(prepare.subprocess, 'check_output', side_effect=lambda command, **kwargs: prepare.SOURCE_COMMIT + '\n' if 'rev-parse' in command else ''), \
                    patch('urllib.request.urlopen', side_effect=AssertionError('Unreviewed prebuilt runtime can enable telemetry')):
                prepare.main()
                self.assertEqual(len(commands), 1)
                self.assertIn('onnxruntime_USE_TELEMETRY=OFF', commands[0])
                for flag in expected:
                    self.assertIn(flag, commands[0])
                for flag in forbidden:
                    self.assertNotIn(flag, commands[0])
                self.assertEqual((output / name).read_bytes(), b'fixture-runtime')
                prepare.main()
                self.assertEqual(len(commands), 1)
                (output / name).unlink()
                prepare.main()
                self.assertEqual(len(commands), 2)

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
            (root / 'Cargo.toml').write_text('[package]\nname = "runtime-artifact-fixture"\nversion = "0.0.0"\nedition = "2024"\n[features]\ndefault = ["ai-image-search"]\nai-image-search = []\n[build-dependencies]\ncc = "1"\n')
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
                command = [compiler, '/std:c11', '/DPLAIN_ONNX_STATIC', '/I' + str(SOURCE.parent / 'onnxruntime'), str(fixture), '/Fe' + str(executable)]
            else:
                command = [compiler, '-std=c11', '-DPLAIN_ONNX_STATIC', '-I', str(SOURCE.parent / 'onnxruntime'), str(fixture), '-o', str(executable)]
            subprocess.run(command, cwd=temporary, check=True, capture_output=True)
            subprocess.run([str(executable)], check=True, capture_output=True)

    @unittest.skipIf(os.name == 'nt', 'POSIX dynamic loader adapter')
    def test_dynamic_library_is_released_after_environment_and_on_initialization_failure(self):
        with tempfile.TemporaryDirectory(prefix='plain-ort-library-') as temporary:
            executable = Path(temporary) / 'library-test'
            compiler = shutil.which('cc') or shutil.which('clang')
            self.assertIsNotNone(compiler)
            subprocess.run([compiler, '-std=c11', '-I', str(SOURCE.parent / 'onnxruntime'),
                            str(Path(__file__).with_name('onnx_library.c')), '-o', str(executable)],
                           cwd=temporary, check=True, capture_output=True)
            subprocess.run([str(executable)], check=True, capture_output=True)

    def test_desktop_runtime_disables_telemetry_without_adding_providers(self):
        for target, name in [('aarch64-apple-darwin', 'libonnxruntime.dylib'),
                             ('x86_64-unknown-linux-gnu', 'libonnxruntime.so'),
                             ('x86_64-pc-windows-msvc', 'onnxruntime.dll')]:
            with self.subTest(target=target):
                self.verify_runtime(target, name, ['--use_coreml'] if 'apple' in target else [], ['--use_xnnpack', '--use_webgpu'])

    def test_mobile_runtime_keeps_existing_platform_providers(self):
        self.verify_runtime('aarch64-linux-android', 'libonnxruntime.so', ['--use_xnnpack', '--android'], [])
        self.verify_runtime('aarch64-apple-ios', 'libonnxruntime.a', ['--use_coreml', '--use_xnnpack', '--ios'], [])


if __name__ == '__main__':
    unittest.main()
