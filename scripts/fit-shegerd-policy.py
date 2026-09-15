#!/usr/bin/env python3
"""Fit the registered SHEGERD-P1 additive policy from P0 trace TSV files."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import struct
import sys
import zlib
from array import array
from collections import defaultdict
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable


TRACE_SCHEMA = "neyrang-shegerd-policy-trace-v1"
ARTIFACT_SCHEMA = "neyrang-shegerd-policy-v1"
REPORT_SCHEMA = "neyrang-shegerd-policy-fit-result-v1"
HEADER = (
    "schema", "record_id", "group_id", "candidate_move", "selected", "stage",
    "from_normalized", "to_normalized", "mover", "victim", "promotion", "phase",
    "previous_to_normalized", "see_bucket", "fen",
)
FAMILY_NAMES = (
    "from_to", "mover_to", "victim_promotion", "phase", "previous_to", "see",
)
FAMILY_SIZES = (64 * 64, 6 * 64, 7 * 7, 3, 65 * 64, 5)
OFFSETS: tuple[int, ...] = tuple(
    sum(FAMILY_SIZES[:index]) for index in range(len(FAMILY_SIZES))
)
TOTAL_WEIGHTS = sum(FAMILY_SIZES)
MAGIC = b"NYRSHGP1"
BINARY_HEADER = struct.Struct("<8s8I")
EPOCHS = 12
LEARNING_RATE = 0.15
ADAGRAD_EPSILON = 1e-8
L2 = 0.0005
WEIGHT_CLAMP = 4.0
QUANTIZATION = 32


class FitError(RuntimeError):
    """A fail-closed policy fitting error."""


@dataclass(frozen=True)
class Candidate:
    move: str
    selected: bool
    features: tuple[int, int, int, int, int, int]


@dataclass(frozen=True)
class Decision:
    record_id: str
    group_id: str
    candidates: tuple[Candidate, ...]


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--train", type=Path, required=True)
    parser.add_argument("--validation", type=Path, required=True)
    parser.add_argument("--float-output", type=Path, required=True)
    parser.add_argument("--quantized-output", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    return parser.parse_args()


def sha256_file(path: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
            size += len(chunk)
    return digest.hexdigest(), size


def reject_holdout_path(path: Path) -> None:
    if any("holdout" in component.lower() for component in path.parts):
        raise FitError("the P1 fitter must not open a holdout path")


def integer(text: str, name: str, lower: int, upper: int, line: int) -> int:
    try:
        value = int(text)
    except ValueError as error:
        raise FitError(f"line {line}: {name} must be an integer") from error
    if value < lower or value > upper:
        raise FitError(f"line {line}: {name} outside [{lower}, {upper}]")
    return value


def parse_trace(path: Path) -> tuple[Decision, ...]:
    reject_holdout_path(path)
    try:
        lines = path.read_text(encoding="utf-8").splitlines()
    except (OSError, UnicodeError) as error:
        raise FitError(f"cannot read trace {path}: {error}") from error
    if not lines or tuple(lines[0].split("\t")) != HEADER:
        raise FitError("trace header does not exactly match P0 schema")

    records: dict[str, list[Candidate]] = {}
    groups: dict[str, str] = {}
    closed: set[str] = set()
    active: str | None = None
    for line_number, text in enumerate(lines[1:], start=2):
        fields = text.split("\t")
        if len(fields) != len(HEADER):
            raise FitError(f"line {line_number}: expected {len(HEADER)} tab fields")
        if any(not field or field.strip() != field for field in fields):
            raise FitError(f"line {line_number}: fields must be non-empty and trimmed")
        row = dict(zip(HEADER, fields, strict=True))
        if row["schema"] != TRACE_SCHEMA:
            raise FitError(f"line {line_number}: unsupported trace schema")
        record_id = row["record_id"]
        if record_id != active:
            if active is not None:
                closed.add(active)
            if record_id in closed:
                raise FitError(f"line {line_number}: record rows must be contiguous")
            active = record_id
        group_id = row["group_id"]
        existing_group = groups.setdefault(record_id, group_id)
        if existing_group != group_id:
            raise FitError(f"line {line_number}: record group changes")
        selected = integer(row["selected"], "selected", 0, 1, line_number) == 1
        stage = row["stage"]
        if stage not in {"quiet", "tactical"}:
            raise FitError(f"line {line_number}: invalid stage")
        from_square = integer(row["from_normalized"], "from_normalized", 0, 63, line_number)
        to_square = integer(row["to_normalized"], "to_normalized", 0, 63, line_number)
        mover = integer(row["mover"], "mover", 1, 6, line_number)
        victim = integer(row["victim"], "victim", 0, 6, line_number)
        promotion = integer(row["promotion"], "promotion", 0, 6, line_number)
        phase = integer(row["phase"], "phase", 0, 2, line_number)
        previous = integer(
            row["previous_to_normalized"], "previous_to_normalized", -1, 63, line_number
        )
        see = integer(row["see_bucket"], "see_bucket", -2, 2, line_number)
        tactical = victim != 0 or promotion != 0
        if (stage == "tactical") != tactical:
            raise FitError(f"line {line_number}: stage does not match victim/promotion")
        candidate = Candidate(
            move=row["candidate_move"],
            selected=selected,
            features=(
                from_square * 64 + to_square,
                (mover - 1) * 64 + to_square,
                victim * 7 + promotion,
                phase,
                (previous + 1) * 64 + to_square,
                see + 2,
            ),
        )
        bucket = records.setdefault(record_id, [])
        if any(item.move == candidate.move for item in bucket):
            raise FitError(f"line {line_number}: duplicate candidate move")
        bucket.append(candidate)

    if not records:
        raise FitError("trace has no decision rows")
    decisions: list[Decision] = []
    for record_id in sorted(records):
        candidates = tuple(records[record_id])
        if len(candidates) < 2:
            raise FitError(f"record {record_id!r} has fewer than two legal moves")
        if sum(candidate.selected for candidate in candidates) != 1:
            raise FitError(f"record {record_id!r} must have exactly one selected move")
        decisions.append(Decision(record_id, groups[record_id], candidates))
    return tuple(decisions)


def indices(candidate: Candidate) -> tuple[int, ...]:
    return tuple(offset + feature for offset, feature in zip(OFFSETS, candidate.features, strict=True))


def score(candidate: Candidate, weights: list[float] | list[int]) -> float | int:
    return sum(weights[index] for index in indices(candidate))


def stable_sigmoid_negative(margin: float) -> float:
    if margin >= 0:
        exp = math.exp(-margin)
        return exp / (1.0 + exp)
    exp = math.exp(margin)
    return 1.0 / (1.0 + exp)


def stable_logistic_loss(margin: float) -> float:
    return math.log1p(math.exp(-abs(margin))) + max(-margin, 0.0)


def grouped_weights(decisions: Iterable[Decision]) -> dict[str, float]:
    counts: dict[str, int] = defaultdict(int)
    for decision in decisions:
        counts[decision.group_id] += 1
    if not counts:
        raise FitError("no decision groups")
    return {
        record_id: 1.0 / (len(counts) * counts[group_id])
        for record_id, group_id in ((item.record_id, item.group_id) for item in decisions)
    }


def objective(decisions: tuple[Decision, ...], weights: list[float]) -> float:
    per_record = grouped_weights(decisions)
    total = 0.0
    for decision in decisions:
        selected = next(candidate for candidate in decision.candidates if candidate.selected)
        siblings = [candidate for candidate in decision.candidates if not candidate.selected]
        total += per_record[decision.record_id] * sum(
            stable_logistic_loss(float(score(selected, weights) - score(sibling, weights)))
            for sibling in siblings
        ) / len(siblings)
    return total


def fit(decisions: tuple[Decision, ...]) -> tuple[list[float], dict[str, float | int]]:
    weights = [0.0] * TOTAL_WEIGHTS
    accumulator = [0.0] * TOTAL_WEIGHTS
    per_record = grouped_weights(decisions)
    before = objective(decisions, weights)
    for _ in range(EPOCHS):
        for decision in decisions:
            selected = next(candidate for candidate in decision.candidates if candidate.selected)
            siblings = [candidate for candidate in decision.candidates if not candidate.selected]
            pair_weight = per_record[decision.record_id] / len(siblings)
            selected_indices = indices(selected)
            for sibling in siblings:
                margin = float(score(selected, weights) - score(sibling, weights))
                probability = stable_sigmoid_negative(margin)
                gradients: dict[int, float] = defaultdict(float)
                for index in selected_indices:
                    gradients[index] -= probability * pair_weight
                for index in indices(sibling):
                    gradients[index] += probability * pair_weight
                for index, gradient in gradients.items():
                    gradient += L2 * pair_weight * weights[index]
                    accumulator[index] += gradient * gradient
                    step = LEARNING_RATE * gradient / math.sqrt(accumulator[index] + ADAGRAD_EPSILON)
                    weights[index] = max(-WEIGHT_CLAMP, min(WEIGHT_CLAMP, weights[index] - step))
    return weights, {"initial_pairwise_loss": before, "final_pairwise_loss": objective(decisions, weights)}


def quantize(value: float) -> int:
    scaled = value * QUANTIZATION
    rounded = math.floor(scaled + 0.5) if scaled >= 0 else math.ceil(scaled - 0.5)
    return max(-32768, min(32767, int(rounded)))


def evaluate(decisions: tuple[Decision, ...], weights: list[int]) -> dict[str, float]:
    per_record = grouped_weights(decisions)
    pairwise = top1 = first_legal = 0.0
    for decision in decisions:
        selected = next(candidate for candidate in decision.candidates if candidate.selected)
        siblings = [candidate for candidate in decision.candidates if not candidate.selected]
        selected_score = score(selected, weights)
        pairwise_score = sum(
            1.0 if selected_score > score(sibling, weights)
            else 0.5 if selected_score == score(sibling, weights)
            else 0.0
            for sibling in siblings
        ) / len(siblings)
        winner = max(enumerate(decision.candidates), key=lambda item: (score(item[1], weights), -item[0]))[1]
        record_weight = per_record[decision.record_id]
        pairwise += record_weight * pairwise_score
        top1 += record_weight * float(winner.selected)
        first_legal += record_weight * float(decision.candidates[0].selected)
    return {"pairwise_accuracy": pairwise, "top1_accuracy": top1, "first_legal_top1_accuracy": first_legal}


def float_payload(weights: list[float]) -> bytes:
    tables: dict[str, list[float]] = {}
    for name, offset, size in zip(FAMILY_NAMES, OFFSETS, FAMILY_SIZES, strict=True):
        tables[name] = weights[offset:offset + size]
    return (json.dumps({"schema": ARTIFACT_SCHEMA, "tables": tables}, separators=(",", ":")) + "\n").encode()


def quantized_payload(weights: list[int]) -> bytes:
    packed = array("h", weights)
    if sys.byteorder != "little":
        packed.byteswap()
    payload = packed.tobytes()
    return BINARY_HEADER.pack(MAGIC, 2, *FAMILY_SIZES, zlib.crc32(payload)) + payload


def write_once(path: Path, content: bytes) -> None:
    if path.exists():
        raise FitError(f"refusing to overwrite artifact {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_name(f".{path.name}.tmp-{os.getpid()}")
    if temporary.exists():
        raise FitError(f"temporary artifact exists {temporary}")
    temporary.write_bytes(content)
    temporary.replace(path)


def run_fit(train: Path, validation: Path, float_output: Path, quantized_output: Path, report: Path) -> dict[str, object]:
    paths = (float_output, quantized_output, report)
    if len({path.resolve() for path in paths}) != len(paths):
        raise FitError("output paths must be distinct")
    if any(path.exists() for path in paths):
        raise FitError("refusing to overwrite an existing output")
    written: list[Path] = []
    try:
        train_decisions = parse_trace(train)
        validation_decisions = parse_trace(validation)
        overlap = {decision.record_id for decision in train_decisions} & {decision.record_id for decision in validation_decisions}
        if overlap:
            raise FitError("train and validation record ids overlap")
        train_groups = {decision.group_id for decision in train_decisions}
        validation_groups = {decision.group_id for decision in validation_decisions}
        if train_groups & validation_groups:
            raise FitError("train and validation opening groups overlap")
        weights, training = fit(train_decisions)
        quantized = [quantize(weight) for weight in weights]
        float_bytes = float_payload(weights)
        quantized_bytes = quantized_payload(quantized)
        write_once(float_output, float_bytes)
        written.append(float_output)
        write_once(quantized_output, quantized_bytes)
        written.append(quantized_output)
        train_hash, train_bytes = sha256_file(train)
        validation_hash, validation_bytes = sha256_file(validation)
        float_hash, float_size = sha256_file(float_output)
        quantized_hash, quantized_size = sha256_file(quantized_output)
        validation_metrics = evaluate(validation_decisions, quantized)
        result: dict[str, object] = {
        "schema": REPORT_SCHEMA,
        "inputs": {
            "train": {"bytes": train_bytes, "sha256": train_hash},
            "validation": {"bytes": validation_bytes, "sha256": validation_hash},
        },
        "parameters": {
            "epochs": EPOCHS,
            "learning_rate": LEARNING_RATE,
            "adagrad_epsilon": ADAGRAD_EPSILON,
            "l2": L2,
            "weight_clamp": [-WEIGHT_CLAMP, WEIGHT_CLAMP],
            "quantization": QUANTIZATION,
            "group_weighting": "each opening-pair group has equal total weight",
        },
        "training": {
            "decisions": len(train_decisions),
            "groups": len(train_groups),
            **training,
        },
        "validation": {
            "decisions": len(validation_decisions),
            "groups": len(validation_groups),
            **validation_metrics,
        },
        "artifacts": {
            "float": {"bytes": float_size, "sha256": float_hash},
            "quantized": {"bytes": quantized_size, "sha256": quantized_hash},
        },
        }
        write_once(report, (json.dumps(result, indent=2, sort_keys=True) + "\n").encode())
        return result
    except BaseException:
        for path in written:
            path.unlink(missing_ok=True)
        raise


def main() -> int:
    args = parse_args()
    try:
        result = run_fit(
            args.train.resolve(), args.validation.resolve(), args.float_output.resolve(),
            args.quantized_output.resolve(), args.report.resolve(),
        )
    except (FitError, OSError, ValueError) as error:
        print(f"fit-shegerd-policy: {error}", file=sys.stderr)
        return 1
    validation = result["validation"]
    assert isinstance(validation, dict)
    print(
        f"validation pairwise={validation['pairwise_accuracy']:.6f} "
        f"top1={validation['top1_accuracy']:.6f}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
