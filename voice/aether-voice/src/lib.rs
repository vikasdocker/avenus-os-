use std::fmt::{self, Display};

#[derive(Debug, Clone, PartialEq)]
pub struct Transcript {
    text: String,
    confidence: f32,
}

impl Transcript {
    pub fn new(text: impl Into<String>, confidence: f32) -> Result<Self, VoiceError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(VoiceError::EmptyTranscript);
        }
        if !(0.0..=1.0).contains(&confidence) {
            return Err(VoiceError::InvalidConfidence);
        }
        Ok(Self { text, confidence })
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn confidence(&self) -> f32 {
        self.confidence
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VoiceError {
    EmptyTranscript,
    InvalidConfidence,
}

impl Display for VoiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyTranscript => formatter.write_str("voice transcript must not be empty"),
            Self::InvalidConfidence => {
                formatter.write_str("voice confidence must be between 0 and 1")
            }
        }
    }
}

impl std::error::Error for VoiceError {}

#[cfg(test)]
mod tests {
    use super::Transcript;

    #[test]
    fn creates_transcript() {
        let transcript = Transcript::new("open settings", 0.91).unwrap_or_else(|error| {
            panic!("{error}");
        });
        assert_eq!(transcript.text(), "open settings");
    }
}
