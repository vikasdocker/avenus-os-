# Phase 1.8: Development Guide

## Environment Setup

### Prerequisites
- Rust 1.70+ (with cargo)
- Linux kernel 5.10+ (for Unix domain sockets)
- Aether OS Phase 1.7 running
- Development tools: git, gcc/clang, make

### Building Aether Shell

```bash
# Clone and enter repo
cd /path/to/aether-os

# Build just the shell
cargo build -p aether-shell

# Build with release optimizations
cargo build -p aether-shell --release

# Run tests
cargo test -p aether-shell

# Run linter
cargo clippy -p aether-shell

# Format code
cargo fmt -p aether-shell
```

### Running Aether Shell

```bash
# Assuming Phase 1.7 system is running
./target/debug/aethersh

# Or from release build
./target/release/aethersh

# With environment variables
AETHER_VERBOSE=1 ./target/debug/aethersh
AETHER_LOG_LEVEL=debug ./target/debug/aethersh
```

---

## Phase-by-Phase Implementation Guide

### Phase 1.8.1: Foundation (Weeks 1-2)

**Goals:**
- REPL loop working
- Basic command parsing
- Command registry structure
- Output formatters
- History/session management

**Key Files to Create:**
```
src/
  main.rs                    # Entry point
  lib.rs                     # Library root
  shell/
    mod.rs
    repl.rs                  # REPL loop
    parser.rs                # Command parser
    executor.rs              # Command execution
  commands/
    mod.rs
    registry.rs              # Command registry trait
  output/
    mod.rs
    formatter.rs             # Formatter trait
    json.rs
    table.rs
    text.rs
  history/
    mod.rs                   # History file management
  session/
    mod.rs                   # Session state
  error.rs                   # Error types
  context.rs                 # Shell context
```

**Implementation Priority:**
1. Create shell context and error types
2. Implement REPL loop (basic prompt/read/write)
3. Implement command parser (tokenizer, validator)
4. Implement command registry (trait + basic registration)
5. Implement output formatters (all three formats)
6. Implement history management (file I/O + filtering)
7. Add basic tests for each component

**Testing Strategy:**
```rust
// Example unit test for parser
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parse_simple_command() {
        let input = "help";
        let parsed = Parser::parse(input).unwrap();
        assert_eq!(parsed.command, "help");
        assert!(parsed.args.is_empty());
    }
    
    #[test]
    fn test_parse_with_flags() {
        let input = "service list --status running --filter ssh";
        let parsed = Parser::parse(input).unwrap();
        assert_eq!(parsed.command, "service");
        assert_eq!(parsed.subcommand, Some("list"));
    }
}
```

**Definition of Done:**
- [ ] Shell starts and shows prompt
- [ ] "help" command works
- [ ] "version" command works
- [ ] Command parsing handles all flag types
- [ ] All three output formatters work
- [ ] History file saves/loads correctly
- [ ] Session state loads on startup
- [ ] Error handling works for all error types
- [ ] Unit test coverage > 80%

---

### Phase 1.8.2: System Commands (Week 2)

**Goals:**
- Implement system commands (help, version, status, health, services, events, audit)
- Foundation ready for IPC integration

**Commands to Implement:**
1. help - local only
2. version - local only
3. status - read-only IPC
4. health - read-only IPC
5. services - read-only IPC
6. events - read-only IPC (query)
7. audit - read-only IPC (query)

**Implementation Steps:**
```rust
// Example system command structure
pub struct HelpCommand;

impl Command for HelpCommand {
    fn name(&self) -> &str { "help" }
    fn description(&self) -> &str { "Display help information" }
    fn aliases(&self) -> Vec<&str> { vec!["h", "?"] }
    
    fn execute(&self, args: &CommandArgs, _ctx: &ShellContext) -> Result<Output> {
        // Implementation
    }
}
```

**IPC Integration Foundation:**
```rust
// Create IPC client helper
pub mod ipc {
    pub struct IpcClient {
        // Connection management
    }
    
    impl IpcClient {
        pub async fn request(&self, req: IpcRequest) -> Result<IpcResponse> {
            // Implement IPC communication
        }
    }
}
```

**Definition of Done:**
- [ ] help command lists all available commands
- [ ] version shows shell and system versions
- [ ] status command queries system core
- [ ] health command checks component health
- [ ] services command lists all services
- [ ] All commands support --format flag
- [ ] JSON output validates against schema
- [ ] Error responses follow standard format
- [ ] Commands have proper help text
- [ ] Integration tests with mock IPC

---

### Phase 1.8.3: IPC Integration & Core Commands (Week 3)

**Goals:**
- IPC client implementation
- Service, Process, Application commands
- Proper error handling and timeouts

**Commands to Implement:**
1. service list, status, restart, logs
2. process list, inspect, start, stop, restart
3. app list, inspect, launch, close

**IPC Client Implementation:**
```rust
// src/ipc/client.rs
pub struct IpcClient {
    socket_path: PathBuf,
    timeout: Duration,
    request_count: Arc<Mutex<u64>>,
}

impl IpcClient {
    pub async fn new(socket_path: PathBuf) -> Result<Self> { ... }
    
    pub async fn send_request(&self, req: IpcRequest) -> Result<IpcResponse> {
        // 1. Connect to socket
        // 2. Serialize request
        // 3. Send with size header
        // 4. Read response with timeout
        // 5. Deserialize and validate
        // 6. Return or error
    }
    
    pub async fn query_services(&self) -> Result<Vec<ServiceInfo>> { ... }
    pub async fn get_service_status(&self, service_id: &str) -> Result<ServiceStatus> { ... }
}
```

**Command Implementation Pattern:**
```rust
pub struct ServiceCommand;

impl Command for ServiceCommand {
    fn name(&self) -> &str { "service" }
    fn subcommands(&self) -> Vec<&str> { 
        vec!["list", "status", "restart", "logs"]
    }
    
    async fn execute(&self, args: &CommandArgs, ctx: &ShellContext) -> Result<Output> {
        match args.subcommand {
            "list" => self.list_services(args, ctx).await,
            "status" => self.get_status(args, ctx).await,
            _ => Err(ShellError::InvalidSubcommand),
        }
    }
}
```

**Definition of Done:**
- [ ] IPC client connects and sends requests
- [ ] Service command works with multiple subcommands
- [ ] Process command works with multiple subcommands
- [ ] Application command works with multiple subcommands
- [ ] All IPC errors are handled gracefully
- [ ] Timeout handling works correctly
- [ ] Output formatting works for complex objects
- [ ] Integration tests with real IPC (if Phase 1.7 available)
- [ ] Command help text is complete

---

### Phase 1.8.4: Filesystem & Network Commands (Week 4)

**Goals:**
- Complete filesystem commands
- Complete network commands
- System control commands (with policy checks)

**Commands to Implement:**
1. fs list, stat, search, storage, mounts
2. network status, interfaces, inspect, addresses, routes, dns, connectivity, stats
3. system shutdown, reboot

**Filesystem Command Implementation:**
```rust
// Commands use local filesystem APIs or IPC
pub struct FilesystemCommand;

impl Command for FilesystemCommand {
    async fn execute_list(&self, args: &CommandArgs) -> Result<Output> {
        // Can use std::fs or IPC to filesystem service
        // Format output based on --format flag
        // Support pagination for large results
    }
}
```

**Network Command Implementation:**
```rust
// Network commands go through IPC
pub struct NetworkCommand;

impl Command for NetworkCommand {
    async fn execute_status(&self, args: &CommandArgs, ctx: &ShellContext) -> Result<Output> {
        let client = ctx.ipc_client();
        let status = client.get_network_status().await?;
        Ok(Output::Network(status))
    }
}
```

**System Control Implementation:**
```rust
// System control requires policy checks
pub struct SystemCommand;

impl Command for SystemCommand {
    async fn execute_shutdown(&self, args: &CommandArgs, ctx: &ShellContext) -> Result<Output> {
        // 1. Check policy permission
        if !ctx.check_policy("system:shutdown")? {
            return Err(ShellError::PermissionDenied);
        }
        
        // 2. Log audit event
        ctx.log_audit("system_shutdown", &args)?;
        
        // 3. Send IPC request with delay/reason
        let req = IpcRequest::SystemShutdown {
            delay: args.delay,
            reason: args.reason.clone(),
        };
        client.send_request(req).await
    }
}
```

**Definition of Done:**
- [ ] fs list command works with various filters
- [ ] fs stat shows detailed file information
- [ ] fs search finds files by pattern
- [ ] fs storage shows mount points and usage
- [ ] Network commands show interfaces and routes
- [ ] network dns command works
- [ ] network connectivity tests connectivity
- [ ] system shutdown/reboot require policy
- [ ] Audit logging works
- [ ] All output formats work
- [ ] Complex data structures format properly

---

### Phase 1.8.5: Polish & Testing (Weeks 5-6)

**Goals:**
- Completion system foundation
- Comprehensive testing
- Documentation
- Performance optimization

**Tasks:**
1. Implement command completion foundation
   - Static completions for command names
   - Flag completions
   - Common argument patterns
   - Future: dynamic completions via IPC

2. Comprehensive testing
   - Unit tests for all modules
   - Integration tests for command workflows
   - Error case testing
   - Performance benchmarks

3. Documentation
   - User guide
   - Troubleshooting guide
   - Architecture documentation
   - API documentation (rustdoc)

4. Performance optimization
   - Profile hot paths
   - Optimize history file I/O
   - Cache service lists
   - Connection pooling for IPC

**Completion System Design:**
```rust
// Completions provided to shell (bash, zsh, etc.)
pub struct CompleterClient {
    registry: &'static CommandRegistry,
}

impl CompleterClient {
    pub fn complete_command(&self, prefix: &str) -> Vec<String> {
        // Return matching command names
    }
    
    pub fn complete_args(&self, cmd: &str, prefix: &str) -> Vec<String> {
        // Return matching arguments
    }
}

// Completion scripts in scripts/
// - scripts/completion.bash
// - scripts/completion.zsh
// - scripts/completion.fish
```

**Definition of Done:**
- [ ] Completion system returns suggestions
- [ ] Bash completion script works
- [ ] Unit test coverage > 85%
- [ ] Integration test coverage > 70%
- [ ] Performance benchmarks documented
- [ ] User documentation complete
- [ ] Rustdoc comments for public API
- [ ] Examples in docs/examples/
- [ ] Security review completed
- [ ] Release notes prepared

---

## Testing Best Practices

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_parser_empty_input() {
        let err = Parser::parse("").unwrap_err();
        assert!(matches!(err, ShellError::EmptyInput));
    }
    
    #[tokio::test]
    async fn test_ipc_client_timeout() {
        let client = IpcClient::with_timeout(Duration::from_millis(1));
        let err = client.send_request(slow_request).await.unwrap_err();
        assert!(matches!(err, ShellError::IpcTimeout));
    }
}
```

### Integration Tests

```rust
// tests/integration_test.rs
#[tokio::test]
async fn test_service_list_workflow() {
    let shell = TestShell::new().await;
    
    // Execute command
    let output = shell.execute("service list").await.unwrap();
    
    // Verify output
    assert!(output.is_ok());
    assert!(!output.services.is_empty());
}
```

### Mock IPC Server

```rust
// For testing without Phase 1.7
pub struct MockIpcServer {
    responses: Arc<Mutex<HashMap<String, IpcResponse>>>,
}

impl MockIpcServer {
    pub fn add_response(&self, command: String, response: IpcResponse) { ... }
}
```

---

## Code Organization Principles

1. **Modularity**: Each command is self-contained
2. **Testability**: Small, focused functions
3. **Error Handling**: All errors propagated with context
4. **Async**: Use tokio for all blocking I/O
5. **Safety**: No unsafe code except in FFI
6. **Performance**: Lazy evaluation where appropriate
7. **Documentation**: Comments for "why", not "what"

---

## Debugging Tips

### Enable Verbose Logging
```bash
RUST_LOG=debug cargo run -p aether-shell
RUST_LOG=aether_shell=debug,aether_core=debug cargo run
```

### Test IPC Directly
```bash
# Send test request to system core
echo '{"command": "status"}' | nc -U /run/aether/ipc/aether-system-core.sock
```

### Check Shell State
```bash
# View command history
cat ~/.aether/history

# View session state
cat ~/.aether/session.json

# Check active connections
lsof -c aethersh
```

### Profile Performance
```bash
# Flamegraph
cargo install flamegraph
cargo flamegraph -p aether-shell

# Benchmarks
cargo bench -p aether-shell
```

---

## Common Issues & Solutions

### IPC Connection Refused
**Symptom:** "Connection refused" errors
**Solution:** Ensure Phase 1.7 is running: `systemctl status aether-system-core`

### Command Timeout
**Symptom:** "IPC timeout" on every command
**Solution:** Check system load and IPC socket permissions

### History File Corruption
**Symptom:** History loading fails
**Solution:** Delete ~/.aether/history and restart (fresh history)

### Output Formatting Issues
**Symptom:** JSON output is malformed
**Solution:** Check that response structs implement Serialize correctly

---

## Performance Targets

- Shell startup: < 100ms
- Command parsing: < 1ms
- IPC request/response: < 500ms (typical)
- History file operations: < 10ms
- Output formatting: < 100ms (even for large results)

---

## Build & Release Checklist

- [ ] All tests pass
- [ ] Clippy warnings resolved
- [ ] Code formatted (cargo fmt)
- [ ] Documentation updated
- [ ] Changelog updated
- [ ] Version bumped in Cargo.toml
- [ ] Git tag created
- [ ] Binary built in release mode
- [ ] Binary size checked
- [ ] Security review done
- [ ] Integration tests pass
- [ ] User documentation reviewed
