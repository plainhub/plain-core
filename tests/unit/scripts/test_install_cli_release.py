import base64
import hashlib
import io
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tempfile
import unittest


INSTALLER = Path(__file__).resolve().parents[3] / "install.sh"


@unittest.skipUnless(shutil.which("openssl") and shutil.which("sha256sum"), "installer tools required")
class InstallCliReleaseTest(unittest.TestCase):
    def run_installer(self, machine, damage=None, use_fixture_key=True):
        with tempfile.TemporaryDirectory() as path:
            root = Path(path)
            checksums = []
            for arch in ("amd64", "arm64"):
                archive = root / f"plainapp-linux-{arch}.tar.gz"
                with tarfile.open(archive, "w:gz") as package:
                    payload = arch.encode()
                    entry = tarfile.TarInfo("plainapp")
                    entry.size = len(payload)
                    package.addfile(entry, io.BytesIO(payload))
                checksums.append(f"{hashlib.sha256(archive.read_bytes()).hexdigest()}  {archive.name}")
            checksums.append(f"{hashlib.sha256(INSTALLER.read_bytes()).hexdigest()}  install.sh")
            manifest = root / "release-manifest.txt"
            version = "4.9.8" if damage == "version" else "4.9.9"
            manifest.write_text(f"version {version}\nschema {'0' * 64}\n" + "\n".join(checksums) + "\n")
            key = root / "key.pem"
            signature = root / "release-manifest.sig"
            subprocess.run(["openssl", "genpkey", "-algorithm", "ED25519", "-out", str(key)], check=True)
            public = subprocess.check_output(["openssl", "pkey", "-in", str(key), "-pubout", "-outform", "DER"])
            subprocess.run(["openssl", "pkeyutl", "-sign", "-rawin", "-inkey", str(key),
                            "-in", str(manifest), "-out", str(signature)], check=True)
            if damage == "signature":
                signature.write_bytes(bytes(64))
            if damage == "archive":
                (root / "plainapp-linux-amd64.tar.gz").write_bytes(b"tampered")
            for name, body in {
                "uname": 'printf "%s\\n" "$TEST_MACHINE"\n',
                "hostname": 'printf "127.0.0.1\\n"\n',
                "curl": """
url=${!#}
case "$url" in
    https://api.github.com/repos/plainhub/plain-core/releases/latest) printf '{"tag_name":"v4.9.9"}\\n' ;;
    https://github.com/plainhub/plain-core/releases/download/v4.9.9/*) cat "$TEST_ROOT/${url##*/}" ;;
    *) exit 99 ;;
esac
""",
                "sudo": """
case "$1" in
    install) cat "$4" > "$TEST_ROOT/installed" ;;
    /usr/local/bin/plainapp) [[ "$2" = install ]] ;;
    systemctl) [[ "$2 $3 $4" = 'enable --now plainapp' ]] ;;
    *) exit 99 ;;
esac
printf '%s\\n' "$1" >> "$TEST_ROOT/sudo-calls"
""",
            }.items():
                command = root / name
                command.write_text("#!/bin/bash\nset -eu\n" + body)
                command.chmod(0o700)
            env = dict(os.environ, PATH=f"{root}:{os.environ['PATH']}",
                       PLAINAPP_RELEASE_PUBLIC_KEY=base64.b64encode(public).decode(),
                       TEST_MACHINE=machine, TEST_ROOT=str(root))
            if not use_fixture_key:
                env.pop("PLAINAPP_RELEASE_PUBLIC_KEY")
            result = subprocess.run(["bash", str(INSTALLER)], env=env, capture_output=True, text=True)
            calls = root / "sudo-calls"
            installed = root / "installed"
            return result, calls.read_text().splitlines() if calls.exists() else [], installed.read_text() if installed.exists() else None

    def test_accepts_signed_package_for_each_architecture(self):
        for machine, arch in (("x86_64", "amd64"), ("aarch64", "arm64")):
            with self.subTest(machine=machine):
                result, calls, installed = self.run_installer(machine)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(calls, ["install", "/usr/local/bin/plainapp", "systemctl"])
                self.assertEqual(installed, arch)

    def test_requires_valid_signature_tag_and_archive_before_installing(self):
        for damage, message in (("signature", "signature verification failed"),
                                ("version", "Invalid release manifest"),
                                ("archive", "checksum mismatch")):
            with self.subTest(damage=damage):
                result, calls, installed = self.run_installer("x86_64", damage)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(message, result.stderr)
                self.assertEqual(calls, [])
                self.assertIsNone(installed)

    def test_default_trust_rejects_an_unknown_signing_key(self):
        result, calls, _ = self.run_installer("x86_64", use_fixture_key=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("signature verification failed", result.stderr)
        self.assertEqual(calls, [])
