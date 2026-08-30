// Aether Graphics - Input management for the graphics stack
//
// Normalizes keyboard, pointer, and touch events into the shared
// InputEvent stream consumed by the compositor and shell.

use crate::error::GraphicsError;
use crate::types::{ButtonState, InputEvent, MouseButton};
use std::collections::VecDeque;

/// Capacity of the internal event queue before oldest events are dropped.
const EVENT_QUEUE_CAPACITY: usize = 1024;

/// Aggregated input device statistics.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InputStats {
    pub mouse_moves: u64,
    pub button_events: u64,
    pub key_events: u64,
    pub scroll_events: u64,
    pub touch_events: u64,
    pub dropped_events: u64,
}

/// Receives raw events, queues them, and dispatches them in order.
pub struct InputManager {
    queue: VecDeque<InputEvent>,
    stats: InputStats,
    cursor_x: f32,
    cursor_y: f32,
}

impl InputManager {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::with_capacity(EVENT_QUEUE_CAPACITY),
            stats: InputStats::default(),
            cursor_x: 0.0,
            cursor_y: 0.0,
        }
    }

    /// Enqueues an event, tracking per-class statistics. Drops the oldest
    /// event when the queue is saturated.
    pub fn push_event(&mut self, event: InputEvent) -> Result<(), GraphicsError> {
        if !matches!(
            event,
            InputEvent::MouseMove { x, y } if x.is_finite() && y.is_finite()
        ) && matches!(event, InputEvent::MouseMove { .. })
        {
            return Err(GraphicsError::Input(
                "Non-finite pointer coordinates rejected".to_string(),
            ));
        }
        match &event {
            InputEvent::MouseMove { x, y } => {
                self.stats.mouse_moves += 1;
                self.cursor_x = *x;
                self.cursor_y = *y;
            }
            InputEvent::MouseButton { .. } => self.stats.button_events += 1,
            InputEvent::KeyInput { .. } => self.stats.key_events += 1,
            InputEvent::Scroll { .. } => self.stats.scroll_events += 1,
            InputEvent::Touch { .. } => self.stats.touch_events += 1,
        }
        if self.queue.len() >= EVENT_QUEUE_CAPACITY {
            self.queue.pop_front();
            self.stats.dropped_events += 1;
        }
        self.queue.push_back(event);
        Ok(())
    }

    /// Pops the next queued event.
    pub fn poll_event(&mut self) -> Option<InputEvent> {
        self.queue.pop_front()
    }

    /// Returns true when at least one event is queued.
    pub fn has_pending(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Returns the last known cursor position.
    pub fn cursor_position(&self) -> (f32, f32) {
        (self.cursor_x, self.cursor_y)
    }

    /// Returns accumulated input statistics.
    pub fn stats(&self) -> InputStats {
        self.stats
    }

    /// Convenience helper mirroring a physical left-button click sequence.
    pub fn synthesize_click(&mut self) -> Result<(), GraphicsError> {
        self.push_event(InputEvent::MouseButton {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
        })?;
        self.push_event(InputEvent::MouseButton {
            button: MouseButton::Left,
            state: ButtonState::Released,
        })
    }
}

impl Default for InputManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_dispatch_in_order() {
        let mut im = InputManager::new();
        im.push_event(InputEvent::MouseMove { x: 1.0, y: 2.0 }).unwrap_or_else(|e| panic!("{e}"));
        im.synthesize_click().unwrap_or_else(|e| panic!("{e}"));
        assert_eq!(im.poll_event(), Some(InputEvent::MouseMove { x: 1.0, y: 2.0 }));
        assert_eq!(
            im.poll_event(),
            Some(InputEvent::MouseButton {
                button: MouseButton::Left,
                state: ButtonState::Pressed,
            })
        );
        assert!(im.has_pending());
    }

    #[test]
    fn non_finite_move_rejected() {
        let mut im = InputManager::new();
        assert!(im.push_event(InputEvent::MouseMove { x: f32::NAN, y: 0.0 }).is_err());
    }

    #[test]
    fn stats_track_event_classes() {
        let mut im = InputManager::new();
        im.synthesize_click().unwrap_or_else(|e| panic!("{e}"));
        let s = im.stats();
        assert_eq!(s.button_events, 2);
        assert_eq!(s.mouse_moves, 0);
    }
}
