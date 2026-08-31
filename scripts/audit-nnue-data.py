#!/usr/bin/env python3
"""Independently parse and legally replay an NEYRANG NNUE game corpus."""

from __future__ import annotations

import argparse
import hashlib
import json
import struct
import sys
from collections import Counter
from pathlib import Path
from typing import Any

import chess


HEADER_SIZE = 32
RECORD_SIZE = 4
MAX_GAME_PLIES = 1024
RESULT_NAMES = {0: "black_win", 1: "draw", 2: "white_win"}
PIECE_TYPES = {
    0: chess.PAWN,
    1: chess.KNIGHT,
    2: chess.BISHOP,
    3: chess.ROOK,
    4: chess.QUEEN,
    5: chess.KING,
    6: chess.ROOK,
}
PROMOTIONS = (chess.KNIGHT, chess.BISHOP, chess.ROOK, chess.QUEEN)
CASTLING_DESTINATIONS = {
    (chess.E1, chess.H1): chess.G1,
    (chess.E1, chess.A1): chess.C1,
    (chess.E8, chess.H8): chess.G8,
    (chess.E8, chess.A8): chess.C8,
}
CASTLING_ROOKS = {chess.A1, chess.H1, chess.A8, chess.H8}


class AuditError(RuntimeError):
    """A fail-closed NNUE corpus error."""


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("corpus", type=Path)
    parser.add_argument("--expected-sha256")
    parser.add_argument("--min-games", type=int, default=1)
    parser.add_argument("--min-scored-positions", type=int, default=1)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    try:
        summary = audit_file(args.corpus)
        if args.expected_sha256 and summary["sha256"] != args.expected_sha256.lower():
            raise AuditError(
                f"SHA-256 mismatch: expected {args.expected_sha256.lower()}, "
                f"got {summary['sha256']}"
            )
        if summary["games"] < args.min_games:
            raise AuditError(
                f"corpus has {summary['games']} games, below minimum {args.min_games}"
            )
        if summary["scored_positions"] < args.min_scored_positions:
            raise AuditError(
                f"corpus has {summary['scored_positions']} scored positions, "
                f"below minimum {args.min_scored_positions}"
            )
    except (AuditError, OSError, ValueError) as error:
        print(f"audit-nnue-data: {error}", file=sys.stderr)
        return 1
    print(json.dumps(summary, sort_keys=True))
    return 0


def audit_file(path: Path) -> dict[str, Any]:
    data = path.read_bytes()
    digest = hashlib.sha256(data).hexdigest()
    offset = 0
    game_count = 0
    scored_positions = 0
    result_counts: Counter[str] = Counter()
    score_min: int | None = None
    score_max: int | None = None
    max_game_plies = 0
    castling_moves = 0
    en_passant_moves = 0
    promotion_moves = 0

    while offset < len(data):
        game_offset = offset
        if len(data) - offset < HEADER_SIZE:
            raise AuditError(f"truncated game header at byte {offset}")
        header = data[offset : offset + HEADER_SIZE]
        offset += HEADER_SIZE
        board, result = decode_header(header, game_offset)
        result_counts[result] += 1
        game_count += 1
        game_plies = 0
        terminated = False

        while offset < len(data):
            if len(data) - offset < RECORD_SIZE:
                raise AuditError(f"truncated move record at byte {offset}")
            encoded, score = struct.unpack_from("<Hh", data, offset)
            offset += RECORD_SIZE
            if encoded == 0 and score == 0:
                terminated = True
                break
            if game_plies == MAX_GAME_PLIES:
                raise AuditError(
                    f"game at byte {game_offset} exceeds {MAX_GAME_PLIES} plies"
                )
            move, move_type = decode_move(board, encoded, game_plies)
            if move not in board.legal_moves:
                raise AuditError(
                    f"illegal move 0x{encoded:04x} at game {game_count}, ply {game_plies}"
                )
            if move_type == 1 and not board.is_en_passant(move):
                raise AuditError(f"move 0x{encoded:04x} has a false en-passant tag")
            if move_type == 2 and not board.is_castling(move):
                raise AuditError(f"move 0x{encoded:04x} has a false castling tag")
            if move_type == 0 and (
                board.is_en_passant(move) or board.is_castling(move) or move.promotion
            ):
                raise AuditError(f"move 0x{encoded:04x} has a missing special-move tag")
            board.push(move)
            game_plies += 1
            scored_positions += 1
            score_min = score if score_min is None else min(score_min, score)
            score_max = score if score_max is None else max(score_max, score)
            castling_moves += move_type == 2
            en_passant_moves += move_type == 1
            promotion_moves += move_type == 3

        if not terminated:
            raise AuditError(f"game at byte {game_offset} has no terminator")
        max_game_plies = max(max_game_plies, game_plies)

    if game_count == 0:
        raise AuditError("corpus contains no games")

    score_summary = None
    if score_min is not None and score_max is not None:
        score_summary = {"min": score_min, "max": score_max}
    return {
        "schema": "neyrang-nnue-data-audit-v1",
        "ok": True,
        "sha256": digest,
        "bytes": len(data),
        "games": game_count,
        "scored_positions": scored_positions,
        "max_game_plies": max_game_plies,
        "results": dict(sorted(result_counts.items())),
        "score_cp": score_summary,
        "special_moves": {
            "castling": castling_moves,
            "en_passant": en_passant_moves,
            "promotion": promotion_moves,
        },
    }


def decode_header(header: bytes, offset: int) -> tuple[chess.Board, str]:
    occupancy = int.from_bytes(header[0:8], "little")
    if occupancy == 0:
        raise AuditError(f"unsupported extension record at byte {offset}")
    if occupancy.bit_count() > 32:
        raise AuditError(f"position at byte {offset} has more than 32 pieces")

    board = chess.Board(None)
    occupied = occupancy
    piece_index = 0
    while occupied:
        least_bit = occupied & -occupied
        square = least_bit.bit_length() - 1
        occupied ^= least_bit
        packed = header[8 + piece_index // 2]
        code = packed & 0x0F if piece_index % 2 == 0 else packed >> 4
        kind_code = code & 7
        piece_type = PIECE_TYPES.get(kind_code)
        if piece_type is None:
            raise AuditError(f"reserved piece code {code} on square {square}")
        color = chess.WHITE if code & 8 == 0 else chess.BLACK
        board.set_piece_at(square, chess.Piece(piece_type, color))
        if kind_code == 6:
            if square not in CASTLING_ROOKS:
                raise AuditError(f"castling-rook marker on non-home square {square}")
            expected_color = (
                chess.WHITE if square in (chess.A1, chess.H1) else chess.BLACK
            )
            if color != expected_color:
                raise AuditError(f"castling-rook marker has wrong color on square {square}")
            board.castling_rights |= chess.BB_SQUARES[square]
        piece_index += 1

    state = header[24]
    board.turn = chess.BLACK if state & 0x80 else chess.WHITE
    en_passant = state & 0x7F
    if en_passant == 64:
        board.ep_square = None
    elif en_passant < 64:
        board.ep_square = en_passant
    else:
        raise AuditError(f"invalid en-passant square {en_passant} at byte {offset}")
    board.halfmove_clock = header[25]
    board.fullmove_number = int.from_bytes(header[26:28], "little")
    if board.fullmove_number == 0:
        raise AuditError(f"invalid fullmove number zero at byte {offset}")
    result = RESULT_NAMES.get(header[30])
    if result is None:
        raise AuditError(f"invalid WDL byte {header[30]} at byte {offset}")
    if not board.is_valid():
        raise AuditError(f"invalid chess position at byte {offset}: {board.fen()}")
    return board, result


def decode_move(board: chess.Board, encoded: int, ply: int) -> tuple[chess.Move, int]:
    from_square = encoded & 0x3F
    to_square = (encoded >> 6) & 0x3F
    promotion_code = (encoded >> 12) & 3
    move_type = encoded >> 14
    if move_type != 3 and promotion_code != 0:
        raise AuditError(
            f"move 0x{encoded:04x} has non-zero reserved promotion bits at ply {ply}"
        )
    if move_type == 2:
        destination = CASTLING_DESTINATIONS.get((from_square, to_square))
        if destination is None:
            raise AuditError(
                f"invalid castling encoding 0x{encoded:04x} at ply {ply}"
            )
        to_square = destination
    promotion = PROMOTIONS[promotion_code] if move_type == 3 else None
    return chess.Move(from_square, to_square, promotion=promotion), move_type


if __name__ == "__main__":
    raise SystemExit(main())
