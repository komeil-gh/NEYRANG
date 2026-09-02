from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

import numpy as np


SCRIPT = Path(__file__).resolve().parents[1] / "analyze-sanj-features.py"


def load_analysis():
    if not SCRIPT.is_file():
        raise AssertionError(f"analysis script does not exist: {SCRIPT}")
    spec = importlib.util.spec_from_file_location("analyze_eval_features", SCRIPT)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class EvalFeatureAnalysisTests(unittest.TestCase):
    def test_group_means_keep_opening_pairs_as_the_sampling_unit(self) -> None:
        analysis = load_analysis()
        self.assertTrue(hasattr(analysis, "group_mean_values"))
        groups = np.array(["pair-b", "pair-a", "pair-a"], dtype=object)
        values = np.array([0.3, -0.2, 0.0])

        names, means = analysis.group_mean_values(groups, values)

        self.assertEqual(names.tolist(), ["pair-a", "pair-b"])
        self.assertTrue(np.allclose(means, [-0.1, 0.3]))

    def test_group_weights_give_every_opening_pair_equal_mass(self) -> None:
        analysis = load_analysis()
        groups = np.array(["pair-a", "pair-a", "pair-b"], dtype=object)

        weights = analysis.group_weights(groups)

        self.assertTrue(np.allclose(weights, [0.25, 0.25, 0.5]))
        self.assertAlmostEqual(float(weights[groups == "pair-a"].sum()), 0.5)
        self.assertAlmostEqual(float(weights[groups == "pair-b"].sum()), 0.5)

    def test_learning_subset_keeps_complete_groups(self) -> None:
        analysis = load_analysis()
        self.assertTrue(hasattr(analysis, "select_complete_groups"))
        groups = np.array(
            ["pair-a", "pair-a", "pair-b", "pair-b", "pair-c"], dtype=object
        )

        first = analysis.select_complete_groups(groups, 3, "fixed-seed")
        second = analysis.select_complete_groups(groups, 3, "fixed-seed")

        self.assertTrue(np.array_equal(first, second))
        self.assertGreaterEqual(int(first.sum()), 3)
        for group in np.unique(groups):
            values = first[groups == group]
            self.assertTrue(values.all() or not values.any())

    def test_group_bootstrap_is_deterministic(self) -> None:
        analysis = load_analysis()
        self.assertTrue(hasattr(analysis, "bootstrap_group_delta"))
        values = np.array([-0.1, 0.0, 0.2])

        first = analysis.bootstrap_group_delta(values, seed=7, replicates=1_000)
        second = analysis.bootstrap_group_delta(values, seed=7, replicates=1_000)

        self.assertEqual(first, second)
        self.assertAlmostEqual(first["mean"], float(values.mean()))
        self.assertLessEqual(first["lower"], first["mean"])
        self.assertGreaterEqual(first["upper"], first["mean"])

    def test_scale_fit_improves_a_known_weighted_problem(self) -> None:
        analysis = load_analysis()
        self.assertTrue(hasattr(analysis, "probability"))
        self.assertTrue(hasattr(analysis, "fit_scale"))
        self.assertTrue(hasattr(analysis, "weighted_loss"))
        cp = np.array([-200.0, -100.0, 100.0, 200.0])
        target = analysis.probability(cp, 2.0)
        weights = np.full(cp.size, 1.0 / cp.size)

        fitted = analysis.fit_scale(target, cp, weights, "mse")

        self.assertAlmostEqual(fitted, 2.0, places=5)
        fitted_loss = analysis.weighted_loss(
            target, analysis.probability(cp, fitted), weights, "mse"
        )
        baseline_loss = analysis.weighted_loss(
            target, analysis.probability(cp, 1.0), weights, "mse"
        )
        self.assertLess(fitted_loss, baseline_loss)

    def test_exact_integer_cp_matches_rust_taper_and_truncation(self) -> None:
        analysis = load_analysis()
        self.assertTrue(hasattr(analysis, "exact_integer_cp"))
        weights = np.asarray(analysis.CURRENT_EFFECTIVE_WEIGHTS, dtype=np.int64)
        mg = np.zeros((2, 25), dtype=np.int64)
        eg = np.zeros((2, 16), dtype=np.int64)
        mg[:, 0] = [1, -1]
        partition = analysis.Partition(
            path=Path("train.features.tsv"),
            record_ids=("row-1", "row-2"),
            groups=np.array(["pair-a", "pair-b"], dtype=object),
            design=np.zeros((2, 42)),
            target=np.array([1.0, 0.0]),
            cp=np.array([18.0, -6.0]),
            names=tuple(f"feature-{index}" for index in range(42)),
            phase=np.array([2, 2], dtype=np.int64),
            mg_coefficients=mg,
            eg_coefficients=eg,
            tempo_sign=np.array([1, 1], dtype=np.int64),
        )

        actual = analysis.exact_integer_cp(partition, weights)

        self.assertEqual(actual.tolist(), [18, 6])

    def test_partition_loader_builds_the_registered_42_columns(self) -> None:
        analysis = load_analysis()
        self.assertTrue(hasattr(analysis, "load_partition"))
        header = [
            "schema",
            "record_id",
            "target",
            "fen",
            "stm",
            "phase",
            "piece_p_delta",
            "piece_n_delta",
            "piece_b_delta",
            "piece_r_delta",
            "piece_q_delta",
            "piece_k_delta",
            "pawn_rank_delta",
            "pawn_file_edge_delta",
            "knight_center_delta",
            "bishop_center_delta",
            "rook_rank_delta",
            "rook_file_edge_delta",
            "queen_center_delta",
            "king_rank_delta",
            "king_file_edge_delta",
            "king_center_delta",
            "bishop_pair_delta",
            "doubled_extra_delta",
            "isolated_pawn_delta",
            "passed_rank_sq_delta",
            "mobility_n_delta",
            "mobility_b_delta",
            "mobility_r_delta",
            "mobility_q_delta",
            "rook_open_delta",
            "rook_semi_open_delta",
            "king_shield_delta",
            "middlegame_cp",
            "endgame_cp",
            "white_cp",
            "tempo_cp",
            "stm_cp",
        ]
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "train.features.tsv"
            rows = []
            for index, phase in enumerate((24, 0), start=1):
                values = {name: "0" for name in header}
                values.update(
                    {
                        "schema": "neyrang-sanj-trace-v1",
                        "record_id": f"source:pair-{index:06d}:game-1:ply-016",
                        "target": "0.5",
                        "fen": "8/8/8/8/8/8/8/K6k w - - 0 1",
                        "stm": "w" if index == 1 else "b",
                        "phase": str(phase),
                        "piece_p_delta": "1",
                        "white_cp": "100",
                        "tempo_cp": "12",
                        "stm_cp": "112" if index == 1 else "-88",
                    }
                )
                rows.append("\t".join(values[name] for name in header))
            path.write_text("\t".join(header) + "\n" + "\n".join(rows) + "\n")

            partition = analysis.load_partition(path)

            invalid_header = header.copy()
            invalid_header[3] = "FEN"
            invalid = Path(temporary) / "invalid.features.tsv"
            invalid.write_text(
                "\t".join(invalid_header) + "\n" + "\n".join(rows) + "\n"
            )
            with self.assertRaisesRegex(ValueError, "canonical 38-column header"):
                analysis.load_partition(invalid)

        self.assertEqual(partition.design.shape, (2, 42))
        self.assertEqual(partition.names[-1], "tempo")
        self.assertEqual(
            partition.groups.tolist(),
            ["source:pair-000001", "source:pair-000002"],
        )
        self.assertTrue(np.allclose(partition.cp, [112.0, 88.0]))

    def test_partition_loader_accepts_hash_bound_pre_contract_schema(self) -> None:
        analysis = load_analysis()
        header = list(analysis.TRACE_COLUMNS)
        values = {name: "0" for name in header}
        values.update(
            {
                "schema": "historical-eval-trace-v1",
                "record_id": "source:pair-000001:game-1:ply-016",
                "target": "0.5",
                "fen": "8/8/8/8/8/8/8/K6k w - - 0 1",
                "stm": "w",
                "phase": "0",
                "white_cp": "0",
                "tempo_cp": "12",
                "stm_cp": "12",
            }
        )
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "train.features.tsv"
            path.write_text(
                "\t".join(header)
                + "\n"
                + "\t".join(values[name] for name in header)
                + "\n"
            )

            partition = analysis.load_partition(path)

        self.assertEqual(partition.cp.tolist(), [12.0])

    def test_partition_analysis_reports_train_validation_only(self) -> None:
        analysis = load_analysis()
        self.assertTrue(hasattr(analysis, "analyze_partitions"))
        names = tuple(f"feature-{index}" for index in range(42))
        train_cp = np.array([-200.0, -100.0, 100.0, 200.0])
        validation_cp = np.array([-150.0, -50.0, 50.0, 150.0])

        def partition(label: str, cp: np.ndarray):
            groups = np.array(
                [f"{label}:pair-000001"] * 2 + [f"{label}:pair-000002"] * 2,
                dtype=object,
            )
            return analysis.Partition(
                path=Path(f"{label}.features.tsv"),
                record_ids=tuple(f"{label}-{index}" for index in range(4)),
                groups=groups,
                design=np.column_stack([cp] + [np.zeros(4)] * 41),
                target=analysis.probability(cp, 1.75),
                cp=cp,
                names=names,
                phase=np.full(4, 24, dtype=np.int64),
                mg_coefficients=np.zeros((4, 25), dtype=np.int64),
                eg_coefficients=np.zeros((4, 16), dtype=np.int64),
                tempo_sign=np.ones(4, dtype=np.int64),
            )

        report = analysis.analyze_partitions(
            partition("train", train_cp),
            partition("validation", validation_cp),
            bootstrap_replicates=200,
            learning_floor=25_000,
        )

        self.assertEqual(report["train"]["rows"], 4)
        self.assertEqual(report["validation"]["groups"], 2)
        self.assertEqual(
            [point["label"] for point in report["learning_curve"]], ["full"]
        )
        first_coverage = report["train"]["design"]["coverage"][0]
        self.assertEqual(first_coverage["unique_values"], 4)
        self.assertAlmostEqual(
            first_coverage["standard_deviation"], float(train_cp.std())
        )
        self.assertAlmostEqual(
            report["learning_curve"][0]["losses"]["mse"]["fitted_scale"],
            1.75,
            places=5,
        )
        self.assertNotIn("holdout", json.dumps(report).lower())


if __name__ == "__main__":
    unittest.main()
