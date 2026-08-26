# Contributing to Aether OS

Aether OS is maintained as a long-term operating-system codebase. Every change must
preserve user control, system reliability, measurable performance, and traceability to
approved requirements.

## Contribution Requirements

1. Open an issue for behavioral changes, platform changes, security-sensitive changes,
   public contracts, build-system changes, or boot-path changes.
2. Keep patches scoped to one coherent product or engineering outcome.
3. Run the local validation pipeline before submitting:

   ```bash
   bash scripts/format.sh
   bash scripts/lint.sh
   bash scripts/test.sh
   ```

4. Include requirement references in commits and pull requests when changing behavior.
5. Add tests for every user-visible behavior, service contract, parser, policy, or boot
   artifact touched by the change.
6. Document public behavior in `docs/` when the change affects developers, operators,
   system integrators, or enterprise administrators.

## Engineering Standards

- Rust is preferred for memory-safe system services.
- C is allowed for low-level utilities and interoperability surfaces.
- Python is allowed for AI orchestration, developer tooling, and offline deterministic
  brain prototypes.
- Qt/QML is the premium shell UI stack.
- CMake owns native C and Qt builds.
- Cargo owns Rust workspaces.
- Shell scripts are used for reproducible developer automation and ISO assembly.

## Review Expectations

Pull requests are reviewed for:

- Correctness and test evidence
- Boot impact
- Security and privacy impact
- Performance and resource impact
- Maintainability over a 20-year product life
- Clear failure and recovery behavior

Changes that introduce silent failure, hidden privilege escalation, unbounded resource
consumption, undocumented public behavior, or non-reproducible build steps are not
accepted.

