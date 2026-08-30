// Framebuffer abstraction + embedded font (shared by shell modules).

use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};

#[derive(Clone, Copy)]
pub struct Rgb(pub u8, pub u8, pub u8);

pub fn pixel(c: Rgb) -> [u8; 4] {
    [c.2, c.1, c.0, 0xFF]
}

pub struct Screen {
    pub file: File,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub buf: Vec<u8>,
}

impl Screen {
    /// Opens /dev/fb0 and reads geometry from sysfs (no ioctls, no unsafe).
    pub fn open() -> Result<Self, String> {
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
        let stride = stride.trim().parse::<u32>().map_err(|_| "bad stride".to_string())?;
        if bpp.trim() != "32" {
            return Err(format!("unsupported bpp {bpp}"));
        }
        Ok(Self::new(file, width, height, stride))
    }

    pub fn new(file: File, width: u32, height: u32, stride: u32) -> Self {
        Self { buf: vec![0; (stride * height) as usize], file, width, height, stride }
    }

    pub fn fill(&mut self, c: Rgb) {
        for chunk in self.buf.chunks_exact_mut(4) {
            chunk.copy_from_slice(&pixel(c));
        }
    }

    pub fn rect(&mut self, x: i64, y: i64, w: u32, h: u32, c: Rgb) {
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

    pub fn glyph(&mut self, ch: char, x: i64, y: i64, s: u32, c: Rgb) {
        if let Some(rows) = glyph_rows(ch) {
            for (ry, row) in rows.iter().enumerate() {
                let Ok(bits) = u8::from_str_radix(row, 2) else { continue };
                for rx in 0..5usize {
                    if (bits >> (4 - rx)) & 1 == 1 {
                        self.rect(x + (rx as u32 * s) as i64, y + (ry as u32 * s) as i64, s, s, c);
                    }
                }
            }
        }
    }

    pub fn text(&mut self, text: &str, x: i64, y: i64, s: u32, c: Rgb) {
        let mut cx = x;
        for ch in text.chars() {
            self.glyph(ch, cx, y, s, c);
            cx += (6 * s) as i64;
        }
    }

    pub fn flush(&mut self) -> Result<(), String> {
        self.file
            .seek(SeekFrom::Start(0))
            .and_then(|_| self.file.write_all(&self.buf))
            .map_err(|e| format!("framebuffer write failed: {e}"))
    }

    pub fn centered_in(&self, x: i64, w: u32, text: &str, s: u32) -> i64 {
        let tw = text.len() as u32 * 6 * s;
        x + ((w.saturating_sub(tw)) / 2) as i64
    }

    pub fn centered_x(&self, text: &str, s: u32) -> i64 {
        self.centered_in(0, self.width, text, s)
    }
}

/// 5x7 uppercase/digit/punctuation font subset shared across surfaces.
pub fn glyph_rows(ch: char) -> Option<&'static [&'static str]> {
    Some(match ch {
        ' ' => &["00000"; 7],
        '.' => &["00000", "00000", "00000", "00000", "00000", "01100", "01100"],
        '/' => &["00001", "00010", "00010", "00100", "01000", "01000", "10000"],
        '*' => &["00000", "10101", "01110", "11111", "01110", "10101", "00000"],
        '+' => &["00000", "00100", "00100", "11111", "00100", "00100", "00000"],
        '-' => &["00000", "00000", "00000", "11111", "00000", "00000", "00000"],
        '=' => &["00000", "00000", "11111", "00000", "11111", "00000", "00000"],
        '>' => &["01000", "00100", "00010", "00001", "00010", "00100", "01000"],
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
        'B' => &["11110", "10001", "10001", "11110", "10001", "10001", "11110"],
        'C' => &["01111", "10000", "10000", "10000", "10000", "10000", "01111"],
        'D' => &["11110", "10001", "10001", "10001", "10001", "10001", "11110"],
        'E' => &["11111", "10000", "10000", "11110", "10000", "10000", "11111"],
        'F' => &["11111", "10000", "10000", "11110", "10000", "10000", "10000"],
        'G' => &["01111", "10000", "10000", "10111", "10001", "10001", "01110"],
        'H' => &["10001", "10001", "10001", "11111", "10001", "10001", "10001"],
        'I' => &["11111", "00100", "00100", "00100", "00100", "00100", "11111"],
        'K' => &["10001", "10010", "10100", "11000", "10100", "10010", "10001"],
        'L' => &["10000", "10000", "10000", "10000", "10000", "10000", "11111"],
        'M' => &["10001", "11011", "10101", "10101", "10001", "10001", "10001"],
        'N' => &["10001", "11001", "10101", "10011", "10001", "10001", "10001"],
        'O' => &["01110", "10001", "10001", "10001", "10001", "10001", "01110"],
        'P' => &["11110", "10001", "10001", "11110", "10000", "10000", "10000"],
        'R' => &["11110", "10001", "10001", "11110", "10100", "10010", "10001"],
        'S' => &["01111", "10000", "10000", "01110", "00001", "00001", "11110"],
        'T' => &["11111", "00100", "00100", "00100", "00100", "00100", "00100"],
        'U' => &["10001", "10001", "10001", "10001", "10001", "10001", "01110"],
        'V' => &["10001", "10001", "10001", "10001", "01010", "01010", "00100"],
        'W' => &["10001", "10001", "10001", "10101", "10101", "11011", "10001"],
        'Y' => &["10001", "10001", "01010", "00100", "00100", "00100", "00100"],
        _ => return None,
    })
}
