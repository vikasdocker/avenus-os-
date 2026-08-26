# Security Policy

Aether OS is security-first because the AI control plane can operate the system. Security
defects are treated as product-critical engineering events.

## Supported Branches

The active development branch and the latest release branch receive security updates.
Additional long-term support branches are created only after a formal support policy is
published for the affected release line.

## Reporting Security Issues

Report security issues privately to the maintainers through the configured project
security channel. Do not disclose exploit details in public issues.

Include:

- Affected component or script
- Reproduction steps
- Expected and observed behavior
- Required privileges
- Data exposure or system-control impact
- Logs or traces with secrets removed

## Security Review Rules

Changes are security-sensitive when they affect:

- Boot
- Privilege boundaries
- AI permissions
- Service startup
- IPC or network listeners
- Plugins
- Update or ISO creation
- Logging, audit, telemetry, memory, or credential handling

Security-sensitive changes require explicit review from an owner of the affected area.

