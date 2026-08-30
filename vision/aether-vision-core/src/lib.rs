//! Aether vision core — the typed model
//! for screen frames, regions of
//! interest, and the universal
//! `aether_screen` reference.
//!
//! Phase 5.1 of the ROADMAP. The runtime
//! is currently a no-op: the model only
//! defines the types. The vision crates
//! (aether-ocr, aether-ui-detector)
//! build on this model. The runtime
//! plugs in real backends (wayland
//! screencopy, X11 getimage, Win32
//! BitBlt, etc.) by implementing
//! `ScreenSource`.
//!
//! The contract is *typed review* —
//! every observation includes its
//! region, timestamp, and source so the
//! shell and the agent can audit what
//! was seen.
//!
//! The model has five pieces:
//!
//! 1. **`PixelFormat`** — the layout of
//!    a frame (RGB, RGBA, BGRA, Gray).
//! 2. **`Frame`** — a captured screen
//!    or window image, with explicit
//!    dimensions and format.
//! 3. **`Region`** — a rectangle on the
//!    frame (origin + size).
//! 4. **`SourceId`** — a typed id for
//!    a screen or window source.
//! 5. **`ScreenSource`** — the trait
//!    the runtime uses to plug in a
//!    real capture backend.

#![deny(unsafe_code)]
#![warn(missing_docs)]
#![allow(clippy::doc_overindented_list_items)]

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

/// The pixel format of a frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PixelFormat {
    /// 8 bits per pixel, grayscale.
    Gray8,
    /// 24 bits per pixel, RGB byte
    /// order.
    Rgb8,
    /// 32 bits per pixel, RGBA byte
    /// order.
    Rgba8,
    /// 32 bits per pixel, BGRA byte
    /// order (Windows native).
    Bgra8,
}

impl PixelFormat {
    /// The bytes per pixel.
    #[must_use]
    pub const fn bytes_per_pixel(&self) -> usize {
        match self {
            Self::Gray8 => 1,
            Self::Rgb8 => 3,
            Self::Rgba8 | Self::Bgra8 => 4,
        }
    }

    /// The kebab-case name.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Gray8 => "gray8",
            Self::Rgb8 => "rgb8",
            Self::Rgba8 => "rgba8",
            Self::Bgra8 => "bgra8",
        }
    }
}

/// A captured frame.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Frame {
    /// The width in pixels.
    pub width: u32,
    /// The height in pixels.
    pub height: u32,
    /// The pixel format.
    pub format: PixelFormat,
    /// The raw pixel data, row-major.
    pub data: Vec<u8>,
    /// The source id (which screen or
    /// window this frame came from).
    pub source: SourceId,
    /// When the frame was captured (ms
    /// since epoch; the caller supplies
    /// the clock).
    pub timestamp_ms: u64,
}

impl Frame {
    /// A solid-color frame, useful for
    /// tests.
    #[must_use]
    pub fn solid(
        width: u32,
        height: u32,
        format: PixelFormat,
        color: [u8; 4],
        source: SourceId,
        timestamp_ms: u64,
    ) -> Self {
        let bpp = format.bytes_per_pixel();
        let n = (width as usize) * (height as usize) * bpp;
        let mut data = Vec::with_capacity(n);
        for i in 0..(width as usize) * (height as usize) {
            for j in 0..bpp {
                data.push(color.get(j).copied().unwrap_or(0));
            }
            let _ = i;
        }
        Self {
            width,
            height,
            format,
            data,
            source,
            timestamp_ms,
        }
    }

    /// The total number of pixels.
    #[must_use]
    pub fn pixel_count(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    /// The total number of bytes
    /// (validates against the data
    /// length).
    #[must_use]
    pub fn expected_size(&self) -> usize {
        (self.pixel_count() as usize) * self.format.bytes_per_pixel()
    }

    /// `true` if the data length matches
    /// the expected size.
    #[must_use]
    pub fn is_well_formed(&self) -> bool {
        self.data.len() == self.expected_size()
    }

    /// The aspect ratio (width / height),
    /// or 0.0 if the height is 0.
    #[must_use]
    pub fn aspect_ratio(&self) -> f32 {
        if self.height == 0 {
            0.0
        } else {
            self.width as f32 / self.height as f32
        }
    }

    /// Get the pixel at `(x, y)`. Returns
    /// `[0; 4]` if the coords are out of
    /// bounds.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        if x >= self.width || y >= self.height {
            return [0, 0, 0, 0];
        }
        let idx = ((y as usize) * (self.width as usize) + (x as usize))
            * self.format.bytes_per_pixel();
        match self.format {
            PixelFormat::Gray8 => [self.data[idx], self.data[idx], self.data[idx], 255],
            PixelFormat::Rgb8 => {
                if idx + 2 < self.data.len() {
                    [self.data[idx], self.data[idx + 1], self.data[idx + 2], 255]
                } else {
                    [0, 0, 0, 0]
                }
            }
            PixelFormat::Rgba8 => {
                if idx + 3 < self.data.len() {
                    [
                        self.data[idx],
                        self.data[idx + 1],
                        self.data[idx + 2],
                        self.data[idx + 3],
                    ]
                } else {
                    [0, 0, 0, 0]
                }
            }
            PixelFormat::Bgra8 => {
                if idx + 3 < self.data.len() {
                    [
                        self.data[idx + 2],
                        self.data[idx + 1],
                        self.data[idx],
                        self.data[idx + 3],
                    ]
                } else {
                    [0, 0, 0, 0]
                }
            }
        }
    }
}

/// A region of interest within a frame:
/// an origin `(x, y)` and a size
/// `(width, height)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Region {
    /// The left edge.
    pub x: u32,
    /// The top edge.
    pub y: u32,
    /// The width.
    pub width: u32,
    /// The height.
    pub height: u32,
}

impl Region {
    /// A new region.
    #[must_use]
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// A region covering the whole
    /// frame.
    #[must_use]
    pub const fn full_frame(width: u32, height: u32) -> Self {
        Self::new(0, 0, width, height)
    }

    /// The right edge.
    #[must_use]
    pub const fn right(&self) -> u32 {
        self.x.saturating_add(self.width)
    }

    /// The bottom edge.
    #[must_use]
    pub const fn bottom(&self) -> u32 {
        self.y.saturating_add(self.height)
    }

    /// The area in pixels.
    #[must_use]
    pub fn area(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    /// `true` if the region has no
    /// pixels.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }

    /// `true` if `(x, y)` is inside the
    /// region.
    #[must_use]
    pub fn contains(&self, x: u32, y: u32) -> bool {
        x >= self.x && x < self.right() && y >= self.y && y < self.bottom()
    }

    /// The intersection with another
    /// region. Returns an empty region
    /// if they don't overlap.
    #[must_use]
    pub fn intersect(&self, other: &Self) -> Self {
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        if right <= x || bottom <= y {
            Self::new(0, 0, 0, 0)
        } else {
            Self::new(x, y, right - x, bottom - y)
        }
    }

    /// Crop the frame to this region.
    /// Returns a new frame with only
    /// the pixels inside the region.
    #[must_use]
    pub fn crop(&self, frame: &Frame) -> Frame {
        let inter = self.intersect(&Region::full_frame(frame.width, frame.height));
        if self.is_empty() || inter.is_empty() {
            return Frame {
                width: 0,
                height: 0,
                format: frame.format,
                data: Vec::new(),
                source: frame.source.clone(),
                timestamp_ms: frame.timestamp_ms,
            };
        }
        let bpp = frame.format.bytes_per_pixel();
        let mut data = Vec::with_capacity((inter.area() as usize) * bpp);
        let src_stride = (frame.width as usize) * bpp;
        for row in inter.y..inter.bottom() {
            let src_start = (row as usize) * src_stride + (inter.x as usize) * bpp;
            let n = (inter.width as usize) * bpp;
            if src_start + n <= frame.data.len() {
                data.extend_from_slice(&frame.data[src_start..src_start + n]);
            }
        }
        Frame {
            width: inter.width,
            height: inter.height,
            format: frame.format,
            data,
            source: frame.source.clone(),
            timestamp_ms: frame.timestamp_ms,
        }
    }
}

/// A typed id for a screen or window
/// source. The runtime plugs in real
/// ids (e.g. "wayland:output-1",
/// "x11:0x4a00001", "win32:hwnd:0x1234").
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SourceId(String);

impl SourceId {
    /// A new id.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The inner string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl core::fmt::Display for SourceId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.0)
    }
}

/// A description of a screen or window
/// source.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SourceInfo {
    /// The source id.
    pub id: SourceId,
    /// A human-readable name.
    pub name: String,
    /// The native width in pixels.
    pub width: u32,
    /// The native height in pixels.
    pub height: u32,
    /// `true` if this is the primary
    /// display.
    pub is_primary: bool,
}

impl SourceInfo {
    /// A new source info.
    #[must_use]
    pub fn new(
        id: impl Into<String>,
        name: impl Into<String>,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            id: SourceId::new(id),
            name: name.into(),
            width,
            height,
            is_primary: false,
        }
    }

    /// Mark as primary.
    #[must_use]
    pub fn primary(mut self) -> Self {
        self.is_primary = true;
        self
    }
}

/// The screen source trait. The runtime
/// plugs in a real backend.
pub trait ScreenSource: Send {
    /// Enumerate the available sources.
    fn enumerate(&self) -> Vec<SourceInfo>;

    /// The primary source id, if any.
    fn primary(&self) -> Option<SourceId>;

    /// Capture a frame from the given
    /// source. The `now_ms` is supplied
    /// by the caller (the wall clock).
    fn capture(&self, source: &SourceId, now_ms: u64) -> Option<Frame>;
}

/// A null source: enumerates a single
/// virtual source and returns solid
/// black frames. Useful for tests and
/// graceful degradation.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NullSource;

impl ScreenSource for NullSource {
    fn enumerate(&self) -> Vec<SourceInfo> {
        alloc::vec![SourceInfo::new(
            "null:0",
            "Null Display",
            1920,
            1080,
        )
        .primary()]
    }

    fn primary(&self) -> Option<SourceId> {
        Some(SourceId::new("null:0"))
    }

    fn capture(&self, source: &SourceId, now_ms: u64) -> Option<Frame> {
        if source.as_str() != "null:0" {
            return None;
        }
        Some(Frame::solid(
            1920,
            1080,
            PixelFormat::Rgb8,
            [0, 0, 0, 255],
            source.clone(),
            now_ms,
        ))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn pixel_format_bytes_per_pixel() {
        assert_eq!(PixelFormat::Gray8.bytes_per_pixel(), 1);
        assert_eq!(PixelFormat::Rgb8.bytes_per_pixel(), 3);
        assert_eq!(PixelFormat::Rgba8.bytes_per_pixel(), 4);
        assert_eq!(PixelFormat::Bgra8.bytes_per_pixel(), 4);
    }

    #[test]
    fn pixel_format_as_str() {
        assert_eq!(PixelFormat::Gray8.as_str(), "gray8");
        assert_eq!(PixelFormat::Bgra8.as_str(), "bgra8");
    }

    #[test]
    fn frame_solid_creates_data() {
        let f = Frame::solid(
            10,
            5,
            PixelFormat::Rgb8,
            [100, 200, 50, 255],
            SourceId::new("test"),
            0,
        );
        assert_eq!(f.width, 10);
        assert_eq!(f.height, 5);
        assert_eq!(f.expected_size(), 150);
        assert_eq!(f.data.len(), 150);
        assert!(f.is_well_formed());
    }

    #[test]
    fn frame_pixel_count_and_aspect() {
        let f = Frame::solid(
            1920,
            1080,
            PixelFormat::Rgb8,
            [0; 4],
            SourceId::new("test"),
            0,
        );
        assert_eq!(f.pixel_count(), 1920 * 1080);
        assert!((f.aspect_ratio() - 16.0 / 9.0).abs() < 1e-4);
    }

    #[test]
    fn frame_aspect_zero_height() {
        let f = Frame::solid(10, 0, PixelFormat::Rgb8, [0; 4], SourceId::new("t"), 0);
        assert_eq!(f.aspect_ratio(), 0.0);
    }

    #[test]
    fn frame_pixel_gray() {
        let f = Frame::solid(
            4,
            4,
            PixelFormat::Gray8,
            [200, 0, 0, 0],
            SourceId::new("t"),
            0,
        );
        let p = f.pixel(2, 2);
        assert_eq!(p, [200, 200, 200, 255]);
    }

    #[test]
    fn frame_pixel_rgb() {
        let f = Frame::solid(
            4,
            4,
            PixelFormat::Rgb8,
            [10, 20, 30, 255],
            SourceId::new("t"),
            0,
        );
        let p = f.pixel(1, 1);
        assert_eq!(p, [10, 20, 30, 255]);
    }

    #[test]
    fn frame_pixel_bgra() {
        let f = Frame::solid(
            4,
            4,
            PixelFormat::Bgra8,
            [10, 20, 30, 40],
            SourceId::new("t"),
            0,
        );
        let p = f.pixel(1, 1);
        // BGRA -> RGB: 30,20,10,40
        assert_eq!(p, [30, 20, 10, 40]);
    }

    #[test]
    fn frame_pixel_out_of_bounds() {
        let f = Frame::solid(4, 4, PixelFormat::Rgb8, [0; 4], SourceId::new("t"), 0);
        assert_eq!(f.pixel(100, 100), [0, 0, 0, 0]);
    }

    #[test]
    fn frame_not_well_formed() {
        let f = Frame {
            width: 10,
            height: 10,
            format: PixelFormat::Rgb8,
            data: alloc::vec![0; 50],
            source: SourceId::new("t"),
            timestamp_ms: 0,
        };
        assert!(!f.is_well_formed());
    }

    #[test]
    fn source_id_display() {
        let s = SourceId::new("wayland:0");
        assert_eq!(s.to_string(), "wayland:0");
    }

    #[test]
    fn source_info_primary() {
        let s = SourceInfo::new("id", "name", 100, 100).primary();
        assert!(s.is_primary);
    }

    #[test]
    fn region_new_and_edges() {
        let r = Region::new(10, 20, 30, 40);
        assert_eq!(r.right(), 40);
        assert_eq!(r.bottom(), 60);
        assert_eq!(r.area(), 1200);
    }

    #[test]
    fn region_is_empty() {
        assert!(Region::new(0, 0, 0, 100).is_empty());
        assert!(!Region::new(0, 0, 100, 100).is_empty());
    }

    #[test]
    fn region_contains() {
        let r = Region::new(10, 20, 30, 40);
        assert!(r.contains(10, 20));
        assert!(r.contains(20, 30));
        assert!(!r.contains(9, 20));
        assert!(!r.contains(40, 20));
    }

    #[test]
    fn region_intersect_overlap() {
        let a = Region::new(0, 0, 100, 100);
        let b = Region::new(50, 50, 100, 100);
        let i = a.intersect(&b);
        assert_eq!(i, Region::new(50, 50, 50, 50));
    }

    #[test]
    fn region_intersect_no_overlap() {
        let a = Region::new(0, 0, 10, 10);
        let b = Region::new(20, 20, 10, 10);
        let i = a.intersect(&b);
        assert!(i.is_empty());
    }

    #[test]
    fn region_intersect_touching() {
        let a = Region::new(0, 0, 10, 10);
        let b = Region::new(10, 0, 10, 10);
        let i = a.intersect(&b);
        assert!(i.is_empty());
    }

    #[test]
    fn region_full_frame() {
        let r = Region::full_frame(100, 50);
        assert_eq!(r, Region::new(0, 0, 100, 50));
    }

    #[test]
    fn region_crop_subsample() {
        let frame = Frame::solid(
            10,
            10,
            PixelFormat::Rgb8,
            [50, 100, 150, 255],
            SourceId::new("t"),
            0,
        );
        let cropped = Region::new(0, 0, 5, 5).crop(&frame);
        assert_eq!(cropped.width, 5);
        assert_eq!(cropped.height, 5);
        assert_eq!(cropped.data.len(), 5 * 5 * 3);
    }

    #[test]
    fn region_crop_empty() {
        let frame = Frame::solid(
            10,
            10,
            PixelFormat::Rgb8,
            [0; 4],
            SourceId::new("t"),
            0,
        );
        let cropped = Region::new(50, 50, 10, 10).crop(&frame);
        assert_eq!(cropped.width, 0);
    }

    #[test]
    fn null_source_enumerates_one() {
        let s = NullSource;
        let v = s.enumerate();
        assert_eq!(v.len(), 1);
        assert!(v[0].is_primary);
    }

    #[test]
    fn null_source_primary() {
        let s = NullSource;
        let p = s.primary();
        assert_eq!(p.unwrap().as_str(), "null:0");
    }

    #[test]
    fn null_source_capture_known() {
        let s = NullSource;
        let f = s.capture(&SourceId::new("null:0"), 100);
        assert!(f.is_some());
        assert_eq!(f.unwrap().width, 1920);
    }

    #[test]
    fn null_source_capture_unknown() {
        let s = NullSource;
        assert!(s.capture(&SourceId::new("ghost"), 100).is_none());
    }
}
