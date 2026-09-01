import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]


class RepositoryContractTests(unittest.TestCase):
    def read(self, relative_path: str) -> str:
        path = ROOT / relative_path
        self.assertTrue(path.is_file(), f"missing repository contract file: {relative_path}")
        return path.read_text(encoding="utf-8")

    def test_ci_runs_cross_platform_engine_gates(self) -> None:
        workflow = self.read(".github/workflows/ci.yml")
        for required in [
            "contents: read",
            "actions/checkout@v6",
            "ubuntu-latest",
            "macos-latest",
            "cargo fmt --check",
            "cargo clippy --all-targets --all-features -- -D warnings",
            "cargo test --all-features",
            "cargo test --manifest-path tools/nnue-reference/Cargo.toml --locked",
            "cargo test --manifest-path tools/nnue-data/Cargo.toml --locked",
            ".venv/bin/pip install -r scripts/requirements.txt",
            ".venv/bin/python -m unittest discover -s scripts/tests",
            "scripts/test-openbench-contract.sh",
        ]:
            self.assertIn(required, workflow)

    def test_issue_forms_require_reproducible_evidence(self) -> None:
        regression = self.read(".github/ISSUE_TEMPLATE/engine-regression.yml")
        for required_id in ["reproduction", "commit", "binary_sha256", "artifacts"]:
            self.assertIn(f"id: {required_id}", regression)

        experiment = self.read(".github/ISSUE_TEMPLATE/experiment.yml")
        for required_id in ["hypothesis", "parent_identity", "decision_rule"]:
            self.assertIn(f"id: {required_id}", experiment)

    def test_pull_requests_carry_engine_evidence(self) -> None:
        template = self.read(".github/PULL_REQUEST_TEMPLATE.md")
        for heading in [
            "## Hypothesis and scope",
            "## Deterministic evidence",
            "## Game evidence",
            "## Reversion rule",
        ]:
            self.assertIn(heading, template)

    def test_public_project_guides_are_linked(self) -> None:
        readme = self.read("README.md")
        self.read("CONTRIBUTING.md")
        self.read("SECURITY.md")
        for link in [
            "docs/naming.md",
            "docs/rekhne.md",
            "docs/sanj.md",
            "docs/openbench.md",
            "docs/development/competitive-roadmap.md",
            "docs/development/nnue-reference.md",
            "docs/development/nnue-data.md",
            "CONTRIBUTING.md",
            "SECURITY.md",
        ]:
            self.assertIn(link, readme)

    def test_nnue_reference_is_isolated_and_documented(self) -> None:
        cargo = self.read("tools/nnue-reference/Cargo.toml")
        specification = self.read("docs/development/nnue-reference.md")
        self.assertIn('neyrang = { path = "../.." }', cargo)
        self.assertIn('unsafe_code = "forbid"', cargo)
        self.assertIn(r"NEYRANG\0", specification)
        self.assertIn("No playing-strength claim", specification)

    def test_nnue_data_codec_is_isolated_lossless_and_documented(self) -> None:
        cargo = self.read("tools/nnue-data/Cargo.toml")
        specification = self.read("docs/development/nnue-data.md")
        gitignore = self.read(".gitignore")
        self.assertIn('neyrang = { path = "../.." }', cargo)
        self.assertIn('unsafe_code = "forbid"', cargo)
        self.assertIn("Viriformat-compatible", specification)
        self.assertIn("white-relative", specification)
        self.assertIn("No playing-strength claim", specification)
        self.assertIn("/tools/nnue-data/target/", gitignore)

    def test_genfens_has_engine_and_independent_provenance_gates(self) -> None:
        engine = self.read("src/tools/genfens.rs")
        integration = self.read("tests/genfens.rs")
        generator = self.read("scripts/generate-opening-shard.py")
        specification = self.read("docs/development/nnue-data.md")
        self.assertIn("SplitMix64", engine)
        self.assertIn("one_batch_matches_independently_sharded_seed_offsets", integration)
        self.assertIn("neyrang-genfens-shard-v1", generator)
        self.assertIn("N1b", specification)


if __name__ == "__main__":
    unittest.main()
