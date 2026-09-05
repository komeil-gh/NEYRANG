from __future__ import annotations

import importlib.util
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

import chess
import chess.pgn


SCRIPT = Path(__file__).resolve().parents[1] / "build-shegerd-policy-corpus.py"
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
            for partition in corpus.PARTITIONS:
                lines = (output / f"{partition}.labels.tsv").read_text(encoding="utf-8").splitlines()
                self.assertEqual(corpus.LABEL_HEADER.strip(), lines[0])
                total += len(lines) - 1
                for line in lines[1:]:
                    self.assertEqual(5, len(line.split("\t")))
            self.assertGreater(total, 0)


if __name__ == "__main__":
    unittest.main()
