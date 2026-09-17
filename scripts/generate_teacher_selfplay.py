#!/usr/bin/env python3
"""Generate a compact, deterministic Stockfish-teacher corpus for SANJ."""

from __future__ import annotations

import argparse
import concurrent.futures
import hashlib
import json
import multiprocessing
import os
import random
import shutil
from collections import Counter
from dataclasses import asdict, dataclass
from pathlib import Path

import chess
import chess.engine

from .teacher_target import ENCODING, wdl_score


SCHEMA = "neyrang-teacher-selfplay-v1"
SCORE_ONLY_ENCODING = ENCODING.replace("-v1", "-v2")
HEADER = f"# {SCORE_ONLY_ENCODING}\n"


@dataclass(frozen=True)
class Config:
    teacher: str
    output: str
    games: int
    workers: int
    nodes: int
    hash_mb: int
    max_plies: int
    variation_plies: int
    multipv: int
    batch_size: int
    max_positions_per_game: int
    max_saturated_per_game: int
    seed: str


def sha256_file(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def stable_random(seed: str, game_id: int) -> random.Random:
    digest = hashlib.sha256(f"{seed}\t{game_id}".encode()).digest()
    return random.Random(int.from_bytes(digest[:8], "big"))


def partition(seed: str, game_id: int) -> str:
    value = int.from_bytes(
        hashlib.sha256(f"{seed}\tpartition\t{game_id}".encode()).digest()[:8],
        "big",
    ) % 100
    return "train" if value < 80 else "validation" if value < 90 else "holdout"


def select_records(
    records: list[tuple[int, str, int]],
    seed: str,
    game_id: int,
    max_positions: int,
    max_saturated: int,
) -> list[tuple[int, str, int]]:
    def rank(record: tuple[int, str, int]) -> bytes:
        ply, _fen, _score = record
        return hashlib.sha256(f"{seed}\trecord\t{game_id}\t{ply}".encode()).digest()

    saturated = sorted(
        (record for record in records if abs(record[2]) == 3040), key=rank
    )[:max_saturated]
    moderate = [record for record in records if abs(record[2]) != 3040]
    selected = sorted(moderate + saturated, key=rank)[:max_positions]
    return sorted(selected)


def choose_line(infos: list[dict], rng: random.Random) -> dict:
    weights = [1.0 / (rank + 1) ** 2 for rank in range(len(infos))]
    return rng.choices(infos, weights=weights, k=1)[0]


def engine_options(config: Config) -> dict:
    return {
        "Threads": 1,
        "Hash": config.hash_mb,
        "UCI_ShowWDL": True,
        "UCI_LimitStrength": False,
        "Skill Level": 20,
        "SyzygyPath": "",
        "SyzygyProbeLimit": 0,
    }


def eligible(board: chess.Board, info: dict, played: chess.Move, ply: int) -> bool:
    score = info["score"].pov(board.turn)
    pv = info.get("pv") or []
    if (
        ply < 16
        or board.is_check()
        or len(board.piece_map()) < 4
        or not pv
        or score.is_mate()
    ):
        return False
    cp = score.score()
    first = pv[0]
    return (
        cp is not None
        and abs(cp) < 10_000
        and not board.is_capture(first)
        and not first.promotion
        and not board.is_capture(played)
        and not played.promotion
    )


def analyse(
    engine: chess.engine.SimpleEngine,
    board: chess.Board,
    config: Config,
    ply: int,
    rng: random.Random,
):
    count = config.multipv if ply < config.variation_plies else 1
    result = engine.analyse(
        board,
        chess.engine.Limit(nodes=config.nodes),
        info=chess.engine.INFO_ALL,
        multipv=count,
    )
    infos = result if isinstance(result, list) else [result]
    if not infos or any(not info.get("pv") for info in infos):
        raise RuntimeError("teacher returned an empty principal variation")
    chosen = choose_line(infos, rng) if count > 1 else infos[0]
    return infos[0], chosen["pv"][0]


def worker(config: Config, worker_id: int, shard: str) -> dict:
    path = Path(shard)
    teacher = Path(config.teacher)
    counts = Counter()
    engine = chess.engine.SimpleEngine.popen_uci(str(teacher), timeout=30)
    try:
        options = engine_options(config)
        missing = sorted(set(options) - set(engine.options))
        if missing:
            raise RuntimeError(f"teacher lacks required UCI options: {missing}")
        engine.configure(options)
        engine_name = engine.id.get("name", "unknown")
        with path.open("x", encoding="ascii") as stream:
            for game_id in range(worker_id, config.games, config.workers):
                engine.configure({"Clear Hash": None})
                board = chess.Board()
                rng = stable_random(config.seed, game_id)
                game_partition = partition(config.seed, game_id)
                records = []
                for ply in range(config.max_plies):
                    if board.outcome(claim_draw=True) is not None:
                        counts["completed_games"] += 1
                        break
                    best, played = analyse(engine, board, config, ply, rng)
                    if played not in board.legal_moves:
                        raise RuntimeError("teacher returned an illegal move")
                    if eligible(board, best, played, ply):
                        wdl = list(best["wdl"].white())
                        if sum(wdl) != 1000:
                            raise RuntimeError("teacher WDL does not sum to 1000")
                        fen = board.fen(en_passant="fen")
                        records.append((ply, fen, wdl_score(wdl)))
                    board.push(played)
                selected = select_records(
                    records,
                    config.seed,
                    game_id,
                    config.max_positions_per_game,
                    config.max_saturated_per_game,
                )
                for ply, fen, score in selected:
                    stream.write(
                        f"{game_partition}\t{game_id}\t{ply}\t{fen}\t{score}\n"
                    )
                    counts[f"raw_{game_partition}"] += 1
                counts["dropped_by_per_game_cap"] += len(records) - len(selected)
                counts["games"] += 1
    finally:
        try:
            engine.quit()
        except BaseException:
            engine.close()
            raise
    counts["engine_name"] = engine_name
    counts["native_exit"] = engine.returncode.result(timeout=5)
    return dict(counts)


def validate(config: Config) -> tuple[Path, Path]:
    teacher = Path(config.teacher).resolve(strict=True)
    output = Path(config.output).resolve()
    if output.exists():
        raise FileExistsError(f"refusing to reuse output directory: {output}")
    for name in (
        "games",
        "workers",
        "nodes",
        "hash_mb",
        "max_plies",
        "variation_plies",
        "multipv",
        "batch_size",
        "max_positions_per_game",
        "max_saturated_per_game",
    ):
        if getattr(config, name) <= 0:
            raise ValueError(f"--{name.replace('_', '-')} must be positive")
    if config.workers > config.games:
        raise ValueError("--workers cannot exceed --games")
    if config.variation_plies > config.max_plies or config.multipv > 8:
        raise ValueError("variation bounds exceed the supported contract")
    if config.max_saturated_per_game > config.max_positions_per_game:
        raise ValueError("--max-saturated-per-game cannot exceed --max-positions-per-game")
    if not config.seed:
        raise ValueError("--seed must not be empty")
    return teacher, output


def generate(config: Config) -> dict:
    teacher, output = validate(config)
    output.mkdir(parents=True)
    shards = output / "shards"
    shards.mkdir()
    shard_paths = [shards / f"{index:02d}.tsv" for index in range(config.workers)]
    try:
        with concurrent.futures.ProcessPoolExecutor(
            max_workers=config.workers,
            mp_context=multiprocessing.get_context("spawn"),
        ) as pool:
            futures = [
                pool.submit(worker, config, index, str(path))
                for index, path in enumerate(shard_paths)
            ]
            worker_reports = [future.result() for future in futures]

        handles = {
            name: (output / f"{name}.txt").open("x", encoding="ascii")
            for name in ("train", "validation", "holdout")
        }
        for stream in handles.values():
            stream.write(HEADER)
        seen = set()
        counts = Counter()
        try:
            for path in shard_paths:
                with path.open(encoding="ascii") as rows:
                    for line in rows:
                        part, _game_id, _ply, fen, score = line.rstrip("\n").split("\t")
                        key = " ".join(fen.split()[:4])
                        if key in seen:
                            counts["duplicates"] += 1
                            continue
                        seen.add(key)
                        handles[part].write(f"{fen} | {score}\n")
                        counts[part] += 1
        finally:
            for stream in handles.values():
                stream.close()

        train = output / "train.txt"
        keep = counts["train"] - counts["train"] % config.batch_size
        if keep == 0:
            raise RuntimeError("training partition is smaller than one complete batch")
        if keep != counts["train"]:
            trimmed = output / "train.trimmed.txt"
            with train.open(encoding="ascii") as source, trimmed.open(
                "x", encoding="ascii"
            ) as target:
                target.write(next(source))
                for index, line in enumerate(source):
                    if index == keep:
                        break
                    target.write(line)
            trimmed.replace(train)
            counts["trimmed_train"] = counts["train"] - keep
            counts["train"] = keep

        artifacts = {}
        for name in ("train.txt", "validation.txt", "holdout.txt"):
            path = output / name
            artifacts[name] = {"bytes": path.stat().st_size, "sha256": sha256_file(path)}
        report = {
            "schema": SCHEMA,
            "encoding": SCORE_ONLY_ENCODING,
            "teacher": {
                "path_basename": teacher.name,
                "sha256": sha256_file(teacher),
                "engine_names": sorted({str(item["engine_name"]) for item in worker_reports}),
            },
            "settings": asdict(config) | {"teacher": teacher.name, "output": output.name},
            "uci_options": engine_options(config) | {"Clear Hash": "before every game"},
            "partition_policy": "SHA-256(seed + TAB + partition + TAB + game_id), 80/10/10",
            "deduplication_key": "first four canonical FEN fields across all partitions",
            "counts": dict(sorted(counts.items())),
            "workers": worker_reports,
            "artifacts": artifacts,
            "generator_sha256": sha256_file(Path(__file__)),
            "boundary": (
                "Score-only teacher rows use no game-result target. Single-thread fixed-node "
                "Stockfish searches label each retained quiet position; the trainer must use "
                "wdl_proportion=0. Generated data and absolute paths remain outside Git."
            ),
        }
        with (output / "manifest.json").open("x") as stream:
            json.dump(report, stream, indent=2, sort_keys=True)
            stream.write("\n")
        shutil.rmtree(shards)
        return report
    except BaseException:
        raise


def parse_args() -> Config:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--teacher", required=True)
    parser.add_argument("--output", required=True)
    parser.add_argument("--games", type=int, required=True)
    parser.add_argument("--workers", type=int, required=True)
    parser.add_argument("--nodes", type=int, required=True)
    parser.add_argument("--hash-mb", type=int, default=32)
    parser.add_argument("--max-plies", type=int, default=160)
    parser.add_argument("--variation-plies", type=int, default=12)
    parser.add_argument("--multipv", type=int, default=4)
    parser.add_argument("--batch-size", type=int, default=16_384)
    parser.add_argument("--max-positions-per-game", type=int, default=64)
    parser.add_argument("--max-saturated-per-game", type=int, default=8)
    parser.add_argument("--seed", required=True)
    return Config(**vars(parser.parse_args()))


def main() -> int:
    try:
        config = parse_args()
        report = generate(config)
    except (OSError, ValueError, RuntimeError, chess.engine.EngineError) as error:
        print(f"generate-teacher-selfplay: {error}", file=os.sys.stderr)
        return 1
    print(
        json.dumps(
            {
                "manifest": str(Path(config.output) / "manifest.json"),
                "counts": report["counts"],
            }
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
