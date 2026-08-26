// Aether Graphical Shell - minimal startup surface.
//
// Proves the pipeline QEMU virtio-gpu -> Linux DRM/KMS -> Aether pixels:
// opens the kernel framebuffer exposed by virtio_gpudrmfb (/dev/fb0),
// renders a static Aether OS splash with an embedded 5x7 bitmap font,
// and keeps it on screen. No window manager, no compositor, no unsafe.
//
// Framebuffer geometry is read from sysfs so no ioctl/unsafe is needed;
// frames are pushed with a single seek+write of the full buffer.

use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::process::ExitCode;
use std::time::{Duration, Instant};

/// Palette (RGB).
const BG: Rgb = Rgb(14, 17, 22);
const CYAN: Rgb = Rgb(34, 211, 238);
const FG: Rgb = Rgb(230, 237, 243);
const DIM: Rgb = Rgb(122, 132, 142);
const GREEN: Rgb = Rgb(74, 222, 128);

const TITLE: &str = "AETHER OS";
const SUBTITLE: &str = "AI-NATIVE OPERATING SYSTEM";
const FOOTER: &str = "SERVICES UP - GPU READY";

const REDRAW_INTERVAL: Duration = Duration::from_secs(10);

#[derive(Clone, Copy)]
struct Rgb(u8, u8, u8);

/// Little-endian XRGB8888 as exposed by virtio_gpudrmfb: bytes are B,G,R,X.
fn pixel(c: Rgb) -> [u8; 4] {
    [c.2, c.1, c.0, 0xFF]
}

/// Open the fbdev device and read geometry from sysfs (no ioctls).
fn open_framebuffer() -> Result<(File, u32, u32, u32), String> {
    let file = OpenOptions::new()
        .write(true)
        .open("/dev/fb0")
        .map_err(|e| format!("cannot open /dev/fb0: {e}"))?;

    let read_sysfs = |name: &str| -> Result<String, String> {
        std::fs::read_to_string(format!("/sys/class/graphics/fb0/{name}"))
            .map_err(|e| format!("cannot read fb0/{name}: {e}"))
    };

    let virtual_size = read_sysfs("virtual_size")?;
    let stride = read_sysfs("stride")?;
    let bpp = read_sysfs("bits_per_pixel")?;

    let mut dims = virtual_size.trim().split(',');
    let width = dims
        .next()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .ok_or_else(|| format!("bad virtual_size '{virtual_size}'"))?;
    let height = dims
        .next()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .ok_or_else(|| format!("bad virtual_size '{virtual_size}'"))?;
    let stride = stride
        .trim()
        .parse::<u32>()
        .map_err(|_| format!("bad stride '{}'", stride.trim()))?;
    let bpp = bpp
        .trim()
        .parse::<u32>()
        .map_err(|_| format!("bad bits_per_pixel '{}'", bpp.trim()))?;

    if bpp != 32 {
        return Err(format!("unsupported bits_per_pixel {bpp}; need 32"));
    }
    if width == 0 || height == 0 || stride < width * 4 {
        return Err(format!("nonsensical geometry {width}x{height} stride {stride}"));
    }
    Ok((file, width, height, stride))
}

struct Screen {
    file: File,
    width: u32,
    height: u32,
    stride: u32,
    buf: Vec<u8>,
}

impl Screen {
    fn new(file: File, width: u32, height: u32, stride: u32) -> Self {
        Self {
            buf: vec![0; (stride * height) as usize],
            file,
            width,
            height,
            stride,
        }
    }

    fn fill(&mut self, c: Rgb) {
        for chunk in self.buf.chunks_exact_mut(4) {
            chunk.copy_from_slice(&pixel(c));
        }
    }

    fn rect(&mut self, x: i64, y: i64, w: u32, h: u32, c: Rgb) {
        let px = pixel(c);
        for row in y.max(0)..(y + h as i64).min(self.height as i64) {
            let start = (x.max(0) as u32 * 4) as usize;
            let end = ((x + w as i64).min(self.width as i64) as u32 * 4) as usize;
            if end <= start {
                continue;
            }
            let base = row as usize * self.stride as usize;
            for chunk in self.buf[base + start..base + end].chunks_exact_mut(4) {
                chunk.copy_from_slice(&px);
            }
        }
    }

    /// Blits one glyph row-bit pattern scaled by `s` at (x, y).
    fn glyph(&mut self, ch: char, x: i64, y: i64, s: u32, c: Rgb) {
        if let Some(rows) = glyph_rows(ch) {
            for (ry, row) in rows.iter().enumerate() {
                let Ok(bits) = u8::from_str_radix(row, 2) else { continue };
                for rx in 0..5usize {
                    if (bits >> (4 - rx)) & 1 == 1 {
                        self.rect(
                            x + (rx as u32 * s) as i64,
                            y + (ry as u32 * s) as i64,
                            s,
                            s,
                            c,
                        );
                    }
                }
            }
        }
    }

    fn text(&mut self, text: &str, x: i64, y: i64, s: u32, c: Rgb) {
        let mut cx = x;
        for ch in text.chars() {
            self.glyph(ch, cx, y, s, c);
            cx += (6 * s) as i64; // 5 columns + 1 space column
        }
    }

    fn flush(&mut self) -> Result<(), String> {
        self.file
            .seek(SeekFrom::Start(0))
            .and_then(|_| self.file.write_all(&self.buf))
            .map_err(|e| format!("framebuffer write failed: {e}"))
    }

    fn centered_x(&self, text: &str, s: u32) -> i64 {
        let text_w = text.len() as u32 * 6 * s;
        self.width.saturating_sub(text_w) as i64 / 2
    }
}

/// Embedded 5x7 font subset (only glyphs this screen needs).
fn glyph_rows(ch: char) -> Option<&'static [&'static str]> {
    Some(match ch {
        ' ' => &["00000"; 7],
        '-' => &["00000", "00000", "00000", "11111", "00000", "00000", "00000"],
        'A' => &["01110", "10001", "10001", "11111", "10001", "10001", "10001"],
        'C' => &["01111", "10000", "10000", "10000", "10000", "10000", "01111"],
        'D' => &["11110", "10001", "10001", "10001", "10001", "10001", "11110"],
        'E' => &["11111", "10000", "10000", "11110", "10000", "10000", "11111"],
        'G' => &["01111", "10000", "10000", "10111", "10001", "10001", "01110"],
        'H' => &["10001", "10001", "10001", "11111", "10001", "10001", "10001"],
        'I' => &["11111", "00100", "00100", "00100", "00100", "00100", "11111"],
        'M' => &["10001", "11011", "10101", "10101", "10001", "10001", "10001"],
        'N' => &["10001", "11001", "10101", "10011", "10001", "10001", "10001"],
        'O' => &["01110", "10001", "10001", "10001", "10001", "10001", "01110"],
        'P' => &["11110", "10001", "10001", "11110", "10000", "10000", "10000"],
        'R' => &["11110", "10001", "10001", "11110", "10100", "10010", "10001"],
        'S' => &["01111", "10000", "10000", "01110", "00001", "00001", "11110"],
        'T' => &["11111", "00100", "00100", "00100", "00100", "00100", "00100"],
        'U' => &["10001", "10001", "10001", "10001", "10001", "10001", "01110"],
        'V' => &["10001", "10001", "10001", "10001", "01010", "01010", "00100"],
        'Y' => &["10001", "10001", "01010", "00100", "00100", "00100", "00100"],
        _ => return None,
    })
}

fn paint(fb: &mut Screen) {
    fb.fill(BG);
    // Thin accent bar across the top edge.
    fb.rect(0, 0, fb.width, 3, CYAN);

    let title_s = 10u32;
    let title_y = fb.height as i64 / 2 - 160;
    fb.text(TITLE, fb.centered_x(TITLE, title_s), title_y, title_s, FG);
    // Underline accent under the title.
    let underline_w = (TITLE.len() as u32 * 6 * title_s).min(fb.width);
    fb.rect(fb.centered_x(TITLE, title_s), title_y + 80, underline_w, 6, CYAN);

    let sub_s = 4u32;
    fb.text(SUBTITLE, fb.centered_x(SUBTITLE, sub_s), title_y + 130, sub_s, DIM);

    let foot_s = 3u32;
    let foot_y = fb.height as i64 - foot_s as i64 * 14 - 40;
    fb.text(FOOTER, fb.centered_x(FOOTER, foot_s), foot_y, foot_s, GREEN);
}

fn run() -> Result<(), String> {
    let (file, width, height, stride) = open_framebuffer()?;
    eprintln!("[gfx] fb0 {width}x{height} stride {stride} ok");
    let mut fb = Screen::new(file, width, height, stride);

    paint(&mut fb);
    fb.flush()?;
    eprintln!("[gfx] aether splash visible");

    let mut last = Instant::now();
    loop {
        std::thread::sleep(Duration::from_millis(500));
        if last.elapsed() >= REDRAW_INTERVAL {
            // Repaint defensively in case console text scrolled over us.
            paint(&mut fb);
            fb.flush()?;
            last = Instant::now();
        }
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[gfx][FAIL] {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_is_little_endian_bgr_order() {
        assert_eq!(pixel(Rgb(1, 2, 3)), [3, 2, 1, 0xFF]);
    }

    #[test]
    fn every_glyph_has_seven_five_bit_rows() {
        for ch in "AETHEROS AI-NATIVE OPERATING SYSTEM SERVICES UP GPU READY"
            .chars()
            .filter(|c| *c != ' ')
        {
            let rows =
                glyph_rows(ch).unwrap_or_else(|| panic!("missing glyph '{ch}'"));
            assert_eq!(rows.len(), 7, "glyph '{ch}' must have 7 rows");
            for r in rows {
                assert_eq!(r.len(), 5, "glyph '{ch}' row '{r}' must be 5 wide");
                assert!(r.bytes().all(|b| b == b'0' || b == b'1'));
            }
        }
    }

    #[test]
    fn rect_clips_out_of_bounds() {
        let (f, w, h, st) = {
            // Tiny in-memory stand-in via pipe-backed file is awkward; use
            // Screen directly with a throwaway File from /dev/null equivalent.
            let f = tempfile_stub();
            (f, 8, 8, 32)
        };
        let mut scr = Screen::new(f, w, h, st);
        scr.fill(BG);
        scr.rect(-4, -4, 16, 16, CYAN); // must not panic
        assert!(scr.buf.chunks_exact(4).all(|p| p == pixel(CYAN)));
    }

    fn tempfile_stub() -> File {
        File::create(std::env::temp_dir().join("aether-gfx-test.bin"))
            .unwrap_or_else(|e| panic!("{e}"))
    }
}
