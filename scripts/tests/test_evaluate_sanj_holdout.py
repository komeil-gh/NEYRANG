from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "evaluate-sanj-holdout.py"


def load_evaluator():
    spec = importlib.util.spec_from_file_location("evaluate_sanj_holdout", SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class SanjHoldoutEvaluatorTests(unittest.TestCase):
    def test_decision_requires_every_gate(self) -> None:
        evaluator = load_evaluator()

        accepted = evaluator.build_decision({"identity": True, "loss": True})
        rejected = evaluator.build_decision({"identity": True, "loss": False})

        self.assertTrue(accepted["playing_tests_authorized"])
        self.assertFalse(rejected["playing_tests_authorized"])
        self.assertFalse(accepted["candidate_retained"])
        self.assertFalse(accepted["strength_claim"])

    def test_only_committed_registration_can_authorize_holdout(self) -> None:
        evaluator = load_evaluator()
        with tempfile.TemporaryDirectory() as directory:
            other = Path(directory) / evaluator.REGISTRATION_PATH.name
            with self.assertRaisesRegex(ValueError, "only the committed H3c"):
                evaluator.validate_registration_path(other)

    def test_only_absent_canonical_result_can_be_written(self) -> None:
        evaluator = load_evaluator()
        with tempfile.TemporaryDirectory() as directory:
            other = Path(directory) / evaluator.RESULT_PATH.name
            with self.assertRaisesRegex(ValueError, "canonical one-time result"):
                evaluator.validate_output_path(other)


if __name__ == "__main__":
    unittest.main()
