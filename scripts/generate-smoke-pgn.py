#!/usr/bin/env python3
"""Generate paired NEYRANG-versus-Stockfish protocol smoke games as PGN."""

from __future__ import annotations

import argparse
import datetime as dt
from pathlib import Path

import chess
import chess.engine
import chess.pgn


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--neyrang", type=Path, required=True)
    parser.add_argument("--stockfish", type=Path, required=True)
    parser.add_argument("--openings", type=Path, default=Path("scripts/openings.epd"))
    parser.add_argument("--output", type=Path, default=Path("results/neyrang-vs-stockfish-smoke.pgn"))
    parser.add_argument("--pairs", type=int, default=5)
    parser.add_argument("--neyrang-depth", type=int, default=3)
    parser.add_argument("--stockfish-depth", type=int, default=1)
    parser.add_argument("--max-plies", type=int, default=400)
    return parser.parse_args()


def load_openings(path: Path, pairs: int) -> list[chess.Board]:
    if pairs < 1:
        raise ValueError("--pairs must be at least 1")

    boards: list[chess.Board] = []
    for line_number, raw_line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw_line.strip()
        if not line or line.startswith("#"):
            continue
        fields = line.split()
        if len(fields) == 4:
            line = f"{line} 0 1"
        try:
            boards.append(chess.Board(line))
        except ValueError as error:
            raise ValueError(f"invalid opening at {path}:{line_number}: {error}") from error
        if len(boards) == pairs:
            break

    if len(boards) < pairs:
        raise ValueError(f"requested {pairs} pairs but {path} contains only {len(boards)} openings")
    return boards


def play_game(
    opening: chess.Board,
    neyrang: chess.engine.SimpleEngine,
    stockfish: chess.engine.SimpleEngine,
    neyrang_is_white: bool,
    neyrang_depth: int,
    stockfish_depth: int,
    max_plies: int,
) -> chess.pgn.Game:
    board = opening.copy(stack=False)
    game = chess.pgn.Game()
    game.setup(board)
    node: chess.pgn.GameNode = game

    for _ in range(max_plies):
        if board.is_game_over(claim_draw=True):
            break
        neyrang_turn = board.turn == chess.WHITE if neyrang_is_white else board.turn == chess.BLACK
        engine = neyrang if neyrang_turn else stockfish
        depth = neyrang_depth if neyrang_turn else stockfish_depth
        result = engine.play(board, chess.engine.Limit(depth=depth))
        if result.move not in board.legal_moves:
            raise RuntimeError(f"engine returned illegal move {result.move} for {board.fen()}")
        board.push(result.move)
        node = node.add_variation(result.move)
    else:
        raise RuntimeError(f"game exceeded the {max_plies}-ply safety limit")

    outcome = board.outcome(claim_draw=True)
    if outcome is None:
        raise RuntimeError("game ended without a terminal outcome")
    game.headers["Result"] = outcome.result()
    game.headers["Termination"] = outcome.termination.name.replace("_", " ").title()
    return game


def main() -> None:
    args = parse_args()
    for executable in (args.neyrang, args.stockfish):
        if not executable.is_file():
            raise FileNotFoundError(executable)

    openings = load_openings(args.openings, args.pairs)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    today = dt.date.today().strftime("%Y.%m.%d")

    neyrang = chess.engine.SimpleEngine.popen_uci(str(args.neyrang.resolve()))
    stockfish = chess.engine.SimpleEngine.popen_uci(str(args.stockfish.resolve()))
    games: list[chess.pgn.Game] = []
    try:
        for opening_index, opening in enumerate(openings, 1):
            for neyrang_is_white in (True, False):
                game = play_game(
                    opening,
                    neyrang,
                    stockfish,
                    neyrang_is_white,
                    args.neyrang_depth,
                    args.stockfish_depth,
                    args.max_plies,
                )
                game_number = len(games) + 1
                game.headers["Event"] = "NEYRANG UCI Protocol Smoke"
                game.headers["Site"] = "Local"
                game.headers["Date"] = today
                game.headers["Round"] = str(game_number)
                neyrang_name = neyrang.id.get("name", "NEYRANG")
                stockfish_name = stockfish.id.get("name", "Stockfish")
                game.headers["White"] = neyrang_name if neyrang_is_white else stockfish_name
                game.headers["Black"] = stockfish_name if neyrang_is_white else neyrang_name
                game.headers["OpeningPair"] = str(opening_index)
                game.headers["NEYRANGDepth"] = str(args.neyrang_depth)
                game.headers["StockfishDepth"] = str(args.stockfish_depth)
                games.append(game)
    finally:
        neyrang.quit()
        stockfish.quit()

    rendered_games: list[str] = []
    for game in games:
        exporter = chess.pgn.StringExporter(headers=True, variations=True, comments=True)
        rendered_games.append(game.accept(exporter).rstrip())

    with args.output.open("w", encoding="utf-8", newline="\n") as output:
        output.write("\n\n".join(rendered_games))
        output.write("\n")

    print(f"wrote {len(games)} complete games to {args.output}")


if __name__ == "__main__":
    main()
