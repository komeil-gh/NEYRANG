from __future__ import annotations

import hashlib
import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path

import chess


SCRIPTS = Path(__file__).resolve().parents[1]
ASSEMBLER_PATH = SCRIPTS / "assemble-nnue-corpus.py"
AUDITOR_PATH = SCRIPTS / "audit-nnue-data.py"


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class NnueCorpusAssemblerTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.assembler = load_module("assemble_nnue_corpus", ASSEMBLER_PATH)
        cls.auditor = load_module("assemble_nnue_auditor", AUDITOR_PATH)

    def test_complete_games_are_shuffled_deterministically_and_reaudited(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            shard_a = write_shard(
                root,
                "train-a.vf",
                [mate_in_one("7k/5K2/6Q1/8/8/8/8/8 w - - 0 1", "g6g7")],
                self.auditor,
            )
            shard_b = write_shard(
                root,
                "train-b.vf",
                [mate_in_one("7k/5K2/4Q3/8/8/8/8/8 w - - 0 1", "e6h6")],
                self.auditor,
            )

            output_ab = root / "corpus-ab.vf"
            output_ba = root / "corpus-ba.vf"
            manifest_ab = self.assembler.assemble_corpus(
                make_config(self.assembler, root, (shard_a, shard_b), output_ab)
            )
            manifest_ba = self.assembler.assemble_corpus(
                make_config(self.assembler, root, (shard_b, shard_a), output_ba)
            )

            self.assertEqual(output_ab.read_bytes(), output_ba.read_bytes())
            self.assertEqual(manifest_ab["schema"], "neyrang-nnue-corpus-v1")
            self.assertEqual(manifest_ab["artifact"]["games"], 2)
            self.assertEqual(manifest_ab["artifact"]["scored_positions"], 2)
            self.assertEqual(manifest_ab["artifact"]["sha256"], manifest_ba["artifact"]["sha256"])
            self.assertEqual(
                manifest_ab["shuffle"]["order_sha256"],
                manifest_ba["shuffle"]["order_sha256"],
            )
            self.assertEqual(manifest_ab["separation"]["duplicate_opening_groups"], 0)
            self.assertEqual(manifest_ab["separation"]["cross_partition_position_keys"], 0)
            self.assertEqual(manifest_ab["audit"]["completion_reasons"], {"checkmate": 2})
            self.assertEqual(
                json.loads(Path(f"{output_ab}.manifest.json").read_text()), manifest_ab
            )

    def test_modified_source_is_rejected_without_publishing_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            shard = write_shard(
                root,
                "train.vf",
                [mate_in_one("7k/5K2/6Q1/8/8/8/8/8 w - - 0 1", "g6g7")],
                self.auditor,
            )
            shard.write_bytes(shard.read_bytes() + b"\0")
            output = root / "rejected.vf"

            with self.assertRaisesRegex(self.assembler.AssemblyError, "SHA-256"):
                self.assembler.assemble_corpus(
                    make_config(self.assembler, root, (shard,), output)
                )

            self.assertFalse(output.exists())
            self.assertFalse(Path(f"{output}.manifest.json").exists())

    def test_duplicate_opening_group_across_shards_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            game = mate_in_one("7k/5K2/6Q1/8/8/8/8/8 w - - 0 1", "g6g7")
            shard_a = write_shard(root, "train-a.vf", [game], self.auditor)
            shard_b = write_shard(root, "train-b.vf", [game], self.auditor)
            output = root / "rejected.vf"

            with self.assertRaisesRegex(self.assembler.AssemblyError, "opening group"):
                self.assembler.assemble_corpus(
                    make_config(self.assembler, root, (shard_a, shard_b), output)
                )

            self.assertFalse(output.exists())

    def test_opt_in_opening_quarantine_is_complete_and_input_order_independent(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            duplicate = mate_in_one(
                "7k/5K2/6Q1/8/8/8/8/8 w - - 0 1", "g6g7"
            )
            clean = mate_in_one(
                "7k/5K2/4Q3/8/8/8/8/8 w - - 0 1", "e6h6"
            )
            shard_a = write_shard(root, "train-a.vf", [duplicate], self.auditor)
            shard_b = write_shard(
                root, "train-b.vf", [duplicate, clean], self.auditor
            )
            output_ab = root / "corpus-ab.vf"
            output_ba = root / "corpus-ba.vf"

            manifest_ab = self.assembler.assemble_corpus(
                make_config(
                    self.assembler,
                    root,
                    (shard_a, shard_b),
                    output_ab,
                    quarantine_duplicate_opening_games=True,
                )
            )
            manifest_ba = self.assembler.assemble_corpus(
                make_config(
                    self.assembler,
                    root,
                    (shard_b, shard_a),
                    output_ba,
                    quarantine_duplicate_opening_games=True,
                )
            )

            self.assertEqual(output_ab.read_bytes(), output_ba.read_bytes())
            self.assertEqual(manifest_ab["artifact"]["games"], 2)
            self.assertEqual(
                manifest_ab["separation"]["detected_duplicate_opening_groups"], 1
            )
            self.assertEqual(
                manifest_ab["separation"]["quarantined_duplicate_opening_games"],
                1,
            )
            self.assertEqual(
                manifest_ab["separation"]
                ["quarantined_duplicate_opening_scored_positions"],
                1,
            )
            self.assertEqual(manifest_ab["separation"]["duplicate_opening_groups"], 0)

    def test_cross_partition_opening_group_is_always_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            game = mate_in_one("7k/5K2/6Q1/8/8/8/8/8 w - - 0 1", "g6g7")
            train_shard = write_shard(root, "train.vf", [game], self.auditor)
            train = root / "train-corpus.vf"
            self.assembler.assemble_corpus(
                make_config(self.assembler, root, (train_shard,), train)
            )
            validation_shard = write_shard(
                root,
                "validation.vf",
                [game],
                self.auditor,
                partition="validation",
            )
            validation = root / "validation-corpus.vf"

            with self.assertRaisesRegex(self.assembler.AssemblyError, "opening group"):
                self.assembler.assemble_corpus(
                    make_config(
                        self.assembler,
                        root,
                        (validation_shard,),
                        validation,
                        partition="validation",
                        disjoint_from=(train,),
                    )
                )

            self.assertFalse(validation.exists())

    def test_cross_partition_position_key_is_rejected_by_default(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            train_game = game_from_line(
                chess.STARTING_FEN, ["f2f3", "e7e5", "g2g4", "d8h4"], result=0
            )
            train_shard = write_shard(root, "train.vf", [train_game], self.auditor)
            train = root / "train-corpus.vf"
            self.assembler.assemble_corpus(
                make_config(self.assembler, root, (train_shard,), train)
            )
            after_f3 = chess.Board()
            after_f3.push_uci("f2f3")
            validation_game = game_from_line(
                after_f3.fen(en_passant="fen"),
                ["e7e5", "g2g4", "d8h4"],
                result=0,
            )
            validation_shard = write_shard(
                root,
                "validation.vf",
                [validation_game],
                self.auditor,
                partition="validation",
            )
            validation = root / "validation-corpus.vf"

            with self.assertRaisesRegex(self.assembler.AssemblyError, "cross-partition"):
                self.assembler.assemble_corpus(
                    make_config(
                        self.assembler,
                        root,
                        (validation_shard,),
                        validation,
                        partition="validation",
                        disjoint_from=(train,),
                    )
                )

            self.assertFalse(validation.exists())

    def test_opt_in_quarantine_drops_the_complete_conflicting_game(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            train_game = game_from_line(
                chess.STARTING_FEN, ["f2f3", "e7e5", "g2g4", "d8h4"], result=0
            )
            train_shard = write_shard(root, "train.vf", [train_game], self.auditor)
            train = root / "train-corpus.vf"
            self.assembler.assemble_corpus(
                make_config(self.assembler, root, (train_shard,), train)
            )
            after_f3 = chess.Board()
            after_f3.push_uci("f2f3")
            conflicting_game = game_from_line(
                after_f3.fen(en_passant="fen"),
                ["e7e5", "g2g4", "d8h4"],
                result=0,
            )
            clean_game = mate_in_one(
                "7k/5K2/6Q1/8/8/8/8/8 w - - 0 1", "g6g7"
            )
            validation_shard = write_shard(
                root,
                "validation.vf",
                [conflicting_game, clean_game],
                self.auditor,
                partition="validation",
            )
            validation = root / "validation-corpus.vf"

            manifest = self.assembler.assemble_corpus(
                make_config(
                    self.assembler,
                    root,
                    (validation_shard,),
                    validation,
                    partition="validation",
                    disjoint_from=(train,),
                    quarantine_cross_partition_games=True,
                )
            )

            self.assertEqual(manifest["artifact"]["games"], 1)
            self.assertEqual(manifest["artifact"]["scored_positions"], 1)
            self.assertEqual(manifest["separation"]["quarantined_games"], 1)
            self.assertEqual(manifest["separation"]["quarantined_scored_positions"], 3)
            self.assertEqual(manifest["separation"]["detected_conflicting_position_keys"], 3)
            self.assertEqual(manifest["separation"]["cross_partition_position_keys"], 0)

    def test_existing_destination_is_never_overwritten(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            shard = write_shard(
                root,
                "train.vf",
                [mate_in_one("7k/5K2/6Q1/8/8/8/8/8 w - - 0 1", "g6g7")],
                self.auditor,
            )
            output = root / "occupied.vf"
            output.write_bytes(b"preserve")

            with self.assertRaisesRegex(self.assembler.AssemblyError, "overwrite"):
                self.assembler.assemble_corpus(
                    make_config(self.assembler, root, (shard,), output)
                )

            self.assertEqual(output.read_bytes(), b"preserve")


def make_config(assembler, root: Path, inputs: tuple[Path, ...], output: Path, **overrides):
    values = {
        "repo_root": root,
        "inputs": inputs,
        "output": output,
        "partition": "train",
        "shuffle_seed": "shuffle-seed",
        "minimum_games": 1,
        "minimum_scored_positions": 1,
        "disjoint_from": (),
        "quarantine_cross_partition_games": False,
        "quarantine_duplicate_opening_games": False,
    }
    values.update(overrides)
    return assembler.CorpusConfig(**values)


def write_shard(
    root: Path,
    name: str,
    games: list[bytes],
    auditor,
    *,
    partition: str = "train",
) -> Path:
    path = root / name
    data = b"".join(games)
    path.write_bytes(data)
    audit = auditor.audit_file(path, require_completed_games=True)
    manifest = {
        "schema": "neyrang-nnue-selfplay-shard-v1",
        "contract": "neyrang-viriformat-strict-v1",
        "artifact": {
            "path": path.relative_to(root).as_posix(),
            "format": "contiguous replay-validated Viriformat games",
            "bytes": len(data),
            "sha256": hashlib.sha256(data).hexdigest(),
            "games": audit["games"],
            "scored_positions": audit["scored_positions"],
        },
        "generator": {
            "binary_sha256": "1" * 64,
            "source_commit": "2" * 40,
            "threads": 1,
            "hash_megabytes": 1,
            "nodes_per_move": 512,
            "wall_time_limit": None,
            "network": None,
            "engine_role": "NEYRANG self-play generator and score teacher",
        },
        "opening_source": {
            "artifact": {"sha256": hashlib.sha256(name.encode()).hexdigest()},
            "license": "UNLICENSED-NEYRANG-INTERNAL",
        },
        "split": {
            "schema": "neyrang-nnue-selfplay-split-v1",
            "seed": "split-seed",
            "train_percent": 80,
            "validation_percent": 10,
            "holdout_percent": 10,
            "partition": partition,
            "membership_sha256": hashlib.sha256(f"membership:{name}".encode()).hexdigest(),
        },
        "scoring": {
            "point_of_view": "White",
            "parent_position": True,
            "header_score": 0,
            "mate_policy": "preserve",
            "saturation": "i16",
        },
        "adjudication": {
            "policy": "rules-only-v1",
            "maximum_plies": 512,
            "maximum_plies_policy": "reject",
            "tablebases": None,
            "evaluation_adjudication": None,
        },
        "filter": {
            "version": "rules-complete-v1",
            "position_sampling": None,
            "score_filter": None,
        },
        "audit": audit,
        "source": {
            "kind": "NEYRANG deterministic self-play",
            "license": "UNLICENSED-NEYRANG-INTERNAL",
            "external_mixing_ratio": 0,
        },
    }
    Path(f"{path}.manifest.json").write_text(json.dumps(manifest), encoding="utf-8")
    return path


def mate_in_one(fen: str, move_uci: str) -> bytes:
    return game_from_line(fen, [move_uci], result=2)


def game_from_line(fen: str, move_uci: list[str], result: int) -> bytes:
    board = chess.Board(fen)
    header = encode_header(board, result=2)
    records = bytearray()
    for item in move_uci:
        move = chess.Move.from_uci(item)
        assert move in board.legal_moves
        encoded = move.from_square | (move.to_square << 6)
        records.extend(encoded.to_bytes(2, "little"))
        records.extend((123).to_bytes(2, "little", signed=True))
        board.push(move)
    assert board.is_checkmate()
    header = encode_header(chess.Board(fen), result=result)
    return header + bytes(records) + b"\0\0\0\0"


def encode_header(board: chess.Board, result: int) -> bytes:
    occupancy = int(board.occupied)
    pieces = bytearray(16)
    codes = {
        chess.PAWN: 0,
        chess.KNIGHT: 1,
        chess.BISHOP: 2,
        chess.ROOK: 3,
        chess.QUEEN: 4,
        chess.KING: 5,
    }
    for index, square in enumerate(chess.scan_forward(occupancy)):
        piece = board.piece_at(square)
        assert piece is not None
        code = codes[piece.piece_type] | (0 if piece.color else 8)
        if index % 2 == 0:
            pieces[index // 2] = code
        else:
            pieces[index // 2] |= code << 4
    state = (0 if board.turn else 0x80) | (board.ep_square if board.ep_square is not None else 64)
    return b"".join(
        [
            occupancy.to_bytes(8, "little"),
            bytes(pieces),
            bytes([state, board.halfmove_clock]),
            board.fullmove_number.to_bytes(2, "little"),
            (0).to_bytes(2, "little", signed=True),
            bytes([result, 0]),
        ]
    )


if __name__ == "__main__":
    unittest.main()
