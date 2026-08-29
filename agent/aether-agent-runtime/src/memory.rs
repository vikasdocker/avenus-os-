// Agent Runtime - Memory foundation
//
// Provides conversation context, session context, and working memory.
// The interface allows future persistent memory. Sensitive information
// is not stored by default.

use std::collections::VecDeque;

/// A single memory entry.
#[derive(Debug, Clone)]
pub struct MemoryEntry {
    pub role: MemoryRole,
    pub content: String,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryRole {
    User,
    Agent,
    System,
    Tool,
}

/// Conversation memory with bounded capacity.
pub struct ConversationMemory {
    entries: VecDeque<MemoryEntry>,
    capacity: usize,
}

impl ConversationMemory {
    pub fn new(capacity: usize) -> Self {
        Self { entries: VecDeque::with_capacity(capacity), capacity }
    }

    /// Adds an entry, evicting oldest if at capacity.
    pub fn add(&mut self, role: MemoryRole, content: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        if self.entries.len() >= self.capacity {
            self.entries.pop_front();
        }
        self.entries.push_back(MemoryEntry { role, content: content.to_string(), timestamp: now });
    }

    /// Returns all entries.
    pub fn entries(&self) -> Vec<&MemoryEntry> {
        self.entries.iter().collect()
    }

    /// Returns the number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns true if empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clears all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Returns entries formatted as context for the LLM.
    pub fn as_context(&self) -> String {
        self.entries
            .iter()
            .map(|e| {
                let role = match e.role {
                    MemoryRole::User => "USER",
                    MemoryRole::Agent => "AGENT",
                    MemoryRole::System => "SYSTEM",
                    MemoryRole::Tool => "TOOL",
                };
                format!("[{role}] {}", e.content)
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl Default for ConversationMemory {
    fn default() -> Self {
        Self::new(32)
    }
}

/// Session-scoped working memory.
pub struct SessionMemory {
    pub conversation: ConversationMemory,
    pub working: std::collections::HashMap<String, String>,
}

impl SessionMemory {
    pub fn new() -> Self {
        Self {
            conversation: ConversationMemory::new(32),
            working: std::collections::HashMap::new(),
        }
    }

    /// Stores a key-value pair in working memory.
    pub fn store(&mut self, key: &str, value: &str) {
        self.working.insert(key.to_string(), value.to_string());
    }

    /// Retrieves a value from working memory.
    pub fn retrieve(&self, key: &str) -> Option<&str> {
        self.working.get(key).map(|s| s.as_str())
    }

    /// Removes a key from working memory.
    pub fn remove(&mut self, key: &str) -> bool {
        self.working.remove(key).is_some()
    }

    /// Clears working memory.
    pub fn clear_working(&mut self) {
        self.working.clear();
    }
}

impl Default for SessionMemory {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversation_memory_bounded() {
        let mut mem = ConversationMemory::new(3);
        mem.add(MemoryRole::User, "a");
        mem.add(MemoryRole::User, "b");
        mem.add(MemoryRole::User, "c");
        mem.add(MemoryRole::User, "d"); // evicts "a"
        assert_eq!(mem.len(), 3);
        assert_eq!(mem.entries()[0].content, "b");
    }

    #[test]
    fn conversation_as_context() {
        let mut mem = ConversationMemory::new(10);
        mem.add(MemoryRole::User, "hello");
        mem.add(MemoryRole::Agent, "hi there");
        let ctx = mem.as_context();
        assert!(ctx.contains("[USER] hello"));
        assert!(ctx.contains("[AGENT] hi there"));
    }

    #[test]
    fn session_memory_store_retrieve() {
        let mut mem = SessionMemory::new();
        mem.store("last_app", "calculator");
        assert_eq!(mem.retrieve("last_app"), Some("calculator"));
        assert_eq!(mem.retrieve("missing"), None);
    }

    #[test]
    fn session_memory_remove() {
        let mut mem = SessionMemory::new();
        mem.store("key", "value");
        assert!(mem.remove("key"));
        assert!(!mem.remove("key"));
    }

    #[test]
    fn conversation_clear() {
        let mut mem = ConversationMemory::new(10);
        mem.add(MemoryRole::User, "x");
        assert!(!mem.is_empty());
        mem.clear();
        assert!(mem.is_empty());
    }
}
