//! Aether OS renderer.
//!
//! Bridges the declarative UI component system
//! (`aether-ui-components`) to actual framebuffer pixels.
//! The renderer owns a `PixelBuffer` and provides drawing
//! primitives that the graphical shell uses instead of
//! raw `Screen` calls.
//!
//! # Architecture
//!
//! ```text
//! Component (layout + style)
//!     ↓
//! ComponentRenderer::render_component()
//!     ↓
//! Drawing primitives (rect, rounded_rect, text, shadow, gradient)
//!     ↓
//! PixelBuffer (RGBA8888 byte slice)
//!     ↓
//! Screen::flush() → /dev/fb0
//! ```

#![deny(unsafe_code)]
#![warn(missing_docs)]

use aether_design_tokens::{Color, Role, Spacing, TypeScale};
use aether_ui_components::ComponentStyle;

// --------------------------------------------------------------------- pixel

/// A raw RGBA pixel (BGRA in memory for fbdev).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pixel(pub u8, pub u8, pub u8, pub u8);

impl Pixel {
    /// Opaque pixel from RGB channels.
    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self(b, g, r, 0xFF)
    }

    /// Opaque pixel from an Aether `Color`.
    #[must_use]
    pub fn from_color(c: Color) -> Self {
        Self::rgb(c.r, c.g, c.b)
    }
}

// --------------------------------------------------------------- pixel buffer

/// A raw pixel buffer backed by a borrowed byte slice. The
/// buffer is stored in BGRA order (matching fbdev's 32bpp layout).
pub struct PixelBuffer<'a> {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Stride in bytes (width × 4, typically).
    pub stride: u32,
    /// Raw pixel data (BGRA8888).
    pub data: &'a mut [u8],
}

impl<'a> PixelBuffer<'a> {
    /// Create a buffer wrapping an existing byte slice.
    ///
    /// # Panics
    ///
    /// Panics if `data.len() < (stride * height) as usize`.
    #[must_use]
    pub fn from_raw(width: u32, height: u32, stride: u32, data: &'a mut [u8]) -> Self {
        assert!(data.len() >= (stride * height) as usize, "buffer too small");
        Self { width, height, stride, data }
    }

    /// Fill every pixel with the given color.
    pub fn fill(&mut self, c: Color) {
        let p = Pixel::from_color(c);
        for chunk in self.data.chunks_exact_mut(4) {
            chunk.copy_from_slice(&[p.0, p.1, p.2, p.3]);
        }
    }

    /// Write a single pixel (clipped).
    pub fn put_pixel(&mut self, x: i64, y: i64, c: Color) {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return;
        }
        let p = Pixel::from_color(c);
        let off = (y as u32 * self.stride + x as u32 * 4) as usize;
        if off + 4 <= self.data.len() {
            self.data[off..off + 4].copy_from_slice(&[p.0, p.1, p.2, p.3]);
        }
    }

    /// Read a single pixel (clipped). Returns `None` if
    /// out of bounds.
    #[must_use]
    pub fn get_pixel(&self, x: i64, y: i64) -> Option<Color> {
        if x < 0 || y < 0 || x >= self.width as i64 || y >= self.height as i64 {
            return None;
        }
        let off = (y as u32 * self.stride + x as u32 * 4) as usize;
        if off + 4 <= self.data.len() {
            Some(Color::rgb(self.data[off + 2], self.data[off + 1], self.data[off]))
        } else {
            None
        }
    }

    /// Draw a filled rectangle.
    pub fn rect(&mut self, x: i64, y: i64, w: u32, h: u32, c: Color) {
        let p = Pixel::from_color(c);
        let px = [p.0, p.1, p.2, p.3];
        for row in y.max(0)..(y + h as i64).min(self.height as i64) {
            let start = (x.max(0) as u32 * 4) as usize;
            let end = ((x + w as i64).min(self.width as i64) as u32 * 4) as usize;
            if end <= start {
                continue;
            }
            let base = row as u32 * self.stride;
            for chunk in
                self.data[(base as usize + start)..(base as usize + end)].chunks_exact_mut(4)
            {
                chunk.copy_from_slice(&px);
            }
        }
    }

    /// Draw a filled rounded rectangle. The radius is
    /// clamped to half the smaller dimension.
    pub fn rounded_rect(&mut self, x: i64, y: i64, w: u32, h: u32, radius: i64, c: Color) {
        let r = radius.min((w.min(h) / 2) as i64);
        if r <= 0 {
            self.rect(x, y, w, h, c);
            return;
        }
        let p = Pixel::from_color(c);
        let px = [p.0, p.1, p.2, p.3];
        for row in y.max(0)..(y + h as i64).min(self.height as i64) {
            let start = (x.max(0) as u32 * 4) as usize;
            let end = ((x + w as i64).min(self.width as i64) as u32 * 4) as usize;
            if end <= start {
                continue;
            }
            let base = row as u32 * self.stride;
            for col in x.max(0)..((x + w as i64).min(self.width as i64)) {
                let inside = Self::inside_rounded_rect(col, row, x, y, w, h, r);
                if inside {
                    let off = base as usize + col as usize * 4;
                    if off + 4 <= self.data.len() {
                        self.data[off..off + 4].copy_from_slice(&px);
                    }
                }
            }
        }
    }

    /// Check if a point is inside a rounded rectangle.
    fn inside_rounded_rect(px: i64, py: i64, x: i64, y: i64, w: u32, h: u32, r: i64) -> bool {
        let x2 = x + w as i64 - 1;
        let y2 = y + h as i64 - 1;
        // Quick bounds check.
        if px < x || px > x2 || py < y || py > y2 {
            return false;
        }
        // Inside the rectangular core (no corner check needed).
        if px >= x + r && px <= x2 - r && py >= y + r && py <= y2 - r {
            return true;
        }
        // Check each corner quadrant.
        let corners = [
            (x + r, y + r),   // top-left
            (x2 - r, y + r),  // top-right
            (x + r, y2 - r),  // bottom-left
            (x2 - r, y2 - r), // bottom-right
        ];
        for (cx, cy) in &corners {
            let dx = px - cx;
            let dy = py - cy;
            if dx * dx + dy * dy <= r * r {
                return true;
            }
        }
        false
    }

    /// Draw a drop shadow behind a rounded rectangle.
    /// The shadow is rendered as a blurred, offset copy.
    #[allow(clippy::too_many_arguments)]
    pub fn shadow(
        &mut self,
        x: i64,
        y: i64,
        w: u32,
        h: u32,
        radius: i64,
        offset: i64,
        blur: i64,
        c: Color,
    ) {
        // Draw a larger, offset, blurred rounded rect.
        let sx = x + offset;
        let sy = y + offset;
        let sw = w + (blur as u32 * 2);
        let sh = h + (blur as u32 * 2);
        let sr = radius + blur;
        self.rounded_rect(sx - blur, sy - blur, sw, sh, sr, c);
    }

    /// Draw a vertical gradient rectangle.
    pub fn gradient_rect(&mut self, x: i64, y: i64, w: u32, h: u32, top: Color, bottom: Color) {
        for row in 0..h {
            let t = if h > 1 { row as f32 / (h - 1) as f32 } else { 0.0 };
            let r = lerp(top.r, bottom.r, t);
            let g = lerp(top.g, bottom.g, t);
            let b = lerp(top.b, bottom.b, t);
            let c = Color::rgb(r, g, b);
            let ry = y + row as i64;
            if ry >= 0 && ry < self.height as i64 {
                self.rect(x, ry, w, 1, c);
            }
        }
    }

    /// Clear the entire buffer to a color.
    pub fn clear(&mut self, c: Color) {
        self.fill(c);
    }

    // --------------------------------------------------------- glass / crystal

    /// Draw a glass panel: a translucent rounded rectangle
    /// with a frosted border, inner highlight, and deep shadow.
    /// The `alpha` parameter (0.0..=1.0) controls translucency.
    pub fn glass_panel(&mut self, x: i64, y: i64, w: u32, h: u32, radius: i64, alpha: f32) {
        let a = alpha.clamp(0.0, 1.0);

        // Deep shadow (two layers for depth).
        self.rounded_rect(x + 2, y + 3, w, h, radius + 2, Color::role(Role::ShadowDeep));
        self.rounded_rect(x + 1, y + 2, w, h, radius + 1, Color::role(Role::Shadow));

        // Main glass fill — blend white with background.
        let bg = Color::role(Role::GlassBg);
        let fill = blend_alpha(Color::role(Role::BgBase), bg, a);
        self.rounded_rect(x, y, w, h, radius, fill);

        // Frost overlay — slightly tinted.
        let frost = blend_alpha(fill, Color::role(Role::GlassFrost), 0.3 * a);
        self.rounded_rect(
            x + 2,
            y + 2,
            w.saturating_sub(4),
            h.saturating_sub(4),
            radius.saturating_sub(1),
            frost,
        );

        // Top highlight — bright line at the top edge.
        if h > 4 {
            self.rounded_rect(
                x + 4,
                y + 1,
                w.saturating_sub(8),
                2,
                1,
                blend_alpha(fill, Color::role(Role::GlassHighlight), 0.6 * a),
            );
        }

        // Border — prismatic edge.
        draw_rect_outline(
            self,
            x,
            y,
            w,
            h,
            radius,
            blend_alpha(fill, Color::role(Role::GlassBorder), 0.5 * a),
        );
    }

    /// Draw a crystal panel: a glass panel with prismatic
    /// edge highlights and a subtle inner glow.
    pub fn crystal_panel(&mut self, x: i64, y: i64, w: u32, h: u32, radius: i64, alpha: f32) {
        let a = alpha.clamp(0.0, 1.0);

        // Glass base.
        self.glass_panel(x, y, w, h, radius, a);

        // Prismatic top-left corner glow.
        let glow_r = (w.min(h) / 3).max(8);
        self.gradient_glow(x + 4, y + 4, glow_r, glow_r, Color::role(Role::CrystalPrism), 0.4 * a);

        // Prismatic bottom-right accent.
        let gx = x + w as i64 - glow_r as i64 - 4;
        let gy = y + h as i64 - glow_r as i64 - 4;
        self.gradient_glow(gx, gy, glow_r, glow_r, Color::role(Role::CrystalRefract), 0.3 * a);

        // Crystal edge line — top.
        if h > 6 {
            for i in 0..3u32 {
                let t = i as f32 / 3.0;
                let c =
                    lerp_color(Color::role(Role::CrystalEdge), Color::role(Role::CrystalShine), t);
                self.rounded_rect(
                    x + 6 + i as i64,
                    y + 2 + i as i64,
                    w.saturating_sub(12 + i * 2),
                    1,
                    0,
                    blend_alpha(Color::role(Role::BgBase), c, 0.4 * a),
                );
            }
        }
    }

    /// Draw a soft radial glow at a position. Used for
    /// crystal highlights and ambient light effects.
    pub fn gradient_glow(&mut self, x: i64, y: i64, w: u32, h: u32, c: Color, alpha: f32) {
        let a = alpha.clamp(0.0, 1.0);
        let cx = x + w as i64 / 2;
        let cy = y + h as i64 / 2;
        let max_r = (w.min(h) as f32 / 2.0).max(1.0);

        for py in y..(y + h as i64) {
            for px in x..(x + w as i64) {
                let dx = px - cx;
                let dy = py - cy;
                let dist = ((dx * dx + dy * dy) as f32).sqrt();
                let t = (1.0 - dist / max_r).clamp(0.0, 1.0);
                if t > 0.0 {
                    let pixel_alpha = t * t * a; // quadratic falloff
                    let bg = self.get_pixel(px, py).unwrap_or(Color::role(Role::BgBase));
                    let blended = blend_alpha(bg, c, pixel_alpha);
                    self.put_pixel(px, py, blended);
                }
            }
        }
    }

    /// Draw a horizontal glow line — used for crystal edges
    /// and separator highlights.
    pub fn glow_line(&mut self, x: i64, y: i64, w: u32, c: Color, alpha: f32) {
        let a = alpha.clamp(0.0, 1.0);
        for px in x..(x + w as i64) {
            // Center bright, edges fade.
            let t = 1.0;
            let bg = self.get_pixel(px, y).unwrap_or(Color::role(Role::BgBase));
            let blended = blend_alpha(bg, c, t * a);
            self.put_pixel(px, y, blended);
        }
        // Blur row above and below for glow spread.
        for dy in [-1i64, 1] {
            let row = y + dy;
            if row < 0 || row >= self.height as i64 {
                continue;
            }
            for px in x..(x + w as i64) {
                let bg = self.get_pixel(px, row).unwrap_or(Color::role(Role::BgBase));
                let blended = blend_alpha(bg, c, 0.3 * a);
                self.put_pixel(px, row, blended);
            }
        }
    }

    /// Draw a window with glass chrome: translucent title bar,
    /// crystal border, and deep layered shadow.
    #[allow(clippy::too_many_arguments)]
    pub fn glass_window(
        &mut self,
        x: i64,
        y: i64,
        w: u32,
        h: u32,
        radius: i64,
        focused: bool,
        alpha: f32,
    ) {
        let a = alpha.clamp(0.0, 1.0);

        // Deep layered shadow.
        for i in 0..4 {
            let offset = (4 - i) as i64;
            let spread = i as u32 * 2;
            let shadow_alpha = 0.08 * (4 - i) as f32 * a;
            self.rounded_rect(
                x - spread as i64,
                y - spread as i64 + offset,
                w + spread * 2,
                h + spread * 2,
                radius + spread as i64,
                blend_alpha(Color::role(Role::BgBase), Color::role(Role::ShadowDeep), shadow_alpha),
            );
        }

        // Window body — glass fill.
        let body_fill =
            blend_alpha(Color::role(Role::BgBase), Color::role(Role::GlassFrost), 0.7 * a);
        self.rounded_rect(x, y, w, h, radius, body_fill);

        // Title bar — slightly more opaque glass.
        let title_h: i64 = 28;
        let title_fill = blend_alpha(body_fill, Color::role(Role::GlassBg), 0.4 * a);
        self.rounded_rect(
            x + 1,
            y + 1,
            w.saturating_sub(2),
            title_h as u32,
            radius.saturating_sub(1),
            title_fill,
        );

        // Crystal top highlight.
        if focused {
            self.glow_line(
                x + 8,
                y,
                w.saturating_sub(16),
                Color::role(Role::CrystalShine),
                0.5 * a,
            );
        }

        // Border.
        let border_color = if focused {
            blend_alpha(body_fill, Color::role(Role::CrystalEdge), 0.6 * a)
        } else {
            blend_alpha(body_fill, Color::role(Role::GlassBorder), 0.4 * a)
        };
        draw_rect_outline(self, x, y, w, h, radius, border_color);

        // Content area — clear to base.
        let (_cx, _cy, cw, ch) = content_rect(x, y, w, h, title_h as u32, 0);
        if ch > 0 && cw > 0 {
            self.rounded_rect(
                _cx,
                _cy,
                cw,
                ch,
                0,
                blend_alpha(body_fill, Color::role(Role::BgBase), 0.3 * a),
            );
        }
    }

    /// Draw a glass panel with hover illumination: a glow
    /// ring around the panel that appears on hover.
    /// The `hover_t` parameter (0.0..=1.0) controls the
    /// illumination strength.
    #[allow(clippy::too_many_arguments)]
    pub fn glass_panel_hover(
        &mut self,
        x: i64,
        y: i64,
        w: u32,
        h: u32,
        radius: i64,
        alpha: f32,
        hover_t: f32,
    ) {
        let ht = hover_t.clamp(0.0, 1.0);

        // Hover glow ring — appears when hovered.
        if ht > 0.01 {
            let glow_color = Color::role(Role::CrystalPrism);
            let spread = (6.0 * ht) as i64;
            let glow_alpha = 0.2 * ht;

            // Top glow.
            self.glow_line(
                x + radius,
                y - 1,
                w.saturating_sub((radius * 2) as u32),
                glow_color,
                glow_alpha * alpha,
            );
            // Bottom glow.
            self.glow_line(
                x + radius,
                y + h as i64,
                w.saturating_sub((radius * 2) as u32),
                glow_color,
                glow_alpha * alpha,
            );
            // Left glow (vertical line with spread).
            for dy in 0..h as i64 {
                let py = y + dy;
                let bg = self.get_pixel(x - 1, py).unwrap_or(Color::role(Role::BgBase));
                self.put_pixel(x - 1, py, blend_alpha(bg, glow_color, glow_alpha * alpha));
            }
            // Right glow.
            for dy in 0..h as i64 {
                let py = y + dy;
                let px = x + w as i64;
                let bg = self.get_pixel(px, py).unwrap_or(Color::role(Role::BgBase));
                self.put_pixel(px, py, blend_alpha(bg, glow_color, glow_alpha * alpha));
            }

            // Corner glow spots.
            let corner_r = (spread + 4) as u32;
            self.gradient_glow(
                x - spread,
                y - spread,
                corner_r,
                corner_r,
                glow_color,
                glow_alpha * 0.6 * alpha,
            );
            self.gradient_glow(
                x + w as i64 - corner_r as i64 + spread,
                y - spread,
                corner_r,
                corner_r,
                glow_color,
                glow_alpha * 0.6 * alpha,
            );
        }

        // Base glass panel.
        self.glass_panel(x, y, w, h, radius, alpha);

        // Hover brightens the top highlight.
        if ht > 0.01 {
            let highlight = blend_alpha(
                Color::role(Role::BgBase),
                Color::role(Role::GlassHighlight),
                0.3 * ht * alpha,
            );
            self.rounded_rect(x + 4, y + 1, w.saturating_sub(8), 2, 1, highlight);
        }
    }

    /// Draw the AI Orb: a prismatic circle with animated glow,
    /// crystal highlights, and state-driven color.
    ///
    /// `cx, cy` — center position.
    /// `radius` — orb radius in pixels.
    /// `state_color` — the AI state's accent color.
    /// `pulse` — animation pulse value (0.0..=1.0).
    /// `alpha` — overall opacity (0.0..=1.0).
    pub fn ai_orb(
        &mut self,
        cx: i64,
        cy: i64,
        radius: i64,
        state_color: Color,
        pulse: f32,
        alpha: f32,
    ) {
        let a = alpha.clamp(0.0, 1.0);
        let p = pulse.clamp(0.0, 1.0);

        // Outer glow ring — pulses with the animation.
        let glow_radius = radius + 8 + (p * 6.0) as i64;
        let glow_alpha = 0.15 + p * 0.25;
        self.gradient_glow(
            cx - glow_radius,
            cy - glow_radius,
            (glow_radius * 2) as u32,
            (glow_radius * 2) as u32,
            state_color,
            glow_alpha * a,
        );

        // Second ambient glow — wider, softer.
        let ambient_r = radius + 16 + (p * 4.0) as i64;
        self.gradient_glow(
            cx - ambient_r,
            cy - ambient_r,
            (ambient_r * 2) as u32,
            (ambient_r * 2) as u32,
            state_color,
            0.06 * a,
        );

        // Orb body — filled circle with glass material.
        for py in (cy - radius)..=(cy + radius) {
            for px in (cx - radius)..=(cx + radius) {
                let dx = px - cx;
                let dy = py - cy;
                let dist_sq = dx * dx + dy * dy;
                let r_sq = radius * radius;
                if dist_sq <= r_sq {
                    let dist = (dist_sq as f32).sqrt();
                    let t = dist / radius as f32; // 0 at center, 1 at edge

                    // Base fill: dark crystal surface.
                    let base = Color::role(Role::DcSurfaceStrong);

                    // State color tint — stronger at center.
                    let tint_strength = (1.0 - t * t) * 0.5 * a;
                    let mut fill = blend_alpha(base, state_color, tint_strength);

                    // Glass highlight — top-left bright spot.
                    let hl_x = cx - radius / 3;
                    let hl_y = cy - radius / 3;
                    let hl_dist =
                        (((px - hl_x) * (px - hl_x) + (py - hl_y) * (py - hl_y)) as f32).sqrt();
                    let hl_r = radius as f32 * 0.6;
                    if hl_dist < hl_r {
                        let hl_t = 1.0 - hl_dist / hl_r;
                        fill = blend_alpha(fill, Color::role(Role::CrystalShine), hl_t * 0.4 * a);
                    }

                    // Prismatic edge shimmer.
                    if t > 0.7 {
                        let edge_t = (t - 0.7) / 0.3;
                        let prism = lerp_color(
                            Color::role(Role::CrystalEdge),
                            Color::role(Role::CrystalPrism),
                            edge_t,
                        );
                        fill = blend_alpha(fill, prism, edge_t * 0.3 * a);
                    }

                    // Inner shadow at bottom for depth.
                    if t > 0.5 && dy > 0 {
                        let shadow_t = ((t - 0.5) / 0.5) * (dy as f32 / radius as f32);
                        fill = blend_alpha(fill, Color::role(Role::ShadowDeep), shadow_t * 0.3 * a);
                    }

                    self.put_pixel(px, py, fill);
                }
            }
        }

        // Crystal edge ring.
        for angle_deg in 0..360i64 {
            let angle = (angle_deg as f32) * std::f32::consts::PI / 180.0;
            let ex = cx + (radius as f32 * angle.cos()) as i64;
            let ey = cy + (radius as f32 * angle.sin()) as i64;
            let bg = self.get_pixel(ex, ey).unwrap_or(Color::role(Role::DcSurfaceStrong));
            let edge_color = lerp_color(
                Color::role(Role::CrystalEdge),
                Color::role(Role::CrystalShine),
                angle.cos() * 0.5 + 0.5,
            );
            self.put_pixel(ex, ey, blend_alpha(bg, edge_color, 0.5 * a));
        }
    }
}

/// Linear interpolation between two u8 values.
fn lerp(a: u8, b: u8, t: f32) -> u8 {
    let a_f = a as f32;
    let b_f = b as f32;
    (a_f + (b_f - a_f) * t).round().clamp(0.0, 255.0) as u8
}

/// Linear interpolation between two colors.
fn lerp_color(a: Color, b: Color, t: f32) -> Color {
    Color::rgb(lerp(a.r, b.r, t), lerp(a.g, b.g, t), lerp(a.b, b.b, t))
}

/// Blend `overlay` over `background` with the given alpha
/// (0.0 = fully background, 1.0 = fully overlay).
fn blend_alpha(background: Color, overlay: Color, alpha: f32) -> Color {
    let a = alpha.clamp(0.0, 1.0);
    let inv = 1.0 - a;
    Color::rgb(
        (background.r as f32 * inv + overlay.r as f32 * a).round() as u8,
        (background.g as f32 * inv + overlay.g as f32 * a).round() as u8,
        (background.b as f32 * inv + overlay.b as f32 * a).round() as u8,
    )
}

/// Compute the content rectangle inside a window frame.
fn content_rect(x: i64, y: i64, w: u32, h: u32, title_h: u32, border: u32) -> (i64, i64, u32, u32) {
    let cx = x + border as i64;
    let cy = y + title_h as i64 + border as i64;
    let cw = w.saturating_sub(border * 2);
    let ch = h.saturating_sub(title_h + border * 2);
    (cx, cy, cw, ch)
}

// -------------------------------------------------------------------- font

/// 5x7 bitmap font. Returns `Some(rows)` for supported
/// characters, `None` otherwise.
pub fn glyph_rows(ch: char) -> Option<&'static [&'static str]> {
    Some(match ch {
        ' ' => &["00000"; 7],
        '!' => &["00100", "00100", "00100", "00100", "00000", "00100", "00000"],
        '"' => &["01010", "01010", "00000", "00000", "00000", "00000", "00000"],
        '#' => &["01010", "01010", "11111", "01010", "11111", "01010", "01010"],
        '$' => &["00100", "01111", "10100", "01110", "00101", "11110", "00100"],
        '%' => &["11001", "11010", "00010", "00100", "01000", "01011", "10011"],
        '&' => &["01100", "10010", "10100", "01000", "10101", "10010", "01101"],
        '\'' => &["00100", "00100", "00000", "00000", "00000", "00000", "00000"],
        '(' => &["00010", "00100", "01000", "01000", "01000", "00100", "00010"],
        ')' => &["01000", "00100", "00010", "00010", "00010", "00100", "01000"],
        '*' => &["00000", "10101", "01110", "11111", "01110", "10101", "00000"],
        '+' => &["00000", "00100", "00100", "11111", "00100", "00100", "00000"],
        ',' => &["00000", "00000", "00000", "00000", "00100", "00100", "01000"],
        '-' => &["00000", "00000", "00000", "11111", "00000", "00000", "00000"],
        '.' => &["00000", "00000", "00000", "00000", "00000", "01100", "01100"],
        '/' => &["00001", "00010", "00010", "00100", "01000", "01000", "10000"],
        '0' => &["01110", "10001", "10011", "10101", "11001", "10001", "01110"],
        '1' => &["00100", "01100", "00100", "00100", "00100", "00100", "01110"],
        '2' => &["01110", "10001", "00001", "00110", "01000", "10000", "11111"],
        '3' => &["11110", "00001", "00001", "01110", "00001", "00001", "11110"],
        '4' => &["00010", "00110", "01010", "10010", "11111", "00010", "00010"],
        '5' => &["11111", "10000", "11110", "00001", "00001", "10001", "01110"],
        '6' => &["00110", "01000", "10000", "11110", "10001", "10001", "01110"],
        '7' => &["11111", "00001", "00010", "00100", "01000", "01000", "01000"],
        '8' => &["01110", "10001", "10001", "01110", "10001", "10001", "01110"],
        '9' => &["01110", "10001", "10001", "01111", "00001", "00010", "01100"],
        ':' => &["00000", "01100", "01100", "00000", "01100", "01100", "00000"],
        ';' => &["00000", "01100", "01100", "00000", "01100", "01100", "01000"],
        '<' => &["00010", "00100", "01000", "10000", "01000", "00100", "00010"],
        '=' => &["00000", "00000", "11111", "00000", "11111", "00000", "00000"],
        '>' => &["01000", "00100", "00010", "00001", "00010", "00100", "01000"],
        '?' => &["01110", "10001", "00001", "00110", "00100", "00000", "00100"],
        '@' => &["01110", "10001", "10111", "10101", "10110", "10000", "01111"],
        'A' => &["01110", "10001", "10001", "11111", "10001", "10001", "10001"],
        'B' => &["11110", "10001", "10001", "11110", "10001", "10001", "11110"],
        'C' => &["01111", "10000", "10000", "10000", "10000", "10000", "01111"],
        'D' => &["11110", "10001", "10001", "10001", "10001", "10001", "11110"],
        'E' => &["11111", "10000", "10000", "11110", "10000", "10000", "11111"],
        'F' => &["11111", "10000", "10000", "11110", "10000", "10000", "10000"],
        'G' => &["01111", "10000", "10000", "10111", "10001", "10001", "01110"],
        'H' => &["10001", "10001", "10001", "11111", "10001", "10001", "10001"],
        'I' => &["11111", "00100", "00100", "00100", "00100", "00100", "11111"],
        'J' => &["00111", "00010", "00010", "00010", "00010", "10010", "01100"],
        'K' => &["10001", "10010", "10100", "11000", "10100", "10010", "10001"],
        'L' => &["10000", "10000", "10000", "10000", "10000", "10000", "11111"],
        'M' => &["10001", "11011", "10101", "10101", "10001", "10001", "10001"],
        'N' => &["10001", "11001", "10101", "10011", "10001", "10001", "10001"],
        'O' => &["01110", "10001", "10001", "10001", "10001", "10001", "01110"],
        'P' => &["11110", "10001", "10001", "11110", "10000", "10000", "10000"],
        'Q' => &["01110", "10001", "10001", "10001", "10101", "10010", "01101"],
        'R' => &["11110", "10001", "10001", "11110", "10100", "10010", "10001"],
        'S' => &["01111", "10000", "10000", "01110", "00001", "00001", "11110"],
        'T' => &["11111", "00100", "00100", "00100", "00100", "00100", "00100"],
        'U' => &["10001", "10001", "10001", "10001", "10001", "10001", "01110"],
        'V' => &["10001", "10001", "10001", "10001", "01010", "01010", "00100"],
        'W' => &["10001", "10001", "10001", "10101", "10101", "11011", "10001"],
        'X' => &["10001", "10001", "01010", "00100", "01010", "10001", "10001"],
        'Y' => &["10001", "10001", "01010", "00100", "00100", "00100", "00100"],
        'Z' => &["11111", "00001", "00010", "00100", "01000", "10000", "11111"],
        '[' => &["11100", "10000", "10000", "10000", "10000", "10000", "11100"],
        '\\' => &["10000", "01000", "01000", "00100", "00010", "00010", "00001"],
        ']' => &["00111", "00001", "00001", "00001", "00001", "00001", "00111"],
        '^' => &["00100", "01010", "10001", "00000", "00000", "00000", "00000"],
        '_' => &["00000", "00000", "00000", "00000", "00000", "00000", "11111"],
        '`' => &["01000", "00100", "00000", "00000", "00000", "00000", "00000"],
        'a' => &["00000", "00000", "01110", "00001", "01111", "10001", "01111"],
        'b' => &["10000", "10000", "11110", "10001", "10001", "10001", "11110"],
        'c' => &["00000", "00000", "01111", "10000", "10000", "10000", "01111"],
        'd' => &["00001", "00001", "01111", "10001", "10001", "10001", "01111"],
        'e' => &["00000", "00000", "01110", "10001", "11111", "10000", "01111"],
        'f' => &["00110", "01001", "01000", "11100", "01000", "01000", "01000"],
        'g' => &["00000", "00000", "01111", "10001", "01111", "00001", "01110"],
        'h' => &["10000", "10000", "10110", "11001", "10001", "10001", "10001"],
        'i' => &["00100", "00000", "01100", "00100", "00100", "00100", "01110"],
        'j' => &["00010", "00000", "00110", "00010", "00010", "10010", "01100"],
        'k' => &["10000", "10000", "10010", "10100", "11000", "10100", "10010"],
        'l' => &["01100", "00100", "00100", "00100", "00100", "00100", "01110"],
        'm' => &["00000", "00000", "11010", "10101", "10101", "10001", "10001"],
        'n' => &["00000", "00000", "10110", "11001", "10001", "10001", "10001"],
        'o' => &["00000", "00000", "01110", "10001", "10001", "10001", "01110"],
        'p' => &["00000", "00000", "11110", "10001", "11110", "10000", "10000"],
        'q' => &["00000", "00000", "01111", "10001", "01111", "00001", "00001"],
        'r' => &["00000", "00000", "10110", "11001", "10000", "10000", "10000"],
        's' => &["00000", "00000", "01111", "10000", "01110", "00001", "11110"],
        't' => &["01000", "01000", "11100", "01000", "01000", "01001", "00110"],
        'u' => &["00000", "00000", "10001", "10001", "10001", "10011", "01101"],
        'v' => &["00000", "00000", "10001", "10001", "10001", "01010", "00100"],
        'w' => &["00000", "00000", "10001", "10001", "10101", "10101", "01010"],
        'x' => &["00000", "00000", "10001", "01010", "00100", "01010", "10001"],
        'y' => &["00000", "00000", "10001", "10001", "01111", "00001", "01110"],
        'z' => &["00000", "00000", "11111", "00010", "00100", "01000", "11111"],
        '{' => &["00110", "00100", "00100", "01000", "00100", "00100", "00110"],
        '|' => &["00100", "00100", "00100", "00100", "00100", "00100", "00100"],
        '}' => &["01100", "00100", "00100", "00010", "00100", "00100", "01100"],
        '~' => &["00000", "00000", "01000", "10101", "00010", "00000", "00000"],
        _ => return None,
    })
}

// -------------------------------------------------------------------- text

/// Render a single character at the given position with
/// the given scale factor. Returns the advance width.
pub fn draw_glyph(buf: &mut PixelBuffer<'_>, ch: char, x: i64, y: i64, s: u32, c: Color) -> i64 {
    if let Some(rows) = glyph_rows(ch) {
        for (ry, row) in rows.iter().enumerate() {
            if let Ok(bits) = u8::from_str_radix(row, 2) {
                for rx in 0..5usize {
                    if (bits >> (4 - rx)) & 1 == 1 {
                        buf.rect(x + (rx as u32 * s) as i64, y + (ry as u32 * s) as i64, s, s, c);
                    }
                }
            }
        }
    }
    6 * s as i64
}

/// Render a string at the given position with scale.
pub fn draw_text(buf: &mut PixelBuffer<'_>, text: &str, x: i64, y: i64, s: u32, c: Color) -> i64 {
    let mut cx = x;
    for ch in text.chars() {
        cx += draw_glyph(buf, ch, cx, y, s, c);
    }
    cx - x
}

/// Measure the width of a string in pixels.
#[must_use]
pub fn text_width(text: &str, s: u32) -> i64 {
    text.len() as i64 * 6 * s as i64
}

/// Center a string horizontally in a region.
#[must_use]
pub fn centered_x(region_x: i64, region_w: u32, text: &str, s: u32) -> i64 {
    let tw = text_width(text, s);
    region_x + ((region_w as i64 - tw) / 2).max(0)
}

// --------------------------------------------------------------- color bridge

/// Convert an Aether `Color` to the framebuffer `Rgb` type
/// used by the graphical shell.
#[must_use]
pub fn color_to_fb(c: Color) -> [u8; 3] {
    [c.r, c.g, c.b]
}

/// Get the semantic color for a component role.
#[must_use]
pub fn role_color(role: Role) -> Color {
    Color::role(role)
}

// ----------------------------------------------------------- component renderer

/// Renders UI components into a `PixelBuffer` using the
/// design token system.
pub struct ComponentRenderer<'a> {
    /// The pixel buffer to render into.
    pub buf: &'a mut PixelBuffer<'a>,
}

impl<'a> ComponentRenderer<'a> {
    /// Create a new renderer targeting the given buffer.
    #[must_use]
    pub fn new(buf: &'a mut PixelBuffer<'a>) -> Self {
        Self { buf }
    }

    /// Render a filled rounded rectangle from a
    /// `ComponentStyle`.
    pub fn fill_style(&mut self, x: i64, y: i64, w: u32, h: u32, style: &ComponentStyle) {
        self.buf.rounded_rect(x, y, w, h, style.radius.px() as i64, style.fill);
    }

    /// Render a button component.
    #[allow(clippy::too_many_arguments)]
    pub fn render_button(
        &mut self,
        x: i64,
        y: i64,
        w: u32,
        h: u32,
        style: &ComponentStyle,
        label: &str,
        scale: u32,
    ) {
        // Background.
        self.buf.rounded_rect(x, y, w, h, style.radius.px() as i64, style.fill);
        // Border (hairline).
        if style.border != style.fill {
            draw_rect_outline(self.buf, x, y, w, h, style.radius.px() as i64, style.border);
        }
        // Label centered.
        let lx = centered_x(x, w, label, scale);
        let ly = y + ((h as i64 - 7 * scale as i64) / 2).max(0);
        draw_text(self.buf, label, lx, ly, scale, style.text);
    }

    /// Render a card component.
    pub fn render_card(
        &mut self,
        x: i64,
        y: i64,
        w: u32,
        h: u32,
        style: &ComponentStyle,
        title: Option<&str>,
    ) {
        // Shadow (for raised/overlay).
        self.buf.shadow(x, y, w, h, style.radius.px() as i64, 2, 4, Color::role(Role::Shadow));
        // Background.
        self.buf.rounded_rect(x, y, w, h, style.radius.px() as i64, style.fill);
        // Border.
        draw_rect_outline(self.buf, x, y, w, h, style.radius.px() as i64, style.border);
        // Title.
        if let Some(title) = title {
            let lx = x + style.radius.px() as i64;
            let ly = y + Spacing::Lg.px() as i64;
            draw_text(self.buf, &title.to_uppercase(), lx, ly, 2, style.text);
        }
    }

    /// Render a panel (sidebar / drawer).
    pub fn render_panel(&mut self, x: i64, y: i64, w: u32, h: u32, style: &ComponentStyle) {
        self.buf.rect(x, y, w, h, style.fill);
        // Hairline on the inner edge.
        self.buf.rect(x + w as i64 - 1, y, 1, h, style.border);
    }

    /// Render a taskbar.
    pub fn render_taskbar(&mut self, x: i64, y: i64, w: u32, h: u32, style: &ComponentStyle) {
        self.buf.rect(x, y, w, h, style.fill);
        // Top accent line.
        self.buf.rect(x, y, w, 2, Color::role(Role::AccentLavender));
    }

    /// Render text with a semantic type scale.
    pub fn render_text(&mut self, text: &str, x: i64, y: i64, scale: TypeScale, c: Color) {
        let s = scale.step();
        // Map type scale size to our bitmap scale factor.
        let bitmap_scale = match s.size_px {
            0..=12 => 1,
            13..=18 => 2,
            19..=24 => 2,
            _ => 3,
        };
        draw_text(self.buf, text, x, y, bitmap_scale, c);
    }
}

/// Draw a rounded rectangle outline (border only).
pub fn draw_rect_outline(
    buf: &mut PixelBuffer<'_>,
    x: i64,
    y: i64,
    w: u32,
    h: u32,
    radius: i64,
    c: Color,
) {
    // Top and bottom edges.
    for col in x..(x + w as i64) {
        if PixelBuffer::inside_rounded_rect(col, y, x, y, w, h, radius) {
            buf.put_pixel(col, y, c);
        }
        if PixelBuffer::inside_rounded_rect(col, y + h as i64 - 1, x, y, w, h, radius) {
            buf.put_pixel(col, y + h as i64 - 1, c);
        }
    }
    // Left and right edges.
    for row in y..(y + h as i64) {
        if PixelBuffer::inside_rounded_rect(x, row, x, y, w, h, radius) {
            buf.put_pixel(x, row, c);
        }
        if PixelBuffer::inside_rounded_rect(x + w as i64 - 1, row, x, y, w, h, radius) {
            buf.put_pixel(x + w as i64 - 1, row, c);
        }
    }
}

// -------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;
    use aether_design_tokens::Radius;

    #[test]
    fn pixel_from_color() {
        let c = Color::rgb(255, 128, 0);
        let p = Pixel::from_color(c);
        assert_eq!(p, Pixel(0, 128, 255, 0xFF));
    }

    #[test]
    fn buffer_dimensions() {
        let mut data = vec![0u8; 20000];
        let buf = PixelBuffer::from_raw(100, 50, 400, &mut data);
        assert_eq!(buf.width, 100);
        assert_eq!(buf.height, 50);
        assert_eq!(buf.stride, 400);
        assert_eq!(buf.data.len(), 20000);
    }

    #[test]
    fn fill_sets_all_pixels() {
        let mut data = vec![0u8; 64];
        let mut buf = PixelBuffer::from_raw(4, 4, 16, &mut data);
        buf.fill(Color::rgb(100, 200, 50));
        for chunk in buf.data.chunks_exact(4) {
            assert_eq!(chunk[0], 50); // B
            assert_eq!(chunk[1], 200); // G
            assert_eq!(chunk[2], 100); // R
            assert_eq!(chunk[3], 0xFF);
        }
    }

    #[test]
    fn rect_fills_correct_region() {
        let mut data = vec![0u8; 400];
        let mut buf = PixelBuffer::from_raw(10, 10, 40, &mut data);
        buf.fill(Color::rgb(0, 0, 0));
        buf.rect(2, 2, 3, 3, Color::rgb(255, 0, 0));
        // Check a pixel inside.
        assert_eq!(buf.get_pixel(3, 3), Some(Color::rgb(255, 0, 0)));
        // Check a pixel outside.
        assert_eq!(buf.get_pixel(0, 0), Some(Color::rgb(0, 0, 0)));
    }

    #[test]
    fn rounded_rect_basic() {
        let mut data = vec![0u8; 1600];
        let mut buf = PixelBuffer::from_raw(20, 20, 80, &mut data);
        buf.fill(Color::rgb(0, 0, 0));
        buf.rounded_rect(0, 0, 20, 20, 6, Color::rgb(255, 255, 255));
        // Center should be filled.
        assert_eq!(buf.get_pixel(10, 10), Some(Color::rgb(255, 255, 255)));
    }

    #[test]
    fn text_width_calculation() {
        assert_eq!(text_width("HI", 2), 24); // 2 chars × 6 × 2
        assert_eq!(text_width("HELLO", 1), 30);
    }

    #[test]
    fn glyph_rows_supports_lowercase() {
        assert!(glyph_rows('a').is_some());
        assert!(glyph_rows('z').is_some());
        assert!(glyph_rows('m').is_some());
    }

    #[test]
    fn glyph_rows_supports_punctuation() {
        assert!(glyph_rows('!').is_some());
        assert!(glyph_rows('?').is_some());
        assert!(glyph_rows('@').is_some());
        assert!(glyph_rows('#').is_some());
    }

    #[test]
    fn lerp_basic() {
        assert_eq!(lerp(0, 100, 0.0), 0);
        assert_eq!(lerp(0, 100, 0.5), 50);
        assert_eq!(lerp(0, 100, 1.0), 100);
    }

    #[test]
    fn component_button_renders_label() {
        let style = ComponentStyle::from_roles(
            Role::AccentLavender,
            Role::TextPrimary,
            Role::Hairline,
            Radius::Md,
        );
        let mut data = vec![0u8; 800 * 200];
        {
            let mut buf = PixelBuffer::from_raw(200, 200, 800, &mut data);
            buf.fill(Color::rgb(0, 0, 0));
            buf.rounded_rect(10, 10, 120, 40, style.radius.px() as i64, style.fill);
        }
        let buf2 = PixelBuffer::from_raw(200, 200, 800, &mut data);
        assert_ne!(buf2.get_pixel(70, 30), Some(Color::rgb(0, 0, 0)));
    }

    #[test]
    fn component_panel_renders() {
        let style = ComponentStyle::from_roles(
            Role::BgPanel,
            Role::TextPrimary,
            Role::Hairline,
            Radius::Lg,
        );
        let mut data = vec![0u8; 400 * 300];
        {
            let mut buf = PixelBuffer::from_raw(100, 300, 400, &mut data);
            buf.fill(Color::rgb(0, 0, 0));
            buf.rounded_rect(0, 0, 100, 300, style.radius.px() as i64, style.fill);
        }
        let buf2 = PixelBuffer::from_raw(100, 300, 400, &mut data);
        assert_ne!(buf2.get_pixel(50, 150), Some(Color::rgb(0, 0, 0)));
    }

    #[test]
    fn component_card_with_title() {
        let style =
            ComponentStyle::from_roles(Role::BgBase, Role::TextPrimary, Role::Hairline, Radius::Lg);
        let mut data = vec![0u8; 600 * 400];
        {
            let mut buf = PixelBuffer::from_raw(200, 400, 600, &mut data);
            buf.fill(Color::rgb(0, 0, 0));
            buf.rounded_rect(10, 10, 180, 80, style.radius.px() as i64, style.fill);
        }
        let buf2 = PixelBuffer::from_raw(200, 400, 600, &mut data);
        assert_ne!(buf2.get_pixel(100, 50), Some(Color::rgb(0, 0, 0)));
    }

    #[test]
    fn shadow_does_not_crash() {
        let mut data = vec![0u8; 400 * 400];
        let mut buf = PixelBuffer::from_raw(100, 100, 400, &mut data);
        buf.fill(Color::rgb(200, 200, 200));
        buf.shadow(10, 10, 80, 80, 8, 3, 4, Color::rgb(0, 0, 0));
        let shadow_pixel = buf.get_pixel(95, 95);
        assert!(shadow_pixel.is_some());
    }

    #[test]
    fn draw_text_returns_end_x() {
        let mut data = vec![0u8; 400 * 100];
        let mut buf = PixelBuffer::from_raw(100, 100, 400, &mut data);
        let end_x = draw_text(&mut buf, "HI", 10, 10, 1, Color::rgb(255, 255, 255));
        assert_eq!(end_x, text_width("HI", 1));
    }

    #[test]
    fn draw_rect_outline_only_border() {
        let mut data = vec![0u8; 400 * 400];
        let mut buf = PixelBuffer::from_raw(100, 100, 400, &mut data);
        buf.fill(Color::rgb(0, 0, 0));
        draw_rect_outline(&mut buf, 10, 10, 80, 80, 0, Color::rgb(255, 255, 255));
        assert_eq!(buf.get_pixel(10, 10), Some(Color::rgb(255, 255, 255)));
        assert_eq!(buf.get_pixel(50, 50), Some(Color::rgb(0, 0, 0)));
    }

    #[test]
    fn full_taskbar_scene() {
        let mut data = vec![0u8; 5120 * 800];
        let style = ComponentStyle::from_roles(
            Role::BgPanel,
            Role::TextPrimary,
            Role::Hairline,
            Radius::Md,
        );
        {
            let mut buf = PixelBuffer::from_raw(1280, 800, 5120, &mut data);
            buf.fill(Color::role(Role::BgBase));
            buf.rounded_rect(0, 0, 1280, 36, style.radius.px() as i64, style.fill);
            buf.rounded_rect(20, 50, 120, 40, style.radius.px() as i64, style.fill);
            buf.rounded_rect(160, 50, 120, 40, style.radius.px() as i64, style.fill);
            buf.rounded_rect(300, 50, 400, 200, style.radius.px() as i64, style.fill);
        }
        let buf2 = PixelBuffer::from_raw(1280, 800, 5120, &mut data);
        assert_ne!(buf2.get_pixel(0, 0), Some(Color::rgb(0, 0, 0)));
        assert_ne!(buf2.get_pixel(80, 70), Some(Color::rgb(0, 0, 0)));
        assert_ne!(buf2.get_pixel(500, 150), Some(Color::rgb(0, 0, 0)));
    }
}
