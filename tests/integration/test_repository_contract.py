from pathlib import Path
import unittest


ROOT = Path(__file__).resolve().parents[2]


class RepositoryContractTests(unittest.TestCase):
    def test_required_top_level_directories_exist(self) -> None:
        expected = {
            ".github",
            "apps",
            "artifacts",
            "assets",
            "brain",
            "core",
            "desktop",
            "docs",
            "docker",
            "infra",
            "kernel",
            "network",
            "scripts",
            "sdk",
            "security",
            "services",
            "shell",
            "storage",
            "system",
            "tests",
            "tools",
            "ui",
            "vision",
            "voice",
        }
        missing = sorted(name for name in expected if not (ROOT / name).is_dir())
        self.assertEqual(missing, [])

    def test_cargo_workspace_contains_domain_crates(self) -> None:
        cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
        for member in [
            "apps/aether-apps",
            "core/aether-core",
            "desktop/aether-desktop",
            "network/aether-network",
            "security/aether-security",
            "storage/aether-storage",
            "vision/aether-vision",
            "voice/aether-voice",
        ]:
            self.assertIn(member, cargo)

    def test_root_governance_files_exist(self) -> None:
        for filename in [
            "README.md",
            "LICENSE",
            "CONTRIBUTING.md",
            "ROADMAP.md",
            "CODE_OF_CONDUCT.md",
            "SECURITY.md",
            "CHANGELOG.md",
            ".editorconfig",
            ".gitignore",
            ".gitattributes",
        ]:
            self.assertTrue((ROOT / filename).is_file(), filename)


if __name__ == "__main__":
    unittest.main()
