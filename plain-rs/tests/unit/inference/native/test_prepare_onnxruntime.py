import importlib.util
import os
import shutil
from pathlib import Path
import sys
import subprocess
import tempfile
import unittest
from unittest.mock import patch

SOURCE = Path(__file__).resolve().parents[4] / 'native/inference/prepare_onnxruntime.py'
spec = importlib.util.spec_from_file_location('prepare_runtime', SOURCE)
prepare = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prepare)


class RuntimePreparationTest(unittest.TestCase):
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
                    patch.object(prepare.subprocess, 'check_output', return_value=prepare.SOURCE_COMMIT + '\n'), \
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

    def test_build_without_inference_does_not_prepare_native_runtime(self):
        with tempfile.TemporaryDirectory(prefix='plain-no-inference-') as temporary:
            root = Path(temporary)
            (root / 'src').mkdir()
            (root / 'src/main.rs').write_text('fn main() {}')
            (root / 'Cargo.toml').write_text('[package]\nname = "no-inference-fixture"\nversion = "0.0.0"\nedition = "2024"\n[features]\ninference = []\n')
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
            (root / 'native/inference/onnxruntime').mkdir(parents=True)
            (root / 'src/main.rs').write_text('fn main() {}')
            (root / 'Cargo.toml').write_text('[package]\nname = "runtime-artifact-fixture"\nversion = "0.0.0"\nedition = "2024"\n[features]\ndefault = ["inference"]\ninference = []\n[build-dependencies]\ncc = "1"\n')
            shutil.copy2(SOURCE.parents[2] / 'build.rs', root / 'build.rs')
            (root / 'native/inference/onnx_session.c').write_text('int fixture(void) { return 1; }')
            (root / 'native/inference/prepare_onnxruntime.py').write_text(
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
