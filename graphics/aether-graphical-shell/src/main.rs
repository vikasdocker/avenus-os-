// Aether Graphical Shell - AI-first interaction surface.
//
// Pipeline proven by this phase:
//   graphical UI -> Aether Agent (ndjson TCP) -> AI Provider -> Agent -> UI
//
// Rendering stays dependency-light: kernel framebuffer (/dev/fb0) with
// geometry from sysfs, full-frame seek+write (no unsafe), embedded 5x7 font.
// Text input arrives over the serial console line; the agent and control
// plane are reached over loopback TCP. The AI never executes anything —
// it only produces text for display in this phase.

use serde_json::{json, Value};
use std::fs::{File, OpenOptions};
use std::io::{Seek, SeekFrom, Write};
use std::process::{Command, ExitCode};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------- palette

const BG: Rgb = Rgb(14, 17, 22);
const PANEL: Rgb = Rgb(24, 29, 38);
const CYAN: Rgb = Rgb(34, 211, 238);
const FG: Rgb = Rgb(230, 237, 243);
const DIM: Rgb = Rgb(122, 132, 142);
const GREEN: Rgb = Rgb(74, 222, 128);
const RED: Rgb = Rgb(248, 113, 113);

#[derive(Clone, Copy)]
struct Rgb(u8, u8, u8);

fn pixel(c: Rgb) -> [u8; 4] {
    [c.2, c.1, c.0, 0xFF]
}

const BRAND: &str = "AETHER OS";
const TAGLINE: &str = "AI-NATIVE OPERATING SYSTEM";
const PROMPT_HINT: &str = "TYPE ON THE SERIAL CONSOLE AND PRESS ENTER";

const AGENT_PORT: u16 = 4748;
const CONTROL_PORT: u16 = 4747;

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

/// Embedded 5x7 font subset (uppercase + digits + punctuation used here).
fn glyph_rows(ch: char) -> Option<&'static [&'static str]> {
    Some(match ch {
        ' ' => &["00000"; 7],
        '-' => &["00000", "00000", "00000", "11111", "00000", "00000", "00000"],
        '.' => &["00000", "00000", "00000", "00000", "00000", "01100", "01100"],
        ':' => &["00000", "01100", "01100", "00000", "01100", "01100", "00000"],
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

/// Wraps `text` into lines of at most `cols` characters.
fn wrap(text: &str, cols: usize) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.split('\n') {
        let mut current = String::new();
        for word in para.split_whitespace() {
            let word = word.to_uppercase();
            if !current.is_empty() && current.chars().count() + 1 + word.chars().count() > cols {
                out.push(std::mem::take(&mut current));
            }
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(&word);
        }
        out.push(current);
    }
    out
}

// ------------------------------------------------------------- ui state

#[derive(Clone)]
enum ChatEntry {
    User(String),
    Ai(String),
    System(String),
}

impl ChatEntry {
    fn prefix_color(&self) -> (&'static str, Rgb) {
        match self {
            Self::User(_) => ("YOU>", FG),
            Self::Ai(_) => ("AI >", CYAN),
            Self::System(_) => ("SYS>", DIM),
        }
    }

    fn text(&self) -> &str {
        match self {
            Self::User(t) | Self::Ai(t) | Self::System(t) => t,
        }
    }
}

struct UiState {
    chat: Vec<ChatEntry>,
    input: String,
    status: Vec<(String, bool)>,
    busy: bool,
}

impl UiState {
    fn new() -> Self {
        Self {
            chat: vec![
                ChatEntry::System("AETHER AGENT ONLINE".to_string()),
                ChatEntry::Ai("HELLO. I AM THE AETHER INTERFACE. SPEAK, AND I LISTEN.".to_string()),
            ],
            input: String::new(),
            status: vec![
                ("OS".to_string(), true),
                ("AGENT".to_string(), false),
                ("CTRL".to_string(), false),
                ("APPS".to_string(), false),
            ],
            busy: false,
        }
    }

    fn push(&mut self, entry: ChatEntry) {
        self.chat.push(entry);
        let max_lines = 18;
        while self.chat.len() > max_lines {
            self.chat.remove(0);
        }
    }
}

// ----------------------------------------------------------- agent client

/// One ndjson request/response exchange with a loopback Aether service.
fn ndjson_call(port: u16, request: &Value) -> Result<Value, String> {
    use std::io::{BufRead, BufReader, Write};
    let mut stream = std::net::TcpStream::connect(("127.0.0.1", port))
        .map_err(|e| format!("connect :{port}: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("timeout: {e}"))?;
    stream
        .write_all(format!("{request}\n").as_bytes())
        .map_err(|e| format!("send: {e}"))?;
    let mut line = String::new();
    BufReader::new(stream)
        .read_line(&mut line)
        .map_err(|e| format!("recv: {e}"))?;
    serde_json::from_str(line.trim()).map_err(|e| format!("decode: {e}"))
}

fn agent_chat(prompt: &str) -> Result<(String, String), String> {
    let reply = ndjson_call(
        AGENT_PORT,
        &json!({ "command": "chat", "argument": prompt }),
    )?;
    let ok = reply.get("ok").and_then(Value::as_bool).unwrap_or(false);
    if !ok {
        return Err(reply["result"]["error"]
            .as_str()
            .unwrap_or("agent refused")
            .to_string());
    }
    Ok((
        reply["result"]["response"]
            .as_str()
            .unwrap_or_default()
            .to_string(),
        reply["result"]["provider"].as_str().unwrap_or("?").to_string(),
    ))
}

/// Refreshes service indicators from the agent and the control plane.
fn refresh_status(status: &mut [(String, bool)]) {
    // Agent reachable?
    let agent_up = ndjson_call(AGENT_PORT, &json!({ "command": "status" }))
        .map(|v| v.get("ok").and_then(Value::as_bool).unwrap_or(false))
        .unwrap_or(false);
    for entry in status.iter_mut() {
        if entry.0 == "AGENT" {
            entry.1 = agent_up;
        }
    }
    // Control plane overall health + per-service states.
    if let Ok(v) = ndjson_call(
        CONTROL_PORT,
        &json!({
            "service_id": "aether-system-core",
            "command": "status",
            "parameters": {},
        }),
    ) {
        let healthy = v["result"]["overall_health"].as_str() == Some("Healthy");
        for entry in status.iter_mut() {
            match entry.0.as_str() {
                "CTRL" => entry.1 = healthy,
                "APPS" => {
                    entry.1 = v["result"]["services"]
                        .as_array()
                        .map(|svcs| {
                            svcs.iter().any(|s| {
                                s["service_id"] == "aether-application-manager"
                                    && s["status"] == "Running"
                            })
                        })
                        .unwrap_or(false);
                }
                _ => {}
            }
        }
    }
}

// ------------------------------------------------------------- rendering

const CONV_SCALE: u32 = 2;
const ROW_H: i64 = 20;

fn draw(fb: &mut Screen, ui: &UiState, cursor_on: bool) {
    fb.fill(BG);
    fb.rect(0, 0, fb.width, 3, CYAN);

    // Header: brand left, centered tagline, status pills right.
    fb.text(BRAND, 24, 20, 3, FG);
    fb.text(TAGLINE, fb.centered_x(TAGLINE, 2), 52, 2, DIM);
    let pill_s = 2u32;
    let mut px = fb.width as i64 - 24;
    for (name, up) in ui.status.iter().rev() {
        let label = format!("{name}:{}", if *up { "UP" } else { "DOWN" });
        let w = label.len() as u32 * 6 * pill_s + 12;
        px -= w as i64;
        let color = if *up { GREEN } else { RED };
        fb.text(&label, px + 6, 26, pill_s, color);
        px -= 14;
    }
    fb.rect(0, 84, fb.width, 2, PANEL);

    // Conversation area.
    let cols = ((fb.width - 48) / (6 * CONV_SCALE)) as usize;
    let mut row_y = 104i64;
    for entry in &ui.chat {
        let (prefix, color) = entry.prefix_color();
        fb.text(prefix, 24, row_y, CONV_SCALE, color);
        for line in wrap(entry.text(), cols.saturating_sub(5)) {
            fb.text(&line, 24 + (6 * CONV_SCALE) as i64, row_y, CONV_SCALE, color);
            row_y += ROW_H;
        }
        row_y += 6;
    }

    // Input area.
    let input_y = fb.height as i64 - 56;
    fb.rect(0, input_y - 14, fb.width, 70, PANEL);
    fb.text(">", 24, input_y, 3, GREEN);
    let shown: String = ui.input.to_uppercase();
    fb.text(&shown, 48, input_y, 3, FG);
    if cursor_on {
        let cx = 48 + (shown.chars().count() as i64) * 6 * 3;
        fb.rect(cx, input_y, 6 * 3, 7 * 3, GREEN);
    }
    if ui.busy {
        fb.text("THINKING...", fb.width as i64 - 24 - (11 * 6 * 2) as i64, input_y + 34, 2, DIM);
    } else {
        fb.text(PROMPT_HINT, fb.width as i64 - 24 - (PROMPT_HINT.len() as u32 * 6 * 2) as i64, input_y + 34, 2, DIM);
    }
}

// ------------------------------------------------------------- event loop

enum UiEvent {
    Char(char),
    Submit,
    StatusTick,
    Reply(Result<(String, String), String>),
}

fn spawn_input_thread(tx: Sender<UiEvent>) {
    std::thread::spawn(move || {
        // Serial console is the input device for this phase. Switch the
        // port to raw mode first so bytes are delivered per-read instead
        // of sitting in the canonical line buffer.
        let stty = Command::new("/bin/stty")
            .args(["-F", "/dev/ttyS0", "raw", "-echo", "-icanon", "min", "1", "time", "0"])
            .status();
        match stty {
            Ok(s) if s.success() => eprintln!("[gfx] ttyS0 set to raw mode"),
            other => eprintln!("[gfx][WARN] stty failed: {other:?}"),
        }
        let Ok(mut serial_file) = OpenOptions::new().read(true).open("/dev/ttyS0") else {
            eprintln!("[gfx][WARN] cannot open /dev/ttyS0 for input");
            return;
        };
        eprintln!("[gfx] serial input armed on /dev/ttyS0");
        let mut byte = [0u8; 1];
        use std::io::Read;
        loop {
            match serial_file.read(&mut byte) {
                Ok(0) => std::thread::sleep(Duration::from_millis(50)),
                Ok(_) => match byte[0] {
                    b'\r' | b'\n' => {
                        let _ = tx.send(UiEvent::Submit);
                    }
                    0x7f | 0x08 => {
                        let _ = tx.send(UiEvent::Char('\u{8}'));
                    }
                    b if b.is_ascii_graphic() || b == b' ' => {
                        let _ = tx.send(UiEvent::Char(b as char));
                    }
                    _ => {}
                }
                Err(e) => {
                    eprintln!("[gfx][WARN] input read error: {e}");
                    std::thread::sleep(Duration::from_millis(100));
                }
            }
        }
    });
}

fn spawn_status_thread(tx: Sender<UiEvent>) {
    std::thread::spawn(move || loop {
        let _ = tx.send(UiEvent::StatusTick);
        std::thread::sleep(Duration::from_secs(5));
    });
}

fn submit(ui_tx: &Sender<UiEvent>, ui_arc: &Arc<Mutex<UiState>>, prompt: String) {
    {
        let mut ui = lock_ui(ui_arc);
        ui.busy = true;
        ui.push(ChatEntry::User(prompt.clone()));
    }
    let tx = ui_tx.clone();
    std::thread::spawn(move || {
        let result = agent_chat(&prompt);
        let _ = tx.send(UiEvent::Reply(result));
    });
}

fn lock_ui(arc: &Arc<Mutex<UiState>>) -> std::sync::MutexGuard<'_, UiState> {
    arc.lock().unwrap_or_else(|p| p.into_inner())
}

fn run() -> Result<(), String> {
    let (file, width, height, stride) = open_framebuffer()?;
    eprintln!("[gfx] fb0 {width}x{height} stride {stride} ok");
    let mut fb = Screen::new(file, width, height, stride);

    let ui_arc = Arc::new(Mutex::new(UiState::new()));
    let (tx, rx): (Sender<UiEvent>, Receiver<UiEvent>) = channel();
    if std::env::var("AETHER_GFX_INPUT").as_deref() == Ok("1") {
        // Exclusive serial ownership (aether=single): raw mode + read loop.
        spawn_input_thread(tx.clone());
    } else {
        eprintln!("[gfx] input disabled; console belongs to the interactive shell");
    }
    spawn_status_thread(tx.clone());

    eprintln!("[gfx] ai interface ready; waiting for input");
    let mut last_blink = Instant::now();
    let mut cursor_on = true;
    loop {
        // Redraw on blink toggle or any event.
        let timeout = if last_blink.elapsed() >= Duration::from_millis(600) {
            cursor_on = !cursor_on;
            last_blink = Instant::now();
            true
        } else {
            false
        };

        match rx.recv_timeout(Duration::from_millis(150)) {
            Ok(UiEvent::Char(c)) => {
                let mut ui = lock_ui(&ui_arc);
                if c == '\u{8}' {
                    ui.input.pop();
                } else if ui.input.chars().count() < 60 {
                    ui.input.push(c);
                }
                draw(&mut fb, &ui, cursor_on);
                fb.flush()?;
            }
            Ok(UiEvent::Submit) => {
                let prompt = {
                    let mut ui = lock_ui(&ui_arc);
                    let prompt = ui.input.trim().to_string();
                    ui.input.clear();
                    prompt
                };
                if !prompt.is_empty() {
                    submit(&tx, &ui_arc, prompt);
                }
                let ui = lock_ui(&ui_arc);
                draw(&mut fb, &ui, cursor_on);
                fb.flush()?;
            }
            Ok(UiEvent::StatusTick) => {
                let mut ui = lock_ui(&ui_arc);
                refresh_status(&mut ui.status);
                draw(&mut fb, &ui, cursor_on);
                fb.flush()?;
            }
            Ok(UiEvent::Reply(result)) => {
                let mut ui = lock_ui(&ui_arc);
                ui.busy = false;
                match result {
                    Ok((reply, provider)) => {
                        eprintln!("[gfx] agent replied via {provider}");
                        ui.push(ChatEntry::Ai(reply));
                        ui.push(ChatEntry::System(format!("VIA {provider}")));
                    }
                    Err(e) => {
                        eprintln!("[gfx][FAIL] agent error: {e}");
                        ui.push(ChatEntry::System(format!("AGENT ERROR: {e}")));
                    }
                }
                draw(&mut fb, &ui, cursor_on);
                fb.flush()?;
            }
            Err(_timeout) => {
                if timeout {
                    let ui = lock_ui(&ui_arc);
                    draw(&mut fb, &ui, cursor_on);
                    fb.flush()?;
                }
            }
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
        let sample = "AETHER OS 0123456789>:.-AI-NATIVE OPERATING SYSTEM SERVICES UP GPU READY CTRL AGENT DOWN THINKING YOU TYPE THE SERIAL CONSOLE AND PRESS ENTER";
        for ch in sample.chars().filter(|c| *c != ' ') {
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
    fn wrap_breaks_long_text_and_uppercases() {
        let lines = wrap("hello brave new aether world of ours today ok then", 12);
        assert!(lines.iter().all(|l| l.chars().count() <= 12));
        assert_eq!(lines[0], "HELLO BRAVE");
        assert!(lines.iter().all(|l| !l.ends_with(' ')));
    }

    #[test]
    fn rect_clips_out_of_bounds() {
        let f = File::create(std::env::temp_dir().join("aether-gfx-test.bin"))
            .unwrap_or_else(|e| panic!("{e}"));
        let mut scr = Screen::new(f, 8, 8, 32);
        scr.fill(BG);
        scr.rect(-4, -4, 16, 16, CYAN); // must not panic
        assert!(scr.buf.chunks_exact(4).all(|p| p == pixel(CYAN)));
    }
}
