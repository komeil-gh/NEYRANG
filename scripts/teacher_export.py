"""Export independently source-audited teacher game shards into text v1."""

import argparse
import hashlib
import importlib.util
import json
import sys
from pathlib import Path

import chess

from .teacher_target import ENCODING, wdl_score

_GROUPING_PATH = Path(__file__).with_name("generate-selfplay-shard.py")
_spec = importlib.util.spec_from_file_location("neyrang_teacher_grouping", _GROUPING_PATH)
_grouping = importlib.util.module_from_spec(_spec)
sys.modules[_spec.name] = _grouping
_spec.loader.exec_module(_grouping)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def ending(board):
    outcome = board.outcome(claim_draw=False)
    if outcome is not None:
        return outcome.result(), outcome.termination.name.lower()
    if board.is_fifty_moves():
        return "1/2-1/2", "fifty-move-claim"
    if board.is_repetition(3):
        return "1/2-1/2", "threefold-claim"
    return None, "maximum-plies"


def checked_game(source, game, rows, max_plies, max_nodes):
    """Replay all rows, then retain eligible rows only from a real completed game.

    Raw UCI score-frame/configuration verification belongs to the separate
    source auditor. This checks typed-record consistency, not teacher truth.
    """
    require(isinstance(source, dict) and isinstance(game, dict), "invalid source/game record")
    require(type(max_plies) is int and max_plies > 0 and type(max_nodes) is int and max_nodes > 0,
            "positive integer budgets required")
    board = chess.Board(source.get("fen", ""))
    require(board.is_valid() and source["fen"] == board.fen(en_passant="fen"), "invalid root FEN")
    group = hashlib.sha256(_grouping.opening_group_key(source["fen"]).encode()).hexdigest()
    require(source.get("group") == group and game.get("group") == group
            and game.get("fen") == source["fen"], "opening group/root mismatch")
    require(isinstance(rows, list) and type(game.get("plies")) is int
            and 0 < len(rows) == game["plies"] <= max_plies, "game row count/cap mismatch")
    eligible = []
    for ply, row in enumerate(rows):
        require(ending(board)[0] is None, "mainline continues beyond terminal/current claim")
        require(isinstance(row, dict) and row.get("group") == group
                and type(row.get("ply")) is int and row["ply"] == ply
                and row.get("fen") == board.fen(en_passant="fen"), "row history mismatch")
        stm, white = row.get("score_stm"), row.get("score_white")
        require(isinstance(stm, dict) and set(stm) == {"cp", "mate"}, "invalid typed STM score")
        require(sum(type(v) is int for v in stm.values()) == 1
                and sum(v is None for v in stm.values()) == 1, "score must be finite cp OR typed mate")
        require(isinstance(white, dict) and set(white) == {"cp", "mate"}
                and all(type(white[k]) is type(v) and white[k] ==
                        (v if board.turn or v is None else -v) for k, v in stm.items()),
                "White score orientation mismatch")
        score = wdl_score(row.get("wdl_white"))
        wdl_score(row.get("wdl_stm"))
        require(row["wdl_white"] == (row["wdl_stm"] if board.turn else row["wdl_stm"][::-1]),
                "WDL orientation mismatch")
        wdl = row["wdl_white"]
        require(type(row.get("expected_score_white")) in (int, float)
                and row["expected_score_white"] == (wdl[0] + wdl[1] / 2) / 1000,
                "WDL expectation mismatch")
        require(all(type(row.get(k)) is int for k in ("depth", "score_frame_nodes", "final_nodes", "tbhits"))
                and row["depth"] > 0 and 0 < row["score_frame_nodes"] <= row["final_nodes"] <= max_nodes
                and row["tbhits"] == 0 and not row.get("lowerbound") and not row.get("upperbound"),
                "invalid exact score telemetry/budget")
        pv = row.get("pv")
        require(isinstance(pv, list) and pv and all(isinstance(m, str) for m in pv)
                and isinstance(row.get("final_bestmove"), str), "missing PV/final move")
        replay = board.copy()
        for notation in pv:
            move = chess.Move.from_uci(notation)
            require(move in replay.legal_moves, "illegal PV move")
            replay.push(move)
        first, final = chess.Move.from_uci(pv[0]), chess.Move.from_uci(row["final_bestmove"])
        require(final in board.legal_moves, "illegal played move")
        flags = {"mate": stm["mate"] is not None,
                 "special_or_high_cp": stm["cp"] is not None and abs(stm["cp"]) >= 10000,
                 "in_check": board.is_check(), "few_pieces": len(board.piece_map()) < 4,
                 "tactical_pv": board.is_capture(first) or bool(first.promotion),
                 "tactical_played": board.is_capture(final) or bool(final.promotion)}
        stored_flags = row.get("eligibility_flags")
        require(isinstance(stored_flags, dict) and all(type(v) is bool for v in stored_flags.values())
                and stored_flags == flags and type(row.get("finite_quiet_candidate")) is bool
                and row["finite_quiet_candidate"] == (not any(flags.values())), "eligibility mismatch")
        if not any(flags.values()):
            eligible.append((row["fen"], score))
        board.push(final)
    result, reason = ending(board)
    require(result is not None or len(rows) == max_plies, "unfinished game before registered cap")
    require(game.get("result") == result and game.get("reason") == reason
            and game.get("final_fen") == board.fen(en_passant="fen"), "game outcome/final FEN mismatch")
    for key, row_key in (("final_nodes", "final_nodes"), ("score_frame_nodes", "score_frame_nodes"),
                         ("finite_quiet_candidates", "finite_quiet_candidate")):
        require(type(game.get(key)) is int and game[key] == sum(row[row_key] for row in rows),
                f"game {key} mismatch")
    if result is None:
        return []
    actual = {"1-0": "1.0", "0-1": "0.0", "1/2-1/2": "0.5"}[result]
    return [f"{fen} | {score} | {actual}" for fen, score in eligible]


def json_lines(stream):
    while line := stream.readline(1_048_577):
        require(len(line) <= 1_048_576, "JSONL record exceeds 1 MiB")
        yield json.loads(line)


def export_shard(input_dir, output_dir, partition, max_plies, max_nodes):
    input_dir, output_dir = Path(input_dir), Path(output_dir)
    if output_dir.exists():
        raise FileExistsError(output_dir)
    require(partition in ("train", "validation", "holdout"), "invalid partition")
    require(type(max_plies) is int and max_plies > 0 and type(max_nodes) is int and max_nodes > 0,
            "positive integer budgets required")
    audit = json.loads((input_dir / "source-audit.json").read_text())
    require(audit.get("schema") == "neyrang-teacher-source-audit-v1" and audit.get("passed") is True
            and audit.get("partition") == partition and audit.get("max_plies") == max_plies
            and audit.get("max_nodes") == max_nodes, "source audit/partition/budget mismatch")
    teacher_hash = audit.get("teacher_sha256", "")
    require(isinstance(teacher_hash, str) and len(teacher_hash) == 64
            and all(c in "0123456789abcdef" for c in teacher_hash), "missing teacher identity")
    names = ("groups.json", "games.jsonl", "labels.jsonl", "protocol.log")
    hashes = {name: _grouping.sha256_file(input_dir / name)[0] for name in (*names, "source-audit.json")}
    require(audit.get("sha256") == {name: hashes[name] for name in names}, "source hash mismatch")
    sources = json.loads((input_dir / "groups.json").read_text())
    require(isinstance(sources, list) and sources
            and all(isinstance(s, dict) and isinstance(s.get("group"), str) for s in sources),
            "empty/invalid opening groups")
    require(len({s["group"] for s in sources}) == len(sources), "duplicate opening groups")
    output_dir.mkdir()
    count = completed = 0
    # A failed export retains its partial files, but never publishes manifest.json.
    with (input_dir / "games.jsonl").open("rb") as summaries, (input_dir / "labels.jsonl").open("rb") as labels, \
            (output_dir / f"{partition}.txt").open("x") as text, \
            (output_dir / "provenance.jsonl").open("x") as provenance:
        games, records = json_lines(summaries), json_lines(labels)
        text.write("# " + ENCODING + "\n")
        for source in sources:
            game = next(games, None)
            require(isinstance(game, dict) and type(game.get("plies")) is int
                    and 0 < game["plies"] <= max_plies, "missing/invalid game summary")
            rows = [next(records, None) for _ in range(game["plies"])]
            converted = checked_game(source, game, rows, max_plies, max_nodes)
            for line in converted:
                text.write(line + "\n")
            provenance.write(json.dumps({"source": source, "game": game, "rows": rows,
                                         "partition": partition, "exported_rows": len(converted)},
                                        sort_keys=True, allow_nan=False) + "\n")
            count += len(converted)
            completed += game["result"] is not None
        eof = object()
        require(next(games, eof) is eof and next(records, eof) is eof, "extra game/label records")
    require(count > 0, "no eligible completed-game rows; export has no manifest")
    require(hashes == {name: _grouping.sha256_file(input_dir / name)[0] for name in hashes},
            "source changed during export")
    report = {"schema": "neyrang-teacher-export-v1", "encoding": ENCODING, "partition": partition,
              "positions": count, "games": len(sources), "completed_games": completed,
              "max_plies": max_plies, "max_nodes": max_nodes,
              "teacher_sha256": teacher_hash, "source_sha256": hashes,
              "tool_sha256": {p.name: _grouping.sha256_file(p)[0] for p in
                              (Path(__file__), Path(__file__).with_name("teacher_target.py"), _GROUPING_PATH)},
              "sha256": {name: _grouping.sha256_file(output_dir / name)[0] for name in
                         (f"{partition}.txt", "provenance.jsonl")},
              "boundary": "Source audit is a required external attestation, not re-executed here. "
                          "Replay is from the known root FEN only. No cross-shard transposition or "
                          "freshness proof, fitting or strength claim."}
    with (output_dir / "manifest.json").open("x") as stream:
        json.dump(report, stream, indent=2, sort_keys=True)
        stream.write("\n")
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--partition", choices=("train", "validation", "holdout"), required=True)
    parser.add_argument("--max-plies", type=int, required=True)
    parser.add_argument("--max-nodes", type=int, required=True)
    args = parser.parse_args()
    try:
        report = export_shard(args.input_dir, args.output_dir, args.partition, args.max_plies, args.max_nodes)
    except (OSError, ValueError, KeyError, TypeError) as error:
        parser.exit(1, f"teacher-export: {error}\n")
    print(json.dumps({"manifest": str(args.output_dir / "manifest.json"), "positions": report["positions"]}))


if __name__ == "__main__":
    main()
