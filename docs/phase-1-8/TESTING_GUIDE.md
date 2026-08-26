# Phase 1.8: Testing Strategy & Examples

## Testing Architecture

### Test Pyramid
```
         /\
        /  \  E2E Tests (5-10%)
       /____\

        /  \
       /    \  Integration Tests (25-35%)
      /______\

       /    \
      /      \  Unit Tests (55-65%)
     /________\
```

### Test Organization
```
src/
  lib.rs                  # Public API exports
  module.rs               # Implementation

tests/
  common/                 # Shared test utilities
    mod.rs
    test_helpers.rs       # Helper functions
    mock_ipc.rs          # Mock IPC server
    fixtures.rs          # Test data

  unit/                   # Unit tests
    test_parser.rs
    test_formatter.rs
    test_history.rs

  integration/            # Integration tests
    test_command_flows.rs
    test_ipc_client.rs
    test_output_formats.rs

benches/
  benchmark_parser.rs
  benchmark_ipc.rs
```

---

## Unit Testing Guide

### Parser Tests

```rust
// tests/unit/test_parser.rs
#[cfg(test)]
mod parser_tests {
    use aether_shell::shell::parser::{Parser, ParseError};
    
    #[test]
    fn test_parse_empty_input() {
        let err = Parser::parse("").unwrap_err();
        assert_eq!(err, ParseError::EmptyInput);
    }
    
    #[test]
    fn test_parse_simple_command() {
        let result = Parser::parse("help").unwrap();
        assert_eq!(result.command, "help");
        assert!(result.args.is_empty());
        assert!(result.flags.is_empty());
    }
    
    #[test]
    fn test_parse_command_with_args() {
        let result = Parser::parse("service status myservice").unwrap();
        assert_eq!(result.command, "service");
        assert_eq!(result.args[0], "status");
        assert_eq!(result.args[1], "myservice");
    }
    
    #[test]
    fn test_parse_flags() {
        let result = Parser::parse(
            "service list --status running --filter ssh"
        ).unwrap();
        
        assert_eq!(result.flags.get("status"), Some(&"running".to_string()));
        assert_eq!(result.flags.get("filter"), Some(&"ssh".to_string()));
    }
    
    #[test]
    fn test_parse_boolean_flags() {
        let result = Parser::parse("process list --detailed --no-header").unwrap();
        assert!(result.flags.contains_key("detailed"));
        assert!(result.flags.contains_key("no-header"));
    }
    
    #[test]
    fn test_parse_quoted_strings() {
        let result = Parser::parse(
            r#"process start "my app" --args "-c \"test\"" "#
        ).unwrap();
        
        assert_eq!(result.args[0], "my app");
    }
    
    #[test]
    fn test_parse_invalid_flag() {
        let err = Parser::parse("help --unknown-flag").unwrap_err();
        assert!(matches!(err, ParseError::UnknownFlag));
    }
    
    #[test]
    fn test_parse_flags_with_equals() {
        let result = Parser::parse("--format=json help").unwrap();
        assert_eq!(result.flags.get("format"), Some(&"json".to_string()));
    }
}
```

### Formatter Tests

```rust
// tests/unit/test_formatter.rs
#[cfg(test)]
mod formatter_tests {
    use aether_shell::output::{Formatter, Output, OutputFormat};
    
    #[test]
    fn test_json_formatter_simple() {
        let output = Output::Text("Hello".to_string());
        let formatter = Formatter::json();
        let result = formatter.format(&output).unwrap();
        
        let json: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(json["ok"], true);
        assert_eq!(json["data"], "Hello");
    }
    
    #[test]
    fn test_table_formatter_with_headers() {
        let data = vec![
            vec!["Name", "Status"],
            vec!["service1", "running"],
            vec!["service2", "stopped"],
        ];
        
        let output = Output::Table {
            headers: vec!["Name".to_string(), "Status".to_string()],
            rows: vec![
                vec!["service1".to_string(), "running".to_string()],
                vec!["service2".to_string(), "stopped".to_string()],
            ],
        };
        
        let formatter = Formatter::table();
        let result = formatter.format(&output).unwrap();
        
        assert!(result.contains("Name"));
        assert!(result.contains("service1"));
        assert!(result.contains("running"));
    }
    
    #[test]
    fn test_error_formatting() {
        let output = Output::Error {
            code: "SERVICE_UNAVAILABLE".to_string(),
            message: "Service not responding".to_string(),
        };
        
        let json_result = Formatter::json().format(&output).unwrap();
        let json: serde_json::Value = serde_json::from_str(&json_result).unwrap();
        
        assert_eq!(json["ok"], false);
        assert_eq!(json["error"]["code"], "SERVICE_UNAVAILABLE");
    }
    
    #[test]
    fn test_json_special_characters() {
        let output = Output::Text(r#"{"key": "value"}"#.to_string());
        let result = Formatter::json().format(&output).unwrap();
        
        // Should properly escape JSON
        let json: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(json["data"], r#"{"key": "value"}"#);
    }
}
```

### History Tests

```rust
// tests/unit/test_history.rs
#[cfg(test)]
mod history_tests {
    use aether_shell::history::HistoryManager;
    use std::io::Write;
    use tempfile::NamedTempFile;
    
    #[test]
    fn test_save_and_load_history() {
        let mut file = NamedTempFile::new().unwrap();
        let mut history = HistoryManager::new(file.path());
        
        history.add("service list");
        history.add("process inspect 1234");
        
        let entries = history.load().unwrap();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0], "service list");
    }
    
    #[test]
    fn test_history_filters_secrets() {
        let mut history = HistoryManager::new(Path::new("/tmp/history"));
        
        // Command with --password should be filtered
        assert!(history.should_filter("service auth --password secret123"));
        assert!(!history.should_filter("service list"));
    }
    
    #[test]
    fn test_history_rotation() {
        let mut history = HistoryManager::new(Path::new("/tmp/history"));
        history.set_max_lines(5);
        
        for i in 0..10 {
            history.add(&format!("command {}", i));
        }
        
        let entries = history.load().unwrap();
        assert_eq!(entries.len(), 5); // Only last 5
    }
    
    #[test]
    fn test_history_search() {
        let mut history = HistoryManager::new(Path::new("/tmp/history"));
        history.add("service list");
        history.add("service status myservice");
        history.add("process list");
        
        let results = history.search("service").unwrap();
        assert_eq!(results.len(), 2);
    }
}
```

---

## Integration Testing Guide

### IPC Client Tests

```rust
// tests/integration/test_ipc_client.rs
#[cfg(test)]
mod ipc_integration_tests {
    use aether_shell::ipc::IpcClient;
    use aether_shell::tests::common::mock_ipc::MockIpcServer;
    use tokio::sync::Arc;
    
    #[tokio::test]
    async fn test_ipc_send_request() {
        let mock = MockIpcServer::new();
        let client = IpcClient::new(mock.socket_path()).await.unwrap();
        
        mock.add_response(
            "service_list",
            json!({
                "ok": true,
                "services": [
                    {"id": "service1", "status": "running"}
                ]
            })
        );
        
        let response = client.send_request(
            IpcRequest::ServiceList {}
        ).await.unwrap();
        
        assert!(response.ok);
    }
    
    #[tokio::test]
    async fn test_ipc_timeout() {
        let mock = MockIpcServer::new();
        mock.set_delay(Duration::from_secs(10));
        
        let mut client = IpcClient::new(mock.socket_path()).await.unwrap();
        client.set_timeout(Duration::from_millis(100));
        
        let err = client.send_request(
            IpcRequest::ServiceList {}
        ).await.unwrap_err();
        
        assert!(matches!(err, IpcError::Timeout));
    }
    
    #[tokio::test]
    async fn test_ipc_error_response() {
        let mock = MockIpcServer::new();
        let client = IpcClient::new(mock.socket_path()).await.unwrap();
        
        mock.add_response(
            "service_status",
            json!({
                "ok": false,
                "error": {
                    "code": "NOT_FOUND",
                    "message": "Service not found"
                }
            })
        );
        
        let err = client.send_request(
            IpcRequest::ServiceStatus { id: "unknown".into() }
        ).await.unwrap_err();
        
        assert!(matches!(err, IpcError::NotFound));
    }
}
```

### Command Integration Tests

```rust
// tests/integration/test_command_flows.rs
#[cfg(test)]
mod command_integration_tests {
    use aether_shell::shell::Shell;
    use aether_shell::tests::common::mock_ipc::MockIpcServer;
    
    #[tokio::test]
    async fn test_help_command() {
        let shell = Shell::new().await.unwrap();
        let output = shell.execute("help").await.unwrap();
        
        // help is local-only, should work without IPC
        assert!(output.contains("service"));
        assert!(output.contains("process"));
    }
    
    #[tokio::test]
    async fn test_service_list_command() {
        let mock = MockIpcServer::new();
        let shell = Shell::with_ipc(mock.client()).await.unwrap();
        
        mock.add_service("service1", "running");
        mock.add_service("service2", "stopped");
        
        let output = shell.execute("service list").await.unwrap();
        
        assert!(output.contains("service1"));
        assert!(output.contains("running"));
    }
    
    #[tokio::test]
    async fn test_multiple_commands_in_sequence() {
        let mock = MockIpcServer::new();
        let shell = Shell::with_ipc(mock.client()).await.unwrap();
        
        // Command 1
        let _ = shell.execute("version").await.unwrap();
        
        // Command 2
        let output = shell.execute("service list").await.unwrap();
        assert!(output.is_ok());
        
        // Command 3
        let _ = shell.execute("help").await.unwrap();
    }
    
    #[tokio::test]
    async fn test_output_format_json() {
        let shell = Shell::new().await.unwrap();
        
        let output = shell.execute(
            "version --format json"
        ).await.unwrap();
        
        let json: serde_json::Value = serde_json::from_str(&output).unwrap();
        assert_eq!(json["ok"], true);
    }
    
    #[tokio::test]
    async fn test_invalid_command_error() {
        let shell = Shell::new().await.unwrap();
        
        let err = shell.execute("invalid_command").await.unwrap_err();
        assert_eq!(err.code, "INVALID_COMMAND");
    }
}
```

---

## Mock IPC Server

```rust
// tests/common/mock_ipc.rs
pub struct MockIpcServer {
    responses: Arc<Mutex<HashMap<String, serde_json::Value>>>,
    delay: Arc<Mutex<Duration>>,
    socket_path: PathBuf,
}

impl MockIpcServer {
    pub fn new() -> Self {
        let socket_path = PathBuf::from("/tmp/mock_ipc.sock");
        Self {
            responses: Arc::new(Mutex::new(HashMap::new())),
            delay: Arc::new(Mutex::new(Duration::from_millis(0))),
            socket_path,
        }
    }
    
    pub fn add_response(&self, command: &str, response: serde_json::Value) {
        self.responses.blocking_lock().insert(
            command.to_string(),
            response
        );
    }
    
    pub fn add_service(&self, id: &str, status: &str) {
        let mut responses = self.responses.blocking_lock();
        let services = responses
            .entry("service_list".to_string())
            .or_insert_with(|| json!({"ok": true, "services": []}));
        
        if let Some(svc_list) = services.get_mut("services").and_then(|v| v.as_array_mut()) {
            svc_list.push(json!({
                "id": id,
                "status": status
            }));
        }
    }
    
    pub fn set_delay(&self, delay: Duration) {
        *self.delay.blocking_lock() = delay;
    }
    
    pub fn socket_path(&self) -> &PathBuf {
        &self.socket_path
    }
    
    pub async fn client(&self) -> IpcClient {
        IpcClient::new(self.socket_path.clone()).await.unwrap()
    }
}
```

---

## Test Data Fixtures

```rust
// tests/common/fixtures.rs
pub fn sample_service_response() -> serde_json::Value {
    json!({
        "ok": true,
        "services": [
            {
                "id": "aether-system-core",
                "name": "System Core",
                "status": "running",
                "uptime_seconds": 3600,
                "memory_mb": 45
            },
            {
                "id": "aether-filesystem",
                "name": "Filesystem Service",
                "status": "running",
                "uptime_seconds": 3500,
                "memory_mb": 28
            }
        ]
    })
}

pub fn sample_process_list() -> serde_json::Value {
    json!({
        "ok": true,
        "processes": [
            {
                "pid": 1234,
                "name": "sshd",
                "status": "running",
                "cpu_percent": 0.5,
                "memory_mb": 12,
                "user": "root"
            },
            {
                "pid": 5678,
                "name": "bash",
                "status": "running",
                "cpu_percent": 0.1,
                "memory_mb": 8,
                "user": "user"
            }
        ]
    })
}

pub fn error_response(code: &str, msg: &str) -> serde_json::Value {
    json!({
        "ok": false,
        "error": {
            "code": code,
            "message": msg
        }
    })
}
```

---

## Performance Benchmarks

```rust
// benches/benchmark_parser.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use aether_shell::shell::parser::Parser;

fn benchmark_simple_parse(c: &mut Criterion) {
    c.bench_function("parse_simple", |b| {
        b.iter(|| Parser::parse(black_box("help")))
    });
}

fn benchmark_complex_parse(c: &mut Criterion) {
    c.bench_function("parse_with_many_flags", |b| {
        b.iter(|| {
            Parser::parse(black_box(
                "service list --status running --filter ssh --sort cpu"
            ))
        })
    });
}

criterion_group!(benches, benchmark_simple_parse, benchmark_complex_parse);
criterion_main!(benches);
```

### Running Benchmarks
```bash
cargo bench -p aether-shell
```

---

## Coverage Reports

### Generate Coverage
```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Generate HTML coverage report
cargo tarpaulin -p aether-shell --html --output-dir target/coverage

# Print coverage summary
cargo tarpaulin -p aether-shell --timeout 300
```

### Coverage Targets
- Unit tests: > 85%
- Integration tests: > 70%
- Overall: > 80%

---

## Continuous Integration

### GitHub Actions Workflow
```yaml
# .github/workflows/shell-tests.yml
name: Aether Shell Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    
    steps:
      - uses: actions/checkout@v3
      
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Run tests
        run: cargo test -p aether-shell
      
      - name: Run clippy
        run: cargo clippy -p aether-shell
      
      - name: Check formatting
        run: cargo fmt -p aether-shell -- --check
      
      - name: Generate coverage
        run: cargo tarpaulin -p aether-shell --timeout 300
```

---

## Test Execution

### Run All Tests
```bash
cargo test -p aether-shell
```

### Run Specific Test
```bash
cargo test -p aether-shell parser_tests::test_parse_simple_command
```

### Run Integration Tests Only
```bash
cargo test -p aether-shell --test integration
```

### Run with Logging
```bash
RUST_LOG=debug cargo test -p aether-shell -- --nocapture
```

### Run in Parallel (faster)
```bash
cargo test -p aether-shell -- --test-threads=8
```

---

## Test Guidelines

### What to Test
- ✅ Normal cases
- ✅ Edge cases (empty, very large, etc.)
- ✅ Error cases
- ✅ Integration between modules
- ✅ IPC communication
- ✅ Output formatting

### What NOT to Test
- ❌ External dependencies (use mocks)
- ❌ User interaction (stdin/stdout)
- ❌ System time (mock time if needed)
- ❌ File system operations (use tempfile)

### Test Naming Convention
```
#[test]
fn test_<module>_<functionality>_<scenario>() {
    // test code
}

// Examples:
test_parser_simple_command()
test_parser_invalid_flags_error()
test_ipc_client_timeout_handling()
test_formatter_json_special_chars()
```

---

## Debugging Tests

### Print Debug Info in Tests
```rust
#[test]
fn test_something() {
    let result = some_function();
    println!("Debug: {:?}", result);  // Use println!
    assert_eq!(result, expected);
}
```

### Run with Output
```bash
cargo test test_something -- --nocapture
```

### Use dbg! Macro
```rust
let value = dbg!(some_function());
```

---

## Summary of Testing Requirements

| Phase | Target Coverage | Focus |
|-------|-----------------|-------|
| 1.8.1 | > 80% | Parser, formatters, history |
| 1.8.2 | > 80% | System commands |
| 1.8.3 | > 75% | IPC integration, service commands |
| 1.8.4 | > 70% | Complex commands, error handling |
| 1.8.5 | > 85% | Full coverage, performance |
