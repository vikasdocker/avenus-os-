// Agent Runtime - Tool system
//
// Tools map to existing Aether services. Each tool declares its required
// capabilities, risk level, and side effects. Tools must not automatically
// receive every capability.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique tool identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ToolId(pub String);

impl ToolId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

impl fmt::Display for ToolId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A tool definition declaring what the tool does and what it requires.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub id: ToolId,
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
    pub output_schema: serde_json::Value,
    pub required_capabilities: Vec<String>,
    pub risk_level: ToolRisk,
    pub timeout_ms: u64,
    pub side_effects: Vec<String>,
    pub requires_confirmation: bool,
}

/// Risk level for tools.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum ToolRisk {
    Low,
    Medium,
    High,
    Critical,
}

impl ToolDefinition {
    pub fn new(id: &str, name: &str, description: &str) -> Self {
        Self {
            id: ToolId::new(id),
            name: name.to_string(),
            description: description.to_string(),
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            required_capabilities: Vec::new(),
            risk_level: ToolRisk::Low,
            timeout_ms: 30_000,
            side_effects: Vec::new(),
            requires_confirmation: false,
        }
    }

    pub fn with_capability(mut self, cap: &str) -> Self {
        self.required_capabilities.push(cap.to_string());
        self
    }

    pub fn with_risk_level(mut self, level: ToolRisk) -> Self {
        self.risk_level = level;
        self
    }

    pub fn with_timeout(mut self, ms: u64) -> Self {
        self.timeout_ms = ms;
        self
    }

    pub fn with_side_effect(mut self, effect: &str) -> Self {
        self.side_effects.push(effect.to_string());
        self
    }

    pub fn with_confirmation(mut self) -> Self {
        self.requires_confirmation = true;
        self
    }

    pub fn with_input_schema(mut self, schema: serde_json::Value) -> Self {
        self.input_schema = schema;
        self
    }

    pub fn with_output_schema(mut self, schema: serde_json::Value) -> Self {
        self.output_schema = schema;
        self
    }

    /// Validates tool input against the declared schema (basic check).
    pub fn validate_input(&self, input: &serde_json::Value) -> Result<(), String> {
        // Basic validation: ensure required fields exist
        if let Some(required) = self.input_schema.get("required") {
            if let Some(fields) = required.as_array() {
                for field in fields {
                    if let Some(name) = field.as_str() {
                        if input.get(name).is_none() {
                            return Err(format!("Missing required field: {name}"));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Tool registry for managing available tools.
pub struct ToolRegistry {
    tools: std::collections::HashMap<ToolId, ToolDefinition>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self { tools: std::collections::HashMap::new() }
    }

    /// Registers a tool. Returns an error if the tool ID already exists.
    pub fn register(&mut self, tool: ToolDefinition) -> Result<(), String> {
        if self.tools.contains_key(&tool.id) {
            return Err(format!("Tool '{}' already registered", tool.id));
        }
        self.tools.insert(tool.id.clone(), tool);
        Ok(())
    }

    /// Unregisters a tool.
    pub fn unregister(&mut self, id: &ToolId) -> bool {
        self.tools.remove(id).is_some()
    }

    /// Looks up a tool by ID.
    pub fn get(&self, id: &ToolId) -> Option<&ToolDefinition> {
        self.tools.get(id)
    }

    /// Lists all registered tools.
    pub fn list(&self) -> Vec<&ToolDefinition> {
        self.tools.values().collect()
    }

    /// Discovers tools by capability requirement.
    pub fn discover(&self, capability: &str) -> Vec<&ToolDefinition> {
        self.tools
            .values()
            .filter(|t| t.required_capabilities.iter().any(|c| c == capability))
            .collect()
    }

    /// Returns the number of registered tools.
    pub fn count(&self) -> usize {
        self.tools.len()
    }

    /// Validates that a tool's input meets its schema.
    pub fn validate_input(&self, id: &ToolId, input: &serde_json::Value) -> Result<(), String> {
        let tool = self.tools.get(id).ok_or_else(|| format!("Tool '{}' not found", id))?;
        tool.validate_input(input)
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Creates the default tool registry with initial tools.
pub fn default_tools() -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    let tools = vec![
        ToolDefinition::new("filesystem.list", "List Files", "List files in a directory")
            .with_capability("file.list")
            .with_input_schema(serde_json::json!({"required": ["path"]})),
        ToolDefinition::new("filesystem.stat", "File Status", "Get file metadata")
            .with_capability("file.read")
            .with_input_schema(serde_json::json!({"required": ["path"]})),
        ToolDefinition::new("filesystem.search", "Search Files", "Search for files")
            .with_capability("file.search")
            .with_input_schema(serde_json::json!({"required": ["query"]})),
        ToolDefinition::new("storage.status", "Storage Status", "Get storage status")
            .with_capability("storage.status"),
        ToolDefinition::new("process.list", "List Processes", "List running processes")
            .with_capability("process.list"),
        ToolDefinition::new("process.inspect", "Inspect Process", "Inspect a process")
            .with_capability("process.inspect")
            .with_input_schema(serde_json::json!({"required": ["pid"]})),
        ToolDefinition::new(
            "application.list",
            "List Applications",
            "List registered applications",
        )
        .with_capability("application.list"),
        ToolDefinition::new("application.launch", "Launch Application", "Launch an application")
            .with_capability("application.launch")
            .with_risk_level(ToolRisk::Medium)
            .with_side_effect("process.start")
            .with_input_schema(serde_json::json!({"required": ["application_id"]})),
        ToolDefinition::new("application.close", "Close Application", "Close an application")
            .with_capability("application.close")
            .with_risk_level(ToolRisk::Medium)
            .with_side_effect("process.stop")
            .with_input_schema(serde_json::json!({"required": ["application_id"]})),
        ToolDefinition::new("network.status", "Network Status", "Get network status")
            .with_capability("network.status"),
        ToolDefinition::new("network.interfaces", "Network Interfaces", "List network interfaces")
            .with_capability("network.interfaces"),
        ToolDefinition::new("system.status", "System Status", "Get system status")
            .with_capability("system.status"),
        ToolDefinition::new("window.list", "List Windows", "List open windows")
            .with_capability("window.list"),
        ToolDefinition::new("window.inspect", "Inspect Window", "Inspect a window")
            .with_capability("window.list")
            .with_input_schema(serde_json::json!({"required": ["window_id"]})),
    ];

    for tool in tools {
        let _ = registry.register(tool);
    }

    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_creation() {
        let t = ToolDefinition::new("test.tool", "Test", "A test tool")
            .with_capability("test.cap")
            .with_risk_level(ToolRisk::Medium)
            .with_timeout(5000)
            .with_side_effect("state.change");
        assert_eq!(t.id, ToolId::new("test.tool"));
        assert_eq!(t.required_capabilities, vec!["test.cap".to_string()]);
        assert_eq!(t.risk_level, ToolRisk::Medium);
        assert_eq!(t.timeout_ms, 5000);
        assert_eq!(t.side_effects, vec!["state.change".to_string()]);
    }

    #[test]
    fn registry_register_and_get() {
        let mut reg = ToolRegistry::new();
        let t = ToolDefinition::new("a", "A", "tool a");
        assert!(reg.register(t).is_ok());
        assert!(reg.get(&ToolId::new("a")).is_some());
        assert_eq!(reg.count(), 1);
    }

    #[test]
    fn registry_rejects_duplicate() {
        let mut reg = ToolRegistry::new();
        assert!(reg.register(ToolDefinition::new("a", "A", "tool a")).is_ok());
        assert!(reg.register(ToolDefinition::new("a", "A2", "tool a2")).is_err());
    }

    #[test]
    fn registry_unregister() {
        let mut reg = ToolRegistry::new();
        assert!(reg.register(ToolDefinition::new("a", "A", "tool a")).is_ok());
        assert!(reg.unregister(&ToolId::new("a")));
        assert!(reg.get(&ToolId::new("a")).is_none());
    }

    #[test]
    fn registry_discover_by_capability() {
        let mut reg = ToolRegistry::new();
        assert!(reg
            .register(ToolDefinition::new("a", "A", "tool a").with_capability("cap.x"))
            .is_ok());
        assert!(reg
            .register(ToolDefinition::new("b", "B", "tool b").with_capability("cap.y"))
            .is_ok());
        assert!(reg
            .register(ToolDefinition::new("c", "C", "tool c").with_capability("cap.x"))
            .is_ok());
        let found = reg.discover("cap.x");
        assert_eq!(found.len(), 2);
    }

    #[test]
    fn default_tools_has_14_entries() {
        let reg = default_tools();
        assert_eq!(reg.count(), 14);
    }

    #[test]
    fn tool_validate_input_missing_required() {
        let t = ToolDefinition::new("x", "X", "x")
            .with_input_schema(serde_json::json!({"required": ["path"]}));
        assert!(t.validate_input(&serde_json::json!({})).is_err());
        assert!(t.validate_input(&serde_json::json!({"path": "/tmp"})).is_ok());
    }
}
