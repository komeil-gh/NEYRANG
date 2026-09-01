from __future__ import annotations

import importlib.util
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPTS = Path(__file__).resolve().parents[1]
AUDITOR_PATH = SCRIPTS / "audit-nnue-data.py"


def load_module(name: str, path: Path):
    spec = importlib.util.spec_from_file_location(name, path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


OFFICIAL_FIXTURE = bytes(
    [
        0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0xFF, 0xFF,
        0x16, 0x42, 0x25, 0x61, 0x00, 0x00, 0x00, 0x00,
        0x88, 0x88, 0x88, 0x88, 0x9E, 0xCA, 0xAD, 0xE9,
        0x40, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00,
        0x0C, 0x07, 0x0A, 0x00,
        0x34, 0x09, 0x14, 0x00,
        0xC3, 0x09, 0xE2, 0xFF,
        0x3C, 0x0D, 0xFF, 0x7F,
        0x27, 0x09, 0xFF, 0x7F,
        0x00, 0x00, 0x00, 0x00,
    ]
)


class NnueDataAuditorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls.auditor = load_module("audit_nnue_data", AUDITOR_PATH)

    def test_official_fixture_is_independently_replayed(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "official.vf"
            path.write_bytes(OFFICIAL_FIXTURE)

            summary = self.auditor.audit_file(path)

        self.assertTrue(summary["ok"])
        self.assertEqual(summary["schema"], "neyrang-nnue-data-audit-v1")
        self.assertEqual(summary["bytes"], 56)
        self.assertEqual(summary["games"], 1)
        self.assertEqual(summary["scored_positions"], 5)
        self.assertEqual(summary["results"], {"white_win": 1})
        self.assertEqual(summary["score_cp"], {"min": -30, "max": 32767})

    def test_illegal_move_is_rejected_even_when_structure_is_well_formed(self) -> None:
        corrupted = bytearray(OFFICIAL_FIXTURE)
        corrupted[32:34] = (12 | (36 << 6)).to_bytes(2, "little")  # e2e5
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "illegal.vf"
            path.write_bytes(corrupted)
            with self.assertRaisesRegex(self.auditor.AuditError, "illegal move"):
                self.auditor.audit_file(path)

    def test_truncated_record_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "truncated.vf"
            path.write_bytes(OFFICIAL_FIXTURE[:-2])
            with self.assertRaisesRegex(self.auditor.AuditError, "truncated"):
                self.auditor.audit_file(path)

    def test_strict_selfplay_audit_requires_a_rules_complete_game(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "mate.vf"
            path.write_bytes(forced_mate_fixture(result=0))

            summary = self.auditor.audit_file(path, require_completed_games=True)

        self.assertEqual(summary["results"], {"black_win": 1})
        self.assertEqual(summary["completion_reasons"], {"checkmate": 1})

    def test_strict_selfplay_audit_rejects_wdl_that_disagrees_with_board(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "wrong-wdl.vf"
            path.write_bytes(forced_mate_fixture(result=2))

            with self.assertRaisesRegex(self.auditor.AuditError, "WDL"):
                self.auditor.audit_file(path, require_completed_games=True)

    def test_strict_selfplay_audit_binds_each_game_to_an_allowed_opening(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "mate.vf"
            path.write_bytes(forced_mate_fixture(result=0))

            summary = self.auditor.audit_file(
                path,
                require_completed_games=True,
                allowed_initial_positions={"rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1"},
            )
            with self.assertRaisesRegex(self.auditor.AuditError, "opening source"):
                self.auditor.audit_file(
                    path,
                    require_completed_games=True,
                    allowed_initial_positions={"7k/5K2/6Q1/8/8/8/8/8 w - - 0 1"},
                )

        self.assertRegex(summary["initial_positions_sha256"], r"^[0-9a-f]{64}$")


def forced_mate_fixture(result: int) -> bytes:
    header = bytearray(OFFICIAL_FIXTURE[:32])
    header[28:30] = (0).to_bytes(2, "little", signed=True)
    header[30] = result
    header[31] = 0
    records = bytearray()
    for source, destination, score in [
        (13, 21, 11),  # f2f3
        (52, 36, -22),  # e7e5
        (14, 30, -33),  # g2g4
        (59, 31, -29_999),  # d8h4#
    ]:
        records.extend((source | (destination << 6)).to_bytes(2, "little"))
        records.extend(score.to_bytes(2, "little", signed=True))
    return bytes(header + records + b"\0\0\0\0")


if __name__ == "__main__":
    unittest.main()
