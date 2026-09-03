#!/usr/bin/env python3
"""Audit fastchess UCI response intervals without trusting match results."""

from __future__ import annotations

import argparse
import hashlib
import json
import re
from dataclasses import dataclass
from pathlib import Path


ENGINE_LINE = re.compile(
    r"^\[Engine\]\s+\[[^\]]+\]\s+<[^>]*>\s+"
    r"(?P<engine>.+?)\s+(?P<direction><---|--->)\s+(?P<message>.*)$"
)
RELEVANT_WORD = re.compile(r"(?:^|\s)(?:go|info|bestmove)(?:\s|$)")


@dataclass
class EngineState:
    phase: str = "idle"
    open_go_line: int | None = None
    go_count: int = 0
    info_count: int = 0
    bestmove_count: int = 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Verify one bestmove per go and no late info in a fastchess trace."
    )
    parser.add_argument("--log", required=True, type=Path)
    parser.add_argument("--engine", action="append", required=True, dest="engines")
    parser.add_argument("--expected-go-count", type=int)
    parser.add_argument("--output", type=Path)
    return parser.parse_args()


def audit(path: Path, expected_engines: list[str], expected_go_count: int | None) -> dict:
    if not path.is_file():
        raise ValueError(f"trace is not a file: {path}")
    if len(expected_engines) != len(set(expected_engines)):
        raise ValueError("--engine values must be unique")
    if expected_go_count is not None and expected_go_count < 0:
        raise ValueError("--expected-go-count must be non-negative")

    raw = path.read_bytes()
    lines = raw.decode("utf-8").splitlines()
    states = {name: EngineState() for name in sorted(expected_engines)}
    violations: list[dict] = []

    def reject(line: int, engine: str, code: str, detail: str) -> None:
        violations.append(
            {"line": line, "engine": engine, "code": code, "detail": detail}
        )

    for number, line in enumerate(lines, 1):
        match = ENGINE_LINE.match(line)
        if match is None:
            if line.startswith("[Engine]") and RELEVANT_WORD.search(line):
                reject(number, "unknown", "malformed_relevant_line", line)
            continue

        engine = match.group("engine").strip()
        direction = match.group("direction")
        message = match.group("message").strip()
        command = message.split(maxsplit=1)[0] if message else ""
        if command not in {"go", "info", "bestmove"}:
            continue
        if engine not in states:
            reject(number, engine, "unexpected_engine", message)
            continue

        state = states[engine]
        if direction == "<---" and command == "go":
            if state.phase == "open":
                reject(
                    number,
                    engine,
                    "overlapping_go",
                    f"previous go at line {state.open_go_line} has no bestmove",
                )
            state.phase = "open"
            state.open_go_line = number
            state.go_count += 1
            continue

        if direction != "--->":
            continue
        if command == "info":
            state.info_count += 1
            if state.phase == "closed":
                reject(number, engine, "info_after_bestmove", message)
        elif command == "bestmove":
            state.bestmove_count += 1
            if state.phase == "open":
                state.phase = "closed"
                state.open_go_line = None
            else:
                reject(
                    number,
                    engine,
                    "orphan_or_duplicate_bestmove",
                    message,
                )

    for engine, state in states.items():
        if state.phase == "open":
            reject(
                len(lines),
                engine,
                "unclosed_go",
                f"go at line {state.open_go_line} has no bestmove",
            )
        if state.go_count != state.bestmove_count:
            reject(
                len(lines),
                engine,
                "response_count_mismatch",
                f"go={state.go_count} bestmove={state.bestmove_count}",
            )

    total_go = sum(state.go_count for state in states.values())
    total_bestmove = sum(state.bestmove_count for state in states.values())
    if expected_go_count is not None and total_go != expected_go_count:
        reject(
            len(lines),
            "all",
            "expected_go_count_mismatch",
            f"expected={expected_go_count} observed={total_go}",
        )

    return {
        "schema": "neyrang-uci-trace-audit-v1",
        "status": "passed" if not violations else "failed",
        "input": {
            "path": str(path),
            "bytes": len(raw),
            "lines": len(lines),
            "sha256": hashlib.sha256(raw).hexdigest(),
        },
        "expected_engines": sorted(expected_engines),
        "expected_go_count": expected_go_count,
        "totals": {
            "go": total_go,
            "info": sum(state.info_count for state in states.values()),
            "bestmove": total_bestmove,
        },
        "engines": {
            engine: {
                "go": state.go_count,
                "info": state.info_count,
                "bestmove": state.bestmove_count,
                "final_phase": state.phase,
            }
            for engine, state in states.items()
        },
        "violations": violations,
    }


def main() -> int:
    args = parse_args()
    try:
        result = audit(args.log, args.engines, args.expected_go_count)
    except (OSError, UnicodeError, ValueError) as error:
        raise SystemExit(str(error)) from error

    rendered = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output is None:
        print(rendered, end="")
    else:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        temporary = args.output.with_name(args.output.name + ".tmp")
        temporary.write_text(rendered, encoding="utf-8")
        temporary.replace(args.output)
        print(
            f"status={result['status']} go={result['totals']['go']} "
            f"bestmove={result['totals']['bestmove']} violations={len(result['violations'])}"
        )
    return 0 if result["status"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
