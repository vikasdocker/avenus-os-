// evdev input decoding: keyboard + mouse over /dev/input/event*.
//
// input_event on 64-bit: timeval(16B) + u16 type + u16 code + i32 value
// = 24 bytes per record. Parsed with from_le_bytes - no unsafe.

use serde_json::json;

#[derive(Debug, Clone, Copy)]
pub enum RawInput {
    MouseMove(i32, i32),
    MouseDown,
    MouseUp,
    KeyPress(u16),
    Wheel(i32),
}

const EV_KEY: u16 = 0x01;
const EV_REL: u16 = 0x02;
const REL_X: u16 = 0x00;
const REL_Y: u16 = 0x01;
const REL_WHEEL: u16 = 0x08;
const BTN_LEFT: u16 = 0x110;

/// Opens every /dev/input/event* device for reading. Device class is
/// resolved lazily on the first distinguishing event.
pub fn open_devices() -> Vec<(std::fs::File, DeviceKind)> {
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/dev/input") {
        for entry in entries.flatten() {
            if let Some(name) = entry.path().file_name().and_then(|n| n.to_str()) {
                if name.contains("event") {
                    if let Ok(f) = std::fs::File::open(entry.path()) {
                        out.push((f, DeviceKind::Unknown));
                    }
                }
            }
        }
    }
    out
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeviceKind {
    Unknown,
    Keyboard,
    Mouse,
}

/// Decodes one 24-byte record into an optional typed event.
pub fn decode(buf: &[u8], kind: &mut DeviceKind) -> Option<RawInput> {
    if buf.len() < 24 {
        return None;
    }
    let etype = u16::from_le_bytes([buf[16], buf[17]]);
    let code = u16::from_le_bytes([buf[18], buf[19]]);
    let value = i32::from_le_bytes([buf[20], buf[21], buf[22], buf[23]]);

    match etype {
        EV_REL => {
            if *kind == DeviceKind::Unknown {
                *kind = DeviceKind::Mouse;
            }
            match code {
                REL_X => Some(RawInput::MouseMove(value, 0)),
                REL_Y => Some(RawInput::MouseMove(0, value)),
                REL_WHEEL => Some(RawInput::Wheel(value)),
                _ => None,
            }
        }
        EV_KEY => {
            if code == BTN_LEFT {
                *kind = DeviceKind::Mouse;
                return Some(if value == 1 { RawInput::MouseDown } else { RawInput::MouseUp });
            }
            if *kind != DeviceKind::Mouse && value == 1 {
                *kind = DeviceKind::Keyboard;
                return Some(RawInput::KeyPress(code));
            }
            None
        }
        _ => None,
    }
}

/// Converts a kernel keycode to a character (press-only events).
pub fn key_to_char(code: u16) -> Option<char> {
    match code {
        2..=10 => Some((b'1' + (code - 2) as u8) as char),
        11 => Some('0'),
        16..=25 => Some((b'q' + (code - 16) as u8) as char),
        30..=38 => Some((b'a' + (code - 30) as u8) as char),
        44..=50 => Some((b'z' + (code - 44) as u8) as char),
        57 => Some(' '),
        28 => Some('\n'),
        14 => Some('\u{8}'),
        12 => Some('-'),
        13 => Some('='),
        51 => Some(';'),
        52 => Some('\''),
        53 => Some('/'),
        55 => Some('*'),
        _ => None,
    }
}

/// JSON log helper.
pub fn key_json(code: u16) -> serde_json::Value {
    json!({ "key": key_to_char(code).map(|c| c.to_string()).unwrap_or_default(), "code": code })
}
