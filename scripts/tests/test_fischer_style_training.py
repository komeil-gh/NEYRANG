import json
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

import chess


ROOT = Path(__file__).resolve().parents[2]
LABELER = ROOT / "scripts" / "label-fischer-decisions.py"
FITTER = ROOT / "scripts" / "fit-fischer-prior.py"
EVALUATOR = ROOT / "scripts" / "evaluate-fischer-prior.py"


class FischerStyleTrainingTests(unittest.TestCase):
    def write_fake_engine(self, path: Path) -> None:
        path.write_text(
            textwrap.dedent(
                """\
                #!/usr/bin/env python3
                import sys

                for raw in sys.stdin:
                    command = raw.strip()
                    if command == "uci":
                        print("id name FixtureTeacher")
                        print("id author NEYRANG")
                        print("option name Threads type spin default 1 min 1 max 1")
                        print("option name Hash type spin default 16 min 1 max 1024")
                        print("option name Clear Hash type button")
                        print("option name Ponder type check default false")
                        print("uciok", flush=True)
                    elif command == "isready":
                        print("readyok", flush=True)
                    elif command.startswith("go"):
                        if "searchmoves d2d4" in command:
                            print("info depth 1 score cp 5 pv d2d4")
                            print("bestmove d2d4", flush=True)
                        else:
                            print("info depth 1 score cp 20 pv e2e4")
                            print("bestmove e2e4", flush=True)
                    elif command == "quit":
                        break
                """
            )
        )
        path.chmod(0o755)

    @staticmethod
    def decision(move: str = "d2d4") -> dict[str, object]:
        board = chess.Board()
        return {
            "fen": board.fen(en_passant="legal"),
            "move": move,
            "game_id": "fixture",
            "ply": 12,
            "legal_moves": board.legal_moves.count(),
            "position_key": "fixture-position",
            "source_ids": ["fixture"],
            "result": "1-0",
            "fischer_color": "white",
        }

    def test_labeler_records_fixed_node_teacher_loss(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            engine = root / "teacher.py"
            source = root / "decisions.jsonl"
            output = root / "labels.jsonl"
            self.write_fake_engine(engine)
            source.write_text(json.dumps(self.decision()) + "\n")
            result = subprocess.run(
                [
                    sys.executable,
                    str(LABELER),
                    "--input",
                    str(source),
                    "--output",
                    str(output),
                    "--engine",
                    str(engine),
                    "--nodes",
                    "1",
                ],
                cwd=ROOT,
                text=True,
                capture_output=True,
            )
            self.assertEqual(0, result.returncode, result.stderr)
            label = json.loads(output.read_text())
            self.assertEqual("e2e4", label["teacher_best_move"])
            self.assertEqual(15, label["teacher_loss_cp"])
            manifest = json.loads(
                output.with_suffix(".jsonl.manifest.json").read_text()
            )
            self.assertEqual("FixtureTeacher", manifest["teacher"]["uci_id"]["name"])

    def test_fitter_is_deterministic_and_learns_repeated_move(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            train = root / "train.jsonl"
            validation = root / "validation.jsonl"
            labels = []
            for index in range(8):
                label = self.decision("e2e4")
                label["game_id"] = f"fixture-{index}"
                label["teacher_loss_cp"] = 0
                labels.append(label)
            payload = "".join(json.dumps(label) + "\n" for label in labels)
            train.write_text(payload)
            validation.write_text(payload)

            hashes = []
            for run in ("a", "b"):
                artifact = root / f"prior-{run}.bin"
                report = root / f"report-{run}.json"
                result = subprocess.run(
                    [
                        sys.executable,
                        str(FITTER),
                        "--train",
                        str(train),
                        "--validation",
                        str(validation),
                        "--output",
                        str(artifact),
                        "--report",
                        str(report),
                        "--min-exposure",
                        "1",
                    ],
                    cwd=ROOT,
                    text=True,
                    capture_output=True,
                )
                self.assertEqual(0, result.returncode, result.stderr)
                report_data = json.loads(report.read_text())
                self.assertEqual(1.0, report_data["validation"]["top1"])
                hashes.append(report_data["artifact"]["sha256"])
            self.assertEqual(hashes[0], hashes[1])

            evaluation = root / "evaluation.json"
            result = subprocess.run(
                [
                    sys.executable,
                    str(EVALUATOR),
                    "--labels",
                    str(validation),
                    "--artifact",
                    str(root / "prior-a.bin"),
                    "--artifact-sha256",
                    hashes[0],
                    "--output",
                    str(evaluation),
                ],
                cwd=ROOT,
                text=True,
                capture_output=True,
            )
            self.assertEqual(0, result.returncode, result.stderr)
            self.assertEqual(1.0, json.loads(evaluation.read_text())["metrics"]["top1"])


if __name__ == "__main__":
    unittest.main()
