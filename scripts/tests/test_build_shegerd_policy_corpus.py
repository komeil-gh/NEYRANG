from __future__ import annotations

import importlib.util
import hashlib
import json
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

import chess
import chess.pgn


SCRIPT = Path(__file__).resolve().parents[1] / "build-shegerd-policy-corpus.py"
ROOT = SCRIPT.parent.parent
POLICY_TRACE_MANIFEST = ROOT / "tools" / "policy-trace" / "Cargo.toml"
SPEC = importlib.util.spec_from_file_location("build_shegerd_policy_corpus", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
corpus = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = corpus
SPEC.loader.exec_module(corpus)


MOVES = "Nf3 Nf6 Ng1 Ng8 " * 8


def pair(round_id: str) -> str:
    return (
        f'[Event "fixture"]\n[Round "{round_id}"]\n[White "A"]\n[Black "B"]\n'
        '[Result "1/2-1/2"]\n[Termination "normal"]\n[TimeControl "0.5+0.005"]\n'
        '[Variant "Standard"]\n[PlyCount "32"]\n\n'
        f'1. {MOVES}1/2-1/2\n\n'
        f'[Event "fixture"]\n[Round "{round_id}"]\n[White "B"]\n[Black "A"]\n'
        '[Result "1/2-1/2"]\n[Termination "normal"]\n[TimeControl "0.5+0.005"]\n'
        '[Variant "Standard"]\n[PlyCount "32"]\n\n'
        f'1. {MOVES}1/2-1/2\n\n'
    )


class BuildShegerdPolicyCorpusTests(unittest.TestCase):
    def write_teacher(self, path: Path) -> None:
        path.write_text(textwrap.dedent("""\
            #!/usr/bin/env python3
            import sys
            side = "w"
            for raw in sys.stdin:
                command = raw.strip()
                if command == "uci":
                    print("id name FixtureTeacher")
                    print("option name Threads type spin default 1 min 1 max 1")
                    print("option name Hash type spin default 64 min 1 max 64")
                    print("option name Clear Hash type button")
                    print("uciok", flush=True)
                elif command == "isready":
                    print("readyok", flush=True)
                elif command.startswith("position fen "):
                    side = command.split()[3]
                elif command.startswith("go nodes"):
                    move = "a2a3" if side == "w" else "a7a6"
                    print(f"info depth 1 pv {move}")
                    print(f"bestmove {move}", flush=True)
                elif command == "quit":
                    break
        """))
        path.chmod(0o755)

    def test_sampler_preserves_previous_destination_and_gap(self) -> None:
        game = chess.pgn.read_game(__import__("io").StringIO(pair("1")))
        assert game is not None
        config = corpus.Config("fixture", Path("source"), Path("teacher"), Path("output"), "sample", "split", 80000, 64, enforce_minimums=False)
        samples, _ = corpus.samples_for_game(game, "fixture", 1, 1, config)
        self.assertLessEqual(len(samples), 4)
        self.assertTrue(all(right.ply - left.ply >= 8 for left, right in zip(sorted(samples, key=lambda item: item.ply), sorted(samples, key=lambda item: item.ply)[1:])))
        self.assertTrue(all(sample.previous_to in {"f3", "f6", "g1", "g8"} for sample in samples))

    def test_partition_assignment_is_deterministic_and_complete(self) -> None:
        openings = {f"8/8/8/8/8/8/8/K{k}6 w - -" for k in range(1, 9)}
        first = corpus.assign_partitions(openings, "seed")
        self.assertEqual(first, corpus.assign_partitions(openings, "seed"))
        self.assertEqual(set(first), openings)
        self.assertEqual(set(first.values()), {"train", "validation", "sealed_holdout"})

    def test_builder_emits_p0_label_contract_with_synthetic_teacher(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source.pgn"
            teacher = root / "teacher.py"
            output = root / "output"
            source.write_text(pair("1") + pair("2"), encoding="utf-8")
            self.write_teacher(teacher)
            config = corpus.Config("fixture", source, teacher, output, "sample", "split", 80000, 64, enforce_minimums=False)
            manifest = corpus.build(config)
            self.assertEqual(corpus.SCHEMA, manifest["schema"])
            total = 0
            populated: Path | None = None
            populated_count = 0
            for partition in corpus.PARTITIONS:
                path = output / f"{partition}.labels.tsv"
                lines = path.read_text(encoding="utf-8").splitlines()
                total += len(lines)
                if lines:
                    populated = path
                    populated_count = len(lines)
                for line in lines:
                    self.assertEqual(5, len(line.split("\t")))
            self.assertGreater(total, 0)
            assert populated is not None
            traced = subprocess.run(
                [
                    "cargo", "run", "--quiet", "--release", "--locked",
                    "--manifest-path", str(POLICY_TRACE_MANIFEST), "--", str(populated),
                ],
                cwd=ROOT,
                text=True,
                capture_output=True,
            )
            self.assertEqual(0, traced.returncode, traced.stderr)
            trace_lines = traced.stdout.splitlines()
            self.assertGreater(len(trace_lines), 1)
            self.assertEqual(15, len(trace_lines[0].split("\t")))
            self.assertEqual(
                populated_count,
                sum(line.split("\t")[4] == "1" for line in trace_lines[1:]),
            )

    def test_source_audit_binds_campaign_and_teacher_identity(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            source = root / "source.pgn"
            teacher = root / "teacher"
            audit = root / "audit.json"
            source.write_text("", encoding="utf-8")
            teacher.write_bytes(b"teacher")
            teacher_hash = hashlib.sha256(b"teacher").hexdigest()
            metadata = {
                **corpus.MATCH_METADATA,
                "engine_a_sha256": teacher_hash,
                "engine_b_sha256": "1" * 64,
                "fastchess_sha256": "2" * 64,
                "openings_sha256": "3" * 64,
            }
            report = {
                "format": "neyrang-match-audit-v1",
                "ok": True,
                "pgn": str(source),
                "candidate": corpus.SOURCE_ENGINES[0],
                "opponent": corpus.SOURCE_ENGINES[1],
                "games": 2000,
                "pairs": 1000,
                "unique_opening_fens": 1000,
                "expected_openings": {"path": "openings.epd", "pairs": 1000},
                "terminations": {"normal": 2000},
                "time_controls": {"0.5+0.005": 2000},
                "telemetry_missing": {},
                "allowed_log_warnings": [],
                "log_anomaly_counts": {"warning": 0, "timeout": 0},
                "metadata": metadata,
            }
            audit.write_text(json.dumps(report), encoding="utf-8")
            config = corpus.Config(
                corpus.SOURCE_ID, source, teacher, root / "output", "sample", "split", 80000, 64,
                source_audit=audit,
            )
            evidence = corpus.validate_source_audit(
                config, {"engines": sorted(corpus.SOURCE_ENGINES)}, teacher_hash,
            )
            self.assertEqual(teacher_hash, evidence["engine_a_sha256"])

            report["metadata"]["engine_a_sha256"] = "4" * 64
            audit.write_text(json.dumps(report), encoding="utf-8")
            with self.assertRaisesRegex(corpus.CorpusError, "teacher binary differs"):
                corpus.validate_source_audit(
                    config, {"engines": sorted(corpus.SOURCE_ENGINES)}, teacher_hash,
                )


if __name__ == "__main__":
    unittest.main()
