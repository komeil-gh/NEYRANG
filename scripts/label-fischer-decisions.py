#!/usr/bin/env python3
"""Label Fischer decisions against a pinned UCI teacher at fixed nodes."""

from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import os
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

import chess
import chess.engine

SCHEMA = "neyrang-fischer-labels-v1"
MATE_SCORE = 32_000


class LabelError(RuntimeError):
    """A fail-closed teacher-labeling error."""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--engine", required=True, type=Path)
    parser.add_argument("--nodes", required=True, type=int)
    parser.add_argument("--hash-mb", type=int, default=64)
    parser.add_argument("--workers", type=int, default=1)
    return parser.parse_args()


def sha256_file(path: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
            size += len(chunk)
    return digest.hexdigest(), size


def read_records(path: Path) -> list[dict[str, Any]]:
    records: list[dict[str, Any]] = []
    with path.open(encoding="utf-8") as stream:
        for line_number, line in enumerate(stream, start=1):
            try:
                record = json.loads(line)
                board = chess.Board(record["fen"])
                move = chess.Move.from_uci(record["move"])
            except (KeyError, ValueError, json.JSONDecodeError) as error:
                raise LabelError(f"invalid input line {line_number}: {error}") from error
            if move not in board.legal_moves:
                raise LabelError(f"input line {line_number} contains an illegal chosen move")
            records.append(record)
    if not records:
        raise LabelError("input contains no decisions")
    return records


def engine_configuration(engine: chess.engine.SimpleEngine, hash_mb: int) -> None:
    required = {"Threads", "Hash", "Clear Hash"}
    missing = sorted(required - engine.options.keys())
    if missing:
        raise LabelError(f"teacher lacks required UCI option: {missing[0]}")
    requested: dict[str, Any] = {"Threads": 1, "Hash": hash_mb}
    if "UCI_AnalyseMode" in engine.options:
        requested["UCI_AnalyseMode"] = True
    if requested:
        engine.configure(requested)


def analyse(
    engine: chess.engine.SimpleEngine,
    board: chess.Board,
    nodes: int,
    game_token: str,
    root_move: chess.Move | None = None,
) -> dict[str, Any]:
    engine.configure({"Clear Hash": None})
    return engine.analyse(
        board,
        chess.engine.Limit(nodes=nodes),
        root_moves=[root_move] if root_move is not None else None,
        info=chess.engine.INFO_SCORE | chess.engine.INFO_PV,
        game=game_token,
    )


def inspect_engine(path: Path, hash_mb: int) -> dict[str, Any]:
    engine = chess.engine.SimpleEngine.popen_uci(str(path))
    try:
        engine_configuration(engine, hash_mb)
        return dict(engine.id)
    finally:
        engine.quit()


def numeric_score(info: dict[str, Any], turn: chess.Color) -> int:
    score = info.get("score")
    if score is None:
        raise LabelError("teacher returned no score")
    value = score.pov(turn).score(mate_score=MATE_SCORE)
    if value is None:
        raise LabelError("teacher returned an unusable score")
    return value


def label_one(
    engine: chess.engine.SimpleEngine,
    nodes: int,
    index_and_record: tuple[int, dict[str, Any]],
) -> tuple[int, dict[str, Any]]:
    index, record = index_and_record
    board = chess.Board(record["fen"])
    chosen = chess.Move.from_uci(record["move"])
    game_token = f"fischer-{record['game_id']}-{record['ply']}"
    discovery_info = analyse(engine, board, nodes, f"{game_token}-discovery")
    best_pv = discovery_info.get("pv") or []
    if not best_pv:
        raise LabelError("teacher returned no principal variation")
    best_move = best_pv[0]
    discovery_score = numeric_score(discovery_info, board.turn)
    best_info = analyse(engine, board, nodes, f"{game_token}-best", best_move)
    best_score = numeric_score(best_info, board.turn)
    if chosen == best_move:
        chosen_score = best_score
    else:
        chosen_info = analyse(engine, board, nodes, f"{game_token}-chosen", chosen)
        chosen_score = numeric_score(chosen_info, board.turn)
    return index, {
        **record,
        "teacher_best_move": best_move.uci(),
        "teacher_discovery_score_cp": discovery_score,
        "teacher_best_score_cp": best_score,
        "teacher_chosen_score_cp": chosen_score,
        "teacher_loss_cp": max(0, best_score - chosen_score),
    }


def label_batch(
    task: tuple[str, int, int, list[tuple[int, dict[str, Any]]]],
) -> list[tuple[int, dict[str, Any]]]:
    engine_path, nodes, hash_mb, batch = task
    engine = chess.engine.SimpleEngine.popen_uci(engine_path)
    try:
        engine_configuration(engine, hash_mb)
        return [label_one(engine, nodes, item) for item in batch]
    finally:
        engine.quit()


def main() -> int:
    args = parse_args()
    try:
        if args.nodes <= 0 or args.hash_mb <= 0 or args.workers <= 0:
            raise LabelError("nodes, hash-mb and workers must be positive")
        input_path = args.input.resolve()
        engine_path = args.engine.resolve()
        output_path = args.output.resolve()
        manifest_path = output_path.with_suffix(output_path.suffix + ".manifest.json")
        if not input_path.is_file() or not engine_path.is_file():
            raise LabelError("input and engine must both be files")
        if output_path.exists() or manifest_path.exists():
            raise LabelError("refusing to overwrite an output or its manifest")
        output_path.parent.mkdir(parents=True, exist_ok=True)
        records = read_records(input_path)
        teacher_id = inspect_engine(engine_path, args.hash_mb)
        engine_hash, engine_bytes = sha256_file(engine_path)
        input_hash, input_bytes = sha256_file(input_path)

        indexed = list(enumerate(records))
        batches = [indexed[offset :: args.workers] for offset in range(args.workers)]
        tasks = [
            (str(engine_path), args.nodes, args.hash_mb, batch)
            for batch in batches
            if batch
        ]
        if len(tasks) == 1:
            labeled = label_batch(tasks[0])
        else:
            with concurrent.futures.ProcessPoolExecutor(max_workers=len(tasks)) as executor:
                labeled = [item for batch in executor.map(label_batch, tasks) for item in batch]
        labeled.sort(key=lambda item: item[0])

        temporary = output_path.with_name(f".{output_path.name}.tmp-{os.getpid()}")
        digest = hashlib.sha256()
        with temporary.open("wb") as stream:
            for _, record in labeled:
                line = (json.dumps(record, sort_keys=True, separators=(",", ":")) + "\n").encode()
                stream.write(line)
                digest.update(line)
        temporary.replace(output_path)
        manifest = {
            "schema": SCHEMA,
            "created_at": datetime.now(timezone.utc).isoformat(),
            "input": {"path": str(input_path), "bytes": input_bytes, "sha256": input_hash},
            "teacher": {
                "path": str(engine_path),
                "bytes": engine_bytes,
                "sha256": engine_hash,
                "uci_id": teacher_id,
                "nodes_per_search": args.nodes,
                "threads": 1,
                "hash_mb_per_worker": args.hash_mb,
                "workers": args.workers,
                "mate_score_cp": MATE_SCORE,
                "hash_cleared_before_every_search": True,
                "score_comparison": "equal-node root-move verification",
            },
            "output": {
                "path": str(output_path),
                "records": len(labeled),
                "bytes": output_path.stat().st_size,
                "sha256": digest.hexdigest(),
            },
        }
        manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    except (LabelError, OSError, chess.engine.EngineError) as error:
        print(f"label-fischer-decisions: {error}", file=sys.stderr)
        return 1
    print(f"labeled {len(labeled)} decisions")
    print(f"manifest: {manifest_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
