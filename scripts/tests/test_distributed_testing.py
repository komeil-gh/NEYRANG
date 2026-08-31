from __future__ import annotations

import importlib.util
import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

import chess


SCRIPT = Path(__file__).resolve().parents[1] / "distributed-testing.py"
SPEC = importlib.util.spec_from_file_location("distributed_testing", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
distributed = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = distributed
SPEC.loader.exec_module(distributed)


class DistributedCampaignTests(unittest.TestCase):
    def test_cli_prepares_and_dry_runs_a_verified_worker(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            assets = make_assets(root, opening_count=4)
            campaign = root / "campaign"
            prepared = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "prepare",
                    "--engine-a",
                    str(assets["engine_a"]),
                    "--engine-a-name",
                    "NEYRANG-candidate",
                    "--engine-a-git-sha",
                    "candidate-commit",
                    "--engine-b",
                    str(assets["engine_b"]),
                    "--engine-b-name",
                    "NEYRANG-parent",
                    "--engine-b-git-sha",
                    "parent-commit",
                    "--fastchess",
                    str(assets["fastchess"]),
                    "--fastchess-version",
                    "fastchess test",
                    "--openings",
                    str(assets["openings"]),
                    "--openings-source",
                    "test-suite",
                    "--openings-license",
                    "CC0-1.0",
                    "--output",
                    str(campaign),
                    "--seed",
                    "campaign-seed",
                    "--pairs",
                    "4",
                    "--pairs-per-shard",
                    "2",
                    "--tc",
                    "0.5+0.005",
                    "--strict",
                ],
                check=True,
                capture_output=True,
                text=True,
            )
            summary = json.loads(prepared.stdout)

            dry_run = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "run-shard",
                    "--manifest",
                    str(campaign / "shards" / "shard-0000.json"),
                    "--campaign-sha256",
                    summary["campaign_sha256"],
                    "--engine-a",
                    str(assets["engine_a"]),
                    "--engine-b",
                    str(assets["engine_b"]),
                    "--fastchess",
                    str(assets["fastchess"]),
                    "--results",
                    str(root / "results"),
                    "--match-script",
                    str(SCRIPT.parent / "match.sh"),
                    "--audit-script",
                    str(SCRIPT.parent / "audit-match.py"),
                    "--dry-run",
                ],
                check=True,
                capture_output=True,
                text=True,
            )
            worker = json.loads(dry_run.stdout)

            self.assertEqual(worker["status"], "verified-dry-run")
            self.assertEqual(worker["environment"]["SHARD_ID"], "shard-0000")
            self.assertIn("--expected-openings", worker["audit_command"])

    def test_cli_worker_emits_a_complete_result_manifest(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            assets = make_assets(root, opening_count=2)
            campaign = root / "campaign"
            summary = distributed.prepare_campaign(
                make_config(assets, campaign, total_pairs=2, pairs_per_shard=2)
            )
            match_script = root / "fake-match.py"
            match_script.write_text(
                "#!/usr/bin/env python3\n"
                "import os\n"
                "from pathlib import Path\n"
                "for key, content in ((\"PGN_OUT\", \"pgn\\n\"), "
                "(\"LOG_OUT\", \"log\\n\"), (\"META_OUT\", \"status=completed\\n\"), "
                "(\"CONFIG_OUT\", \"{}\\n\")):\n"
                "    Path(os.environ[key]).write_text(content, encoding=\"utf-8\")\n",
                encoding="utf-8",
            )
            match_script.chmod(0o755)
            audit_script = root / "fake-audit.py"
            audit_script.write_text(
                "#!/usr/bin/env python3\n"
                "import argparse, json\n"
                "parser = argparse.ArgumentParser()\n"
                "parser.add_argument('--expected-games', type=int, required=True)\n"
                "args, _ = parser.parse_known_args()\n"
                "pairs = args.expected_games // 2\n"
                "print(json.dumps({'format': 'neyrang-match-audit-v1', 'ok': True, "
                "'games': args.expected_games, 'pairs': pairs, "
                "'wdl': {'wins': 1, 'draws': args.expected_games - 2, 'losses': 1}, "
                "'score': args.expected_games / 2, "
                "'pentanomial': [0, 0, pairs, 0, 0]}))\n",
                encoding="utf-8",
            )
            results = root / "results"

            completed = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "run-shard",
                    "--manifest",
                    str(campaign / "shards" / "shard-0000.json"),
                    "--campaign-sha256",
                    summary["campaign_sha256"],
                    "--engine-a",
                    str(assets["engine_a"]),
                    "--engine-b",
                    str(assets["engine_b"]),
                    "--fastchess",
                    str(assets["fastchess"]),
                    "--results",
                    str(results),
                    "--match-script",
                    str(match_script),
                    "--audit-script",
                    str(audit_script),
                ],
                check=True,
                capture_output=True,
                text=True,
            )
            result = json.loads(completed.stdout)

            self.assertEqual(result["format"], "neyrang-distributed-result-v1")
            self.assertEqual(result["games"], 4)
            self.assertTrue((results / "shard-0000.result.json").is_file())

    def test_preparation_is_deterministic_and_partitions_unique_pairs(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            assets = make_assets(root, opening_count=8)
            first = root / "first"
            second = root / "second"

            first_summary = distributed.prepare_campaign(
                make_config(assets, first, total_pairs=5, pairs_per_shard=2)
            )
            second_summary = distributed.prepare_campaign(
                make_config(assets, second, total_pairs=5, pairs_per_shard=2)
            )

            self.assertEqual(first_summary["campaign_id"], second_summary["campaign_id"])
            self.assertEqual(
                (first / "campaign.json").read_bytes(),
                (second / "campaign.json").read_bytes(),
            )
            self.assertEqual(first_summary["pairs"], 5)
            self.assertEqual(first_summary["shards"], 3)

            selected: list[str] = []
            pair_counts: list[int] = []
            for index in range(3):
                left_manifest = first / "shards" / f"shard-{index:04}.json"
                right_manifest = second / "shards" / f"shard-{index:04}.json"
                self.assertEqual(left_manifest.read_bytes(), right_manifest.read_bytes())
                manifest = json.loads(left_manifest.read_text(encoding="utf-8"))
                pair_counts.append(manifest["pairs"])
                openings = first / manifest["openings_file"]
                selected.extend(openings.read_text(encoding="utf-8").splitlines())
                self.assertNotIn(str(first), left_manifest.read_text(encoding="utf-8"))

            self.assertEqual(pair_counts, [2, 2, 1])
            self.assertEqual(len(selected), 5)
            self.assertEqual(len({canonical_fen(fen) for fen in selected}), 5)

    def test_preparation_rejects_duplicate_positions_and_existing_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            assets = make_assets(root, opening_count=3)
            openings = assets["openings"]
            lines = openings.read_text(encoding="utf-8").splitlines()
            duplicate = " ".join(lines[0].split()[:4]) + " 99 42"
            openings.write_text("\n".join([*lines, duplicate]) + "\n", encoding="utf-8")

            with self.assertRaisesRegex(distributed.CampaignError, "duplicate canonical"):
                distributed.prepare_campaign(
                    make_config(assets, root / "campaign", total_pairs=4, pairs_per_shard=2)
                )

            clean_assets = make_assets(root / "clean", opening_count=4)
            output = root / "occupied"
            output.mkdir()
            with self.assertRaisesRegex(distributed.CampaignError, "refusing to overwrite"):
                distributed.prepare_campaign(
                    make_config(clean_assets, output, total_pairs=4, pairs_per_shard=2)
                )

    def test_time_control_must_round_trip_through_millisecond_pgn_precision(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            assets = make_assets(root, opening_count=2)
            config = make_config(
                assets, root / "campaign", total_pairs=2, pairs_per_shard=2
            )
            invalid = distributed.CampaignConfig(
                **{**config.__dict__, "time_control": "0.05+0.0005"}
            )

            with self.assertRaisesRegex(distributed.CampaignError, "millisecond"):
                distributed.prepare_campaign(invalid)

            self.assertEqual(distributed.canonical_time_control("40/10.000+0.100"), "40/10+0.1")
            self.assertEqual(distributed.canonical_time_control("10+0.1"), "10+0.1")

    def test_worker_verification_rejects_any_asset_drift(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            assets = make_assets(root, opening_count=4)
            campaign = root / "campaign"
            summary = distributed.prepare_campaign(
                make_config(assets, campaign, total_pairs=4, pairs_per_shard=2)
            )
            manifest = campaign / "shards" / "shard-0000.json"

            verified = distributed.verify_shard_assets(
                manifest,
                engine_a=assets["engine_a"],
                engine_b=assets["engine_b"],
                fastchess=assets["fastchess"],
                campaign_sha256=summary["campaign_sha256"],
            )
            self.assertEqual(verified["shard_id"], "shard-0000")

            assets["engine_a"].write_bytes(b"tampered candidate")
            with self.assertRaisesRegex(distributed.CampaignError, "engine_a SHA-256"):
                distributed.verify_shard_assets(
                    manifest,
                    engine_a=assets["engine_a"],
                    engine_b=assets["engine_b"],
                    fastchess=assets["fastchess"],
                    campaign_sha256=summary["campaign_sha256"],
                )

    def test_worker_environment_is_bound_to_campaign_and_shard_identity(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            assets = make_assets(root, opening_count=4)
            campaign = root / "campaign"
            summary = distributed.prepare_campaign(
                make_config(assets, campaign, total_pairs=4, pairs_per_shard=2)
            )
            manifest = campaign / "shards" / "shard-0001.json"
            results = root / "worker-results"

            environment = distributed.build_shard_environment(
                manifest,
                engine_a=assets["engine_a"],
                engine_b=assets["engine_b"],
                fastchess=assets["fastchess"],
                results=results,
                campaign_sha256=summary["campaign_sha256"],
            )

            self.assertEqual(environment["CAMPAIGN_ID"], summary["campaign_id"])
            self.assertEqual(environment["SHARD_ID"], "shard-0001")
            self.assertEqual(environment["PAIR_OFFSET"], "2")
            self.assertEqual(environment["GAMES"], "4")
            self.assertEqual(environment["OPENING_ORDER"], "sequential")
            self.assertEqual(environment["STRICT"], "1")
            self.assertEqual(environment["THREADS"], "1")
            self.assertEqual(
                Path(environment["PGN_OUT"]), results / "shard-0001.pgn"
            )

            campaign_file = campaign / "campaign.json"
            campaign_file.write_text(
                campaign_file.read_text(encoding="utf-8") + " ", encoding="utf-8"
            )
            with self.assertRaisesRegex(distributed.CampaignError, "campaign SHA-256"):
                distributed.build_shard_environment(
                    manifest,
                    engine_a=assets["engine_a"],
                    engine_b=assets["engine_b"],
                    fastchess=assets["fastchess"],
                    results=root / "other-results",
                    campaign_sha256=summary["campaign_sha256"],
                )

    def test_result_manifest_hashes_every_artifact_and_detects_tampering(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            assets = make_assets(root, opening_count=2)
            campaign = root / "campaign"
            summary = distributed.prepare_campaign(
                make_config(assets, campaign, total_pairs=2, pairs_per_shard=2)
            )
            manifest = campaign / "shards" / "shard-0000.json"
            results = root / "results"
            results.mkdir()
            write_fake_result_artifacts(results, "shard-0000", games=4, pairs=2)

            result = distributed.finalize_shard_result(
                manifest,
                results=results,
                campaign_sha256=summary["campaign_sha256"],
            )

            self.assertEqual(result["format"], "neyrang-distributed-result-v1")
            self.assertEqual(result["games"], 4)
            self.assertEqual(set(result["artifacts"]), {"pgn", "log", "metadata", "config", "audit"})
            result_path = results / "shard-0000.result.json"
            self.assertTrue(result_path.is_file())

            (results / "shard-0000.pgn").write_text("tampered", encoding="utf-8")
            with self.assertRaisesRegex(distributed.CampaignError, "artifact pgn SHA-256"):
                distributed.verify_result_manifest(
                    manifest,
                    result_path=result_path,
                    results=results,
                    campaign_sha256=summary["campaign_sha256"],
                )

    def test_campaign_audit_requires_every_shard_and_aggregates_exact_counts(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            assets = make_assets(root, opening_count=4)
            campaign = root / "campaign"
            summary = distributed.prepare_campaign(
                make_config(assets, campaign, total_pairs=4, pairs_per_shard=2)
            )
            results = root / "results"
            results.mkdir()
            for index in range(2):
                shard_id = f"shard-{index:04}"
                write_fake_result_artifacts(results, shard_id, games=4, pairs=2)
                distributed.finalize_shard_result(
                    campaign / "shards" / f"{shard_id}.json",
                    results=results,
                    campaign_sha256=summary["campaign_sha256"],
                )

            audit = distributed.audit_campaign_results(
                campaign / "campaign.json",
                results=results,
                campaign_sha256=summary["campaign_sha256"],
            )

            self.assertTrue(audit["ok"])
            self.assertEqual(audit["shards"], 2)
            self.assertEqual(audit["pairs"], 4)
            self.assertEqual(audit["games"], 8)
            self.assertEqual(audit["wdl"], {"wins": 2, "draws": 4, "losses": 2})
            self.assertEqual(audit["pentanomial"], [0, 0, 4, 0, 0])

            (results / "shard-0001.result.json").unlink()
            with self.assertRaisesRegex(distributed.CampaignError, "missing result manifest"):
                distributed.audit_campaign_results(
                    campaign / "campaign.json",
                    results=results,
                    campaign_sha256=summary["campaign_sha256"],
                )

    def test_coordinator_replays_each_raw_shard_audit_independently(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            assets = make_assets(root, opening_count=2)
            campaign = root / "campaign"
            summary = distributed.prepare_campaign(
                make_config(assets, campaign, total_pairs=2, pairs_per_shard=2)
            )
            results = root / "results"
            results.mkdir()
            write_fake_result_artifacts(results, "shard-0000", games=4, pairs=2)
            distributed.finalize_shard_result(
                campaign / "shards" / "shard-0000.json",
                results=results,
                campaign_sha256=summary["campaign_sha256"],
            )
            matching = root / "matching-audit.py"
            write_fake_auditor(matching, mismatch=False)

            replays = distributed.replay_campaign_audits(
                campaign / "campaign.json",
                results=results,
                campaign_sha256=summary["campaign_sha256"],
                audit_script=matching,
            )

            self.assertEqual(len(replays), 1)
            self.assertEqual(replays[0]["shard_id"], "shard-0000")
            self.assertEqual(replays[0]["games"], 4)

            mismatching = root / "mismatching-audit.py"
            write_fake_auditor(mismatching, mismatch=True)
            with self.assertRaisesRegex(distributed.CampaignError, "independent replay differs"):
                distributed.replay_campaign_audits(
                    campaign / "campaign.json",
                    results=results,
                    campaign_sha256=summary["campaign_sha256"],
                    audit_script=mismatching,
                )

            output = root / "campaign-audit.json"
            completed = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "audit-campaign",
                    "--campaign",
                    str(campaign / "campaign.json"),
                    "--campaign-sha256",
                    summary["campaign_sha256"],
                    "--results",
                    str(results),
                    "--audit-script",
                    str(matching),
                    "--output",
                    str(output),
                ],
                check=True,
                capture_output=True,
                text=True,
            )
            cli_audit = json.loads(completed.stdout)
            self.assertTrue(cli_audit["independent_replay"])
            self.assertEqual(cli_audit["replayed_shards"], 1)
            self.assertEqual(json.loads(output.read_text(encoding="utf-8")), cli_audit)


def make_config(
    assets: dict[str, Path],
    output: Path,
    *,
    total_pairs: int,
    pairs_per_shard: int,
) -> distributed.CampaignConfig:
    return distributed.CampaignConfig(
        engine_a=assets["engine_a"],
        engine_a_name="NEYRANG-candidate",
        engine_a_git_sha="candidate-commit",
        engine_b=assets["engine_b"],
        engine_b_name="NEYRANG-parent",
        engine_b_git_sha="parent-commit",
        fastchess=assets["fastchess"],
        fastchess_version="fastchess test",
        openings=assets["openings"],
        openings_source="test-suite",
        openings_license="CC0-1.0",
        output=output,
        seed="campaign-seed",
        total_pairs=total_pairs,
        pairs_per_shard=pairs_per_shard,
        time_control="0.5+0.005",
        hash_mb=64,
        threads=1,
        move_overhead_ms=100,
        time_margin_ms=0,
        strict=True,
        warning_policy="reject-all",
    )


def make_assets(root: Path, *, opening_count: int) -> dict[str, Path]:
    root.mkdir(parents=True, exist_ok=True)
    engine_a = root / "candidate"
    engine_b = root / "parent"
    fastchess = root / "fastchess"
    openings = root / "openings.epd"
    engine_a.write_bytes(b"candidate binary")
    engine_b.write_bytes(b"parent binary")
    fastchess.write_bytes(b"fastchess binary")
    openings.write_text(
        "\n".join(make_openings(opening_count)) + "\n", encoding="utf-8"
    )
    return {
        "engine_a": engine_a,
        "engine_b": engine_b,
        "fastchess": fastchess,
        "openings": openings,
    }


def write_fake_result_artifacts(
    results: Path, shard_id: str, *, games: int, pairs: int
) -> None:
    for suffix, content in (
        ("pgn", "pgn payload\n"),
        ("log", "log payload\n"),
        ("meta.txt", "status=completed\n"),
        ("config.json", "{}\n"),
    ):
        (results / f"{shard_id}.{suffix}").write_text(content, encoding="utf-8")
    (results / f"{shard_id}.audit.json").write_text(
        json.dumps(
            {
                "format": "neyrang-match-audit-v1",
                "ok": True,
                "games": games,
                "pairs": pairs,
                "wdl": {"wins": 1, "draws": games - 2, "losses": 1},
                "score": games / 2,
                "pentanomial": [0, 0, pairs, 0, 0],
            },
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )


def write_fake_auditor(path: Path, *, mismatch: bool) -> None:
    path.write_text(
        "#!/usr/bin/env python3\n"
        "import argparse, json\n"
        "parser = argparse.ArgumentParser()\n"
        "parser.add_argument('--expected-games', type=int, required=True)\n"
        "args, _ = parser.parse_known_args()\n"
        "games = args.expected_games\n"
        "pairs = games // 2\n"
        + (
            "wdl = {'wins': games, 'draws': 0, 'losses': 0}\n"
            "score = games\n"
            "penta = [pairs, 0, 0, 0, 0]\n"
            if mismatch
            else "wdl = {'wins': 1, 'draws': games - 2, 'losses': 1}\n"
            "score = games / 2\n"
            "penta = [0, 0, pairs, 0, 0]\n"
        )
        + "print(json.dumps({'format': 'neyrang-match-audit-v1', 'ok': True, "
        "'games': games, 'pairs': pairs, 'wdl': wdl, 'score': score, "
        "'pentanomial': penta}))\n",
        encoding="utf-8",
    )


def make_openings(count: int) -> list[str]:
    board = chess.Board()
    result: list[str] = []
    for index in range(count):
        result.append(board.fen(en_passant="fen"))
        moves = sorted(board.legal_moves, key=lambda move: move.uci())
        board.push(moves[index % len(moves)])
    return result


def canonical_fen(fen: str) -> str:
    return " ".join(chess.Board(fen).fen(en_passant="fen").split()[:4])


if __name__ == "__main__":
    unittest.main()
