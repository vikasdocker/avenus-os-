// Aether Notes - minimal text scratchpad application.
//
// Second test app for the multi-window desktop: opens a second window,
// displays typed text (forwarded by the shell when focused), and closes
// cleanly on request.

use aether_surface::{Surface, SurfaceEvent};
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const BG: [u8; 3] = [18, 22, 28];
const ACCENT: [u8; 3] = [34, 211, 238];
const FG: [u8; 3] = [230, 237, 243];
const DIM: [u8; 3] = [122, 132, 142];

struct Region {
    file: File,
    stride: u32,
    rect: aether_surface::Rect,
}

impl Region {
    fn open(rect: aether_surface::Rect) -> Result<Self, String> {
        let file =
            OpenOptions::new().write(true).open("/dev/fb0").map_err(|e| format!("fb0: {e}"))?;
        let s = std::fs::read_to_string("/sys/class/graphics/fb0/stride")
            .map_err(|e| format!("stride: {e}"))?;
        Ok(Self {
            file,
            stride: s.trim().parse::<u32>().map_err(|_| "bad stride".to_string())?,
            rect,
        })
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

fn font(ch: char) -> Option<Vec<&'static str>> {
    Some(match ch.to_ascii_uppercase() {
        'A' => vec!["01110", "10001", "10001", "11111", "10001", "10001", "10001"],
        'C' => vec!["01111", "10000", "10000", "10000", "10000", "10000", "01111"],
        'D' => vec!["11110", "10001", "10001", "10001", "10001", "10001", "11110"],
        'E' => vec!["11111", "10000", "10000", "11110", "10000", "10000", "11111"],
        'H' => vec!["10001", "10001", "10001", "11111", "10001", "10001", "10001"],
        'I' => vec!["11111", "00100", "00100", "00100", "00100", "00100", "11111"],
        'L' => vec!["10000", "10000", "10000", "10000", "10000", "10000", "11111"],
        'M' => vec!["10001", "11011", "10101", "10101", "10001", "10001", "10001"],
        'N' => vec!["10001", "11001", "10101", "10011", "10001", "10001", "10001"],
        'O' => vec!["01110", "10001", "10001", "10001", "10001", "10001", "01110"],
        'P' => vec!["11110", "10001", "10001", "11110", "10000", "10000", "10000"],
        'R' => vec!["11110", "10001", "10001", "11110", "10100", "10010", "10001"],
        'S' => vec!["01111", "10000", "10000", "01110", "00001", "00001", "11110"],
        'T' => vec!["11111", "00100", "00100", "00100", "00100", "00100", "00100"],
        'U' => vec!["10001", "10001", "10001", "10001", "10001", "10001", "01110"],
        _ => return None,
    })
}

fn draw_text(fb: &mut Region, s: &str, dx: u32, dy: u32, rgb: &[u8; 3]) -> Result<(), String> {
    let mut cx = dx;
    for ch in s.chars() {
        if let Some(rows) = font(ch) {
            for (ry, row) in rows.iter().enumerate() {
                let Ok(bits) = u8::from_str_radix(row, 2) else { continue };
                for rx in 0..5usize {
                    if (bits >> (4 - rx)) & 1 == 1 {
                        fb.put(cx + rx as u32 * 4, dy + ry as u32 * 4, 4, 4, rgb)?;
                    }
                }
            }
        }
        cx += 24;
    }
    Ok(())
}

fn draw(fb: &mut Region, lines: &[String]) -> Result<(), String> {
    fb.fill()?;
    fb.put(0, 0, fb.rect.width, 6, &ACCENT)?;
    draw_text(fb, "AETHER NOTES", 16, 20, &ACCENT)?;

    // Text content, newest lines visible.
    let visible = (fb.rect.height.saturating_sub(90)) / 24;
    let start = lines.len().saturating_sub(visible as usize);
    let mut y = 60;
    for line in &lines[start..] {
        draw_text(fb, line, 16, y, &FG)?;
        y += 24;
    }

    // Caret line at the bottom.
    draw_text(fb, "_", 16, fb.rect.height.saturating_sub(30), &DIM)?;
    Ok(())
}

fn nlog(msg: &str) {
    let _ = std::fs::write("/tmp/notes.log", format!("{msg}\n"));
}

fn run() -> Result<(), String> {
    eprintln!("[notes] starting");
    let mut surface = Surface::connect("notes", "Notes", 420, 360)?;
    let rect = surface.rect();
    eprintln!(
        "[notes] window {} at {},{} {}x{}",
        surface.window_id(),
        rect.x,
        rect.y,
        rect.width,
        rect.height
    );

    nlog("registered ok");
    let mut region = Region::open(rect)?;
    nlog("region opened");
    let lines_state = Arc::new(Mutex::new(vec![String::new()]));
    let closed = Arc::new(std::sync::atomic::AtomicBool::new(false));
    draw(&mut region, &lines_state.lock().unwrap_or_else(|p| p.into_inner()))?;
    eprintln!("[notes] notes window visible");

    // Event thread: mutates the text model.
    {
        let lines_state = Arc::clone(&lines_state);
        let closed = Arc::clone(&closed);
        std::thread::spawn(move || loop {
            match surface.poll() {
                Some(SurfaceEvent::CloseRequested) | None => {
                    eprintln!("[notes] close requested; exiting cleanly");
                    closed.store(true, std::sync::atomic::Ordering::SeqCst);
                    break;
                }
                Some(SurfaceEvent::Key(c)) => {
                    let mut lines = lines_state.lock().unwrap_or_else(|p| p.into_inner());
                    if lines.last().is_some_and(|l| l.chars().count() > 30) {
                        lines.push(String::new());
                    }
                    if let Some(last) = lines.last_mut() {
                        last.push(c);
                    }
                }
                Some(SurfaceEvent::Backspace) => {
                    if let Some(last) =
                        lines_state.lock().unwrap_or_else(|p| p.into_inner()).last_mut()
                    {
                        last.pop();
                    }
                }
                Some(SurfaceEvent::Enter) => {
                    lines_state.lock().unwrap_or_else(|p| p.into_inner()).push(String::new());
                }
            }
        });
    }

    // Refresh thread: keeps content on screen despite shell repaints.
    {
        let lines_state = Arc::clone(&lines_state);
        let closed = Arc::clone(&closed);
        std::thread::spawn(move || loop {
            if closed.load(std::sync::atomic::Ordering::SeqCst) {
                break;
            }
            {
                let lines = lines_state.lock().unwrap_or_else(|p| p.into_inner());
                if let Err(e) = draw(&mut region, &lines) {
                    eprintln!("[notes][FAIL] {e}");
                    break;
                }
            }
            std::thread::sleep(Duration::from_secs(2));
        });
    }

    while !closed.load(std::sync::atomic::Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(())
}

fn main() -> std::process::ExitCode {
    match run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("[notes][FAIL] {e}");
            std::process::ExitCode::FAILURE
        }
    }
}
