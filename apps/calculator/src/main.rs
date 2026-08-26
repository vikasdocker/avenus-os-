// Aether Calculator - minimal graphical Aether application.
//
// Validates the application runtime lifecycle:
//   DISCOVER -> LAUNCH -> RUN (graphical surface visible) -> CLOSE.
//
// Surface model: the app claims the exclusive display surface via a
// pid-stamped lock file (/run/aether/surface-holder). The graphical shell
// pauses its own repaints while the lock is alive and reclaims the screen
// once the holder disappears. This is surface arbitration, not a window
// manager.

use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::process::ExitCode;
use std::time::Duration;

const BG: Rgb = Rgb(14, 17, 22);
const PANEL: Rgb = Rgb(28, 34, 44);
const CYAN: Rgb = Rgb(34, 211, 238);
const FG: Rgb = Rgb(230, 237, 243);
const DIM: Rgb = Rgb(122, 132, 142);
const KEY: Rgb = Rgb(52, 62, 78);
const GREEN: Rgb = Rgb(74, 222, 128);

#[derive(Clone, Copy)]
struct Rgb(u8, u8, u8);

fn pixel(c: Rgb) -> [u8; 4] {
    [c.2, c.1, c.0, 0xFF]
}

const SURFACE_HOLDER: &str = "/run/aether/surface-holder";

// ------------------------------------------------------------ framebuffer

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
        .map_err(|_| "bad stride".to_string())?;
    if bpp.trim() != "32" {
        return Err(format!("unsupported bpp {bpp}"));
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
            cx += (6 * s) as i64;
        }
    }

    fn centered_in(&self, x: i64, w: u32, text: &str, s: u32) -> i64 {
        let tw = text.len() as u32 * 6 * s;
        x + ((w.saturating_sub(tw)) / 2) as i64
    }

    fn flush(&mut self) -> Result<(), String> {
        self.file
            .seek(SeekFrom::Start(0))
            .and_then(|_| self.file.write_all(&self.buf))
            .map_err(|e| format!("framebuffer write failed: {e}"))
    }
}

/// 5x7 font subset for the calculator surface.
fn glyph_rows(ch: char) -> Option<&'static [&'static str]> {
    Some(match ch {
        ' ' => &["00000"; 7],
        '.' => &["00000", "00000", "00000", "00000", "00000", "01100", "01100"],
        '/' => &["00001", "00010", "00010", "00100", "01000", "01000", "10000"],
        '*' => &["00000", "10101", "01110", "11111", "01110", "10101", "00000"],
        '+' => &["00000", "00100", "00100", "11111", "00100", "00100", "00000"],
        '-' => &["00000", "00000", "00000", "11111", "00000", "00000", "00000"],
        '=' => &["00000", "00000", "11111", "00000", "11111", "00000", "00000"],
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
        'A' => &["01110", "10001", "10001", "11111", "10001", "10001", "10001"],
        'C' => &["01111", "10000", "10000", "10000", "10000", "10000", "01111"],
        'D' => &["11110", "10001", "10001", "10001", "10001", "10001", "11110"],
        'E' => &["11111", "10000", "10000", "11110", "10000", "10000", "11111"],
        'H' => &["10001", "10001", "10001", "11111", "10001", "10001", "10001"],
        'I' => &["11111", "00100", "00100", "00100", "00100", "00100", "11111"],
        'L' => &["10000", "10000", "10000", "10000", "10000", "10000", "11111"],
        'M' => &["10001", "11011", "10101", "10101", "10001", "10001", "10001"],
        'O' => &["01110", "10001", "10001", "10001", "10001", "10001", "01110"],
        'P' => &["11110", "10001", "10001", "11110", "10000", "10000", "10000"],
        'R' => &["11110", "10001", "10001", "11110", "10100", "10010", "10001"],
        'T' => &["11111", "00100", "00100", "00100", "00100", "00100", "00100"],
        'U' => &["10001", "10001", "10001", "10001", "10001", "10001", "01110"],
        'V' => &["10001", "10001", "10001", "10001", "01010", "01010", "00100"],
        _ => return None,
    })
}

// ---------------------------------------------------------------- drawing

const BUTTON_LABELS: &[&[&str]] = &[
    &["7", "8", "9", "/"],
    &["4", "5", "6", "*"],
    &["1", "2", "3", "-"],
    &["0", ".", "=", "+"],
];

fn paint(fb: &mut Screen, pid: u32) {
    fb.fill(BG);

    // Centered calculator panel.
    let pw = 420u32;
    let ph = 560u32;
    let px = ((fb.width.saturating_sub(pw)) / 2) as i64;
    let py = ((fb.height.saturating_sub(ph)) / 2) as i64;

    // Window frame + title bar.
    fb.rect(px, py, pw, ph, PANEL);
    fb.rect(px, py, pw, 6, CYAN);
    fb.text(
        "AETHER CALCULATOR",
        fb.centered_in(px, pw, "AETHER CALCULATOR", 2),
        py + 22,
        2,
        CYAN,
    );

    // Display strip.
    let disp_y = py + 60;
    fb.rect(px + 24, disp_y, pw - 48, 70, BG);
    fb.rect(px + 24, disp_y, pw - 48, 3, DIM);
    fb.text("0", px + i64::from(pw) - 24 - 24, disp_y + 30, 4, FG);

    // Button grid.
    let grid_x = px + 24;
    let grid_y = disp_y + 96;
    let cell_w = (pw - 48 - 3 * 12) / 4;
    let cell_h = 74u32;
    for (row, labels) in BUTTON_LABELS.iter().enumerate() {
        for (col, label) in labels.iter().enumerate() {
            let bx = grid_x + i64::from(col as u32 * (cell_w + 12));
            let by = grid_y + i64::from(row as u32 * (cell_h + 12));
            fb.rect(bx, by, cell_w, cell_h, KEY);
            fb.text(
                label,
                fb.centered_in(bx, cell_w, label, 3),
                by + (cell_h.saturating_sub(7 * 3)) as i64 / 2,
                3,
                FG,
            );
        }
    }

    // Identity footer: proves this surface belongs to an Aether app.
    fb.text(
        "AETHER OS APP",
        fb.centered_in(px, pw, "AETHER OS APP", 2),
        py + ph as i64 - 40,
        2,
        GREEN,
    );
    let pid_text = format!("PID {pid}");
    fb.text(
        &pid_text,
        fb.centered_in(px, pw, &pid_text, 1),
        py + ph as i64 - 18,
        1,
        DIM,
    );
}

// ------------------------------------------------------- surface claiming

/// Claims the exclusive display surface on behalf of this pid.
fn claim_surface(pid: u32) -> Result<(), String> {
    std::fs::create_dir_all("/run/aether")
        .map_err(|e| format!("mkdir /run/aether: {e}"))?;
    std::fs::write(SURFACE_HOLDER, format!("{pid}\n"))
        .map_err(|e| format!("claim surface: {e}"))
}

/// Releases the claim when this pid still owns it.
fn release_surface(pid: u32) {
    if let Ok(holder) = std::fs::read_to_string(SURFACE_HOLDER) {
        if holder.trim() == pid.to_string() {
            let _ = std::fs::remove_file(SURFACE_HOLDER);
        }
    }
}

fn run() -> Result<(), String> {
    // Identify ourselves to the OS before touching the display.
    let pid = std::process::id();
    eprintln!("[calculator] started as aether app (pid {pid})");
    claim_surface(pid)?;

    let (file, width, height, stride) = open_framebuffer()?;
    eprintln!("[calculator] surface acquired {width}x{height}");
    let mut fb = Screen::new(file, width, height, stride);

    paint(&mut fb, pid);
    fb.flush()?;
    eprintln!("[calculator] calculator panel visible");

    // Keep the surface fresh until the runtime closes us.
    loop {
        std::thread::sleep(Duration::from_secs(2));
        paint(&mut fb, pid);
        fb.flush()?;
    }
}

fn main() -> ExitCode {
    let result = run();
    release_surface(std::process::id());
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[calculator][FAIL] {e}");
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
    fn button_labels_have_glyphs() {
        for row in BUTTON_LABELS {
            for label in *row {
                for ch in label.chars() {
                    assert!(
                        glyph_rows(ch).is_some(),
                        "missing glyph '{ch}' for button label"
                    );
                }
            }
        }
    }

    #[test]
    fn title_has_glyphs() {
        for ch in "AETHER CALCULATOR 0123456789 PID ".trim().chars() {
            assert!(glyph_rows(ch).is_some(), "missing glyph '{ch}'");
        }
    }
}
