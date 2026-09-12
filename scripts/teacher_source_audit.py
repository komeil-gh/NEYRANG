"""Independent streaming raw-UCI audit for registered teacher game shards."""

import argparse
import hashlib
import json
from pathlib import Path

import chess
import chess.pgn

from .teacher_export import checked_game, json_lines, require

SETTINGS = {"Threads": "1", "Hash": "64", "MultiPV": "1", "Ponder": "false",
            "UCI_ShowWDL": "true", "UCI_LimitStrength": "false", "Skill Level": "20",
            "SyzygyPath": "", "SyzygyProbeLimit": "0"}


def sha256(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def searches(stream, state):
    """Parse raw lines independently; never consume python-chess merged info."""
    options, command, selected, nodes, running = {}, None, None, 0, False
    for line in stream:
        require(len(line) <= 1_048_576, "oversize protocol line")
        if "Connection lost (" in line:
            require("Connection lost (exit code: 0, error: None)" in line, "teacher exit failure")
            state["closed"] += 1
        if " >> " in line:
            event = line.split(" >> ", 1)[1].strip()
            if event.startswith("id name "):
                require(event == "id name Stockfish 18", "teacher identity mismatch")
                state["identity"] += 1
            elif event.startswith("option name "):
                name, _, definition = event[12:].partition(" type ")
                if name in SETTINGS:
                    require(" default" in definition, "missing option default")
                    value = definition.split(" default", 1)[1].strip().split(" min ", 1)[0]
                    options[name] = "" if value == "<empty>" else value
            elif event.startswith("info ") and not event.startswith("info string "):
                require(running, "score outside search")
                tokens = event[5:].split()
                if "nodes" in tokens:
                    nodes = max(nodes, int(tokens[tokens.index("nodes") + 1]))
                fields = ("score", "wdl", "pv", "depth", "nodes", "tbhits")
                if all(k in tokens for k in fields) and not {"upperbound", "lowerbound"} & set(tokens):
                    if "multipv" in tokens and tokens[tokens.index("multipv") + 1] != "1":
                        continue
                    if int(tokens[tokens.index("depth") + 1]) > 0 and int(tokens[tokens.index("nodes") + 1]) > 0:
                        selected = tokens
            elif event.startswith("bestmove "):
                require(running and selected is not None, "missing exact frame/search")
                yield command, selected, nodes, event.split()[1], state["games"], state["clears"]
                command, selected, nodes, running = None, None, 0, False
        elif " << " in line:
            event = line.split(" << ", 1)[1].strip()
            if event.startswith("setoption name "):
                require(not running, "configuration during search")
                name, separator, value = event[15:].partition(" value")
                value = value.strip()
                if name == "Clear Hash":
                    require(not separator, "invalid clear hash")
                    state["clears"] += 1
                elif name == "UCI_Chess960":
                    require(value == "false", "Chess960 not registered")
                else:
                    require(name in SETTINGS and separator, "unregistered option write")
                    options[name] = value
            elif event == "ucinewgame":
                require(not running, "new game during search")
                state["games"] += 1
            elif event.startswith("position "):
                require(not running and command is None and event.startswith("position fen "), "invalid position command")
                command = event[13:]
            elif event.startswith("go "):
                require(not running and command is not None and event == "go nodes 100000", "wrong search budget/order")
                require(options == SETTINGS and state["identity"] == 1, "effective teacher settings mismatch")
                running = True
            elif event == "quit":
                require(not running and command is None, "unfinished search on quit")
                state["quit"] += 1
    require(not running and command is None, "incomplete final search")
    require(state["closed"] == state["quit"] == state["identity"] == 1, "missing clean teacher lifecycle")


def audit_shard(root, groups_sha256, teacher_sha256, partition="train", max_plies=256):
    root = Path(root)
    require(partition in ("train", "validation", "holdout") and type(max_plies) is int
            and max_plies > 0, "invalid audit contract")
    require(sha256(root / "groups.json") == groups_sha256, "frozen group hash mismatch")
    require(len(teacher_sha256) == 64 and all(c in "0123456789abcdef" for c in teacher_sha256), "bad teacher hash")
    names = ("groups.json", "games.jsonl", "labels.jsonl", "protocol.log", "games.pgn",
             "result.json", "exit.txt", "runner.stderr")
    hashes = {name: sha256(root / name) for name in names}
    groups = json.loads((root / "groups.json").read_text())
    result = json.loads((root / "result.json").read_text())
    require(isinstance(groups, list) and 0 < len(groups) <= 256, "invalid shard group count")
    require(len({g["group"] for g in groups}) == len(groups), "duplicate group")
    require(type(result.get("native_exit")) is int and result["native_exit"] == 0
            and result.get("teacher_sha256") == teacher_sha256, "native exit/identity missing")
    require((root / "exit.txt").read_text().strip() == "0"
            and not (root / "runner.stderr").read_bytes(), "wrapper failure/stderr")
    state = dict.fromkeys(("closed", "quit", "identity", "games", "clears"), 0)
    counts = dict.fromkeys(("positions", "final_nodes", "score_frame_nodes", "completed_games", "earlier_frames"), 0)
    with (root / "protocol.log").open() as protocol, (root / "labels.jsonl").open("rb") as labels, \
            (root / "games.jsonl").open("rb") as summaries, (root / "games.pgn").open() as pgn:
        raw, rows, games = searches(protocol, state), json_lines(labels), json_lines(summaries)
        for index, source in enumerate(groups, 1):
            game = next(games, None)
            require(isinstance(game, dict) and type(game.get("plies")) is int
                    and 0 < game["plies"] <= max_plies, "missing/corrupt game")
            group_rows = [next(rows, None) for _ in range(game["plies"])]
            checked_game(source, game, group_rows, max_plies, 110000)
            history = []
            for row in group_rows:
                event = next(raw, None)
                require(event is not None, "missing raw search")
                command, tokens, nodes, best, game_index, clears = event
                expected = source["fen"] + (" moves " + " ".join(history) if history else "")
                require(command == expected and game_index == clears == index, "raw root/history/game mismatch")
                kind, value = tokens[tokens.index("score") + 1:tokens.index("score") + 3]
                require(kind in ("cp", "mate"), "unknown score type")
                require(row["score_stm"] == {"cp": None, "mate": None, kind: int(value)}, "raw score mismatch")
                wdl_index = tokens.index("wdl")
                require(row["wdl_stm"] == list(map(int, tokens[wdl_index + 1:wdl_index + 4])), "raw WDL mismatch")
                require(row["pv"] == tokens[tokens.index("pv") + 1:] and row["final_bestmove"] == best, "raw moves mismatch")
                for field, token in (("depth", "depth"), ("score_frame_nodes", "nodes"), ("tbhits", "tbhits")):
                    require(row[field] == int(tokens[tokens.index(token) + 1]), "raw telemetry mismatch")
                require(row["final_nodes"] == nodes, "final work mismatch")
                counts["positions"] += 1
                for key in ("final_nodes", "score_frame_nodes"):
                    counts[key] += row[key]
                counts["earlier_frames"] += row["score_frame_nodes"] < nodes
                history.append(best)
            document = chess.pgn.read_game(pgn)
            require(document is not None and not document.errors, "missing/invalid PGN")
            require(document.headers.get("Group") == source["group"] and document.headers.get("Round") == str(index)
                    and document.headers.get("Result") == (game["result"] or "*")
                    and document.headers.get("Termination") == game["reason"], "PGN headers mismatch")
            require(document.board().fen(en_passant="fen") == source["fen"]
                    and [m.uci() for m in document.mainline_moves()] == history, "PGN trajectory mismatch")
            counts["completed_games"] += game["result"] is not None
        eof = object()
        require(next(raw, eof) is eof and next(rows, eof) is eof and next(games, eof) is eof
                and chess.pgn.read_game(pgn) is None, "extra raw/game/label/PGN record")
    require(state["games"] == state["clears"] == len(groups), "lifecycle count mismatch")
    require(type(result.get("attempts")) is int and result["attempts"] == len(groups), "attempt count mismatch")
    for key in ("positions", "final_nodes", "score_frame_nodes", "completed_games"):
        require(type(result.get(key)) is int and result[key] == counts[key], "summary count mismatch")
    require(hashes == {name: sha256(root / name) for name in names}, "source changed during audit")
    return {"schema": "neyrang-teacher-source-audit-v1", "passed": True, "partition": partition,
            "max_plies": max_plies, "max_nodes": 110000, "teacher_sha256": teacher_sha256,
            "sha256": {name: hashes[name] for name in names[:4]}, "supporting_sha256": hashes,
            "auditor_sha256": sha256(__file__), "groups": len(groups), **counts}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input-dir", required=True, type=Path)
    parser.add_argument("--groups-sha256", required=True)
    parser.add_argument("--teacher-sha256", required=True)
    parser.add_argument("--partition", required=True, choices=("train", "validation", "holdout"))
    args = parser.parse_args()
    destination = args.input_dir / "source-audit.json"
    require(not destination.exists(), "refusing to overwrite source audit")
    report = audit_shard(args.input_dir, args.groups_sha256, args.teacher_sha256, args.partition)
    with destination.open("x") as stream:
        json.dump(report, stream, indent=2, sort_keys=True)
        stream.write("\n")
    print(json.dumps({"passed": True, "groups": report["groups"], "positions": report["positions"]}))


if __name__ == "__main__":
    main()
