import hashlib
import json
import os
import subprocess
import sys
import tempfile
import textwrap
import time
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "profile-search.py"

SAMPLE_REPORT = textwrap.dedent(
    """\
    Analysis of sampling neyrang (pid 4242) every 1 millisecond
    Process:         neyrang [4242]
    Path:            /tmp/neyrang
    Code Type:       ARM64
    Platform:        macOS
    Date/Time:       2026-09-02 05:05:37.321 +0330
    OS Version:      macOS 26.5.2 (25F84)
    Analysis Tool:   /usr/bin/sample

    Call graph:
        100 Thread_1   DispatchQueue_1: com.apple.main-thread  (serial)

    Total number in stack (recursive counted multiple, when >=5):
            40       recursive symbol

    Sort by top of stack, same collapsed (when >= 5):
            _RNv_neyrang4sanj9classical8evaluate  (in neyrang)        30
            _RNv_neyrang4sanj5pawns8evaluate  (in neyrang)        10
            _RNv_neyrang7shegerd8ordering10MovePicker9next_move  (in neyrang)        15
            _RNv_neyrang7shegerd3see23least_valuable_attacker  (in neyrang)        12
            _RNv_neyrang7shegerd3see16prepare_exchange  (in neyrang)        8
            _RNv_neyrang5chess7movegen14generate_legal  (in neyrang)        10
            _RNv_neyrang5chess8position8Position9make_move  (in neyrang)        5
            _RNv_neyrang6rekhne6driver8Searcher7qsearch  (in neyrang)        5
            _platform_memmove  (in libsystem_platform.dylib)        5

    Binary Images:
    """
)

BENCH_OUTPUT = textwrap.dedent(
    """\
    positions: 5
    nodes: 180591
    time: 100 ms
    nps: 1805910
    checksum: e04f83f9a9880115
    """
)


def write_executable(path: Path, source: str) -> None:
    path.write_text(source, encoding="utf-8")
    path.chmod(0o755)


class SearchProfileTests(unittest.TestCase):
    def run_script(self, *arguments: str, timeout: float = 5.0) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [sys.executable, str(SCRIPT), *arguments],
            text=True,
            capture_output=True,
            timeout=timeout,
            check=False,
        )

    def test_analyze_emits_hash_bound_profile_categories(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            engine = root / "neyrang"
            engine.write_bytes(b"frozen-engine")
            profile = root / "profile.sample.txt"
            profile.write_text(SAMPLE_REPORT, encoding="utf-8")
            bench = root / "bench.txt"
            bench.write_text(BENCH_OUTPUT, encoding="utf-8")
            output = root / "result.json"

            completed = self.run_script(
                "analyze",
                "--engine",
                str(engine),
                "--bench-output",
                str(bench),
                "--profile",
                str(profile),
                "--depth",
                "5",
                "--duration",
                "1",
                "--interval-ms",
                "1",
                "--output-json",
                str(output),
            )

            self.assertEqual(0, completed.returncode, completed.stderr)
            payload = json.loads(output.read_text(encoding="utf-8"))
            self.assertEqual("neyrang-search-profile-v1", payload["schema"])
            self.assertEqual(hashlib.sha256(b"frozen-engine").hexdigest(), payload["engine"]["sha256"])
            self.assertEqual(180591, payload["benchmark"]["nodes"])
            self.assertEqual("e04f83f9a9880115", payload["benchmark"]["checksum"])
            self.assertEqual(100, payload["samples"]["total"])
            self.assertEqual(90, payload["samples"]["classified"])
            self.assertEqual(10, payload["samples"]["unclassified_visible"])
            self.assertEqual(0, payload["samples"]["suppressed"])
            self.assertEqual(40, payload["samples"]["categories"]["classical_evaluation"]["count"])
            self.assertEqual(20, payload["samples"]["categories"]["exact_see"]["count"])
            self.assertEqual(15, payload["samples"]["categories"]["move_picker"]["count"])
            self.assertEqual(10, payload["samples"]["categories"]["legal_move_generation"]["count"])
            self.assertEqual(5, payload["samples"]["categories"]["make_unmake"]["count"])

    def test_analyze_is_fail_closed_and_refuses_overwrite(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            engine = root / "neyrang"
            engine.write_bytes(b"engine")
            profile = root / "bad.sample.txt"
            profile.write_text("Call graph:\n", encoding="utf-8")
            bench = root / "bench.txt"
            bench.write_text(BENCH_OUTPUT, encoding="utf-8")
            output = root / "result.json"

            failed = self.run_script(
                "analyze",
                "--engine",
                str(engine),
                "--bench-output",
                str(bench),
                "--profile",
                str(profile),
                "--depth",
                "5",
                "--duration",
                "1",
                "--interval-ms",
                "1",
                "--output-json",
                str(output),
            )
            self.assertNotEqual(0, failed.returncode)
            self.assertFalse(output.exists())

            profile.write_text(SAMPLE_REPORT, encoding="utf-8")
            output.write_text("do not replace", encoding="utf-8")
            refused = self.run_script(
                "analyze",
                "--engine",
                str(engine),
                "--bench-output",
                str(bench),
                "--profile",
                str(profile),
                "--depth",
                "5",
                "--duration",
                "1",
                "--interval-ms",
                "1",
                "--output-json",
                str(output),
            )
            self.assertNotEqual(0, refused.returncode)
            self.assertEqual("do not replace", output.read_text(encoding="utf-8"))

    def test_capture_runs_real_children_and_binds_unchanged_engine(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            engine = root / "fake-neyrang"
            write_executable(
                engine,
                "#!/usr/bin/env python3\n"
                "import time\n"
                "time.sleep(0.15)\n"
                f"print({BENCH_OUTPUT!r}, end='')\n",
            )
            sampler = root / "fake-sample"
            write_executable(
                sampler,
                "#!/usr/bin/env python3\n"
                "import pathlib, sys\n"
                "path = pathlib.Path(sys.argv[sys.argv.index('-file') + 1])\n"
                f"path.write_text({SAMPLE_REPORT!r}, encoding='utf-8')\n",
            )
            raw = root / "captured.sample.txt"
            output = root / "captured.json"

            completed = self.run_script(
                "capture",
                "--engine",
                str(engine),
                "--sample-tool",
                str(sampler),
                "--depth",
                "5",
                "--duration",
                "1",
                "--interval-ms",
                "1",
                "--timeout",
                "2",
                "--raw-profile",
                str(raw),
                "--output-json",
                str(output),
            )

            self.assertEqual(0, completed.returncode, completed.stderr)
            payload = json.loads(output.read_text(encoding="utf-8"))
            self.assertTrue(payload["engine"]["unchanged_during_capture"])
            self.assertEqual([str(engine), "bench", "5"], payload["workload"]["command"])
            self.assertEqual(hashlib.sha256(raw.read_bytes()).hexdigest(), payload["profile"]["raw_sha256"])

    def test_capture_terminates_engine_when_sampler_fails(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            engine = root / "slow-neyrang"
            write_executable(
                engine,
                "#!/usr/bin/env python3\nimport time\ntime.sleep(30)\n",
            )
            sampler = root / "broken-sample"
            write_executable(sampler, "#!/bin/sh\nexit 3\n")
            started = time.monotonic()
            completed = self.run_script(
                "capture",
                "--engine",
                str(engine),
                "--sample-tool",
                str(sampler),
                "--depth",
                "5",
                "--duration",
                "1",
                "--interval-ms",
                "1",
                "--timeout",
                "2",
                "--raw-profile",
                str(root / "raw.txt"),
                "--output-json",
                str(root / "result.json"),
                timeout=3,
            )

            self.assertNotEqual(0, completed.returncode)
            self.assertLess(time.monotonic() - started, 2.5)
            self.assertFalse((root / "raw.txt").exists())
            self.assertFalse((root / "result.json").exists())


if __name__ == "__main__":
    unittest.main()
