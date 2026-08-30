from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path

import chess
import chess.pgn


SCRIPT = Path(__file__).resolve().parents[1] / "select-fresh-openings.py"
SPEC = importlib.util.spec_from_file_location("select_fresh_openings", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
selector = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = selector
SPEC.loader.exec_module(selector)


class FreshOpeningSelectorTests(unittest.TestCase):
    def test_selection_is_deterministic_and_applies_all_exclusions(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            openings = make_openings(8)
            book = root / "book.epd"
            book.write_text("\n".join(reversed(openings)) + "\n", encoding="utf-8")
            excluded_epd = root / "excluded.epd"
            excluded_epd.write_text(openings[0] + "\n", encoding="utf-8")
            excluded_pgn = root / "excluded.pgn"
            write_pgn(excluded_pgn, openings[1])

            first = root / "first.epd"
            second = root / "second.epd"
            summary = selector.select_openings(
                make_config(root, book, excluded_pgn, excluded_epd, first)
            )
            selector.select_openings(
                make_config(root, book, excluded_pgn, excluded_epd, second)
            )

            self.assertEqual(first.read_bytes(), second.read_bytes())
            selected = {
                selector.canonical_fen(line)
                for line in first.read_text(encoding="utf-8").splitlines()
            }
            self.assertEqual(len(selected), 4)
            self.assertNotIn(selector.canonical_fen(openings[0]), selected)
            self.assertNotIn(selector.canonical_fen(openings[1]), selected)
            self.assertEqual(summary["counts"]["eligible"], 6)
            self.assertEqual(summary["counts"]["unselected_eligible"], 2)

    def test_duplicate_book_positions_are_canonicalized(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            openings = make_openings(2)
            duplicate = " ".join(openings[0].split()[:4]) + " 99 42"
            book = root / "book.epd"
            book.write_text(
                f"{openings[0]}\n{duplicate}\n{openings[1]}\n", encoding="utf-8"
            )
            output = root / "output.epd"
            summary = selector.select_openings(
                selector.SelectionConfig(root, book, (), (), output, "seed", 2)
            )
            self.assertEqual(summary["book"]["duplicates"], 1)
            self.assertEqual(summary["output"]["unique_openings"], 2)

    def test_insufficient_and_existing_outputs_fail_closed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            book = root / "book.epd"
            book.write_text(make_openings(1)[0] + "\n", encoding="utf-8")
            output = root / "output.epd"
            config = selector.SelectionConfig(root, book, (), (), output, "seed", 2)
            with self.assertRaisesRegex(selector.SelectionError, "only 1 remain"):
                selector.select_openings(config)

            output.write_text("occupied\n", encoding="utf-8")
            config = selector.SelectionConfig(root, book, (), (), output, "seed", 1)
            with self.assertRaisesRegex(selector.SelectionError, "refusing to overwrite"):
                selector.select_openings(config)


def make_config(
    root: Path,
    book: Path,
    excluded_pgn: Path,
    excluded_epd: Path,
    output: Path,
) -> selector.SelectionConfig:
    return selector.SelectionConfig(
        repo_root=root,
        book=book,
        excluded_pgns=(excluded_pgn,),
        excluded_epds=(excluded_epd,),
        output=output,
        seed="deterministic-seed",
        count=4,
    )


def make_openings(count: int) -> list[str]:
    board = chess.Board()
    result = []
    for index in range(count):
        result.append(board.fen(en_passant="fen"))
        legal = sorted(board.legal_moves, key=lambda item: item.uci())
        move = legal[index % len(legal)]
        board.push(move)
    return result


def write_pgn(path: Path, fen: str) -> None:
    game = chess.pgn.Game()
    game.setup(chess.Board(fen))
    game.headers["Result"] = "*"
    exporter = chess.pgn.StringExporter(headers=True, variations=False, comments=False)
    path.write_text(game.accept(exporter) + "\n\n", encoding="utf-8")


if __name__ == "__main__":
    unittest.main()
