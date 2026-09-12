import hashlib
import json
import tempfile
import unittest
from pathlib import Path

import chess
import chess.pgn

from scripts.teacher_source_audit import audit_shard
from scripts.tests.test_teacher_export import fixture


def make_shard(root):
    source, game, rows = fixture()
    (root / "groups.json").write_text(json.dumps([source]))
    (root / "games.jsonl").write_text(json.dumps(game) + "\n")
    (root / "labels.jsonl").write_text("".join(json.dumps(r) + "\n" for r in rows))
    board = chess.Board(source["fen"])
    lines = [" >> id name Stockfish 18", " << setoption name Threads value 1",
             " << setoption name Hash value 64", " << setoption name MultiPV value 1",
             " << setoption name Ponder value false", " << setoption name UCI_ShowWDL value true",
             " << setoption name UCI_LimitStrength value false", " << setoption name Skill Level value 20",
             " << setoption name SyzygyPath value", " << setoption name SyzygyProbeLimit value 0",
             " << setoption name Clear Hash", " << ucinewgame"]
    for row in rows:
        command = source["fen"] + (" moves " + " ".join(m.uci() for m in board.move_stack) if board.move_stack else "")
        score = row["score_stm"]["cp"]
        wdl = " ".join(map(str, row["wdl_stm"]))
        move = row["final_bestmove"]
        lines += [" << position fen " + command, " << go nodes 100000",
                  f" >> info depth 10 score cp {score} wdl {wdl} nodes 900 tbhits 0 pv {move}",
                  f" >> info depth 11 score cp 999 upperbound wdl 999 1 0 nodes 1000 tbhits 0 pv {move}",
                  f" >> bestmove {move}"]
        board.push_uci(move)
    lines += [" << quit", "Connection lost (exit code: 0, error: None)"]
    (root / "protocol.log").write_text("\n".join(lines) + "\n")
    document = chess.pgn.Game.from_board(board)
    document.headers.update(Group=source["group"], Result=game["result"], Round="1", Termination=game["reason"])
    (root / "games.pgn").write_text(str(document) + "\n")
    (root / "exit.txt").write_text("0\n")
    (root / "runner.stderr").write_text("")
    (root / "result.json").write_text(json.dumps({"native_exit": 0, "teacher_sha256": "0" * 64,
        "attempts": 1, "positions": 4, "completed_games": 1, "final_nodes": 4000, "score_frame_nodes": 3600}))
    return hashlib.sha256((root / "groups.json").read_bytes()).hexdigest()


class TeacherSourceAuditTest(unittest.TestCase):
    def test_advertised_defaults_plus_effective_overrides(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            digest = make_shard(root)
            path = root / "protocol.log"
            defaults = {"Threads": "1", "Hash": "16", "MultiPV": "1", "Ponder": "false",
                        "UCI_ShowWDL": "false", "UCI_LimitStrength": "false", "Skill Level": "20",
                        "SyzygyPath": "<empty>", "SyzygyProbeLimit": "7"}
            lines = []
            for name, value in defaults.items():
                kind = "check" if value in ("true", "false") else "string" if name == "SyzygyPath" else "spin"
                lines.append(f" >> option name {name} type {kind} default {value}" + (" min 0 max 1024" if kind == "spin" else ""))
            for line in path.read_text().splitlines():
                if "setoption name " in line and any("name " + n + " value" in line for n in
                    ("Threads", "MultiPV", "Ponder", "UCI_LimitStrength", "Skill Level", "SyzygyPath")):
                    continue
                lines.append(line)
            path.write_text("\n".join(lines) + "\n")
            self.assertTrue(audit_shard(root, digest, "0" * 64)["passed"])

    def test_coherent_earlier_frame_complete_history_and_real_outcome(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            digest = make_shard(root)
            report = audit_shard(root, digest, "0" * 64)
            self.assertIsNotNone(report)
            self.assertTrue(report["passed"])
            self.assertEqual(report["positions"], 4)
            self.assertEqual(report["earlier_frames"], 4)
            self.assertEqual(set(report["sha256"]), {"groups.json", "games.jsonl", "labels.jsonl", "protocol.log"})

    def test_raw_history_config_bound_exit_and_extra_records_fail(self):
        changes = [
            ("protocol.log", "Hash value 64", "Hash value 32"),
            ("protocol.log", " moves f2f3 e7e5", " moves f2f3 e7e6"),
            ("protocol.log", "nodes 900 tbhits", "upperbound nodes 900 tbhits"),
            ("protocol.log", "exit code: 0", "exit code: 7"),
            ("protocol.log", "go nodes 100000", "go nodes 200000"),
            ("protocol.log", "score cp 80", "score cp 81"),
            ("protocol.log", " << quit", " << go nodes 100000\n << quit"),
            ("exit.txt", "0", "1"),
            ("result.json", '"native_exit": 0', '"native_exit": null'),
            ("games.pgn", '[Result "0-1"]', '[Result "1-0"]'),
        ]
        for filename, before, after in changes:
            with self.subTest(filename=filename, before=before), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                digest = make_shard(root)
                path = root / filename
                path.write_text(path.read_text().replace(before, after, 1))
                with self.assertRaises(ValueError):
                    audit_shard(root, digest, "0" * 64)


if __name__ == "__main__":
    unittest.main()
