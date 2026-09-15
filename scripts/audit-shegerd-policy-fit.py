#!/usr/bin/env python3
"""Independently audit a SHEGERD-P1 quantized fit and repeated fit."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import struct
import sys
import zlib
from collections import Counter
from dataclasses import dataclass
from pathlib import Path


TRACE_SCHEMA = "neyrang-shegerd-policy-trace-v1"
REPORT_SCHEMA = "neyrang-shegerd-policy-fit-result-v2"
AUDIT_SCHEMA = "neyrang-shegerd-policy-fit-audit-v2"
HEADER = (
    "schema", "record_id", "group_id", "candidate_move", "selected", "stage",
    "from_normalized", "to_normalized", "mover", "victim", "promotion", "phase",
    "previous_to_normalized", "see_bucket", "fen",
)
FAMILY_SIZES = (64 * 64, 6 * 64, 7 * 7, 3, 65 * 64, 5)
OFFSETS = tuple(sum(FAMILY_SIZES[:index]) for index in range(len(FAMILY_SIZES)))
BINARY_HEADER = struct.Struct("<8s8I")
MAGIC = b"NYRSHGP1"


class AuditError(RuntimeError):
    """A fail-closed P1 fit audit error."""


@dataclass(frozen=True)
class Candidate:
    move: str
    selected: bool
    move_class: str
    features: tuple[int, int, int, int, int, int]


@dataclass(frozen=True)
class Decision:
    record_id: str
    group_id: str
    candidates: tuple[Candidate, ...]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--validation", type=Path, required=True)
    parser.add_argument("--float", dest="float_path", type=Path, required=True)
    parser.add_argument("--quantized", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--repeat-float", type=Path, required=True)
    parser.add_argument("--repeat-quantized", type=Path, required=True)
    parser.add_argument("--repeat-report", type=Path, required=True)
    return parser.parse_args()


def sha256_file(path: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
            size += len(chunk)
    return digest.hexdigest(), size


def bounded_int(text: str, lower: int, upper: int, line: int, field: str) -> int:
    try:
        value = int(text)
    except ValueError as error:
        raise AuditError(f"line {line}: {field} must be an integer") from error
    if value < lower or value > upper:
        raise AuditError(f"line {line}: {field} outside [{lower}, {upper}]")
    return value


def parse_validation(path: Path) -> tuple[Decision, ...]:
    if any("holdout" in part.lower() for part in path.parts):
        raise AuditError("fit audit must not open a holdout path")
    lines = path.read_text(encoding="utf-8").splitlines()
    if not lines or tuple(lines[0].split("\t")) != HEADER:
        raise AuditError("validation trace header differs")
    records: dict[str, list[Candidate]] = {}
    groups: dict[str, str] = {}
    order: list[str] = []
    closed: set[str] = set()
    active: str | None = None
    for line_number, line in enumerate(lines[1:], start=2):
        fields = line.split("\t")
        if len(fields) != len(HEADER) or any(not field for field in fields):
            raise AuditError(f"line {line_number}: malformed trace row")
        row = dict(zip(HEADER, fields, strict=True))
        if row["schema"] != TRACE_SCHEMA:
            raise AuditError(f"line {line_number}: trace schema differs")
        record_id = row["record_id"]
        if record_id != active:
            if active is not None:
                closed.add(active)
            if record_id in closed:
                raise AuditError(f"line {line_number}: record rows are not contiguous")
            active = record_id
            order.append(record_id)
        group_id = row["group_id"]
        if groups.setdefault(record_id, group_id) != group_id:
            raise AuditError(f"line {line_number}: record group changes")
        victim = bounded_int(row["victim"], 0, 6, line_number, "victim")
        promotion = bounded_int(row["promotion"], 0, 6, line_number, "promotion")
        tactical = victim != 0 or promotion != 0
        if (row["stage"] == "tactical") != tactical or row["stage"] not in {"quiet", "tactical"}:
            raise AuditError(f"line {line_number}: stage differs from move features")
        see = bounded_int(row["see_bucket"], -2, 2, line_number, "see")
        move_class = (
            "quiet" if row["stage"] == "quiet"
            else "good_tactical" if promotion != 0 or see >= 0
            else "bad_tactical"
        )
        candidate = Candidate(
            row["candidate_move"],
            bounded_int(row["selected"], 0, 1, line_number, "selected") == 1,
            move_class,
            (
                bounded_int(row["from_normalized"], 0, 63, line_number, "from") * 64
                + bounded_int(row["to_normalized"], 0, 63, line_number, "to"),
                (bounded_int(row["mover"], 1, 6, line_number, "mover") - 1) * 64
                + bounded_int(row["to_normalized"], 0, 63, line_number, "to"),
                victim * 7 + promotion,
                bounded_int(row["phase"], 0, 2, line_number, "phase"),
                (bounded_int(row["previous_to_normalized"], -1, 63, line_number, "previous") + 1) * 64
                + bounded_int(row["to_normalized"], 0, 63, line_number, "to"),
                see + 2,
            ),
        )
        bucket = records.setdefault(record_id, [])
        if any(existing.move == candidate.move for existing in bucket):
            raise AuditError(f"line {line_number}: duplicate candidate move")
        bucket.append(candidate)
    decisions = tuple(Decision(record_id, groups[record_id], tuple(records[record_id])) for record_id in order)
    if not decisions:
        raise AuditError("validation trace is empty")
    if any(len(item.candidates) < 2 or sum(candidate.selected for candidate in item.candidates) != 1 for item in decisions):
        raise AuditError("each decision must have at least two moves and one selection")
    return decisions


def read_weights(path: Path) -> tuple[int, ...]:
    payload = path.read_bytes()
    if len(payload) < BINARY_HEADER.size:
        raise AuditError("quantized artifact is truncated")
    magic, version, *header_values = BINARY_HEADER.unpack_from(payload)
    sizes, expected_checksum = header_values[:-1], header_values[-1]
    if magic != MAGIC or version != 2 or tuple(sizes) != FAMILY_SIZES:
        raise AuditError("quantized artifact header differs")
    count = sum(FAMILY_SIZES)
    if len(payload) != BINARY_HEADER.size + count * 2:
        raise AuditError("quantized artifact length differs")
    weights = payload[BINARY_HEADER.size:]
    if zlib.crc32(weights) != expected_checksum:
        raise AuditError("quantized artifact checksum differs")
    values = struct.unpack_from(f"<{count}h", weights)
    if any(abs(value) > 128 for value in values):
        raise AuditError("quantized artifact contains an out-of-range weight")
    return values


def candidate_score(candidate: Candidate, weights: tuple[int, ...]) -> int:
    return sum(weights[offset + feature] for offset, feature in zip(OFFSETS, candidate.features, strict=True))


def metrics(decisions: tuple[Decision, ...], weights: tuple[int, ...]) -> dict[str, float | int]:
    input_decisions = len(decisions)
    comparable = []
    for decision in decisions:
        selected = next(candidate for candidate in decision.candidates if candidate.selected)
        candidates = tuple(
            candidate for candidate in decision.candidates
            if candidate.move_class == selected.move_class
        )
        if len(candidates) >= 2:
            comparable.append(Decision(decision.record_id, decision.group_id, candidates))
    decisions = tuple(comparable)
    if not decisions:
        raise AuditError("validation has no MovePicker-stage-comparable decisions")
    group_counts = Counter(decision.group_id for decision in decisions)
    pairwise = top1 = first_legal = 0.0
    for decision in decisions:
        selected = next(candidate for candidate in decision.candidates if candidate.selected)
        selected_score = candidate_score(selected, weights)
        siblings = [candidate for candidate in decision.candidates if not candidate.selected]
        pair_score = sum(
            1.0 if selected_score > candidate_score(candidate, weights)
            else 0.5 if selected_score == candidate_score(candidate, weights)
            else 0.0
            for candidate in siblings
        ) / len(siblings)
        winner = max(
            enumerate(decision.candidates),
            key=lambda item: (candidate_score(item[1], weights), -item[0]),
        )[1]
        record_weight = 1.0 / (len(group_counts) * group_counts[decision.group_id])
        pairwise += record_weight * pair_score
        top1 += record_weight * float(winner.selected)
        first_legal += record_weight * float(decision.candidates[0].selected)
    return {
        "input_decisions": input_decisions,
        "decisions": len(decisions),
        "groups": len(group_counts),
        "pairwise_accuracy": pairwise,
        "top1_accuracy": top1,
        "first_legal_top1_accuracy": first_legal,
    }


def audit(
    validation: Path,
    float_path: Path,
    quantized: Path,
    report_path: Path,
    repeat_float: Path,
    repeat_quantized: Path,
    repeat_report: Path,
) -> dict[str, object]:
    for first, repeated, name in (
        (float_path, repeat_float, "float"),
        (quantized, repeat_quantized, "quantized"),
        (report_path, repeat_report, "report"),
    ):
        if first.read_bytes() != repeated.read_bytes():
            raise AuditError(f"repeated {name} artifact differs")
    report = json.loads(report_path.read_text(encoding="utf-8"))
    if report.get("schema") != REPORT_SCHEMA:
        raise AuditError("fit report schema differs")
    if report.get("parameters", {}).get("comparison_pool") != "selected move's MovePicker runtime class":
        raise AuditError("fit report comparison pool differs")
    for path, name in ((float_path, "float"), (quantized, "quantized")):
        digest, size = sha256_file(path)
        if report.get("artifacts", {}).get(name) != {"bytes": size, "sha256": digest}:
            raise AuditError(f"fit report {name} identity differs")
    calculated = metrics(parse_validation(validation), read_weights(quantized))
    reported = report.get("validation")
    if not isinstance(reported, dict):
        raise AuditError("fit report lacks validation metrics")
    for key, value in calculated.items():
        if isinstance(value, float):
            if not math.isclose(value, reported.get(key, math.nan), abs_tol=1e-15):
                raise AuditError(f"validation {key} differs")
        elif reported.get(key) != value:
            raise AuditError(f"validation {key} differs")
    if calculated["pairwise_accuracy"] <= 0.5:
        raise AuditError("validation pairwise accuracy did not beat chance")
    if calculated["top1_accuracy"] <= calculated["first_legal_top1_accuracy"]:
        raise AuditError("validation top-1 did not beat first-legal baseline")
    return {
        "schema": AUDIT_SCHEMA,
        "ok": True,
        "validation": calculated,
        "quantized": dict(zip(("sha256", "bytes"), sha256_file(quantized))),
        "repeat_byte_identical": {"float": True, "quantized": True, "report": True},
    }


def main() -> int:
    args = parse_args()
    try:
        result = audit(
            args.validation.resolve(), args.float_path.resolve(), args.quantized.resolve(),
            args.report.resolve(), args.repeat_float.resolve(), args.repeat_quantized.resolve(),
            args.repeat_report.resolve(),
        )
    except (AuditError, OSError, ValueError, json.JSONDecodeError) as error:
        print(f"audit-shegerd-policy-fit: {error}", file=sys.stderr)
        return 1
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
