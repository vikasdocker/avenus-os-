// Conversation context - small bounded memory for pronoun resolution.
//
// Keeps the last few exchanges so that "it", "that", "bring it to front"
// can be resolved to the most recently mentioned application or window.
// Bounded to avoid unbounded memory or leaking sensitive data.

use std::collections::VecDeque;

/// One turn of conversation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversationTurn {
    pub user_text: String,
    /// Apps mentioned or acted on in this turn (normalized ids).
    pub apps: Vec<String>,
    /// Windows referenced.
    pub windows: Vec<String>,
    /// Files referenced (relative paths).
    pub files: Vec<String>,
}

/// Bounded conversation memory.
#[derive(Debug, Clone)]
pub struct ConversationContext {
    turns: VecDeque<ConversationTurn>,
    capacity: usize,
    /// Last app id mentioned across any turn (normalized).
    last_app: Option<String>,
    /// Last window title referenced.
    last_window: Option<String>,
    /// Last file path referenced.
    last_file: Option<String>,
}

impl ConversationContext {
    pub fn new(capacity: usize) -> Self {
        Self {
            turns: VecDeque::with_capacity(capacity),
            capacity: capacity.max(2),
            last_app: None,
            last_window: None,
            last_file: None,
        }
    }

    pub fn last_app(&self) -> Option<&str> {
        self.last_app.as_deref()
    }

    pub fn last_window(&self) -> Option<&str> {
        self.last_window.as_deref()
    }

    pub fn last_file(&self) -> Option<&str> {
        self.last_file.as_deref()
    }

    /// Record a user turn and the apps/windows/files that turn resolved to.
    pub fn push(&mut self, user_text: &str, apps: Vec<String>, windows: Vec<String>) {
        self.push_with_files(user_text, apps, windows, Vec::new());
    }

    /// Extended push that also tracks files.
    pub fn push_with_files(
        &mut self,
        user_text: &str,
        apps: Vec<String>,
        windows: Vec<String>,
        files: Vec<String>,
    ) {
        if !apps.is_empty() {
            self.last_app = apps.last().cloned();
        }
        if !windows.is_empty() {
            self.last_window = windows.last().cloned();
        } else if !apps.is_empty() {
            // Windows often mirror apps; use app as window fallback.
            self.last_window = apps.last().cloned();
        }
        if !files.is_empty() {
            self.last_file = files.last().cloned();
        }
        if self.turns.len() >= self.capacity {
            self.turns.pop_front();
        }
        self.turns.push_back(ConversationTurn {
            user_text: user_text.to_string(),
            apps,
            windows,
            files,
        });
    }

    /// Resolve a pronoun target using bounded history.
    /// If text contains "it", "that", "this", "them", return last_app.
    pub fn resolve_pronoun(&self, text: &str) -> Option<String> {
        let lower = text.to_ascii_lowercase();
        let has_pronoun = lower.split_whitespace().any(|w| {
            matches!(
                w.trim_matches(|c: char| !c.is_ascii_alphabetic()),
                "it" | "that" | "this" | "them" | "there"
            )
        }) || lower.contains(" it ")
            || lower.contains(" it.")
            || lower.ends_with(" it");
        if has_pronoun {
            self.last_app.clone().or_else(|| self.last_window.clone())
        } else {
            None
        }
    }

    /// Number of stored turns.
    pub fn len(&self) -> usize {
        self.turns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }

    /// Return a short debug summary for logging (no sensitive raw text dump).
    pub fn summary(&self) -> String {
        format!(
            "turns={} last_app={:?} last_window={:?} last_file={:?}",
            self.turns.len(),
            self.last_app,
            self.last_window,
            self.last_file
        )
    }
}

impl Default for ConversationContext {
    fn default() -> Self {
        Self::new(8)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_capacity_evicts_oldest() {
        let mut ctx = ConversationContext::new(2);
        ctx.push("Open Calculator", vec!["calculator".to_string()], vec![]);
        ctx.push("Open Notes", vec!["notes".to_string()], vec![]);
        ctx.push("Open Files", vec!["files".to_string()], vec![]);
        assert_eq!(ctx.len(), 2);
        assert_eq!(ctx.last_app(), Some("files"));
    }

    #[test]
    fn pronoun_resolution_uses_last_app() {
        let mut ctx = ConversationContext::new(4);
        ctx.push("Open Notes", vec!["notes".to_string()], vec![]);
        assert_eq!(ctx.resolve_pronoun("Bring it to the front"), Some("notes".to_string()));
        assert_eq!(ctx.resolve_pronoun("minimize it"), Some("notes".to_string()));
        assert_eq!(ctx.resolve_pronoun("Open Calculator"), None);
    }

    #[test]
    fn last_app_tracks_across_turns() {
        let mut ctx = ConversationContext::default();
        assert!(ctx.last_app().is_none());
        ctx.push("Open Calculator", vec!["calculator".to_string()], vec![]);
        assert_eq!(ctx.last_app(), Some("calculator"));
        ctx.push("Bring it to front", vec!["calculator".to_string()], vec![]);
        assert_eq!(ctx.last_app(), Some("calculator"));
    }

    #[test]
    fn it_pronoun_edge_cases() {
        let mut ctx = ConversationContext::new(4);
        ctx.push("Open Notes", vec!["notes".to_string()], vec![]);
        assert_eq!(ctx.resolve_pronoun("Bring it to the front."), Some("notes".to_string()));
        assert_eq!(ctx.resolve_pronoun("it"), Some("notes".to_string()));
    }
}
