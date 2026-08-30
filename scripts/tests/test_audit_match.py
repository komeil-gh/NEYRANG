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


if __name__ == "__main__":
    unittest.main()
