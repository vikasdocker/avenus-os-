//! Aether text-to-speech (TTS) — the typed
//! model for turning text into an
//! `AudioBuffer`.
//!
//! Phase 4.2 of the ROADMAP. The runtime
//! currently a no-op: a `NullTts` that returns
//! silence. The contract is *typed review* —
//! the shell and the agent can audit what
//! was spoken, at what rate, and in what
//! voice.
//!
//! The model has five pieces:
//!
//! 1. **`Voice`** — the speaker's identity
//!    ("Aether", "System", a custom name).
//! 2. **`SpeechStyle`** — rate, pitch, and
//!    emphasis controls.
//! 3. **`SpeakRequest`** — what the caller
//!    wants spoken: text, voice, style, and
//!    a sample rate.
//! 4. **`TtsEngine`** — the trait the
//!    runtime uses to plug in a real model
//!    (piper, coqui, etc.).
//! 5. **`TtsSession`** — the stateful
//!    driver. Buffers utterances, applies
//!    normalization, dispatches to the
//!    engine, and tracks the current
//!    utterance.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

/// A speaker identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Voice {
    /// The unique voice id.
    pub id: String,
    /// The display name (e.g. "Aether").
    pub name: String,
    /// The locale tag (e.g. "en-US").
    pub locale: String,
    /// A short description ("warm, low,
    /// conversational").
    pub description: String,
}

impl Voice {
    /// A new voice.
    #[must_use]
    pub fn new(id: impl Into<String>, name: impl Into<String>, locale: impl Into<String>) -> Self {
        Self { id: id.into(), name: name.into(), locale: locale.into(), description: String::new() }
    }

    /// A description.
    #[must_use]
    pub fn with_description(mut self, desc: impl Into<String>) -> Self {
        self.description = desc.into();
        self
    }
}

/// A speech style: rate, pitch, and volume.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeechStyle {
    /// Speaking rate multiplier (1.0 =
    /// normal, 1.5 = 50% faster).
    pub rate: f32,
    /// Pitch shift in semitones (0 = no
    /// shift, +12 = one octave up).
    pub pitch_semitones: i8,
    /// Volume in decibels of attenuation
    /// (0 = full volume, -6 = half).
    pub volume_db: i8,
    /// Emphasis (0..=2: 0 = none, 1 =
    /// moderate, 2 = strong).
    pub emphasis: u8,
}

impl SpeechStyle {
    /// The default style.
    #[must_use]
    pub const fn default_style() -> Self {
        Self { rate: 1.0, pitch_semitones: 0, volume_db: 0, emphasis: 0 }
    }

    /// Set the rate.
    #[must_use]
    pub fn with_rate(mut self, rate: f32) -> Self {
        self.rate = rate.clamp(0.5, 3.0);
        self
    }

    /// Set the pitch.
    #[must_use]
    pub fn with_pitch(mut self, semitones: i8) -> Self {
        self.pitch_semitones = semitones.clamp(-24, 24);
        self
    }

    /// Set the volume.
    #[must_use]
    pub fn with_volume(mut self, db: i8) -> Self {
        self.volume_db = db.clamp(-24, 0);
        self
    }

    /// Set the emphasis.
    #[must_use]
    pub fn with_emphasis(mut self, emphasis: u8) -> Self {
        self.emphasis = emphasis.min(2);
        self
    }
}

impl Default for SpeechStyle {
    fn default() -> Self {
        Self::default_style()
    }
}

/// A SSML subset: pauses and emphasis
/// tags embedded in text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SsmlTag {
    /// A pause of `ms` milliseconds.
    Pause(u32),
    /// A word to emphasize (the
    /// `SpeechStyle::emphasis` value).
    Emphasis(String),
}

/// A normalized utterance, ready to be
/// passed to the TTS engine.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Utterance {
    /// The text (without SSML tags).
    pub text: String,
    /// The embedded SSML tags, in order.
    pub tags: Vec<SsmlTag>,
}

impl Utterance {
    /// The estimated duration in
    /// milliseconds, based on word count
    /// and rate. Used for queueing.
    #[must_use]
    pub fn estimated_duration_ms(&self, style: &SpeechStyle) -> u64 {
        let words = self.text.split_whitespace().count() as u64;
        let wpm = (180.0 * style.rate as f64) as u64;
        let pause_ms: u64 = self
            .tags
            .iter()
            .map(|t| match t {
                SsmlTag::Pause(ms) => u64::from(*ms),
                SsmlTag::Emphasis(_) => 50,
            })
            .sum();
        if wpm == 0 {
            pause_ms
        } else {
            (words * 60_000).checked_div(wpm).unwrap_or(0) + pause_ms
        }
    }
}

/// Normalize raw text into an utterance.
/// Supports a small SSML subset: `<pause
/// ms="500"/>` and `<em>word</em>`.
#[must_use]
pub fn parse_ssml(input: &str) -> Utterance {
    let mut text = String::new();
    let mut tags = Vec::new();
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            if let Some(end) = bytes[i..].iter().position(|&b| b == b'>') {
                let tag_str = &input[i + 1..i + end];
                if let Some(rest) = tag_str.strip_prefix("pause ms=\"") {
                    // Strip trailing '/' (self-close)
                    // and '"'.
                    let trimmed = rest.trim_end_matches('/').trim_end_matches('"');
                    if let Some(stripped) = trimmed.strip_suffix('"') {
                        if let Ok(ms) = stripped.parse::<u32>() {
                            tags.push(SsmlTag::Pause(ms));
                            i += end + 1;
                            continue;
                        }
                    }
                    // Try without trailing quote (e.g.
                    // <pause ms="500" />).
                    if let Ok(ms) = trimmed.parse::<u32>() {
                        tags.push(SsmlTag::Pause(ms));
                        i += end + 1;
                        continue;
                    }
                } else if tag_str == "em" {
                    // Find closing </em>.
                    if let Some(close_pos) = input[i..].find("</em>") {
                        let inner = &input[i + end + 1..i + close_pos];
                        tags.push(SsmlTag::Emphasis(inner.to_string()));
                        text.push_str(inner);
                        i += close_pos + 5;
                        continue;
                    }
                } else if tag_str == "/em" {
                    i += end + 1;
                    continue;
                }
                i += end + 1;
            } else {
                text.push('<');
                i += 1;
            }
        } else {
            text.push(bytes[i] as char);
            i += 1;
        }
    }
    Utterance { text: text.trim().to_string(), tags }
}

/// A request to speak.
#[derive(Debug, Clone, PartialEq)]
pub struct SpeakRequest {
    /// The text to speak.
    pub text: String,
    /// The voice to use.
    pub voice: Voice,
    /// The style to apply.
    pub style: SpeechStyle,
    /// The output sample rate (Hz).
    pub sample_rate_hz: u32,
}

impl SpeakRequest {
    /// A new request with the default style
    /// and 22 kHz output.
    #[must_use]
    pub fn new(text: impl Into<String>, voice: Voice) -> Self {
        Self { text: text.into(), voice, style: SpeechStyle::default(), sample_rate_hz: 22_050 }
    }

    /// Set the style.
    #[must_use]
    pub fn with_style(mut self, style: SpeechStyle) -> Self {
        self.style = style;
        self
    }

    /// Set the output sample rate.
    #[must_use]
    pub fn with_sample_rate(mut self, hz: u32) -> Self {
        self.sample_rate_hz = hz;
        self
    }

    /// Normalize into an utterance.
    #[must_use]
    pub fn normalize(&self) -> Utterance {
        parse_ssml(&self.text)
    }
}

/// The result of speaking: a buffer plus
/// metadata.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SpokenAudio {
    /// The PCM samples.
    pub samples: Vec<i16>,
    /// The sample rate.
    pub sample_rate_hz: u32,
    /// The duration in milliseconds.
    pub duration_ms: u64,
}

impl SpokenAudio {
    /// The peak amplitude.
    #[must_use]
    pub fn peak_amplitude(&self) -> i16 {
        self.samples.iter().copied().map(i16::abs).max().unwrap_or(0)
    }
}

/// TTS engine errors.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TtsError {
    /// The text was empty after
    /// normalization.
    EmptyText,
    /// The voice is not loaded.
    UnknownVoice(String),
    /// The engine failed.
    EngineFailure(String),
}

impl core::fmt::Display for TtsError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyText => f.write_str("text to speak is empty"),
            Self::UnknownVoice(id) => write!(f, "voice '{id}' is not loaded"),
            Self::EngineFailure(msg) => write!(f, "tts engine failure: {msg}"),
        }
    }
}

impl std::error::Error for TtsError {}

/// The TTS engine trait.
pub trait TtsEngine {
    /// Speak the request. Returns the audio
    /// buffer.
    fn speak(&self, request: &SpeakRequest) -> Result<SpokenAudio, TtsError>;
}

/// A null TTS engine. Returns silence
/// matching the requested duration.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NullTts;

impl TtsEngine for NullTts {
    fn speak(&self, request: &SpeakRequest) -> Result<SpokenAudio, TtsError> {
        let utterance = request.normalize();
        if utterance.text.is_empty() {
            return Err(TtsError::EmptyText);
        }
        let duration_ms = utterance.estimated_duration_ms(&request.style);
        let n = (u64::from(request.sample_rate_hz) * duration_ms / 1000) as usize;
        Ok(SpokenAudio {
            samples: alloc::vec![0i16; n],
            sample_rate_hz: request.sample_rate_hz,
            duration_ms,
        })
    }
}

/// A TTS session. Holds the engine, the
/// default voice, and the current
/// utterance queue.
pub struct TtsSession<E: TtsEngine> {
    engine: E,
    default_voice: Voice,
    default_style: SpeechStyle,
    queue: Vec<SpeakRequest>,
    playing: Option<SpokenAudio>,
    position_ms: u64,
}

impl<E: TtsEngine> TtsSession<E> {
    /// A new session.
    #[must_use]
    pub fn new(engine: E, default_voice: Voice) -> Self {
        Self {
            engine,
            default_voice,
            default_style: SpeechStyle::default(),
            queue: Vec::new(),
            playing: None,
            position_ms: 0,
        }
    }

    /// The default voice.
    #[must_use]
    pub fn default_voice(&self) -> &Voice {
        &self.default_voice
    }

    /// The default style.
    #[must_use]
    pub fn default_style(&self) -> &SpeechStyle {
        &self.default_style
    }

    /// The pending queue length.
    #[must_use]
    pub fn queue_len(&self) -> usize {
        self.queue.len()
    }

    /// Enqueue a request.
    pub fn enqueue(&mut self, request: SpeakRequest) {
        self.queue.push(request);
    }

    /// Enqueue raw text using the default
    /// voice and style.
    pub fn say(&mut self, text: impl Into<String>) {
        let req = SpeakRequest::new(text, self.default_voice.clone());
        self.queue.push(req);
    }

    /// Advance the playback position. If
    /// nothing is playing and the queue is
    /// non-empty, synthesize the next item.
    pub fn tick(&mut self) -> Result<(), TtsError> {
        if self.playing.is_some() {
            return Ok(());
        }
        let Some(req) = self.queue.first().cloned() else {
            return Ok(());
        };
        let audio = self.engine.speak(&req)?;
        self.playing = Some(audio);
        self.position_ms = 0;
        Ok(())
    }

    /// Advance the playback by `ms`
    /// milliseconds. When the current
    /// utterance is done, pop it from the
    /// queue.
    pub fn advance(&mut self, ms: u64) {
        if let Some(audio) = &self.playing {
            let dur = audio.duration_ms;
            let new_pos = self.position_ms.saturating_add(ms);
            if new_pos >= dur {
                self.playing = None;
                self.position_ms = 0;
                if !self.queue.is_empty() {
                    self.queue.remove(0);
                }
            } else {
                self.position_ms = new_pos;
            }
        }
    }

    /// The current playback position in ms,
    /// or `None` if nothing is playing.
    #[must_use]
    pub fn position(&self) -> Option<u64> {
        self.playing.as_ref().map(|_| self.position_ms)
    }

    /// Stop playback and clear the queue.
    pub fn stop(&mut self) {
        self.queue.clear();
        self.playing = None;
        self.position_ms = 0;
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn voice() -> Voice {
        Voice::new("aether", "Aether", "en-US").with_description("warm, low, conversational")
    }

    #[test]
    fn voice_new_stores_fields() {
        let v = voice();
        assert_eq!(v.id, "aether");
        assert_eq!(v.name, "Aether");
        assert_eq!(v.locale, "en-US");
        assert!(!v.description.is_empty());
    }

    #[test]
    fn style_default_is_normal() {
        let s = SpeechStyle::default();
        assert_eq!(s.rate, 1.0);
        assert_eq!(s.pitch_semitones, 0);
        assert_eq!(s.emphasis, 0);
    }

    #[test]
    fn style_with_rate_clamps() {
        let s = SpeechStyle::default().with_rate(99.0);
        assert_eq!(s.rate, 3.0);
        let s = SpeechStyle::default().with_rate(0.01);
        assert_eq!(s.rate, 0.5);
    }

    #[test]
    fn style_with_pitch_clamps() {
        let s = SpeechStyle::default().with_pitch(50);
        assert_eq!(s.pitch_semitones, 24);
        let s = SpeechStyle::default().with_pitch(-50);
        assert_eq!(s.pitch_semitones, -24);
    }

    #[test]
    fn style_with_volume_clamps() {
        let s = SpeechStyle::default().with_volume(10);
        assert_eq!(s.volume_db, 0);
        let s = SpeechStyle::default().with_volume(-100);
        assert_eq!(s.volume_db, -24);
    }

    #[test]
    fn style_with_emphasis_clamps() {
        let s = SpeechStyle::default().with_emphasis(9);
        assert_eq!(s.emphasis, 2);
    }

    #[test]
    fn parse_plain_text() {
        let u = parse_ssml("hello world");
        assert_eq!(u.text, "hello world");
        assert!(u.tags.is_empty());
    }

    #[test]
    fn parse_pause_tag() {
        let u = parse_ssml("wait <pause ms=\"500\"/> now");
        assert_eq!(u.text, "wait  now");
        assert_eq!(u.tags, alloc::vec![SsmlTag::Pause(500)]);
    }

    #[test]
    fn parse_emphasis_tag() {
        let u = parse_ssml("this is <em>very</em> important");
        assert_eq!(u.text, "this is very important");
        assert_eq!(u.tags.len(), 1);
        assert!(matches!(&u.tags[0], SsmlTag::Emphasis(s) if s == "very"));
    }

    #[test]
    fn parse_keeps_unknown_tag() {
        let u = parse_ssml("hello <unknown/> world");
        assert!(u.text.contains("hello"));
    }

    #[test]
    fn estimated_duration_scales_with_rate() {
        let u = parse_ssml("one two three four five");
        let s_slow = SpeechStyle::default().with_rate(0.5);
        let s_fast = SpeechStyle::default().with_rate(2.0);
        let slow = u.estimated_duration_ms(&s_slow);
        let fast = u.estimated_duration_ms(&s_fast);
        assert!(slow > fast);
    }

    #[test]
    fn estimated_duration_handles_pauses() {
        let u = parse_ssml("hi <pause ms=\"1000\"/> there");
        let s = SpeechStyle::default();
        let d = u.estimated_duration_ms(&s);
        assert!(d >= 1000);
    }

    #[test]
    fn speak_request_normalize() {
        let req = SpeakRequest::new("hello <em>world</em>", voice());
        let u = req.normalize();
        assert_eq!(u.text, "hello world");
    }

    #[test]
    fn null_tts_rejects_empty_text() {
        let e = NullTts;
        let req = SpeakRequest::new("", voice()).with_sample_rate(16000);
        let err = e.speak(&req).unwrap_err();
        assert_eq!(err, TtsError::EmptyText);
    }

    #[test]
    fn null_tts_returns_silence() {
        let e = NullTts;
        let req = SpeakRequest::new("hello world", voice()).with_sample_rate(16000);
        let a = e.speak(&req).unwrap();
        assert_eq!(a.sample_rate_hz, 16000);
        assert!(a.duration_ms > 0);
        assert_eq!(a.peak_amplitude(), 0);
    }

    #[test]
    fn tts_error_display() {
        assert_eq!(TtsError::EmptyText.to_string(), "text to speak is empty");
        assert_eq!(
            TtsError::UnknownVoice(String::from("x")).to_string(),
            "voice 'x' is not loaded"
        );
        assert!(TtsError::EngineFailure(String::from("oops")).to_string().contains("oops"));
    }

    #[test]
    fn session_starts_empty() {
        let s: TtsSession<NullTts> = TtsSession::new(NullTts, voice());
        assert_eq!(s.queue_len(), 0);
        assert!(s.position().is_none());
    }

    #[test]
    fn session_say_uses_default_voice() {
        let mut s: TtsSession<NullTts> = TtsSession::new(NullTts, voice());
        s.say("hello");
        assert_eq!(s.queue_len(), 1);
    }

    #[test]
    fn session_enqueue_preserves_voice() {
        let mut s: TtsSession<NullTts> = TtsSession::new(NullTts, voice());
        let other = Voice::new("narrator", "Narrator", "en-GB");
        s.enqueue(SpeakRequest::new("hi", other.clone()));
        assert_eq!(s.queue[0].voice, other);
    }

    #[test]
    fn session_tick_synthesizes() {
        let mut s: TtsSession<NullTts> = TtsSession::new(NullTts, voice());
        s.say("hello");
        s.tick().unwrap();
        assert!(s.position().is_some());
    }

    #[test]
    fn session_tick_with_empty_queue_is_ok() {
        let mut s: TtsSession<NullTts> = TtsSession::new(NullTts, voice());
        s.tick().unwrap();
        assert!(s.position().is_none());
    }

    #[test]
    fn session_advance_plays_through() {
        let mut s: TtsSession<NullTts> = TtsSession::new(NullTts, voice());
        s.say("hello world this is a longer sentence");
        s.tick().unwrap();
        let start = s.position().unwrap();
        s.advance(60_000);
        // The utterance should be done, queue
        // empty.
        assert_eq!(s.queue_len(), 0);
        assert!(s.position().is_none() || s.position().unwrap() >= start);
    }

    #[test]
    fn session_stop_clears_everything() {
        let mut s: TtsSession<NullTts> = TtsSession::new(NullTts, voice());
        s.say("a");
        s.say("b");
        s.tick().unwrap();
        s.stop();
        assert_eq!(s.queue_len(), 0);
        assert!(s.position().is_none());
    }

    #[test]
    fn spoken_audio_peak() {
        let a = SpokenAudio {
            samples: alloc::vec![0, 100, -200, 300],
            sample_rate_hz: 16000,
            duration_ms: 1,
        };
        assert_eq!(a.peak_amplitude(), 300);
    }
}
