// Aether Calculator - graphical test application.
//
// Registers a window with the desktop shell, paints a mock calculator
// panel inside its content rectangle, and exits cleanly when the shell
// asks it to close.

use aether_surface::{Surface, SurfaceEvent};
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::time::Duration;

const BG: [u8; 3] = [28, 34, 44];
const DISPLAY: [u8; 3] = [14, 17, 22];
const KEY: [u8; 3] = [52, 62, 78];
const ACCENT: [u8; 3] = [34, 211, 238];
const FG: [u8; 3] = [230, 237, 243];
const GREEN: [u8; 3] = [74, 222, 128];

struct Region {
    file: File,
    stride: u32,
    rect: aether_surface::Rect,
}

impl Region {
    fn open(rect: aether_surface::Rect) -> Result<Self, String> {
        let file =
            OpenOptions::new().write(true).open("/dev/fb0").map_err(|e| format!("fb0: {e}"))?;
        Ok(Self { file, stride: read_stride()?, rect })
    }

    fn put(&mut self, dx: u32, dy: u32, w: u32, h: u32, rgb: &[u8; 3]) -> Result<(), String> {
        let px = [rgb[2], rgb[1], rgb[0], 0xFF];
        let x = self.rect.x + dx as i32;
        let y = self.rect.y + dy as i32;
        let bw = w.min(self.rect.width.saturating_sub(dx)) as usize * 4;
        let bh = h.min(self.rect.height.saturating_sub(dy));
        let mut row = vec![0u8; bw];
        for chunk in row.chunks_exact_mut(4) {
            chunk.copy_from_slice(&px);
        }
        for r in 0..bh {
            let off = ((y + r as i32) * self.stride as i32 + x * 4) as u64;
            self.file.seek(SeekFrom::Start(off)).map_err(|e| e.to_string())?;
            self.file.write_all(&row).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn fill(&mut self) -> Result<(), String> {
        self.put(0, 0, self.rect.width, self.rect.height, &BG)
    }
}

/// Embedded 5x7 glyph rows for every character this app renders.
fn font(ch: char) -> Option<Vec<&'static str>> {
    Some(match ch.to_ascii_uppercase() {
        '0' => vec!["01110", "10001", "10011", "10101", "11001", "10001", "01110"],
        '1' => vec!["00100", "01100", "00100", "00100", "00100", "00100", "01110"],
        '2' => vec!["01110", "10001", "00001", "00110", "01000", "10000", "11111"],
        '3' => vec!["11110", "00001", "00001", "01110", "00001", "00001", "11110"],
        '4' => vec!["00010", "00110", "01010", "10010", "11111", "00010", "00010"],
        '5' => vec!["11111", "10000", "11110", "00001", "00001", "10001", "01110"],
        '6' => vec!["00110", "01000", "10000", "11110", "10001", "10001", "01110"],
        '7' => vec!["11111", "00001", "00010", "00100", "01000", "01000", "01000"],
        '8' => vec!["01110", "10001", "10001", "01110", "10001", "10001", "01110"],
        '9' => vec!["01110", "10001", "10001", "01111", "00001", "00010", "01100"],
        '+' => vec!["00000", "00100", "00100", "11111", "00100", "00100", "00000"],
        '-' => vec!["00000", "00000", "00000", "11111", "00000", "00000", "00000"],
        '*' => vec!["00000", "10101", "01110", "11111", "01110", "10101", "00000"],
        '/' => vec!["00001", "00010", "00010", "00100", "01000", "01000", "10000"],
        '=' => vec!["00000", "00000", "11111", "00000", "11111", "00000", "00000"],
        '.' => vec!["00000", "00000", "00000", "00000", "00000", "01100", "01100"],
        'A' => vec!["01110", "10001", "10001", "11111", "10001", "10001", "10001"],
        'C' => vec!["01111", "10000", "10000", "10000", "10000", "10000", "01111"],
        'E' => vec!["11111", "10000", "10000", "11110", "10000", "10000", "11111"],
        'H' => vec!["10001", "10001", "10001", "11111", "10001", "10001", "10001"],
        'L' => vec!["10000", "10000", "10000", "10000", "10000", "10000", "11111"],
        'O' => vec!["01110", "10001", "10001", "10001", "10001", "10001", "01110"],
        'P' => vec!["11110", "10001", "10001", "11110", "10000", "10000", "10000"],
        'R' => vec!["11110", "10001", "10001", "11110", "10100", "10010", "10001"],
        'T' => vec!["11111", "00100", "00100", "00100", "00100", "00100", "00100"],
        _ => return None,
    })
}

fn draw_text(fb: &mut Region, s: &str, mut dx: u32, dy: u32, rgb: &[u8; 3]) -> Result<(), String> {
    for ch in s.chars() {
        if let Some(rows) = font(ch) {
            for (ry, row) in rows.iter().enumerate() {
                let Ok(bits) = u8::from_str_radix(row, 2) else { continue };
                for rx in 0..5usize {
                    if (bits >> (4 - rx)) & 1 == 1 {
                        fb.put(dx + rx as u32 * 4, dy + ry as u32 * 4, 4, 4, rgb)?;
                    }
                }
            }
        }
        dx += 24;
    }
    Ok(())
}

const KEYPAD: &[&str] = &["789/", "456*", "123-", "0.=+"];

fn draw_panel(fb: &mut Region, display: &str) -> Result<(), String> {
    fb.fill()?;
    // Accent header strip.
    fb.put(0, 0, fb.rect.width, 6, &ACCENT)?;
    // Display area (right-aligned digits).
    fb.put(12, 24, fb.rect.width - 24, 56, &DISPLAY)?;
    let shown: String = display.chars().rev().take(9).collect();
    draw_text(fb, &shown, fb.rect.width - 24 - shown.chars().count() as u32 * 24, 36, &FG)?;

    // Keypad grid.
    let start_y = 110;
    for (r, row) in KEYPAD.iter().enumerate() {
        for (c, ch) in row.chars().enumerate() {
            let bx = 20 + c as u32 * 92;
            let by = start_y + r as u32 * 84;
            fb.put(bx, by, 76, 66, &KEY)?;
            if let Some(g) = font(ch) {
                for (ry, row_bits) in g.iter().enumerate() {
                    let Ok(bits) = u8::from_str_radix(row_bits, 2) else { continue };
                    for rx in 0..5usize {
                        if (bits >> (4 - rx)) & 1 == 1 {
                            fb.put(bx + 28 + rx as u32 * 4, by + 18 + ry as u32 * 4, 4, 4, &FG)?;
                        }
                    }
                }
            }
        }
    }

    // Identity footer: proves this surface belongs to an Aether application.
    draw_text(fb, "AETHER OS APP", 16, fb.rect.height.saturating_sub(28), &GREEN)?;
    Ok(())
}

fn read_stride() -> Result<u32, String> {
    let s = std::fs::read_to_string("/sys/class/graphics/fb0/stride")
        .map_err(|e| format!("stride: {e}"))?;
    s.trim().parse::<u32>().map_err(|_| "bad stride".to_string())
}

use std::sync::{Arc, Mutex};

fn run() -> Result<(), String> {
    eprintln!("[calculator] starting");
    let mut surface = Surface::connect("calculator", "Calculator", 400, 480)?;
    let rect = surface.rect();
    eprintln!(
        "[calculator] window {} at {},{} {}x{}",
        surface.window_id(),
        rect.x,
        rect.y,
        rect.width,
        rect.height
    );

    let mut region = Region::open(rect)?;
    let display_state = Arc::new(Mutex::new(String::from("0")));
    let closed = Arc::new(std::sync::atomic::AtomicBool::new(false));

    // Event thread: mutates the display model.
    {
        let display_state = Arc::clone(&display_state);
        let closed = Arc::clone(&closed);
        std::thread::spawn(move || loop {
            match surface.poll() {
                Some(SurfaceEvent::CloseRequested) | None => {
                    eprintln!("[calculator] close requested; exiting cleanly");
                    closed.store(true, std::sync::atomic::Ordering::SeqCst);
                    break;
                }
                Some(SurfaceEvent::Key(c)) => {
                    let mut d = display_state.lock().unwrap_or_else(|p| p.into_inner());
                    if d.as_str() == "0" {
                        d.clear();
                    }
                    d.push(c);
                }
                Some(SurfaceEvent::Backspace) => {
                    let mut d = display_state.lock().unwrap_or_else(|p| p.into_inner());
                    d.pop();
                    if d.is_empty() {
                        d.push('0');
                    }
                }
                Some(SurfaceEvent::Enter) => {
                    *display_state.lock().unwrap_or_else(|p| p.into_inner()) = "0".to_string();
                }
            }
        });
    }

    // Refresh thread: keeps the panel on screen even after shell repaints.
    {
        let display_state = Arc::clone(&display_state);
        let closed = Arc::clone(&closed);
        std::thread::spawn(move || loop {
            if closed.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            let snapshot = display_state.lock().unwrap_or_else(|p| p.into_inner()).clone();
            if let Err(e) = draw_panel(&mut region, &snapshot) {
                eprintln!("[calculator][FAIL] {e}");
                closed.store(true, std::sync::atomic::Ordering::SeqCst);
                break;
            }
            std::thread::sleep(Duration::from_secs(2));
        });
    }

    // Initial paint happens on the main thread handle, then we simply wait:
    // both worker threads own clones of the underlying state; the file
    // handle lives in `region` moved into the refresh thread above? It is
    // moved below instead - keep main blocked until closed.
    while !closed.load(std::sync::atomic::Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[calculator][FAIL] {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
