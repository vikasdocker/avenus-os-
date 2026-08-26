# Phase 1.8: Aether Shell and System Control Interface
## Detailed Implementation Plan

### OVERVIEW
This phase delivers the native Aether Shell (aethersh), a unified command-line interface for system control, 
monitoring, and management. The shell provides interactive access to system services via IPC while maintaining 
security, audit compliance, and user experience excellence.

### DIRECTORY STRUCTURE

```
shell/
├── aether-shell/                 # Main shell binary crate
│   ├── Cargo.toml
│   ├── src/
│   │   ├── main.rs              # Entry point, REPL orchestrator
│   │   ├── lib.rs               # Library exports
│   │   ├── shell/
│   │   │   ├── mod.rs           # Shell orchestrator
│   │   │   ├── repl.rs          # REPL loop implementation
│   │   │   ├── parser.rs        # Command parsing
│   │   │   └── executor.rs      # Command execution dispatch
│   │   ├── commands/
│   │   │   ├── mod.rs           # Command registry
│   │   │   ├── system.rs        # System commands (help, version, status, health, exit)
│   │   │   ├── service.rs       # Service commands via IPC
│   │   │   ├── filesystem.rs    # Filesystem commands via IPC
│   │   │   ├── process.rs       # Process commands via IPC
│   │   │   ├── application.rs   # Application commands via IPC
│   │   │   ├── network.rs       # Network commands via IPC
│   │   │   └── registry.rs      # Command registry implementation
│   │   ├── output/
│   │   │   ├── mod.rs           # Output formatting
│   │   │   ├── formatter.rs     # Format selection and dispatch
│   │   │   ├── json.rs          # JSON output formatter
│   │   │   ├── table.rs         # Human-readable table formatter
│   │   │   ├── text.rs          # Plain text formatter
│   │   │   └── error.rs         # Error formatting
│   │   ├── history/
│   │   │   ├── mod.rs           # History management
│   │   │   ├── storage.rs       # History file operations
│   │   │   └── filter.rs        # Secret filtering for safe storage
│   │   ├── session/
│   │   │   ├── mod.rs           # Session state management
│   │   │   ├── context.rs       # Execution context
│   │   │   └── state.rs         # Session state persistence
│   │   ├── ipc/
│   │   │   ├── mod.rs           # IPC client abstraction
│   │   │   ├── client.rs        # IPC socket operations
│   │   │   ├── request.rs       # Request construction
│   │   │   └── response.rs      # Response parsing
│   │   ├── completion/
│   │   │   ├── mod.rs           # Completion system (foundation)
│   │   │   ├── engine.rs        # Completion engine
│   │   │   └── providers.rs     # Completion providers
│   │   ├── security/
│   │   │   ├── mod.rs           # Security utilities
│   │   │   ├── policy.rs        # Policy enforcement
│   │   │   └── audit.rs         # Audit logging
│   │   ├── error.rs             # Error types and handling
│   │   └── config.rs            # Configuration
│   └── tests/
│       ├── integration_tests.rs
│       ├── fixtures/
│       └── mocks/
└── README.md

docs/
└── phase-1-8/
    ├── IMPLEMENTATION_PLAN.md    # This document
    ├── ARCHITECTURE.md
    ├── COMMAND_REGISTRY.md
    ├── IPC_PROTOCOL.md
    ├── OUTPUT_FORMATS.md
    ├── SECURITY_MODEL.md
    ├── TESTING_STRATEGY.md
    └── COMPLETION_SYSTEM.md
```

### MODULE ARCHITECTURE

#### 1. Shell Orchestrator (shell/)
- **Purpose**: Core REPL loop and command orchestration
- **Responsibilities**:
  - Interactive prompt display (aether>)
  - Line reading and preprocessing
  - Command parsing and validation
  - Execution dispatch
  - Output formatting
  - Error handling and recovery
- **Key Files**: repl.rs, parser.rs, executor.rs

#### 2. Command Registry (commands/)
- **Purpose**: Extensible command registration and lookup
- **Responsibilities**:
  - Register all commands (system, filesystem, process, app, network)
  - Command metadata (name, description, aliases, args)
  - Help text generation
  - Parameter validation
  - Command discovery
- **Key Files**: registry.rs, mod.rs

#### 3. System Commands (commands/system.rs)
Commands: help, version, status, health, services, events, audit, exit
- No IPC dependency (local only)
- Help dynamically generated from registry
- Version from Cargo.toml
- Status provides shell session info
- Health aggregates service health from last IPC fetch

#### 4. Service Commands (commands/service.rs)
Commands: service list, service status, service restart
- IPC to System Core
- Request construction for service operations
- Response parsing and formatting
- Error handling and retry logic

#### 5. Filesystem Commands (commands/filesystem.rs)
Commands: fs list, fs stat, fs search, fs storage, fs mounts
- IPC to Filesystem Service
- Path handling and validation
- Recursive operations
- Storage information aggregation

#### 6. Process Commands (commands/process.rs)
Commands: process list, process inspect, process start, process stop, process restart
- IPC to Process Manager
- PID filtering and search
- Process lifecycle operations
- Resource and state inspection

#### 7. Application Commands (commands/application.rs)
Commands: app list, app inspect, app launch, app close
- IPC to Application Manager
- App discovery and manifest inspection
- Lifecycle management
- Environment and capability inspection

#### 8. Network Commands (commands/network.rs)
Commands: network status, interfaces, inspect, addresses, routes, dns, connectivity, stats
- IPC to Network Service
- Network topology display
- Configuration inspection
- Diagnostics and testing

#### 9. Output Formatting (output/)
- **Formatter**: Dispatch based on --format flag (json, table, text)
- **JSON**: Structured output for parsing and automation
- **Table**: Human-readable aligned columns
- **Text**: Plain text for simple output
- **Consistency**: Common result structures across all commands

#### 10. History Management (history/)
- **Storage**: ~/.aether/history (1000 command limit)
- **Filtering**: Strip secrets before storage (passwords, tokens, --password, --token)
- **Search**: Query by pattern
- **Persistence**: Auto-save after each command
- **Security**: File mode 0600, loaded only for current session

#### 11. Session State (session/)
- **Context**: Current directory, output format, user, environment
- **Persistence**: ~/.aether/session.json (not forced across sessions)
- **State**: Last command, exit code, output format preference
- **Environment**: Shell variables (AETHER_HOME, AETHER_SHELL_VERSION)

#### 12. IPC Client (ipc/)
- **Request Builder**: Construct IPC requests for each service
- **Socket Communication**: Unix-domain socket to System Core
- **Response Parsing**: Parse structured IPC responses
- **Error Handling**: Convert IPC errors to shell errors
- **Retry Logic**: Exponential backoff for transient failures

#### 13. Completion System (completion/)
- **Foundation Layer**: Parser for command structure
- **Command Completion**: List matching commands
- **Argument Completion**: Service names, PID lists, paths
- **Flag Completion**: Valid flags for each command
- **Dynamic**: Load from service discovery at runtime

#### 14. Security Module (security/)
- **Policy Enforcement**: Permission checks before IPC calls
- **Audit Logging**: Record all commands with timestamp and user
- **Sensitive Data**: Mask in output when needed
- **Authentication**: Peer credential checking (future)

### COMMAND REGISTRY DESIGN

```rust
pub trait Command: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn aliases(&self) -> &[&str];
    fn args(&self) -> &[CommandArg];
    fn execute(&self, args: &[String], ctx: &ShellContext) -> Result<Output>;
}

pub struct CommandRegistry {
    commands: HashMap<String, Arc<dyn Command>>,
    aliases: HashMap<String, String>,
}

pub struct CommandArg {
    pub name: String,
    pub required: bool,
    pub help: String,
    pub value_kind: ArgValueKind,
}

pub enum ArgValueKind {
    String,
    ServiceId,
    ProcessId,
    FilePath,
    AppId,
}
```

### IPC INTEGRATION POINTS

1. **System Core** (Service Manager):
   - health, services, service status/start/stop/restart
   - audit, events queries
   - shutdown, reboot

2. **Filesystem Service**:
   - list, stat, search, mounts
   - storage information

3. **Process Manager**:
   - list, inspect
   - start, stop, restart operations

4. **Application Manager**:
   - list, inspect
   - launch, close operations

5. **Network Service**:
   - status, interfaces, addresses, routes
   - dns, connectivity, stats queries

### OUTPUT FORMAT SPECIFICATIONS

All commands support --format flag (default: table)

**JSON Schema Example**:
```json
{
  "ok": true,
  "command": "service status",
  "data": {
    "id": "example-service",
    "status": "running",
    "uptime_ms": 123456
  }
}
```

**Table Format Example**:
```
SERVICE ID          STATUS    UPTIME
example-service     running   2m 3s
other-service       stopped   —
```

**Error Handling**:
```json
{
  "ok": false,
  "command": "service status",
  "error": {
    "code": "NOT_FOUND",
    "message": "Service 'foo' not found"
  }
}
```

### HISTORY AND SESSION MANAGEMENT

**History File**: ~/.aether/history
- One command per line (most recent last)
- Format: [timestamp] command args
- Secrets filtered before storage
- Max 1000 lines (rotate old entries)
- Searchable via history search <pattern>

**Session State**: ~/.aether/session.json
```json
{
  "last_command": "service list",
  "output_format": "table",
  "shell_version": "0.1.0",
  "created_at": "2026-08-23T11:59:03Z"
}
```

### COMPLETION SYSTEM DESIGN

**Phase 1.8 Scope**: Foundation only
- Command name completion
- Common flag completion (--help, --format)
- Static argument lists for known enums

**Future Phases**:
- Dynamic service/process/app name completion
- Path completion from filesystem service
- Shell integration (bash/zsh completion scripts)

### SECURITY ARCHITECTURE

1. **Socket Security**:
   - Mode 0600 (user-only)
   - Peer credential verification
   - Request size limits (8KB)

2. **Audit Logging**:
   - Every command logged with timestamp
   - User context captured
   - Failed commands also logged
   - Retention via System Core audit service

3. **Permission Model**:
   - Inherited from IPC peer credentials
   - Policy checks before sensitive operations
   - Shutdown/reboot require explicit policy

4. **Secret Protection**:
   - Command line args NOT logged
   - Passwords/tokens filtered from history
   - Sensitive output masked in table format

### TESTING STRATEGY

**Unit Tests**:
- Command parsing (valid/invalid inputs)
- Output formatting (all formats)
- History filtering (secret detection)
- Error handling

**Integration Tests**:
- Mock IPC client
- Full command flow (parse → execute → format)
- Service communication
- Error scenarios

**System Tests**:
- Real IPC to System Core
- Multi-command workflows
- History persistence
- Session state

**Test Fixtures**:
- Mock service responses
- Sample command inputs
- Error scenarios

### IMPLEMENTATION PHASES

#### Phase 1.8.1: Foundation (Week 1-2)
- [ ] Cargo project setup
- [ ] Shell REPL loop (repl.rs, main.rs)
- [ ] Command parser (parser.rs)
- [ ] Command registry (registry.rs)
- [ ] Basic output formatting (json, table, text)

#### Phase 1.8.2: System Commands (Week 2-3)
- [ ] help, version, status, health, services, events, audit
- [ ] exit command
- [ ] History storage and filtering
- [ ] Session state management

#### Phase 1.8.3: IPC Integration (Week 3-4)
- [ ] IPC client (client.rs, request.rs, response.rs)
- [ ] Service commands
- [ ] Process commands
- [ ] Application commands

#### Phase 1.8.4: Advanced Commands (Week 4-5)
- [ ] Filesystem commands
- [ ] Network commands
- [ ] System control (shutdown, reboot with policy)
- [ ] Error handling and recovery

#### Phase 1.8.5: Completion & Polish (Week 5-6)
- [ ] Completion system foundation
- [ ] Comprehensive testing
- [ ] Documentation
- [ ] Performance optimization

### SECURITY VALIDATION

Before phase completion:
- [ ] No secrets in history
- [ ] IPC socket permissions enforced
- [ ] Audit logging working
- [ ] Peer credentials validated
- [ ] All commands properly authorized
- [ ] Error messages don't leak sensitive info

### BUILD CONFIGURATION

```toml
[package]
name = "aether-shell"
version.workspace = true
edition.workspace = true

[dependencies]
aether-core = { path = "../../core/aether-core" }
tokio = { version = "1", features = ["full"] }
clap = { version = "4", features = ["derive"] }
serde_json = "1"
serde = { version = "1", features = ["derive"] }
termcolor = "1"
regex = "1"

[dev-dependencies]
tempfile = "3"
```

### SUCCESS CRITERIA

1. **Functionality**:
   - All 30+ commands implemented and working
   - IPC communication reliable
   - All output formats functional

2. **Quality**:
   - >90% test coverage
   - No panics in normal operation
   - Proper error handling throughout

3. **Security**:
   - No secrets in history
   - All operations audited
   - IPC properly authenticated

4. **Documentation**:
   - Command help text complete
   - Architecture documentation
   - Developer guide for adding commands

5. **Performance**:
   - Sub-100ms for local commands
   - Sub-500ms for remote IPC commands
   - History operations <50ms

### RISKS AND MITIGATIONS

| Risk | Mitigation |
|------|-----------|
| IPC service unavailable | Graceful error messaging, cached data |
| Large output datasets | Pagination support, streaming responses |
| Command injection | Input validation, proper escaping |
| Performance issues | Profiling, lazy loading, caching |
| History bloat | Rotation policy, cleanup utilities |

### DELIVERABLES CHECKLIST

- [ ] aether-shell binary (aethersh)
- [ ] All commands implemented
- [ ] Help system complete
- [ ] Output formatting (JSON, table, text)
- [ ] History persistence with filtering
- [ ] Session state management
- [ ] IPC integration for all services
- [ ] Completion foundation
- [ ] Comprehensive test suite
- [ ] Security audit complete
- [ ] User documentation
- [ ] Developer guide
- [ ] Architecture documentation
