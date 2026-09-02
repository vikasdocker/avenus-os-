# Aether OS — Claude Code Instructions

## Core Rules

- This is a large multi-phase Aether OS project.
- Work strictly on the current requested phase/task.
- Do not implement future phases unless explicitly requested.
- Do not redesign existing architecture without a concrete reason.
- Do not modify unrelated files.
- Before editing, inspect the relevant existing code.
- Prefer small incremental changes.
- Reuse existing architecture and patterns.
- Do not create duplicate implementations.
- Do not remove working functionality unless explicitly required.

## Scope Control

- First identify the exact files required for the task.
- Avoid scanning the entire repository unless necessary.
- Keep changes minimal and focused.
- Never modify generated files unless required.
- Never modify Git configuration unless explicitly requested.

## Implementation

- Make one logical change at a time.
- After each meaningful change, run the smallest relevant validation.
- For Rust changes:
  - cargo fmt --check
  - cargo check
  - relevant cargo test
- Fix compilation/test errors before moving forward.

## Planning

Before implementation:

1. Understand the requested task.
2. Inspect relevant existing code.
3. Identify the smallest set of files to change.
4. Implement the change.
5. Validate it.
6. Summarize what changed.

Do not start unrelated work.

## Git

- Never reset, revert, or discard user changes.
- Never force push.
- Do not create commits unless explicitly requested.
- Do not modify unrelated user changes.

## Communication

- Do not repeatedly ask for confirmation for ordinary development actions.
- If blocked by a genuine ambiguity, explain the blocker briefly.
- Do not invent APIs, files, commands, or architecture.
- If something is uncertain, inspect the repository first.

## Aether OS Architecture

- Treat existing architecture as authoritative.
- Preserve existing module boundaries.
- Reuse existing services, traits, types, capabilities, and IPC patterns.
- Prefer extending existing abstractions over creating parallel systems.

## Completion

A task is complete only when:

1. The requested implementation exists.
2. Relevant tests/checks pass.
3. No unrelated files were modified.
4. The final response states exactly what changed and what was validated.