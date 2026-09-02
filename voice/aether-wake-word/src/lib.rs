//! Aether wake-word detection — keyword
//! spotting with confidence scoring,
//! false-positive control, and an energy
//! gate.
//!
//! Phase 4.3 of the ROADMAP. The runtime
//! is currently a no-op: it accepts audio
//! frames, runs a simple energy gate, then
//! matches phoneme-ish features against a
//! reference fingerprint. The contract is
//! *typed review* — the shell and the agent
//! can audit what was heard and whether
//! it's a hit.
//!
//! The model has six pieces:
//!
//! 1. **`WakeWord`** — a user-configurable
//!    keyword ("hey aether", "computer",
//!    ...). Stored as a phoneme-ish
//!    fingerprint plus display text.
//! 2. **`EnergyGate`** — a per-frame RMS
//!    threshold to keep the detector from
//!    running on silence.
//! 3. **`WakeEngine`** — the trait the
//!    runtime uses to plug in a real model
//!    (porcupine, snowboy, etc.).
//! 4. **`ReferenceEngine`** — a simple
//!    fingerprint matcher good enough for
//!    tests and graceful degradation.
//! 5. **`WakeDetector`** — the stateful
//!    driver. Buffers frames, gates them
//!    by energy, dispatches to the engine.
//! 6. **`WakeEvent`** — what the detector
//!    returns: the matched keyword and a
//!    confidence score.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use aether_stt::AudioBuffer;

/// A wake word definition: the display
/// text, a phoneme fingerprint, and a
/// matching threshold.
#[derive(Debug, Clone, PartialEq)]
pub struct WakeWord {
    /// The id (e.g. "hey-aether").
    pub id: String,
    /// The display text ("hey aether").
    pub text: String,
    /// A simple phoneme-ish fingerprint:
    /// the normalized low-frequency energy
    /// profile of the word.
    pub fingerprint: Vec<f32>,
    /// The similarity threshold (0.0..=1.0)
    /// for a match.
    pub threshold: f32,
}

impl WakeWord {
    /// A new wake word. The fingerprint is
    /// empty by default — the caller is
    /// expected to fill it in (e.g. from a
    /// sample recording).
    #[must_use]
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self { id: id.into(), text: text.into(), fingerprint: Vec::new(), threshold: 0.7 }
    }

    /// Set the threshold.
    #[must_use]
    pub fn with_threshold(mut self, t: f32) -> Self {
        self.threshold = t.clamp(0.0, 1.0);
        self
    }

    /// Set the fingerprint.
    #[must_use]
    pub fn with_fingerprint(mut self, fp: Vec<f32>) -> Self {
        self.fingerprint = fp;
        self
    }

    /// `true` if the fingerprint is set.
    #[must_use]
    pub fn has_fingerprint(&self) -> bool {
        !self.fingerprint.is_empty()
    }
}

/// The result of a single detection.
#[derive(Debug, Clone, PartialEq)]
pub struct WakeEvent {
    /// The wake word id that matched.
    pub word_id: String,
    /// The display text of the wake word.
    pub word_text: String,
    /// The similarity score (0.0..=1.0).
    pub confidence: f32,
    /// When the wake event happened (ms
    /// since the detector started).
    pub timestamp_ms: u64,
}

/// Wake engine errors.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WakeError {
    /// The audio buffer was empty.
    EmptyAudio,
    /// The wake word has no fingerprint
    /// loaded.
    Unfingerprinted(String),
    /// The engine failed.
    EngineFailure(String),
}

impl core::fmt::Display for WakeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyAudio => f.write_str("audio buffer is empty"),
            Self::Unfingerprinted(id) => write!(f, "wake word '{id}' has no fingerprint"),
            Self::EngineFailure(msg) => write!(f, "wake engine failure: {msg}"),
        }
    }
}

impl std::error::Error for WakeError {}

/// A scored candidate match.
#[derive(Debug, Clone, PartialEq)]
pub struct WakeCandidate {
    /// The wake word id.
    pub word_id: String,
    /// The similarity score.
    pub similarity: f32,
}

/// The wake engine trait.
pub trait WakeEngine {
    /// Score a buffer against all known
    /// wake words. Returns the top
    /// candidates sorted by similarity
    /// descending.
    fn detect(
        &self,
        buffer: &AudioBuffer,
        words: &[WakeWord],
    ) -> Result<Vec<WakeCandidate>, WakeError>;
}

/// A reference wake engine. Computes a
/// simple normalized cross-correlation
/// between the buffer's energy profile and
/// each word's fingerprint.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ReferenceEngine;

impl ReferenceEngine {
    /// Compute the energy profile of a
    /// buffer at 16 bins. Each bin is the
    /// RMS of `samples.len()/16` samples
    /// (last bin takes the remainder).
    #[must_use]
    #[allow(clippy::cast_precision_loss)]
    pub fn profile(buffer: &AudioBuffer) -> Vec<f32> {
        const BINS: usize = 16;
        if buffer.samples.is_empty() {
            return alloc::vec![0.0; BINS];
        }
        let n = buffer.samples.len();
        let bin_size = n / BINS;
        let mut out = Vec::with_capacity(BINS);
        for i in 0..BINS {
            let start = i * bin_size;
            let end = if i == BINS - 1 { n } else { start + bin_size };
            if start >= end {
                out.push(0.0);
                continue;
            }
            let mut sum: f64 = 0.0;
            for s in &buffer.samples[start..end] {
                let v = f64::from(*s);
                sum += v * v;
            }
            let rms = (sum / (end - start) as f64).sqrt();
            out.push((rms / f64::from(i16::MAX)) as f32);
        }
        out
    }

    /// The similarity between two
    /// fingerprints, in [0, 1]. Uses
    /// cosine similarity, falling back to
    /// 0 when either side is empty.
    #[must_use]
    pub fn similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.is_empty() || b.is_empty() {
            return 0.0;
        }
        let n = a.len().min(b.len());
        let mut dot = 0.0_f64;
        let mut na = 0.0_f64;
        let mut nb = 0.0_f64;
        for i in 0..n {
            let x = f64::from(a[i]);
            let y = f64::from(b[i]);
            dot += x * y;
            na += x * x;
            nb += y * y;
        }
        let denom = (na * nb).sqrt();
        if denom == 0.0 {
            0.0
        } else {
            (dot / denom).clamp(0.0, 1.0) as f32
        }
    }
}

impl WakeEngine for ReferenceEngine {
    fn detect(
        &self,
        buffer: &AudioBuffer,
        words: &[WakeWord],
    ) -> Result<Vec<WakeCandidate>, WakeError> {
        if buffer.samples.is_empty() {
            return Err(WakeError::EmptyAudio);
        }
        let profile = Self::profile(buffer);
        let mut out: Vec<WakeCandidate> = words
            .iter()
            .filter(|w| w.has_fingerprint())
            .map(|w| WakeCandidate {
                word_id: w.id.clone(),
                similarity: Self::similarity(&profile, &w.fingerprint),
            })
            .collect();
        out.sort_by(|a, b| {
            b.similarity.partial_cmp(&a.similarity).unwrap_or(core::cmp::Ordering::Equal)
        });
        Ok(out)
    }
}

/// An energy gate: rejects frames below
/// an RMS threshold.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EnergyGate {
    /// The minimum RMS to pass.
    pub min_rms: u32,
}

impl EnergyGate {
    /// A default gate (RMS >= 200).
    #[must_use]
    pub const fn default_gate() -> Self {
        Self { min_rms: 200 }
    }

    /// `true` if the buffer has enough
    /// energy to be considered.
    #[must_use]
    pub fn passes(&self, buffer: &AudioBuffer) -> bool {
        buffer.rms() >= self.min_rms
    }
}

impl Default for EnergyGate {
    fn default() -> Self {
        Self::default_gate()
    }
}

/// A stateful wake detector. Buffers
/// frames, gates by energy, dispatches to
/// the engine, applies a refractory
/// period so a single utterance doesn't
/// fire many times.
pub struct WakeDetector<E: WakeEngine> {
    engine: E,
    words: Vec<WakeWord>,
    gate: EnergyGate,
    refractory_ms: u32,
    last_fire_at_ms: u64,
    total_ms_seen: u64,
    history: Vec<WakeEvent>,
}

impl<E: WakeEngine> WakeDetector<E> {
    /// A new detector.
    #[must_use]
    pub fn new(engine: E) -> Self {
        Self {
            engine,
            words: Vec::new(),
            gate: EnergyGate::default(),
            refractory_ms: 1000,
            last_fire_at_ms: 0,
            total_ms_seen: 0,
            history: Vec::new(),
        }
    }

    /// The number of registered wake
    /// words.
    #[must_use]
    pub fn word_count(&self) -> usize {
        self.words.len()
    }

    /// Register a wake word.
    pub fn register(&mut self, word: WakeWord) {
        self.words.push(word);
    }

    /// Remove a wake word by id.
    pub fn unregister(&mut self, id: &str) -> bool {
        let before = self.words.len();
        self.words.retain(|w| w.id != id);
        before != self.words.len()
    }

    /// The registered words.
    #[must_use]
    pub fn words(&self) -> &[WakeWord] {
        &self.words
    }

    /// Set the energy gate.
    pub fn set_gate(&mut self, gate: EnergyGate) {
        self.gate = gate;
    }

    /// Set the refractory period
    /// (milliseconds after a fire during
    /// which further fires are ignored).
    pub fn set_refractory_ms(&mut self, ms: u32) {
        self.refractory_ms = ms;
    }

    /// The total audio time seen by the
    /// detector.
    #[must_use]
    pub fn total_ms_seen(&self) -> u64 {
        self.total_ms_seen
    }

    /// The detection history.
    #[must_use]
    pub fn history(&self) -> &[WakeEvent] {
        &self.history
    }

    /// Reset the detector's history and
    /// refractory counter.
    pub fn reset(&mut self) {
        self.last_fire_at_ms = 0;
        self.total_ms_seen = 0;
        self.history.clear();
    }

    /// Feed a buffer. Returns the wake
    /// event, if any.
    pub fn feed(&mut self, buffer: &AudioBuffer) -> Option<WakeEvent> {
        self.total_ms_seen = self.total_ms_seen.saturating_add(buffer.duration_ms());
        if !self.gate.passes(buffer) {
            return None;
        }
        // Refractory. Only check after the
        // first fire; `last_fire_at_ms = 0`
        // would otherwise suppress the
        // very first hit.
        if self.last_fire_at_ms > 0
            && self.total_ms_seen.saturating_sub(self.last_fire_at_ms)
                < u64::from(self.refractory_ms)
        {
            return None;
        }
        let candidates = match self.engine.detect(buffer, &self.words) {
            Ok(c) => c,
            Err(_) => return None,
        };
        let best = candidates.first()?;
        let word = self.words.iter().find(|w| w.id == best.word_id)?;
        if best.similarity < word.threshold {
            return None;
        }
        let event = WakeEvent {
            word_id: word.id.clone(),
            word_text: word.text.clone(),
            confidence: best.similarity,
            timestamp_ms: self.total_ms_seen,
        };
        self.last_fire_at_ms = self.total_ms_seen;
        self.history.push(event.clone());
        Some(event)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn word(id: &str, text: &str, fp: Vec<f32>, threshold: f32) -> WakeWord {
        WakeWord::new(id, text).with_fingerprint(fp).with_threshold(threshold)
    }

    fn loud_buffer() -> AudioBuffer {
        let samples: Vec<i16> = (0..1600).map(|i| ((i * 100) % 20000) as i16).collect();
        AudioBuffer { sample_rate_hz: 16000, samples }
    }

    fn silent_buffer() -> AudioBuffer {
        AudioBuffer::silence(16000, 100)
    }

    #[test]
    fn wake_word_default_threshold() {
        let w = WakeWord::new("x", "x");
        assert_eq!(w.threshold, 0.7);
        assert!(!w.has_fingerprint());
    }

    #[test]
    fn wake_word_threshold_clamps() {
        let w = WakeWord::new("x", "x").with_threshold(99.0);
        assert_eq!(w.threshold, 1.0);
    }

    #[test]
    fn wake_word_fingerprint() {
        let w = WakeWord::new("x", "x").with_fingerprint(alloc::vec![0.1, 0.2, 0.3]);
        assert!(w.has_fingerprint());
    }

    #[test]
    fn reference_profile_is_16_bins() {
        let p = ReferenceEngine::profile(&loud_buffer());
        assert_eq!(p.len(), 16);
    }

    #[test]
    fn reference_profile_empty_buffer() {
        let p = ReferenceEngine::profile(&AudioBuffer::silence(16000, 0));
        assert_eq!(p.len(), 16);
        assert!(p.iter().all(|&v| v == 0.0));
    }

    #[test]
    fn similarity_identical_is_one() {
        let a = alloc::vec![0.1, 0.2, 0.3, 0.4];
        assert!((ReferenceEngine::similarity(&a, &a) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn similarity_orthogonal_is_zero() {
        let a = alloc::vec![1.0, 0.0, 0.0, 0.0];
        let b = alloc::vec![0.0, 1.0, 0.0, 0.0];
        assert!(ReferenceEngine::similarity(&a, &b).abs() < 1e-6);
    }

    #[test]
    fn similarity_empty_is_zero() {
        let a: Vec<f32> = Vec::new();
        let b = alloc::vec![0.1, 0.2];
        assert_eq!(ReferenceEngine::similarity(&a, &b), 0.0);
    }

    #[test]
    fn engine_rejects_empty_buffer() {
        let e = ReferenceEngine;
        let words = alloc::vec![word("a", "alpha", alloc::vec![0.1; 16], 0.5)];
        let err = e.detect(&AudioBuffer::silence(16000, 0), &words).unwrap_err();
        assert_eq!(err, WakeError::EmptyAudio);
    }

    #[test]
    fn engine_filters_unfingerprintd() {
        let e = ReferenceEngine;
        let words = alloc::vec![WakeWord::new("a", "alpha")];
        let candidates = e.detect(&loud_buffer(), &words).unwrap();
        assert!(candidates.is_empty());
    }

    #[test]
    fn engine_scores_in_range() {
        let e = ReferenceEngine;
        let fp = ReferenceEngine::profile(&loud_buffer());
        let words = alloc::vec![word("a", "alpha", fp, 0.5)];
        let c = e.detect(&loud_buffer(), &words).unwrap();
        assert_eq!(c.len(), 1);
        assert!((0.0..=1.0).contains(&c[0].similarity));
    }

    #[test]
    fn engine_sorts_by_similarity() {
        let e = ReferenceEngine;
        let buf = loud_buffer();
        let fp = ReferenceEngine::profile(&buf);
        let words = alloc::vec![
            word("a", "alpha", fp.clone(), 0.5),
            word("b", "bravo", alloc::vec![0.5; 16], 0.5),
            word("c", "charlie", fp, 0.5),
        ];
        let c = e.detect(&buf, &words).unwrap();
        // Top candidate should be one of
        // 'a' or 'c' (same fingerprint).
        assert!(c[0].word_id == "a" || c[0].word_id == "c");
    }

    #[test]
    fn energy_gate_default() {
        let g = EnergyGate::default();
        assert_eq!(g.min_rms, 200);
    }

    #[test]
    fn energy_gate_passes_loud() {
        let g = EnergyGate::default();
        assert!(g.passes(&loud_buffer()));
    }

    #[test]
    fn energy_gate_rejects_silent() {
        let g = EnergyGate::default();
        assert!(!g.passes(&silent_buffer()));
    }

    #[test]
    fn wake_error_display() {
        assert_eq!(WakeError::EmptyAudio.to_string(), "audio buffer is empty");
        assert_eq!(
            WakeError::Unfingerprinted(String::from("a")).to_string(),
            "wake word 'a' has no fingerprint"
        );
    }

    #[test]
    fn detector_starts_empty() {
        let d: WakeDetector<ReferenceEngine> = WakeDetector::new(ReferenceEngine);
        assert_eq!(d.word_count(), 0);
        assert!(d.history().is_empty());
    }

    #[test]
    fn detector_register_unregister() {
        let mut d: WakeDetector<ReferenceEngine> = WakeDetector::new(ReferenceEngine);
        d.register(WakeWord::new("a", "alpha"));
        d.register(WakeWord::new("b", "bravo"));
        assert_eq!(d.word_count(), 2);
        assert!(d.unregister("a"));
        assert_eq!(d.word_count(), 1);
        assert!(!d.unregister("missing"));
    }

    #[test]
    fn detector_feed_silent_returns_none() {
        let mut d: WakeDetector<ReferenceEngine> = WakeDetector::new(ReferenceEngine);
        d.register(word("a", "alpha", alloc::vec![0.5; 16], 0.1));
        let event = d.feed(&silent_buffer());
        assert!(event.is_none());
    }

    #[test]
    fn detector_feed_loud_with_match_fires() {
        let mut d: WakeDetector<ReferenceEngine> = WakeDetector::new(ReferenceEngine);
        let fp = ReferenceEngine::profile(&loud_buffer());
        d.register(word("a", "alpha", fp, 0.0));
        let event = d.feed(&loud_buffer());
        assert!(event.is_some());
        assert_eq!(event.unwrap().word_id, "a");
    }

    #[test]
    fn detector_refractory_period() {
        let mut d: WakeDetector<ReferenceEngine> = WakeDetector::new(ReferenceEngine);
        d.set_refractory_ms(5000);
        let fp = ReferenceEngine::profile(&loud_buffer());
        d.register(word("a", "alpha", fp, 0.0));
        let _ = d.feed(&loud_buffer());
        // Second feed should be ignored.
        let second = d.feed(&loud_buffer());
        assert!(second.is_none());
    }

    #[test]
    fn detector_records_history() {
        let mut d: WakeDetector<ReferenceEngine> = WakeDetector::new(ReferenceEngine);
        d.set_refractory_ms(0);
        let fp = ReferenceEngine::profile(&loud_buffer());
        d.register(word("a", "alpha", fp, 0.0));
        let _ = d.feed(&loud_buffer());
        let _ = d.feed(&loud_buffer());
        assert_eq!(d.history().len(), 2);
    }

    #[test]
    fn detector_resets() {
        let mut d: WakeDetector<ReferenceEngine> = WakeDetector::new(ReferenceEngine);
        let fp = ReferenceEngine::profile(&loud_buffer());
        d.register(word("a", "alpha", fp, 0.0));
        let _ = d.feed(&loud_buffer());
        d.reset();
        assert!(d.history().is_empty());
        assert_eq!(d.total_ms_seen(), 0);
    }
}
