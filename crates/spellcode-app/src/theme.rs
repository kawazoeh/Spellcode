//! Colour handling.
//!
//! The application chrome is strictly black and white; the terminal keeps a
//! real 256 colour palette so that full-screen programs still look right.

use gpui::{Hsla, Rgba, black, rgb};

/// Indices past the 256 xterm entries, as used by the emulator.
pub const FOREGROUND: usize = 256;
pub const BACKGROUND: usize = 257;
pub const CURSOR: usize = 258;
const DIM_NAMED_FIRST: usize = 259;
const DIM_NAMED_LAST: usize = 265;
const BRIGHT_FOREGROUND: usize = 266;
pub const DIM_FOREGROUND: usize = 267;
pub const DIM_BACKGROUND: usize = 268;

/// Greyscale shades used by the chrome.
pub const BLACK: u32 = 0x000000;
pub const NEAR_BLACK: u32 = 0x0b0b0b;
pub const HOVER: u32 = 0x1f1f1f;
pub const HAIRLINE: u32 = 0x262626;
pub const FAINT: u32 = 0x6b6b6b;
pub const DIM: u32 = 0xa3a3a3;
pub const WHITE: u32 = 0xffffff;

/// A palette in the classic xterm layout: 16 named colours, a 6x6x6 colour
/// cube, and a 24 step grayscale ramp.
#[derive(Clone)]
pub struct Palette {
    entries: [Rgba; 256],
    pub foreground: Rgba,
    pub background: Rgba,
    pub cursor: Rgba,
    pub cursor_text: Rgba,
}

impl Palette {
    pub fn dark() -> Self {
        let mut entries: [Rgba; 256] = [black().into(); 256];

        // Base 16: readable on black, close to the macOS Terminal defaults.
        const BASE_16: [u32; 16] = [
            0x3b3b3b, 0xe06c75, 0x98c379, 0xe5c07b, 0x61afef, 0xc678dd, 0x56b6c2, 0xababab,
            0x5c5c5c, 0xe06c75, 0x98c379, 0xe5c07b, 0x61afef, 0xc678dd, 0x56b6c2, 0xffffff,
        ];
        for (index, value) in BASE_16.iter().enumerate() {
            entries[index] = rgb(*value);
        }

        // Colour cube: 16 + 36r + 6g + b.
        const STEPS: [f32; 6] = [0., 95., 135., 175., 215., 255.];
        for (r, &red) in STEPS.iter().enumerate() {
            for (g, &green) in STEPS.iter().enumerate() {
                for (b, &blue) in STEPS.iter().enumerate() {
                    entries[16 + 36 * r + 6 * g + b] = Rgba {
                        r: red / 255.,
                        g: green / 255.,
                        b: blue / 255.,
                        a: 1.,
                    };
                }
            }
        }

        // Grayscale ramp: 232 + i.
        for i in 0..24 {
            let value = 8. + i as f32 * 10.;
            entries[232 + i] = Rgba {
                r: value / 255.,
                g: value / 255.,
                b: value / 255.,
                a: 1.,
            };
        }

        Self {
            entries,
            foreground: rgb(0xd8d8d8),
            background: rgb(BLACK),
            cursor: rgb(0xf2f2f2),
            cursor_text: rgb(BLACK),
        }
    }

    pub fn get(&self, index: usize) -> Rgba {
        self.entries[index.min(255)]
    }

    /// Resolves a terminal colour index.
    ///
    /// The emulator's index space is wider than the 256 xterm entries: 256 is
    /// the default foreground, 257 the default background, 258 the cursor, and
    /// 259..=268 the dim variants.
    pub fn color_at(&self, index: usize) -> Rgba {
        match index {
            0..=255 => self.entries[index],
            FOREGROUND => self.foreground,
            BACKGROUND => self.background,
            CURSOR => self.cursor,
            BRIGHT_FOREGROUND => self.entries[8],
            DIM_FOREGROUND => mix(self.background, self.foreground, 0.6),
            DIM_BACKGROUND => mix(self.background, self.foreground, 0.25),
            DIM_NAMED_FIRST..=DIM_NAMED_LAST => {
                mix(self.background, self.entries[index - DIM_NAMED_FIRST], 0.55)
            }
            _ => self.foreground,
        }
    }
}

/// Application chrome colours. Black and white only.
#[derive(Clone, Copy)]
pub struct Theme {
    pub background: Hsla,
    pub surface: Hsla,
    pub border: Hsla,
    pub text: Hsla,
    pub text_dim: Hsla,
    pub text_faint: Hsla,
    pub accent_soft: Hsla,
}

impl Theme {
    pub fn monochrome() -> Self {
        Self {
            background: shade(BLACK),
            surface: shade(NEAR_BLACK),
            border: shade(HAIRLINE),
            text: shade(WHITE),
            text_dim: shade(DIM),
            text_faint: shade(FAINT),
            accent_soft: shade(HOVER),
        }
    }
}

fn shade(value: u32) -> Hsla {
    rgb(value).into()
}

/// Linear blend between two colours; `t` is the weight of `to`.
pub fn mix(from: Rgba, to: Rgba, t: f32) -> Rgba {
    let t = t.clamp(0., 1.);
    Rgba {
        r: from.r + (to.r - from.r) * t,
        g: from.g + (to.g - from.g) * t,
        b: from.b + (to.b - from.b) * t,
        a: from.a + (to.a - from.a) * t,
    }
}

/// Applies an alpha to a colour without touching its hue.
pub fn fade(color: Hsla, alpha: f32) -> Hsla {
    Hsla {
        a: color.a * alpha,
        ..color
    }
}

/// Converts a `#rrggbb` string into a colour.
pub fn parse_hex(input: &str) -> Option<Rgba> {
    let hex = input.trim().strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    Some(rgb(u32::from_str_radix(hex, 16).ok()?))
}
