//! Aether UI element detector — typed
//! detection of buttons, text fields,
//! links, and images on a screen.
//!
//! Phase 5.3 of the ROADMAP. The
//! runtime is currently a no-op: a
//! `NullUiDetector` returns no
//! elements. The contract is *typed
//! review* — every detected element
//! carries its type, bbox, label, and
//! confidence.
//!
//! The model has six pieces:
//!
//! 1. **`UiElementKind`** — the kind
//!    of element (button, text field,
//!    link, image, ...).
//! 2. **`UiElement`** — a single
//!    detected element.
//! 3. **`UiDetectionResult`** — the
//!    full output.
//! 4. **`UiDetector`** — the trait
//!    the runtime uses to plug in a
//!    real detector.
//! 5. **`NullUiDetector`** — the
//!    no-op fallback.
//! 6. **`UiDetectorSession`** — the
//!    stateful driver.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

use aether_ocr::OcrResult;
use aether_vision_core::{Frame, Region};

/// The kind of a UI element.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum UiElementKind {
    /// A clickable button.
    Button,
    /// A text input field.
    TextField,
    /// A checkable box or radio.
    Checkbox,
    /// A dropdown selector.
    Dropdown,
    /// A hyperlink.
    Link,
    /// A static image.
    Image,
    /// A label / heading / paragraph.
    Label,
    /// An unclassified element.
    Unknown,
}

impl UiElementKind {
    /// The kebab-case name.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Button => "button",
            Self::TextField => "text-field",
            Self::Checkbox => "checkbox",
            Self::Dropdown => "dropdown",
            Self::Link => "link",
            Self::Image => "image",
            Self::Label => "label",
            Self::Unknown => "unknown",
        }
    }
}

/// A single detected element.
#[derive(Debug, Clone, PartialEq)]
pub struct UiElement {
    /// The element kind.
    pub kind: UiElementKind,
    /// The bounding box.
    pub bbox: Region,
    /// The label / text associated
    /// with the element (e.g. the
    /// button's caption).
    pub label: String,
    /// The detection confidence
    /// (0.0..=1.0).
    pub confidence: f32,
}

impl UiElement {
    /// A new element.
    #[must_use]
    pub fn new(
        kind: UiElementKind,
        bbox: Region,
        label: impl Into<String>,
        confidence: f32,
    ) -> Self {
        Self { kind, bbox, label: label.into(), confidence: confidence.clamp(0.0, 1.0) }
    }

    /// `true` if the element is
    /// interactive (button, text
    /// field, checkbox, dropdown,
    /// link).
    #[must_use]
    pub const fn is_interactive(&self) -> bool {
        matches!(
            self.kind,
            UiElementKind::Button
                | UiElementKind::TextField
                | UiElementKind::Checkbox
                | UiElementKind::Dropdown
                | UiElementKind::Link
        )
    }
}

/// The full result of a detection.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct UiDetectionResult {
    /// The detected elements.
    pub elements: Vec<UiElement>,
}

impl UiDetectionResult {
    /// A new, empty result.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `true` if no elements were
    /// detected.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    /// The number of detected
    /// elements.
    #[must_use]
    pub fn len(&self) -> usize {
        self.elements.len()
    }

    /// Filter by kind.
    #[must_use]
    pub fn filter_kind(&self, kind: UiElementKind) -> Vec<&UiElement> {
        self.elements.iter().filter(|e| e.kind == kind).collect()
    }

    /// The interactive elements.
    #[must_use]
    pub fn interactive(&self) -> Vec<&UiElement> {
        self.elements.iter().filter(|e| e.is_interactive()).collect()
    }

    /// Merge in OCR text: any element
    /// whose bbox contains an OCR
    /// word's bbox is labeled with
    /// that word's text.
    pub fn merge_ocr(&mut self, ocr: &OcrResult) {
        for word in ocr.lines.iter().flat_map(|l| l.words.iter()) {
            for element in &mut self.elements {
                if element.bbox.contains(word.bbox.x, word.bbox.y) && element.label.is_empty() {
                    element.label = word.text.clone();
                    break;
                }
            }
        }
    }
}

/// A request to detect UI elements in
/// a frame.
#[derive(Debug, Clone, PartialEq)]
pub struct UiDetectionRequest {
    /// The frame to inspect.
    pub frame: Frame,
    /// The kinds to detect (empty =
    /// all).
    pub kinds: Vec<UiElementKind>,
    /// The minimum confidence
    /// (0.0..=1.0).
    pub min_confidence: f32,
}

impl UiDetectionRequest {
    /// A new request for all kinds.
    #[must_use]
    pub fn all(frame: Frame) -> Self {
        Self { frame, kinds: Vec::new(), min_confidence: 0.0 }
    }

    /// Limit to a specific kind.
    #[must_use]
    pub fn with_kind(mut self, kind: UiElementKind) -> Self {
        self.kinds.push(kind);
        self
    }

    /// Set the minimum confidence.
    #[must_use]
    pub fn with_min_confidence(mut self, c: f32) -> Self {
        self.min_confidence = c.clamp(0.0, 1.0);
        self
    }
}

/// UI detector engine errors.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UiDetectionError {
    /// The frame is empty.
    EmptyFrame,
    /// The engine failed.
    EngineFailure(String),
}

impl core::fmt::Display for UiDetectionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::EmptyFrame => f.write_str("frame has no data"),
            Self::EngineFailure(msg) => write!(f, "ui detection engine failure: {msg}"),
        }
    }
}

impl std::error::Error for UiDetectionError {}

/// The UI detector engine trait.
pub trait UiDetector {
    /// Detect elements in the request.
    fn detect(&self, request: &UiDetectionRequest) -> Result<UiDetectionResult, UiDetectionError>;
}

/// A null UI detector. Returns an
/// empty result. Used for tests and
/// graceful degradation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NullUiDetector;

impl UiDetector for NullUiDetector {
    fn detect(&self, request: &UiDetectionRequest) -> Result<UiDetectionResult, UiDetectionError> {
        if request.frame.data.is_empty() {
            return Err(UiDetectionError::EmptyFrame);
        }
        Ok(UiDetectionResult::new())
    }
}

/// A stateful UI detector session.
pub struct UiDetectorSession<D: UiDetector> {
    detector: D,
    last_result: Option<UiDetectionResult>,
}

impl<D: UiDetector> UiDetectorSession<D> {
    /// A new session.
    #[must_use]
    pub fn new(detector: D) -> Self {
        Self { detector, last_result: None }
    }

    /// The most recent result.
    #[must_use]
    pub fn last_result(&self) -> Option<&UiDetectionResult> {
        self.last_result.as_ref()
    }

    /// Run detection on a frame, with
    /// optional OCR labels.
    pub fn detect(
        &mut self,
        request: &UiDetectionRequest,
        ocr: Option<&OcrResult>,
    ) -> Result<UiDetectionResult, UiDetectionError> {
        let raw = self.detector.detect(request)?;
        let mut result = UiDetectionResult {
            elements: raw
                .elements
                .into_iter()
                .filter(|e| e.confidence >= request.min_confidence)
                .filter(|e| request.kinds.is_empty() || request.kinds.contains(&e.kind))
                .collect(),
        };
        if let Some(ocr) = ocr {
            result.merge_ocr(ocr);
        }
        self.last_result = Some(result.clone());
        Ok(result)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use aether_ocr::OcrLine;
    use aether_vision_core::{PixelFormat, SourceId};

    fn frame() -> Frame {
        Frame::solid(100, 100, PixelFormat::Rgb8, [255, 255, 255, 255], SourceId::new("t"), 0)
    }

    #[test]
    fn kind_as_str() {
        assert_eq!(UiElementKind::Button.as_str(), "button");
        assert_eq!(UiElementKind::TextField.as_str(), "text-field");
    }

    #[test]
    fn element_new_clamps_confidence() {
        let e = UiElement::new(UiElementKind::Button, Region::new(0, 0, 10, 10), "OK", 2.0);
        assert_eq!(e.confidence, 1.0);
    }

    #[test]
    fn element_is_interactive() {
        let button = UiElement::new(UiElementKind::Button, Region::new(0, 0, 1, 1), "", 0.5);
        let image = UiElement::new(UiElementKind::Image, Region::new(0, 0, 1, 1), "", 0.5);
        assert!(button.is_interactive());
        assert!(!image.is_interactive());
    }

    #[test]
    fn result_filter_kind() {
        let r = UiDetectionResult {
            elements: alloc::vec![
                UiElement::new(UiElementKind::Button, Region::new(0, 0, 1, 1), "OK", 0.9),
                UiElement::new(UiElementKind::TextField, Region::new(0, 0, 1, 1), "", 0.9),
            ],
        };
        assert_eq!(r.filter_kind(UiElementKind::Button).len(), 1);
        assert_eq!(r.interactive().len(), 2);
    }

    #[test]
    fn result_empty_helpers() {
        let r = UiDetectionResult::new();
        assert!(r.is_empty());
        assert_eq!(r.len(), 0);
    }

    #[test]
    fn result_merge_ocr_labels() {
        let mut r = UiDetectionResult {
            elements: alloc::vec![UiElement::new(
                UiElementKind::Button,
                Region::new(0, 0, 100, 50),
                "",
                0.9,
            )],
        };
        let ocr = OcrResult {
            lines: alloc::vec![OcrLine {
                words: alloc::vec![aether_ocr::OcrWord::new(
                    "OK",
                    Region::new(10, 10, 20, 20),
                    0.9,
                )],
                bbox: Region::new(10, 10, 20, 20),
            }],
        };
        r.merge_ocr(&ocr);
        assert_eq!(r.elements[0].label, "OK");
    }

    #[test]
    fn result_merge_ocr_skips_labeled() {
        let mut r = UiDetectionResult {
            elements: alloc::vec![UiElement::new(
                UiElementKind::Button,
                Region::new(0, 0, 100, 50),
                "Cancel",
                0.9,
            )],
        };
        let ocr = OcrResult {
            lines: alloc::vec![OcrLine {
                words: alloc::vec![aether_ocr::OcrWord::new(
                    "OK",
                    Region::new(10, 10, 20, 20),
                    0.9,
                )],
                bbox: Region::new(10, 10, 20, 20),
            }],
        };
        r.merge_ocr(&ocr);
        // Existing label preserved.
        assert_eq!(r.elements[0].label, "Cancel");
    }

    #[test]
    fn request_all_and_with() {
        let req = UiDetectionRequest::all(frame())
            .with_kind(UiElementKind::Button)
            .with_min_confidence(0.7);
        assert_eq!(req.kinds.len(), 1);
        assert_eq!(req.min_confidence, 0.7);
    }

    #[test]
    fn null_detector_rejects_empty() {
        let d = NullUiDetector;
        let err = d
            .detect(&UiDetectionRequest::all(Frame {
                width: 10,
                height: 10,
                format: PixelFormat::Rgb8,
                data: Vec::new(),
                source: SourceId::new("t"),
                timestamp_ms: 0,
            }))
            .unwrap_err();
        assert_eq!(err, UiDetectionError::EmptyFrame);
    }

    #[test]
    fn null_detector_returns_empty() {
        let d = NullUiDetector;
        let r = d.detect(&UiDetectionRequest::all(frame())).unwrap();
        assert!(r.is_empty());
    }

    #[test]
    fn ui_error_display() {
        assert_eq!(UiDetectionError::EmptyFrame.to_string(), "frame has no data");
        assert!(UiDetectionError::EngineFailure(String::from("x")).to_string().contains("x"));
    }

    #[test]
    fn session_detect_with_min_confidence() {
        let mut s = UiDetectorSession::new(NullUiDetector);
        let r = s.detect(&UiDetectionRequest::all(frame()).with_min_confidence(0.5), None).unwrap();
        assert!(r.is_empty());
        assert!(s.last_result().is_some());
    }

    #[test]
    fn session_detect_with_ocr() {
        let mut s = UiDetectorSession::new(NullUiDetector);
        let r = s.detect(&UiDetectionRequest::all(frame()), None).unwrap();
        assert!(r.is_empty());
    }
}
