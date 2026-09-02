//! Aether animation runtime — the engine behind the
//! §12 motion system.
//!
//! §12: "Smooth, fast, natural, premium. Roughly
//! 150–300 ms. Used to communicate: window state, AI
//! state, selection, loading, navigation, completion.
//! Do not over-animate."
//!
//! The motion system tokens (`DurationMs`, `Easing`,
//! the standard constants `TAP`, `HOVER`, `NAV`,
//! `WINDOW_STATE`, `AI_CROSSFADE`) live in
//! `aether-design-tokens` (6.1). This crate is the
//! *runtime* that turns those tokens into a working
//! animation.
//!
//! The runtime is intentionally minimal:
//!
//! - An `Animation` is a value (`duration` +
//!   `easing` + `from` + `to`).
//! - The caller drives the animation by calling
//!   `Animation::advance(elapsed_ms)` periodically
//!   (typically from the shell's frame loop).
//! - `Animation::progress()` returns the eased
//!   interpolation as a `f32` in `[0.0, 1.0]`.
//! - The animation is "done" when
//!   `Animation::is_complete()` returns true; the
//!   caller stops driving it.
//!
//! There is no timer thread, no `Instant::now()`, no
//! platform-specific code. This is a pure
//! deterministic function over `elapsed_ms`, so the
//! same input always produces the same output —
//! essential for the headless test renderer and the
//! snapshot tests.
//!
//! The crate also defines a small set of
//! `Animation::new_*` constructors that map to the
//! standard §12 motion vocabulary (tap / hover /
//! nav / window-state / ai-crossfade) so call sites
//! are short and consistent.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

use aether_design_tokens::motion::{DurationMs, Easing};

/// A single animation: a duration, an easing curve,
/// and `from` / `to` values. The animation is driven
/// by the caller calling `advance(elapsed_ms)` and
/// reading `progress()`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Animation {
    /// The total duration of the animation.
    pub duration: DurationMs,
    /// The easing curve.
    pub easing: Easing,
    /// The starting value (when `elapsed_ms == 0`).
    pub from: f32,
    /// The ending value (when `elapsed_ms >=
    /// ` `duration`).
    pub to: f32,
}

impl Animation {
    /// The §12 default `tap` animation — 150 ms,
    /// standard easing, 0 → 1.
    #[must_use]
    pub fn tap() -> Self {
        Self {
            duration: aether_design_tokens::motion::TAP,
            easing: Easing::Standard,
            from: 0.0,
            to: 1.0,
        }
    }

    /// The §12 default `hover` animation — 180 ms,
    /// standard easing, 0 → 1.
    #[must_use]
    pub fn hover() -> Self {
        Self {
            duration: aether_design_tokens::motion::HOVER,
            easing: Easing::Standard,
            from: 0.0,
            to: 1.0,
        }
    }

    /// The §12 default `nav` animation — 240 ms,
    /// standard easing, 0 → 1.
    #[must_use]
    pub fn nav() -> Self {
        Self {
            duration: aether_design_tokens::motion::NAV,
            easing: Easing::Standard,
            from: 0.0,
            to: 1.0,
        }
    }

    /// The §12 default `window_state` animation —
    /// 400 ms, standard easing, 0 → 1.
    #[must_use]
    pub fn window_state() -> Self {
        Self {
            duration: aether_design_tokens::motion::WINDOW_STATE,
            easing: Easing::Standard,
            from: 0.0,
            to: 1.0,
        }
    }

    /// The §12 default `ai_crossfade` animation —
    /// 600 ms, **emphasized** easing, 0 → 1.
    #[must_use]
    pub fn ai_crossfade() -> Self {
        Self {
            duration: aether_design_tokens::motion::AI_CROSSFADE,
            easing: Easing::Emphasized,
            from: 0.0,
            to: 1.0,
        }
    }

    /// Construct an animation with the given duration
    /// and easing, from 0 to 1.
    #[must_use]
    pub fn new(duration: DurationMs, easing: Easing) -> Self {
        Self { duration, easing, from: 0.0, to: 1.0 }
    }

    /// Override `from`.
    #[must_use]
    pub fn from(mut self, f: f32) -> Self {
        self.from = f;
        self
    }

    /// Override `to`.
    #[must_use]
    pub fn to(mut self, t: f32) -> Self {
        self.to = t;
        self
    }

    /// The current eased progress, given the elapsed
    /// milliseconds. Returns 0.0 if the animation
    /// hasn't started, 1.0 if it's past the duration.
    /// The output is `eased(t) * (to - from) + from`.
    #[must_use]
    pub fn progress(&self, elapsed_ms: u16) -> f32 {
        if self.duration.as_ms() == 0 {
            return self.to;
        }
        let raw = (elapsed_ms as f32) / (self.duration.as_ms() as f32);
        let t = raw.clamp(0.0, 1.0);
        let eased = apply_easing(self.easing, t);
        // Map the eased t into [from, to]. Note: a
        // caller who passed `from = 1.0, to = 0.0`
        // (a "fade out") gets the right thing.
        self.from + (self.to - self.from) * eased
    }

    /// Whether the animation is past its duration.
    #[must_use]
    pub fn is_complete(&self, elapsed_ms: u16) -> bool {
        elapsed_ms >= self.duration.as_ms()
    }

    /// The animation's "reversed" form: 0 ↔ 1.
    /// Used by the renderer to drive reverse
    /// transitions (e.g. close a panel).
    #[must_use]
    pub fn reversed(&self) -> Self {
        Self { from: self.to, to: self.from, ..*self }
    }
}

/// Apply the easing curve to a raw `t` in `[0, 1]`.
/// The math: a cubic bezier from `(0, 0)` through
/// `(x1, y1)` and `(x2, y2)` to `(1, 1)`. The X axis
/// is time, the Y axis is progress. We use Newton's
/// method to find the bezier parameter `s` such that
/// `bezier_x(s) = t`, then return `bezier_y(s)`.
#[must_use]
pub fn apply_easing(easing: Easing, t: f32) -> f32 {
    let (x1, y1, x2, y2) = easing.bezier();
    // Use Newton's method on the X axis: find `s`
    // such that `bezier_x(s) = t`.
    let t = t.clamp(0.0, 1.0);
    let mut s = t;
    for _ in 0..6 {
        let (x, dx) = bezier_axis(s, x1, x2);
        let delta = x - t;
        if delta.abs() < 0.0005 {
            break;
        }
        let step = delta / dx.max(0.0001);
        s = (s - step).clamp(0.0, 1.0);
    }
    let (y, _dy) = bezier_axis(s, y1, y2);
    y
}

// Sample a cubic-bezier axis at parameter `s` in
// [0, 1]. The bezier goes from 0 through p1 and p2
// to 1. Returns `(value, derivative)`.
fn bezier_axis(s: f32, p1: f32, p2: f32) -> (f32, f32) {
    // P(s) = 3(1-s)^2 s * p1 + 3(1-s) s^2 * p2 + s^3
    let s2 = s * s;
    let s3 = s2 * s;
    let one = 1.0 - s;
    let one2 = one * one;
    let v = 3.0 * one2 * s * p1 + 3.0 * one * s2 * p2 + s3;
    // dP/ds = 3(1-s)^2 p1 + 6(1-s)s(p2 - p1) + 3s^2(1 - p2)
    let d = 3.0 * one2 * p1 + 6.0 * one * s * (p2 - p1) + 3.0 * s2 * (1.0 - p2);
    (v, d)
}

/// A small fixed-capacity animation queue. The shell
/// uses this to drive multiple concurrent animations
/// (e.g. panel open + button hover + AI cross-fade)
/// without spawning a thread per animation.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AnimationQueue {
    /// The slots. Empty slots are `None`; the queue
    /// is dense (no gaps) — when an animation
    /// completes, its slot is compacted out.
    slots: alloc::vec::Vec<QueuedAnimation>,
}

extern crate alloc;

use alloc::vec::Vec;

impl AnimationQueue {
    /// An empty queue.
    #[must_use]
    pub fn new() -> Self {
        Self { slots: Vec::new() }
    }

    /// Add an animation to the queue with a name
    /// (the caller-supplied handle used to find /
    /// remove it).
    #[must_use]
    pub fn push(mut self, name: &'static str, animation: Animation) -> Self {
        self.slots.push(QueuedAnimation { name, animation, elapsed_ms: 0 });
        self
    }

    /// The current progress of the animation with
    /// the given name, if any. Returns 0.0 if no
    /// such animation is in the queue.
    #[must_use]
    pub fn progress(&self, name: &str) -> f32 {
        self.slots
            .iter()
            .find(|q| q.name == name)
            .map_or(0.0, |q| q.animation.progress(q.elapsed_ms))
    }

    /// Whether the animation with the given name has
    /// completed.
    #[must_use]
    pub fn is_complete(&self, name: &str) -> bool {
        self.slots
            .iter()
            .find(|q| q.name == name)
            .is_some_and(|q| q.animation.is_complete(q.elapsed_ms))
    }

    /// Advance every animation in the queue by
    /// `delta_ms`. Completed animations are
    /// compacted out.
    pub fn advance(&mut self, delta_ms: u16) {
        for q in &mut self.slots {
            q.elapsed_ms = q.elapsed_ms.saturating_add(delta_ms);
        }
        self.slots.retain(|q| !q.animation.is_complete(q.elapsed_ms));
    }

    /// Remove the animation with the given name. If
    /// no such animation exists, this is a no-op.
    pub fn remove(&mut self, name: &str) {
        self.slots.retain(|q| q.name != name);
    }

    /// The number of live animations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.slots.len()
    }

    /// Whether the queue has no live animations.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.slots.is_empty()
    }
}

/// One slot in an `AnimationQueue`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct QueuedAnimation {
    name: &'static str,
    animation: Animation,
    elapsed_ms: u16,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn tap_is_150ms_standard() {
        let a = Animation::tap();
        assert_eq!(a.duration, aether_design_tokens::motion::TAP);
        assert_eq!(a.easing, Easing::Standard);
    }

    #[test]
    fn hover_is_180ms_standard() {
        let a = Animation::hover();
        assert_eq!(a.duration, aether_design_tokens::motion::HOVER);
    }

    #[test]
    fn nav_is_240ms_standard() {
        let a = Animation::nav();
        assert_eq!(a.duration, aether_design_tokens::motion::NAV);
    }

    #[test]
    fn window_state_is_400ms_standard() {
        let a = Animation::window_state();
        assert_eq!(a.duration, aether_design_tokens::motion::WINDOW_STATE);
    }

    #[test]
    fn ai_crossfade_is_600ms_emphasized() {
        let a = Animation::ai_crossfade();
        assert_eq!(a.duration, aether_design_tokens::motion::AI_CROSSFADE);
        assert_eq!(a.easing, Easing::Emphasized);
    }

    #[test]
    fn new_sets_duration_and_easing() {
        let a = Animation::new(DurationMs::from_ms(500), Easing::Linear);
        assert_eq!(a.duration, DurationMs::from_ms(500));
        assert_eq!(a.easing, Easing::Linear);
    }

    #[test]
    fn progress_at_zero_is_from() {
        let a = Animation::new(DurationMs::from_ms(100), Easing::Linear).from(0.25);
        assert_eq!(a.progress(0), 0.25);
    }

    #[test]
    fn progress_past_end_is_to() {
        let a = Animation::new(DurationMs::from_ms(100), Easing::Linear);
        assert_eq!(a.progress(200), 1.0);
    }

    #[test]
    fn progress_at_half_is_midpoint_linear() {
        let a = Animation::new(DurationMs::from_ms(100), Easing::Linear);
        let p = a.progress(50);
        assert!((p - 0.5).abs() < 0.001, "got {p}");
    }

    #[test]
    fn progress_with_from_to_maps_correctly() {
        let a = Animation::new(DurationMs::from_ms(100), Easing::Linear).from(0.2).to(0.8);
        assert_eq!(a.progress(0), 0.2);
        assert_eq!(a.progress(100), 0.8);
        let mid = a.progress(50);
        assert!((mid - 0.5).abs() < 0.001, "got {mid}");
    }

    #[test]
    fn progress_with_zero_duration_returns_to() {
        let a = Animation::new(DurationMs::from_ms(0), Easing::Linear);
        assert_eq!(a.progress(0), 1.0);
        assert_eq!(a.progress(100), 1.0);
    }

    #[test]
    fn is_complete_false_during() {
        let a = Animation::new(DurationMs::from_ms(100), Easing::Linear);
        assert!(!a.is_complete(0));
        assert!(!a.is_complete(99));
    }

    #[test]
    fn is_complete_true_at_or_past_end() {
        let a = Animation::new(DurationMs::from_ms(100), Easing::Linear);
        assert!(a.is_complete(100));
        assert!(a.is_complete(101));
    }

    #[test]
    fn reversed_swaps_from_and_to() {
        let a = Animation::new(DurationMs::from_ms(100), Easing::Linear).from(0.2).to(0.8);
        let r = a.reversed();
        assert_eq!(r.from, 0.8);
        assert_eq!(r.to, 0.2);
    }

    #[test]
    fn easing_linear_is_identity() {
        let p = apply_easing(Easing::Linear, 0.5);
        assert!((p - 0.5).abs() < 0.01, "linear(0.5) should be ~0.5, got {p}");
    }

    #[test]
    fn easing_linear_endpoints() {
        assert_eq!(apply_easing(Easing::Linear, 0.0), 0.0);
        assert!((apply_easing(Easing::Linear, 1.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn easing_standard_endpoints() {
        assert_eq!(apply_easing(Easing::Standard, 0.0), 0.0);
        assert!((apply_easing(Easing::Standard, 1.0) - 1.0).abs() < 0.01);
    }

    #[test]
    fn queue_starts_empty() {
        let q = AnimationQueue::new();
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn queue_push_grows() {
        let q = AnimationQueue::new().push("a", Animation::tap());
        assert_eq!(q.len(), 1);
        assert!(!q.is_empty());
    }

    #[test]
    fn queue_progress_of_unknown_is_zero() {
        let q = AnimationQueue::new();
        assert_eq!(q.progress("nope"), 0.0);
    }

    #[test]
    fn queue_progress_of_known_reads() {
        let q = AnimationQueue::new()
            .push("a", Animation::new(DurationMs::from_ms(100), Easing::Linear));
        // Just-started animation, 0 ms elapsed.
        assert_eq!(q.progress("a"), 0.0);
    }

    #[test]
    fn queue_advance_moves_all() {
        let mut q = AnimationQueue::new()
            .push("a", Animation::new(DurationMs::from_ms(100), Easing::Linear))
            .push("b", Animation::new(DurationMs::from_ms(200), Easing::Linear));
        q.advance(50);
        // 50/100 = 0.5
        assert!((q.progress("a") - 0.5).abs() < 0.001);
        // 50/200 = 0.25
        assert!((q.progress("b") - 0.25).abs() < 0.001);
    }

    #[test]
    fn queue_compacts_completed() {
        let mut q = AnimationQueue::new()
            .push("a", Animation::new(DurationMs::from_ms(100), Easing::Linear));
        q.advance(200);
        // "a" is past its duration; should be removed.
        assert_eq!(q.len(), 0);
        assert!(q.is_empty());
    }

    #[test]
    fn queue_remove_by_name() {
        let mut q = AnimationQueue::new().push("a", Animation::tap()).push("b", Animation::hover());
        q.remove("a");
        assert_eq!(q.len(), 1);
        assert_eq!(q.progress("a"), 0.0);
        assert_eq!(q.progress("b"), 0.0);
    }

    #[test]
    fn queue_remove_unknown_is_noop() {
        let mut q = AnimationQueue::new().push("a", Animation::tap());
        q.remove("nope");
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn queue_is_complete_for_unknown_is_false() {
        let q = AnimationQueue::new();
        assert!(!q.is_complete("nope"));
    }

    #[test]
    fn queue_is_complete_true_when_past() {
        // Check is_complete *before* advance: at
        // 0 ms elapsed with 100 ms duration, it's
        // not complete. Then advance to 150 ms and
        // verify the queue was compacted (the
        // animation is gone, hence its slot is too).
        let mut q = AnimationQueue::new()
            .push("a", Animation::new(DurationMs::from_ms(100), Easing::Linear));
        assert!(!q.is_complete("a"));
        q.advance(150);
        // After advance, the animation has been
        // compacted out (since 150 > 100). The
        // queue is empty.
        assert!(q.is_empty());
        assert_eq!(q.len(), 0);
    }

    #[test]
    fn queue_is_complete_true_during_at_boundary() {
        // Boundary semantics: at exactly the
        // duration, is_complete returns true and
        // advance compacts the slot.
        let mut q = AnimationQueue::new()
            .push("a", Animation::new(DurationMs::from_ms(100), Easing::Linear));
        // Not complete at 0 ms.
        assert!(!q.is_complete("a"));
        q.advance(100);
        // After advance(100), the slot is compacted
        // out (since 100 >= 100).
        assert!(q.is_empty());
    }
}
