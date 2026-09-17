#!/usr/bin/env python3
"""Build a teacher corpus for the NNUE side of a fixed SANJ blend."""

from __future__ import annotations

import argparse
import csv
from pathlib import Path


TEACHER_HEADER = "# neyrang-teacher-wdl-logit400-v2"


def counter_score(teacher_white: int, classical_stm: int, stm: str, mix: int) -> int:
    sign = 1 if stm == "w" else -1
    teacher_stm = sign * teacher_white
    network_stm = round((100 * teacher_stm - (100 - mix) * classical_stm) / mix)
    return sign * max(-3040, min(3040, network_stm))


def build(teacher: Path, features: Path, output: Path, mix: int) -> tuple[int, int]:
    if not 1 <= mix <= 100:
        raise ValueError("mix must be from 1 through 100")
    if output.exists():
        raise FileExistsError(f"refusing to overwrite {output}")

    clipped = 0
    rows = 0
    output.parent.mkdir(parents=True, exist_ok=True)
    with teacher.open(encoding="ascii") as teacher_stream, features.open(
        encoding="ascii", newline=""
    ) as feature_stream, output.open("x", encoding="ascii") as output_stream:
        if teacher_stream.readline().rstrip("\n") != TEACHER_HEADER:
            raise ValueError("unexpected teacher header")
        feature_rows = csv.DictReader(feature_stream, delimiter="\t")
        output_stream.write(f"{TEACHER_HEADER}\n")
        for line_number, (teacher_line, feature) in enumerate(
            zip(teacher_stream, feature_rows, strict=True), 2
        ):
            fen, separator, score_text = teacher_line.rstrip("\n").rpartition("|")
            if not separator:
                raise ValueError(f"teacher line {line_number}: missing separator")
            fen = fen.strip()
            if fen != feature["fen"]:
                raise ValueError(f"teacher line {line_number}: feature FEN differs")
            teacher_score = int(score_text.strip())
            stm = feature["stm"]
            classical_stm = int(feature["stm_cp"])
            raw_stm = (
                100 * (teacher_score if stm == "w" else -teacher_score)
                - (100 - mix) * classical_stm
            ) / mix
            clipped += abs(raw_stm) > 3040
            score = counter_score(teacher_score, classical_stm, stm, mix)
            output_stream.write(f"{fen} | {score}\n")
            rows += 1
    if rows == 0:
        raise ValueError("teacher corpus contains no rows")
    return rows, clipped


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("teacher", type=Path)
    parser.add_argument("features", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--mix", type=int, required=True)
    args = parser.parse_args()
    rows, clipped = build(args.teacher, args.features, args.output, args.mix)
    print(f"rows={rows} clipped={clipped} mix={args.mix} output={args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
