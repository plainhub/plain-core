from pathlib import Path
import tomllib
import unittest


ROOT = Path(__file__).resolve().parents[2]


class BuildProfileTests(unittest.TestCase):
    def test_dev_business_packages_skip_optimizer(self):
        profile = tomllib.loads((ROOT / "Cargo.toml").read_text())["profile"]["dev"]
        self.assertTrue(profile["incremental"])
        for name in ["plain-rs", "plain-server", "plainapp"]:
            self.assertEqual(profile["package"][name]["opt-level"], 0, name)

    def test_dev_compute_packages_keep_optimization(self):
        profile = tomllib.loads((ROOT / "Cargo.toml").read_text())["profile"]["dev"]
        self.assertEqual(profile["package"]["plain-libs"]["opt-level"], 1)
        for name in ["*", "plain-graphql", "plain-graphql-macros", "rav1d", "heic-rs", "tiff"]:
            self.assertEqual(profile["package"][name]["opt-level"], 3, name)


if __name__ == "__main__":
    unittest.main()
