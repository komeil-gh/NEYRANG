import tomllib
import unittest
from pathlib import Path


class TrainerBackendTest(unittest.TestCase):
    def test_cuda_is_opt_in_without_changing_default_metal(self):
        path = Path(__file__).resolve().parents[2] / "tools/nnue-trainer/Cargo.toml"
        manifest = tomllib.loads(path.read_text())
        self.assertEqual(manifest.get("features", {}).get("default"), ["metal"])
        self.assertEqual(manifest["features"]["metal"], ["bullet/metal"])
        self.assertEqual(manifest["features"]["cuda"], ["bullet/cuda"])
        self.assertNotIn("features", manifest["dependencies"]["bullet"])


if __name__ == "__main__":
    unittest.main()
