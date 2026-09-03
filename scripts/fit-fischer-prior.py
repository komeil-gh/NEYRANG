#!/usr/bin/env python3
"""Fit a compact, factorised root-ordering prior from teacher-gated moves."""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import struct
import sys
from array import array
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Iterable

import chess

SCHEMA = "neyrang-fischer-prior-v1"
MAGIC = b"NYRFSP1\0"
VERSION = 1
PHASES = 3
PIECES = 6
SQUARES = 64
FROM_TO_LEN = PHASES * PIECES * SQUARES * SQUARES
PIECE_TO_LEN = PHASES * PIECES * SQUARES
HEADER = struct.Struct("<8sIIIIII")
NON_PAWN_VALUE = {
    chess.KNIGHT: 3,
    chess.BISHOP: 3,
    chess.ROOK: 5,
    chess.QUEEN: 9,
}


class FitError(RuntimeError):
    """A fail-closed prior fitting error."""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--train", required=True, type=Path)
    parser.add_argument("--validation", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--report", required=True, type=Path)
    parser.add_argument("--max-cp-loss", type=int, default=50)
    parser.add_argument("--prior-strength", type=float, default=16.0)
    parser.add_argument("--shrinkage", type=float, default=32.0)
    parser.add_argument("--min-exposure", type=int, default=4)
    parser.add_argument("--quant", type=int, default=256)
    return parser.parse_args()


def sha256_file(path: Path) -> tuple[str, int]:
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(chunk)
            size += len(chunk)
    return digest.hexdigest(), size


def records(path: Path) -> Iterable[dict[str, Any]]:
    with path.open(encoding="utf-8") as stream:
        for line_number, line in enumerate(stream, start=1):
            try:
                record = json.loads(line)
                board = chess.Board(record["fen"])
                move = chess.Move.from_uci(record["move"])
                int(record["teacher_loss_cp"])
            except (KeyError, ValueError, json.JSONDecodeError) as error:
                raise FitError(f"invalid label line {line_number} in {path}: {error}") from error
            if move not in board.legal_moves:
                raise FitError(f"illegal chosen move on line {line_number} in {path}")
            yield record


def phase(board: chess.Board) -> int:
    material = sum(
        value * len(board.pieces(piece, color))
        for piece, value in NON_PAWN_VALUE.items()
        for color in chess.COLORS
    )
    return 0 if material >= 52 else 1 if material >= 24 else 2


def normalized_square(square: chess.Square, turn: chess.Color) -> int:
    return square if turn == chess.WHITE else square ^ 56


def indices(board: chess.Board, move: chess.Move) -> tuple[int, int]:
    piece = board.piece_type_at(move.from_square)
    if piece is None:
        raise FitError("legal move has no source piece")
    bucket = phase(board)
    source = normalized_square(move.from_square, board.turn)
    target = normalized_square(move.to_square, board.turn)
    piece_index = piece - 1
    from_to = (((bucket * PIECES + piece_index) * SQUARES + source) * SQUARES) + target
    piece_to = ((bucket * PIECES + piece_index) * SQUARES) + target
    return from_to, piece_to


def fit_table(
    selected: array, exposure: array, phase_totals: list[tuple[int, int]],
    block_size: int, prior_strength: float, shrinkage: float,
    min_exposure: int, quant: int,
) -> array:
    weights = array("h", [0]) * len(exposure)
    for index, count in enumerate(exposure):
        if count < min_exposure:
            continue
        bucket = index // block_size
        phase_selected, phase_exposure = phase_totals[bucket]
        baseline = phase_selected / phase_exposure
        probability = (selected[index] + prior_strength * baseline) / (
            count + prior_strength
        )
        raw = math.log(probability / baseline) * (count / (count + shrinkage))
        weights[index] = max(-32767, min(32767, round(raw * quant)))
    return weights


def score_move(board: chess.Board, move: chess.Move, tables: tuple[array, array]) -> int:
    from_to, piece_to = indices(board, move)
    return tables[0][from_to] + tables[1][piece_to]


def choose(board: chess.Board, tables: tuple[array, array]) -> tuple[chess.Move, list[chess.Move]]:
    ranked = sorted(
        board.legal_moves,
        key=lambda move: (-score_move(board, move, tables), move.uci()),
    )
    return ranked[0], ranked


def evaluate(path: Path, tables: tuple[array, array], max_cp_loss: int) -> dict[str, Any]:
    total = 0
    top1 = 0
    top3 = 0
    uniform_sum = 0.0
    losses: Counter[str] = Counter()
    for record in records(path):
        loss = int(record["teacher_loss_cp"])
        if loss > max_cp_loss:
            losses["teacher_rejected"] += 1
            continue
        board = chess.Board(record["fen"])
        chosen_move = chess.Move.from_uci(record["move"])
        best, ranked = choose(board, tables)
        total += 1
        top1 += best == chosen_move
        top3 += chosen_move in ranked[:3]
        uniform_sum += 1.0 / board.legal_moves.count()
    if total == 0:
        raise FitError(f"no teacher-accepted validation decisions in {path}")
    return {
        "teacher_accepted": total,
        "teacher_rejected": losses["teacher_rejected"],
        "top1": top1 / total,
        "top3": top3 / total,
        "uniform_top1_expectation": uniform_sum / total,
    }


def main() -> int:
    args = parse_args()
    try:
        if args.max_cp_loss < 0 or args.prior_strength <= 0 or args.shrinkage < 0:
            raise FitError("loss, prior and shrinkage parameters are invalid")
        if args.min_exposure <= 0 or not 0 < args.quant <= 4096:
            raise FitError("min-exposure and quant are invalid")
        train_path = args.train.resolve()
        validation_path = args.validation.resolve()
        output_path = args.output.resolve()
        report_path = args.report.resolve()
        if not train_path.is_file() or not validation_path.is_file():
            raise FitError("train and validation labels must be files")
        if output_path.exists() or report_path.exists():
            raise FitError("refusing to overwrite an artifact or report")

        from_to_selected = array("I", [0]) * FROM_TO_LEN
        from_to_exposure = array("I", [0]) * FROM_TO_LEN
        piece_to_selected = array("I", [0]) * PIECE_TO_LEN
        piece_to_exposure = array("I", [0]) * PIECE_TO_LEN
        phase_totals = [[0, 0] for _ in range(PHASES)]
        accepted = rejected = 0
        for record in records(train_path):
            if int(record["teacher_loss_cp"]) > args.max_cp_loss:
                rejected += 1
                continue
            board = chess.Board(record["fen"])
            chosen_move = chess.Move.from_uci(record["move"])
            bucket = phase(board)
            legal_moves = list(board.legal_moves)
            phase_totals[bucket][0] += 1
            phase_totals[bucket][1] += len(legal_moves)
            for move in legal_moves:
                from_to, piece_to = indices(board, move)
                from_to_exposure[from_to] += 1
                piece_to_exposure[piece_to] += 1
            chosen_from_to, chosen_piece_to = indices(board, chosen_move)
            from_to_selected[chosen_from_to] += 1
            piece_to_selected[chosen_piece_to] += 1
            accepted += 1
        if accepted == 0:
            raise FitError("training labels contain no teacher-accepted decisions")

        frozen_totals = [(selected, exposure) for selected, exposure in phase_totals]
        from_to_weights = fit_table(
            from_to_selected,
            from_to_exposure,
            frozen_totals,
            PIECES * SQUARES * SQUARES,
            args.prior_strength,
            args.shrinkage,
            args.min_exposure,
            args.quant,
        )
        piece_to_weights = fit_table(
            piece_to_selected,
            piece_to_exposure,
            frozen_totals,
            PIECES * SQUARES,
            args.prior_strength,
            args.shrinkage,
            args.min_exposure,
            args.quant,
        )
        if sys.byteorder != "little":
            from_to_weights.byteswap()
            piece_to_weights.byteswap()

        output_path.parent.mkdir(parents=True, exist_ok=True)
        report_path.parent.mkdir(parents=True, exist_ok=True)
        temporary = output_path.with_name(f".{output_path.name}.tmp-{os.getpid()}")
        with temporary.open("wb") as stream:
            stream.write(
                HEADER.pack(
                    MAGIC, VERSION, PHASES, PIECES, SQUARES, FROM_TO_LEN, PIECE_TO_LEN
                )
            )
            stream.write(from_to_weights.tobytes())
            stream.write(piece_to_weights.tobytes())
        temporary.replace(output_path)

        train_hash, train_bytes = sha256_file(train_path)
        validation_hash, validation_bytes = sha256_file(validation_path)
        artifact_hash, artifact_bytes = sha256_file(output_path)
        tables = (from_to_weights, piece_to_weights)
        report = {
            "schema": SCHEMA,
            "created_at": datetime.now(timezone.utc).isoformat(),
            "inputs": {
                "train": {"path": str(train_path), "bytes": train_bytes, "sha256": train_hash},
                "validation": {
                    "path": str(validation_path),
                    "bytes": validation_bytes,
                    "sha256": validation_hash,
                },
            },
            "parameters": {
                "max_cp_loss": args.max_cp_loss,
                "prior_strength": args.prior_strength,
                "shrinkage": args.shrinkage,
                "min_exposure": args.min_exposure,
                "quant": args.quant,
                "phase_non_pawn_material_thresholds": [52, 24],
                "black_square_normalization": "vertical_flip_xor_56",
            },
            "training": {
                "teacher_accepted": accepted,
                "teacher_rejected": rejected,
                "phase_totals": frozen_totals,
                "nonzero_from_to_weights": sum(weight != 0 for weight in from_to_weights),
                "nonzero_piece_to_weights": sum(weight != 0 for weight in piece_to_weights),
            },
            "validation": evaluate(validation_path, tables, args.max_cp_loss),
            "artifact": {
                "path": str(output_path),
                "bytes": artifact_bytes,
                "sha256": artifact_hash,
                "format": "little-endian header followed by signed i16 tables",
            },
        }
        report_path.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    except (FitError, OSError, OverflowError) as error:
        print(f"fit-fischer-prior: {error}", file=sys.stderr)
        return 1
    print(f"trained on {accepted} teacher-gated decisions")
    print(f"artifact: {output_path}")
    print(f"report: {report_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
