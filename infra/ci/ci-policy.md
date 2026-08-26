# CI Policy

The Aether OS CI policy requires every pull request to validate:

- Rust workspace formatting, linting, and tests when Cargo is available.
- Native CMake configuration, build, and tests when CMake is available.
- Python unit tests and repository policy tests.
- Shell syntax and ShellCheck when available.
- Initramfs assembly in the Linux CI environment.

Release workflows must produce immutable artifacts and record the source revision,
toolchain version, and validation result.

