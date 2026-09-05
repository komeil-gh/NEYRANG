#!/usr/bin/env python3
"""Create the registered SHEGERD-P1 teacher-label inputs from an audited PGN."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shutil
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import chess
import chess.engine
import chess.pgn


SCHEMA = "neyrang-shegerd-p1-corpus-v1"
SAMPLE_SCHEMA = "neyrang-shegerd-p1-sample-v1"
SPLIT_SCHEMA = "neyrang-shegerd-p1-split-v1"
PARTITIONS = ("train", "validation", "sealed_holdout")
LABEL_HEADER = "record_id\tgroup_id\tprevious_to\tteacher_move\tfen\n"
MINIMUMS = {
    "positions_total": 4000,
    "positions_train": 3000,
    "positions_validation": 400,
    "positions_sealed_holdout": 400,
    "groups_train": 600,
    "groups_validation": 75,
    "groups_sealed_holdout": 75,
}


class CorpusError(RuntimeError):
    """A fail-closed P1 corpus error."""


@dataclass(frozen=True)
class Sample:
    ply: int
    fen: str
    previous_to: str


@dataclass(frozen=True)
class Record:
    partition: str
    record_id: str
    group_id: str
    position_key: str
    previous_to: str
    fen: str


@dataclass(frozen=True)
class Config:
    source_id: str
    source: Path
    teacher: Path
    output_dir: Path
    sample_seed: str
    split_seed: str
    nodes: int
    hash_mb: int
    min_ply: int = 16
    tail_plies: int = 8
    samples_per_game: int = 4
    min_sample_gap: int = 8
    time_control: str = "0.5+0.005"
    enforce_minimums: bool = True


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--source-id", default="shegerd-p1-selfplay-v1")
    parser.add_argument("--teacher", required=True, type=Path)
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--sample-seed", default="20260905-shegerd-p1-sample-v1")
    parser.add_argument("--split-seed", default="20260905-shegerd-p1-split-v1")
    parser.add_argument("--nodes", type=int, default=80000)
    parser.add_argument("--hash-mb", type=int, default=64)
    return parser.parse_args()


def canonical_fen(fen: str) -> str:
    fields = fen.split()
    if len(fields) < 4:
        raise CorpusError("FEN has fewer than four fields")
    return " ".join(fields[:4])


def digest(*parts: str) -> bytes:
    return hashlib.sha256("\t".join(parts).encode("utf-8")).digest()


def sha256_file(path: Path) -> tuple[str, int]:
    value = hashlib.sha256()
    size = 0
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            value.update(chunk)
            size += len(chunk)
    return value.hexdigest(), size


def read_pairs(path: Path, expected_time_control: str) -> list[tuple[int, chess.pgn.Game, chess.pgn.Game]]:
    by_round: dict[str, list[chess.pgn.Game]] = defaultdict(list)
    with path.open(encoding="utf-8") as stream:
        game_number = 0
        while game := chess.pgn.read_game(stream):
            game_number += 1
            if game.errors:
                raise CorpusError(f"game {game_number}: PGN parse errors")
            round_id = game.headers.get("Round")
            if not round_id:
                raise CorpusError(f"game {game_number}: missing round header")
            by_round[round_id].append(game)
    if game_number == 0 or game_number % 2:
        raise CorpusError("source must contain a nonzero even game count")
    pairs: list[tuple[int, chess.pgn.Game, chess.pgn.Game]] = []
    for pair_index, round_id in enumerate(sorted(by_round, key=round_key), start=1):
        games = by_round[round_id]
        if len(games) != 2:
            raise CorpusError(f"round {round_id!r} contains {len(games)} games, expected two")
        left, right = games
        validate_pair(pair_index, left, right, expected_time_control)
        pairs.append((pair_index, left, right))
    return pairs


def round_key(value: str) -> tuple[int, tuple[int, ...] | str]:
    parts = value.split(".")
    return (0, tuple(int(part) for part in parts)) if all(part.isdigit() for part in parts) else (1, value)


def validate_pair(index: int, left: chess.pgn.Game, right: chess.pgn.Game, time_control: str) -> None:
    for game_number, game in enumerate((left, right), start=1):
        if game.headers.get("Result") not in {"1-0", "0-1", "1/2-1/2"}:
            raise CorpusError(f"pair {index} game {game_number}: invalid result")
        if game.headers.get("Termination") != "normal":
            raise CorpusError(f"pair {index} game {game_number}: termination is not normal")
        if game.headers.get("TimeControl") != time_control:
            raise CorpusError(f"pair {index} game {game_number}: unexpected time control")
        if game.headers.get("Variant", "Standard") not in {"Standard", "Chess"}:
            raise CorpusError(f"pair {index} game {game_number}: unsupported variant")
        moves = list(game.mainline_moves())
        if game.headers.get("PlyCount") not in {None, str(len(moves))}:
            raise CorpusError(f"pair {index} game {game_number}: PlyCount differs")
    if canonical_fen(left.headers.get("FEN", chess.STARTING_FEN)) != canonical_fen(right.headers.get("FEN", chess.STARTING_FEN)):
        raise CorpusError(f"pair {index}: opening FEN differs")
    if not (left.headers.get("White") == right.headers.get("Black") and left.headers.get("Black") == right.headers.get("White")):
        raise CorpusError(f"pair {index}: colors are not reversed")


def samples_for_game(game: chess.pgn.Game, source_id: str, pair_index: int, game_index: int, config: Config) -> tuple[list[Sample], Counter[str]]:
    board = game.board()
    moves = list(game.mainline_moves())
    previous_to = "-"
    eligible: list[Sample] = []
    counters: Counter[str] = Counter()
    for ply, move in enumerate(moves):
        if ply < config.min_ply:
            counters["opening_skipped"] += 1
        elif len(moves) - ply < config.tail_plies:
            counters["tail_skipped"] += 1
        elif board.is_check():
            counters["check_rejected"] += 1
        elif board.legal_moves.count() < 2:
            counters["single_legal_move_rejected"] += 1
        else:
            eligible.append(Sample(ply, board.fen(en_passant="fen"), previous_to))
            counters["eligible"] += 1
        board.push(move)
        previous_to = chess.square_name(move.to_square)
    ranked = sorted(
        eligible,
        key=lambda sample: digest(SAMPLE_SCHEMA, config.sample_seed, source_id, str(pair_index), str(game_index), str(sample.ply)),
    )
    selected: list[Sample] = []
    for sample in ranked:
        if all(abs(sample.ply - existing.ply) >= config.min_sample_gap for existing in selected):
            selected.append(sample)
            if len(selected) == config.samples_per_game:
                break
    counters["selected_before_pair_balance"] = len(selected)
    return selected, counters


def assign_partitions(openings: set[str], seed: str) -> dict[str, str]:
    ranked = sorted(openings, key=lambda item: (digest(SPLIT_SCHEMA, seed, item), item))
    train_end = len(ranked) * 80 // 100
    validation_end = len(ranked) * 90 // 100
    return {
        opening: "train" if index < train_end else "validation" if index < validation_end else "sealed_holdout"
        for index, opening in enumerate(ranked)
    }


def extract_records(config: Config) -> tuple[list[Record], dict[str, Any]]:
    pairs = read_pairs(config.source, config.time_control)
    opening_by_pair = {
        pair_index: canonical_fen(left.headers.get("FEN", chess.STARTING_FEN))
        for pair_index, left, _ in pairs
    }
    partitions = assign_partitions(set(opening_by_pair.values()), config.split_seed)
    counters: Counter[str] = Counter()
    records: list[Record] = []
    for pair_index, left, right in pairs:
        left_samples, left_counts = samples_for_game(left, config.source_id, pair_index, 1, config)
        right_samples, right_counts = samples_for_game(right, config.source_id, pair_index, 2, config)
        counters.update(left_counts)
        counters.update(right_counts)
        count = min(len(left_samples), len(right_samples))
        counters["pair_balance_dropped"] += len(left_samples) + len(right_samples) - (2 * count)
        if count == 0:
            counters["pairs_without_balanced_samples"] += 1
            continue
        counters["pairs_sampled"] += 1
        opening = opening_by_pair[pair_index]
        group_id = hashlib.sha256(opening.encode("utf-8")).hexdigest()
        for game_index, samples in ((1, left_samples[:count]), (2, right_samples[:count])):
            for sample in sorted(samples, key=lambda item: item.ply):
                record_id = f"{config.source_id}:pair-{pair_index:06d}:game-{game_index}:ply-{sample.ply:03d}"
                records.append(Record(
                    partitions[opening], record_id, group_id, canonical_fen(sample.fen), sample.previous_to, sample.fen,
                ))
    retained, deduplication = deduplicate(records)
    return retained, {
        "pairs": len(pairs),
        "sampling": dict(sorted(counters.items())),
        "deduplication": deduplication,
        "partition_opening_groups": dict(Counter(partitions.values())),
    }


def deduplicate(records: list[Record]) -> tuple[list[Record], dict[str, int]]:
    by_key: dict[str, list[Record]] = defaultdict(list)
    for record in records:
        by_key[record.position_key].append(record)
    kept: list[Record] = []
    within = cross_keys = cross_records = 0
    for candidates in by_key.values():
        partitions = {candidate.partition for candidate in candidates}
        if len(partitions) > 1:
            cross_keys += 1
            cross_records += len(candidates)
            continue
        kept.append(min(candidates, key=lambda candidate: candidate.record_id))
        within += len(candidates) - 1
    return sorted(kept, key=lambda item: item.record_id), {
        "input_records": len(records), "unique_position_keys": len(by_key), "within_partition_records_removed": within,
        "cross_partition_keys_removed": cross_keys, "cross_partition_records_removed": cross_records, "records_kept": len(kept),
    }


def configure_teacher(engine: chess.engine.SimpleEngine, hash_mb: int) -> dict[str, str]:
    for option in ("Threads", "Hash", "Clear Hash"):
        if option not in engine.options:
            raise CorpusError(f"teacher lacks required UCI option {option}")
    engine.configure({"Threads": 1, "Hash": hash_mb})
    return dict(engine.id)


def label_records(records: list[Record], teacher: Path, nodes: int, hash_mb: int) -> tuple[dict[str, list[str]], dict[str, str]]:
    output = {partition: [LABEL_HEADER] for partition in PARTITIONS}
    engine = chess.engine.SimpleEngine.popen_uci(str(teacher))
    try:
        identity = configure_teacher(engine, hash_mb)
        for record in records:
            board = chess.Board(record.fen)
            engine.configure({"Clear Hash": None})
            analysis = engine.analyse(board, chess.engine.Limit(nodes=nodes), info=chess.engine.INFO_PV)
            pv = analysis.get("pv") or []
            if not pv or pv[0] not in board.legal_moves:
                raise CorpusError(f"teacher produced no legal PV move for {record.record_id}")
            output[record.partition].append(
                f"{record.record_id}\t{record.group_id}\t{record.previous_to}\t{pv[0].uci()}\t{record.fen}\n"
            )
    except chess.engine.EngineError as error:
        raise CorpusError(f"teacher failure: {error}") from error
    finally:
        engine.quit()
    return output, identity


def enforce_minimums(records: list[Record], enabled: bool) -> dict[str, dict[str, int]]:
    counts = {partition: {"records": 0, "groups": 0} for partition in PARTITIONS}
    for partition in PARTITIONS:
        selected = [record for record in records if record.partition == partition]
        counts[partition] = {"records": len(selected), "groups": len({record.group_id for record in selected})}
    if enabled:
        expected = {
            "train": (MINIMUMS["positions_train"], MINIMUMS["groups_train"]),
            "validation": (MINIMUMS["positions_validation"], MINIMUMS["groups_validation"]),
            "sealed_holdout": (MINIMUMS["positions_sealed_holdout"], MINIMUMS["groups_sealed_holdout"]),
        }
        if len(records) < MINIMUMS["positions_total"]:
            raise CorpusError("P1 corpus does not meet total position minimum")
        for partition, (position_minimum, group_minimum) in expected.items():
            if counts[partition]["records"] < position_minimum or counts[partition]["groups"] < group_minimum:
                raise CorpusError(f"P1 corpus does not meet {partition} minimum")
    return counts


def build(config: Config) -> dict[str, Any]:
    if not config.source.is_file() or not config.teacher.is_file():
        raise CorpusError("source and teacher must be files")
    if not re.fullmatch(r"[A-Za-z0-9][A-Za-z0-9._-]*", config.source_id):
        raise CorpusError("source id is invalid")
    if not config.sample_seed or not config.split_seed:
        raise CorpusError("sampling and split seeds must be non-empty")
    if config.output_dir.exists():
        raise CorpusError("refusing to overwrite an existing output directory")
    if config.nodes != 80000 or config.hash_mb != 64 or config.samples_per_game != 4 or config.min_sample_gap != 8:
        raise CorpusError("registered P1 teacher or sampling setting changed")
    if config.min_ply != 16 or config.tail_plies != 8:
        raise CorpusError("registered P1 ply boundary changed")
    records, extraction = extract_records(config)
    if config.enforce_minimums and extraction["pairs"] != 1000:
        raise CorpusError("registered P1 source must contain exactly 1000 opening pairs")
    counts = enforce_minimums(records, config.enforce_minimums)
    labels, teacher_identity = label_records(records, config.teacher, config.nodes, config.hash_mb)
    temporary = config.output_dir.with_name(f".{config.output_dir.name}.tmp-{os.getpid()}")
    if temporary.exists():
        raise CorpusError("temporary output directory already exists")
    temporary.mkdir(parents=True)
    try:
        artifacts: dict[str, dict[str, Any]] = {}
        for partition in PARTITIONS:
            path = temporary / f"{partition}.labels.tsv"
            payload = "".join(labels[partition]).encode("utf-8")
            path.write_bytes(payload)
            artifacts[partition] = {"path": path.name, "bytes": len(payload), "sha256": hashlib.sha256(payload).hexdigest(), **counts[partition]}
        source_hash, source_bytes = sha256_file(config.source)
        teacher_hash, teacher_bytes = sha256_file(config.teacher)
        manifest = {
            "schema": SCHEMA,
            "source": {"id": config.source_id, "bytes": source_bytes, "sha256": source_hash, "pairs": extraction["pairs"]},
            "teacher": {"bytes": teacher_bytes, "sha256": teacher_hash, "uci_id": teacher_identity, "nodes": config.nodes, "threads": 1, "hash_mb": config.hash_mb, "clear_hash_per_position": True},
            "sampling": {"schema": SAMPLE_SCHEMA, "seed": config.sample_seed, "min_ply": config.min_ply, "tail_plies": config.tail_plies, "samples_per_game": config.samples_per_game, "min_sample_gap": config.min_sample_gap, "pair_balanced": True},
            "split": {"schema": SPLIT_SCHEMA, "seed": config.split_seed, "unit": "canonical starting FEN", "fractions": {"train": 0.8, "validation": 0.1, "sealed_holdout": 0.1}},
            "extraction": extraction,
            "artifacts": artifacts,
        }
        (temporary / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        temporary.replace(config.output_dir)
        return manifest
    except BaseException:
        shutil.rmtree(temporary, ignore_errors=True)
        raise


def main() -> int:
    args = parse_args()
    try:
        manifest = build(Config(
            args.source_id, args.source.resolve(), args.teacher.resolve(), args.output_dir.resolve(), args.sample_seed,
            args.split_seed, args.nodes, args.hash_mb,
        ))
    except (CorpusError, OSError, ValueError, chess.engine.EngineError) as error:
        print(f"build-shegerd-policy-corpus: {error}", file=sys.stderr)
        return 1
    print(json.dumps({"ok": True, "artifacts": manifest["artifacts"]}, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
