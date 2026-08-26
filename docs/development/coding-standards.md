# Coding Standards

## General

- Public behavior must be traceable to approved requirements.
- Error messages must identify the failed operation, not expose secrets, and provide a
  recovery path when one exists.
- Long-running operations must expose progress, cancellation, or a bounded timeout.
- Scripts must fail closed with `set -euo pipefail`.
- Files must be formatted with repository tooling before review.

## Rust

- The workspace forbids unsafe Rust.
- Use explicit error propagation instead of process-wide panics.
- Keep services deterministic under test.
- Prefer small modules with clear ownership boundaries.

## C

- Use C11.
- Compile with warnings enabled.
- Validate all command-line input.
- Check all file and allocation results.
- Keep C components small and auditable.

## Python

- Use the standard library unless a dependency is explicitly approved.
- Keep AI orchestration interfaces deterministic under unit test.
- Avoid global mutable state outside clearly owned runtime objects.

## QML

- Keep shell UI state visible, minimal, and testable.
- Avoid embedding system policy in QML.
- Use QML for presentation and interaction composition.

