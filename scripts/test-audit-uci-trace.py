#!/usr/bin/env python3

from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).with_name("audit-uci-trace.py")


def engine_line(engine: str, direction: str, message: str, tick: int) -> str:
    return f"[Engine] [00:00:00.{tick:06d}] < 0x1>  {engine} {direction} {message}"


class TraceAuditTests(unittest.TestCase):
    def run_audit(self, lines: list[str], expected_go_count: int = 2):
        directory = tempfile.TemporaryDirectory()
        root = Path(directory.name)
        trace = root / "engine.log"
        output = root / "audit.json"
        trace.write_text("\n".join(lines) + "\n", encoding="utf-8")
        command = [
            sys.executable,
            str(SCRIPT),
            "--log",
            str(trace),
            "--engine",
            "NEYRANG-A",
            "--engine",
            "NEYRANG-B",
            "--expected-go-count",
            str(expected_go_count),
            "--output",
            str(output),
        ]
        completed = subprocess.run(command, capture_output=True, text=True, check=False)
        result = json.loads(output.read_text(encoding="utf-8"))
        return directory, command, completed, output, result

    def test_accepts_complete_intervals_and_is_deterministic(self):
        lines = [
            engine_line("NEYRANG-A", "<---", "go wtime 100 btime 100", 1),
            engine_line("NEYRANG-A", "--->", "info depth 1 nodes 1", 2),
            engine_line("NEYRANG-A", "--->", "bestmove e2e4", 3),
            engine_line("NEYRANG-B", "<---", "go wtime 100 btime 100", 4),
            engine_line("NEYRANG-B", "--->", "bestmove e7e5", 5),
        ]
        directory, command, completed, output, result = self.run_audit(lines)
        self.addCleanup(directory.cleanup)
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(result["status"], "passed")
        first = output.read_bytes()
        rerun = subprocess.run(command, capture_output=True, text=True, check=False)
        self.assertEqual(rerun.returncode, 0, rerun.stderr)
        self.assertEqual(output.read_bytes(), first)

    def test_rejects_response_interval_violations(self):
        cases = {
            "duplicate": [
                engine_line("NEYRANG-A", "<---", "go depth 1", 1),
                engine_line("NEYRANG-A", "--->", "bestmove e2e4", 2),
                engine_line("NEYRANG-A", "--->", "bestmove d2d4", 3),
            ],
            "late_info": [
                engine_line("NEYRANG-A", "<---", "go depth 1", 1),
                engine_line("NEYRANG-A", "--->", "bestmove e2e4", 2),
                engine_line("NEYRANG-A", "--->", "info depth 2 nodes 2", 3),
            ],
            "overlap": [
                engine_line("NEYRANG-A", "<---", "go depth 1", 1),
                engine_line("NEYRANG-A", "<---", "go depth 2", 2),
                engine_line("NEYRANG-A", "--->", "bestmove e2e4", 3),
            ],
            "open_eof": [engine_line("NEYRANG-A", "<---", "go depth 1", 1)],
        }
        for name, lines in cases.items():
            with self.subTest(name=name):
                directory, _, completed, _, result = self.run_audit(
                    lines, expected_go_count=sum("<--- go" in line for line in lines)
                )
                try:
                    self.assertEqual(completed.returncode, 1)
                    self.assertEqual(result["status"], "failed")
                    self.assertTrue(result["violations"])
                finally:
                    directory.cleanup()


if __name__ == "__main__":
    unittest.main()
