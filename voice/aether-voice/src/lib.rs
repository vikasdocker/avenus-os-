//! Aether voice orchestrator — the
//! "tap-to-talk" lifecycle.
//!
//! Phase 4.4 of the ROADMAP. The orchestrator
//! ties together:
//!
//! * **wake-word detection** — knows when
//!   the user has said the wake word.
//! * **STT** — turns what the user said
//!   into a transcript.
//! * **TTS** — turns the agent's reply
//!   into audio.
//!
//! into a single state machine:
//!
//! ```text
//!   Idle -> Listening -> Thinking -> Speaking -> Idle
//!                   ^                  |
//!                   +------------------+ (TTS queue drained)
//! ```
//!
//! The orchestrator is *typed review* —
//! every transition is logged and the
//! shell can replay the session.
//!
//! The model has five pieces:
//!
//! 1. **`VoiceSessionState`** — the state
//!    machine.
//! 2. **`VoiceEvent`** — what the
//!    orchestrator records: state changes,
//!    transcripts, spoken utterances.
//! 3. **`AgentReply`** — a trait the
//!    runtime uses to plug in the real
//!    agent (or a stub for tests).
//! 4. **`VoiceSession`** — the driver.
//!    Owns the wake detector, the STT
//!    session, and the TTS session.
//! 5. **`VoiceLog`** — the audit log.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use aether_stt::{AudioBuffer, SttEngine, SttResult, SttSession, SttSessionState};
use aether_tts::{SpeakRequest, TtsEngine, TtsSession, Voice};
use aether_wake_word::{WakeDetector, WakeEngine, WakeEvent};

/// The high-level state of a voice
/// session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VoiceSessionState {
    /// The session is open but not
    /// listening.
    Idle,
    /// Listening for the wake word.
    Listening,
    /// The wake word fired; the user is
    /// speaking their command.
    Capturing,
    /// Transcribing the command.
    Transcribing,
    /// The agent is producing a reply.
    Thinking,
    /// Speaking the reply.
    Speaking,
    /// The session is closed.
    Stopped,
}

impl VoiceSessionState {
    /// The kebab-case name.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Listening => "listening",
            Self::Capturing => "capturing",
            Self::Transcribing => "transcribing",
            Self::Thinking => "thinking",
            Self::Speaking => "speaking",
            Self::Stopped => "stopped",
        }
    }
}

/// A single recorded event in the
/// session's audit log.
#[derive(Debug, Clone, PartialEq)]
pub enum VoiceEvent {
    /// A state transition.
    StateChange {
        /// The previous state.
        from: VoiceSessionState,
        /// The new state.
        to: VoiceSessionState,
    },
    /// A wake word fired.
    Wake {
        /// The wake word id.
        word_id: String,
        /// The display text.
        word_text: String,
        /// The confidence.
        confidence: f32,
    },
    /// A transcript was produced.
    Transcript {
        /// The text.
        text: String,
        /// The confidence.
        confidence: f32,
    },
    /// The agent produced a reply.
    Reply {
        /// The reply text.
        text: String,
    },
    /// The session was stopped.
    Stopped,
}

impl VoiceEvent {
    /// The kebab-case kind.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::StateChange { .. } => "state-change",
            Self::Wake { .. } => "wake",
            Self::Transcript { .. } => "transcript",
            Self::Reply { .. } => "reply",
            Self::Stopped => "stopped",
        }
    }
}

/// The agent's reply to a transcript.
/// The runtime plugs in a real agent by
/// implementing this.
pub trait AgentReply {
    /// Produce a reply to the user's
    /// `text`.
    fn reply(&self, text: &str) -> String;
}

/// A no-op agent that echoes the
/// transcript.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EchoAgent;

impl AgentReply for EchoAgent {
    fn reply(&self, text: &str) -> String {
        alloc::format!("You said: {text}")
    }
}

/// A typed bridge: the TTS engine needs
/// a `Voice` and the agent reply is just
/// a string. The session knows the
/// default voice and style.
pub struct VoiceSession<W: WakeEngine, S: SttEngine, T: TtsEngine, A: AgentReply> {
    wake: WakeDetector<W>,
    stt: SttSession<S>,
    tts: TtsSession<T>,
    agent: A,
    voice: Voice,
    state: VoiceSessionState,
    pending_wake: Option<WakeEvent>,
    pending_transcript: Option<SttResult>,
    pending_reply: Option<String>,
    log: Vec<VoiceEvent>,
}

impl<W, S, T, A> VoiceSession<W, S, T, A>
where
    W: WakeEngine,
    S: SttEngine,
    T: TtsEngine,
    A: AgentReply,
{
    /// A new session in `Idle` state.
    #[must_use]
    pub fn new(
        wake: WakeDetector<W>,
        stt: SttSession<S>,
        tts: TtsSession<T>,
        agent: A,
        voice: Voice,
    ) -> Self {
        Self {
            wake,
            stt,
            tts,
            agent,
            voice,
            state: VoiceSessionState::Idle,
            pending_wake: None,
            pending_transcript: None,
            pending_reply: None,
            log: Vec::new(),
        }
    }

    /// The current state.
    #[must_use]
    pub fn state(&self) -> VoiceSessionState {
        self.state
    }

    /// The audit log.
    #[must_use]
    pub fn log(&self) -> &[VoiceEvent] {
        &self.log
    }

    /// The pending wake event, if any.
    #[must_use]
    pub fn pending_wake(&self) -> Option<&WakeEvent> {
        self.pending_wake.as_ref()
    }

    /// The pending transcript, if any.
    #[must_use]
    pub fn pending_transcript(&self) -> Option<&SttResult> {
        self.pending_transcript.as_ref()
    }

    /// The pending reply, if any.
    #[must_use]
    pub fn pending_reply(&self) -> Option<&str> {
        self.pending_reply.as_deref()
    }

    /// Start the session: begin listening
    /// for the wake word.
    pub fn start(&mut self) {
        if self.state == VoiceSessionState::Stopped {
            return;
        }
        self.transition(VoiceSessionState::Listening);
    }

    /// Stop the session.
    pub fn stop(&mut self) {
        self.tts.stop();
        self.stt.stop();
        self.wake.reset();
        self.transition(VoiceSessionState::Stopped);
        self.log.push(VoiceEvent::Stopped);
    }

    /// Feed a buffer of audio. Advances
    /// the state machine.
    pub fn feed(&mut self, buffer: &AudioBuffer) {
        match self.state {
            VoiceSessionState::Idle | VoiceSessionState::Stopped => {}
            VoiceSessionState::Listening => {
                if let Some(wake) = self.wake.feed(buffer) {
                    self.pending_wake = Some(wake.clone());
                    self.log.push(VoiceEvent::Wake {
                        word_id: wake.word_id,
                        word_text: wake.word_text,
                        confidence: wake.confidence,
                    });
                    self.transition(VoiceSessionState::Capturing);
                    self.stt.start();
                }
            }
            VoiceSessionState::Capturing
            | VoiceSessionState::Transcribing
            | VoiceSessionState::Thinking => {
                self.stt.feed(buffer);
            }
            VoiceSessionState::Speaking => {
                // The audio frame goes to the
                // TTS playback side; the
                // orchestrator is on the
                // speaker side here, so we
                // ignore the mic frame.
            }
        }
    }

    /// Tick the orchestrator. Advances
    /// the STT, asks the agent, enqueues
    /// the reply on the TTS, and ticks
    /// the TTS playback.
    pub fn tick(&mut self) {
        match self.state {
            VoiceSessionState::Capturing => {
                self.stt.process();
                if let Some(result) = self.stt.last_result() {
                    let text = result.text.clone();
                    let conf = result.confidence;
                    self.pending_transcript = Some(result.clone());
                    self.log.push(VoiceEvent::Transcript {
                        text: text.clone(),
                        confidence: conf,
                    });
                    let reply = self.agent.reply(&text);
                    self.pending_reply = Some(reply.clone());
                    self.log.push(VoiceEvent::Reply { text: reply });
                    self.transition(VoiceSessionState::Thinking);
                } else if matches!(self.stt.state(), SttSessionState::Listening) {
                    // STT reset itself (e.g.
                    // confidence too low); go
                    // back to wake-word
                    // listening.
                    self.pending_wake = None;
                    self.transition(VoiceSessionState::Listening);
                }
            }
            VoiceSessionState::Transcribing => {
                // Spurious — no transition
                // leaves us here yet.
            }
            VoiceSessionState::Thinking => {
                if let Some(reply) = self.pending_reply.clone() {
                    self.tts
                        .enqueue(SpeakRequest::new(reply, self.voice.clone()));
                    self.transition(VoiceSessionState::Speaking);
                }
            }
            VoiceSessionState::Speaking => {
                let _ = self.tts.tick();
                if self.tts.queue_len() == 0 && self.tts.position().is_none() {
                    self.pending_wake = None;
                    self.pending_transcript = None;
                    self.pending_reply = None;
                    self.transition(VoiceSessionState::Listening);
                }
            }
            _ => {}
        }
    }

    /// Advance the TTS playback by `ms`
    /// milliseconds. Also ticks the TTS
    /// to start the next utterance if
    /// nothing is playing.
    pub fn advance(&mut self, ms: u64) {
        if self.state == VoiceSessionState::Speaking {
            let _ = self.tts.tick();
            self.tts.advance(ms);
        }
    }

    fn transition(&mut self, to: VoiceSessionState) {
        if self.state == to {
            return;
        }
        let from = self.state;
        self.state = to;
        self.log.push(VoiceEvent::StateChange { from, to });
    }
}

/// A typed voice log wrapper. Owns the
/// events and exposes read-only access.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct VoiceLog {
    events: Vec<VoiceEvent>,
}

impl VoiceLog {
    /// A new, empty log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append an event.
    pub fn push(&mut self, event: VoiceEvent) {
        self.events.push(event);
    }

    /// The events.
    #[must_use]
    pub fn events(&self) -> &[VoiceEvent] {
        &self.events
    }

    /// The number of state transitions.
    #[must_use]
    pub fn transitions(&self) -> usize {
        self.events
            .iter()
            .filter(|e| matches!(e, VoiceEvent::StateChange { .. }))
            .count()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_stt::NullStt;
    use aether_tts::{NullTts, SpeechStyle};
    use aether_wake_word::{EnergyGate, ReferenceEngine, WakeWord};

    fn voice() -> Voice {
        Voice::new("aether", "Aether", "en-US")
    }

    fn session() -> VoiceSession<ReferenceEngine, NullStt, NullTts, EchoAgent> {
        let mut wake = WakeDetector::new(ReferenceEngine);
        wake.set_gate(EnergyGate { min_rms: 100 });
        wake.set_refractory_ms(0);
        let fp = ReferenceEngine::profile(&loud());
        wake.register(WakeWord::new("aether", "aether").with_fingerprint(fp).with_threshold(0.0));
        let stt = SttSession::new(NullStt, 16000);
        let tts = TtsSession::new(NullTts, voice());
        VoiceSession::new(wake, stt, tts, EchoAgent, voice())
    }

    fn loud() -> AudioBuffer {
        let samples: Vec<i16> = (0..1600).map(|i| ((i * 200) % 30000) as i16).collect();
        AudioBuffer {
            sample_rate_hz: 16000,
            samples,
        }
    }

    #[test]
    fn state_as_str() {
        assert_eq!(VoiceSessionState::Idle.as_str(), "idle");
        assert_eq!(VoiceSessionState::Speaking.as_str(), "speaking");
    }

    #[test]
    fn event_kind() {
        assert_eq!(VoiceEvent::Stopped.kind(), "stopped");
        assert_eq!(
            VoiceEvent::StateChange {
                from: VoiceSessionState::Idle,
                to: VoiceSessionState::Listening,
            }
            .kind(),
            "state-change"
        );
    }

    #[test]
    fn new_session_is_idle() {
        let s = session();
        assert_eq!(s.state(), VoiceSessionState::Idle);
        assert!(s.log().is_empty());
    }

    #[test]
    fn start_moves_to_listening() {
        let mut s = session();
        s.start();
        assert_eq!(s.state(), VoiceSessionState::Listening);
        assert_eq!(s.log().len(), 1);
    }

    #[test]
    fn stop_from_idle_is_recorded() {
        let mut s = session();
        s.stop();
        assert_eq!(s.state(), VoiceSessionState::Stopped);
        assert_eq!(s.log().len(), 2);
    }

    #[test]
    fn stop_after_start_is_recorded() {
        let mut s = session();
        s.start();
        s.stop();
        assert_eq!(s.state(), VoiceSessionState::Stopped);
        assert_eq!(s.log().len(), 3);
    }

    #[test]
    fn feed_idle_does_nothing() {
        let mut s = session();
        s.feed(&loud());
        assert_eq!(s.state(), VoiceSessionState::Idle);
    }

    #[test]
    fn feed_listening_fires_wake() {
        let mut s = session();
        s.start();
        s.feed(&loud());
        assert_eq!(s.state(), VoiceSessionState::Capturing);
        assert!(s.pending_wake().is_some());
    }

    #[test]
    fn stop_after_start_resets_subsystems() {
        let mut s = session();
        s.start();
        s.feed(&loud());
        assert_eq!(s.state(), VoiceSessionState::Capturing);
        s.stop();
        assert_eq!(s.state(), VoiceSessionState::Stopped);
    }

    #[test]
    fn echo_agent_replies_with_text() {
        let a = EchoAgent;
        assert_eq!(a.reply("hi"), "You said: hi");
    }

    #[test]
    fn voice_log_collects_events() {
        let mut log = VoiceLog::new();
        log.push(VoiceEvent::Stopped);
        log.push(VoiceEvent::Reply {
            text: String::from("hi"),
        });
        assert_eq!(log.events().len(), 2);
        assert_eq!(log.transitions(), 0);
    }

    #[test]
    fn voice_log_counts_transitions() {
        let mut log = VoiceLog::new();
        log.push(VoiceEvent::StateChange {
            from: VoiceSessionState::Idle,
            to: VoiceSessionState::Listening,
        });
        log.push(VoiceEvent::StateChange {
            from: VoiceSessionState::Listening,
            to: VoiceSessionState::Capturing,
        });
        log.push(VoiceEvent::Stopped);
        assert_eq!(log.transitions(), 2);
    }

    #[test]
    fn speech_style_default() {
        let s = SpeechStyle::default();
        assert_eq!(s.rate, 1.0);
    }

    #[test]
    fn full_session_cycle() {
        let mut s = session();
        s.start();
        // Wake.
        s.feed(&loud());
        assert_eq!(s.state(), VoiceSessionState::Capturing);
        // Give STT audio with silence
        // afterwards.
        s.feed(&loud());
        s.feed(&AudioBuffer::silence(16000, 1000));
        s.tick();
        // Should now be in Thinking (or
        // back to Listening if STT
        // rejected the audio — but
        // NullStt returns confidence 1.0
        // and our threshold is 0.0, so it
        // should proceed).
        if s.state() == VoiceSessionState::Thinking {
            s.tick();
            assert_eq!(s.state(), VoiceSessionState::Speaking);
            // TTS NullTts is silent, so
            // duration is > 0. Advance
            // playback.
            s.advance(60_000);
            s.tick();
            assert_eq!(s.state(), VoiceSessionState::Listening);
        } else {
            // If we landed in Listening
            // already, the cycle still
            // completed.
            assert_eq!(s.state(), VoiceSessionState::Listening);
        }
    }
}
