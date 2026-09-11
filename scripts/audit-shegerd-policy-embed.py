#!/usr/bin/env python3
"""Independently verify the embedded SHEGERD-P2 integer-score fixture."""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
from dataclasses import dataclass
from pathlib import Path

import chess


MODEL_SHA256 = "64f103f44f55714a7396810d09de16c138a9a7d5587a9f77698eb472725c4c86"
MAGIC = b"NYRSHGP1"
VERSION = 1
FAMILY_SIZES = (4096, 384, 49, 3, 4160, 5)
FAMILY_OFFSETS = (0, 4096, 4480, 4529, 4532, 8692)
HEADER = "case\tfen\tmove\tprevious_to\texchange\texpected"


class AuditError(RuntimeError):
    """A fail-closed P2 parity-audit error."""


@dataclass(frozen=True)
class Case:
    name: str
    fen: str
    move: str
    previous_to: str
    exchange: int


CASES = (
    Case("white-opening-absent", chess.STARTING_FEN, "e2e4", "-", 0),
    Case(
        "black-opening-present",
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq - 0 1",
        "e7e5",
        "e4",
        0,
    ),
    Case("middle-phase-quiet", "4k3/rr5r/8/8/8/8/RR5R/4K3 w - - 0 1", "a2a3", "h7", 0),
    Case("winning-capture", "6k1/8/5p2/3qp3/2P1Q3/8/8/6K1 w - - 0 1", "c4d5", "d5", 100),
    Case("see-minus-two", "6k1/8/5p2/3qp3/2P1Q3/8/8/6K1 w - - 0 1", "e4e5", "d5", -100),
    Case("see-minus-one", "6k1/8/5p2/3qp3/2P1Q3/8/8/6K1 w - - 0 1", "e4e5", "d5", -1),
    Case("see-zero", "6k1/8/5p2/3qp3/2P1Q3/8/8/6K1 w - - 0 1", "c4d5", "d5", 0),
    Case("see-plus-one", "6k1/8/5p2/3qp3/2P1Q3/8/8/6K1 w - - 0 1", "c4d5", "d5", 1),
    Case("en-passant", "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1", "e5d6", "d5", 100),
    Case("quiet-promotion", "4k3/P7/8/8/8/8/8/4K3 w - - 0 1", "a7a8q", "-", 100),
)


def sha256(path: Path) -> tuple[str, int]:
    payload = path.read_bytes()
    return hashlib.sha256(payload).hexdigest(), len(payload)


def read_weights(path: Path) -> tuple[int, ...]:
    payload = path.read_bytes()
    digest = hashlib.sha256(payload).hexdigest()
    if digest != MODEL_SHA256:
        raise AuditError(f"unexpected model SHA-256 {digest}")
    if len(payload) != 36 + 2 * sum(FAMILY_SIZES):
        raise AuditError("unexpected model byte length")
    magic, version, *sizes = struct.unpack_from("<8s7I", payload)
    if magic != MAGIC or version != VERSION or tuple(sizes) != FAMILY_SIZES:
        raise AuditError("invalid model header")
    return struct.unpack_from(f"<{sum(FAMILY_SIZES)}h", payload, 36)


def normalized(square: int, color: chess.Color) -> int:
    return square if color == chess.WHITE else square ^ 56


def material_phase(board: chess.Board) -> int:
    material = sum(
        len(board.pieces(piece, color)) * value
        for color in chess.COLORS
        for piece, value in (
            (chess.KNIGHT, 3),
            (chess.BISHOP, 3),
            (chess.ROOK, 5),
            (chess.QUEEN, 9),
        )
    )
    return 0 if material >= 52 else 1 if material >= 24 else 2


def see_bucket(exchange: int) -> int:
    if exchange <= -100:
        return 0
    if exchange < 0:
        return 1
    if exchange == 0:
        return 2
    if exchange < 100:
        return 3
    return 4


def score(case: Case, weights: tuple[int, ...]) -> tuple[int, dict[str, object]]:
    board = chess.Board(case.fen)
    move = chess.Move.from_uci(case.move)
    if move not in board.legal_moves:
        raise AuditError(f"{case.name}: move is not legal")
    mover = board.piece_type_at(move.from_square)
    if mover is None:
        raise AuditError(f"{case.name}: move has no source piece")
    victim = chess.PAWN if board.is_en_passant(move) else board.piece_type_at(move.to_square) or 0
    promotion = move.promotion or 0
    origin = normalized(move.from_square, board.turn)
    destination = normalized(move.to_square, board.turn)
    previous = (
        0
        if case.previous_to == "-"
        else normalized(chess.parse_square(case.previous_to), board.turn) + 1
    )
    features = (
        origin * 64 + destination,
        (mover - 1) * 64 + destination,
        victim * 7 + promotion,
        material_phase(board),
        previous * 64 + destination,
        see_bucket(case.exchange),
    )
    value = sum(weights[offset + feature] for offset, feature in zip(FAMILY_OFFSETS, features, strict=True))
    coverage = {
        "color": "white" if board.turn == chess.WHITE else "black",
        "phase": features[3],
        "previous": "absent" if previous == 0 else "present",
        "capture": board.is_capture(move),
        "en_passant": board.is_en_passant(move),
        "promotion": promotion != 0,
        "see_bucket": features[5],
    }
    return value, coverage


def write_fixture(path: Path, weights: tuple[int, ...]) -> None:
    if path.exists():
        raise AuditError(f"refusing to overwrite {path}")
    lines = [HEADER]
    for case in CASES:
        value, _ = score(case, weights)
        lines.append(
            "\t".join(
                (case.name, case.fen, case.move, case.previous_to, str(case.exchange), str(value))
            )
        )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def audit(fixture: Path, weights: tuple[int, ...]) -> dict[str, object]:
    lines = fixture.read_text(encoding="utf-8").splitlines()
    if not lines or lines[0] != HEADER or len(lines) != len(CASES) + 1:
        raise AuditError("fixture shape does not match the registered public cases")
    observed_coverage: list[dict[str, object]] = []
    for line_number, (line, case) in enumerate(zip(lines[1:], CASES, strict=True), start=2):
        fields = line.split("\t")
        if len(fields) != 6:
            raise AuditError(f"line {line_number}: expected six fields")
        name, fen, move, previous_to, exchange, expected = fields
        if (name, fen, move, previous_to, int(exchange)) != (
            case.name,
            case.fen,
            case.move,
            case.previous_to,
            case.exchange,
        ):
            raise AuditError(f"line {line_number}: case definition changed")
        actual, coverage = score(case, weights)
        if actual != int(expected):
            raise AuditError(f"line {line_number}: expected {expected}, independently scored {actual}")
        observed_coverage.append(coverage)
    colors = {row["color"] for row in observed_coverage}
    phases = {row["phase"] for row in observed_coverage}
    previous = {row["previous"] for row in observed_coverage}
    buckets = {row["see_bucket"] for row in observed_coverage}
    if colors != {"white", "black"} or phases != {0, 1, 2} or previous != {"absent", "present"}:
        raise AuditError("fixture misses registered color, phase or previous-move coverage")
    if buckets != {0, 1, 2, 3, 4}:
        raise AuditError("fixture misses an exact SEE bucket")
    if not any(row["en_passant"] for row in observed_coverage):
        raise AuditError("fixture misses en passant")
    if not any(row["promotion"] for row in observed_coverage):
        raise AuditError("fixture misses promotion")
    fixture_hash, fixture_bytes = sha256(fixture)
    return {
        "schema": "neyrang-shegerd-p2-score-parity-audit-v1",
        "ok": True,
        "cases": len(CASES),
        "model_sha256": MODEL_SHA256,
        "fixture": {"bytes": fixture_bytes, "sha256": fixture_hash},
        "coverage": {
            "colors": sorted(colors),
            "phases": sorted(phases),
            "previous": sorted(previous),
            "see_buckets": sorted(buckets),
            "en_passant": True,
            "promotion": True,
        },
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--fixture", type=Path, required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--write-fixture", action="store_true")
    args = parser.parse_args()
    weights = read_weights(args.model)
    if args.write_fixture:
        if args.output is not None:
            raise AuditError("--output is not used while writing the fixture")
        write_fixture(args.fixture, weights)
        return
    if args.output is None:
        raise AuditError("--output is required for an audit")
    if args.output.exists():
        raise AuditError(f"refusing to overwrite {args.output}")
    report = audit(args.fixture, weights)
    args.output.write_text(json.dumps(report, sort_keys=True) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()
