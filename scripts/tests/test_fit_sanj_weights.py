from __future__ import annotations

import importlib.util
import sys
import unittest
from pathlib import Path

import numpy as np


SCRIPT = Path(__file__).resolve().parents[1] / "fit-sanj-weights.py"


def load_fitter():
    if not SCRIPT.is_file():
        raise AssertionError(f"fitter script does not exist: {SCRIPT}")
    spec = importlib.util.spec_from_file_location("fit_sanj_weights", SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class SanjWeightFitterTests(unittest.TestCase):
    def test_rounding_is_half_away_from_zero(self) -> None:
        fitter = load_fitter()

        rounded = fitter.round_half_away_from_zero(
            np.array([-2.5, -1.5, -0.49, 0.49, 1.5, 2.5])
        )

        self.assertEqual(rounded.tolist(), [-3, -2, 0, 0, 2, 3])

    def test_inner_split_is_deterministic_and_keeps_complete_groups(self) -> None:
        fitter = load_fitter()
        groups = np.array(
            ["pair-c", "pair-a", "pair-a", "pair-b", "pair-d", "pair-d"],
            dtype=object,
        )

        first = fitter.inner_group_split(groups, "registered-seed", 0.25)
        second = fitter.inner_group_split(groups, "registered-seed", 0.25)

        self.assertTrue(np.array_equal(first[0], second[0]))
        self.assertTrue(np.array_equal(first[1], second[1]))
        self.assertFalse(np.any(first[0] & first[1]))
        self.assertTrue(np.all(first[0] | first[1]))
        self.assertEqual(len(np.unique(groups[first[1]])), 1)
        for group in np.unique(groups):
            self.assertTrue(
                first[0][groups == group].all()
                or first[1][groups == group].all()
            )

    def test_regularized_cross_entropy_gradient_matches_finite_difference(self) -> None:
        fitter = load_fitter()
        design = np.array(
            [[1.0, 0.0], [0.5, 1.0], [-1.0, 0.25]], dtype=np.float64
        )
        target = np.array([1.0, 0.5, 0.0])
        row_weights = np.array([0.2, 0.3, 0.5])
        baseline = np.array([2.0, -1.0])
        radii = np.array([3.0, 2.0])
        candidate = np.array([2.4, -0.7])
        fixed_output = np.array([0.25, -0.5, 0.75])

        value, gradient = fitter.objective_and_gradient(
            candidate,
            design,
            target,
            row_weights,
            baseline,
            radii,
            scale=0.8,
            regularization=0.01,
            fixed_output=fixed_output,
        )
        epsilon = 1e-5
        numeric = np.empty_like(candidate)
        for index in range(candidate.size):
            left = candidate.copy()
            right = candidate.copy()
            left[index] -= epsilon
            right[index] += epsilon
            left_value = fitter.objective_and_gradient(
                left,
                design,
                target,
                row_weights,
                baseline,
                radii,
                scale=0.8,
                regularization=0.01,
                fixed_output=fixed_output,
            )[0]
            right_value = fitter.objective_and_gradient(
                right,
                design,
                target,
                row_weights,
                baseline,
                radii,
                scale=0.8,
                regularization=0.01,
                fixed_output=fixed_output,
            )[0]
            numeric[index] = (right_value - left_value) / (2.0 * epsilon)

        self.assertTrue(np.isfinite(value))
        self.assertTrue(np.allclose(gradient, numeric, atol=1e-8, rtol=1e-6))

    def test_path_guard_rejects_any_holdout_name(self) -> None:
        fitter = load_fitter()

        with self.assertRaisesRegex(ValueError, "holdout"):
            fitter.validate_feature_path(
                Path("testing/holdout/train.features.tsv"),
                expected_name="train.features.tsv",
            )

    def test_baseline_literal_mapping_reconstructs_current_source_values(self) -> None:
        fitter = load_fitter()
        analysis = fitter.load_analysis_module()

        literals = fitter.effective_to_source_literals(
            np.asarray(analysis.CURRENT_EFFECTIVE_WEIGHTS, dtype=np.int64)
        )

        self.assertEqual(literals["mg_material"], [69, 300, 312, 405, 1094, 0])
        self.assertEqual(literals["eg_material"], [79, 280, 330, 589, 1077, 0])
        self.assertEqual(literals["tempo"], 6)
        self.assertEqual(literals["mg_psqt"], [3, 0, 14, 6, 4, 3, 4, -6, -5])
        self.assertEqual(literals["eg_psqt"], [6, 0, 9, 4, 4, 1, 4])

    def test_material_order_gate_rejects_unsafe_vector(self) -> None:
        fitter = load_fitter()
        analysis = fitter.load_analysis_module()
        unsafe = np.asarray(analysis.CURRENT_EFFECTIVE_WEIGHTS, dtype=np.int64)
        unsafe = unsafe.copy()
        unsafe[3] = unsafe[4] + 1

        self.assertFalse(fitter.material_order_is_safe(unsafe))

    def test_decision_metadata_uses_python_booleans(self) -> None:
        fitter = load_fitter()

        rejected = fitter.build_decision(False)
        accepted = fitter.build_decision(True)

        self.assertIs(rejected["candidate_frozen"], False)
        self.assertIs(rejected["holdout_access_authorized"], False)
        self.assertIs(rejected["weight_source_changed"], False)
        self.assertIs(accepted["candidate_frozen"], True)

    def test_registration_path_is_explicitly_selectable(self) -> None:
        fitter = load_fitter()
        args = fitter.parse_args_from(
            [
                "--train-features",
                "train.features.tsv",
                "--validation-features",
                "validation.features.tsv",
                "--implementation-commit",
                "0" * 40,
                "--registration",
                "n13.json",
                "--output",
                "report.json",
            ]
        )
        self.assertEqual(args.registration, Path("n13.json"))


if __name__ == "__main__":
    unittest.main()
