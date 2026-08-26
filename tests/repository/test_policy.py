from pathlib import Path
import os
import stat
import sys

ROOT = Path(__file__).resolve().parents[2]
FORBIDDEN_MARKERS = ("TO" + "DO", "FIX" + "ME", "PLACE" + "HOLDER")
TEXT_SUFFIXES = {
    ".c",
    ".h",
    ".cpp",
    ".rs",
    ".py",
    ".sh",
    ".ps1",
    ".md",
    ".qml",
    ".toml",
    ".yml",
    ".yaml",
    ".json",
    ".service",
    ".aether-service",
    ".cfg",
    ".config",
}


def iter_text_files():
    ignored_parts = {".git", "artifacts", "build", "target", "__pycache__"}
    for directory, subdirectories, filenames in os.walk(ROOT):
        subdirectories[:] = [name for name in subdirectories if name not in ignored_parts]
        current = Path(directory)
        for filename in filenames:
            path = current / filename
            if path.suffix in TEXT_SUFFIXES or path.name in {"CMakeLists.txt", "Makefile", "LICENSE"}:
                yield path


def test_no_forbidden_markers() -> None:
    violations = []
    for path in iter_text_files():
        text = path.read_text(encoding="utf-8")
        for marker in FORBIDDEN_MARKERS:
            if marker in text:
                violations.append(f"{path.relative_to(ROOT)} contains {marker}")
    if violations:
        raise AssertionError("\n".join(violations))


def test_service_descriptors_are_complete() -> None:
    for path in (ROOT / "system" / "services.d").glob("*.service"):
        values = {}
        for line in path.read_text(encoding="utf-8").splitlines():
            if line.strip():
                key, value = line.split("=", 1)
                values[key] = value
        assert values["name"]
        assert values["command"].startswith("/opt/aether/bin/")
        assert values["critical"] in {"true", "false"}
        assert values["restart"] in {"true", "false"}


def test_shell_scripts_are_not_world_writable() -> None:
    root_text = ROOT.as_posix()
    if os.name == "nt" or root_text.startswith("/mnt/") or root_text.startswith("/cygdrive/"):
        return
    for path in (ROOT / "scripts").rglob("*.sh"):
        mode = path.stat().st_mode
        assert not (mode & stat.S_IWOTH), f"{path.relative_to(ROOT)} is world-writable"


if __name__ == "__main__":
    try:
        test_no_forbidden_markers()
        test_service_descriptors_are_complete()
        test_shell_scripts_are_not_world_writable()
    except AssertionError as error:
        print(error, file=sys.stderr)
        raise SystemExit(1)
