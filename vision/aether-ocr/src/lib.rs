//! Aether OCR — text extraction from a
//! screen frame.
//!
//! Phase 5.2 of the ROADMAP. The runtime
//! is currently a no-op: a `NullOcr`
//! engine that returns an empty
//! result. The contract is *typed
//! review* — every word carries its
//! bounding box, its confidence, and
//! the region it came from.
//!
//! The model has six pieces:
//!
//! 1. **`OcrRequest`** — what the
//!    caller wants read: a frame and
//!    an optional region.
//! 2. **`OcrWord`** — a single
//!    recognized word with its
//!    bounding box and confidence.
//! 3. **`OcrLine`** — a line of text
//!    (a sequence of words with a
//!    shared baseline).
//! 4. **`OcrResult`** — the full
//!    output: all lines, the joined
//!    text, and the average
//!    confidence.
//! 5. **`OcrEngine`** — the trait
//!    the runtime uses to plug in a
//!    real model (tesseract,
//!    easyocr, paddleocr, etc.).
//! 6. **`OcrSession`** — the stateful
//!    driver. Holds the engine and
//!    the most recent result.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use aether_vision_core::{Frame, Region};

/// A single recognized word.
#[derive(Debug, Clone, PartialEq)]
pub struct OcrWord {
    /// The word text.
    pub text: String,
    /// The bounding box of the word.
    pub bbox: Region,
    /// The confidence (0.0..=1.0).
    pub confidence: f32,
}

impl OcrWord {
    /// A new word.
    #[must_use]
    pub fn new(text: impl Into<String>, bbox: Region, confidence: f32) -> Self {
        Self {
            text: text.into(),
            bbox,
            confidence: confidence.clamp(0.0, 1.0),
        }
    }
}

/// A line of text: a sequence of words
/// sharing a baseline.
#[derive(Debug, Clone, PartialEq)]
pub struct OcrLine {
    /// The words in the line, left-to-
    /// right.
    pub words: Vec<OcrWord>,
    /// The bounding box of the line.
    pub bbox: Region,
}

impl OcrLine {
    /// The joined text of the line.
    #[must_use]
    pub fn text(&self) -> String {
        self.words
            .iter()
            .map(|w| w.text.as_str())
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// The average confidence of the
    /// line's words.
    #[must_use]
    pub fn average_confidence(&self) -> f32 {
        if self.words.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.words.iter().map(|w| w.confidence).sum();
        sum / self.words.len() as f32
    }
}

/// The full result of an OCR pass.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct OcrResult {
    /// The lines, top-to-bottom.
    pub lines: Vec<OcrLine>,
}

impl OcrResult {
    /// A new, empty result.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `true` if no text was
    /// recognized.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    /// The total number of words.
    #[must_use]
    pub fn word_count(&self) -> usize {
        self.lines.iter().map(|l| l.words.len()).sum()
    }

    /// The joined text of every line,
    /// newline-separated.
    #[must_use]
    pub fn text(&self) -> String {
        self.lines
            .iter()
            .map(OcrLine::text)
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// The average confidence of every
    /// word.
    #[must_use]
    pub fn average_confidence(&self) -> f32 {
        let n = self.word_count();
        if n == 0 {
            return 0.0;
        }
        let sum: f32 = self
            .lines
            .iter()
            .flat_map(|l| l.words.iter())
            .map(|w| w.confidence)
            .sum();
        sum / n as f32
    }

    /// The bounding box covering all
    /// recognized text.
    #[must_use]
    pub fn bbox(&self) -> Option<Region> {
        let mut acc: Option<Region> = None;
        for line in &self.lines {
            for word in &line.words {
                acc = Some(match acc {
                    None => word.bbox,
                    Some(a) => a.union(&word.bbox),
                });
            }
        }
        acc
    }
}

/// A request to OCR a frame.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OcrRequest {
    /// The frame to read.
    pub frame: Frame,
    /// The region to OCR, or `None` to
    /// OCR the whole frame.
    pub region: Option<Region>,
    /// A language hint ("en", "en+de",
    /// "auto", etc.).
    pub language: String,
}

impl OcrRequest {
    /// A new request for the whole
    /// frame.
    #[must_use]
    pub fn whole(frame: Frame) -> Self {
        Self {
            frame,
            region: None,
            language: alloc::string::String::from("en"),
        }
    }

    /// Set the region.
    #[must_use]
    pub fn with_region(mut self, region: Region) -> Self {
        self.region = Some(region);
        self
    }

    /// Set the language.
    #[must_use]
    pub fn with_language(mut self, lang: impl Into<String>) -> Self {
        self.language = lang.into();
        self
    }
}

/// OCR engine errors.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum OcrError {
    /// The frame is empty.
    EmptyFrame,
    /// The engine failed.
    EngineFailure(String),
}

impl core::fmt::Display for OcrError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyFrame => f.write_str("frame has no data"),
            Self::EngineFailure(msg) => write!(f, "ocr engine failure: {msg}"),
        }
    }
}

impl std::error::Error for OcrError {}

/// The OCR engine trait.
pub trait OcrEngine {
    /// Run OCR on the request.
    fn recognize(&self, request: &OcrRequest) -> Result<OcrResult, OcrError>;
}

/// A null OCR engine. Returns an
/// empty result. Used for tests and
/// graceful degradation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NullOcr;

impl OcrEngine for NullOcr {
    fn recognize(&self, request: &OcrRequest) -> Result<OcrResult, OcrError> {
        if request.frame.data.is_empty() {
            return Err(OcrError::EmptyFrame);
        }
        Ok(OcrResult::new())
    }
}

/// Extend `Region` with a `union` for
/// OCR bbox merging.
trait RegionExt {
    /// The smallest region containing
    /// both `self` and `other`.
    fn union(&self, other: &Self) -> Self;
}

impl RegionExt for Region {
    fn union(&self, other: &Self) -> Self {
        if self.is_empty() {
            return *other;
        }
        if other.is_empty() {
            return *self;
        }
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let right = self.right().max(other.right());
        let bottom = self.bottom().max(other.bottom());
        Region::new(x, y, right - x, bottom - y)
    }
}

/// A stateful OCR session.
pub struct OcrSession<E: OcrEngine> {
    engine: E,
    last_result: Option<OcrResult>,
    min_confidence: f32,
}

impl<E: OcrEngine> OcrSession<E> {
    /// A new session.
    #[must_use]
    pub fn new(engine: E) -> Self {
        Self {
            engine,
            last_result: None,
            min_confidence: 0.0,
        }
    }

    /// Set the minimum acceptable
    /// confidence.
    pub fn set_min_confidence(&mut self, c: f32) {
        self.min_confidence = c.clamp(0.0, 1.0);
    }

    /// The most recent result.
    #[must_use]
    pub fn last_result(&self) -> Option<&OcrResult> {
        self.last_result.as_ref()
    }

    /// Run OCR on a frame. The result
    /// is filtered to words above the
    /// minimum confidence.
    pub fn recognize(&mut self, request: &OcrRequest) -> Result<OcrResult, OcrError> {
        let raw = self.engine.recognize(request)?;
        let filtered = OcrResult {
            lines: raw
                .lines
                .into_iter()
                .map(|mut line| {
                    line.words.retain(|w| w.confidence >= self.min_confidence);
                    line
                })
                .filter(|l| !l.words.is_empty())
                .collect(),
        };
        self.last_result = Some(filtered.clone());
        Ok(filtered)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_vision_core::{PixelFormat, SourceId};

    fn frame(w: u32, h: u32) -> Frame {
        Frame::solid(
            w,
            h,
            PixelFormat::Rgb8,
            [255, 255, 255, 255],
            SourceId::new("t"),
            0,
        )
    }

    #[test]
    fn word_new_clamps_confidence() {
        let w = OcrWord::new("hello", Region::new(0, 0, 10, 10), 5.0);
        assert_eq!(w.confidence, 1.0);
        let w = OcrWord::new("hello", Region::new(0, 0, 10, 10), -0.5);
        assert_eq!(w.confidence, 0.0);
    }

    #[test]
    fn line_text_and_confidence() {
        let line = OcrLine {
            words: alloc::vec![
                OcrWord::new("hello", Region::new(0, 0, 10, 10), 0.9),
                OcrWord::new("world", Region::new(20, 0, 10, 10), 0.7),
            ],
            bbox: Region::new(0, 0, 30, 10),
        };
        assert_eq!(line.text(), "hello world");
        assert!((line.average_confidence() - 0.8).abs() < 1e-6);
    }

    #[test]
    fn line_empty_text() {
        let line = OcrLine {
            words: Vec::new(),
            bbox: Region::new(0, 0, 0, 0),
        };
        assert_eq!(line.text(), "");
        assert_eq!(line.average_confidence(), 0.0);
    }

    #[test]
    fn result_text_and_word_count() {
        let r = OcrResult {
            lines: alloc::vec![OcrLine {
                words: alloc::vec![
                    OcrWord::new("a", Region::new(0, 0, 1, 1), 0.9),
                    OcrWord::new("b", Region::new(2, 0, 1, 1), 0.9),
                ],
                bbox: Region::new(0, 0, 3, 1),
            }],
        };
        assert_eq!(r.word_count(), 2);
        assert_eq!(r.text(), "a b");
    }

    #[test]
    fn result_average_confidence_empty() {
        let r = OcrResult::new();
        assert_eq!(r.average_confidence(), 0.0);
        assert!(r.is_empty());
    }

    #[test]
    fn result_bbox_empty() {
        let r = OcrResult::new();
        assert!(r.bbox().is_none());
    }

    #[test]
    fn result_bbox_merges_words() {
        let r = OcrResult {
            lines: alloc::vec![OcrLine {
                words: alloc::vec![
                    OcrWord::new("a", Region::new(0, 0, 10, 10), 0.9),
                    OcrWord::new("b", Region::new(20, 0, 10, 10), 0.9),
                ],
                bbox: Region::new(0, 0, 30, 10),
            }],
        };
        let bb = r.bbox().unwrap();
        assert_eq!(bb.x, 0);
        assert_eq!(bb.width, 30);
    }

    #[test]
    fn request_whole_and_with() {
        let req = OcrRequest::whole(frame(100, 100))
            .with_region(Region::new(0, 0, 50, 50))
            .with_language("de");
        assert!(req.region.is_some());
        assert_eq!(req.language, "de");
    }

    #[test]
    fn null_ocr_rejects_empty_frame() {
        let e = NullOcr;
        let err = e
            .recognize(&OcrRequest::whole(Frame {
                width: 10,
                height: 10,
                format: PixelFormat::Rgb8,
                data: Vec::new(),
                source: SourceId::new("t"),
                timestamp_ms: 0,
            }))
            .unwrap_err();
        assert_eq!(err, OcrError::EmptyFrame);
    }

    #[test]
    fn null_ocr_returns_empty() {
        let e = NullOcr;
        let r = e.recognize(&OcrRequest::whole(frame(10, 10))).unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn ocr_error_display() {
        assert_eq!(OcrError::EmptyFrame.to_string(), "frame has no data");
        assert!(OcrError::EngineFailure(String::from("x"))
            .to_string()
            .contains("x"));
    }

    #[test]
    fn session_filters_by_confidence() {
        let mut s = OcrSession::new(NullOcr);
        s.set_min_confidence(0.5);
        // The null engine returns no
        // words, so the filtered result
        // is also empty.
        let r = s.recognize(&OcrRequest::whole(frame(10, 10))).unwrap();
        assert!(r.is_empty());
        assert!(s.last_result().is_some());
    }

    #[test]
    fn region_union_with_empty() {
        let a = Region::new(0, 0, 10, 10);
        let empty = Region::new(0, 0, 0, 0);
        assert_eq!(a.union(&empty), a);
        assert_eq!(empty.union(&a), a);
    }

    #[test]
    fn region_union_overlap() {
        let a = Region::new(0, 0, 10, 10);
        let b = Region::new(5, 5, 10, 10);
        let u = a.union(&b);
        assert_eq!(u, Region::new(0, 0, 15, 15));
    }
}
