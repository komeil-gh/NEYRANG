#!/usr/bin/env python3
"""Build a deterministic, pair-first SANJ corpus from audited PGNs."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable

import chess
import chess.pgn


CORPUS_SCHEMA = "neyrang-sanj-corpus-v1"
DENSE_SAMPLING_SCHEMA = "neyrang-sanj-dense-sampling-v1"
RESULT_TARGET = {"1-0": 1.0, "0-1": 0.0, "1/2-1/2": 0.5}
PARTITIONS = ("train", "validation", "holdout")
SOURCE_ID = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]*$")


class CorpusError(RuntimeError):
    """A fail-closed corpus validation error."""


@dataclass(frozen=True)
class SourceSpec:
    source_id: str
    path: Path


@dataclass(frozen=True)
class BuildConfig:
    repo_root: Path
    output_dir: Path
    sources: tuple[SourceSpec, ...]
    seed: str
    min_ply: int = 16
    tail_plies: int = 8
    samples_per_game: int = 1
    min_sample_gap: int = 0
    train_percent: int = 80
    validation_percent: int = 10
    fixed_partition: str | None = None


@dataclass(frozen=True)
class Record:
    partition: str
    record_id: str
    target: float
    fen: str
    position_key: str
    source_id: str
    pair_index: int
    game_in_pair: int
    ply: int


@dataclass(frozen=True)
class Sample:
    ply: int
    fen: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--source",
        action="append",
        required=True,
        metavar="ID=PGN",
        help="audited paired PGN source; repeat for multiple sources",
    )
    parser.add_argument("--output-dir", required=True, type=Path)
    parser.add_argument("--seed", required=True)
    parser.add_argument("--min-ply", type=int, default=16)
    parser.add_argument("--tail-plies", type=int, default=8)
    parser.add_argument("--samples-per-game", type=int, default=1)
    parser.add_argument("--min-sample-gap", type=int, default=0)
    parser.add_argument(
        "--fixed-partition",
        choices=PARTITIONS,
        help="place every opening group in one partition for a separately sealed corpus",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = Path(__file__).resolve().parent.parent
    try:
        sources = parse_sources(args.source, repo_root)
        output_dir = resolve_inside_repo(args.output_dir, repo_root, "output directory")
        config = BuildConfig(
            repo_root=repo_root,
            output_dir=output_dir,
            sources=sources,
            seed=args.seed,
            min_ply=args.min_ply,
            tail_plies=args.tail_plies,
            samples_per_game=args.samples_per_game,
            min_sample_gap=args.min_sample_gap,
            fixed_partition=args.fixed_partition,
        )
        manifest = build_corpus(config)
    except (CorpusError, OSError) as error:
        print(f"build-sanj-corpus: {error}", file=sys.stderr)
        return 1

    counts = manifest["records"]["partitions"]
    print(
        "built "
        f"{sum(item['records'] for item in counts.values())} records: "
        + ", ".join(f"{name}={counts[name]['records']}" for name in PARTITIONS)
    )
    print(f"manifest: {config.output_dir.relative_to(config.repo_root)}/manifest.json")
    return 0


def parse_sources(specifications: list[str], repo_root: Path) -> tuple[SourceSpec, ...]:
    sources: list[SourceSpec] = []
    seen: set[str] = set()
    for specification in specifications:
        source_id, separator, path_text = specification.partition("=")
        if not separator or not source_id or not path_text:
            raise CorpusError(f"invalid --source {specification!r}; expected ID=PGN")
        if not SOURCE_ID.fullmatch(source_id):
            raise CorpusError(f"invalid source id {source_id!r}")
        if source_id in seen:
            raise CorpusError(f"duplicate source id {source_id!r}")
        seen.add(source_id)
        path = resolve_inside_repo(Path(path_text), repo_root, f"source {source_id!r}")
        if not path.is_file():
            raise CorpusError(f"source {source_id!r} is not a file: {path_text}")
        sources.append(SourceSpec(source_id, path))
    return tuple(sorted(sources, key=lambda source: source.source_id))


def resolve_inside_repo(path: Path, repo_root: Path, label: str) -> Path:
    candidate = path if path.is_absolute() else repo_root / path
    resolved = candidate.resolve()
    try:
        resolved.relative_to(repo_root.resolve())
    except ValueError as error:
        raise CorpusError(f"{label} must be inside the repository") from error
    return resolved


def validate_config(config: BuildConfig) -> None:
    if not config.sources:
        raise CorpusError("at least one source is required")
    if not config.seed:
        raise CorpusError("seed must not be empty")
    if config.min_ply < 0 or config.tail_plies < 0:
        raise CorpusError("min-ply and tail-plies must be non-negative")
    if config.samples_per_game <= 0:
        raise CorpusError("samples-per-game must be positive")
    if config.min_sample_gap < 0:
        raise CorpusError("min-sample-gap must be non-negative")
    if config.samples_per_game == 1 and config.min_sample_gap != 0:
        raise CorpusError("single-sample mode requires min-sample-gap 0")
    if config.samples_per_game > 1 and config.min_sample_gap == 0:
        raise CorpusError("dense mode requires a positive min-sample-gap")
    if config.train_percent <= 0 or config.validation_percent <= 0:
        raise CorpusError("train and validation percentages must be positive")
    if config.train_percent + config.validation_percent >= 100:
        raise CorpusError("train plus validation percentages must be below 100")
    if config.fixed_partition is not None and config.fixed_partition not in PARTITIONS:
        raise CorpusError(f"fixed partition must be one of {PARTITIONS}")
    try:
        config.output_dir.resolve().relative_to(config.repo_root.resolve())
    except ValueError as error:
        raise CorpusError("output directory must be inside the repository") from error


def build_corpus(config: BuildConfig) -> dict[str, Any]:
    validate_config(config)
    if config.output_dir.exists():
        raise CorpusError(f"refusing to overwrite existing output: {relative(config.output_dir, config.repo_root)}")

    config.output_dir.parent.mkdir(parents=True, exist_ok=True)
    temporary = config.output_dir.with_name(f".{config.output_dir.name}.tmp-{os.getpid()}")
    if temporary.exists():
        raise CorpusError(f"temporary output already exists: {relative(temporary, config.repo_root)}")
    temporary.mkdir()

    try:
        source_manifests: list[dict[str, Any]] = []
        all_records: list[Record] = []
        for source in config.sources:
            source_manifest, records = extract_source(source, config)
            source_manifests.append(source_manifest)
            all_records.extend(records)

        records, duplicate_summary = deduplicate_records(all_records)
        partitioned = {name: [] for name in PARTITIONS}
        for record in records:
            partitioned[record.partition].append(record)
        for values in partitioned.values():
            values.sort(key=lambda record: record.record_id)

        artifacts = write_partitions(temporary, partitioned)
        manifest = make_manifest(
            config,
            source_manifests,
            all_records,
            partitioned,
            duplicate_summary,
            artifacts,
        )
        write_text(
            temporary / "manifest.json",
            json.dumps(manifest, indent=2, sort_keys=True) + "\n",
        )
        temporary.replace(config.output_dir)
        return manifest
    except BaseException:
        shutil.rmtree(temporary, ignore_errors=True)
        raise


def extract_source(
    source: SourceSpec, config: BuildConfig
) -> tuple[dict[str, Any], list[Record]]:
    source_hash, source_bytes = sha256_file(source.path)
    stats: Counter[str] = Counter()
    engine_names: set[str] = set()
    time_controls: Counter[str] = Counter()
    results: Counter[str] = Counter()
    pair_record_counts: Counter[str] = Counter()
    target_record_counts: Counter[str] = Counter()
    records: list[Record] = []

    with source.path.open(encoding="utf-8") as handle:
        pair_index = 0
        while True:
            left = chess.pgn.read_game(handle)
            if left is None:
                break
            right = chess.pgn.read_game(handle)
            pair_index += 1
            if right is None:
                raise CorpusError(
                    f"{source.source_id} pair {pair_index}: source has an odd trailing game"
                )
            stats["games_parsed"] += 2
            stats["pairs_parsed"] += 1
            validate_pair(source.source_id, pair_index, left, right)

            opening_fen = left.headers.get("FEN", chess.STARTING_FEN)
            opening_key = canonical_position_key(opening_fen)
            partition = partition_for_opening(opening_key, config)
            left_samples, left_stats = sample_game(
                left, source.source_id, pair_index, 1, config
            )
            right_samples, right_stats = sample_game(
                right, source.source_id, pair_index, 2, config
            )
            stats.update(left_stats)
            stats.update(right_stats)

            for game in (left, right):
                engine_names.update(
                    filter(None, (game.headers.get("White"), game.headers.get("Black")))
                )
                time_controls[game.headers.get("TimeControl", "missing")] += 1
                results[game.headers["Result"]] += 1

            paired_count = min(len(left_samples), len(right_samples))
            dropped = len(left_samples) + len(right_samples) - 2 * paired_count
            if dropped:
                stats["pair_balancing_records_dropped"] += dropped
            if paired_count == 0:
                stats["pairs_without_two_samples"] += 1
                continue

            stats["pairs_sampled"] += 1
            pair_record_counts[str(2 * paired_count)] += 1
            for game_in_pair, game, samples in (
                (1, left, left_samples),
                (2, right, right_samples),
            ):
                for sample in sorted(samples[:paired_count], key=lambda item: item.ply):
                    record_id = (
                        f"{source.source_id}:pair-{pair_index:06d}:"
                        f"game-{game_in_pair}:ply-{sample.ply:03d}"
                    )
                    records.append(
                        Record(
                            partition=partition,
                            record_id=record_id,
                            target=RESULT_TARGET[game.headers["Result"]],
                            fen=sample.fen,
                            position_key=canonical_position_key(sample.fen),
                            source_id=source.source_id,
                            pair_index=pair_index,
                            game_in_pair=game_in_pair,
                            ply=sample.ply,
                        )
                    )
                    stats["records_sampled"] += 1
                    target_record_counts[format_target(RESULT_TARGET[game.headers["Result"]])] += 1

    return (
        {
            "source_id": source.source_id,
            "path": relative(source.path, config.repo_root),
            "sha256": source_hash,
            "bytes": source_bytes,
            "engines": sorted(engine_names),
            "time_controls": sorted_counter(time_controls),
            "results": sorted_counter(results),
            "pair_record_counts": sorted_counter(pair_record_counts),
            "target_record_counts": sorted_counter(target_record_counts),
            "counts": sorted_counter(stats),
        },
        records,
    )


def validate_pair(
    source_id: str,
    pair_index: int,
    left: chess.pgn.Game,
    right: chess.pgn.Game,
) -> None:
    for game_in_pair, game in enumerate((left, right), start=1):
        if game.errors:
            raise CorpusError(
                f"{source_id} pair {pair_index} game {game_in_pair}: "
                f"PGN parse errors: {game.errors}"
            )
        result = game.headers.get("Result")
        if result not in RESULT_TARGET:
            raise CorpusError(
                f"{source_id} pair {pair_index} game {game_in_pair}: "
                f"invalid result {result!r}"
            )
        if game.headers.get("Termination") != "normal":
            raise CorpusError(
                f"{source_id} pair {pair_index} game {game_in_pair}: "
                f"termination is {game.headers.get('Termination')!r}"
            )
        variant = game.headers.get("Variant", "Standard")
        if variant not in {"Standard", "Chess"}:
            raise CorpusError(
                f"{source_id} pair {pair_index} game {game_in_pair}: "
                f"unsupported variant {variant!r}"
            )
        moves = list(game.mainline_moves())
        ply_count = game.headers.get("PlyCount")
        if ply_count is not None and ply_count != str(len(moves)):
            raise CorpusError(
                f"{source_id} pair {pair_index} game {game_in_pair}: "
                f"PlyCount {ply_count!r} != {len(moves)}"
            )

    left_fen = left.headers.get("FEN", chess.STARTING_FEN)
    right_fen = right.headers.get("FEN", chess.STARTING_FEN)
    if left_fen != right_fen:
        raise CorpusError(f"{source_id} pair {pair_index}: opening FENs differ")
    if left.headers.get("Round") != right.headers.get("Round"):
        raise CorpusError(f"{source_id} pair {pair_index}: round headers differ")
    if not (
        left.headers.get("White") == right.headers.get("Black")
        and left.headers.get("Black") == right.headers.get("White")
    ):
        raise CorpusError(f"{source_id} pair {pair_index}: engine colors are not reversed")


def sample_game(
    game: chess.pgn.Game,
    source_id: str,
    pair_index: int,
    game_in_pair: int,
    config: BuildConfig,
) -> tuple[list[Sample], Counter[str]]:
    moves = list(game.mainline_moves())
    board = game.board()
    eligible: list[Sample] = []
    stats: Counter[str] = Counter()

    for ply, move in enumerate(moves):
        if ply < config.min_ply:
            stats["positions_opening_skipped"] += 1
        elif len(moves) - ply < config.tail_plies:
            stats["positions_tail_skipped"] += 1
        elif board.is_check():
            stats["positions_check_rejected"] += 1
        elif has_legal_tactical_move(board):
            stats["positions_tactical_rejected"] += 1
        else:
            eligible.append(Sample(ply=ply, fen=board.fen(en_passant="fen")))
            stats["positions_eligible"] += 1
        board.push(move)

    if not eligible:
        stats["games_without_eligible_position"] += 1
        return [], stats

    if config.samples_per_game == 1:
        digest = stable_digest(
            CORPUS_SCHEMA,
            config.seed,
            source_id,
            str(pair_index),
            str(game_in_pair),
            "sample",
        )
        selected = [eligible[int.from_bytes(digest[:8], "big") % len(eligible)]]
    else:
        ranked = sorted(
            eligible,
            key=lambda sample: stable_digest(
                DENSE_SAMPLING_SCHEMA,
                config.seed,
                source_id,
                str(pair_index),
                str(game_in_pair),
                str(sample.ply),
                "rank",
            ),
        )
        selected = []
        for sample in ranked:
            if all(
                abs(sample.ply - existing.ply) >= config.min_sample_gap
                for existing in selected
            ):
                selected.append(sample)
                if len(selected) == config.samples_per_game:
                    break
        stats["positions_selected"] += len(selected)

    stats["games_with_eligible_position"] += 1
    return selected, stats


def has_legal_tactical_move(board: chess.Board) -> bool:
    return any(board.is_capture(move) or move.promotion for move in board.legal_moves)


def partition_for_opening(opening_key: str, config: BuildConfig) -> str:
    if config.fixed_partition is not None:
        return config.fixed_partition
    digest = stable_digest(CORPUS_SCHEMA, config.seed, opening_key, "partition")
    bucket = int.from_bytes(digest[:8], "big") % 100
    if bucket < config.train_percent:
        return "train"
    if bucket < config.train_percent + config.validation_percent:
        return "validation"
    return "holdout"


def deduplicate_records(
    records: Iterable[Record],
) -> tuple[list[Record], dict[str, int]]:
    by_position: dict[str, list[Record]] = defaultdict(list)
    for record in records:
        by_position[record.position_key].append(record)

    kept: list[Record] = []
    cross_partition_keys = 0
    cross_partition_records = 0
    within_partition_records = 0
    for position_key in sorted(by_position):
        candidates = sorted(by_position[position_key], key=lambda record: record.record_id)
        partitions = {record.partition for record in candidates}
        if len(partitions) > 1:
            cross_partition_keys += 1
            cross_partition_records += len(candidates)
            continue
        kept.append(candidates[0])
        within_partition_records += len(candidates) - 1

    return (
        sorted(kept, key=lambda record: record.record_id),
        {
            "input_records": sum(len(values) for values in by_position.values()),
            "unique_position_keys": len(by_position),
            "within_partition_records_removed": within_partition_records,
            "cross_partition_keys_removed": cross_partition_keys,
            "cross_partition_records_removed": cross_partition_records,
            "records_kept": len(kept),
        },
    )


def write_partitions(
    directory: Path, partitioned: dict[str, list[Record]]
) -> dict[str, dict[str, Any]]:
    artifacts: dict[str, dict[str, Any]] = {}
    for partition in PARTITIONS:
        path = directory / f"{partition}.input.tsv"
        lines = ["# record_id\ttarget\tFEN"]
        for record in partitioned[partition]:
            lines.append(
                f"{record.record_id}\t{format_target(record.target)}\t{record.fen}"
            )
        write_text(path, "\n".join(lines) + "\n")
        digest, byte_count = sha256_file(path)
        artifacts[partition] = {
            "path": path.name,
            "sha256": digest,
            "bytes": byte_count,
            "lines": len(lines),
            "records": len(lines) - 1,
        }
    return artifacts


def make_manifest(
    config: BuildConfig,
    sources: list[dict[str, Any]],
    sampled_records: list[Record],
    partitioned: dict[str, list[Record]],
    duplicate_summary: dict[str, int],
    artifacts: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    script_hash, _ = sha256_file(Path(__file__))
    record_summary: dict[str, Any] = {"partitions": {}}
    for partition in PARTITIONS:
        values = partitioned[partition]
        record_summary["partitions"][partition] = {
            **artifacts[partition],
            "targets": sorted_counter(Counter(format_target(item.target) for item in values)),
            "side_to_move": sorted_counter(
                Counter(item.fen.split()[1] for item in values)
            ),
            "sources": sorted_counter(Counter(item.source_id for item in values)),
            "opening_pairs": len({(item.source_id, item.pair_index) for item in values}),
        }
    record_summary["sampled_before_deduplication"] = len(sampled_records)
    record_summary["deduplication"] = duplicate_summary

    if config.fixed_partition is None:
        split = {
            "seed": config.seed,
            "group_key": "canonical first-four-field starting FEN",
            "train_percent": config.train_percent,
            "validation_percent": config.validation_percent,
            "holdout_percent": 100
            - config.train_percent
            - config.validation_percent,
        }
    else:
        split = {
            "mode": "fixed-partition-v1",
            "group_key": "canonical first-four-field starting FEN",
            "partition": config.fixed_partition,
            "seed": config.seed,
        }

    return {
        "schema": CORPUS_SCHEMA,
        "created_utc": datetime.now(timezone.utc).isoformat(),
        "output_dir": relative(config.output_dir, config.repo_root),
        "git_commit": git_commit(config.repo_root),
        "builder": {
            "path": manifest_path(Path(__file__), config.repo_root),
            "sha256": script_hash,
            "python": sys.version.split()[0],
            "python_chess": chess.__version__,
            "platform": platform.platform(),
            "machine": platform.machine(),
        },
        "split": split,
        "sampling": {
            "mode": (
                "single-hash-v1"
                if config.samples_per_game == 1
                else "pair-balanced-gap-hash-v1"
            ),
            "selector_schema": (
                CORPUS_SCHEMA
                if config.samples_per_game == 1
                else DENSE_SAMPLING_SCHEMA
            ),
            "min_recorded_ply": config.min_ply,
            "tail_plies_excluded": config.tail_plies,
            "max_positions_per_game": config.samples_per_game,
            "min_sample_gap_plies": config.min_sample_gap,
            "pair_balanced_record_count": True,
            "reject_in_check": True,
            "reject_any_legal_capture_or_promotion": True,
            "position_key": "canonical first four FEN fields",
            "target": "White-relative WDL in {0,0.5,1}",
        },
        "sources": sources,
        "records": record_summary,
    }


def canonical_position_key(fen: str) -> str:
    fields = fen.split()
    if len(fields) < 4:
        raise CorpusError(f"FEN has fewer than four fields: {fen!r}")
    return " ".join(fields[:4])


def format_target(target: float) -> str:
    return {0.0: "0", 0.5: "0.5", 1.0: "1"}[target]


def stable_digest(*parts: str) -> bytes:
    digest = hashlib.sha256()
    for part in parts:
        encoded = part.encode("utf-8")
        digest.update(len(encoded).to_bytes(8, "big"))
        digest.update(encoded)
    return digest.digest()


def sha256_file(path: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            digest.update(chunk)
            size += len(chunk)
    return digest.hexdigest(), size


def git_commit(repo_root: Path) -> str:
    result = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=repo_root,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        check=False,
    )
    return result.stdout.strip() if result.returncode == 0 else "unavailable"


def relative(path: Path, repo_root: Path) -> str:
    try:
        return path.resolve().relative_to(repo_root.resolve()).as_posix()
    except ValueError as error:
        raise CorpusError(f"path is outside repository: {path}") from error


def manifest_path(path: Path, repo_root: Path) -> str:
    try:
        return relative(path, repo_root)
    except CorpusError:
        return path.name


def sorted_counter(counter: Counter[str]) -> dict[str, int]:
    return {key: counter[key] for key in sorted(counter)}


def write_text(path: Path, text: str) -> None:
    with path.open("w", encoding="utf-8", newline="\n") as handle:
        handle.write(text)


if __name__ == "__main__":
    raise SystemExit(main())
