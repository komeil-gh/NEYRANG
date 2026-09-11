from __future__ import annotations

import importlib.util
import json
import sys
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "fit-shegerd-policy.py"
SPEC = importlib.util.spec_from_file_location("fit_shegerd_policy", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
policy = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = policy
SPEC.loader.exec_module(policy)

AUDIT_SCRIPT = Path(__file__).resolve().parents[1] / "audit-shegerd-policy-fit.py"
AUDIT_SPEC = importlib.util.spec_from_file_location("audit_shegerd_policy_fit", AUDIT_SCRIPT)
assert AUDIT_SPEC is not None and AUDIT_SPEC.loader is not None
auditor = importlib.util.module_from_spec(AUDIT_SPEC)
sys.modules[AUDIT_SPEC.name] = auditor
AUDIT_SPEC.loader.exec_module(auditor)


def row(record: str, group: str, move: str, selected: int, square: int) -> str:
    return "\t".join(
        (
            policy.TRACE_SCHEMA, record, group, move, str(selected), "quiet",
            str(square), str(square), "1", "0", "0", "0", "-1", "0",
            "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        )
    )


def trace(records: tuple[tuple[str, str, int], ...]) -> str:
    lines = ["\t".join(policy.HEADER)]
    for record_id, group_id, selected_square in records:
        lines.extend(
            (
                row(record_id, group_id, "b2b3", int(selected_square == 9), 9),
                row(record_id, group_id, "a2a3", int(selected_square == 8), 8),
                row(record_id, group_id, "c2c3", int(selected_square == 10), 10),
            )
        )
    return "\n".join(lines) + "\n"


class FitShegerdPolicyTests(unittest.TestCase):
    def test_repeat_fit_is_byte_identical_and_group_aware(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            train = root / "train.tsv"
            validation = root / "validation.tsv"
            train.write_text(trace((("t1", "g1", 8), ("t2", "g1", 8), ("t3", "g2", 8))), encoding="utf-8")
            validation.write_text(trace((("v1", "g3", 8), ("v2", "g4", 8))), encoding="utf-8")
            first = policy.run_fit(train, validation, root / "first.float.json", root / "first.bin", root / "first.json")
            second = policy.run_fit(train, validation, root / "second.float.json", root / "second.bin", root / "second.json")
            self.assertEqual((root / "first.float.json").read_bytes(), (root / "second.float.json").read_bytes())
            self.assertEqual((root / "first.bin").read_bytes(), (root / "second.bin").read_bytes())
            self.assertEqual((root / "first.json").read_bytes(), (root / "second.json").read_bytes())
            self.assertGreater(first["validation"]["pairwise_accuracy"], 0.5)
            self.assertGreater(first["validation"]["top1_accuracy"], first["validation"]["first_legal_top1_accuracy"])
            self.assertEqual(first, second)
            self.assertEqual(json.loads((root / "first.json").read_text())["schema"], policy.REPORT_SCHEMA)
            audit = auditor.audit(
                validation,
                root / "first.float.json",
                root / "first.bin",
                root / "first.json",
                root / "second.float.json",
                root / "second.bin",
                root / "second.json",
            )
            self.assertTrue(audit["ok"])

    def test_rejects_noncontiguous_rows_and_holdout_paths(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            malformed = root / "malformed.tsv"
            malformed.write_text(
                "\t".join(policy.HEADER) + "\n" + row("r1", "g1", "a2a3", 1, 8) + "\n" + row("r2", "g1", "a2a3", 1, 8) + "\n" + row("r1", "g1", "b2b3", 0, 9) + "\n",
                encoding="utf-8",
            )
            with self.assertRaisesRegex(policy.FitError, "contiguous"):
                policy.parse_trace(malformed)
            with self.assertRaisesRegex(policy.FitError, "holdout"):
                policy.reject_holdout_path(root / "sealed_holdout.tsv")

    def test_rejects_multiple_selected_candidates(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "bad.tsv"
            path.write_text("\t".join(policy.HEADER) + "\n" + row("r1", "g1", "a2a3", 1, 8) + "\n" + row("r1", "g1", "b2b3", 1, 9) + "\n", encoding="utf-8")
            with self.assertRaisesRegex(policy.FitError, "exactly one selected"):
                policy.parse_trace(path)


if __name__ == "__main__":
    unittest.main()
