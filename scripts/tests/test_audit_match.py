from __future__ import annotations

import importlib.util
import tempfile
import unittest
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "audit-match.py"
SPEC = importlib.util.spec_from_file_location("audit_match", MODULE_PATH)
assert SPEC is not None and SPEC.loader is not None
AUDIT_MATCH = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(AUDIT_MATCH)


class ScanLogTests(unittest.TestCase):
    def scan(
        self,
        text: str,
        *,
        warning_policy: str = "reject-all",
        candidate: str = "NEYRANG-G1",
        opponent: str = "NEYRANG-0.2.0",
    ) -> tuple[dict[str, int], list[str]]:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "match.log"
            path.write_text(text, encoding="utf-8")
            counts, _, _, allowed = AUDIT_MATCH.scan_log(
                path,
                warning_policy=warning_policy,
                candidate=candidate,
                opponent=opponent,
            )
            return counts, allowed

    def test_zero_summary_counters_are_not_anomalies(self) -> None:
        counts, _ = self.scan("Player: NEYRANG\n  Timeouts: 0\n  Crashed: 0\n")

        self.assertEqual(counts["timeout"], 0)
        self.assertEqual(counts["crash"], 0)

    def test_nonzero_summary_counters_are_anomalies(self) -> None:
        counts, _ = self.scan("Player: NEYRANG\n  Timeouts: 2\n  Crashed: 1\n")

        self.assertEqual(counts["timeout"], 1)
        self.assertEqual(counts["crash"], 1)

    def test_free_form_failure_messages_remain_detected(self) -> None:
        counts, _ = self.scan(
            "engine timed out waiting for bestmove\nengine crash detected\n"
        )

        self.assertEqual(counts["timeout"], 1)
        self.assertEqual(counts["crash"], 1)

    def test_default_policy_rejects_every_warning(self) -> None:
        counts, allowed = self.scan(
            "Warning; PV continues after threefold repetition - move f7d7 from NEYRANG-0.2.0\n"
        )

        self.assertEqual(counts["warning"], 1)
        self.assertEqual(allowed, [])

    def test_explicit_policy_allows_only_named_opponent_threefold_pv(self) -> None:
        line = (
            "Warning; PV continues after threefold repetition - "
            "move f7d7 from NEYRANG-0.2.0"
        )
        counts, allowed = self.scan(
            f"{line}\n",
            warning_policy="allow-opponent-threefold-pv",
        )

        self.assertEqual(counts["warning"], 0)
        self.assertEqual(allowed, [line])

    def test_policy_rejects_same_warning_from_candidate(self) -> None:
        counts, allowed = self.scan(
            "Warning; PV continues after threefold repetition - move f7d7 from NEYRANG-G1\n",
            warning_policy="allow-opponent-threefold-pv",
        )

        self.assertEqual(counts["warning"], 1)
        self.assertEqual(allowed, [])

    def test_draw_rule_policy_allows_only_named_opponent(self) -> None:
        threefold = (
            "Warning; PV continues after threefold repetition - "
            "move f7d7 from NEYRANG-0.2.0"
        )
        fifty_move = (
            "Warning; PV continues after fifty-move rule - "
            "move a2a1q from NEYRANG-0.2.0"
        )
        candidate = (
            "Warning; PV continues after fifty-move rule - "
            "move a7a8q from NEYRANG-G1"
        )
        counts, allowed = self.scan(
            f"{threefold}\n{fifty_move}\n{candidate}\n",
            warning_policy="allow-opponent-draw-rule-pv",
        )

        self.assertEqual(counts["warning"], 1)
        self.assertEqual(allowed, [threefold, fifty_move])

    def test_policy_rejects_other_or_malformed_warnings(self) -> None:
        warnings = [
            "Warning; PV continues after fifty-move rule - move f7d7 from NEYRANG-0.2.0",
            "Warning; PV continues after checkmate - move f7d7 from NEYRANG-0.2.0",
            "Warning; PV continues after stalemate - move f7d7 from NEYRANG-0.2.0",
            "Warning; Illegal PV move - move f7d7 from NEYRANG-0.2.0",
            "Warning; Illegal move f7d7 played by NEYRANG-0.2.0",
            "Warning; PV continues after threefold repetition - move nope from NEYRANG-0.2.0",
            "Warning; unexpected runner condition",
        ]
        counts, allowed = self.scan(
            "\n".join(warnings) + "\n",
            warning_policy="allow-opponent-threefold-pv",
        )

        self.assertEqual(counts["warning"], len(warnings))
        self.assertEqual(allowed, [])

    def test_utf16_windows_log_final_summary_is_parsed(self) -> None:
        text = (
            "Elo: -227.15 +/- 43.59, nElo: -293.63 +/- 42.56\r\n"
            "Games: 256, Wins: 30, Losses: 177, Draws: 49, "
            "Points: 54.5 (21.29 %)\r\n"
            "Ptnml(0-2): [58, 40, 24, 3, 3]\r\n"
        )
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "match.log"
            path.write_text(text, encoding="utf-16")

            counts, _, final, _ = AUDIT_MATCH.scan_log(path)

        self.assertFalse(any(counts.values()))
        self.assertEqual(final["games"], 256)
        self.assertEqual(final["wins"], 30)
        self.assertEqual(final["losses"], 177)
        self.assertEqual(final["draws"], 49)
        self.assertEqual(final["pentanomial"], [58, 40, 24, 3, 3])
        self.assertEqual(final["elo"], -227.15)


class ExpectedMetadataTests(unittest.TestCase):
    def test_node_limit_requires_dash_time_control_in_metadata_and_pgn(self) -> None:
        self.assertEqual(
            AUDIT_MATCH.audit_limit_metadata(
                {"limit_mode": "nodes", "time_control": "-"},
                AUDIT_MATCH.Counter({"-": 10}),
            ),
            [],
        )
        errors = AUDIT_MATCH.audit_limit_metadata(
            {"limit_mode": "nodes", "time_control": "10+0.1"},
            AUDIT_MATCH.Counter({"-": 10}),
        )
        self.assertIn("expected '-'", errors[0])

    def test_exact_fields_pass(self) -> None:
        expected, errors = AUDIT_MATCH.audit_expected_metadata(
            {"limit_mode": "per-engine-nodes", "engine_a_nodes": "30000"},
            ["limit_mode=per-engine-nodes", "engine_a_nodes=30000"],
        )

        self.assertEqual(
            expected,
            {"limit_mode": "per-engine-nodes", "engine_a_nodes": "30000"},
        )
        self.assertEqual(errors, [])

    def test_missing_or_mismatched_fields_fail(self) -> None:
        _, errors = AUDIT_MATCH.audit_expected_metadata(
            {"limit_mode": "time"},
            ["limit_mode=per-engine-nodes", "engine_a_nodes=30000"],
        )

        self.assertEqual(len(errors), 2)
        self.assertIn("metadata limit_mode is 'time'", errors[0])
        self.assertIn("metadata engine_a_nodes is None", errors[1])

    def test_malformed_and_conflicting_expectations_fail(self) -> None:
        _, errors = AUDIT_MATCH.audit_expected_metadata(
            {"nodes": "30000"},
            ["missing-separator", "nodes=30000", "nodes=29200"],
        )

        self.assertEqual(len(errors), 2)
        self.assertIn("invalid expected metadata field", errors[0])
        self.assertIn("conflicting expected metadata values", errors[1])

    def test_expectations_require_metadata(self) -> None:
        _, errors = AUDIT_MATCH.audit_expected_metadata(
            None, ["limit_mode=per-engine-nodes"]
        )

        self.assertEqual(errors, ["expected metadata fields require --meta"])


class ExpectedOpeningTests(unittest.TestCase):
    @staticmethod
    def suite_path(directory: str) -> Path:
        path = Path(directory) / "suite.epd"
        path.write_text(
            "8/8/8/8/8/8/K6k/8 w - - 0 1\n"
            "8/8/8/8/8/8/1K5k/8 b - - 0 1\n"
            "8/8/8/8/8/8/2K4k/8 w - - 0 1\n",
            encoding="utf-8",
        )
        return path

    def test_uncapturable_en_passant_square_is_canonicalized_away(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "shard.epd"
            path.write_text(
                "rn1qkb1r/1b3ppp/p3pn2/2pp4/NpP5/3P1NP1/PP2PPBP/R1BQ1RK1 w kq c6\n",
                encoding="utf-8",
            )

            _, errors = AUDIT_MATCH.audit_expected_openings(
                [
                    "rn1qkb1r/1b3ppp/p3pn2/2pp4/NpP5/3P1NP1/PP2PPBP/R1BQ1RK1 w kq -"
                ],
                path,
            )

            self.assertEqual(errors, [])

    def test_exact_canonical_pair_sequence_passes(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "shard.epd"
            path.write_text(
                "8/8/8/8/8/8/K6k/8 w - - 0 1\n"
                "8/8/8/8/8/8/1K5k/8 b - - 12 40\n",
                encoding="utf-8",
            )

            expected, errors = AUDIT_MATCH.audit_expected_openings(
                [
                    "8/8/8/8/8/8/K6k/8 w - - 99 77",
                    "8/8/8/8/8/8/1K5k/8 b - - 0 1",
                ],
                path,
            )

            self.assertEqual(len(expected), 2)
            self.assertEqual(errors, [])

    def test_wrong_order_or_count_fails(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "shard.epd"
            path.write_text(
                "8/8/8/8/8/8/K6k/8 w - - 0 1\n"
                "8/8/8/8/8/8/1K5k/8 b - - 0 1\n",
                encoding="utf-8",
            )

            _, order_errors = AUDIT_MATCH.audit_expected_openings(
                [
                    "8/8/8/8/8/8/1K5k/8 b - - 0 1",
                    "8/8/8/8/8/8/K6k/8 w - - 0 1",
                ],
                path,
            )
            _, count_errors = AUDIT_MATCH.audit_expected_openings(
                ["8/8/8/8/8/8/K6k/8 w - - 0 1"], path
            )

            self.assertIn("pair 1 opening", order_errors[0])
            self.assertIn("expected 2 opening pairs", count_errors[0])

    def test_shuffled_suite_accepts_unique_subset_and_complete_cycle(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = self.suite_path(directory)
            suite, subset_errors = AUDIT_MATCH.audit_expected_opening_suite(
                [
                    "8/8/8/8/8/8/2K4k/8 w - - 7 9",
                    "8/8/8/8/8/8/K6k/8 w - - 0 1",
                ],
                path,
            )
            _, cycle_errors = AUDIT_MATCH.audit_expected_opening_suite(
                [suite[2], suite[0], suite[1]], path
            )

            self.assertEqual(subset_errors, [])
            self.assertEqual(cycle_errors, [])

    def test_shuffled_suite_accepts_exact_periodic_suffix(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = self.suite_path(directory)
            suite, _ = AUDIT_MATCH.read_canonical_openings(path)

            _, errors = AUDIT_MATCH.audit_expected_opening_suite(
                [suite[1], suite[2], suite[0], suite[1], suite[2]], path
            )

            self.assertEqual(errors, [])

    def test_shuffled_suite_rejects_duplicate_outside_and_nonperiodic_openings(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = self.suite_path(directory)
            suite, _ = AUDIT_MATCH.read_canonical_openings(path)

            _, duplicate_errors = AUDIT_MATCH.audit_expected_opening_suite(
                [suite[0], suite[0]], path
            )
            _, outside_errors = AUDIT_MATCH.audit_expected_opening_suite(
                ["8/8/8/8/8/8/3K3k/8 w - - 0 1"], path
            )
            _, periodic_errors = AUDIT_MATCH.audit_expected_opening_suite(
                [suite[2], suite[0], suite[1], suite[0]], path
            )

            self.assertTrue(any("repeated" in error for error in duplicate_errors))
            self.assertTrue(any("outside" in error for error in outside_errors))
            self.assertTrue(any("does not repeat" in error for error in periodic_errors))

    def test_shuffled_suite_rejects_canonical_duplicate_source(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "duplicates.epd"
            path.write_text(
                "8/8/8/8/8/8/K6k/8 w - - 0 1\n"
                "8/8/8/8/8/8/K6k/8 w - - 99 77\n",
                encoding="utf-8",
            )

            _, errors = AUDIT_MATCH.audit_expected_opening_suite([], path)

            self.assertIn("expected opening suite contains canonical duplicates", errors)


class PairingTests(unittest.TestCase):
    @staticmethod
    def game(round_number: str, white: str, black: str) -> object:
        game = AUDIT_MATCH.chess.pgn.Game()
        game.headers["Round"] = round_number
        game.headers["White"] = white
        game.headers["Black"] = black
        game.headers["FEN"] = AUDIT_MATCH.chess.STARTING_FEN
        game.headers["Result"] = "1/2-1/2"
        return game

    def test_concurrent_completion_order_is_grouped_by_round(self) -> None:
        candidate = "NEYRANG-candidate"
        opponent = "NEYRANG-parent"
        games = [
            self.game("2", candidate, opponent),
            self.game("1", candidate, opponent),
            self.game("2", opponent, candidate),
            self.game("1", opponent, candidate),
        ]

        pairs, errors = AUDIT_MATCH.pair_games_by_round(games, 2)

        self.assertEqual(errors, [])
        self.assertEqual([pair[0].headers["Round"] for pair in pairs], ["1", "2"])
        self.assertTrue(
            all(
                left.headers["White"] == right.headers["Black"]
                for left, right in pairs
            )
        )

    def test_missing_or_overfull_round_is_rejected(self) -> None:
        games = [
            self.game("1", "A", "B"),
            self.game("1", "B", "A"),
            self.game("1", "A", "B"),
            self.game("2", "A", "B"),
        ]

        pairs, errors = AUDIT_MATCH.pair_games_by_round(games, 2)

        self.assertEqual(pairs, [])
        self.assertTrue(any("round '1' contains 3 games" in error for error in errors))
        self.assertTrue(any("round '2' contains 1 games" in error for error in errors))

if __name__ == "__main__":
    unittest.main()
