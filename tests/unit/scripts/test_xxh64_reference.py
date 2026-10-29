import importlib.util
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock


class Xxh64ReferenceTests(unittest.TestCase):
    def test_modified_reference_header_is_rejected_before_compilation(self):
        source = Path(__file__).resolve().parents[3] / "scripts/generate-xxh64-reference.py"
        spec = importlib.util.spec_from_file_location("xxh64_reference", source)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        with tempfile.TemporaryDirectory() as directory:
            header = Path(directory) / "xxhash.h"
            header.write_bytes(b"modified reference implementation")
            with mock.patch.object(sys, "argv", ["generate", "--header", str(header), "--check"]), mock.patch.object(module.subprocess, "run") as compile:
                with self.assertRaisesRegex(ValueError, "checksum mismatch"):
                    module.main()
                compile.assert_not_called()


if __name__ == "__main__":
    unittest.main()
