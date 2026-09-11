#!/usr/bin/env python3
"""Independently replay and audit a SHEGERD-P1 corpus and its P0 traces."""

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


SCHEMA = "neyrang-shegerd-p1-corpus-v1"
AUDIT_SCHEMA = "neyrang-shegerd-p1-corpus-audit-v1"
SAMPLE_SCHEMA = "neyrang-shegerd-p1-sample-v1"
SPLIT_SCHEMA = "neyrang-shegerd-p1-split-v1"
TRACE_SCHEMA = "neyrang-shegerd-policy-trace-v1"
SOURCE_ID = "shegerd-p1-t6-f1-v1"
SOURCE_ENGINES = ("NEYRANG-T6-P1", "NEYRANG-F1-P1")
SOURCE_COMMITS = {
    "engine_a_git_sha": "7c2b72bd20dbd4ac8f744aa7d7af5e1beab32b52",
    "engine_b_git_sha": "52737b51a1d7b68c3c52b6bf819aa2a292e94153",
}
PARTITIONS = ("train", "validation", "sealed_holdout")
TRACE_HEADER = (
    "schema\trecord_id\tgroup_id\tcandidate_move\tselected\tstage\t"
    "from_normalized\tto_normalized\tmover\tvictim\tpromotion\tphase\t"
    "previous_to_normalized\tsee_bucket\tfen"
)
RECORD_ID = re.compile(
    r"^(?P<source>[A-Za-z0-9][A-Za-z0-9._-]*):pair-(?P<pair>[0-9]{6,}):"
    r"game-(?P<game>[12]):ply-(?P<ply>[0-9]{3,})$"
)


class AuditError(RuntimeError):
    """A fail-closed P1 corpus audit error."""


@dataclass(frozen=True)
class Sample:
    ply: int
    fen: str
    previous_to: str


@dataclass(frozen=True)
class ExpectedRecord:
    partition: str
    record_id: str
    group_id: str
    position_key: str
    previous_to: str
    fen: str


@dataclass(frozen=True)
class LabelRecord:
    record_id: str
    group_id: str
    previous_to: str
    teacher_move: str
    fen: str


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--source", required=True, type=Path)
    parser.add_argument("--source-audit", required=True, type=Path)
    parser.add_argument("--corpus-dir", required=True, type=Path)
    parser.add_argument("--trace-dir", required=True, type=Path)
    return parser.parse_args()


def canonical_fen(fen: str) -> str:
    fields = fen.split()
    if len(fields) < 4:
        raise AuditError("FEN has fewer than four fields")
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


def round_key(value: str) -> tuple[int, tuple[int, ...] | str]:
    parts = value.split(".")
    return (
        (0, tuple(int(part) for part in parts))
        if parts and all(part.isdigit() for part in parts)
        else (1, value)
    )


def read_pairs(
    path: Path, time_control: str, expected_engines: tuple[str, str] | None
) -> list[tuple[int, chess.pgn.Game, chess.pgn.Game]]:
    by_round: dict[str, list[chess.pgn.Game]] = defaultdict(list)
    with path.open(encoding="utf-8") as stream:
        game_number = 0
        while game := chess.pgn.read_game(stream):
            game_number += 1
            if game.errors:
                raise AuditError(f"game {game_number}: PGN parse errors")
            round_id = game.headers.get("Round")
            if not round_id:
                raise AuditError(f"game {game_number}: missing round header")
            by_round[round_id].append(game)
    if game_number == 0 or game_number % 2:
        raise AuditError("source must contain a nonzero even game count")
    pairs: list[tuple[int, chess.pgn.Game, chess.pgn.Game]] = []
    for pair_index, round_id in enumerate(sorted(by_round, key=round_key), start=1):
        games = by_round[round_id]
        if len(games) != 2:
            raise AuditError(
                f"round {round_id!r} contains {len(games)} games, expected two"
            )
        left, right = games
        validate_pair(pair_index, left, right, time_control, expected_engines)
        pairs.append((pair_index, left, right))
    return pairs


def validate_pair(
    index: int,
    left: chess.pgn.Game,
    right: chess.pgn.Game,
    time_control: str,
    expected_engines: tuple[str, str] | None,
) -> None:
    for game_number, game in enumerate((left, right), start=1):
        if game.headers.get("Result") not in {"1-0", "0-1", "1/2-1/2"}:
            raise AuditError(f"pair {index} game {game_number}: invalid result")
        if game.headers.get("Termination") != "normal":
            raise AuditError(f"pair {index} game {game_number}: termination is not normal")
        if game.headers.get("TimeControl") != time_control:
            raise AuditError(f"pair {index} game {game_number}: unexpected time control")
        if game.headers.get("Variant", "Standard") not in {"Standard", "Chess"}:
            raise AuditError(f"pair {index} game {game_number}: unsupported variant")
        moves = list(game.mainline_moves())
        if game.headers.get("PlyCount") not in {None, str(len(moves))}:
            raise AuditError(f"pair {index} game {game_number}: PlyCount differs")
        if expected_engines is not None and {
            game.headers.get("White"),
            game.headers.get("Black"),
        } != set(expected_engines):
            raise AuditError(f"pair {index} game {game_number}: engine labels differ")
    if canonical_fen(left.headers.get("FEN", chess.STARTING_FEN)) != canonical_fen(
        right.headers.get("FEN", chess.STARTING_FEN)
    ):
        raise AuditError(f"pair {index}: opening FEN differs")
    if not (
        left.headers.get("White") == right.headers.get("Black")
        and left.headers.get("Black") == right.headers.get("White")
    ):
        raise AuditError(f"pair {index}: colors are not reversed")


def sample_game(
    game: chess.pgn.Game,
    source_id: str,
    pair_index: int,
    game_index: int,
    sample_seed: str,
    min_ply: int,
    tail_plies: int,
    samples_per_game: int,
    min_sample_gap: int,
) -> tuple[list[Sample], Counter[str]]:
    board = game.board()
    moves = list(game.mainline_moves())
    previous_to = "-"
    eligible: list[Sample] = []
    counters: Counter[str] = Counter()
    for ply, move in enumerate(moves):
        if ply < min_ply:
            counters["opening_skipped"] += 1
        elif len(moves) - ply < tail_plies:
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
        key=lambda sample: digest(
            SAMPLE_SCHEMA,
            sample_seed,
            source_id,
            str(pair_index),
            str(game_index),
            str(sample.ply),
        ),
    )
    selected: list[Sample] = []
    for sample in ranked:
        if all(
            abs(sample.ply - existing.ply) >= min_sample_gap
            for existing in selected
        ):
            selected.append(sample)
            if len(selected) == samples_per_game:
                break
    counters["selected_before_pair_balance"] = len(selected)
    return selected, counters


def assign_partitions(openings: set[str], seed: str) -> dict[str, str]:
    ranked = sorted(openings, key=lambda item: (digest(SPLIT_SCHEMA, seed, item), item))
    train_end = len(ranked) * 80 // 100
    validation_end = len(ranked) * 90 // 100
    return {
        opening: (
            "train"
            if index < train_end
            else "validation"
            if index < validation_end
            else "sealed_holdout"
        )
        for index, opening in enumerate(ranked)
    }


def replay(
    source: Path,
    source_id: str,
    sampling: dict[str, Any],
    split: dict[str, Any],
    expected_engines: tuple[str, str] | None,
) -> tuple[list[ExpectedRecord], dict[str, Any]]:
    pairs = read_pairs(source, "0.5+0.005", expected_engines)
    opening_by_pair = {
        index: canonical_fen(left.headers.get("FEN", chess.STARTING_FEN))
        for index, left, _ in pairs
    }
    partitions = assign_partitions(set(opening_by_pair.values()), require_string(split, "seed"))
    records: list[ExpectedRecord] = []
    counters: Counter[str] = Counter()
    engines: set[str] = set()
    for pair_index, left, right in pairs:
        engines.update(
            name
            for game in (left, right)
            for name in (game.headers.get("White"), game.headers.get("Black"))
            if name
        )
        selected: list[list[Sample]] = []
        for game_index, game in enumerate((left, right), start=1):
            samples, sample_counts = sample_game(
                game,
                source_id,
                pair_index,
                game_index,
                require_string(sampling, "seed"),
                require_int(sampling, "min_ply"),
                require_int(sampling, "tail_plies"),
                require_int(sampling, "samples_per_game"),
                require_int(sampling, "min_sample_gap"),
            )
            selected.append(samples)
            counters.update(sample_counts)
        count = min(len(selected[0]), len(selected[1]))
        counters["pair_balance_dropped"] += len(selected[0]) + len(selected[1]) - 2 * count
        if count == 0:
            counters["pairs_without_balanced_samples"] += 1
            continue
        counters["pairs_sampled"] += 1
        opening = opening_by_pair[pair_index]
        group_id = hashlib.sha256(opening.encode("utf-8")).hexdigest()
        for game_index, samples in enumerate(selected, start=1):
            for sample in sorted(samples[:count], key=lambda item: item.ply):
                record_id = (
                    f"{source_id}:pair-{pair_index:06d}:"
                    f"game-{game_index}:ply-{sample.ply:03d}"
                )
                if RECORD_ID.fullmatch(record_id) is None:
                    raise AuditError(f"record ID is outside schema: {record_id!r}")
                records.append(
                    ExpectedRecord(
                        partitions[opening],
                        record_id,
                        group_id,
                        canonical_fen(sample.fen),
                        sample.previous_to,
                        sample.fen,
                    )
                )
    retained, deduplication = deduplicate(records)
    return retained, {
        "pairs": len(pairs),
        "engines": sorted(engines),
        "sampling": dict(sorted(counters.items())),
        "deduplication": deduplication,
        "partition_opening_groups": dict(Counter(partitions.values())),
    }


def deduplicate(
    records: list[ExpectedRecord],
) -> tuple[list[ExpectedRecord], dict[str, int]]:
    grouped: dict[str, list[ExpectedRecord]] = defaultdict(list)
    for record in records:
        grouped[record.position_key].append(record)
    kept: list[ExpectedRecord] = []
    within = cross_keys = cross_records = 0
    for candidates in grouped.values():
        partitions = {candidate.partition for candidate in candidates}
        if len(partitions) > 1:
            cross_keys += 1
            cross_records += len(candidates)
            continue
        kept.append(min(candidates, key=lambda candidate: candidate.record_id))
        within += len(candidates) - 1
    return sorted(kept, key=lambda item: item.record_id), {
        "input_records": len(records),
        "unique_position_keys": len(grouped),
        "within_partition_records_removed": within,
        "cross_partition_keys_removed": cross_keys,
        "cross_partition_records_removed": cross_records,
        "records_kept": len(kept),
    }


def inside_directory(path: Path, directory: Path, label: str) -> Path:
    resolved = path.resolve()
    try:
        resolved.relative_to(directory.resolve())
    except ValueError as error:
        raise AuditError(f"{label} is outside its registered directory") from error
    return resolved


def verify_artifact(path: Path, entry: dict[str, Any]) -> tuple[str, int]:
    actual_hash, actual_bytes = sha256_file(path)
    if actual_hash != require_string(entry, "sha256"):
        raise AuditError(f"SHA-256 differs for {path}")
    if actual_bytes != require_int(entry, "bytes"):
        raise AuditError(f"byte count differs for {path}")
    return actual_hash, actual_bytes


def audit_source_report(
    source: Path,
    source_audit: Path,
    source_entry: dict[str, Any],
    teacher_entry: dict[str, Any],
    enforce_campaign: bool,
) -> dict[str, Any] | None:
    audit_entry = source_entry.get("audit")
    if not enforce_campaign and audit_entry is None:
        return None
    if not isinstance(audit_entry, dict):
        raise AuditError("source audit manifest entry is missing")
    audit_hash, audit_bytes = verify_artifact(source_audit, audit_entry)
    report = json.loads(source_audit.read_text(encoding="utf-8"))
    if report.get("format") != "neyrang-match-audit-v1" or report.get("ok") is not True:
        raise AuditError("source audit is unsupported or failed")
    audited_pgn = Path(str(report.get("pgn", "")))
    audited_pgn = (
        audited_pgn
        if audited_pgn.is_absolute()
        else Path(__file__).resolve().parent.parent / audited_pgn
    )
    if audited_pgn.resolve() != source.resolve():
        raise AuditError("source audit belongs to another PGN")
    if report.get("candidate") != SOURCE_ENGINES[0] or report.get("opponent") != SOURCE_ENGINES[1]:
        raise AuditError("source audit engine labels differ")
    if report.get("games") != 2000 or report.get("pairs") != 1000:
        raise AuditError("source audit game or pair count differs")
    if report.get("unique_opening_fens") != 1000:
        raise AuditError("source audit opening count differs")
    expected_openings = report.get("expected_openings")
    if not isinstance(expected_openings, dict) or expected_openings.get("pairs") != 1000:
        raise AuditError("source audit did not verify the frozen opening list")
    if report.get("terminations") != {"normal": 2000} or report.get(
        "time_controls"
    ) != {"0.5+0.005": 2000}:
        raise AuditError("source audit termination or time control differs")
    if report.get("telemetry_missing") != {} or report.get("allowed_log_warnings") != []:
        raise AuditError("source audit telemetry or warning evidence differs")
    anomalies = report.get("log_anomaly_counts")
    if (
        not isinstance(anomalies, dict)
        or not anomalies
        or any(value != 0 for value in anomalies.values())
    ):
        raise AuditError("source audit did not prove zero log anomalies")
    metadata = report.get("metadata")
    if not isinstance(metadata, dict):
        raise AuditError("source audit metadata is missing")
    expected_metadata = {
        "format": "neyrang-match-v1",
        "engine_a_name": SOURCE_ENGINES[0],
        "engine_b_name": SOURCE_ENGINES[1],
        **SOURCE_COMMITS,
        "games": "2000",
        "pairs": "1000",
        "opening_order": "sequential",
        "opening_seed": "2026090501",
        "limit_mode": "time",
        "time_control": "0.5+0.005",
        "thread_mode": "shared",
        "threads": "1",
        "engine_a_threads": "1",
        "engine_b_threads": "1",
        "hash_mb": "64",
        "move_overhead_ms": "100",
        "concurrency": "1",
        "time_margin_ms": "0",
        "show_latency": "1",
        "strict": "1",
        "warning_policy": "reject-all",
        "adjudication": "fastchess-default",
        "status": "completed",
    }
    if any(metadata.get(key) != value for key, value in expected_metadata.items()):
        raise AuditError("source audit metadata differs from the frozen campaign")
    for field, value in {
        "format": report["format"],
        "candidate": report["candidate"],
        "opponent": report["opponent"],
        **SOURCE_COMMITS,
    }.items():
        expected = metadata.get(field, value) if field in SOURCE_COMMITS else value
        if audit_entry.get(field) != expected:
            raise AuditError(f"source audit {field} identity differs")
    for field in (
        "engine_a_sha256",
        "engine_b_sha256",
        "fastchess_sha256",
        "openings_sha256",
    ):
        value = metadata.get(field, "")
        if re.fullmatch(r"[0-9a-f]{64}", value) is None or audit_entry.get(field) != value:
            raise AuditError(f"source audit {field} identity differs")
    if teacher_entry.get("sha256") != metadata["engine_a_sha256"]:
        raise AuditError("teacher identity differs from source engine A")
    return {"bytes": audit_bytes, "sha256": audit_hash}


def read_labels(path: Path, expected: list[ExpectedRecord]) -> list[LabelRecord]:
    lines = path.read_text(encoding="utf-8").splitlines()
    if len(lines) != len(expected):
        raise AuditError(f"{path.name} record count differs from replay")
    labels: list[LabelRecord] = []
    for line_number, (line, record) in enumerate(zip(lines, expected, strict=True), 1):
        fields = line.split("\t")
        if len(fields) != 5 or any(not field or field.strip() != field for field in fields):
            raise AuditError(f"{path.name}:{line_number}: malformed label row")
        label = LabelRecord(*fields)
        if (
            label.record_id != record.record_id
            or label.group_id != record.group_id
            or label.previous_to != record.previous_to
            or label.fen != record.fen
        ):
            raise AuditError(f"{path.name}:{line_number}: label envelope differs from replay")
        board = chess.Board(label.fen)
        try:
            teacher_move = chess.Move.from_uci(label.teacher_move)
        except ValueError as error:
            raise AuditError(f"{path.name}:{line_number}: invalid teacher move") from error
        if teacher_move not in board.legal_moves:
            raise AuditError(f"{path.name}:{line_number}: teacher move is not legal")
        labels.append(label)
    return labels


def normalized(square: int, turn: chess.Color) -> int:
    return square if turn == chess.WHITE else square ^ 56


def phase(board: chess.Board) -> int:
    material = sum(
        value
        * (len(board.pieces(piece, chess.WHITE)) + len(board.pieces(piece, chess.BLACK)))
        for piece, value in (
            (chess.KNIGHT, 3),
            (chess.BISHOP, 3),
            (chess.ROOK, 5),
            (chess.QUEEN, 9),
        )
    )
    return 0 if material >= 52 else 1 if material >= 24 else 2


def validate_trace_row(
    fields: list[str], label: LabelRecord, board: chess.Board
) -> tuple[str, bool]:
    if len(fields) != 15 or fields[0] != TRACE_SCHEMA:
        raise AuditError(f"trace row for {label.record_id!r} has invalid schema or width")
    if (
        fields[1] != label.record_id
        or fields[2] != label.group_id
        or fields[14] != label.fen
    ):
        raise AuditError(f"trace row for {label.record_id!r} changes record identity")
    try:
        move = chess.Move.from_uci(fields[3])
        numbers = [int(fields[index]) for index in range(6, 14)]
    except ValueError as error:
        raise AuditError(f"trace row for {label.record_id!r} contains an invalid value") from error
    if move not in board.legal_moves or fields[4] not in {"0", "1"}:
        raise AuditError(f"trace row for {label.record_id!r} contains an illegal candidate")
    tactical = board.is_capture(move) or move.promotion is not None
    victim = (
        chess.PAWN
        if board.is_en_passant(move)
        else (board.piece_type_at(move.to_square) or 0)
    )
    previous = (
        -1
        if label.previous_to == "-"
        else normalized(chess.parse_square(label.previous_to), board.turn)
    )
    expected = [
        normalized(move.from_square, board.turn),
        normalized(move.to_square, board.turn),
        board.piece_type_at(move.from_square),
        victim,
        move.promotion or 0,
        phase(board),
        previous,
    ]
    if fields[5] != ("tactical" if tactical else "quiet") or numbers[:7] != expected:
        raise AuditError(f"trace row for {label.record_id!r} has incorrect derived features")
    if numbers[7] not in {-2, -1, 0, 1, 2} or (not tactical and numbers[7] != 0):
        raise AuditError(f"trace row for {label.record_id!r} has an invalid SEE bucket")
    return move.uci(), fields[4] == "1"


def audit_trace(path: Path, labels: list[LabelRecord]) -> dict[str, int | str]:
    trace_hash, trace_bytes = sha256_file(path)
    lines = path.read_text(encoding="utf-8").splitlines()
    if not lines or lines[0] != TRACE_HEADER:
        raise AuditError(f"{path.name}: trace header differs")
    grouped: list[tuple[str, list[list[str]]]] = []
    seen: set[str] = set()
    for line_number, line in enumerate(lines[1:], 2):
        fields = line.split("\t")
        record_id = fields[1] if len(fields) > 1 else ""
        if not grouped or grouped[-1][0] != record_id:
            if record_id in seen:
                raise AuditError(f"{path.name}:{line_number}: noncontiguous record rows")
            seen.add(record_id)
            grouped.append((record_id, []))
        grouped[-1][1].append(fields)
    if [record_id for record_id, _ in grouped] != [label.record_id for label in labels]:
        raise AuditError(f"{path.name}: trace record sequence differs from labels")
    for (record_id, rows), label in zip(grouped, labels, strict=True):
        board = chess.Board(label.fen)
        candidates: set[str] = set()
        selected: list[str] = []
        for fields in rows:
            candidate, is_selected = validate_trace_row(fields, label, board)
            if candidate in candidates:
                raise AuditError(f"{path.name}: duplicate candidate for {record_id!r}")
            candidates.add(candidate)
            if is_selected:
                selected.append(candidate)
        legal = {move.uci() for move in board.legal_moves}
        if candidates != legal:
            raise AuditError(f"{path.name}: incomplete legal sibling set for {record_id!r}")
        if selected != [label.teacher_move]:
            raise AuditError(f"{path.name}: selected move differs for {record_id!r}")
    return {
        "records": len(labels),
        "data_rows": len(lines) - 1,
        "columns": 15,
        "bytes": trace_bytes,
        "sha256": trace_hash,
    }


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


def audit(
    source: Path,
    source_audit: Path | None,
    corpus_dir: Path,
    trace_dir: Path,
    *,
    enforce_campaign: bool = True,
) -> dict[str, Any]:
    manifest_path = inside_directory(
        corpus_dir / "manifest.json", corpus_dir, "manifest"
    )
    manifest_hash, manifest_bytes = sha256_file(manifest_path)
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest.get("schema") != SCHEMA:
        raise AuditError("unsupported corpus manifest schema")
    source_entry = require_mapping(manifest, "source")
    source_id = require_string(source_entry, "id")
    if enforce_campaign and source_id != SOURCE_ID:
        raise AuditError("corpus source ID differs from the frozen campaign")
    verify_artifact(source, source_entry)
    teacher_entry = require_mapping(manifest, "teacher")
    expected_teacher = {
        "nodes": 80000,
        "threads": 1,
        "hash_mb": 64,
        "clear_hash_per_position": True,
    }
    if any(teacher_entry.get(key) != value for key, value in expected_teacher.items()):
        raise AuditError("teacher contract differs")
    if teacher_entry.get("reset_method") not in {
        "clear-hash-button",
        "ucinewgame-per-position",
    }:
        raise AuditError("teacher reset method differs")
    if source_audit is None and source_entry.get("audit") is not None:
        raise AuditError("source audit input is required")
    source_audit_evidence = (
        audit_source_report(
            source, source_audit, source_entry, teacher_entry, enforce_campaign
        )
        if source_audit is not None
        else None
    )
    builder = require_mapping(manifest, "builder")
    repo_root = Path(__file__).resolve().parent.parent
    builder_path = inside_directory(repo_root / require_string(builder, "path"), repo_root, "builder")
    verify_artifact(builder_path, builder)
    runtime = require_mapping(manifest, "runtime")
    for field in ("python", "implementation", "python_chess", "system", "machine"):
        require_string(runtime, field)
    sampling = require_mapping(manifest, "sampling")
    split = require_mapping(manifest, "split")
    expected_sampling = {
        "schema": SAMPLE_SCHEMA,
        "seed": "20260905-shegerd-p1-sample-v1",
        "min_ply": 16,
        "tail_plies": 8,
        "samples_per_game": 4,
        "min_sample_gap": 8,
        "pair_balanced": True,
    }
    expected_split = {
        "schema": SPLIT_SCHEMA,
        "seed": "20260905-shegerd-p1-split-v1",
        "unit": "canonical starting FEN",
        "fractions": {"train": 0.8, "validation": 0.1, "sealed_holdout": 0.1},
    }
    if sampling != expected_sampling or split != expected_split:
        raise AuditError("sampling or split contract differs")
    expected_engines = SOURCE_ENGINES if enforce_campaign else None
    records, extraction = replay(source, source_id, sampling, split, expected_engines)
    if manifest.get("extraction") != extraction:
        raise AuditError("extraction summary differs from independent replay")
    if (
        source_entry.get("pairs") != extraction["pairs"]
        or source_entry.get("engines") != extraction["engines"]
    ):
        raise AuditError("source summary differs from independent replay")
    if enforce_campaign and (
        extraction["pairs"] != 1000
        or extraction["engines"] != sorted(SOURCE_ENGINES)
    ):
        raise AuditError("source campaign size or engines differ")
    expected_by_partition = {partition: [] for partition in PARTITIONS}
    for record in records:
        expected_by_partition[record.partition].append(record)
    artifact_entries = require_mapping(manifest, "artifacts")
    partition_reports: dict[str, Any] = {}
    all_ids: set[str] = set()
    all_positions: set[str] = set()
    for partition in PARTITIONS:
        expected = expected_by_partition[partition]
        entry = require_mapping(artifact_entries, partition)
        expected_name = f"{partition}.labels.tsv"
        if require_string(entry, "path") != expected_name:
            raise AuditError(f"{partition} label path differs")
        label_path = inside_directory(
            corpus_dir / expected_name, corpus_dir, f"{partition} labels"
        )
        label_hash, label_bytes = verify_artifact(label_path, entry)
        labels = read_labels(label_path, expected)
        groups = len({record.group_id for record in expected})
        if entry.get("records") != len(labels) or entry.get("groups") != groups:
            raise AuditError(f"{partition} count summary differs")
        for record in expected:
            if record.record_id in all_ids or record.position_key in all_positions:
                raise AuditError("record or canonical position leaks across partitions")
            all_ids.add(record.record_id)
            all_positions.add(record.position_key)
        trace_path = inside_directory(
            trace_dir / f"{partition}.trace.tsv", trace_dir, f"{partition} trace"
        )
        trace_report = audit_trace(trace_path, labels)
        label_report: dict[str, Any] = {
            "records": len(labels),
            "bytes": label_bytes,
            "sha256": label_hash,
        }
        if partition != "sealed_holdout":
            label_report["groups"] = groups
        partition_reports[partition] = {"labels": label_report, "trace": trace_report}
    return {
        "schema": AUDIT_SCHEMA,
        "ok": True,
        "source": {
            "bytes": require_int(source_entry, "bytes"),
            "sha256": require_string(source_entry, "sha256"),
            "games": extraction["pairs"] * 2,
            "pairs": extraction["pairs"],
        },
        "source_audit": source_audit_evidence,
        "manifest": {"bytes": manifest_bytes, "sha256": manifest_hash},
        "partitions": partition_reports,
        "unique_record_ids": len(all_ids),
        "unique_position_keys": len(all_positions),
    }


def main() -> int:
    args = parse_args()
    try:
        summary = audit(
            args.source.resolve(),
            args.source_audit.resolve(),
            args.corpus_dir.resolve(),
            args.trace_dir.resolve(),
        )
    except (AuditError, OSError, ValueError, json.JSONDecodeError) as error:
        print(f"audit-shegerd-policy-corpus: {error}", file=sys.stderr)
        return 1
    print(json.dumps(summary, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
