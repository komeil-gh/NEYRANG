import importlib.util
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "smp-bench.py"
SPEC = importlib.util.spec_from_file_location("smp_bench", SCRIPT)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC.loader is not None
SPEC.loader.exec_module(MODULE)


class SmpBenchTests(unittest.TestCase):
    def test_rotating_schedule_balances_first_configuration(self):
        self.assertEqual(
            MODULE.rotating_schedule([1, 2, 4], 4),
            [[1, 2, 4], [2, 4, 1], [4, 1, 2], [1, 2, 4]],
        )

    def test_info_parser_requires_complete_numeric_telemetry(self):
        parsed = MODULE.parse_info(
            "info depth 12 seldepth 19 score cp 34 nodes 123456 nps 2345678 "
            "hashfull 81 time 53 pv e2e4 e7e5"
        )
        self.assertEqual(parsed["depth"], 12)
        self.assertEqual(parsed["seldepth"], 19)
        self.assertEqual(parsed["score"], {"kind": "cp", "value": 34})
        self.assertEqual(parsed["nodes"], 123456)
        self.assertEqual(parsed["pv"], ["e2e4", "e7e5"])

        with self.assertRaisesRegex(ValueError, "missing info field: nodes"):
            MODULE.parse_info("info depth 2 seldepth 3 score cp 0 time 1 pv e2e4")

    def test_nearest_rank_percentile_is_explicit(self):
        self.assertEqual(MODULE.nearest_rank([9, 1, 5, 3, 7], 0.5), 5)
        self.assertEqual(MODULE.nearest_rank(list(range(1, 21)), 0.95), 19)

    def test_summary_applies_registered_scaling_and_deadline_gates(self):
        samples = []
        for threads, nps_values, overshoots in [
            (1, [100, 102, 98], [0, 1, 2]),
            (2, [140, 142, 138], [1, 2, 3]),
            (4, [180, 182, 178], [2, 3, 4]),
        ]:
            for run, nps in enumerate(nps_values):
                samples.append(
                    {
                        "threads": threads,
                        "run": run,
                        "aggregate_nps": nps,
                        "positions": [
                            {"deadline_overshoot_ms": value} for value in overshoots
                        ],
                    }
                )

        summary, decision = MODULE.summarize(samples, [1, 2, 4])

        self.assertEqual(summary["2"]["scaling_vs_threads_1"], 1.4)
        self.assertEqual(summary["4"]["scaling_vs_threads_1"], 1.8)
        self.assertTrue(decision["passed"])

        samples[-1]["positions"] = [{"deadline_overshoot_ms": 31}] * 20
        _, decision = MODULE.summarize(samples, [1, 2, 4])
        self.assertFalse(decision["passed"])
        self.assertIn("Threads 4 p95 deadline overshoot", decision["failures"][0])


if __name__ == "__main__":
    unittest.main()
