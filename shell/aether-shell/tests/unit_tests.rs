// Unit tests for command parsing and registry
#[cfg(test)]
mod tests {
    use aether_shell::command::CommandRegistry;
    use aether_shell::history::ShellHistory;
    use aether_shell::output::OutputFormatter;
    use aether_shell::session::ShellSession;

    #[tokio::test]
    async fn test_registry_help_command() {
        let registry = CommandRegistry::new();
        let session = ShellSession::new();
        let mut formatter = OutputFormatter::new();
        let history = ShellHistory::new();

        let result = registry.execute("help", &[], &session, &mut formatter, &history).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_registry_version_command() {
        let registry = CommandRegistry::new();
        let session = ShellSession::new();
        let mut formatter = OutputFormatter::new();
        let history = ShellHistory::new();

        let result = registry.execute("version", &[], &session, &mut formatter, &history).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_registry_unknown_command() {
        let registry = CommandRegistry::new();
        let session = ShellSession::new();
        let mut formatter = OutputFormatter::new();
        let history = ShellHistory::new();

        let result = registry.execute("nonexistent", &[], &session, &mut formatter, &history).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_shell_session_creation() {
        let session = ShellSession::new();
        assert!(!session.session_id().is_empty());
        assert!(!session.actor().is_empty());
        assert!(session.has_capability("shell.basic"));
    }

    #[test]
    fn test_shell_session_capabilities() {
        let mut session = ShellSession::new();
        assert!(session.has_capability("filesystem.read"));

        session.add_capability("custom.capability".to_string());
        assert!(session.has_capability("custom.capability"));
    }

    #[test]
    fn test_shell_history_add() {
        let mut history = ShellHistory::new();
        history.add("help");
        history.add("status");

        let entries = history.get_all();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_shell_history_filters_sensitive() {
        let mut history = ShellHistory::new();
        history.add("help");
        history.add("set password=secret");

        let entries = history.get_all();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0], "help");
    }

    #[test]
    fn test_shell_history_clear() {
        let mut history = ShellHistory::new();
        history.add("help");
        history.add("status");
        history.clear();

        let entries = history.get_all();
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_output_formatter_text_mode() {
        let formatter = OutputFormatter::new();
        // Just verify it was created without error
        let _ = formatter;
    }

    #[test]
    fn test_output_formatter_json_mode() {
        let mut formatter = OutputFormatter::new();
        formatter.set_json_mode(true);
        // Just verify it was modified without error
        let _ = formatter;
    }
}
