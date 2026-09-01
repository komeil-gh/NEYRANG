#!/usr/bin/env python3
"""Independently replay and audit a deterministic NEYRANG SANJ corpus."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
import sys
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Any

import chess
import chess.pgn


PARTITIONS = ("train", "validation", "holdout")
DENSE_SAMPLING_SCHEMA = "neyrang-sanj-dense-sampling-v1"
RESULT_TARGET = {"1-0": "1", "0-1": "0", "1/2-1/2": "0.5"}
RECORD_ID = re.compile(
    r"^(?P<source>[A-Za-z0-9][A-Za-z0-9._-]*):"
    r"pair-(?P<pair>[0-9]{6,}):game-(?P<game>[12]):ply-(?P<ply>[0-9]{3,})$"
)


class AuditError(RuntimeError):
    """A fail-closed corpus audit error."""


@dataclass(frozen=True)
class ExpectedRecord:
    partition: str
    record_id: str
    target: str
    fen: str
    position_key: str
    source_id: str
    pair_index: int

    def line(self) -> str:
        return f"{self.record_id}\t{self.target}\t{self.fen}"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("corpus_dir", type=Path)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    repo_root = Path(__file__).resolve().parent.parent
    try:
        corpus_dir = inside_repo(args.corpus_dir, repo_root, "corpus directory")
        summary = audit_corpus(repo_root, corpus_dir)
    except (AuditError, OSError, ValueError, json.JSONDecodeError) as error:
        print(f"audit-sanj-corpus: {error}", file=sys.stderr)
        return 1
    print(json.dumps(summary, sort_keys=True))
    return 0


def audit_corpus(repo_root: Path, corpus_dir: Path) -> dict[str, Any]:
    manifest_path = corpus_dir / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    schema = require_string(manifest, "schema")
    if schema != "neyrang-sanj-corpus-v1":
        raise AuditError(f"unsupported corpus schema {schema!r}")

    split = require_mapping(manifest, "split")
    seed = require_string(split, "seed")
    train_percent = require_int(split, "train_percent")
    validation_percent = require_int(split, "validation_percent")
    if train_percent <= 0 or validation_percent <= 0:
        raise AuditError("split percentages must be positive")
    if train_percent + validation_percent >= 100:
        raise AuditError("train plus validation percentages must be below 100")

    sampling = require_mapping(manifest, "sampling")
    min_ply = require_int(sampling, "min_recorded_ply")
    tail_plies = require_int(sampling, "tail_plies_excluded")
    samples_per_game = require_int(sampling, "max_positions_per_game")
    if samples_per_game <= 0:
        raise AuditError("max positions per game must be positive")
    if samples_per_game == 1:
        min_sample_gap = sampling.get("min_sample_gap_plies", 0)
        mode = sampling.get("mode", "single-hash-v1")
        selector_schema = sampling.get("selector_schema", schema)
        pair_balanced = sampling.get("pair_balanced_record_count", True)
    else:
        min_sample_gap = require_int(sampling, "min_sample_gap_plies")
        mode = require_string(sampling, "mode")
        selector_schema = require_string(sampling, "selector_schema")
        pair_balanced = sampling.get("pair_balanced_record_count")
    if not isinstance(min_sample_gap, int) or isinstance(min_sample_gap, bool):
        raise AuditError("min_sample_gap_plies must be an integer")
    if not isinstance(mode, str) or not mode:
        raise AuditError("mode must be a non-empty string")
    if not isinstance(selector_schema, str) or not selector_schema:
        raise AuditError("selector_schema must be a non-empty string")
    if min_sample_gap < 0:
        raise AuditError("minimum sample gap must be non-negative")
    if samples_per_game == 1:
        if min_sample_gap != 0 or mode != "single-hash-v1" or selector_schema != schema:
            raise AuditError("invalid single-sample manifest")
    elif (
        min_sample_gap == 0
        or mode != "pair-balanced-gap-hash-v1"
        or selector_schema != DENSE_SAMPLING_SCHEMA
    ):
        raise AuditError("invalid dense-sample manifest")
    if pair_balanced is not True:
        raise AuditError("corpus does not require pair-balanced record counts")
    if sampling.get("reject_in_check") is not True:
        raise AuditError("corpus does not require check rejection")
    if sampling.get("reject_any_legal_capture_or_promotion") is not True:
        raise AuditError("corpus does not require tactical-move rejection")

    verify_builder(repo_root, manifest)
    expected: list[ExpectedRecord] = []
    source_summaries: dict[str, dict[str, int]] = {}
    source_entries = manifest.get("sources")
    if not isinstance(source_entries, list) or not source_entries:
        raise AuditError("manifest sources must be a non-empty list")
    seen_sources: set[str] = set()
    for source in source_entries:
        if not isinstance(source, dict):
            raise AuditError("manifest source must be an object")
        source_id = require_string(source, "source_id")
        if source_id in seen_sources:
            raise AuditError(f"duplicate source id {source_id!r}")
        seen_sources.add(source_id)
        source_path = inside_repo(
            Path(require_string(source, "path")), repo_root, f"source {source_id!r}"
        )
        verify_artifact(source_path, source)
        records, evidence = replay_source(
            source_id=source_id,
            source_path=source_path,
            schema=schema,
            seed=seed,
            min_ply=min_ply,
            tail_plies=tail_plies,
            samples_per_game=samples_per_game,
            min_sample_gap=min_sample_gap,
            selector_schema=selector_schema,
            train_percent=train_percent,
            validation_percent=validation_percent,
        )
        expected.extend(records)
        compare_source_evidence(
            source_id,
            source,
            evidence,
            require_dense_fields=samples_per_game > 1,
        )
        source_summaries[source_id] = evidence["counts"]

    deduplicated, deduplication = deduplicate(expected)
    records_manifest = require_mapping(manifest, "records")
    if records_manifest.get("sampled_before_deduplication") != len(expected):
        raise AuditError("sampled-before-deduplication count differs")
    if records_manifest.get("deduplication") != deduplication:
        raise AuditError("deduplication summary differs")

    expected_by_partition = {name: [] for name in PARTITIONS}
    for record in deduplicated:
        expected_by_partition[record.partition].append(record)
    for values in expected_by_partition.values():
        values.sort(key=lambda item: item.record_id)

    partition_manifest = require_mapping(records_manifest, "partitions")
    audited_counts: dict[str, int] = {}
    all_record_ids: set[str] = set()
    all_position_keys: set[str] = set()
    for partition in PARTITIONS:
        entry = require_mapping(partition_manifest, partition)
        path = inside_repo(
            corpus_dir / require_string(entry, "path"),
            repo_root,
            f"{partition} partition",
        )
        verify_artifact(path, entry)
        expected_records = expected_by_partition[partition]
        expected_text = "# record_id\ttarget\tFEN\n"
        if expected_records:
            expected_text += "\n".join(record.line() for record in expected_records) + "\n"
        actual_text = path.read_text(encoding="utf-8")
        if actual_text != expected_text:
            raise AuditError(f"{partition} TSV differs from independent replay")
        verify_partition_summary(partition, entry, expected_records)
        audited_counts[partition] = len(expected_records)
        for record in expected_records:
            if record.record_id in all_record_ids:
                raise AuditError(f"duplicate record id {record.record_id!r}")
            if record.position_key in all_position_keys:
                raise AuditError(f"duplicate position key {record.position_key!r}")
            all_record_ids.add(record.record_id)
            all_position_keys.add(record.position_key)

    return {
        "ok": True,
        "sources": source_summaries,
        "records": audited_counts,
        "unique_record_ids": len(all_record_ids),
        "unique_position_keys": len(all_position_keys),
        "deduplication": deduplication,
    }


def replay_source(
    *,
    source_id: str,
    source_path: Path,
    schema: str,
    seed: str,
    min_ply: int,
    tail_plies: int,
    samples_per_game: int,
    min_sample_gap: int,
    selector_schema: str,
    train_percent: int,
    validation_percent: int,
) -> tuple[list[ExpectedRecord], dict[str, Any]]:
    records: list[ExpectedRecord] = []
    counts: Counter[str] = Counter()
    engines: set[str] = set()
    time_controls: Counter[str] = Counter()
    results: Counter[str] = Counter()
    pair_record_counts: Counter[str] = Counter()
    target_record_counts: Counter[str] = Counter()

    with source_path.open(encoding="utf-8") as handle:
        pair_index = 0
        while True:
            left = chess.pgn.read_game(handle)
            if left is None:
                break
            right = chess.pgn.read_game(handle)
            pair_index += 1
            if right is None:
                raise AuditError(f"{source_id} pair {pair_index}: odd trailing game")
            validate_pair(source_id, pair_index, left, right)
            counts["games_parsed"] += 2
            counts["pairs_parsed"] += 1

            opening_fen = left.headers.get("FEN", chess.STARTING_FEN)
            opening_key = position_key(opening_fen)
            partition = partition_for(
                schema,
                seed,
                opening_key,
                train_percent,
                validation_percent,
            )
            samples: list[list[tuple[int, str]]] = []
            for game_in_pair, game in enumerate((left, right), start=1):
                sample, sample_counts = sample_game(
                    game,
                    schema,
                    seed,
                    source_id,
                    pair_index,
                    game_in_pair,
                    min_ply,
                    tail_plies,
                    samples_per_game,
                    min_sample_gap,
                    selector_schema,
                )
                counts.update(sample_counts)
                samples.append(sample)
                engines.update(
                    value
                    for value in (game.headers.get("White"), game.headers.get("Black"))
                    if value
                )
                time_controls[game.headers.get("TimeControl", "missing")] += 1
                results[game.headers["Result"]] += 1

            paired_count = min(len(samples[0]), len(samples[1]))
            dropped = len(samples[0]) + len(samples[1]) - 2 * paired_count
            if dropped:
                counts["pair_balancing_records_dropped"] += dropped
            if paired_count == 0:
                counts["pairs_without_two_samples"] += 1
                continue
            counts["pairs_sampled"] += 1
            pair_record_counts[str(2 * paired_count)] += 1
            for game_in_pair, (game, game_samples) in enumerate(
                zip((left, right), samples, strict=True), start=1
            ):
                for ply, fen in sorted(game_samples[:paired_count]):
                    record_id = (
                        f"{source_id}:pair-{pair_index:06d}:"
                        f"game-{game_in_pair}:ply-{ply:03d}"
                    )
                    if RECORD_ID.fullmatch(record_id) is None:
                        raise AuditError(f"record id is outside schema: {record_id!r}")
                    records.append(
                        ExpectedRecord(
                            partition=partition,
                            record_id=record_id,
                            target=RESULT_TARGET[game.headers["Result"]],
                            fen=fen,
                            position_key=position_key(fen),
                            source_id=source_id,
                            pair_index=pair_index,
                        )
                    )
                    counts["records_sampled"] += 1
                    target_record_counts[RESULT_TARGET[game.headers["Result"]]] += 1

    return records, {
        "counts": sorted_counter(counts),
        "engines": sorted(engines),
        "time_controls": sorted_counter(time_controls),
        "results": sorted_counter(results),
        "pair_record_counts": sorted_counter(pair_record_counts),
        "target_record_counts": sorted_counter(target_record_counts),
    }


def validate_pair(
    source_id: str,
    pair_index: int,
    left: chess.pgn.Game,
    right: chess.pgn.Game,
) -> None:
    for game_index, game in enumerate((left, right), start=1):
        if game.errors:
            raise AuditError(
                f"{source_id} pair {pair_index} game {game_index}: PGN parse errors"
            )
        if game.headers.get("Result") not in RESULT_TARGET:
            raise AuditError(
                f"{source_id} pair {pair_index} game {game_index}: invalid result"
            )
        if game.headers.get("Termination") != "normal":
            raise AuditError(
                f"{source_id} pair {pair_index} game {game_index}: non-normal termination"
            )
        if game.headers.get("Variant", "Standard") not in {"Standard", "Chess"}:
            raise AuditError(
                f"{source_id} pair {pair_index} game {game_index}: unsupported variant"
            )
        moves = list(game.mainline_moves())
        if game.headers.get("PlyCount", str(len(moves))) != str(len(moves)):
            raise AuditError(
                f"{source_id} pair {pair_index} game {game_index}: PlyCount differs"
            )

    left_fen = left.headers.get("FEN", chess.STARTING_FEN)
    right_fen = right.headers.get("FEN", chess.STARTING_FEN)
    if left_fen != right_fen:
        raise AuditError(f"{source_id} pair {pair_index}: opening FENs differ")
    if left.headers.get("Round") != right.headers.get("Round"):
        raise AuditError(f"{source_id} pair {pair_index}: rounds differ")
    if not (
        left.headers.get("White") == right.headers.get("Black")
        and left.headers.get("Black") == right.headers.get("White")
    ):
        raise AuditError(f"{source_id} pair {pair_index}: colors are not reversed")


def sample_game(
    game: chess.pgn.Game,
    schema: str,
    seed: str,
    source_id: str,
    pair_index: int,
    game_in_pair: int,
    min_ply: int,
    tail_plies: int,
    samples_per_game: int,
    min_sample_gap: int,
    selector_schema: str,
) -> tuple[list[tuple[int, str]], Counter[str]]:
    moves = list(game.mainline_moves())
    board = game.board()
    eligible: list[tuple[int, str]] = []
    counts: Counter[str] = Counter()
    for ply, move in enumerate(moves):
        if ply < min_ply:
            counts["positions_opening_skipped"] += 1
        elif len(moves) - ply < tail_plies:
            counts["positions_tail_skipped"] += 1
        elif board.is_check():
            counts["positions_check_rejected"] += 1
        elif any(board.is_capture(legal) or legal.promotion for legal in board.legal_moves):
            counts["positions_tactical_rejected"] += 1
        else:
            eligible.append((ply, board.fen(en_passant="fen")))
            counts["positions_eligible"] += 1
        board.push(move)

    if not eligible:
        counts["games_without_eligible_position"] += 1
        return [], counts

    if samples_per_game == 1:
        chosen = stable_digest(
            schema,
            seed,
            source_id,
            str(pair_index),
            str(game_in_pair),
            "sample",
        )
        selected = [eligible[int.from_bytes(chosen[:8], "big") % len(eligible)]]
    else:
        ranked = sorted(
            eligible,
            key=lambda sample: stable_digest(
                selector_schema,
                seed,
                source_id,
                str(pair_index),
                str(game_in_pair),
                str(sample[0]),
                "rank",
            ),
        )
        selected = []
        for sample in ranked:
            if all(
                abs(sample[0] - existing[0]) >= min_sample_gap
                for existing in selected
            ):
                selected.append(sample)
                if len(selected) == samples_per_game:
                    break
        counts["positions_selected"] += len(selected)

    counts["games_with_eligible_position"] += 1
    return selected, counts


def partition_for(
    schema: str,
    seed: str,
    opening_key: str,
    train_percent: int,
    validation_percent: int,
) -> str:
    chosen = stable_digest(schema, seed, opening_key, "partition")
    bucket = int.from_bytes(chosen[:8], "big") % 100
    if bucket < train_percent:
        return "train"
    if bucket < train_percent + validation_percent:
        return "validation"
    return "holdout"


def deduplicate(
    records: list[ExpectedRecord],
) -> tuple[list[ExpectedRecord], dict[str, int]]:
    grouped: dict[str, list[ExpectedRecord]] = defaultdict(list)
    for record in records:
        grouped[record.position_key].append(record)
    kept: list[ExpectedRecord] = []
    cross_keys = 0
    cross_records = 0
    within_records = 0
    for key in sorted(grouped):
        candidates = sorted(grouped[key], key=lambda item: item.record_id)
        if len({item.partition for item in candidates}) > 1:
            cross_keys += 1
            cross_records += len(candidates)
            continue
        kept.append(candidates[0])
        within_records += len(candidates) - 1
    return sorted(kept, key=lambda item: item.record_id), {
        "input_records": len(records),
        "unique_position_keys": len(grouped),
        "within_partition_records_removed": within_records,
        "cross_partition_keys_removed": cross_keys,
        "cross_partition_records_removed": cross_records,
        "records_kept": len(kept),
    }


def compare_source_evidence(
    source_id: str,
    manifest: dict[str, Any],
    actual: dict[str, Any],
    *,
    require_dense_fields: bool,
) -> None:
    for field in ("counts", "engines", "time_controls", "results"):
        if manifest.get(field) != actual[field]:
            raise AuditError(f"{source_id} {field} differs from independent replay")
    for field in ("pair_record_counts", "target_record_counts"):
        if field not in manifest:
            if require_dense_fields:
                raise AuditError(f"{source_id} {field} is missing")
            continue
        if manifest[field] != actual[field]:
            raise AuditError(f"{source_id} {field} differs from independent replay")


def verify_partition_summary(
    partition: str,
    manifest: dict[str, Any],
    records: list[ExpectedRecord],
) -> None:
    expected = {
        "lines": len(records) + 1,
        "records": len(records),
        "targets": sorted_counter(Counter(record.target for record in records)),
        "side_to_move": sorted_counter(
            Counter(record.fen.split()[1] for record in records)
        ),
        "sources": sorted_counter(Counter(record.source_id for record in records)),
        "opening_pairs": len(
            {(record.source_id, record.pair_index) for record in records}
        ),
    }
    for field, value in expected.items():
        if manifest.get(field) != value:
            raise AuditError(f"{partition} {field} summary differs")


def verify_builder(repo_root: Path, manifest: dict[str, Any]) -> None:
    builder = require_mapping(manifest, "builder")
    path = inside_repo(
        Path(require_string(builder, "path")), repo_root, "builder path"
    )
    digest, _ = sha256_file(path)
    if digest != require_string(builder, "sha256"):
        raise AuditError("builder hash differs from manifest")


def verify_artifact(path: Path, manifest: dict[str, Any]) -> None:
    digest, byte_count = sha256_file(path)
    if digest != require_string(manifest, "sha256"):
        raise AuditError(f"SHA-256 differs for {path}")
    if byte_count != require_int(manifest, "bytes"):
        raise AuditError(f"byte count differs for {path}")


def inside_repo(path: Path, repo_root: Path, label: str) -> Path:
    candidate = path if path.is_absolute() else repo_root / path
    resolved = candidate.resolve()
    try:
        resolved.relative_to(repo_root.resolve())
    except ValueError as error:
        raise AuditError(f"{label} is outside the repository") from error
    return resolved


def position_key(fen: str) -> str:
    fields = fen.split()
    if len(fields) < 4:
        raise AuditError(f"FEN has fewer than four fields: {fen!r}")
    return " ".join(fields[:4])


def stable_digest(*parts: str) -> bytes:
    result = hashlib.sha256()
    for part in parts:
        encoded = part.encode("utf-8")
        result.update(len(encoded).to_bytes(8, "big"))
        result.update(encoded)
    return result.digest()


def sha256_file(path: Path) -> tuple[str, int]:
    result = hashlib.sha256()
    byte_count = 0
    with path.open("rb") as handle:
        while chunk := handle.read(1024 * 1024):
            result.update(chunk)
            byte_count += len(chunk)
    return result.hexdigest(), byte_count


def sorted_counter(counter: Counter[str]) -> dict[str, int]:
    return {key: counter[key] for key in sorted(counter)}


def require_mapping(mapping: dict[str, Any], name: str) -> dict[str, Any]:
    value = mapping.get(name)
    if not isinstance(value, dict):
        raise AuditError(f"{name} must be an object")
    return value


def require_string(mapping: dict[str, Any], name: str) -> str:
    value = mapping.get(name)
    if not isinstance(value, str) or not value:
        raise AuditError(f"{name} must be a non-empty string")
    return value


def require_int(mapping: dict[str, Any], name: str) -> int:
    value = mapping.get(name)
    if not isinstance(value, int) or isinstance(value, bool):
        raise AuditError(f"{name} must be an integer")
    return value


if __name__ == "__main__":
    raise SystemExit(main())
