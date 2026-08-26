# Phase 1.8: Security & Audit Guide

## Security Architecture

### Defense in Depth Layers

```
┌──────────────────────────────────────┐
│  Layer 1: Input Validation           │
│  - Command parsing                   │
│  - Argument validation               │
│  - Type checking                     │
└──────────────────────────────────────┘
                  ▼
┌──────────────────────────────────────┐
│  Layer 2: Authentication & Auth      │
│  - Unix socket peer credentials      │
│  - User/Group verification           │
│  - Policy checks                     │
└──────────────────────────────────────┘
                  ▼
┌──────────────────────────────────────┐
│  Layer 3: Transport Security         │
│  - Unix domain sockets (0600)        │
│  - Request/response size limits      │
│  - Timeout protection                │
└──────────────────────────────────────┘
                  ▼
┌──────────────────────────────────────┐
│  Layer 4: Output Sanitization        │
│  - JSON escaping                     │
│  - Secret filtering                  │
│  - Path traversal prevention         │
└──────────────────────────────────────┘
                  ▼
┌──────────────────────────────────────┐
│  Layer 5: Audit & Logging            │
│  - Command logging                   │
│  - Event tracking                    │
│  - Forensics support                 │
└──────────────────────────────────────┘
```

## Input Validation

### Command Parsing Security

All input must be validated:
- Maximum command length: 256 characters
- Maximum arguments: 50 per command
- Maximum argument length: 4KB per argument
- Flag pattern: ^--?[a-z0-9-]+$

Type-specific validators:
- Service IDs: aether-[a-z0-9-]+
- Process IDs: positive integers only
- File paths: no ".." sequences, no null bytes
- URLs: proper format validation

## Authentication & Authorization

### Unix Socket Security

- Socket path: /run/aether/ipc/aether-system-core.sock
- Permissions: 0600 (read/write owner only)
- Socket owner: root or running user
- Peer credential verification: UID, GID, PID

### Policy Enforcement

Operations requiring elevated privileges:
- system:shutdown (requires root)
- system:reboot (requires root)
- process:start (depends on policy)
- service:restart (depends on policy)

## Transport Security

### IPC Limits

- Maximum request size: 8KB
- Maximum response size: 1MB
- IPC timeout: 30 seconds
- Concurrent connections: limited

### DoS Protection

- Request size limits enforced
- Response size limits enforced
- Timeout protection on all IPC calls
- Rate limiting (future enhancement)

## Output Sanitization

### Secret Filtering

Commands with secrets are NOT stored in history:
- --password flag
- --token flag
- --secret flag
- --api-key flag
- --api_key flag (variant)

When secrets are detected in command history, they are redacted.

### JSON Output Security

- All JSON output properly escaped
- Unicode characters handled correctly
- No information leakage in error messages
- Path information sanitized

## Audit & Logging

### Audit Trail

All sensitive operations are logged:
- Timestamp (ISO 8601)
- User UID/GID
- Command executed
- Success/failure status
- Error details (if failed)
- Duration in milliseconds

### Forensics Support

Query audit logs by:
- User ID
- Time range
- Operation type
- Status (success/failure)

## Security Checklist

### Before Release

- [ ] All user input validated
- [ ] All paths checked for traversal attacks
- [ ] Unix socket permissions verified (0600)
- [ ] Peer credentials checked for privileged ops
- [ ] Policy checks enabled for all sensitive ops
- [ ] Secret patterns in history filtering
- [ ] JSON output properly escaped
- [ ] Audit logging implemented
- [ ] Timeout protection on IPC
- [ ] Error messages don't leak system info
- [ ] No unsafe code except where documented
- [ ] No hardcoded credentials
- [ ] No debug info in release builds
- [ ] Security review completed

## Common Vulnerabilities to Prevent

1. **Path Traversal**: Validate all paths, reject ".."
2. **Command Injection**: Never pass user input to shell
3. **Information Disclosure**: Don't leak system paths in errors
4. **Privilege Escalation**: Always check policy before ops
5. **DoS**: Enforce size/timeout limits
6. **Man-in-the-Middle**: Use Unix sockets + peer creds
7. **Secret Leakage**: Filter secrets from history
8. **Audit Bypass**: Log all sensitive operations

## OWASP Top 10 Mitigation

| OWASP | Mitigation in aethersh |
|-------|------------------------|
| A1: Injection | Input validation, no shell spawning |
| A2: Auth | Peer credentials, policy checks |
| A3: Sensitive Data | Secret filtering, audit logging |
| A4: XML/XXE | No XML parsing |
| A5: Broken Access | Policy-based authz |
| A6: Config | Secure defaults, 0600 files |
| A7: Logging | Comprehensive audit trail |
| A8: CSRF | IPC not web-based |
| A9: Using Components | Dependency audit |
| A10: API Limits | Size/timeout limits |

## Incident Response

If a security issue is found:

1. Stop the shell (kill process)
2. Preserve audit logs and history
3. Analyze events leading up to issue
4. Document timeline and impact
5. Report to security team

## Key Security Features

- Unix socket peer credential verification
- Policy-based authorization
- Comprehensive input validation
- Secret filtering in history files
- JSON output escaping
- Timeout protection
- Audit logging for all operations
- Secure defaults (0600 permissions)
- No hardcoded credentials
- No unsafe code (except documented FFI)

## Security Testing

Include security tests for:
- Invalid command formats
- Path traversal attempts
- Privilege escalation attempts
- Timeout and size limit handling
- JSON injection prevention
- Secret filtering validation
- Audit logging completeness

## Future Enhancements

- Rate limiting on IPC
- Command signature verification
- Encrypted audit logs
- Audit log rotation
- Policy versioning
- Enhanced credential verification
- Multi-factor authentication support
