//! Turns GPUI keystrokes into the byte sequences a terminal program expects.

use gpui::Keystroke;

/// Encodes a keystroke for the PTY.
///
/// Returns `None` when the key is not ours to handle (application shortcuts
/// such as `cmd`-chorded keys, or keys we do not know about).
pub fn encode(keystroke: &Keystroke, application_cursor: bool) -> Option<Vec<u8>> {
    let modifiers = keystroke.modifiers;
    if modifiers.platform || modifiers.control && modifiers.alt && modifiers.shift {
        return None;
    }

    let mut bytes = match keystroke.key.as_str() {
        "enter" => vec![b'\r'],
        "tab" => vec![b'\t'],
        "backspace" => vec![0x7f],
        "escape" => vec![0x1b],
        "space" => vec![b' '],
        "up" => cursor_key(b'A', application_cursor),
        "down" => cursor_key(b'B', application_cursor),
        "right" => cursor_key(b'C', application_cursor),
        "left" => cursor_key(b'D', application_cursor),
        "home" => home_end_key(b'H', application_cursor),
        "end" => home_end_key(b'F', application_cursor),
        "pageup" => vec![0x1b, b'[', b'5', b'~'],
        "pagedown" => vec![0x1b, b'[', b'6', b'~'],
        "insert" => vec![0x1b, b'[', b'2', b'~'],
        "delete" => vec![0x1b, b'[', b'3', b'~'],
        "f1" => vec![0x1b, b'O', b'P'],
        "f2" => vec![0x1b, b'O', b'Q'],
        "f3" => vec![0x1b, b'O', b'R'],
        "f4" => vec![0x1b, b'O', b'S'],
        "f5" => vec![0x1b, b'[', b'1', b'5', b'~'],
        "f6" => vec![0x1b, b'[', b'1', b'7', b'~'],
        "f7" => vec![0x1b, b'[', b'1', b'8', b'~'],
        "f8" => vec![0x1b, b'[', b'1', b'9', b'~'],
        "f9" => vec![0x1b, b'[', b'2', b'0', b'~'],
        "f10" => vec![0x1b, b'[', b'2', b'1', b'~'],
        "f11" => vec![0x1b, b'[', b'2', b'3', b'~'],
        "f12" => vec![0x1b, b'[', b'2', b'4', b'~'],
        _ => return encode_text(keystroke),
    };

    if modifiers.control && bytes.len() == 1 {
        bytes = vec![control_byte(bytes[0])];
    }

    if modifiers.alt {
        bytes.insert(0, 0x1b);
    }

    Some(bytes)
}

/// Printable characters, plus control chords such as `ctrl-c`.
fn encode_text(keystroke: &Keystroke) -> Option<Vec<u8>> {
    let text = keystroke.key_char.as_ref().or(Some(&keystroke.key))?;
    if text.is_empty() {
        return None;
    }

    if keystroke.modifiers.control {
        let mut bytes = Vec::with_capacity(text.len());
        for character in text.chars() {
            let mut buf = [0u8; 4];
            let encoded = character.encode_utf8(&mut buf).as_bytes();
            bytes.push(control_byte(encoded[0]));
        }
        return Some(bytes);
    }

    Some(text.as_bytes().to_vec())
}

fn cursor_key(final_byte: u8, application_cursor: bool) -> Vec<u8> {
    if application_cursor {
        vec![0x1b, b'O', final_byte]
    } else {
        vec![0x1b, b'[', final_byte]
    }
}

fn home_end_key(final_byte: u8, application_cursor: bool) -> Vec<u8> {
    if application_cursor {
        vec![0x1b, b'O', final_byte]
    } else {
        vec![0x1b, b'[', final_byte]
    }
}

/// Maps a byte to its C0 control equivalent (`a` -> 0x01, `@` -> 0x00, ...).
fn control_byte(byte: u8) -> u8 {
    const SUBSTITUTIONS: &[u8] = b"@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_";
    match SUBSTITUTIONS
        .iter()
        .position(|candidate| *candidate == byte.to_ascii_uppercase())
    {
        Some(index) => index as u8,
        None if byte == b' ' => 0,
        None => byte,
    }
}

/// Wraps pasted text according to the terminal's bracketed paste mode.
pub fn encode_paste(text: &str, bracketed: bool) -> Vec<u8> {
    if !bracketed {
        return text.as_bytes().to_vec();
    }

    let mut bytes = Vec::with_capacity(text.len() + 12);
    bytes.extend_from_slice(b"\x1b[200~");
    bytes.extend_from_slice(text.as_bytes());
    bytes.extend_from_slice(b"\x1b[201~");
    bytes
}
