import hashlib
import importlib.util
import io
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[3]


def load(name):
    spec = importlib.util.spec_from_file_location(name.replace("-", "_"), ROOT / "scripts" / (name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class NumberingSourceTests(unittest.TestCase):
    def test_local_sources_are_verified_without_network_access(self):
        module = load("generate-phone-numbering")
        raw = b"fixed source fixture"
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "source"
            path.write_bytes(raw)
            with mock.patch.object(module.urllib.request, "urlopen") as request:
                self.assertEqual(module.source(path, "https://example.invalid/source", hashlib.sha256(raw).hexdigest()), raw)
                with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                    module.source(path, "https://example.invalid/source", "0" * 64)
                request.assert_not_called()
                path.unlink()
                with self.assertRaises(FileNotFoundError):
                    module.source(path, "https://example.invalid/source", "0" * 64)
                request.assert_not_called()

    def test_downloaded_sources_require_the_pinned_digest(self):
        module = load("generate-phone-numbering")
        with mock.patch.object(module.urllib.request, "urlopen", return_value=io.BytesIO(b"modified source")):
            with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                module.source(None, "https://example.invalid/source", "0" * 64)

    def test_invalid_metadata_is_rejected_before_xml_parsing_or_output(self):
        module = load("generate-phone-numbering")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "modified.xml"
            path.write_bytes(b"<untrusted/>")
            with mock.patch.object(sys, "argv", ["generate", "--metadata", str(path), "--unicode", str(path), "--check"]), \
                    mock.patch.object(module.ET, "fromstring") as parse:
                with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                    module.main()
                parse.assert_not_called()

    def test_unknown_reference_jars_are_rejected_before_compilation(self):
        module = load("generate-phone-parser-reference")
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "libphonenumber-9.0.41.jar"
            path.write_bytes(b"modified jar")
            with mock.patch.object(sys, "argv", ["generate", "--classpath", str(path), "--java-home", directory, "--check"]), \
                    mock.patch.object(module.subprocess, "run") as compile:
                with self.assertRaisesRegex(ValueError, "pinned Google"):
                    module.main()
                compile.assert_not_called()


if __name__ == "__main__":
    unittest.main()
