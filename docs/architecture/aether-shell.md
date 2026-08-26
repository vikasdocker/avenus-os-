# Aether Shell (Phase 1.8) - Architecture and Design

## Overview

The Aether Shell (`aethersh`) is the primary user-facing command interface for Aether OS. It operates as a trusted intermediary between users and Aether system services, communicating through Aether IPC rather than directly invoking Linux commands.

## Core Principles

1. **No Direct Linux System Calls**: The shell never directly manipulates Linux internals. All operations flow through Aether services.

2. **AI-Compatible Design**: The command architecture is identical to what future AI agents will use. A shell command like `app launch browser` resolves to the same capability request as an AI intent "Open Firefox."

3. **Defense in Depth**: Every command passes through parser → validator → identity check → capability check → authorization → IPC → service.

4. **Structured Errors**: All errors are machine-readable with error codes, messages, and service context.

5. **Audit Everything**: Command execution is logged with actor, command, capability decision, and result (without logging secrets).

## Architecture

```
User Input
    ↓
Interactive Prompt (aether>)
    ↓
Command Parser
    ↓
Command Registry (trait-based dispatch)
    ↓
Argument Validation
    ↓
Session/Identity Check
    ↓
Capability Check
    ↓
Policy/Confirmation Check
    ↓
Service IPC Call
    ↓
Structured Output Formatter
    ↓
Human-readable or JSON
```

## Module Structure

```
src/
├── main.rs                 # Entry point, REPL loop
├── command/
│   ├── mod.rs             # Command trait, registry
│   ├── system.rs          # help, version, status, health, services, events, audit
│   ├── filesystem.rs      # fs list, stat, search, storage, mounts
│   ├── process.rs         # process list, inspect, start, stop, restart
│   ├── application.rs     # app list, inspect, launch, close
│   └── network.rs         # network status, interfaces, inspect, etc.
├── session.rs             # Session state, identity, capabilities
├── history.rs             # Command history with secret filtering
└── output.rs              # Formatter for text/JSON output

tests/
└── unit_tests.rs          # Command, session, history tests
```

## Command Registry Design

Commands are registered as trait implementations of the `Command` trait:

```rust
pub trait Command: Send + Sync {
    fn metadata(&self) -> &CommandMetadata;
    async fn execute(
        &self,
        args: &[&str],
        session: &ShellSession,
        formatter: &mut OutputFormatter,
        history: &ShellHistory,
    ) -> Result<()>;
}

pub struct CommandMetadata {
    pub name: String,
    pub description: String,
    pub usage: String,
    pub required_capability: Option<String>,
    pub risk_level: String,
    pub requires_confirmation: bool,
}
```

Each command declares:
- **Name**: Command identifier
- **Description**: Help text
- **Usage**: Argument syntax
- **Required Capability**: Which Aether capability is needed
- **Risk Level**: low/medium/high/critical
- **Confirmation Required**: Whether user must confirm execution

## System Commands

### help
- **Purpose**: Display available commands
- **Capability**: None required
- **Risk**: Low
- **Usage**: `help [command]`
- **Output**: Command list with descriptions

### version
- **Purpose**: Display shell version
- **Capability**: None required
- **Risk**: Low
- **Output**: Version, phase, status

### status
- **Purpose**: Show system status
- **Capability**: None required
- **Risk**: Low
- **Output**: System state, session ID, actor, uptime, service counts

### health
- **Purpose**: Display system health
- **Capability**: None required
- **Risk**: Low
- **Output**: Overall health, component health

### services
- **Purpose**: List all registered services
- **Capability**: None required (unless filtering restricted)
- **Risk**: Low
- **Output**: Service list with status

### events
- **Purpose**: Show system events
- **Capability**: None required
- **Risk**: Low
- **Usage**: `events [service]`
- **Output**: Event log with timestamps

### audit
- **Purpose**: Show audit log
- **Capability**: audit.read required
- **Risk**: Low
- **Usage**: `audit [--limit N]`
- **Output**: Audit entries (no sensitive data)

### system
- **Purpose**: System control
- **Capability**: system.control required
- **Risk**: Critical
- **Usage**: `system shutdown|reboot`
- **Confirmation**: Required
- **Output**: Confirmation receipt

## Filesystem Commands

All filesystem operations use the Filesystem Service IPC, never direct Linux syscalls.

### fs list <path>
- Lists directory contents
- Capability: `filesystem.read`
- Respects filesystem permissions

### fs stat <path>
- Shows file metadata
- Capability: `filesystem.read`

### fs search <path> <pattern>
- Searches for files matching pattern
- Capability: `filesystem.read`

### fs storage
- Shows storage usage and quotas
- Capability: `storage.read`

### fs mounts
- Shows mounted filesystems
- Capability: `filesystem.read`

## Process Commands

All operations through Process Manager IPC.

### process list
- Lists all processes with status
- Capability: `process.read`

### process inspect <pid>
- Shows detailed process info
- Capability: `process.read`

### process start <executable>
- Starts a new process
- Capability: `process.start`

### process stop <pid>
- Terminates a process
- Capability: `process.stop`
- Confirmation: May be required

### process restart <pid>
- Stops and starts a process
- Capability: `process.restart`

## Application Commands

All operations through Application Manager IPC.

### app list
- Lists installed/registered applications
- Capability: `application.read`

### app inspect <id>
- Shows application details
- Capability: `application.read`

### app launch <id>
- Starts an application
- Capability: `application.launch`

### app close <id>
- Closes an application
- Capability: `application.close`

## Network Commands

All operations through Network Service IPC.

### network status
- Shows network connectivity status
- Capability: `network.read`

### network interfaces
- Lists network interfaces
- Capability: `network.read`

### network inspect <interface>
- Shows interface details
- Capability: `network.read`

### network addresses
- Shows IP addresses
- Capability: `network.read`

### network routes
- Shows routing table
- Capability: `network.read`

### network dns
- Shows DNS configuration
- Capability: `network.read`

### network connectivity
- Tests network connectivity
- Capability: `network.read`

### network stats
- Shows network statistics
- Capability: `network.read`

## Session Management

Each shell session maintains:

```rust
pub struct ShellSession {
    session_id: String,              // UUID for this session
    actor: String,                   // Current user
    authentication_state: AuthenticationState,
    capabilities: Vec<String>,       // Granted capabilities
    shell_version: String,
    startup_time: SystemTime,
    current_context: String,         // Current directory/context
}
```

- **Session ID**: Unique identifier for this shell session (for audit)
- **Actor**: Authenticated user identity
- **Capabilities**: Dynamically granted abilities (subset of user's total capabilities)
- **Context**: Current directory or execution context

## Output Formatting

The shell supports two output modes, selectable via flag or environment variable:

### Text Mode (Default)
```
aether> version
Aether Shell v0.1.0
Phase: 1.8
Status: development
```

### JSON Mode
```
aether> version --json
{
  "name": "Aether Shell",
  "version": "0.1.0",
  "phase": "1.8",
  "status": "development"
}
```

Set via:
- Command flag: `--json`
- Environment: `AETHER_JSON_OUTPUT=1`
- Prefix: `json <command>`

## Error Handling

All errors are structured:

```json
{
  "error": {
    "code": "PERMISSION_DENIED",
    "message": "Capability 'filesystem.write' not available",
    "service": "filesystem",
    "request": "fs.write"
  }
}
```

Standard error codes:
- `UNKNOWN_COMMAND`: Command not found
- `INVALID_ARGUMENTS`: Bad argument format
- `MISSING_ARGUMENT`: Required argument not provided
- `INVALID_PATH`: Path validation failed
- `INVALID_PID`: PID validation failed
- `PERMISSION_DENIED`: Capability check failed
- `NOT_AUTHENTICATED`: No session identity
- `AUTHORIZATION_DENIED`: Policy denied request
- `CONFIRMATION_REQUIRED`: User must confirm
- `CONFIRMATION_DENIED`: User declined
- `SERVICE_UNAVAILABLE`: Service not responding
- `IPC_FAILURE`: IPC call failed
- `INTERNAL_ERROR`: Unexpected error

## History Management

The shell maintains command history with security:

- **Persistent**: Saved to `~/.aether_shell_history`
- **Size Limited**: Maximum 1000 entries (configurable)
- **Secret Filtering**: Commands containing "password", "token", "secret", "key", or "credential" are never stored
- **Clearable**: `history clear` command

## Argument Validation

Before sending to service:

1. **Count Check**: Required vs. optional args
2. **Type Check**: String, number, enum, resource ID
3. **Path Validation**: Path exists, is readable, within authorized scope
4. **Number Validation**: Valid integer/float, within range
5. **Enum Validation**: Value is in allowed set
6. **ID Validation**: Resource IDs match required format

Server-side validation is still required (never trust client).

## Capability System

Commands declare required capabilities. Session has granted capabilities. Before execution:

```rust
if !session.has_capability(&command_metadata.required_capability) {
    return Err("PERMISSION_DENIED");
}
```

Capabilities are hierarchical:
- `filesystem.*` - Filesystem access
- `process.*` - Process management
- `network.*` - Network access
- `system.*` - System control
- `audit.*` - Audit log access

## Confirmation Policy

Critical operations (shutdown, process termination, filesystem deletion) may require user confirmation. The shell:

1. Shows operation description
2. Requests confirmation
3. Logs confirmation decision in audit
4. Proceeds or aborts based on response

## Audit Logging

Every command execution is logged with:
- **Session ID**: Links to this shell session
- **Actor**: Who ran the command
- **Command**: What was executed
- **Capability**: What was checked
- **Decision**: Allowed/Denied
- **Result**: Success/Failure
- **Duration**: How long it took

Never logged:
- Passwords
- Tokens
- Secrets
- File contents
- Private credentials
- Sensitive command arguments

## IPC Integration

Each command module contains an IPC client for its service:

```rust
// filesystem.rs - calls Filesystem Service IPC
pub async fn fs_list(path: &str) -> Result<FilesystemListing> {
    let request = IpcRequest {
        service_id: "filesystem".to_string(),
        command: "list".to_string(),
        parameters: json!({"path": path}),
    };
    
    let response = ipc_client.send(&request).await?;
    // ... parse and return
}
```

No direct Linux syscalls. No direct service spawning. All through IPC.

## Future AI Compatibility

The command architecture is designed for AI use:

```rust
// Tomorrow's AI agent will construct the same request
let request = AetherRequest {
    action: "application.launch",
    target: "browser",
    source: "ai_agent",
};

// Today's shell also produces the same request
let request = AetherRequest {
    action: "application.launch",
    target: "browser",
    source: "shell_user",
};

// Both go through the same capability check, policy, audit
```

## Security Model

**Threat**: Malicious shell commands
**Defense**: Every command is:
1. Parsed (not executed as shell)
2. Validated (arguments checked)
3. Authorized (capability verified)
4. Audited (logged)
5. Isolated (via IPC, not direct syscalls)

**Threat**: Secret leakage
**Defense**: History filters secrets, audit doesn't log sensitive args

**Threat**: Privilege escalation
**Defense**: Session capabilities are subset of user's total, strictly enforced

**Threat**: IPC spoofing
**Defense**: IPC socket is local-only, mode 0600, future peer credential verification

## Performance Targets

- Shell startup: < 100ms
- Command parsing: < 10ms
- IPC round-trip: < 50ms
- JSON output generation: < 10ms

## Testing Strategy

### Unit Tests
- Command metadata loading
- Argument parsing
- Session creation and capabilities
- History filtering
- Output formatting

### Integration Tests
- Command execution with mock services
- Capability enforcement
- Error handling
- JSON output validation

### End-to-End Tests (QEMU)
- Interactive shell startup
- Live command execution
- IPC to real services
- Output validation

## Next Phases

1. **Phase 1.8.1**: Implement Filesystem IPC client
2. **Phase 1.8.2**: Implement Process IPC client
3. **Phase 1.8.3**: Implement Application IPC client
4. **Phase 1.8.4**: Implement Network IPC client
5. **Phase 1.8.5**: Integration and QEMU validation
