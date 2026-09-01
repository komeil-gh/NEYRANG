#!/usr/bin/env python3
"""Independently audit a paired fastchess match and its retained artifacts."""

from __future__ import annotations

import argparse
import json
import math
import re
import statistics
import sys
from collections import Counter, defaultdict
from pathlib import Path
from typing import Any

import chess
import chess.pgn


RESULTS = {"1-0", "0-1", "1/2-1/2"}
RESULT_SCORE = {"1-0": 1.0, "0-1": 0.0, "1/2-1/2": 0.5}
TELEMETRY_PATTERNS = {
    "time_left": re.compile(r"(?:^|,\s*)tl=(-?\d+(?:\.\d+)?)s(?:,|$)"),
    "latency": re.compile(r"(?:^|,\s*)latency=(-?\d+(?:\.\d+)?)s(?:,|$)"),
    "nodes": re.compile(r"(?:^|,\s*)n=(\d+)(?:,|$)"),
    "seldepth": re.compile(r"(?:^|,\s*)sd=(\d+)(?:,|$)"),
    "nps": re.compile(r"(?:^|,\s*)nps=(\d+)(?:,|$)"),
    "hashfull": re.compile(r"(?:^|,\s*)hashfull=(\d+)(?:,|$)"),
    "pv": re.compile(r'(?:^|,\s*)pv="[^"]*"(?:,|$)'),
}
LOG_ANOMALIES = {
    "timeout": re.compile(
        r"\btimeouts?\s*:\s*[1-9]\d*\b|\btimed\s+out\b|\btimeout\b(?!\s*:\s*0\b)",
        re.IGNORECASE,
    ),
    "time_forfeit": re.compile(r"\btime\s+(?:forfeit|loss)\b", re.IGNORECASE),
    "illegal_move": re.compile(r"\billegal\s+move\b", re.IGNORECASE),
    "disconnect": re.compile(r"\bdisconnect(?:ed|ion)?\b", re.IGNORECASE),
    "crash": re.compile(
        r"\bcrashed\s*:\s*[1-9]\d*\b|\bcrash\b|\bcrashed\b(?!\s*:\s*0\b)",
        re.IGNORECASE,
    ),
    "stall": re.compile(r"\bstall(?:ed|ing)?\b", re.IGNORECASE),
    "fatal": re.compile(r"\bfatal\b", re.IGNORECASE),
    "protocol_error": re.compile(r"\bprotocol\s+error\b", re.IGNORECASE),
    "forfeit": re.compile(r"\bforfeit(?:ed)?\b", re.IGNORECASE),
}
WARNING_POLICIES = ("reject-all", "allow-opponent-threefold-pv")
WARNING_LINE = re.compile(r"\bwarning\b", re.IGNORECASE)
OPPONENT_THREEFOLD_PV_WARNING = re.compile(
    r"^Warning; PV continues after threefold repetition - "
    r"move ([a-h][1-8][a-h][1-8][qrbn]?) from (.+)$"
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pgn", required=True, type=Path)
    parser.add_argument("--candidate", required=True)
    parser.add_argument("--opponent", required=True)
    parser.add_argument("--expected-games", required=True, type=int)
    parser.add_argument("--expected-time-control")
    parser.add_argument(
        "--expected-openings",
        type=Path,
        help="require each color-reversed pair to match this EPD/FEN sequence",
    )
    parser.add_argument(
        "--expect-meta",
        action="append",
        default=[],
        metavar="KEY=VALUE",
        help="require an exact metadata field; repeat for multiple fields",
    )
    parser.add_argument("--log", type=Path)
    parser.add_argument("--meta", type=Path)
    parser.add_argument(
        "--warning-policy",
        choices=WARNING_POLICIES,
        default="reject-all",
        help="narrow post-run warning policy; defaults to rejecting every warning",
    )
    return parser.parse_args()


def nearest_rank(values: list[float], percentile: float) -> float:
    if not values:
        raise ValueError("cannot calculate a percentile of an empty sample")
    ordered = sorted(values)
    index = max(0, math.ceil(percentile * len(ordered)) - 1)
    return ordered[index]


def latency_summary(values: list[float]) -> dict[str, float | int]:
    return {
        "samples": len(values),
        "mean_ms": statistics.fmean(values) * 1000.0,
        "p50_ms": nearest_rank(values, 0.50) * 1000.0,
        "p95_ms": nearest_rank(values, 0.95) * 1000.0,
        "p99_ms": nearest_rank(values, 0.99) * 1000.0,
        "max_ms": max(values) * 1000.0,
    }


def raw_event_results(path: Path) -> list[tuple[str | None, str | None]]:
    raw = path.read_text(encoding="utf-8")
    blocks = [
        block
        for block in re.split(r"(?m)(?=^\[Event\s+\")", raw)
        if block.strip()
    ]
    results: list[tuple[str | None, str | None]] = []
    for block in blocks:
        header = re.search(r'(?m)^\[Result\s+"([^"]+)"\]\s*$', block)
        movetext = re.search(r"(1-0|0-1|1/2-1/2|\*)\s*$", block)
        results.append(
            (
                header.group(1) if header else None,
                movetext.group(1) if movetext else None,
            )
        )
    return results


def candidate_score(result: str, candidate_is_white: bool) -> float:
    white_score = RESULT_SCORE[result]
    return white_score if candidate_is_white else 1.0 - white_score


def parse_metadata(path: Path) -> dict[str, str]:
    fields: dict[str, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        if "=" in line:
            key, value = line.split("=", 1)
            fields[key] = value
    return fields


def audit_expected_metadata(
    actual: dict[str, str] | None, expected_items: list[str]
) -> tuple[dict[str, str], list[str]]:
    expected: dict[str, str] = {}
    errors: list[str] = []
    for item in expected_items:
        key, separator, value = item.partition("=")
        if not separator or not key:
            errors.append(f"invalid expected metadata field {item!r}; use KEY=VALUE")
            continue
        if key in expected and expected[key] != value:
            errors.append(f"conflicting expected metadata values for {key!r}")
            continue
        expected[key] = value

    if expected and actual is None:
        errors.append("expected metadata fields require --meta")
        return expected, errors
    if actual is not None:
        for key, value in expected.items():
            if actual.get(key) != value:
                errors.append(
                    f"metadata {key} is {actual.get(key)!r}, expected {value!r}"
                )
    return expected, errors


def audit_expected_openings(
    pair_fens: list[str], path: Path
) -> tuple[list[str], list[str]]:
    expected: list[str] = []
    errors: list[str] = []
    for line_number, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw.strip()
        if not line:
            continue
        try:
            expected.append(canonical_opening(line))
        except ValueError as error:
            errors.append(f"invalid expected opening at {path}:{line_number}: {error}")

    if len(pair_fens) != len(expected):
        errors.append(
            f"found {len(pair_fens)} opening pairs, expected {len(expected)} opening pairs"
        )
    for index, (actual, wanted) in enumerate(zip(pair_fens, expected, strict=False), 1):
        try:
            canonical_actual = canonical_opening(actual)
        except ValueError as error:
            errors.append(f"pair {index} has invalid opening FEN: {error}")
            continue
        if canonical_actual != wanted:
            errors.append(
                f"pair {index} opening is {canonical_actual!r}, expected {wanted!r}"
            )
    return expected, errors


def _round_sort_key(round_header: str) -> tuple[int, tuple[int, ...] | str]:
    parts = round_header.split(".")
    if parts and all(part.isdigit() for part in parts):
        return 0, tuple(int(part) for part in parts)
    return 1, round_header


def pair_games_by_round(
    games: list[chess.pgn.Game], expected_pairs: int
) -> tuple[list[tuple[chess.pgn.Game, chess.pgn.Game]], list[str]]:
    """Recover color-reversed pairs independently of PGN completion order."""
    errors: list[str] = []
    games_by_round: dict[str, list[chess.pgn.Game]] = defaultdict(list)

    for index, game in enumerate(games, 1):
        round_header = game.headers.get("Round")
        if not round_header:
            errors.append(f"game {index} has no round header")
            continue
        games_by_round[round_header].append(game)

    if len(games_by_round) != expected_pairs:
        errors.append(
            f"found {len(games_by_round)} round groups, expected {expected_pairs} pairs"
        )

    pairs: list[tuple[chess.pgn.Game, chess.pgn.Game]] = []
    for round_header in sorted(games_by_round, key=_round_sort_key):
        round_games = games_by_round[round_header]
        if len(round_games) != 2:
            errors.append(
                f"round {round_header!r} contains {len(round_games)} games, expected 2"
            )
            continue
        pairs.append((round_games[0], round_games[1]))

    return pairs, errors


def canonical_opening(value: str) -> str:
    fields = value.split()
    if len(fields) == 6:
        board = chess.Board(value)
    else:
        board = chess.Board()
        board.set_epd(value)
    return " ".join(board.fen(en_passant="fen").split()[:4])


def warning_is_allowed(
    line: str,
    *,
    warning_policy: str,
    candidate: str,
    opponent: str,
) -> bool:
    if warning_policy != "allow-opponent-threefold-pv":
        return False
    match = OPPONENT_THREEFOLD_PV_WARNING.fullmatch(line)
    return bool(match and match.group(2) == opponent and match.group(2) != candidate)


def scan_log(
    path: Path,
    *,
    warning_policy: str = "reject-all",
    candidate: str = "",
    opponent: str = "",
) -> tuple[dict[str, int], list[str], dict[str, Any], list[str]]:
    if warning_policy not in WARNING_POLICIES:
        raise ValueError(f"unknown warning policy: {warning_policy}")
    text = path.read_text(encoding="utf-8", errors="replace")
    lines = text.splitlines()
    counts: dict[str, int] = {}
    examples: list[str] = []
    allowed_warnings: list[str] = []
    rejected_warnings: list[str] = []
    for line in lines:
        if not WARNING_LINE.search(line):
            continue
        if warning_is_allowed(
            line,
            warning_policy=warning_policy,
            candidate=candidate,
            opponent=opponent,
        ):
            allowed_warnings.append(line)
        else:
            rejected_warnings.append(line)
    counts["warning"] = len(rejected_warnings)
    examples.extend(f"warning: {line}" for line in rejected_warnings[:3])
    for name, pattern in LOG_ANOMALIES.items():
        matched = [line for line in lines if pattern.search(line)]
        counts[name] = len(matched)
        examples.extend(f"{name}: {line}" for line in matched[:3])

    summaries = list(
        re.finditer(
            r"Games:\s*(\d+),\s*Wins:\s*(\d+),\s*Losses:\s*(\d+),"
            r"\s*Draws:\s*(\d+),\s*Points:\s*([0-9.]+)\s*\(([0-9.]+)\s*%\)",
            text,
        )
    )
    pentas = list(
        re.finditer(r"Ptnml\(0-2\):\s*\[\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*,\s*(\d+)\s*\]", text)
    )
    ratings = list(
        re.finditer(
            r"Elo:\s*([+-]?[0-9.]+)\s*\+/-\s*([0-9.]+),\s*"
            r"nElo:\s*([+-]?[0-9.]+)\s*\+/-\s*([0-9.]+)",
            text,
        )
    )

    final: dict[str, Any] = {}
    if summaries:
        values = summaries[-1].groups()
        final["games"] = int(values[0])
        final["wins"] = int(values[1])
        final["losses"] = int(values[2])
        final["draws"] = int(values[3])
        final["points"] = float(values[4])
        final["score_percent"] = float(values[5])
    if pentas:
        final["pentanomial"] = [int(value) for value in pentas[-1].groups()]
    if ratings:
        values = ratings[-1].groups()
        final["elo"] = float(values[0])
        final["elo_error"] = float(values[1])
        final["nelo"] = float(values[2])
        final["nelo_error"] = float(values[3])
    return counts, examples, final, allowed_warnings


def main() -> int:
    args = parse_args()
    errors: list[str] = []
    games: list[chess.pgn.Game] = []

    with args.pgn.open(encoding="utf-8") as pgn:
        while game := chess.pgn.read_game(pgn):
            games.append(game)

    if len(games) != args.expected_games:
        errors.append(f"expected {args.expected_games} games, parsed {len(games)}")
    if args.expected_games % 2:
        errors.append("expected game count must be even for a paired audit")

    raw_results = raw_event_results(args.pgn)
    if len(raw_results) != len(games):
        errors.append(
            f"raw event count {len(raw_results)} differs from parsed game count {len(games)}"
        )

    wdl = Counter()
    termination_counts = Counter()
    time_controls = Counter()
    color_counts = Counter()
    telemetry_missing = Counter()
    latency: dict[str, list[float]] = defaultdict(list)
    time_left: dict[str, list[float]] = defaultdict(list)
    total_plies = 0
    pair_scores: list[float] = []
    pair_opening_fens: list[str] = []
    unique_fens: set[str] = set()

    for index, game in enumerate(games):
        headers = game.headers
        white = headers.get("White")
        black = headers.get("Black")
        result = headers.get("Result")
        termination = headers.get("Termination")
        time_control = headers.get("TimeControl")
        fen = headers.get("FEN", chess.STARTING_FEN)

        if game.errors:
            errors.append(f"game {index + 1} has PGN parse errors: {game.errors}")
        if {white, black} != {args.candidate, args.opponent}:
            errors.append(
                f"game {index + 1} engines are {white!r}/{black!r}, expected candidate/opponent"
            )
            continue
        if result not in RESULTS:
            errors.append(f"game {index + 1} has invalid result {result!r}")
            continue
        if termination != "normal":
            errors.append(f"game {index + 1} termination is {termination!r}")
        if args.expected_time_control and time_control != args.expected_time_control:
            errors.append(
                f"game {index + 1} time control is {time_control!r}, expected {args.expected_time_control!r}"
            )
        if index < len(raw_results):
            header_result, movetext_result = raw_results[index]
            if header_result != result or movetext_result != result:
                errors.append(
                    f"game {index + 1} result mismatch: parsed={result!r}, "
                    f"raw_header={header_result!r}, movetext={movetext_result!r}"
                )

        candidate_is_white = white == args.candidate
        score = candidate_score(result, candidate_is_white)
        wdl[{1.0: "wins", 0.5: "draws", 0.0: "losses"}[score]] += 1
        color_counts["white" if candidate_is_white else "black"] += 1
        termination_counts[termination] += 1
        time_controls[time_control] += 1
        unique_fens.add(fen)

        board = game.board()
        for node in game.mainline():
            actor = white if board.turn == chess.WHITE else black
            total_plies += 1
            comment = node.comment
            for field, pattern in TELEMETRY_PATTERNS.items():
                match = pattern.search(comment)
                if match is None:
                    telemetry_missing[field] += 1
                    continue
                if field == "latency":
                    latency[actor].append(float(match.group(1)))
                elif field == "time_left":
                    time_left[actor].append(float(match.group(1)))
            board.push(node.move)

    expected_pairs = args.expected_games // 2
    paired_games, pairing_errors = pair_games_by_round(games, expected_pairs)
    errors.extend(pairing_errors)
    for pair_index, (left, right) in enumerate(paired_games):
        left_headers = left.headers
        right_headers = right.headers
        left_fen = left_headers.get("FEN", chess.STARTING_FEN)
        right_fen = right_headers.get("FEN", chess.STARTING_FEN)
        if left_fen != right_fen:
            errors.append(f"pair {pair_index + 1} has different opening FENs")
        pair_opening_fens.append(left_fen)
        if not (
            left_headers.get("White") == right_headers.get("Black")
            and left_headers.get("Black") == right_headers.get("White")
        ):
            errors.append(f"pair {pair_index + 1} does not reverse colors")

        pair_score = 0.0
        pair_valid = True
        for game in (left, right):
            result = game.headers.get("Result")
            white = game.headers.get("White")
            if result not in RESULTS or white not in {args.candidate, args.opponent}:
                pair_valid = False
                break
            pair_score += candidate_score(result, white == args.candidate)
        if pair_valid:
            pair_scores.append(pair_score)

    expected_openings: list[str] | None = None
    if args.expected_openings:
        expected_openings, opening_errors = audit_expected_openings(
            pair_opening_fens, args.expected_openings
        )
        errors.extend(opening_errors)

    if any(telemetry_missing.values()):
        errors.append(f"missing telemetry fields: {dict(telemetry_missing)}")
    if color_counts["white"] != color_counts["black"]:
        errors.append(f"candidate color imbalance: {dict(color_counts)}")

    pentanomial = [pair_scores.count(score / 2) for score in range(5)]
    telemetry: dict[str, Any] = {}
    for engine in (args.candidate, args.opponent):
        engine_latency = latency[engine]
        engine_time_left = time_left[engine]
        if not engine_latency or not engine_time_left:
            errors.append(f"missing telemetry samples for {engine}")
            continue
        telemetry[engine] = {
            "latency": latency_summary(engine_latency),
            "time_left": {
                "samples": len(engine_time_left),
                "min_s": min(engine_time_left),
                "zero_samples": sum(value == 0 for value in engine_time_left),
                "negative_samples": sum(value < 0 for value in engine_time_left),
            },
        }
        if any(value < 0 for value in engine_time_left):
            errors.append(f"negative time-left sample recorded for {engine}")

    metadata: dict[str, str] | None = None
    if args.meta:
        metadata = parse_metadata(args.meta)
        if metadata.get("status") != "completed":
            errors.append(f"metadata status is {metadata.get('status')!r}, expected 'completed'")
        if metadata.get("games") != str(args.expected_games):
            errors.append(
                f"metadata games is {metadata.get('games')!r}, expected {args.expected_games}"
            )
    if args.warning_policy != "reject-all":
        if metadata is None:
            errors.append("compatibility warning policy requires --meta")
        else:
            if metadata.get("warning_policy") != args.warning_policy:
                errors.append(
                    "metadata warning_policy is "
                    f"{metadata.get('warning_policy')!r}, expected {args.warning_policy!r}"
                )
            if metadata.get("strict") != "0":
                errors.append(
                    f"metadata strict is {metadata.get('strict')!r}, expected '0' "
                    "for compatibility warning policy"
                )
    expected_metadata, metadata_errors = audit_expected_metadata(
        metadata, args.expect_meta
    )
    errors.extend(metadata_errors)

    log_anomaly_counts: dict[str, int] | None = None
    log_summary: dict[str, Any] | None = None
    allowed_log_warnings: list[str] = []
    if args.log:
        log_anomaly_counts, examples, log_summary, allowed_log_warnings = scan_log(
            args.log,
            warning_policy=args.warning_policy,
            candidate=args.candidate,
            opponent=args.opponent,
        )
        if any(log_anomaly_counts.values()):
            errors.append(f"log anomalies found: {examples}")
        independently_counted = {
            "games": len(games),
            "wins": wdl["wins"],
            "losses": wdl["losses"],
            "draws": wdl["draws"],
            "pentanomial": pentanomial,
        }
        for field, value in independently_counted.items():
            if log_summary.get(field) != value:
                errors.append(
                    f"fastchess final {field} is {log_summary.get(field)!r}, "
                    f"independent audit found {value!r}"
                )

    summary = {
        "format": "neyrang-match-audit-v1",
        "ok": not errors,
        "pgn": str(args.pgn),
        "candidate": args.candidate,
        "opponent": args.opponent,
        "games": len(games),
        "pairs": len(pair_scores),
        "candidate_colors": dict(color_counts),
        "wdl": {
            "wins": wdl["wins"],
            "draws": wdl["draws"],
            "losses": wdl["losses"],
        },
        "score": wdl["wins"] + 0.5 * wdl["draws"],
        "score_percent": (
            100.0 * (wdl["wins"] + 0.5 * wdl["draws"]) / len(games)
            if games
            else 0.0
        ),
        "pentanomial": pentanomial,
        "unique_opening_fens": len(unique_fens),
        "expected_openings": (
            {
                "path": str(args.expected_openings),
                "pairs": len(expected_openings or []),
            }
            if args.expected_openings
            else None
        ),
        "terminations": dict(termination_counts),
        "time_controls": dict(time_controls),
        "plies": total_plies,
        "telemetry_missing": dict(telemetry_missing),
        "telemetry": telemetry,
        "log_anomaly_counts": log_anomaly_counts,
        "warning_policy": args.warning_policy,
        "allowed_log_warnings": allowed_log_warnings,
        "fastchess_final_summary": log_summary,
        "metadata": metadata,
        "expected_metadata": expected_metadata,
        "errors": errors,
        "python": sys.version.split()[0],
        "python_chess": chess.__version__,
    }
    print(json.dumps(summary, indent=2, sort_keys=True))
    return 0 if not errors else 1


if __name__ == "__main__":
    raise SystemExit(main())
