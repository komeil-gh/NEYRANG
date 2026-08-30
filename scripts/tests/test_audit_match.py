from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "audit-match.py"
SPEC = importlib.util.spec_from_file_location("audit_match", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
AUDIT_MATCH = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDIT_MATCH)


class ScanLogTests(unittest.TestCase):
    def scan(self, text: str) -> dict[str, int]:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "match.log"
            path.write_text(text, encoding="utf-8")
            counts, _, _ = AUDIT_MATCH.scan_log(path)
            return counts

    def test_zero_summary_counters_are_not_anomalies(self) -> None:
        counts = self.scan("Player: NEYRANG\n  Timeouts: 0\n  Crashed: 0\n")

        self.assertEqual(counts["timeout"], 0)
        self.assertEqual(counts["crash"], 0)

    def test_nonzero_summary_counters_are_anomalies(self) -> None:
        counts = self.scan("Player: NEYRANG\n  Timeouts: 2\n  Crashed: 1\n")

        self.assertEqual(counts["timeout"], 1)
        self.assertEqual(counts["crash"], 1)

    def test_free_form_failure_messages_remain_detected(self) -> None:
        counts = self.scan("engine timed out waiting for bestmove\nengine crash detected\n")

        self.assertEqual(counts["timeout"], 1)
        self.assertEqual(counts["crash"], 1)


class ExpectedMetadataTests(unittest.TestCase):
    def test_exact_fields_pass(self) -> None:
        expected, errors = AUDIT_MATCH.audit_expected_metadata(
            {"limit_mode": "per-engine-nodes", "engine_a_nodes": "30000"},
            ["limit_mode=per-engine-nodes", "engine_a_nodes=30000"],
        )

        self.assertEqual(
            expected,
            {"limit_mode": "per-engine-nodes", "engine_a_nodes": "30000"},
        )
        self.assertEqual(errors, [])

    def test_missing_or_mismatched_fields_fail(self) -> None:
        _, errors = AUDIT_MATCH.audit_expected_metadata(
            {"limit_mode": "time"},
            ["limit_mode=per-engine-nodes", "engine_a_nodes=30000"],
        )

        self.assertEqual(len(errors), 2)
        self.assertIn("metadata limit_mode is 'time'", errors[0])
        self.assertIn("metadata engine_a_nodes is None", errors[1])

    def test_malformed_and_conflicting_expectations_fail(self) -> None:
        _, errors = AUDIT_MATCH.audit_expected_metadata(
            {"nodes": "30000"},
            ["missing-separator", "nodes=30000", "nodes=29200"],
        )

        self.assertEqual(len(errors), 2)
        self.assertIn("invalid expected metadata field", errors[0])
        self.assertIn("conflicting expected metadata values", errors[1])

    def test_expectations_require_metadata(self) -> None:
        _, errors = AUDIT_MATCH.audit_expected_metadata(
            None, ["limit_mode=per-engine-nodes"]
        )

        self.assertEqual(errors, ["expected metadata fields require --meta"])

if __name__ == "__main__":
    unittest.main()
