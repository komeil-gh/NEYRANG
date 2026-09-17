#!/usr/bin/env python3
"""Fit bounded symmetric PSQT residuals to SANJ teacher traces."""

from __future__ import annotations

import argparse
import csv
import json
import math
from pathlib import Path

import numpy as np
from scipy import sparse
from scipy.sparse.linalg import lsqr


PIECES = "PNBRQK"
FEATURES = len(PIECES) * 32
MAX_PHASE = 24


def board_coefficients(board_fen: str) -> dict[int, int]:
    coefficients: dict[int, int] = {}
    ranks = board_fen.split("/")
    if len(ranks) != 8:
        raise ValueError("invalid FEN board")
    for fen_rank, row in enumerate(ranks):
        rank = 7 - fen_rank
        file = 0
        for char in row:
            if char.isdigit():
                file += int(char)
                continue
            upper = char.upper()
            if upper not in PIECES or file >= 8:
                raise ValueError("invalid FEN board")
            white = char.isupper()
            relative_rank = rank if white else 7 - rank
            folded_file = min(file, 7 - file)
            index = PIECES.index(upper) * 32 + relative_rank * 4 + folded_file
            coefficients[index] = coefficients.get(index, 0) + (1 if white else -1)
            file += 1
        if file != 8:
            raise ValueError("invalid FEN board")
    return coefficients


def load(path: Path):
    data: list[float] = []
    indices: list[int] = []
    indptr = [0]
    targets = []
    baselines = []
    with path.open(encoding="ascii", newline="") as stream:
        for row in csv.DictReader(stream, delimiter="\t"):
            phase = int(row["phase"])
            if not 0 <= phase <= MAX_PHASE:
                raise ValueError("phase outside evaluator contract")
            coefficients = board_coefficients(row["fen"].split()[0])
            for index, value in coefficients.items():
                if phase:
                    indices.append(index)
                    data.append(value * phase / MAX_PHASE)
                if phase < MAX_PHASE:
                    indices.append(FEATURES + index)
                    data.append(value * (MAX_PHASE - phase) / MAX_PHASE)
            indptr.append(len(indices))
            probability = float(row["target"])
            targets.append(400.0 * math.log(probability / (1.0 - probability)))
            baselines.append(float(row["white_cp"]))
    matrix = sparse.csr_matrix(
        (np.asarray(data), np.asarray(indices), np.asarray(indptr)),
        shape=(len(targets), 2 * FEATURES),
    )
    return matrix, np.asarray(targets), np.asarray(baselines)


def metrics(target_cp, predicted_cp):
    target = 1.0 / (1.0 + np.exp(-target_cp / 400.0))
    predicted = 1.0 / (1.0 + np.exp(-predicted_cp / 400.0))
    clipped = np.clip(predicted, np.finfo(float).eps, 1 - np.finfo(float).eps)
    return {
        "bce": float(np.mean(-target * np.log(clipped) - (1 - target) * np.log(1 - clipped))),
        "mse": float(np.mean(np.square(predicted - target))),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("train", type=Path)
    parser.add_argument("validation", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--alphas", default="100,1000,10000")
    parser.add_argument("--limit", type=int, default=96)
    args = parser.parse_args()
    if args.output.exists():
        raise FileExistsError(f"refusing to overwrite {args.output}")
    alphas = [float(value) for value in args.alphas.split(",")]
    if not alphas or min(alphas) <= 0 or args.limit <= 0:
        raise ValueError("alphas and limit must be positive")

    train_x, train_target, train_baseline = load(args.train)
    validation_x, validation_target, validation_baseline = load(args.validation)
    candidates = []
    for alpha in alphas:
        residual = train_target - train_baseline
        fitted = lsqr(
            train_x,
            residual,
            damp=math.sqrt(alpha),
            atol=1e-8,
            btol=1e-8,
            iter_lim=200,
        )[0]
        weights = np.clip(np.rint(fitted), -args.limit, args.limit).astype(int)
        train_metrics = metrics(train_target, train_baseline + train_x @ weights)
        validation_metrics = metrics(
            validation_target, validation_baseline + validation_x @ weights
        )
        candidates.append(
            {
                "alpha": alpha,
                "train": train_metrics,
                "validation": validation_metrics,
                "weights": weights.tolist(),
            }
        )
    candidates.sort(
        key=lambda candidate: (
            candidate["validation"]["bce"],
            candidate["validation"]["mse"],
            candidate["alpha"],
        )
    )
    result = {
        "schema": "neyrang-sanj-psqt-residual-fit-v1",
        "train_rows": train_x.shape[0],
        "validation_rows": validation_x.shape[0],
        "feature_order": "mg then eg; P,N,B,R,Q,K; relative rank then folded file",
        "weight_limit": args.limit,
        "baseline_train": metrics(train_target, train_baseline),
        "baseline_validation": metrics(validation_target, validation_baseline),
        "candidates": candidates,
        "selected": candidates[0],
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="ascii")
    selected = result["selected"]
    print(
        f"selected_alpha={selected['alpha']} "
        f"validation_bce={selected['validation']['bce']:.12f} "
        f"validation_mse={selected['validation']['mse']:.12f}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
