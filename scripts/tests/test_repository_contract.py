import subprocess
import tomllib
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
            "actions/checkout@d23441a48e516b6c34aea4fa41551a30e30af803 # v6.1.0",
            "ubuntu-latest",
            "macos-latest",
            "cargo fmt --check",
            "cargo fmt --manifest-path tools/policy-trace/Cargo.toml --check",
            "cargo clippy --locked --all-targets --all-features -- -D warnings",
            "cargo test --all-features --locked",
            "cargo test --manifest-path tools/nnue-reference/Cargo.toml --locked",
            "cargo test --manifest-path tools/nnue-data/Cargo.toml --locked",
            "cargo test --manifest-path tools/nnue-trainer/Cargo.toml --locked --no-default-features",
            "cargo test --manifest-path tools/policy-trace/Cargo.toml --locked",
            ".venv/bin/pip install -r scripts/requirements-sanj.txt",
            ".venv/bin/python -m unittest discover -s scripts/tests",
            "scripts/test-openbench-contract.sh",
        ]:
            self.assertIn(required, workflow)

    def test_public_project_files_are_linked(self) -> None:
        readme = self.read("README.md")
        for path in [
            "docs/naming.md",
            "docs/rekhne.md",
            "docs/sanj.md",
            "docs/openbench.md",
            "CONTRIBUTING.md",
            "SECURITY.md",
            "LICENSE",
        ]:
            self.read(path)
            self.assertIn(path, readme)

    def test_nnue_tools_are_isolated(self) -> None:
        reference = self.read("tools/nnue-reference/Cargo.toml")
        data = self.read("tools/nnue-data/Cargo.toml")
        gitignore = self.read(".gitignore")
        self.assertIn('neyrang = { path = "../..", features = ["nnue"] }', reference)
        self.assertIn('unsafe_code = "forbid"', reference)
        self.assertIn('neyrang = { path = "../.." }', data)
        self.assertIn('unsafe_code = "forbid"', data)
        self.assertIn("/tools/nnue-data/target/", gitignore)
        self.assertIn("/.runtime/", gitignore)

    def test_generated_builds_are_not_tracked(self) -> None:
        tracked = subprocess.run(
            ["git", "ls-files", "builds"],
            cwd=ROOT,
            check=True,
            capture_output=True,
            text=True,
        ).stdout
        self.assertEqual("", tracked)

    def test_playing_binary_has_no_third_party_runtime(self) -> None:
        cargo = tomllib.loads(self.read("Cargo.toml"))
        self.assertEqual({}, cargo.get("dependencies", {}))
        self.assertNotIn("stockfish-nnue", cargo["features"])
        self.assertFalse((ROOT / "src/sanj/stockfish.rs").exists())
        self.assertFalse((ROOT / "assets/models/nn-37f18f62d772.nnue").exists())
        self.assertNotIn("nnue-rs", self.read("Cargo.lock"))

    def test_genfens_has_independent_provenance_gates(self) -> None:
        self.assertIn("SplitMix64", self.read("src/tools/genfens.rs"))
        self.assertIn(
            "one_batch_matches_independently_sharded_seed_offsets",
            self.read("tests/genfens.rs"),
        )
        self.assertIn(
            "neyrang-genfens-shard-v1",
            self.read("scripts/generate-opening-shard.py"),
        )

    def test_retired_identity_is_absent_from_tracked_tree(self) -> None:
        retired = ("a" + "kht").encode("ascii")
        failures: list[str] = []
        tracked = subprocess.run(
            ["git", "ls-files", "-z"],
            cwd=ROOT,
            check=True,
            capture_output=True,
        ).stdout.split(b"\0")
        for encoded in sorted(item for item in tracked if item):
            relative = Path(encoded.decode("utf-8"))
            path = ROOT / relative
            if retired in str(relative).lower().encode("utf-8"):
                failures.append(f"path:{relative}")
                continue
            if retired in path.read_bytes().lower():
                failures.append(f"content:{relative}")
        self.assertEqual([], failures, "retired identity remains: " + ", ".join(failures))


if __name__ == "__main__":
    unittest.main()
