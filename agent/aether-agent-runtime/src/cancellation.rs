// Agent Runtime - Cancellation support
//
// Supports cancelling sessions, plans, actions, and timeouts.
// The system must safely stop execution.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// A cancellation token that can be shared across threads.
#[derive(Debug, Clone)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Requests cancellation.
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
    }

    /// Returns true if cancellation has been requested.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    /// Resets the cancellation state (for reuse).
    pub fn reset(&self) {
        self.cancelled.store(false, Ordering::SeqCst);
    }

    /// Blocks until cancelled or timeout expires.
    /// Returns true if cancelled, false if timeout.
    pub fn wait(&self, timeout: std::time::Duration) -> bool {
        let start = std::time::Instant::now();
        while !self.is_cancelled() {
            if start.elapsed() >= timeout {
                return false;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        true
    }
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_starts_not_cancelled() {
        let t = CancellationToken::new();
        assert!(!t.is_cancelled());
    }

    #[test]
    fn cancel_sets_flag() {
        let t = CancellationToken::new();
        t.cancel();
        assert!(t.is_cancelled());
    }

    #[test]
    fn reset_clears_flag() {
        let t = CancellationToken::new();
        t.cancel();
        t.reset();
        assert!(!t.is_cancelled());
    }

    #[test]
    fn clone_shares_state() {
        let t1 = CancellationToken::new();
        let t2 = t1.clone();
        t1.cancel();
        assert!(t2.is_cancelled());
    }

    #[test]
    fn wait_returns_false_on_timeout() {
        let t = CancellationToken::new();
        let result = t.wait(std::time::Duration::from_millis(10));
        assert!(!result);
    }

    #[test]
    fn wait_returns_true_when_cancelled() {
        let t = CancellationToken::new();
        let t2 = t.clone();
        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(5));
            t2.cancel();
        });
        let result = t.wait(std::time::Duration::from_secs(1));
        assert!(result);
    }
}
