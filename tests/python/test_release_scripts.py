"""Repository contract tests for the release / ISO scripts.

These tests verify that the release pipeline shell scripts exist,
are executable on POSIX systems, carry the expected shebang, and
expose the documented CLI flags. They do not actually build the
ISO (that requires xorriso / grub-mkrescue, which the test
environment may not have).
"""

from __future__ import annotations

import os
import re
import stat
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]


def _assert_bash_shebang(test: unittest.TestCase, text: str) -> None:
    first_line = text.splitlines()[0]
    test.assertTrue(
        first_line.startswith("#!/usr/bin/env bash"),
        f"unexpected shebang: {first_line!r}",
    )


def _assert_executable(test: unittest.TestCase, path: Path) -> None:
    if os.name != "posix":
        return
    mode = path.stat().st_mode
    test.assertTrue(
        mode & stat.S_IXUSR,
        f"{path.name} must be executable for the user",
    )


class ReleaseValidateScriptTests(unittest.TestCase):
    """`scripts/release-validate.sh` is the CI gate."""

    def setUp(self) -> None:
        self.path = REPO_ROOT / "scripts" / "release-validate.sh"
        self.text = self.path.read_text(encoding="utf-8")

    def test_exists(self) -> None:
        self.assertTrue(self.path.is_file(), f"missing: {self.path}")

    def test_has_bash_shebang(self) -> None:
        _assert_bash_shebang(self, self.text)

    def test_executable(self) -> None:
        _assert_executable(self, self.path)

    def test_documents_all_flags(self) -> None:
        for flag in ("--skip-release-build", "--skip-python", "--skip-iso"):
            self.assertIn(flag, self.text, f"flag {flag} not documented in script")

    def test_covers_ten_steps(self) -> None:
        # Steps are registered as `step "N. <title>"`; verify there
        # are at least 10 distinct step counters.
        counters = re.findall(r'step "(\d+)\.', self.text)
        self.assertGreaterEqual(
            len(set(counters)),
            10,
            f"expected >= 10 release-validation steps, got {sorted(set(counters))}",
        )

    def test_reports_workspace_membership(self) -> None:
        self.assertIn("workspace Cargo.toml membership", self.text)

    def test_reports_clippy(self) -> None:
        self.assertIn("cargo clippy --workspace --all-targets", self.text)

    def test_reports_rustfmt(self) -> None:
        self.assertIn("cargo fmt --all -- --check", self.text)


class BuildIsoScriptTests(unittest.TestCase):
    """`scripts/iso/build-iso.sh` produces the bootable ISO."""

    def setUp(self) -> None:
        self.path = REPO_ROOT / "scripts" / "iso" / "build-iso.sh"
        self.text = self.path.read_text(encoding="utf-8")

    def test_exists(self) -> None:
        self.assertTrue(self.path.is_file(), f"missing: {self.path}")

    def test_has_bash_shebang(self) -> None:
        _assert_bash_shebang(self, self.text)

    def test_executable(self) -> None:
        _assert_executable(self, self.path)

    def test_uses_grub_mkrescue(self) -> None:
        self.assertIn("grub-mkrescue", self.text)

    def test_writes_grub_config(self) -> None:
        self.assertIn("grub.cfg", self.text)

    def test_boot_entries_present(self) -> None:
        # The generated GRUB config must offer at least the default
        # boot and a recovery shell entry.
        for needle in (
            "linux /boot/vmlinuz",
            "initrd /boot/aether/initramfs.cpio.gz",
            "recovery shell",
        ):
            self.assertIn(needle, self.text, f"missing: {needle!r}")


class QemuIsoScriptTests(unittest.TestCase):
    """`scripts/run/qemu-iso.sh` boots the ISO under QEMU."""

    def setUp(self) -> None:
        self.path = REPO_ROOT / "scripts" / "run" / "qemu-iso.sh"
        self.text = self.path.read_text(encoding="utf-8")

    def test_exists(self) -> None:
        self.assertTrue(self.path.is_file(), f"missing: {self.path}")

    def test_has_bash_shebang(self) -> None:
        _assert_bash_shebang(self, self.text)

    def test_executable(self) -> None:
        _assert_executable(self, self.path)

    def test_supports_smoke_mode(self) -> None:
        self.assertIn("--smoke", self.text)
        self.assertIn("ISO SMOKE TEST", self.text)

    def test_mounts_cdrom(self) -> None:
        self.assertIn("-cdrom", self.text)


class ReleaseScriptStageTests(unittest.TestCase):
    """`scripts/release.sh` stages every Aether binary, including
    the new `aether-sandbox` kernel-enforcement binary."""

    def setUp(self) -> None:
        self.path = REPO_ROOT / "scripts" / "release.sh"
        self.text = self.path.read_text(encoding="utf-8")

    def test_exists(self) -> None:
        self.assertTrue(self.path.is_file(), f"missing: {self.path}")

    def test_has_bash_shebang(self) -> None:
        _assert_bash_shebang(self, self.text)

    def test_stages_aether_sandbox(self) -> None:
        self.assertIn(
            "aether-sandbox",
            self.text,
            "aether-sandbox must be staged by scripts/release.sh",
        )


class InitramfsBuildTests(unittest.TestCase):
    """`scripts/iso/build-initramfs.sh` copies every Aether binary
    into the initramfs tree, including the new `aether-sandbox`."""

    def setUp(self) -> None:
        self.path = REPO_ROOT / "scripts" / "iso" / "build-initramfs.sh"
        self.text = self.path.read_text(encoding="utf-8")

    def test_exists(self) -> None:
        self.assertTrue(self.path.is_file(), f"missing: {self.path}")

    def test_has_bash_shebang(self) -> None:
        _assert_bash_shebang(self, self.text)

    def test_copies_aether_sandbox(self) -> None:
        self.assertIn(
            "aether-sandbox",
            self.text,
            "aether-sandbox must be copied into the initramfs",
        )


if __name__ == "__main__":
    unittest.main()
