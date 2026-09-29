//! Font Awesome icons, and the tab colours they can be paired with.

use gpui::{Font, FontWeight, SharedString};

/// The Font Awesome build bundled with the app.
const FONT: &[u8] = include_bytes!("../assets/fontawesome-solid.ttf");

/// Registers the bundled icon font. Call once, before the first window.
pub fn install(cx: &mut gpui::App) {
    let _ = cx
        .text_system()
        .add_fonts(vec![std::borrow::Cow::Borrowed(FONT)]);
}

/// The Font Awesome face, at the weight the Solid styles use.
pub fn font() -> Font {
    Font {
        weight: FontWeight(900.),
        ..gpui::font("Font Awesome 6 Free")
    }
}

macro_rules! icons {
    ($($name:ident => $glyph:expr;)*) => {
        /// One of the icons offered in the tab picker.
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub enum Icon {
            $($name,)*
        }

        impl Icon {
            /// Every icon, in the order the picker shows them.
            pub const ALL: &'static [Icon] = &[$(Icon::$name,)*];

            pub fn glyph(self) -> char {
                match self {
                    $(Icon::$name => $glyph,)*
                }
            }
        }
    };
}

icons! {
    Terminal => '\u{f120}';
    Circle   => '\u{f111}';
    Square   => '\u{f0c8}';
    Bolt     => '\u{f0e7}';
    Cog      => '\u{f013}';
    Wrench   => '\u{f0ad}';
    Hammer   => '\u{f6e3}';
    Code     => '\u{f121}';
    Keyboard => '\u{f11c}';
    Display  => '\u{f108}';
    Cpu      => '\u{f2db}';
    Server   => '\u{f233}';
    Network  => '\u{f6ff}';
    Satellite=> '\u{f7bf}';
    Gauge    => '\u{f624}';
    Chart    => '\u{f200}';
    Database => '\u{f1c0}';
    Cube     => '\u{f1b2}';
    Rocket   => '\u{f135}';
    Ghost    => '\u{f1b1}';
    Skull    => '\u{f54c}';
    Robot    => '\u{f544}';
    Bug      => '\u{f188}';
    Flask    => '\u{f0c3}';
    Anchor   => '\u{f13d}';
    Key      => '\u{f084}';
    Lock     => '\u{f023}';
    Shield   => '\u{f132}';
    Search   => '\u{f002}';
    Clock    => '\u{f017}';
    Play     => '\u{f04b}';
    Home     => '\u{f015}';
    Folder   => '\u{f07b}';
    Book     => '\u{f02d}';
    Star     => '\u{f005}';
    Heart    => '\u{f004}';
    Fire     => '\u{f06d}';
    Cloud    => '\u{f0c2}';
    Sun      => '\u{f185}';
    Moon     => '\u{f186}';
    Window   => '\u{f2dc}';
    Plug     => '\u{f1e6}';
}

/// A colour a tab background can take, as `#rrggbb`.
pub struct Swatch {
    pub value: &'static str,
}

pub const SWATCHES: &[Swatch] = &[
    Swatch { value: "#000000" },
    Swatch { value: "#0b0b0b" },
    Swatch { value: "#171717" },
    Swatch { value: "#1f1f1f" },
    Swatch { value: "#262626" },
    Swatch { value: "#404040" },
    Swatch { value: "#5c5c5c" },
    Swatch { value: "#1a2740" },
    Swatch { value: "#0f2a2a" },
    Swatch { value: "#231a3a" },
    Swatch { value: "#2d1a26" },
    Swatch { value: "#3a1f14" },
    Swatch { value: "#10240f" },
];

/// Renders an icon as a string, ready to be shaped as a text run.
pub fn glyph(icon: Icon) -> SharedString {
    SharedString::from(icon.glyph().to_string())
}
