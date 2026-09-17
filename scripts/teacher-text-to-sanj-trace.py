#!/usr/bin/env python3
"""Convert score-only teacher text into grouped SANJ trace input."""

from __future__ import annotations

import argparse
import math
from pathlib import Path


HEADER = "# neyrang-teacher-wdl-logit400-v2"


def ply_from_fen(fen: str) -> int:
    fields = fen.split()
    if len(fields) != 6 or fields[1] not in {"w", "b"}:
        raise ValueError("teacher row has an invalid FEN")
    fullmove = int(fields[5])
    if fullmove < 1:
        raise ValueError("teacher row has an invalid fullmove number")
    return 2 * (fullmove - 1) + (fields[1] == "b")


def convert(source: Path, output: Path, partition: str) -> tuple[int, int]:
    if "holdout" in source.name.casefold() or "holdout" in partition.casefold():
        raise ValueError("holdout input is forbidden before candidate selection")
    if output.exists():
        raise FileExistsError(f"refusing to overwrite {output}")
    rows = 0
    group = 0
    previous_ply: int | None = None
    output.parent.mkdir(parents=True, exist_ok=True)
    with source.open(encoding="ascii") as input_stream, output.open(
        "x", encoding="ascii"
    ) as output_stream:
        if input_stream.readline().rstrip("\n") != HEADER:
            raise ValueError("unexpected teacher text header")
        for line_number, line in enumerate(input_stream, 2):
            fen, separator, score_text = line.rstrip("\n").rpartition("|")
            if not separator:
                raise ValueError(f"line {line_number}: missing score separator")
            fen = fen.strip()
            score = int(score_text.strip())
            if not -3040 <= score <= 3040:
                raise ValueError(f"line {line_number}: score outside encoding contract")
            ply = ply_from_fen(fen)
            if previous_ply is None or ply <= previous_ply:
                group += 1
            previous_ply = ply
            rows += 1
            target = 1.0 / (1.0 + math.exp(-score / 400.0))
            record_id = (
                f"n13-{partition}:pair-{group:06d}:game-1:ply-{rows:09d}"
            )
            output_stream.write(f"{record_id}\t{target:.17g}\t{fen}\n")
    if rows == 0:
        raise ValueError("teacher text contains no rows")
    return rows, group


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--partition", required=True)
    args = parser.parse_args()
    rows, groups = convert(args.source, args.output, args.partition)
    print(f"rows={rows} groups={groups} output={args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
