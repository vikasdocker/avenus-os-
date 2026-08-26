# Aether Shell Development Guide

## Building the Shell

### Prerequisites

- Rust 1.70+ (check `rust-toolchain.toml`)
- Cargo
- System dependencies (standard dev headers)

### Build Commands

```bash
# Build all workspace members including shell
cargo build

# Build just the shell
cargo build -p aether-shell

# Build in release mode
cargo build -p aether-shell --release

# Build with verbose output
cargo build -p aether-shell -v

# Build with specific features
cargo build -p aether-shell --features "json-output"
```

### Build Output

```
shell/aether-shell/target/debug/aethersh    # Debug binary
shell/aether-shell/target/release/aethersh  # Release binary
```

## Running the Shell

### Interactive Mode

```bash
# Debug
./target/debug/aethersh

# Release
./target/release/aethersh

# With logging
RUST_LOG=aether_shell=debug ./target/debug/aethersh

# With JSON output
AETHER_JSON_OUTPUT=1 ./target/debug/aethersh
```

### Non-Interactive Mode (future)

```bash
# Run single command
aethersh help
aethersh version
aethersh status
```

## Testing

### Unit Tests

```bash
# Run all tests
cargo test -p aether-shell

# Run tests with output
cargo test -p aether-shell -- --nocapture

# Run specific test
cargo test -p aether-shell test_command_parsing

# Run with multiple threads
cargo test -p aether-shell -- --test-threads=1
```

### Test Coverage

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate coverage
cargo tarpaulin -p aether-shell --out Html

# View coverage report
open tarpaulin-report.html
```

### Integration Tests

```bash
# Integration tests (tests/integration_tests.rs)
cargo test -p aether-shell --test '*'
```

## Development Workflow

### Adding a New Command

1. **Create command module**

```rust
// src/command/mycommand.rs
use async_trait::async_trait;
use crate::command::{Command, CommandMetadata};

pub struct MyCommand;

#[async_trait]
impl Command for MyCommand {
    fn metadata(&self) -> &CommandMetadata {
        &CommandMetadata {
            name: "mycommand".to_string(),
            description: "Description of my command".to_string(),
            usage: "mycommand [args]".to_string(),
            required_capability: Some("system.read".to_string()),
            risk_level: "low".to_string(),
            requires_confirmation: false,
        }
    }

    async fn execute(
        &self,
        args: &[&str],
        session: &ShellSession,
        formatter: &mut OutputFormatter,
        history: &ShellHistory,
    ) -> Result<()> {
        // Command implementation
        let result = json!({
            "command": "mycommand",
            "result": "success"
        });
        formatter.output(&result)?;
        Ok(())
    }
}
```

2. **Register in CommandRegistry**

```rust
// In command/mod.rs
pub mod mycommand;

// In CommandRegistry::new()
commands.insert("mycommand".to_string(), Box::new(mycommand::MyCommand));
```

3. **Add tests**

```rust
#[tokio::test]
async fn test_mycommand_execution() {
    let registry = CommandRegistry::new();
    let session = ShellSession::new();
    let mut formatter = OutputFormatter::new();
    let history = ShellHistory::new();

    let result = registry.execute("mycommand", &[], &session, &mut formatter, &history).await;
    assert!(result.is_ok());
}
```

4. **Update documentation**

Add command to `docs/architecture/aether-shell.md`

### Modifying Command Behavior

1. Edit command implementation
2. Update tests
3. Run tests: `cargo test`
4. Update documentation
5. Commit with message like: `feat: add subcommand to mycommand`

### Adding IPC Support

Each command needs an IPC client to call its service. Future implementation will add:

```rust
// src/ipc/filesystem_client.rs
pub struct FilesystemClient {
    socket_path: String,
}

impl FilesystemClient {
    pub async fn list(&self, path: &str) -> Result<Vec<FilesystemEntry>> {
        let request = IpcRequest {
            service_id: "filesystem".to_string(),
            command: "list".to_string(),
            parameters: json!({"path": path}),
        };
        
        let response = self.send(&request).await?;
        // Parse response...
        Ok(...)
    }
}
```

## Code Style

### Formatting

```bash
# Format all code
cargo fmt

# Check formatting
cargo fmt -- --check
```

### Linting

```bash
# Run clippy
cargo clippy -p aether-shell

# Run with strict settings
cargo clippy -p aether-shell -- -D warnings
```

### Code Review Checklist

- [ ] All tests pass: `cargo test`
- [ ] No clippy warnings: `cargo clippy`
- [ ] Code formatted: `cargo fmt`
- [ ] Documentation updated
- [ ] No unwrap() or expect() unless justified
- [ ] Error messages are user-friendly
- [ ] No hardcoded paths or values
- [ ] Security checklist complete

## Debugging

### Environment Variables

```bash
# Enable debug logging
RUST_LOG=debug ./target/debug/aethersh

# Enable specific module logging
RUST_LOG=aether_shell::command=debug ./target/debug/aethersh

# Enable all aether logging
RUST_LOG=aether=trace ./target/debug/aethersh

# Enable JSON output for debugging
AETHER_JSON_OUTPUT=1 ./target/debug/aethersh
```

### Debugger (lldb or gdb)

```bash
# With LLDB (macOS)
lldb ./target/debug/aethersh
(lldb) run
(lldb) bt  # Backtrace
(lldb) p variable  # Print variable

# With gdb (Linux)
gdb ./target/debug/aethersh
(gdb) run
(gdb) bt
(gdb) print variable
```

### Print Debugging

```rust
// In code
dbg!(value);
eprintln!("Debug: {:?}", value);

// In logs
tracing::debug!("Message: {:?}", value);
```

## Performance Profiling

### Startup Time

```bash
# Measure startup time
time ./target/release/aethersh

# Profile with perf (Linux)
perf stat ./target/release/aethersh
```

### Flame Graphs

```bash
# Install flamegraph
cargo install flamegraph

# Generate flame graph
cargo flamegraph -p aether-shell
```

## CI/CD Integration

### GitHub Actions

The shell builds are integrated into:
- `.github/workflows/build.yml` - Build and test
- `.github/workflows/lint.yml` - Format and clippy checks

### Pre-commit Hooks

```bash
# Install pre-commit hooks
pre-commit install

# Run manually
pre-commit run --all-files
```

## Documentation

### Code Documentation

```rust
/// Executes a command with the given arguments.
///
/// # Arguments
/// * `command_name` - Name of the command to execute
/// * `args` - Command arguments
///
/// # Returns
/// * `Ok(())` if command executed successfully
/// * `Err(e)` if command failed
///
/// # Example
/// ```
/// let result = shell.execute("help", &[]);
/// ```
pub async fn execute(&self, command_name: &str, args: &[&str]) -> Result<()> {
    // ...
}
```

### Generate Documentation

```bash
# Generate and open documentation
cargo doc -p aether-shell --open
```

## Troubleshooting

### Build Failures

```bash
# Clean build
cargo clean -p aether-shell
cargo build -p aether-shell

# Check dependencies
cargo tree -p aether-shell

# Update dependencies
cargo update -p aether-shell
```

### Test Failures

```bash
# Run failing test with backtrace
RUST_BACKTRACE=1 cargo test test_name -- --nocapture

# Run single-threaded
cargo test -- --test-threads=1

# Keep test output
cargo test -- --nocapture --test-threads=1
```

### Runtime Issues

```bash
# Enable full logging
RUST_LOG=trace ./target/debug/aethersh

# Redirect to file for analysis
./target/debug/aethersh 2> shell.log

# Check with strace (Linux)
strace -e trace=open,read,write ./target/debug/aethersh
```

## Release Process

### Version Bumping

1. Update version in `shell/aether-shell/Cargo.toml`
2. Update version in root `Cargo.toml` if workspace version
3. Update CHANGELOG.md

### Building Release

```bash
# Build release
cargo build -p aether-shell --release

# Verify binary
./target/release/aethersh --version

# Run release tests
cargo test -p aether-shell --release
```

### Creating Release Binary

```bash
# Copy release binary
cp target/release/aethersh ./releases/aethersh-1.8.0

# Generate checksum
sha256sum ./releases/aethersh-1.8.0 > ./releases/aethersh-1.8.0.sha256
```

## Contributing

### Pull Request Process

1. Fork repository
2. Create feature branch: `git checkout -b feat/my-feature`
3. Make changes
4. Add tests
5. Run full test suite
6. Format code: `cargo fmt`
7. Check lints: `cargo clippy`
8. Commit: `git commit -am "feat: description"`
9. Push: `git push origin feat/my-feature`
10. Create pull request

### Commit Message Format

```
<type>: <description>

<body>

<footer>
```

Types: feat, fix, docs, style, refactor, perf, test, chore

Example:
```
feat: add network connectivity command

Implement network.connectivity subcommand to check external connectivity.
Supports IPv4 and IPv6 connectivity tests.

Fixes #123
```

## Resources

- [Rust Book](https://doc.rust-lang.org/book/)
- [Async Rust](https://rust-lang.github.io/async-book/)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [Aether OS Docs](../../docs/)
- [Project Genesis SRS](../../docs/project-genesis-srs-part-1.md)
