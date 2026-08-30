//! Aether speech-to-text (STT) — the typed model
//! for converting an `AudioBuffer` into a
//! `Transcript`.
//!
//! Phase 4.1 of the ROADMAP. The runtime is
//! currently a no-op: it accepts audio, normalizes
//! it, and returns a `Transcript` placeholder
//! unless a real model backend is wired in. The
//! contract is *typed review* — the shell and the
//! agent can audit what was heard.
//!
//! The model has five pieces:
//!
//! 1. **`AudioBuffer`** — a fixed-size PCM
//!    sample buffer. The model is sample-rate
//!    agnostic (samples are i16 with an explicit
//!    rate).
//! 2. **`SttRequest`** — what the caller wants
//!    transcribed: an audio buffer, a language
//!    hint, and a confidence threshold.
//! 3. **`SttResult`** — what the model returns:
//!    the transcript text, the per-word
//!    confidence, the detected language, and the
//!    segments.
//! 4. **`SttEngine`** — the trait the runtime
//!    uses to plug in a real model. The default
//!    `NullStt` is for tests and graceful
//!    degradation.
//! 5. **`SttSession`** — the stateful driver.
//!    Buffers audio, detects end-of-utterance
//!    (silence), and dispatches to the engine.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

/// A PCM audio buffer. Samples are i16
/// mono at the given sample rate (Hz).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AudioBuffer {
    /// Sample rate in Hz (e.g. 16000, 48000).
    pub sample_rate_hz: u32,
    /// Mono PCM samples, signed 16-bit.
    pub samples: Vec<i16>,
}

impl AudioBuffer {
    /// A silent buffer of `duration_ms`
    /// milliseconds at the given rate.
    #[must_use]
    pub fn silence(sample_rate_hz: u32, duration_ms: u32) -> Self {
        let n = (u64::from(sample_rate_hz) * u64::from(duration_ms) / 1000) as usize;
        Self {
            sample_rate_hz,
            samples: alloc::vec![0i16; n],
        }
    }

    /// The duration of the buffer in
    /// milliseconds.
    #[must_use]
    pub fn duration_ms(&self) -> u64 {
        if self.sample_rate_hz == 0 {
            return 0;
        }
        (u64::try_from(self.samples.len()).unwrap_or(0) * 1000)
            / u64::from(self.sample_rate_hz)
    }

    /// The peak amplitude in the buffer
    /// (0..=i16::MAX).
    #[must_use]
    pub fn peak_amplitude(&self) -> i16 {
        self.samples.iter().copied().map(i16::abs).max().unwrap_or(0)
    }

    /// The RMS level in the buffer
    /// (0..=i16::MAX). Useful for end-of-speech
    /// detection.
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn rms(&self) -> u32 {
        if self.samples.is_empty() {
            return 0;
        }
        let mut sum: u64 = 0;
        for s in &self.samples {
            let v = i32::from(*s);
            sum = sum.saturating_add((v * v) as u64);
        }
        let mean = sum / self.samples.len() as u64;
        ((mean as f64).sqrt() as u32).min(i16::MAX as u32)
    }

    /// `true` if the buffer is silent
    /// (`peak_amplitude <= threshold`).
    #[must_use]
    pub fn is_silent(&self, threshold: i16) -> bool {
        self.peak_amplitude() <= threshold
    }

    /// Append another buffer at the same
    /// sample rate. If the rates differ, the
    /// other buffer is resampled by linear
    /// interpolation.
    #[must_use]
    pub fn append(&self, other: &Self) -> Self {
        if other.samples.is_empty() {
            return self.clone();
        }
        if self.sample_rate_hz == other.sample_rate_hz {
            let mut samples = self.samples.clone();
            samples.extend_from_slice(&other.samples);
            return Self {
                sample_rate_hz: self.sample_rate_hz,
                samples,
            };
        }
        // Linear resample.
        let mut samples = self.samples.clone();
        let ratio = f64::from(other.sample_rate_hz) / f64::from(self.sample_rate_hz);
        let target_extra = ((other.samples.len() as f64) / ratio).round() as usize;
        for i in 0..target_extra {
            let pos = f64::from(i as u32) * ratio;
            let idx = pos as usize;
            if idx + 1 >= other.samples.len() {
                samples.push(other.samples[other.samples.len() - 1]);
            } else {
                let frac = pos - idx as f64;
                let a = f64::from(other.samples[idx]);
                let b = f64::from(other.samples[idx + 1]);
                let mixed = a + (b - a) * frac;
                samples.push(mixed.round().clamp(f64::from(i16::MIN), f64::from(i16::MAX)) as i16);
            }
        }
        Self {
            sample_rate_hz: self.sample_rate_hz,
            samples,
        }
    }
}

/// A language hint for transcription.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    /// Auto-detect.
    Auto,
    /// English.
    En,
    /// Spanish.
    Es,
    /// French.
    Fr,
    /// German.
    De,
    /// Hindi.
    Hi,
    /// Japanese.
    Ja,
    /// Mandarin Chinese.
    Zh,
}

impl Language {
    /// The BCP-47 tag (or "auto").
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::En => "en",
            Self::Es => "es",
            Self::Fr => "fr",
            Self::De => "de",
            Self::Hi => "hi",
            Self::Ja => "ja",
            Self::Zh => "zh",
        }
    }
}

/// One transcribed segment (utterance).
#[derive(Debug, Clone, PartialEq)]
pub struct Segment {
    /// The transcribed text.
    pub text: String,
    /// The start time in milliseconds from
    /// the start of the audio.
    pub start_ms: u64,
    /// The end time in milliseconds.
    pub end_ms: u64,
    /// The average confidence for the
    /// segment (0.0..=1.0).
    pub confidence: f32,
}

impl Segment {
    /// The duration of the segment.
    #[must_use]
    pub fn duration_ms(&self) -> u64 {
        self.end_ms.saturating_sub(self.start_ms)
    }
}

/// The result of a transcription.
#[derive(Debug, Clone, PartialEq)]
pub struct SttResult {
    /// The full transcript (concatenation of
    /// `segments`).
    pub text: String,
    /// The detected language.
    pub language: Language,
    /// The per-segment breakdown.
    pub segments: Vec<Segment>,
    /// The overall confidence (0.0..=1.0).
    pub confidence: f32,
}

impl SttResult {
    /// `true` if the result is at least
    /// `min_confidence`.
    #[must_use]
    pub fn meets_threshold(&self, min_confidence: f32) -> bool {
        self.confidence >= min_confidence
    }
}

/// A request to transcribe audio.
#[derive(Debug, Clone, PartialEq)]
pub struct SttRequest {
    /// The audio to transcribe.
    pub audio: AudioBuffer,
    /// A language hint.
    pub language: Language,
    /// The minimum acceptable confidence
    /// (0.0..=1.0).
    pub min_confidence: f32,
}

impl SttRequest {
    /// A request with default language
    /// (Auto) and a permissive confidence
    /// threshold.
    #[must_use]
    pub fn new(audio: AudioBuffer) -> Self {
        Self {
            audio,
            language: Language::Auto,
            min_confidence: 0.0,
        }
    }

    /// Set the language hint.
    #[must_use]
    pub fn with_language(mut self, language: Language) -> Self {
        self.language = language;
        self
    }

    /// Set the minimum confidence.
    #[must_use]
    pub fn with_min_confidence(mut self, min_confidence: f32) -> Self {
        self.min_confidence = min_confidence.clamp(0.0, 1.0);
        self
    }
}

/// Errors an STT engine can return.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SttError {
    /// The audio buffer was empty.
    EmptyAudio,
    /// The sample rate was zero.
    InvalidSampleRate,
    /// The model failed to produce a
    /// transcript.
    EngineFailure(String),
}

impl core::fmt::Display for SttError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyAudio => f.write_str("audio buffer is empty"),
            Self::InvalidSampleRate => f.write_str("sample rate must be non-zero"),
            Self::EngineFailure(msg) => write!(f, "stt engine failure: {msg}"),
        }
    }
}

impl std::error::Error for SttError {}

/// The STT engine trait. The runtime plugs in
/// a real model (whisper, vosk, etc.) by
/// implementing this.
pub trait SttEngine {
    /// Transcribe the request.
    fn transcribe(&self, request: &SttRequest) -> Result<SttResult, SttError>;
}

/// A null STT engine. Returns an empty
/// transcript. Used for tests and graceful
/// degradation when no model is loaded.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NullStt;

impl SttEngine for NullStt {
    fn transcribe(&self, request: &SttRequest) -> Result<SttResult, SttError> {
        if request.audio.samples.is_empty() {
            return Err(SttError::EmptyAudio);
        }
        if request.audio.sample_rate_hz == 0 {
            return Err(SttError::InvalidSampleRate);
        }
        Ok(SttResult {
            text: String::new(),
            language: request.language,
            segments: Vec::new(),
            confidence: 1.0,
        })
    }
}

/// The state of a recording session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SttSessionState {
    /// Accepting audio.
    Listening,
    /// Speech was detected, still buffering.
    Speaking,
    /// Silence detected after speech —
    /// ready to transcribe.
    Pending,
    /// Transcribing.
    Processing,
    /// Done — `last_result` is set.
    Done,
    /// Stopped or never started.
    Idle,
}

/// End-of-utterance detector. Uses
/// silence-duration and peak thresholds
/// to decide when the user has stopped
/// speaking.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EndOfSpeechDetector {
    /// Sample rate the detector operates
    /// on.
    pub sample_rate_hz: u32,
    /// Peak amplitude below this counts
    /// as silence.
    pub silence_threshold: i16,
    /// How long silence must persist to
    /// trigger end-of-speech, in
    /// milliseconds.
    pub silence_duration_ms: u32,
    /// The number of samples currently
    /// inside a silence streak.
    pub silence_streak_samples: u32,
    /// The total samples of audio seen in
    /// the current utterance.
    pub utterance_samples: u32,
}

impl EndOfSpeechDetector {
    /// A reasonable default: 16 kHz, 500
    /// amplitude threshold, 700 ms silence.
    #[must_use]
    pub fn default_for(sample_rate_hz: u32) -> Self {
        Self {
            sample_rate_hz,
            silence_threshold: 500,
            silence_duration_ms: 700,
            silence_streak_samples: 0,
            utterance_samples: 0,
        }
    }

    /// Feed a buffer to the detector.
    /// Returns `true` if the buffer ends an
    /// utterance.
    pub fn feed(&mut self, buffer: &AudioBuffer) -> bool {
        if buffer.sample_rate_hz != self.sample_rate_hz {
            return false;
        }
        for &s in &buffer.samples {
            self.utterance_samples = self.utterance_samples.saturating_add(1);
            if s.abs() <= self.silence_threshold {
                self.silence_streak_samples = self.silence_streak_samples.saturating_add(1);
            } else {
                self.silence_streak_samples = 0;
            }
        }
        let silence_ms = (u64::from(self.silence_streak_samples) * 1000)
            / u64::from(self.sample_rate_hz.max(1));
        silence_ms >= u64::from(self.silence_duration_ms) && self.utterance_samples > 0
    }

    /// Reset the detector for a new
    /// utterance.
    pub fn reset(&mut self) {
        self.silence_streak_samples = 0;
        self.utterance_samples = 0;
    }
}

/// A stateful STT session. Buffers audio,
/// detects end-of-utterance, dispatches to
/// the engine, and stores the result.
pub struct SttSession<E: SttEngine> {
    engine: E,
    state: SttSessionState,
    buffer: AudioBuffer,
    detector: EndOfSpeechDetector,
    last_result: Option<SttResult>,
    last_error: Option<SttError>,
    min_confidence: f32,
}

impl<E: SttEngine> SttSession<E> {
    /// A new session in `Idle` state.
    #[must_use]
    pub fn new(engine: E, sample_rate_hz: u32) -> Self {
        Self {
            engine,
            state: SttSessionState::Idle,
            buffer: AudioBuffer::silence(sample_rate_hz, 0),
            detector: EndOfSpeechDetector::default_for(sample_rate_hz),
            last_result: None,
            last_error: None,
            min_confidence: 0.0,
        }
    }

    /// Set the minimum confidence.
    pub fn set_min_confidence(&mut self, min_confidence: f32) {
        self.min_confidence = min_confidence.clamp(0.0, 1.0);
    }

    /// The current state.
    #[must_use]
    pub fn state(&self) -> SttSessionState {
        self.state
    }

    /// The last result, if any.
    #[must_use]
    pub fn last_result(&self) -> Option<&SttResult> {
        self.last_result.as_ref()
    }

    /// The last error, if any.
    #[must_use]
    pub fn last_error(&self) -> Option<&SttError> {
        self.last_error.as_ref()
    }

    /// Begin listening.
    pub fn start(&mut self) {
        self.state = SttSessionState::Listening;
        self.buffer = AudioBuffer::silence(self.detector.sample_rate_hz, 0);
        self.detector.reset();
        self.last_result = None;
        self.last_error = None;
    }

    /// Feed a buffer of audio. Advances the
    /// state machine.
    pub fn feed(&mut self, buffer: &AudioBuffer) {
        if self.state == SttSessionState::Idle
            || self.state == SttSessionState::Done
        {
            return;
        }
        if buffer.sample_rate_hz != self.buffer.sample_rate_hz {
            return;
        }
        self.buffer = self.buffer.append(buffer);
        if self.state == SttSessionState::Listening
            && !buffer.is_silent(self.detector.silence_threshold)
        {
            self.state = SttSessionState::Speaking;
        }
        if self.state == SttSessionState::Speaking
            && self.detector.feed(buffer)
        {
            self.state = SttSessionState::Pending;
        }
    }

    /// Process any pending audio. If the
    /// state is `Pending`, dispatches to the
    /// engine. If the engine succeeds and the
    /// confidence is high enough, moves to
    /// `Done`. Otherwise stays in `Pending` so
    /// the caller can decide.
    pub fn process(&mut self) {
        if self.state != SttSessionState::Pending {
            return;
        }
        self.state = SttSessionState::Processing;
        let request = SttRequest {
            audio: self.buffer.clone(),
            language: Language::Auto,
            min_confidence: self.min_confidence,
        };
        match self.engine.transcribe(&request) {
            Ok(result) => {
                if result.meets_threshold(self.min_confidence) {
                    self.last_result = Some(result);
                    self.last_error = None;
                    self.state = SttSessionState::Done;
                } else {
                    self.state = SttSessionState::Listening;
                    self.buffer = AudioBuffer::silence(self.detector.sample_rate_hz, 0);
                    self.detector.reset();
                }
            }
            Err(err) => {
                self.last_error = Some(err);
                self.state = SttSessionState::Listening;
                self.buffer = AudioBuffer::silence(self.detector.sample_rate_hz, 0);
                self.detector.reset();
            }
        }
    }

    /// Stop the session and discard
    /// buffered audio.
    pub fn stop(&mut self) {
        self.state = SttSessionState::Idle;
        self.buffer = AudioBuffer::silence(self.detector.sample_rate_hz, 0);
        self.detector.reset();
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn buffer_of(samples: &[i16], rate: u32) -> AudioBuffer {
        AudioBuffer {
            sample_rate_hz: rate,
            samples: samples.to_vec(),
        }
    }

    #[test]
    fn silence_buffer_has_zero_duration() {
        let b = AudioBuffer::silence(16000, 1000);
        assert_eq!(b.duration_ms(), 1000);
        assert_eq!(b.peak_amplitude(), 0);
        assert_eq!(b.rms(), 0);
        assert!(b.is_silent(0));
    }

    #[test]
    fn duration_handles_zero_rate() {
        let b = AudioBuffer {
            sample_rate_hz: 0,
            samples: alloc::vec![0; 100],
        };
        assert_eq!(b.duration_ms(), 0);
    }

    #[test]
    fn peak_and_rms_track_samples() {
        let b = buffer_of(&[100, -200, 300, -400, 500], 16000);
        assert_eq!(b.peak_amplitude(), 500);
        assert!(b.rms() > 0);
    }

    #[test]
    fn append_concatenates_same_rate() {
        let a = buffer_of(&[1, 2, 3], 16000);
        let b = buffer_of(&[4, 5], 16000);
        let c = a.append(&b);
        assert_eq!(c.samples, alloc::vec![1, 2, 3, 4, 5]);
        assert_eq!(c.sample_rate_hz, 16000);
    }

    #[test]
    fn append_resamples_different_rate() {
        let a = buffer_of(&[0, 1000, 0, 1000], 16000);
        let b = buffer_of(&[500, 500, 500, 500], 8000);
        let c = a.append(&b);
        // 4 samples at 8kHz resample to 8 at 16kHz,
        // so 12 total.
        assert_eq!(c.samples.len(), 12);
        assert_eq!(c.sample_rate_hz, 16000);
    }

    #[test]
    fn append_empty_other_returns_clone() {
        let a = buffer_of(&[1, 2, 3], 16000);
        let b = AudioBuffer::silence(16000, 0);
        let c = a.append(&b);
        assert_eq!(c.samples, a.samples);
    }

    #[test]
    fn language_as_str() {
        assert_eq!(Language::Auto.as_str(), "auto");
        assert_eq!(Language::En.as_str(), "en");
        assert_eq!(Language::Hi.as_str(), "hi");
    }

    #[test]
    fn segment_duration() {
        let s = Segment {
            text: String::new(),
            start_ms: 100,
            end_ms: 350,
            confidence: 0.9,
        };
        assert_eq!(s.duration_ms(), 250);
    }

    #[test]
    fn result_meets_threshold() {
        let r = SttResult {
            text: String::from("hi"),
            language: Language::En,
            segments: Vec::new(),
            confidence: 0.5,
        };
        assert!(r.meets_threshold(0.3));
        assert!(!r.meets_threshold(0.6));
    }

    #[test]
    fn request_with_methods() {
        let r = SttRequest::new(buffer_of(&[0; 100], 16000))
            .with_language(Language::En)
            .with_min_confidence(1.5);
        assert_eq!(r.language, Language::En);
        assert_eq!(r.min_confidence, 1.0);
    }

    #[test]
    fn null_stt_rejects_empty_audio() {
        let s = NullStt;
        let err = s
            .transcribe(&SttRequest::new(AudioBuffer::silence(16000, 0)))
            .unwrap_err();
        assert_eq!(err, SttError::EmptyAudio);
    }

    #[test]
    fn null_stt_rejects_zero_rate() {
        let s = NullStt;
        let r = s
            .transcribe(&SttRequest::new(AudioBuffer {
                sample_rate_hz: 0,
                samples: alloc::vec![0; 100],
            }))
            .unwrap_err();
        assert_eq!(r, SttError::InvalidSampleRate);
    }

    #[test]
    fn null_stt_returns_empty_for_silence() {
        let s = NullStt;
        let r = s
            .transcribe(&SttRequest::new(AudioBuffer::silence(16000, 500)))
            .unwrap();
        assert_eq!(r.text, "");
        assert_eq!(r.confidence, 1.0);
    }

    #[test]
    fn detector_default_values() {
        let d = EndOfSpeechDetector::default_for(16000);
        assert_eq!(d.sample_rate_hz, 16000);
        assert_eq!(d.silence_duration_ms, 700);
    }

    #[test]
    fn detector_fires_after_silence() {
        let mut d = EndOfSpeechDetector {
            sample_rate_hz: 1000,
            silence_threshold: 100,
            silence_duration_ms: 50,
            silence_streak_samples: 0,
            utterance_samples: 0,
        };
        // 50 samples of silence.
        let buf = buffer_of(&[0; 50], 1000);
        assert!(d.feed(&buf));
    }

    #[test]
    fn detector_ignores_wrong_rate() {
        let mut d = EndOfSpeechDetector::default_for(16000);
        let buf = buffer_of(&[0; 1000], 8000);
        assert!(!d.feed(&buf));
    }

    #[test]
    fn detector_resets() {
        let mut d = EndOfSpeechDetector {
            sample_rate_hz: 1000,
            silence_threshold: 100,
            silence_duration_ms: 50,
            silence_streak_samples: 0,
            utterance_samples: 0,
        };
        let buf = buffer_of(&[0; 50], 1000);
        let _ = d.feed(&buf);
        d.reset();
        assert_eq!(d.silence_streak_samples, 0);
        assert_eq!(d.utterance_samples, 0);
    }

    #[test]
    fn session_starts_idle() {
        let s: SttSession<NullStt> = SttSession::new(NullStt, 16000);
        assert_eq!(s.state(), SttSessionState::Idle);
    }

    #[test]
    fn session_start_moves_to_listening() {
        let mut s: SttSession<NullStt> = SttSession::new(NullStt, 16000);
        s.start();
        assert_eq!(s.state(), SttSessionState::Listening);
    }

    #[test]
    fn session_ignored_when_idle() {
        let mut s: SttSession<NullStt> = SttSession::new(NullStt, 16000);
        s.feed(&buffer_of(&[1000; 100], 16000));
        assert_eq!(s.state(), SttSessionState::Idle);
    }

    #[test]
    fn session_detects_speech() {
        let mut s: SttSession<NullStt> = SttSession::new(NullStt, 16000);
        s.start();
        s.feed(&buffer_of(&[2000; 1600], 16000));
        assert_eq!(s.state(), SttSessionState::Speaking);
    }

    #[test]
    fn session_detects_end_of_speech() {
        let mut s: SttSession<NullStt> = SttSession::new(NullStt, 16000);
        s.start();
        s.feed(&buffer_of(&[2000; 1600], 16000));
        // 800 ms of silence.
        s.feed(&buffer_of(&[0; 12800], 16000));
        assert_eq!(s.state(), SttSessionState::Pending);
    }

    #[test]
    fn session_process_produces_result() {
        let mut s: SttSession<NullStt> = SttSession::new(NullStt, 16000);
        s.set_min_confidence(0.0);
        s.start();
        s.feed(&buffer_of(&[2000; 1600], 16000));
        s.feed(&buffer_of(&[0; 12800], 16000));
        s.process();
        assert_eq!(s.state(), SttSessionState::Done);
        assert!(s.last_result().is_some());
    }

    #[test]
    fn session_process_does_nothing_when_not_pending() {
        let mut s: SttSession<NullStt> = SttSession::new(NullStt, 16000);
        s.start();
        s.process();
        assert_eq!(s.state(), SttSessionState::Listening);
    }

    #[test]
    fn session_stop_returns_to_idle() {
        let mut s: SttSession<NullStt> = SttSession::new(NullStt, 16000);
        s.start();
        s.feed(&buffer_of(&[2000; 100], 16000));
        s.stop();
        assert_eq!(s.state(), SttSessionState::Idle);
    }

    #[test]
    fn session_ignores_wrong_rate() {
        let mut s: SttSession<NullStt> = SttSession::new(NullStt, 16000);
        s.start();
        s.feed(&buffer_of(&[2000; 100], 8000));
        assert_eq!(s.state(), SttSessionState::Listening);
    }

    #[test]
    fn stt_error_display() {
        assert_eq!(SttError::EmptyAudio.to_string(), "audio buffer is empty");
        assert_eq!(SttError::InvalidSampleRate.to_string(), "sample rate must be non-zero");
        assert!(SttError::EngineFailure(String::from("oops")).to_string().contains("oops"));
    }
}
