# Phase 1.8: Aether Shell - Quick Reference Card

## 🎯 Project at a Glance

| Aspect | Details |
|--------|---------|
| **Project** | Aether OS Phase 1.8 - Aether Shell (aethersh) |
| **Goal** | Interactive CLI for system control and monitoring |
| **Timeline** | 6 weeks (5 phases, 2 weeks per phase) |
| **Status** | Planning complete, ready for implementation |
| **Type** | Native Rust binary (async/Tokio) |
| **Language** | Rust 1.70+ |
| **Platform** | Linux 5.10+ (Unix sockets) |

---

## 📂 Directory Structure

```
aether-shell/
├── src/
│   ├── main.rs                 # Entry point
│   ├── lib.rs                  # Library root
│   ├── shell/                  # REPL and parser
│   ├── commands/               # 6+ command modules
│   ├── output/                 # Formatters (JSON/table/text)
│   ├── ipc/                    # Service communication
│   ├── history/                # Command history
│   ├── session/                # Session state
│   ├── security/               # Policy & audit
│   └── error.rs                # Error types
├── tests/                      # Integration tests
├── benches/                    # Performance benchmarks
├── Cargo.toml                  # Package config
└── README.md                   # Local README
```

---

## 🔨 Build & Run Commands

```bash
# Build
cargo build -p aether-shell

# Build release (optimized)
cargo build -p aether-shell --release

# Run
cargo run -p aether-shell

# Run tests
cargo test -p aether-shell

# Run with logging
RUST_LOG=debug cargo run -p aether-shell

# Lint
cargo clippy -p aether-shell

# Format
cargo fmt -p aether-shell

# Benchmarks
cargo bench -p aether-shell

# Coverage
cargo tarpaulin -p aether-shell --html
```

---

## 📦 Key Dependencies

| Crate | Purpose | Version |
|-------|---------|---------|
| `tokio` | Async runtime | 1.x |
| `serde` | Serialization | 1.x |
| `serde_json` | JSON support | 1.x |
| `clap` | Arg parsing | 4.x |
| `termcolor` | Colored output | 1.x |
| `regex` | Pattern matching | 1.x |
| `chrono` | Timestamps | 0.4.x |
| `anyhow` | Error handling | 1.x |

---

## 📋 Module Breakdown (14 total)

### Core (6)
- `shell::repl` - REPL loop
- `shell::parser` - Command parsing
- `shell::executor` - Command dispatch
- `commands::registry` - Command registry
- `output::formatter` - Output formatting
- `error` - Error types

### Commands (6)
- `commands::system` - help, version, status, health
- `commands::service` - Service management
- `commands::process` - Process management
- `commands::filesystem` - Filesystem access
- `commands::application` - App management
- `commands::network` - Network info

### Support (2)
- `history` - Command history file
- `session` - Session state persistence

---

## 🎮 Command Categories (35+ total)

| Category | Count | Commands |
|----------|-------|----------|
| **System** | 7 | help, version, status, health, services, events, audit, exit |
| **Service** | 4 | list, status, restart, logs |
| **Process** | 5 | list, inspect, start, stop, restart |
| **Filesystem** | 5 | list, stat, search, storage, mounts |
| **Application** | 4 | list, inspect, launch, close |
| **Network** | 8 | status, interfaces, inspect, addresses, routes, dns, connectivity, stats |
| **System Control** | 2 | shutdown, reboot |

---

## 🔌 IPC Integration Points

```
Commands with IPC Calls:
  ├─ Service commands → system-core (ServiceQuery)
  ├─ Process commands → process-manager (ProcessQuery)
  ├─ App commands → application-manager (AppQuery)
  ├─ Filesystem commands → filesystem-service (FSQuery)
  ├─ Network commands → network-service (NetworkQuery)
  └─ Control commands → system-core (SystemControl)

Socket Path: /run/aether/ipc/aether-system-core.sock
Max Request: 8KB
Max Response: 1MB
Timeout: 30 seconds
```

---

## 📊 Output Formats

```bash
# Default (table)
$ aether> service list
Name                 Status    CPU%    Memory
aether-system-core   running   2.1     45MB

# JSON format
$ aether> service list --format json
{"ok": true, "services": [...]}

# Text format
$ aether> service list --format text
service1: running
service2: stopped
```

---

## 🔐 Security Checklist

- [ ] Unix socket peer credentials check
- [ ] Policy enforcement for sensitive ops
- [ ] Input validation on all commands
- [ ] Path traversal attack prevention
- [ ] Secret filtering in history (--password, --token)
- [ ] JSON output properly escaped
- [ ] Audit logging for all ops
- [ ] Error messages don't leak info
- [ ] Size/timeout limits enforced
- [ ] No unsafe code (except documented)

---

## ✅ Phase Checklist

### Phase 1.8.1 (Weeks 1-2): Foundation
- [ ] REPL loop with prompt
- [ ] Command parser (tokenize/validate)
- [ ] Command registry
- [ ] Formatters (JSON/table/text)
- [ ] History file management
- [ ] Session state loading
- [ ] Local commands work (help, version)
- [ ] > 80% test coverage

### Phase 1.8.2 (Week 2): System Commands
- [ ] help command
- [ ] version command
- [ ] status command (IPC)
- [ ] health command (IPC)
- [ ] services command (IPC)
- [ ] events command (IPC)
- [ ] audit command (IPC)
- [ ] > 80% test coverage

### Phase 1.8.3 (Week 3): IPC Integration
- [ ] IPC client async implementation
- [ ] Service commands (4 subcommands)
- [ ] Process commands (5 subcommands)
- [ ] Application commands (4 subcommands)
- [ ] Error handling with timeouts
- [ ] Mock IPC server for tests
- [ ] > 75% test coverage

### Phase 1.8.4 (Week 4): Advanced Commands
- [ ] Filesystem commands (5 subcommands)
- [ ] Network commands (8 subcommands)
- [ ] System shutdown/reboot (with policy)
- [ ] Policy enforcement
- [ ] Audit logging integration
- [ ] > 70% test coverage

### Phase 1.8.5 (Weeks 5-6): Polish
- [ ] Completion system (bash/zsh)
- [ ] > 85% test coverage
- [ ] Performance benchmarks
- [ ] User documentation
- [ ] Security review
- [ ] Release preparation

---

## 🧪 Testing Strategy

```
Unit Tests (55-65%)
├─ Parser: tokenize, validate, edge cases
├─ Formatters: JSON escape, table layout
├─ History: file I/O, filtering
└─ Commands: logic, error cases

Integration Tests (25-35%)
├─ IPC client communication
├─ Command workflows
├─ Output format consistency
└─ Error handling

E2E Tests (5-10%)
├─ Full shell workflows
├─ Real service interaction
└─ User scenarios
```

**Target Coverage: > 85%**

---

## 🎯 Success Criteria

- [x] Plan document created
- [ ] All 35+ commands implemented
- [ ] All commands support --format flag
- [ ] IPC communication working
- [ ] History saved/loaded correctly
- [ ] Audit logging functional
- [ ] > 85% test coverage
- [ ] Security review passed
- [ ] Performance targets met
- [ ] Documentation complete
- [ ] Release ready

---

## 🚨 Common Pitfalls

1. **IPC Timeouts** → Implement proper async/await and timeout handling
2. **Large Output** → Implement pagination for big result sets
3. **History Corruption** → Use atomic writes and validation
4. **Secret Leakage** → Filter commands with --password, --token, etc.
5. **Path Traversal** → Reject ".." in all file paths
6. **Clippy Warnings** → Fix early to maintain code quality
7. **Missing Tests** → Write tests alongside implementation

---

## 📈 Performance Targets

| Operation | Target | Notes |
|-----------|--------|-------|
| Shell startup | < 100ms | Cold start |
| Parse command | < 1ms | All commands |
| IPC request/response | < 500ms | Network dependent |
| History file I/O | < 10ms | Append/load |
| Output formatting | < 100ms | Even large results |

---

## 🔗 Key Files to Know

| File | Purpose |
|------|---------|
| `src/shell/parser.rs` | Command parsing logic |
| `src/commands/registry.rs` | Command registry trait |
| `src/ipc/client.rs` | IPC client implementation |
| `src/output/formatter.rs` | Output formatting |
| `src/history/mod.rs` | History file management |
| `src/security/policy.rs` | Policy enforcement |
| `tests/mock_ipc.rs` | Mock IPC for testing |

---

## 💬 Help Commands

```bash
# General help
help

# Help for specific command
help service

# Help with examples
help process list

# All commands
help --all

# Long format
help --detailed
```

---

## 🔍 Debugging

```bash
# Verbose mode
aethersh --verbose

# Debug logging
RUST_LOG=debug aethersh

# Detailed logging
RUST_LOG=aether_shell=debug,aether_core=debug aethersh

# Check history
cat ~/.aether/history

# Check session state
cat ~/.aether/session.json

# Test IPC directly
echo '{"command": "status"}' | nc -U /run/aether/ipc/aether-system-core.sock
```

---

## 🚀 Quick Start (Developer)

1. **Clone/navigate to repo:**
   ```bash
   cd /path/to/aether-os
   ```

2. **Review documentation:**
   ```bash
   cat docs/phase-1-8/INDEX.md
   cat docs/phase-1-8/PHASE_1_8_IMPLEMENTATION_PLAN.md
   ```

3. **Start Phase 1.8.1:**
   - Read DEVELOPMENT_GUIDE.md Phase 1.8.1 section
   - Create directory structure
   - Implement parser, REPL, formatters
   - Write tests

4. **Progress through phases:**
   - Refer to COMMAND_REFERENCE.md for specs
   - Use TESTING_GUIDE.md for test patterns
   - Check SECURITY_GUIDE.md for auth/audit

5. **Before release:**
   - Run: `cargo test -p aether-shell`
   - Check: `cargo clippy -p aether-shell`
   - Format: `cargo fmt -p aether-shell`
   - Review: SECURITY_GUIDE.md checklist

---

## 📚 Documentation Quick Links

- **Overview:** README.md
- **Complete Spec:** PHASE_1_8_IMPLEMENTATION_PLAN.md
- **Architecture:** ARCHITECTURE.md
- **Commands:** COMMAND_REFERENCE.md
- **Implementation:** DEVELOPMENT_GUIDE.md
- **Testing:** TESTING_GUIDE.md
- **Security:** SECURITY_GUIDE.md
- **Index:** INDEX.md

---

## 🎓 Learning Resources

**For Rust async/IPC:**
- Tokio documentation (async runtime)
- Unix domain socket examples
- Serialization with serde_json

**For command-line design:**
- clap documentation (argument parsing)
- Previous shell implementations
- COMMAND_REFERENCE.md examples

**For testing Rust:**
- tokio-test crate
- Mock patterns in TESTING_GUIDE.md
- Criterion for benchmarks

---

## 💾 File Locations

```
Main documentation:    docs/phase-1-8/
Source code (future):  shell/src/
Tests (future):        shell/tests/
Benchmarks (future):   shell/benches/
Build output:          target/
```

---

## 📞 Key Contacts

For questions about:
- **Architecture:** See ARCHITECTURE.md
- **Commands:** See COMMAND_REFERENCE.md
- **Implementation:** See DEVELOPMENT_GUIDE.md
- **Testing:** See TESTING_GUIDE.md
- **Security:** See SECURITY_GUIDE.md
- **Timeline:** See README.md

---

**Version 1.0 - January 2024**
**Status: Ready for Implementation ✅**

All documentation complete. Begin with DEVELOPMENT_GUIDE.md Phase 1.8.1.
