import os
from pathlib import Path
import subprocess
import textwrap
import unittest


WORKFLOW = Path(__file__).resolve().parents[3] / ".github/workflows/release.yml"


class CliReleaseUploadTest(unittest.TestCase):
    def test_upload_uses_the_configured_product_prefix(self):
        step = WORKFLOW.read_text().split("      - name: Upload versioned assets to R2\n")[1]
        body = step.split("        run: |\n")[1].split("\n      - ")[0]
        script = 'set -eu\naws() { echo "$4"; }\n' + textwrap.dedent(body)
        env = dict(os.environ, AWS_ACCESS_KEY_ID="test", AWS_SECRET_ACCESS_KEY="test",
                   AWS_ENDPOINT_URL_S3="test", R2_BUCKET="fixture", TAG="v4.0.2")
        for prefix in ("cli", "cli/", ""):
            with self.subTest(prefix=prefix):
                result = subprocess.run(["bash", "-c", script], env=dict(env, R2_PREFIX=prefix),
                                        capture_output=True, text=True)
                if prefix:
                    self.assertEqual(result.returncode, 0, result.stderr)
                    self.assertEqual(result.stdout.splitlines(), ["s3://fixture/cli/v4.0.2/"] * 7)
                else:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertEqual(result.stdout, "")
