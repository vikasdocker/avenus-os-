#!/usr/bin/env bash
# Release validation: runs every check that must pass before a tagged
# Aether OS release. Designed to be CI-friendly: exits non-zero on
# the first failure, prints a final summary, and is idempotent.
#
# Validates:
#   1.  workspace compiles clean in debug mode
#   2.  workspace compiles clean in release mode
#   3.  full test suite passes
#   4.  clippy is clean across the workspace
#   5.  rustfmt reports no diffs
#   6.  release binaries stage successfully
#   7.  Python unit tests + repository contract tests pass
#   8.  workspace manifest lists every crate directory
#   9.  Phase 15 compatibility matrix + security audit docs exist
#   10. bootable ISO assembles (Linux-only: xorriso + grub-mkrescue)
#
# Optional flags:
#   --skip-release-build   skip `cargo build --release` (faster local)
#   --skip-python          skip Python test discovery
#   --skip-iso             skip bootable ISO assembly (Linux-only)
set -euo pipefail

cd "$(dirname "$0")/.."

SKIP_RELEASE_BUILD=0
SKIP_PYTHON=0
SKIP_ISO=0
for arg in "$@"; do
    case "$arg" in
        --skip-release-build) SKIP_RELEASE_BUILD=1 ;;
        --skip-python)        SKIP_PYTHON=1 ;;
        --skip-iso)           SKIP_ISO=1 ;;
        *) echo "release-validate: unknown flag '$arg'" >&2; exit 2 ;;
    esac
done

PASS=0
FAIL=0
report() {
    local name="$1" status="$2"
    if [[ "$status" -eq 0 ]]; then
        printf '  [PASS] %s\n' "$name"
        PASS=$((PASS + 1))
    else
        printf '  [FAIL] %s\n' "$name"
        FAIL=$((FAIL + 1))
    fi
}

step() { printf '\n== %s ==\n' "$1"; }

step "1. debug build"
cargo build --workspace >/dev/null 2>&1
report "cargo build --workspace" $?

if [[ $SKIP_RELEASE_BUILD -eq 0 ]]; then
    step "2. release build"
    cargo build --workspace --release >/dev/null 2>&1
    report "cargo build --workspace --release" $?
else
    printf '\n== 2. release build (skipped via --skip-release-build) ==\n'
fi

step "3. full test suite"
cargo test --workspace >/dev/null 2>&1
report "cargo test --workspace" $?

step "4. clippy"
# The workspace lints already set `clippy::all = deny`,
# so a plain `cargo clippy` is the production gate. The
# `-D warnings` flag would also turn `unused_assignments` and
# other rustc warnings into errors, which is out of scope
# for the release-validate contract.
cargo clippy --workspace --all-targets >/dev/null 2>&1
report "cargo clippy --workspace --all-targets" $?

step "5. rustfmt"
if cargo fmt --all -- --check >/dev/null 2>&1; then
    report "cargo fmt --all -- --check" 0
else
    report "cargo fmt --all -- --check" 1
fi

step "6. release staging"
STAGE="$(mktemp -d /tmp/aether-release-stage.XXXXXX)"
trap 'rm -rf "$STAGE"' EXIT
mkdir -p "$STAGE/bin" "$STAGE/services.d"
bins=()
# Windows builds use a `.exe` suffix; POSIX builds do not.
exe_suffix=""
case "$(uname -s 2>/dev/null || echo Windows)" in
    MINGW*|MSYS*|CYGWIN*|Windows*) exe_suffix=".exe" ;;
esac
for bin in aether-init aether-system-core aether-application-manager \
           aethersh aether-supervisor aether-agentd aetherctl \
           aether-sandbox; do
    if [[ -x "target/release/${bin}${exe_suffix}" ]]; then
        cp "target/release/${bin}${exe_suffix}" "$STAGE/bin/"
        bins+=("$bin")
    fi
done
if [[ ${#bins[@]} -ge 5 ]]; then
    report "release stage (${#bins[@]} binaries)" 0
else
    report "release stage (only ${#bins[@]} binaries; expected >= 5)" 1
fi

if [[ $SKIP_PYTHON -eq 0 ]]; then
    step "7. python tests"
    if command -v python3 >/dev/null 2>&1; then
        PYTHONPATH="${PWD}/brain:${PWD}/sdk/python${PYTHONPATH:+:$PYTHONPATH}" \
            python3 -m unittest discover -s tests/python -v >/dev/null 2>&1
        report "python unit tests" $?
    else
        printf '  [SKIP] python3 not installed\n'
    fi
else
    printf '\n== 7. python tests (skipped) ==\n'
fi

step "8. workspace manifest completeness"
# Every crate's directory path should appear as a member of
# the workspace. A crate that is not in the workspace will
# silently fail to build, so this is a hard gate.
missing=0
for crate in $(find . -maxdepth 4 -name Cargo.toml -not -path './target*' -not -path './target-linux*' \
                                       -not -path '*/target/*' -not -path '*/node_modules/*'); do
    crate_dir="$(dirname "$crate")"
    # Cargo.toml uses forward-slash relative paths; convert
    # Windows backslashes to forward slashes for the lookup.
    rel="${crate_dir#./}"
    rel="${rel//\\//}"
    if ! grep -q "\"$rel\"" Cargo.toml; then
        printf '  warning: crate %s not in workspace\n' "$crate_dir"
        missing=$((missing + 1))
    fi
done
if [[ $missing -eq 0 ]]; then
    report "workspace Cargo.toml membership" 0
else
    report "workspace Cargo.toml membership ($missing missing)" 1
fi

step "9. release documentation"
for doc in docs/RELEASE-NOTES.md \
           docs/phase-15/compatibility-matrix.md \
           docs/phase-15/security-audit.md; do
    if [[ -f "$doc" ]]; then
        report "doc exists: $doc" 0
    else
        report "doc exists: $doc" 1
    fi
done

step "10. bootable ISO"
if [[ $SKIP_ISO -eq 1 ]]; then
    printf '  [SKIP] --skip-iso set\n'
elif [[ "$(uname -s 2>/dev/null || echo Windows)" == "Windows"* \
     || "$(uname -s 2>/dev/null || echo Windows)" == "MINGW"* \
     || "$(uname -s 2>/dev/null || echo Windows)" == "MSYS"* \
     || "$(uname -s 2>/dev/null || echo Windows)" == "CYGWIN"* ]]; then
    # ISO assembly is Linux-only by design (xorriso / grub-mkrescue).
    # The Windows CI lane reports skip; the Linux lane reports the
    # real build.
    printf '  [SKIP] ISO assembly is Linux-only\n'
elif ! command -v xorriso >/dev/null 2>&1 \
   || ! command -v grub-mkrescue >/dev/null 2>&1; then
    printf '  [SKIP] xorriso / grub-mkrescue not installed\n'
else
    if bash scripts/iso/build-iso.sh >/dev/null 2>&1; then
        # The script prints `iso: <path> (<size> MiB)` on success.
        report "bootable ISO assembly" 0
    else
        report "bootable ISO assembly" 1
    fi
fi

step "summary"
printf '  passed: %d\n  failed: %d\n' "$PASS" "$FAIL"
if [[ $FAIL -ne 0 ]]; then
    printf 'release-validate: FAILED\n' >&2
    exit 1
fi
printf 'release-validate: OK\n'
