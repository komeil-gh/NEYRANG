import importlib.util
from pathlib import Path
import sys
import unittest


SCRIPT = Path(__file__).resolve().parents[1] / "compare-nnue-teacher.py"
SPEC = importlib.util.spec_from_file_location("compare_nnue_teacher", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


class ComparisonTests(unittest.TestCase):
    def test_paired_summary_is_deterministic_and_directional(self):
        first = MODULE.paired_summary(
            [0.04, 0.09, 0.16, 0.25],
            [0.01, 0.04, 0.09, 0.16],
            "h5f-test",
            1_000,
        )
        second = MODULE.paired_summary(
            [0.04, 0.09, 0.16, 0.25],
            [0.01, 0.04, 0.09, 0.16],
            "h5f-test",
            1_000,
        )

        self.assertEqual(first, second)
        self.assertTrue(first["passed"])
        self.assertLess(first["candidate_mse"], first["baseline_mse"])
        self.assertLess(first["paired_delta_upper_95"], 0.0)

    def test_paired_summary_rejects_unpaired_input(self):
        with self.assertRaises(MODULE.ComparisonError):
            MODULE.paired_summary([0.1], [], "h5f-test", 100)


if __name__ == "__main__":
    unittest.main()

