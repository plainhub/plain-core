import json
import os
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "scripts"))
import core_dependency_state as state


class LocalCoreTest(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        root = Path(self.directory.name).resolve()
        environment = patch.dict(os.environ, {"CARGO_HOME": str(root / "cargo-home")})
        environment.start()
        self.addCleanup(environment.stop)
        origin = root / "origin"
        origin.mkdir()
        (origin / "Cargo.toml").write_text(
            '[workspace]\nresolver = "3"\nmembers = ["plain-rs", "plain-server"]\n'
        )
        for name in ("plain-rs", "plain-server"):
            self.package(origin / name, name)
        for args in (
            ["init", "-b", "main"],
            ["add", "."],
            ["-c", "user.name=Fixture", "-c", "user.email=fixture", "commit", "-m", "fixture"],
        ):
            subprocess.run(["git", *args], cwd=origin, check=True, capture_output=True)
        self.core = root / "core"
        shutil.copytree(origin, self.core, ignore=shutil.ignore_patterns(".git"))
        self.package(self.core / "plain-libs", "plain-libs")
        (self.core / "Cargo.toml").write_text(
            '[workspace]\nresolver = "3"\nmembers = ["plain-libs", "plain-rs", "plain-server"]\n'
        )
        self.consumer = root / "consumer"
        self.package(self.consumer, "consumer")
        self.manifest = self.consumer / "Cargo.toml"
        self.url = origin.as_uri()
        self.manifest.write_text(self.manifest.read_text() + "\n[dependencies]\n" + "".join(
            f'{name} = {{ git = "{self.url}", branch = "main" }}\n'
            for name in ("plain-rs", "plain-server")
        ))
        self.override_url = patch.object(state, "URL", self.url)
        self.override_url.start()
        self.addCleanup(self.override_url.stop)
        state.cargo(self.consumer, self.manifest, "metadata", "--format-version", "1")
        self.manifest.write_text(
            self.manifest.read_text() + f'plain-libs = {{ git = "{self.url}", branch = "main" }}\n'
        )
        self.config = self.consumer / ".cargo/config.toml"
        self.config.parent.mkdir()
        old_patch = state.local_patch(self.core, {"plain-rs", "plain-server"})
        self.config.write_text('[net]\noffline = true\n' + old_patch)
        self.record = self.config.parent / "core-dependency-state"
        self.record.mkdir()
        (self.record / "state.json").write_text(json.dumps({
            "core": str(self.core), "patch": old_patch,
        }))

    def package(self, path, name):
        (path / "src").mkdir(parents=True)
        (path / "src/lib.rs").write_text("")
        (path / "Cargo.toml").write_text(
            f'[package]\nname = "{name}"\nversion = "0.1.0"\nedition = "2024"\n'
        )

    def test_refresh_installs_new_package_before_resolving_once(self):
        with patch.object(state, "cargo", wraps=state.cargo) as cargo:
            state.configure(self.consumer, self.manifest, self.core, "local")
        self.assertEqual(cargo.call_count, 1)
        metadata = state.cargo(self.consumer, self.manifest, "metadata", "--format-version", "1")
        self.assertEqual(state.resolved_patches(metadata), {
            (name, self.core / name / "Cargo.toml") for name in state.PACKAGES
        })
        self.assertEqual(self.config.read_text().count("plain-libs ="), 1)
        self.assertEqual(json.loads((self.record / "state.json").read_text())["patch"],
                         state.local_patch(self.core, state.PACKAGES))

    def test_failed_resolution_restores_config_lock_and_state(self):
        paths = [self.config, self.manifest.parent / "Cargo.lock", self.record / "state.json"]
        before = [path.read_bytes() for path in paths]
        with patch.object(state, "cargo", side_effect=ValueError("fixture failure")):
            with self.assertRaisesRegex(ValueError, "fixture failure"):
                state.configure(self.consumer, self.manifest, self.core, "local")
        self.assertEqual([path.read_bytes() for path in paths], before)


if __name__ == "__main__":
    unittest.main()
