#!/usr/bin/env python3
"""Analyze NEYRANG train/validation SANJ features without opening holdout."""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import platform
import re
from dataclasses import dataclass
from pathlib import Path

import numpy as np


MG_FEATURES = (
    "piece_p_delta",
    "piece_n_delta",
    "piece_b_delta",
    "piece_r_delta",
    "piece_q_delta",
    "pawn_rank_delta",
    "pawn_file_edge_delta",
    "knight_center_delta",
    "bishop_center_delta",
    "rook_rank_delta",
    "rook_file_edge_delta",
    "queen_center_delta",
    "king_rank_delta",
    "king_file_edge_delta",
    "bishop_pair_delta",
    "doubled_extra_delta",
    "isolated_pawn_delta",
    "passed_rank_sq_delta",
    "mobility_n_delta",
    "mobility_b_delta",
    "mobility_r_delta",
    "mobility_q_delta",
    "rook_open_delta",
    "rook_semi_open_delta",
    "king_shield_delta",
)
EG_FEATURES = (
    "piece_p_delta",
    "piece_n_delta",
    "piece_b_delta",
    "piece_r_delta",
    "piece_q_delta",
    "pawn_rank_delta",
    "pawn_file_edge_delta",
    "knight_center_delta",
    "bishop_center_delta",
    "rook_rank_delta",
    "queen_center_delta",
    "king_center_delta",
    "bishop_pair_delta",
    "doubled_extra_delta",
    "isolated_pawn_delta",
    "passed_rank_sq_delta",
)
TRACE_COLUMNS = (
    "schema",
    "record_id",
    "target",
    "fen",
    "stm",
    "phase",
    "piece_p_delta",
    "piece_n_delta",
    "piece_b_delta",
    "piece_r_delta",
    "piece_q_delta",
    "piece_k_delta",
    "pawn_rank_delta",
    "pawn_file_edge_delta",
    "knight_center_delta",
    "bishop_center_delta",
    "rook_rank_delta",
    "rook_file_edge_delta",
    "queen_center_delta",
    "king_rank_delta",
    "king_file_edge_delta",
    "king_center_delta",
    "bishop_pair_delta",
    "doubled_extra_delta",
    "isolated_pawn_delta",
    "passed_rank_sq_delta",
    "mobility_n_delta",
    "mobility_b_delta",
    "mobility_r_delta",
    "mobility_q_delta",
    "rook_open_delta",
    "rook_semi_open_delta",
    "king_shield_delta",
    "middlegame_cp",
    "endgame_cp",
    "white_cp",
    "tempo_cp",
    "stm_cp",
)
RECORD_ID = re.compile(
    r"^(?P<group>[A-Za-z0-9][A-Za-z0-9._-]*:pair-[0-9]{6,}):"
    r"game-[12]:ply-[0-9]{3,}$"
)
CURRENT_TRACE_SCHEMA = "neyrang-sanj-trace-v1"
PRE_CONTRACT_TRACE_SCHEMA = re.compile(
    r"^[a-z0-9][a-z0-9-]*-eval-trace-v1$"
)
CURRENT_EFFECTIVE_WEIGHTS = (
    82,
    313,
    353,
    477,
    1_020,
    7,
    2,
    9,
    5,
    2,
    1,
    2,
    -9,
    -3,
    28,
    -11,
    -10,
    2,
    4,
    5,
    2,
    1,
    18,
    10,
    9,
    94,
    263,
    287,
    512,
    936,
    12,
    1,
    7,
    4,
    3,
    1,
    8,
    38,
    -16,
    -8,
    4,
    12,
)


@dataclass(frozen=True)
class Partition:
    path: Path
    record_ids: tuple[str, ...]
    groups: np.ndarray
    design: np.ndarray
    target: np.ndarray
    cp: np.ndarray
    names: tuple[str, ...]
    phase: np.ndarray
    mg_coefficients: np.ndarray
    eg_coefficients: np.ndarray
    tempo_sign: np.ndarray


def _integer_columns(rows: list[dict[str, str]], names: tuple[str, ...]) -> np.ndarray:
    values = np.asarray(
        [[float(row[name]) for name in names] for row in rows], dtype=np.float64
    )
    if not np.array_equal(values, np.trunc(values)):
        raise ValueError("trace coefficients must be integers")
    return values.astype(np.int64)


def exact_integer_cp(partition: Partition, weights: np.ndarray) -> np.ndarray:
    """Reproduce Rust's tapered integer score in the White-relative frame."""
    candidate = np.asarray(weights)
    if candidate.shape != (42,) or not np.array_equal(candidate, np.trunc(candidate)):
        raise ValueError("effective weights must be 42 integers")
    candidate = candidate.astype(np.int64)
    middlegame = partition.mg_coefficients @ candidate[:25]
    endgame = partition.eg_coefficients @ candidate[25:41]
    numerator = (
        middlegame * partition.phase
        + endgame * (24 - partition.phase)
    )
    tapered = np.where(numerator >= 0, numerator // 24, -((-numerator) // 24))
    return tapered + partition.tempo_sign * candidate[41]


def load_partition(path: Path) -> Partition:
    with path.open(encoding="utf-8", newline="") as handle:
        reader = csv.DictReader(handle, delimiter="\t")
        if tuple(reader.fieldnames or ()) != TRACE_COLUMNS:
            raise ValueError(f"{path}: expected canonical 38-column header")
        rows = list(reader)
    if not rows:
        raise ValueError(f"{path}: partition is empty")

    record_ids: list[str] = []
    groups: list[str] = []
    for row in rows:
        schema = row.get("schema", "")
        if (
            schema != CURRENT_TRACE_SCHEMA
            and PRE_CONTRACT_TRACE_SCHEMA.fullmatch(schema) is None
        ):
            raise ValueError(f"{path}: unexpected trace schema")
        record_id = row["record_id"]
        match = RECORD_ID.fullmatch(record_id)
        if match is None:
            raise ValueError(f"{path}: invalid record id {record_id!r}")
        record_ids.append(record_id)
        groups.append(match.group("group"))
    if len(set(record_ids)) != len(record_ids):
        raise ValueError(f"{path}: duplicate record id")

    stm = np.asarray([row["stm"] for row in rows], dtype=object)
    if not np.isin(stm, np.asarray(["w", "b"], dtype=object)).all():
        raise ValueError(f"{path}: side to move must be w or b")

    phase_values = np.asarray([float(row["phase"]) for row in rows])
    if not np.array_equal(phase_values, np.trunc(phase_values)):
        raise ValueError(f"{path}: phase must be an integer")
    phase = phase_values.astype(np.int64)
    if np.any((phase < 0.0) | (phase > 24.0)):
        raise ValueError(f"{path}: phase is outside [0,24]")
    target = np.asarray([float(row["target"]) for row in rows])
    if not np.isin(target, np.asarray([0.0, 0.5, 1.0])).all():
        raise ValueError(f"{path}: target is outside WDL values")

    mg_coefficients = _integer_columns(rows, MG_FEATURES)
    eg_coefficients = _integer_columns(rows, EG_FEATURES)
    columns: list[np.ndarray] = []
    names: list[str] = []
    for index, feature in enumerate(MG_FEATURES):
        columns.append(mg_coefficients[:, index] * phase / 24.0)
        names.append(f"mg:{feature}")
    for index, feature in enumerate(EG_FEATURES):
        columns.append(eg_coefficients[:, index] * (24.0 - phase) / 24.0)
        names.append(f"eg:{feature}")
    tempo_sign = np.where(stm == "w", 1, -1).astype(np.int64)
    columns.append(tempo_sign.astype(np.float64))
    names.append("tempo")
    design = np.column_stack(columns)
    cp = np.asarray(
        [
            float(row["white_cp"])
            + (
                float(row["tempo_cp"])
                if row["stm"] == "w"
                else -float(row["tempo_cp"])
            )
            for row in rows
        ]
    )
    stm_cp = np.asarray([float(row["stm_cp"]) for row in rows])
    if not np.array_equal(stm_cp, np.where(stm == "w", cp, -cp)):
        raise ValueError(f"{path}: stm_cp does not reconstruct from white_cp and tempo")
    if (
        not np.isfinite(design).all()
        or not np.isfinite(target).all()
        or not np.isfinite(cp).all()
    ):
        raise ValueError(f"{path}: non-finite numeric value")
    return Partition(
        path=path,
        record_ids=tuple(record_ids),
        groups=np.asarray(groups, dtype=object),
        design=design,
        target=target,
        cp=cp,
        names=tuple(names),
        phase=phase,
        mg_coefficients=mg_coefficients,
        eg_coefficients=eg_coefficients,
        tempo_sign=tempo_sign,
    )


def group_weights(groups: np.ndarray) -> np.ndarray:
    """Return normalized row weights that give each group equal total mass."""
    unique, inverse, counts = np.unique(
        groups, return_inverse=True, return_counts=True
    )
    if unique.size == 0:
        raise ValueError("at least one group is required")
    return 1.0 / (unique.size * counts[inverse])


def group_mean_values(
    groups: np.ndarray, values: np.ndarray
) -> tuple[np.ndarray, np.ndarray]:
    """Collapse row values to sorted opening-pair means."""
    if groups.ndim != 1 or values.ndim != 1 or groups.size != values.size:
        raise ValueError("groups and values must be equal-length vectors")
    if groups.size == 0:
        raise ValueError("at least one group is required")
    unique, inverse, counts = np.unique(
        groups, return_inverse=True, return_counts=True
    )
    sums = np.bincount(inverse, weights=values)
    return unique, sums / counts


def select_complete_groups(
    groups: np.ndarray, minimum_rows: int, seed: str
) -> np.ndarray:
    """Select hash-ranked complete groups until the row floor is reached."""
    if minimum_rows <= 0:
        raise ValueError("minimum rows must be positive")
    unique, counts = np.unique(groups, return_counts=True)
    if int(counts.sum()) < minimum_rows:
        raise ValueError("minimum rows exceed the partition size")
    ranked = sorted(
        zip(unique.tolist(), counts.tolist(), strict=True),
        key=lambda item: (
            hashlib.sha256(f"{seed}\t{item[0]}".encode()).digest(),
            item[0],
        ),
    )
    selected: list[str] = []
    rows = 0
    for group, count in ranked:
        selected.append(group)
        rows += count
        if rows >= minimum_rows:
            break
    return np.isin(groups, np.asarray(selected, dtype=object))


def bootstrap_group_delta(
    values: np.ndarray, *, seed: int, replicates: int
) -> dict[str, float]:
    """Return a deterministic percentile bootstrap over group-mean deltas."""
    if values.ndim != 1 or values.size == 0:
        raise ValueError("group deltas must be a non-empty vector")
    if replicates <= 0:
        raise ValueError("bootstrap replicates must be positive")
    generator = np.random.Generator(np.random.PCG64(seed))
    samples: list[np.ndarray] = []
    remaining = replicates
    while remaining:
        count = min(remaining, 1_000)
        indices = generator.integers(0, values.size, size=(count, values.size))
        samples.append(values[indices].mean(axis=1))
        remaining -= count
    means = np.concatenate(samples)
    lower, upper = np.percentile(means, [2.5, 97.5])
    return {
        "mean": float(values.mean()),
        "lower": float(lower),
        "upper": float(upper),
    }


def probability(cp: np.ndarray, scale: float) -> np.ndarray:
    exponent = np.clip(-scale * cp / 400.0, -50.0, 50.0)
    return 1.0 / (1.0 + np.power(10.0, exponent))


def loss_values(target: np.ndarray, prediction: np.ndarray, kind: str) -> np.ndarray:
    if kind == "mse":
        return np.square(target - prediction)
    if kind == "cross_entropy":
        clipped = np.clip(prediction, 1e-12, 1.0 - 1e-12)
        return -(target * np.log(clipped) + (1.0 - target) * np.log(1.0 - clipped))
    raise ValueError(f"unsupported loss {kind!r}")


def weighted_loss(
    target: np.ndarray,
    prediction: np.ndarray,
    weights: np.ndarray,
    kind: str,
) -> float:
    return float(np.dot(weights, loss_values(target, prediction, kind)))


def fit_scale(
    target: np.ndarray,
    cp: np.ndarray,
    weights: np.ndarray,
    kind: str,
) -> float:
    lo, hi = 0.01, 4.0
    ratio = (5.0**0.5 - 1.0) / 2.0
    left = hi - ratio * (hi - lo)
    right = lo + ratio * (hi - lo)
    for _ in range(100):
        left_loss = weighted_loss(target, probability(cp, left), weights, kind)
        right_loss = weighted_loss(target, probability(cp, right), weights, kind)
        if left_loss < right_loss:
            hi = right
            right = left
            left = hi - ratio * (hi - lo)
        else:
            lo = left
            left = right
            right = lo + ratio * (hi - lo)
    return (lo + hi) / 2.0


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def partition_summary(partition: Partition) -> dict[str, object]:
    singular = np.linalg.svd(partition.design, compute_uv=False)
    tolerance = (
        np.finfo(partition.design.dtype).eps
        * max(partition.design.shape)
        * singular[0]
    )
    rank = int(np.count_nonzero(singular > tolerance))
    condition = None
    if singular[-1] > 0.0:
        condition = float(singular[0] / singular[-1])
    target_values, target_counts = np.unique(
        partition.target, return_counts=True
    )
    targets = {
        format(float(value), ".12g"): int(count)
        for value, count in zip(
            target_values.tolist(), target_counts.tolist(), strict=True
        )
    }
    coverage = []
    for index, name in enumerate(partition.names):
        column = partition.design[:, index]
        coverage.append(
            {
                "name": name,
                "nonzero_rows": int(np.count_nonzero(column)),
                "unique_values": int(np.unique(column).size),
                "minimum": float(column.min()),
                "maximum": float(column.max()),
                "range": float(column.max() - column.min()),
                "standard_deviation": float(column.std()),
            }
        )
    return {
        "rows": int(partition.target.size),
        "groups": int(np.unique(partition.groups).size),
        "target_counts": targets,
        "cp": {
            "minimum": float(partition.cp.min()),
            "maximum": float(partition.cp.max()),
            "mean": float(partition.cp.mean()),
        },
        "design": {
            "columns": int(partition.design.shape[1]),
            "rank": rank,
            "condition_number": condition,
            "largest_singular_value": float(singular[0]),
            "smallest_singular_value": float(singular[-1]),
            "coverage": coverage,
        },
    }


def _loss_report(
    train: Partition,
    validation: Partition,
    train_mask: np.ndarray,
    kind: str,
    bootstrap_replicates: int,
) -> dict[str, object]:
    train_target = train.target[train_mask]
    train_cp = train.cp[train_mask]
    train_groups = train.groups[train_mask]
    train_weights = group_weights(train_groups)
    validation_weights = group_weights(validation.groups)
    fitted_scale = fit_scale(
        train_target, train_cp, train_weights, kind
    )

    train_baseline_values = loss_values(
        train_target, probability(train_cp, 1.0), kind
    )
    train_fitted_values = loss_values(
        train_target, probability(train_cp, fitted_scale), kind
    )
    validation_baseline_values = loss_values(
        validation.target, probability(validation.cp, 1.0), kind
    )
    validation_fitted_values = loss_values(
        validation.target,
        probability(validation.cp, fitted_scale),
        kind,
    )
    _, validation_group_deltas = group_mean_values(
        validation.groups,
        validation_fitted_values - validation_baseline_values,
    )

    train_baseline = float(np.dot(train_weights, train_baseline_values))
    train_fitted = float(np.dot(train_weights, train_fitted_values))
    validation_baseline = float(
        np.dot(validation_weights, validation_baseline_values)
    )
    validation_fitted = float(
        np.dot(validation_weights, validation_fitted_values)
    )
    return {
        "fitted_scale": fitted_scale,
        "train": {
            "baseline": train_baseline,
            "fitted": train_fitted,
            "delta": train_fitted - train_baseline,
        },
        "validation": {
            "baseline": validation_baseline,
            "fitted": validation_fitted,
            "delta": validation_fitted - validation_baseline,
            "group_bootstrap_delta": bootstrap_group_delta(
                validation_group_deltas,
                seed=20260831,
                replicates=bootstrap_replicates,
            ),
        },
    }


def analyze_partitions(
    train: Partition,
    validation: Partition,
    *,
    bootstrap_replicates: int = 10_000,
    learning_floor: int = 25_000,
) -> dict[str, object]:
    if train.design.shape[1] != 42 or validation.design.shape[1] != 42:
        raise ValueError("both partitions must use the registered 42-column design")
    if train.names != validation.names:
        raise ValueError("train and validation feature names differ")
    overlap = np.intersect1d(
        np.unique(train.groups), np.unique(validation.groups)
    )
    if overlap.size:
        raise ValueError("train and validation opening-pair groups overlap")

    points: list[tuple[str, np.ndarray]] = []
    if train.target.size > learning_floor:
        points.append(
            (
                f"{learning_floor}-complete-groups",
                select_complete_groups(
                    train.groups, learning_floor, "20260831-learning"
                ),
            )
        )
    points.append(("full", np.ones(train.target.size, dtype=bool)))

    learning_curve = []
    for label, mask in points:
        learning_curve.append(
            {
                "label": label,
                "training_rows": int(mask.sum()),
                "training_groups": int(np.unique(train.groups[mask]).size),
                "losses": {
                    kind: _loss_report(
                        train,
                        validation,
                        mask,
                        kind,
                        bootstrap_replicates,
                    )
                    for kind in ("mse", "cross_entropy")
                },
            }
        )
    return {
        "schema": "neyrang-sanj-train-validation-diagnostic-v1",
        "protocol": {
            "sampling_unit": "opening-pair group",
            "baseline_scale": 1.0,
            "scale_bounds": [0.01, 4.0],
            "scale_iterations": 100,
            "learning_group_seed": "20260831-learning",
            "bootstrap_seed": 20260831,
            "bootstrap_replicates": bootstrap_replicates,
        },
        "train": partition_summary(train),
        "validation": partition_summary(validation),
        "learning_curve": learning_curve,
    }


def analyze_corpus(
    corpus_dir: Path,
    *,
    bootstrap_replicates: int = 10_000,
) -> dict[str, object]:
    train_path = corpus_dir / "train.features.tsv"
    validation_path = corpus_dir / "validation.features.tsv"
    train = load_partition(train_path)
    validation = load_partition(validation_path)
    report = analyze_partitions(
        train,
        validation,
        bootstrap_replicates=bootstrap_replicates,
    )
    report["inputs"] = {
        "train": {
            "path": str(train_path),
            "sha256": sha256_file(train_path),
        },
        "validation": {
            "path": str(validation_path),
            "sha256": sha256_file(validation_path),
        },
    }
    report["runtime"] = {
        "python": platform.python_version(),
        "numpy": np.__version__,
        "script_sha256": sha256_file(Path(__file__)),
    }
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("corpus_dir", type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--bootstrap-replicates", type=int, default=10_000)
    args = parser.parse_args()
    report = analyze_corpus(
        args.corpus_dir,
        bootstrap_replicates=args.bootstrap_replicates,
    )
    encoded = json.dumps(
        report, ensure_ascii=False, indent=2, sort_keys=True, allow_nan=False
    ) + "\n"
    if args.output is None:
        print(encoded, end="")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(encoded, encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
