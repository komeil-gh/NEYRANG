from __future__ import annotations

import hashlib
import importlib.util
import json
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest
from pathlib import Path

import chess
import chess.pgn


ROOT = Path(__file__).resolve().parents[2]
AUDITOR_PATH = ROOT / "scripts" / "audit-shegerd-policy-corpus.py"
BUILDER_PATH = ROOT / "scripts" / "build-shegerd-policy-corpus.py"
POLICY_TRACE_MANIFEST = ROOT / "tools" / "policy-trace" / "Cargo.toml"


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[name] = module
    spec.loader.exec_module(module)
    return module


auditor = load_module("audit_shegerd_policy_corpus", AUDITOR_PATH)
builder = load_module("build_shegerd_policy_corpus_for_audit", BUILDER_PATH)


def fixture_board(index: int) -> chess.Board:
    board = chess.Board(None)
    board.set_piece_at(chess.A1, chess.Piece(chess.KING, chess.WHITE))
    board.set_piece_at(chess.H8, chess.Piece(chess.KING, chess.BLACK))
    board.set_piece_at(chess.B1, chess.Piece(chess.KNIGHT, chess.WHITE))
    board.set_piece_at(chess.G8, chess.Piece(chess.KNIGHT, chess.BLACK))
    board.set_piece_at(chess.C2 + index % 5, chess.Piece(chess.PAWN, chess.WHITE))
    board.set_piece_at(chess.C7 + index // 5, chess.Piece(chess.PAWN, chess.BLACK))
    board.turn = chess.WHITE
    return board


def fixture_game(index: int, round_id: str, white: str, black: str) -> chess.pgn.Game:
    board = fixture_board(index)
    game = chess.pgn.Game()
    game.setup(board)
    game.headers.update(
        {
            "Event": "fixture",
            "Round": round_id,
            "White": white,
            "Black": black,
            "Result": "1/2-1/2",
            "Termination": "normal",
            "TimeControl": "0.5+0.005",
            "Variant": "Standard",
            "PlyCount": "32",
        }
    )
    node = game
    moves = ("b1a3", "g8h6", "a3b1", "h6g8") * 8
    for uci in moves:
        move = chess.Move.from_uci(uci)
        assert move in board.legal_moves
        node = node.add_variation(move)
        board.push(move)
    return game


class AuditShegerdPolicyCorpusTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls._fixture = tempfile.TemporaryDirectory()
        root = Path(cls._fixture.name)
        cls.source = root / "source.pgn"
        cls.teacher = root / "teacher.py"
        cls.corpus = root / "corpus"
        cls.trace = root / "trace"
        with cls.source.open("w", encoding="utf-8") as stream:
            exporter = chess.pgn.FileExporter(stream)
            for index in range(10):
                fixture_game(index, str(index + 1), "A", "B").accept(exporter)
                fixture_game(index, str(index + 1), "B", "A").accept(exporter)
        cls.teacher.write_text(
            textwrap.dedent(
                f"""\
                #!{sys.executable}
                import chess
                import sys
                move = "0000"
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
                        board = chess.Board(command.removeprefix("position fen "))
                        move = min(candidate.uci() for candidate in board.legal_moves)
                    elif command.startswith("go nodes"):
                        print(f"info depth 1 pv {{move}}")
                        print(f"bestmove {{move}}", flush=True)
                    elif command == "quit":
                        break
                """
            ),
            encoding="utf-8",
        )
        cls.teacher.chmod(0o755)
        builder.build(
            builder.Config(
                "fixture",
                cls.source,
                cls.teacher,
                cls.corpus,
                "20260905-shegerd-p1-sample-v1",
                "20260905-shegerd-p1-split-v1",
                80000,
                64,
                enforce_minimums=False,
            )
        )
        cls.trace.mkdir()
        for partition in auditor.PARTITIONS:
            labels = cls.corpus / f"{partition}.labels.tsv"
            traced = subprocess.run(
                [
                    "cargo",
                    "run",
                    "--quiet",
                    "--release",
                    "--locked",
                    "--manifest-path",
                    str(POLICY_TRACE_MANIFEST),
                    "--",
                    str(labels),
                ],
                cwd=ROOT,
                text=True,
                capture_output=True,
            )
            if traced.returncode:
                raise RuntimeError(traced.stderr)
            (cls.trace / f"{partition}.trace.tsv").write_text(
                traced.stdout, encoding="utf-8"
            )

    @classmethod
    def tearDownClass(cls) -> None:
        cls._fixture.cleanup()

    def copies(self) -> tuple[tempfile.TemporaryDirectory, Path, Path]:
        temporary = tempfile.TemporaryDirectory()
        root = Path(temporary.name)
        corpus = root / "corpus"
        trace = root / "trace"
        shutil.copytree(self.corpus, corpus)
        shutil.copytree(self.trace, trace)
        return temporary, corpus, trace

    def test_valid_replay_passes_and_sealed_report_is_bounded(self) -> None:
        summary = auditor.audit(
            self.source, None, self.corpus, self.trace, enforce_campaign=False
        )
        self.assertTrue(summary["ok"])
        sealed = summary["partitions"]["sealed_holdout"]
        self.assertEqual({"records", "bytes", "sha256"}, set(sealed["labels"]))
        self.assertEqual(
            {"records", "data_rows", "columns", "bytes", "sha256"},
            set(sealed["trace"]),
        )

    def test_illegal_teacher_move_fails_after_valid_hash_update(self) -> None:
        temporary, corpus, trace = self.copies()
        try:
            manifest_path = corpus / "manifest.json"
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            partition = next(
                name
                for name in auditor.PARTITIONS
                if manifest["artifacts"][name]["records"]
            )
            labels = corpus / f"{partition}.labels.tsv"
            lines = labels.read_text(encoding="utf-8").splitlines()
            fields = lines[0].split("\t")
            fields[3] = "a1a8"
            lines[0] = "\t".join(fields)
            labels.write_text("\n".join(lines) + "\n", encoding="utf-8")
            payload = labels.read_bytes()
            manifest["artifacts"][partition]["bytes"] = len(payload)
            manifest["artifacts"][partition]["sha256"] = hashlib.sha256(payload).hexdigest()
            manifest_path.write_text(
                json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8"
            )
            with self.assertRaisesRegex(auditor.AuditError, "teacher move is not legal"):
                auditor.audit(self.source, None, corpus, trace, enforce_campaign=False)
        finally:
            temporary.cleanup()

    def test_missing_legal_sibling_and_duplicate_selection_fail(self) -> None:
        temporary, corpus, trace = self.copies()
        try:
            partition = next(
                name
                for name in auditor.PARTITIONS
                if (trace / f"{name}.trace.tsv").read_text(encoding="utf-8").count("\n") > 2
            )
            path = trace / f"{partition}.trace.tsv"
            lines = path.read_text(encoding="utf-8").splitlines()
            path.write_text("\n".join(lines[:1] + lines[2:]) + "\n", encoding="utf-8")
            with self.assertRaisesRegex(auditor.AuditError, "incomplete legal sibling set|trace record sequence"):
                auditor.audit(self.source, None, corpus, trace, enforce_campaign=False)

            shutil.copy2(self.trace / f"{partition}.trace.tsv", path)
            lines = path.read_text(encoding="utf-8").splitlines()
            first_id = lines[1].split("\t")[1]
            for index in range(2, len(lines)):
                fields = lines[index].split("\t")
                if fields[1] != first_id:
                    break
                if fields[4] == "0":
                    fields[4] = "1"
                    lines[index] = "\t".join(fields)
                    break
            path.write_text("\n".join(lines) + "\n", encoding="utf-8")
            with self.assertRaisesRegex(auditor.AuditError, "selected move differs"):
                auditor.audit(self.source, None, corpus, trace, enforce_campaign=False)
        finally:
            temporary.cleanup()

    def test_hash_mismatch_and_cross_partition_dedup_are_fail_closed(self) -> None:
        temporary, corpus, trace = self.copies()
        try:
            labels = corpus / "train.labels.tsv"
            labels.write_text(labels.read_text(encoding="utf-8") + "x\n", encoding="utf-8")
            with self.assertRaisesRegex(auditor.AuditError, "SHA-256 differs"):
                auditor.audit(self.source, None, corpus, trace, enforce_campaign=False)
        finally:
            temporary.cleanup()
        left = auditor.ExpectedRecord("train", "a", "g", "key", "-", chess.STARTING_FEN)
        right = auditor.ExpectedRecord("validation", "b", "g", "key", "-", chess.STARTING_FEN)
        kept, evidence = auditor.deduplicate([left, right])
        self.assertEqual([], kept)
        self.assertEqual(1, evidence["cross_partition_keys_removed"])


if __name__ == "__main__":
    unittest.main()
