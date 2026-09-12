import copy
import hashlib
import json
import tempfile
import unittest
from pathlib import Path

import chess

from scripts.teacher_export import checked_game, export_shard
from scripts.teacher_target import ENCODING


def fixture(moves="f2f3 e7e5 g2g4 d8h4", fen=chess.STARTING_FEN):
    board = chess.Board(fen)
    keys = [" ".join(b.fen(en_passant="fen").split()[:4]) for b in (board, board.mirror())]
    group = hashlib.sha256(min(keys).encode()).hexdigest()
    source = {"fen": fen, "group": group}
    rows = []
    for ply, move in enumerate(moves.split()):
        final = chess.Move.from_uci(move)
        assert final in board.legal_moves
        flags = {"mate": False, "special_or_high_cp": False, "in_check": board.is_check(),
                 "few_pieces": len(board.piece_map()) < 4,
                 "tactical_pv": board.is_capture(final) or bool(final.promotion),
                 "tactical_played": board.is_capture(final) or bool(final.promotion)}
        rows.append({"group": group, "ply": ply, "fen": board.fen(en_passant="fen"),
                     "score_stm": {"cp": 80 if board.turn else -80, "mate": None},
                     "score_white": {"cp": 80, "mate": None},
                     "wdl_stm": [700, 200, 100] if board.turn else [100, 200, 700],
                     "wdl_white": [700, 200, 100], "expected_score_white": 0.8,
                     "pv": [move], "final_bestmove": move, "depth": 10,
                     "score_frame_nodes": 900, "final_nodes": 1000, "tbhits": 0,
                     "eligibility_flags": flags, "finite_quiet_candidate": not any(flags.values())})
        board.push(final)
    ending = board.outcome(claim_draw=False)
    if ending:
        result, reason = ending.result(), ending.termination.name.lower()
    elif board.is_fifty_moves():
        result, reason = "1/2-1/2", "fifty-move-claim"
    elif board.is_repetition(3):
        result, reason = "1/2-1/2", "threefold-claim"
    else:
        result, reason = None, "maximum-plies"
    game = {**source, "plies": len(rows), "final_fen": board.fen(en_passant="fen"),
            "result": result, "reason": reason,
            "final_nodes": sum(r["final_nodes"] for r in rows),
            "score_frame_nodes": sum(r["score_frame_nodes"] for r in rows),
            "finite_quiet_candidates": sum(r["finite_quiet_candidate"] for r in rows)}
    return source, game, rows


class TeacherExportTest(unittest.TestCase):
    def test_complete_game_keeps_white_target_distinct_from_black_win(self):
        source, game, rows = fixture()
        out = checked_game(source, game, rows, 4, 1100)
        self.assertEqual(out, [f'{r["fen"]} | 555 | 0.0' for r in rows])

    def test_current_draw_claims_and_unfinished_caps(self):
        for moves, fen in [
            ("g1f3 g8f6 f3g1 f6g8 g1f3 g8f6 f3g1 f6g8", chess.STARTING_FEN),
            ("b1c3", chess.STARTING_FEN.replace(" 0 1", " 99 1")),
        ]:
            source, game, rows = fixture(moves, fen)
            self.assertEqual(len(checked_game(source, game, rows, len(rows), 1100)), len(rows))
        source, game, rows = fixture("g1f3")
        self.assertEqual(checked_game(source, game, rows, 1, 1100), [])
        game["result"] = "1/2-1/2"
        with self.assertRaises(ValueError):
            checked_game(source, game, rows, 1, 1100)

    def test_both_pv_and_played_capture_and_typed_scores_control_eligibility(self):
        source, game, rows = fixture("h1g1 a8b8 g1h1 b8a8 h1g1 a8b8 g1h1 b8a8",
                                     "k7/8/8/3p4/4P3/8/8/7K w - - 0 1")
        rows[0]["pv"] = ["e4d5"]
        rows[0]["eligibility_flags"]["tactical_pv"] = True
        rows[0]["finite_quiet_candidate"] = False
        game["finite_quiet_candidates"] -= 1
        self.assertEqual(len(checked_game(source, game, rows, 8, 1100)), 7)
        source, game, rows = fixture("e4d5 a8b8 h1g1 b8a8 g1h1 a8b8 h1g1 b8a8 g1h1",
                                     "k7/8/8/3p4/4P3/8/8/7K w - - 0 1")
        # This trajectory reaches a current threefold claim with non-pawn material absent.
        # Its quiet rows have fewer than four pieces after the first capture, so none qualify.
        self.assertEqual(checked_game(source, game, rows, len(rows), 1100), [])
        source, game, rows = fixture()
        for kind, value, flag in [("mate", 3, "mate"), ("cp", 10000, "special_or_high_cp")]:
            changed = copy.deepcopy(rows)
            changed[0]["score_white"] = changed[0]["score_stm"] = {"cp": None, "mate": None, kind: value}
            changed[0]["eligibility_flags"][flag] = True
            changed[0]["finite_quiet_candidate"] = False
            summary = {**game, "finite_quiet_candidates": 3}
            self.assertEqual(len(checked_game(source, summary, changed, 4, 1100)), 3)

    def test_corrupt_history_scores_flags_and_results_fail_closed(self):
        source, game, rows = fixture()
        for key, value in [("ply", 1), ("group", "wrong"), ("fen", chess.STARTING_FEN.replace(" w ", " b ")),
                           ("final_bestmove", "e2e5"), ("pv", ["e2e5"]), ("tbhits", 1),
                           ("final_nodes", 1101), ("score_frame_nodes", 1001), ("depth", 0),
                           ("wdl_white", [True, 999, 0]), ("score_stm", {"cp": 80, "mate": 1}),
                           ("expected_score_white", float("nan")), ("finite_quiet_candidate", False)]:
            changed = copy.deepcopy(rows)
            changed[0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                checked_game(source, game, changed, 4, 1100)
        with self.assertRaises(ValueError):
            checked_game(source, {**game, "result": "1-0"}, rows, 4, 1100)

    def test_shard_export_binds_sources_keeps_sidecar_and_refuses_reuse(self):
        source, game, rows = fixture()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            inputs = root / "input"
            inputs.mkdir()
            payloads = {"groups.json": json.dumps([source]), "games.jsonl": json.dumps(game) + "\n",
                        "labels.jsonl": "".join(json.dumps(r) + "\n" for r in rows),
                        "protocol.log": "synthetic fixture; not live teacher evidence\n"}
            for name, contents in payloads.items():
                (inputs / name).write_text(contents)
            audit = {"schema": "neyrang-teacher-source-audit-v1", "passed": True, "partition": "train",
                     "max_plies": 4, "max_nodes": 1100, "teacher_sha256": "0" * 64,
                     "sha256": {name: hashlib.sha256(contents.encode()).hexdigest() for name, contents in payloads.items()}}
            (inputs / "source-audit.json").write_text(json.dumps(audit))
            output = root / "export"
            report = export_shard(inputs, output, "train", 4, 1100)
            self.assertEqual(report["positions"], 4)
            self.assertEqual((output / "train.txt").read_text().splitlines()[0], "# " + ENCODING)
            provenance = json.loads((output / "provenance.jsonl").read_text())
            self.assertEqual(provenance["source"], source)
            self.assertEqual(provenance["rows"], rows)
            self.assertEqual(provenance["game"], game)
            self.assertEqual(report, json.loads((output / "manifest.json").read_text()))
            with self.assertRaises(FileExistsError):
                export_shard(inputs, output, "train", 4, 1100)
            with self.assertRaises(ValueError):
                export_shard(inputs, root / "wrong-partition", "validation", 4, 1100)
            (inputs / "labels.jsonl").write_text(payloads["labels.jsonl"] + "{}\n")
            with self.assertRaises(ValueError):
                export_shard(inputs, root / "mutated", "train", 4, 1100)
            # Literal JSON null is data, not EOF, even when the attestation hash matches.
            (inputs / "labels.jsonl").write_text(payloads["labels.jsonl"] + "null\n")
            audit["sha256"]["labels.jsonl"] = hashlib.sha256((inputs / "labels.jsonl").read_bytes()).hexdigest()
            (inputs / "source-audit.json").write_text(json.dumps(audit))
            with self.assertRaises(ValueError):
                export_shard(inputs, root / "null-tail", "train", 4, 1100)

    def test_boolean_expectation_is_not_a_numeric_target(self):
        source, game, rows = fixture()
        rows[0].update(wdl_white=[1000, 0, 0], wdl_stm=[1000, 0, 0], expected_score_white=True)
        with self.assertRaises(ValueError):
            checked_game(source, game, rows, 4, 1100)


if __name__ == "__main__":
    unittest.main()
