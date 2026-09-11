from __future__ import annotations

import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "audit-shegerd-policy-embed.py"
MODEL = ROOT / "docs" / "evidence" / "shegerd-p1-policy.bin"
FIXTURE = ROOT / "docs" / "evidence" / "shegerd-p2-score-parity.tsv"


class PolicyEmbedAuditTests(unittest.TestCase):
    def test_public_fixture_passes_and_tampering_fails(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "audit.json"
            result = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--model",
                    str(MODEL),
                    "--fixture",
                    str(FIXTURE),
                    "--output",
                    str(output),
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertTrue(json.loads(output.read_text(encoding="utf-8"))["ok"])

            tampered = Path(directory) / "tampered.tsv"
            text = FIXTURE.read_text(encoding="utf-8")
            last = text.rstrip("\n").splitlines()
            fields = last[-1].split("\t")
            fields[-1] = str(int(fields[-1]) + 1)
            last[-1] = "\t".join(fields)
            tampered.write_text("\n".join(last) + "\n", encoding="utf-8")
            rejected = subprocess.run(
                [
                    sys.executable,
                    str(SCRIPT),
                    "--model",
                    str(MODEL),
                    "--fixture",
                    str(tampered),
                    "--output",
                    str(Path(directory) / "rejected.json"),
                ],
                capture_output=True,
                text=True,
                check=False,
            )
            self.assertNotEqual(rejected.returncode, 0)


if __name__ == "__main__":
    unittest.main()
