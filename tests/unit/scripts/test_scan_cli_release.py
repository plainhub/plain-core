import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


SCRIPT = Path(__file__).resolve().parents[3] / "scripts/scan-cli-release.sh"


@unittest.skipUnless(shutil.which("jq") and shutil.which("sha256sum"), "release runner tools required")
class ScanCliReleaseTest(unittest.TestCase):
    def run_scan(self, mode, key="test-key", large=False):
        with tempfile.TemporaryDirectory() as base:
            root = Path(base)
            for arch in ("amd64", "arm64"):
                (root / f"plainapp-linux-{arch}.tar.gz").write_bytes(arch.encode())
            if large:
                with (root / "plainapp-linux-amd64.tar.gz").open("r+b") as archive:
                    archive.truncate(32 * 1024 * 1024 + 1)
            curl = root / "curl"
            curl.write_text("""#!/bin/bash
set -e
url=${!#}
case "$url" in
    https://www.virustotal.com/api/v3/files/upload_url)
        printf '{"data":"http://www.virustotal.com/upload/once"}'
        ;;
    https://www.virustotal.com/api/v3/files|https://www.virustotal.com/upload/once)
        printf '{"data":{"id":"stub/id"}}'
        ;;
    https://www.virustotal.com/api/v3/analyses/stub%2Fid)
        case "$VT_TEST_MODE" in
            completed) printf '{"data":{"attributes":{"status":"completed","stats":{"malicious":1,"suspicious":1,"harmless":30,"undetected":20}}}}' ;;
            pending) printf '{"data":{"attributes":{"status":"queued"}}}' ;;
            failed) exit 22 ;;
        esac
        ;;
    *) echo "Unexpected URL $url" >&2; exit 1 ;;
esac
""")
            curl.chmod(0o700)
            sleep = root / "sleep"
            sleep.write_text("#!/bin/bash\nexit 0\n")
            sleep.chmod(0o700)
            env = dict(os.environ, PATH=f"{root}:{os.environ['PATH']}", VT_TEST_MODE=mode, VT_API_KEY=key)
            return subprocess.run(["bash", str(SCRIPT), str(root)], env=env, capture_output=True, text=True, timeout=15)

    def test_scans_both_archives_and_counts_flags(self):
        result = self.run_scan("completed", large=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        for arch in ("amd64", "arm64"):
            self.assertIn(f"| `plainapp-linux-{arch}.tar.gz` | 2/52 | [Report]", result.stdout)

    def test_incomplete_or_failed_scan_cannot_generate_release_report(self):
        for mode in ("pending", "failed"):
            with self.subTest(mode=mode):
                result = self.run_scan(mode)
                self.assertNotEqual(result.returncode, 0)
                self.assertNotIn("/detection", result.stdout)

    def test_secret_is_required(self):
        result = self.run_scan("completed", key="")
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("VIRUSTOTAL_API_KEY", result.stderr)
