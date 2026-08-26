# Aether Shell: Command Reference & API

## Global Flags

All commands support these global flags:
```
-h, --help              Show help for command
-v, --verbose           Verbose output
--format <FORMAT>       Output format: table (default), json, text
--timeout <SECS>        Timeout for IPC calls (default: 30s)
--no-cache              Bypass service cache
```

## System Commands

### help
Display help information.

**Usage:**
```
help                    # Show general help
help <COMMAND>         # Show help for specific command
help --all             # Show all commands
```

**Output (JSON format):**
```json
{
  "ok": true,
  "commands": [
    {
      "name": "service status",
      "description": "Get service status",
      "usage": "service status <SERVICE_ID>",
      "category": "service"
    }
  ]
}
```

### version
Display shell and system version information.

**Usage:**
```
version
version --all
```

**Output:**
```
Aether Shell v1.8.0
Aether OS v0.1.0
Built: 2024-01-15 14:30:00 UTC
Commit: abc1234def567890
```

### status
Display overall system status.

**Usage:**
```
status
status --detailed
```

**Output (JSON):**
```json
{
  "ok": true,
  "status": {
    "uptime": 3600,
    "services_total": 12,
    "services_running": 11,
    "services_failed": 1,
    "memory_usage_percent": 45.2,
    "processes_total": 128
  }
}
```

### health
Check health of core system components.

**Usage:**
```
health
health --check <COMPONENT>
```

**Components:** ipc, core, services, filesystem, network, process-manager, application-manager

**Output (JSON):**
```json
{
  "ok": true,
  "health": {
    "ipc": "healthy",
    "core": "healthy",
    "services": "degraded",
    "timestamp": "2024-01-15T14:30:00Z"
  }
}
```

### exit, quit, logout
Exit the shell gracefully.

**Usage:**
```
exit              # Exit with code 0
exit 1            # Exit with specified code
quit
logout
```

---

## Service Commands

### service list
List all available services.

**Usage:**
```
service list
service list --status running
service list --filter network
```

**Flags:**
- `--status <STATUS>`: Filter by status (running, stopped, failed, loading)
- `--filter <PATTERN>`: Filter by name pattern

**Output (JSON):**
```json
{
  "ok": true,
  "services": [
    {
      "id": "aether-system-core",
      "name": "System Core",
      "status": "running",
      "uptime_seconds": 3600,
      "version": "0.1.0"
    }
  ]
}
```

### service status
Get detailed status of a service.

**Usage:**
```
service status <SERVICE_ID>
service status aether-system-core
```

**Output (JSON):**
```json
{
  "ok": true,
  "service": {
    "id": "aether-system-core",
    "status": "running",
    "pid": 1234,
    "memory_mb": 45,
    "cpu_usage_percent": 2.5,
    "restarts": 0,
    "uptime_seconds": 3600,
    "last_error": null
  }
}
```

### service restart
Restart a service.

**Usage:**
```
service restart <SERVICE_ID>
service restart aether-system-core --timeout 60
```

**Flags:**
- `--timeout <SECS>`: Timeout for restart operation

**Output (JSON):**
```json
{
  "ok": true,
  "message": "Service restarted successfully"
}
```

### service logs
Get service logs.

**Usage:**
```
service logs <SERVICE_ID>
service logs aether-system-core --lines 50
service logs aether-system-core --since 1h
```

**Flags:**
- `--lines <N>`: Number of log lines (default: 20)
- `--since <DURATION>`: Show logs since (1h, 30m, etc.)
- `--level <LEVEL>`: Filter by log level (info, warn, error)

---

## Process Commands

### process list
List all processes.

**Usage:**
```
process list
process list --filter ssh
process list --sort cpu
```

**Flags:**
- `--filter <PATTERN>`: Filter by name
- `--sort <FIELD>`: Sort by field (pid, cpu, memory, name)

**Output (JSON):**
```json
{
  "ok": true,
  "processes": [
    {
      "pid": 1234,
      "name": "sshd",
      "status": "running",
      "cpu_percent": 0.5,
      "memory_mb": 12,
      "user": "root"
    }
  ]
}
```

### process inspect
Get detailed info about a process.

**Usage:**
```
process inspect <PID>
process inspect 1234
```

**Output (JSON):**
```json
{
  "ok": true,
  "process": {
    "pid": 1234,
    "name": "sshd",
    "status": "running",
    "parent_pid": 1,
    "user": "root",
    "group": "root",
    "memory_mb": 12,
    "cpu_percent": 0.5,
    "start_time": "2024-01-15T10:00:00Z",
    "command": "/usr/sbin/sshd -D"
  }
}
```

### process start
Start a new process.

**Usage:**
```
process start <COMMAND>
process start /bin/bash --args -c "sleep 100"
process start myapp --cwd /tmp
```

**Flags:**
- `--args <ARGS>`: Command arguments
- `--cwd <PATH>`: Working directory
- `--env <KEY=VALUE>`: Environment variables (repeatable)

**Output (JSON):**
```json
{
  "ok": true,
  "process": {
    "pid": 5678,
    "name": "bash"
  }
}
```

### process stop
Stop a process.

**Usage:**
```
process stop <PID>
process stop 1234 --signal SIGTERM
```

**Flags:**
- `--signal <SIGNAL>`: Signal to send (SIGTERM, SIGKILL, etc., default: SIGTERM)

### process restart
Restart a process (stop then start).

**Usage:**
```
process restart <PID>
```

---

## Filesystem Commands

### fs list
List directory contents.

**Usage:**
```
fs list
fs list /path/to/dir
fs list --long
fs list --sort size
```

**Flags:**
- `--long`: Long format with details
- `--sort <FIELD>`: Sort by field (name, size, modified, type)
- `--all`: Show hidden files

**Output (Table):**
```
Name              Type   Size      Modified
config.json       file   2.1K      2024-01-15 14:30
data              dir    4.0K      2024-01-15 13:00
app.log           file   156K      2024-01-15 12:45
```

**Output (JSON):**
```json
{
  "ok": true,
  "entries": [
    {
      "name": "config.json",
      "type": "file",
      "size": 2048,
      "permissions": "644",
      "modified": "2024-01-15T14:30:00Z"
    }
  ]
}
```

### fs stat
Get detailed file/directory stats.

**Usage:**
```
fs stat /path/to/file
fs stat --recursive
```

**Output (JSON):**
```json
{
  "ok": true,
  "stat": {
    "path": "/etc/config.json",
    "type": "file",
    "size": 2048,
    "permissions": "644",
    "owner": "root",
    "group": "root",
    "created": "2024-01-15T10:00:00Z",
    "modified": "2024-01-15T14:30:00Z",
    "accessed": "2024-01-15T14:31:00Z"
  }
}
```

### fs search
Search for files.

**Usage:**
```
fs search <PATTERN>
fs search "*.log" --type file
fs search <PATTERN> --path /var/log
```

**Flags:**
- `--type <TYPE>`: Filter by type (file, dir, symlink)
- `--path <PATH>`: Start search path
- `--max-depth <N>`: Maximum directory depth

### fs storage
Get storage usage statistics.

**Usage:**
```
fs storage
fs storage /path/to/mount
fs storage --format human
```

**Output (JSON):**
```json
{
  "ok": true,
  "mounts": [
    {
      "path": "/",
      "filesystem": "/dev/sda1",
      "total_mb": 102400,
      "used_mb": 51200,
      "available_mb": 51200,
      "usage_percent": 50
    }
  ]
}
```

### fs mounts
List mounted filesystems.

**Usage:**
```
fs mounts
fs mounts --filter /dev
```

---

## Application Commands

### app list
List installed applications.

**Usage:**
```
app list
app list --running
app list --filter web
```

**Flags:**
- `--running`: Show only running apps
- `--filter <PATTERN>`: Filter by name

**Output (JSON):**
```json
{
  "ok": true,
  "applications": [
    {
      "id": "webserver-01",
      "name": "Web Server",
      "version": "1.0.0",
      "status": "running",
      "category": "service"
    }
  ]
}
```

### app inspect
Get detailed app information.

**Usage:**
```
app inspect <APP_ID>
```

**Output (JSON):**
```json
{
  "ok": true,
  "application": {
    "id": "webserver-01",
    "name": "Web Server",
    "description": "Main web server",
    "version": "1.0.0",
    "status": "running",
    "process_id": 1234,
    "memory_mb": 128,
    "cpu_percent": 5.2
  }
}
```

### app launch
Launch an application.

**Usage:**
```
app launch <APP_ID>
app launch webserver-01 --args key=value
```

**Flags:**
- `--args <ARGS>`: Application arguments

### app close
Close a running application.

**Usage:**
```
app close <APP_ID>
app close webserver-01 --force
```

---

## Network Commands

### network status
Get overall network status.

**Usage:**
```
network status
```

**Output (JSON):**
```json
{
  "ok": true,
  "status": {
    "interfaces_count": 3,
    "active_connections": 12,
    "dns_servers": ["8.8.8.8", "8.8.4.4"],
    "default_gateway": "192.168.1.1"
  }
}
```

### network interfaces
List network interfaces.

**Usage:**
```
network interfaces
network interfaces --format detailed
```

**Output (Table):**
```
Interface  Status  IP Address      Netmask         MAC
eth0       up      192.168.1.100   255.255.255.0   00:11:22:33:44:55
lo         up      127.0.0.1       255.0.0.0       00:00:00:00:00:00
```

### network inspect
Get detailed interface information.

**Usage:**
```
network inspect <INTERFACE>
network inspect eth0
```

### network addresses
Show network addresses and routes.

**Usage:**
```
network addresses
network addresses --ipv6
```

### network routes
Display routing table.

**Usage:**
```
network routes
network routes --default-only
```

### network dns
Show DNS configuration.

**Usage:**
```
network dns
network dns --set 8.8.8.8 8.8.4.4
```

### network connectivity
Test network connectivity.

**Usage:**
```
network connectivity
network connectivity --host google.com
```

### network stats
Display network statistics.

**Usage:**
```
network stats
network stats --interface eth0
```

---

## System Control Commands

### system shutdown
Shutdown the system.

**Usage:**
```
system shutdown
system shutdown --delay 300
system shutdown --reason "Maintenance window"
```

**Flags:**
- `--delay <SECS>`: Delay before shutdown
- `--reason <TEXT>`: Reason for shutdown
- `--force`: Force immediate shutdown

**Requires:** shutdown policy permission

### system reboot
Reboot the system.

**Usage:**
```
system reboot
system reboot --delay 300
```

**Flags:**
- `--delay <SECS>`: Delay before reboot

**Requires:** reboot policy permission

---

## Error Response Format

All errors follow this structure:

**JSON Format:**
```json
{
  "ok": false,
  "error": {
    "code": "SERVICE_UNAVAILABLE",
    "message": "Service 'aether-filesystem' is not responding",
    "details": {
      "service": "aether-filesystem",
      "timeout": 30
    }
  }
}
```

**Text Format:**
```
Error: SERVICE_UNAVAILABLE
Service 'aether-filesystem' is not responding
```

---

## Common Error Codes

- `INVALID_COMMAND`: Command not recognized
- `INVALID_ARGS`: Invalid arguments for command
- `PARSE_ERROR`: Could not parse command
- `SERVICE_UNAVAILABLE`: Required service not responding
- `PERMISSION_DENIED`: User lacks permission
- `TIMEOUT`: IPC call timed out
- `INTERNAL_ERROR`: Unexpected system error
- `NOT_FOUND`: Resource not found (process, service, file, etc.)
- `ALREADY_EXISTS`: Resource already exists
- `INVALID_STATE`: Invalid operation for current state
