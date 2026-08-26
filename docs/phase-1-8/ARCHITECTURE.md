# Phase 1.8: Aether Shell Architecture

## High-Level Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    User Terminal                            │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                   Aether Shell (aethersh)                   │
├─────────────────────────────────────────────────────────────┤
│ REPL Loop                                                   │
│  └─ Prompt Display ("aether>")                              │
│  └─ Line Reading & Preprocessing                            │
│  └─ History Management (filtering, storage)                 │
│  └─ Session State Management                                │
└──────────────────────────┬──────────────────────────────────┘
                           │
┌──────────────────────────▼──────────────────────────────────┐
│                  Command Processing                         │
├─────────────────────────────────────────────────────────────┤
│ Parser: Tokenize and validate command                       │
│ Registry: Lookup and resolve command                        │
│ Executor: Dispatch to appropriate handler                   │
└──────────────────────────┬──────────────────────────────────┘
                           │
        ┌──────────────────┼──────────────────┐
        │                  │                  │
        ▼                  ▼                  ▼
   ┌─────────┐        ┌─────────┐      ┌──────────┐
   │ System  │        │ Local   │      │ Remote   │
   │ Commands│        │ State   │      │ IPC      │
   │ (help,  │        │ (history│      │ Commands │
   │version) │        │session) │      │(service, │
   └─────────┘        └─────────┘      │ process) │
        │                  │                  │
        └──────────────────┼──────────────────┘
                           │
                    ┌──────▼──────┐
                    │   Output    │
                    │ Formatter   │
                    ├─────────────┤
                    │ - JSON      │
                    │ - Table     │
                    │ - Text      │
                    └──────┬──────┘
                           │
                           ▼
                    ┌──────────────┐
                    │ User Display │
                    └──────────────┘
```

## Command Execution Flow

```
Input: "service status example-service"
   │
   ▼
┌─────────────────────────────────────┐
│ Parser.parse()                      │
│ - Tokenize: ["service", "status", …]
│ - Match command: "service status"   │
│ - Extract args: ["example-service"] │
└────────────┬────────────────────────┘
             │
             ▼
┌─────────────────────────────────────┐
│ Registry.lookup("service status")   │
│ - Find handler: ServiceCommand      │
│ - Validate args count and types     │
│ - Return command handler            │
└────────────┬────────────────────────┘
             │
             ▼
┌─────────────────────────────────────┐
│ ServiceCommand.execute()            │
│ - Build IPC request                 │
│ - Connect to System Core            │
│ - Send request                      │
│ - Parse response                    │
│ - Convert to Output type            │
└────────────┬────────────────────────┘
             │
             ▼
┌─────────────────────────────────────┐
│ Formatter.format(output)            │
│ - Check --format flag (table/json)  │
│ - Render using appropriate formatter│
│ - Return formatted string           │
└────────────┬────────────────────────┘
             │
             ▼
┌─────────────────────────────────────┐
│ Display output                      │
│ Log command to history              │
│ Update session state                │
│ Show next prompt                    │
└─────────────────────────────────────┘
```

## Module Dependencies

```
main.rs
  └─ shell/repl.rs (REPL orchestrator)
      ├─ shell/parser.rs (tokenize/parse)
      ├─ shell/executor.rs (dispatch)
      ├─ commands/registry.rs (lookup)
      ├─ history/mod.rs (save/load)
      └─ session/mod.rs (state)
          │
          ├─ commands/system.rs (help, version, status)
          ├─ commands/service.rs (service commands)
          │   └─ ipc/client.rs (IPC communication)
          ├─ commands/process.rs
          │   └─ ipc/client.rs
          ├─ commands/filesystem.rs
          │   └─ ipc/client.rs
          ├─ commands/application.rs
          │   └─ ipc/client.rs
          └─ commands/network.rs
              └─ ipc/client.rs
                  ├─ ipc/request.rs (build requests)
                  └─ ipc/response.rs (parse responses)
                      └─ aether-core (IPC types)

output/
  ├─ formatter.rs
  ├─ json.rs
  ├─ table.rs
  └─ text.rs

security/
  ├─ policy.rs
  └─ audit.rs
```

## State Management

### Session State Persistence
```
~/.aether/
├── config.json          # User preferences
├── session.json         # Current session state
├── history              # Command history (plaintext)
└── audit.log            # Audit trail (future)
```

### In-Memory State
```
ShellContext {
  output_format: OutputFormat,    // --format flag value
  current_dir: PathBuf,            // current working directory
  exit_code: i32,                  // last command exit code
  last_command: Option<String>,    // for history
  env: HashMap<String, String>,    // shell env vars
}
```

## Error Handling Strategy

```
Command Execution Errors:
  ├─ Parse Error
  │   └─ Show help for command, explain the issue
  ├─ IPC Communication Error
  │   ├─ Service unavailable → cached response or error
  │   └─ Protocol error → internal error
  ├─ Service Error
  │   └─ Structured error from remote service
  └─ Local Error
      └─ File I/O, permissions, etc.

All errors formatted as:
{
  "ok": false,
  "error": {
    "code": "ERROR_CODE",
    "message": "Human-readable message"
  }
}
```

## Async Runtime

Shell uses Tokio for:
- IPC socket I/O
- Concurrent service queries (where beneficial)
- Timeout handling on IPC calls

Main REPL loop remains synchronous for simplicity:
- Read line from stdin
- Parse command
- Execute command (async within)
- Format and display result
- Update history/session state

## Testing Architecture

```
Unit Tests (tests/ directory)
├─ parser_tests.rs
│   └─ Test command parsing edge cases
├─ formatter_tests.rs
│   └─ Test all output formatters
├─ history_tests.rs
│   └─ Test history filtering and storage
├─ commands_tests.rs
│   └─ Test each command with mocks
└─ ipc_tests.rs
    └─ Test IPC client with mocks

Integration Tests
├─ Full workflow tests
├─ Mock IPC server for testing
└─ Multi-command scenarios

System Tests (in infra/)
├─ Real system calls
├─ IPC to actual services
└─ End-to-end workflows
```

## Extensibility Points

### Adding New Commands
1. Create new file in commands/ (e.g., commands/mycommand.rs)
2. Implement Command trait
3. Register in registry.rs
4. Add help text
5. Add tests

### Adding New Output Format
1. Create new formatter in output/ (e.g., output/yaml.rs)
2. Implement OutputFormatter trait
3. Add variant to OutputFormat enum
4. Update formatter dispatcher
5. Add tests

### Adding New Service Integration
1. Define IPC protocol for new service
2. Create new method in ipc/client.rs
3. Create new command module
4. Implement Command trait
5. Register and document

## Performance Considerations

- Lazy-load IPC connections (connect on first command)
- Cache service list for help generation
- Async IPC queries where possible
- Pagination for large result sets
- History limited to 1000 entries
- Session state loaded only at startup

## Security Layers

1. **Transport Security**
   - Unix-domain socket (0600 permissions)
   - Local-only access
   - Request/response size limits

2. **Authentication**
   - Peer credential verification (future)
   - User/group checking (Linux capabilities)

3. **Authorization**
   - Policy checks before privileged operations
   - Inherited from IPC permission model

4. **Audit**
   - Command logging with timestamp
   - Failed command tracking
   - Sensitive data filtering

5. **Input Validation**
   - Command parsing with strict validation
   - Argument type checking
   - Service ID/PID format validation
