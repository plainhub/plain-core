import hashlib
import json
import re
import subprocess
import tomllib
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
NATIVE = ROOT / "vendor/native"
PACKAGES = {
    "objc2": "0.6.5",
    "objc2-encode": "4.1.0",
    "block2": "0.6.2",
    "dispatch2": "0.3.1",
    "objc2-foundation": "0.3.2",
    "objc2-av-foundation": "0.3.2",
    "objc2-core-foundation": "0.3.2",
    "objc2-core-media": "0.3.2",
    "objc2-core-video": "0.3.2",
    "windows": "0.57.0",
}


class NativeVideoDependencies(unittest.TestCase):
    def test_native_packages_share_one_local_source_and_keep_upstream_provenance(self):
        patches = tomllib.loads((ROOT / "Cargo.toml").read_text())["patch"]["crates-io"]
        for name, version in PACKAGES.items():
            with self.subTest(package=name):
                package_root = NATIVE / name
                data = tomllib.loads((package_root / "Cargo.toml").read_text())
                self.assertEqual(data["package"]["version"], version)
                self.assertEqual((ROOT / patches[name]["path"]).resolve(), package_root)
                upstream = data["package"]["metadata"]["upstream"]
                self.assertRegex(upstream["revision"], r"^[a-f0-9]{40}$")
                self.assertRegex(upstream["archive-sha256"], r"^[a-f0-9]{64}$")
                for dependency, settings in data.get("dependencies", {}).items():
                    if dependency in PACKAGES:
                        self.assertEqual((package_root / settings["path"]).resolve(), NATIVE / dependency)
                    else:
                        self.assertIn(dependency, {"bitflags", "windows-core", "windows-targets"})
                self.assertTrue((package_root / ("license-mit" if name == "windows" else "LICENSE.md")).is_file())

    def test_platform_adapters_use_local_target_dependencies(self):
        data = tomllib.loads((ROOT / "plain-rs/Cargo.toml").read_text())
        targets = data["target"]
        apple = targets['cfg(any(target_os = "macos", target_os = "ios"))']["dependencies"]
        windows = targets['cfg(target_os = "windows")']["dependencies"]
        for name in PACKAGES.keys() - {"objc2-encode"}:
            dependency = (windows if name == "windows" else apple)[name]
            self.assertEqual((ROOT / "plain-rs" / dependency["path"]).resolve(), NATIVE / name)
            self.assertFalse(dependency["default-features"])
            self.assertTrue(dependency["optional"])

    def test_unused_windows_sdk_namespaces_are_absent(self):
        source = NATIVE / "windows/src"
        self.assertFalse((source / "Windows/Win32/Graphics").exists())
        self.assertFalse((source / "Windows/ApplicationModel").exists())
        self.assertLess(sum(p.stat().st_size for p in source.rglob("*.rs")), 250_000)

    def test_reference_counted_pointer_forwarding_has_one_source(self):
        shared = NATIVE / "objc2/src/rc/retained_forwarding_impls.rs"
        for package in ["dispatch2", "objc2-core-foundation"]:
            source = NATIVE / package / "src/retained.rs"
            match = re.search(r'#\[path = "([^"]+)"\]\s*mod forwarding_impls;', source.read_text())
            self.assertIsNotNone(match)
            self.assertEqual((source.parent / match[1]).resolve(), shared)
            self.assertFalse((source.parent / "retained/forwarding_impls.rs").exists())

    def test_android_and_linux_do_not_build_apple_or_windows_packages(self):
        for target in ["aarch64-linux-android", "x86_64-unknown-linux-gnu"]:
            with self.subTest(target=target):
                result = subprocess.run(
                    ["cargo", "tree", "--offline", "--locked", "--workspace", "--all-features",
                     "--target", target, "--edges", "normal", "--prefix", "none", "--format", "{p}"],
                    cwd=ROOT, capture_output=True, text=True, check=True,
                )
                packages = {line.split()[0] for line in result.stdout.splitlines() if line.strip()}
                self.assertFalse(packages.intersection(PACKAGES))

    def test_retained_com_vtables_match_the_pinned_upstream_slots_and_signatures(self):
        expected = json.loads((Path(__file__).parent / "fixtures/native_video_vtables.json").read_text())
        actual = {}
        for path in (NATIVE / "windows/src").rglob("*.rs"):
            source = path.read_text()
            for match in re.finditer(r"pub struct (\w+_Vtbl)\s*\{([^{}]*)\}", source):
                body = re.sub(r"\s+", "", match[2])
                body = re.sub(r",([)\]])", r"\1", body)
                actual[(path.relative_to(NATIVE / "windows/src").as_posix(), match[1])] = hashlib.sha256(body.encode()).hexdigest()
        self.assertEqual(actual, {(item["path"], item["name"]): item["sha256"] for item in expected})


if __name__ == "__main__":
    unittest.main()
