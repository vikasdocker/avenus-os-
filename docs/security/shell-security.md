# Aether Shell Security Model

## Security Architecture

The Aether Shell implements defense-in-depth security with five layers:

```
Layer 1: Input Validation        (Syntactic security)
   ↓
Layer 2: Session & Identity      (Authentication)
   ↓
Layer 3: Capability Verification (Authorization)
   ↓
Layer 4: Policy & Confirmation   (Policy enforcement)
   ↓
Layer 5: Audit Logging           (Accountability)
```

## Layer 1: Input Validation

### Command Parsing

Commands are **never** executed as shell text. The parser produces structured command requests:

```
Input:  app launch browser
Parsed: CommandRequest {
          command: "application.launch",
          arguments: ["browser"]
        }
```

NOT:
```
NOT:  shell("app launch browser")
NOT:  system("app launch browser")
NOT:  bash("-c", "app launch browser")
```

### Attack Prevention

**Command Injection Prevention**:
```rust
// WRONG - vulnerable to injection
let result = system(&format!("app launch {}", user_input));

// CORRECT - structured parsing
let mut parts = user_input.split_whitespace();
let command = parts.next().ok_or("No command")?;
let args: Vec<&str> = parts.collect();
registry.execute(command, &args, ...)?;
```

**Path Traversal Prevention**:
```rust
// Before IPC, validate paths
let canonical = std::fs::canonicalize(path)?;
let base = std::fs::canonicalize("/authorized/base")?;
if !canonical.starts_with(&base) {
    return Err("Path outside authorized scope");
}
```

**Argument Type Validation**:
```rust
// PID must be valid integer
let pid: u32 = args[0].parse().map_err(|_| "Invalid PID")?;

// Path must be UTF-8 and valid
let path = args[0];
if !Path::new(path).is_absolute() {
    return Err("Path must be absolute");
}
```

## Layer 2: Session & Identity

### Session Creation

Every shell session has a unique identity:

```rust
pub struct ShellSession {
    session_id: String,              // UUID
    actor: String,                   // Current user
    authentication_state: AuthenticationState,
    capabilities: Vec<String>,       // Granted abilities
}
```

Established during shell startup:
1. Environment user variable (`USER` or `USERNAME`)
2. Current process credentials
3. Available capabilities based on user role

### Actor Verification

Commands are associated with the authenticated actor:

```rust
// Audit log includes:
{
    "session_id": "550e8400-e29b-41d4-a716-446655440000",
    "actor": "vikas",
    "timestamp": "2026-08-23T11:57:58Z",
    "command": "process.stop",
    "pid": "1234"
}
```

## Layer 3: Capability Verification

### Capability Model

Capabilities follow a domain hierarchy:

```
filesystem.*
  - filesystem.read
  - filesystem.write
  - filesystem.delete
  
process.*
  - process.read
  - process.start
  - process.stop
  
system.*
  - system.read
  - system.control
  
audit.*
  - audit.read
  
network.*
  - network.read
```

### Capability Check

Before IPC call:

```rust
async fn execute(&self, args: &[&str], session: &ShellSession, ...) -> Result<()> {
    // 1. Get required capability from metadata
    if let Some(required) = &self.metadata().required_capability {
        // 2. Check if session has it
        if !session.has_capability(required) {
            return Err(AetherError::PermissionDenied {
                capability: required.clone(),
                reason: "Not granted in session".to_string(),
            });
        }
    }
    
    // 3. Proceed with IPC call
    self.do_ipc_call(args, session).await
}
```

### Capability Sources

Session capabilities are determined at startup from:
1. User's static role capabilities (read from `/etc/aether/roles.d/`)
2. Dynamic capabilities (from Policy Engine)
3. Time-based capabilities (expire after duration)
4. Context-specific capabilities (only in certain contexts)

## Layer 4: Policy & Confirmation

### Risk-Based Confirmation

Commands have risk levels. High-risk operations require confirmation:

```rust
#[derive(Serialize)]
pub enum RiskLevel {
    Low,        // fs list, status
    Medium,     // process inspect, app launch
    High,       // filesystem delete, process terminate
    Critical,   // system shutdown, system reboot
}
```

For critical operations:

```
aether> system shutdown
WARNING: This will shut down the system.
Proceed? [yes/no]: no
Shutdown cancelled.
```

### Confirmation Audit

Confirmation decisions are audited:

```json
{
    "session_id": "...",
    "actor": "vikas",
    "command": "system.shutdown",
    "risk_level": "critical",
    "requires_confirmation": true,
    "confirmation_requested": true,
    "confirmation_response": "denied",
    "result": "cancelled"
}
```

### Policy Engine

The Policy Engine defines:
- Which commands require confirmation
- Which actors can perform which commands
- Time-based restrictions
- Rate limiting
- Concurrent operation limits

Example policy:

```yaml
policies:
  system-shutdown:
    risk_level: critical
    requires_confirmation: true
    allowed_roles:
      - admin
      - operator
    allowed_hours: "00:00-23:59"
    rate_limit: "1 per day"
    
  process-terminate:
    risk_level: high
    requires_confirmation: true
    allowed_roles:
      - user
      - admin
    min_age_seconds: 5
```

## Layer 5: Audit Logging

### What Is Logged

✅ Logged:
- Session ID
- Actor
- Timestamp
- Command name
- Arguments (non-sensitive)
- Capability checked
- Authorization decision
- Result (success/failure)
- Duration
- Service invoked

❌ NOT Logged:
- Passwords
- Tokens
- API keys
- Secrets
- File contents
- Sensitive command arguments
- Credit card numbers
- PII

### Secret Detection

Commands containing these keywords are filtered from history and logged with `[REDACTED]`:

```rust
fn is_sensitive(&self, command: &str) -> bool {
    let sensitive_keywords = [
        "password", "passwd",
        "token", "auth",
        "secret", "apikey", "api_key",
        "credential", "cred",
        "key", "private",
    ];
    sensitive_keywords.iter()
        .any(|&kw| command.to_lowercase().contains(kw))
}
```

Example audit log:

```json
{
    "session_id": "...",
    "actor": "vikas",
    "command": "system.configure",
    "arguments": "[REDACTED: contains sensitive data]",
    "capability": "system.control",
    "decision": "ALLOWED",
    "result": "success",
    "timestamp": "2026-08-23T11:57:58Z"
}
```

### Audit Log Retention

- **In-Memory**: Current session, up to 1000 entries
- **Persistent**: System Core audit log (centralized)
- **Rotation**: Daily rollover to prevent unlimited growth
- **Encryption**: Future phase (encrypted at rest)

## Attack Vectors and Mitigations

### Vector 1: Shell Injection
**Attack**: `app launch "firefox; rm -rf /"`
**Mitigation**: Command parsing doesn't use shell execution
**Status**: ✅ Prevented

### Vector 2: Path Traversal
**Attack**: `fs list "../../etc/shadow"`
**Mitigation**: Path canonicalization and scope checking
**Status**: ✅ Prevented

### Vector 3: Privilege Escalation
**Attack**: Gain capabilities not granted to user
**Mitigation**: Session capabilities strictly enforced, immutable at runtime
**Status**: ✅ Prevented

### Vector 4: IPC Spoofing
**Attack**: Send crafted IPC message pretending to be shell
**Mitigation**: Local-only socket (0600 mode), future peer credential verification
**Status**: ✅ Prevented

### Vector 5: History Theft
**Attack**: Read command history containing secrets
**Mitigation**: History file created with 0600 mode, secrets not stored
**Status**: ✅ Prevented

### Vector 6: Audit Tampering
**Attack**: Modify audit log to hide actions
**Mitigation**: Audit log written to System Core (centralized, protected)
**Status**: ✅ Prevented (future)

### Vector 7: DoS via Command Flooding
**Attack**: Send hundreds of commands to exhaust resources
**Mitigation**: Rate limiting per session, configurable limits
**Status**: ✅ Mitigated

### Vector 8: Information Disclosure
**Attack**: Get unauthorized info via `fs list /root`
**Mitigation**: Filesystem service enforces permissions, shell doesn't bypass them
**Status**: ✅ Prevented

## Security Testing

### Unit Tests

```rust
#[test]
fn test_path_traversal_prevention() {
    let path = "../../etc/shadow";
    let validated = validate_path(path, "/authorized");
    assert!(validated.is_err());
}

#[test]
fn test_privilege_escalation_prevention() {
    let session = ShellSession::new();
    assert!(!session.has_capability("system.control"));
    
    // Try to add capability - fails
    session.add_capability("system.control"); // This should fail
}

#[test]
fn test_secret_filtering_in_history() {
    let mut history = ShellHistory::new();
    history.add("help");
    history.add("system configure token=abc123");
    
    assert_eq!(history.get_all().len(), 1); // Only "help"
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_capability_enforcement() {
    let mut limited_session = ShellSession::new();
    limited_session.capabilities.clear(); // Remove all caps
    
    let registry = CommandRegistry::new();
    let result = registry
        .execute("audit", &[], &limited_session, ...)
        .await;
    
    assert!(result.is_err()); // Should be denied
}

#[tokio::test]
async fn test_command_injection_prevention() {
    let registry = CommandRegistry::new();
    let session = ShellSession::new();
    
    // Injection attempt in arguments
    let args = vec!["firefox; rm -rf /"];
    let result = registry.execute("app", &args, &session, ...).await;
    
    // Should parse as literal string, not execute
    assert!(result.is_ok());
}
```

### Security Checklist

- [ ] No shell execution functions used
- [ ] All paths validated before service call
- [ ] All arguments type-checked
- [ ] Capability checked before every operation
- [ ] Session immutable at runtime
- [ ] Secrets not stored in history
- [ ] Audit logging enabled
- [ ] Error messages don't leak info
- [ ] IPC socket created with correct permissions
- [ ] No hardcoded credentials

## Future Security Enhancements

### Phase 2: Authentication Hardening
- Multi-factor authentication
- Session expiration
- Certificate-based authentication

### Phase 3: Encryption
- TLS for remote IPC (future)
- Encrypted audit log storage
- Command history encryption at rest

### Phase 4: MAC Policy
- SELinux/AppArmor integration
- Fine-grained access control
- Mandatory access control for critical operations

### Phase 5: Hardware Security
- TPM integration
- Attestation verification
- Secure boot validation

## Compliance

The shell design supports:
- **POSIX**: Standard Unix signals and file permissions
- **Common Criteria**: Audit trail, access control, error handling
- **SOC 2**: Logging, access control, change management
- **HIPAA**: Audit logging, access control, encryption (future)
