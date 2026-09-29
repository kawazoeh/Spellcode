<p align="center">
  <img src="assets/spellcode01.png" alt="The Spellcode app icon: a four-pointed star with bracket-like marks above, below and on either side, drawn as faint outlines on a white square." width="96">
</p>

<h1 align="center">Spellcode</h1>

A black-and-white, GPU-rendered tabbed terminal for macOS and Windows, written
in Rust with [GPUI](https://gpui.rs).

[![Build status](https://img.shields.io/github/actions/workflow/status/kawazoeh/Spellcode/ci.yml?branch=main&label=build)](https://github.com/kawazoeh/Spellcode/actions)
[![License: MIT](https://img.shields.io/github/license/kawazoeh/Spellcode)](https://github.com/kawazoeh/Spellcode/blob/main/LICENSE)
[![Latest release](https://img.shields.io/github/v/release/kawazoeh/Spellcode)](https://github.com/kawazoeh/Spellcode/releases)

**English** · [Français](README.fr.md)

## Why it exists

A terminal is idle most of the time. Spellcode only repaints a pane when its PTY
produced output or its cursor blinked, so an open but quiet tab costs nothing
while a full repaint costs about as much as a blinking cursor. The tab bar is
the title bar, so no separate chrome row takes vertical space. Every tab is a
real PTY behind a full 256-colour VT emulator, so full-screen programs behave,
while the app's own chrome stays strictly greyscale. There is no built-in
launcher list: a single `config.toml` declares the entries that appear in the
new-tab menu.

It is aimed at developers who want a small, fast, keyboard-first terminal they
can configure with one TOML file and build from a single Rust workspace.

## Installation

### Prebuilt releases

Every release publishes six artefacts:

| Platform          | Artefact                             | Kind                        |
| ----------------- | ------------------------------------ | --------------------------- |
| macOS, Apple silicon | `Spellcode-macos-arm64.dmg`       | Drag-and-drop disk image    |
| macOS, Apple silicon | `Spellcode-macos-arm64.zip`       | Zipped app bundle           |
| macOS, Intel      | `Spellcode-macos-x86_64.dmg`         | Drag-and-drop disk image    |
| macOS, Intel      | `Spellcode-macos-x86_64.zip`         | Zipped app bundle           |
| Windows, x64      | `Setup-Spellcode-<version>-x64.exe`  | Installer (NSIS)            |
| Windows, x64      | `Spellcode-windows-x86_64.zip`       | Portable build              |

On macOS, prefer the `.dmg`: open it and drag `Spellcode.app` onto the
`Applications` shortcut. The `.zip` is the same bundle without the disk image,
kept as a lighter fallback. On Windows, the `Setup-…-x64.exe` installer adds a
Start-menu entry, an uninstaller and a *Programs and Features* entry; the `.zip`
is a portable build you can run from the folder you extract it into.

**First launch.** The macOS bundle is signed **ad-hoc only** — no Developer ID
and no notarisation — and the Windows binaries are unsigned, so both systems
warn the first time:

- **macOS:** right-click the app, choose **Open**, then confirm. A plain
  double-click still shows the "unidentified developer" dialog; removing that
  step needs an Apple Developer ID and notarisation, which the project does not
  have. [docs/macos.md](docs/macos.md) details what is and is not signed.
- **Windows:** when SmartScreen reports "Windows protected your PC", choose
  **More info**, then **Run anyway**. See [docs/windows.md](docs/windows.md).

### Build from source

Requires Rust (see [Building requirements](#building-requirements)).

```sh
git clone https://github.com/kawazoeh/Spellcode spellcode
cd spellcode
cargo build --release
```

On macOS the binary is `target/release/spellcode-app`; to get a real app bundle
and then a disk image:

```sh
scripts/bundle.sh   # target/Spellcode.app, sealed and ad-hoc signed
scripts/dmg.sh      # Spellcode.dmg around that bundle
```

`scripts/bundle.sh` takes an optional icon path and defaults to
`assets/spellcode01.png`.

On Windows, drop the macOS-only renderer feature and build the workspace:

```sh
cargo build --release --no-default-features
```

GPUI compiles its HLSL shaders with `fxc.exe` at build time, so the Windows SDK
must be installed and discoverable; [docs/windows.md](docs/windows.md) lists the
exact requirements. The binary is `target/release/spellcode-app.exe`.

Run the test suite with:

```sh
cargo test --workspace
```

## Building requirements

- **Rust 1.85 or newer.** The workspace uses edition 2024.
- **macOS 11.0 or newer.** The window draws native traffic-light buttons over a
  translucent, blurred surface, and the app bundle declares
  `LSMinimumSystemVersion` 11.0.
- **No full Xcode install is required** for the macOS build. GPUI's default
  macOS renderer compiles its shaders with the Xcode `metal` toolchain, which is
  not part of the Command Line Tools; the default `macos-blade` feature uses the
  Blade backend instead, which validates its WGSL shaders through Rust.
  `scripts/bundle.sh` uses `sips` and `iconutil`, both shipped with macOS, to
  build the `.icns`.
- **Windows** needs the MSVC toolchain and a Windows SDK that provides
  `fxc.exe`. Build with `--no-default-features`, because `macos-blade` is a
  macOS-only renderer. See [docs/windows.md](docs/windows.md) for the details.
- **Linux** is compiled and tested in CI with `--no-default-features` (GPUI's
  X11 and Wayland backends), but no Linux binaries are published.

## Keyboard

| Shortcut                | Action                                       |
| ----------------------- | -------------------------------------------- |
| `+` (click)             | New shell tab                                |
| `+` (right-click)       | Menu: a plain shell, then configured entries |
| tab (left-click)        | Activate that tab                            |
| tab (right-click)       | Rename, icon, background, reset, close       |
| `cmd+t`                 | New shell tab                                |
| `cmd+w`                 | Close the current tab                        |
| `cmd+n` / `cmd+p`       | Next / previous tab                          |
| `cmd+=` / `cmd+-`       | Font size                                    |
| `cmd+0`                 | Reset the font size                          |
| `cmd+v`                 | Paste (bracketed when the program asks)      |
| `cmd+k` (in terminal)   | Clear the screen (sends `Ctrl+L`)            |
| `cmd+l` (in terminal)   | Kill to the end of the line (sends `Ctrl+K`) |
| `cmd+r` (in terminal)   | Jump to the top of the scrollback            |
| `cmd+end` or `cmd+down` | Jump to the bottom                           |
| mouse wheel             | Scroll the scrollback                        |

When a session has exited, `Enter` or `r` restarts it. The right-click menus
are the mouse equivalents of `cmd+w`, so nothing here depends on remembering a
shortcut. The list above is the macOS set.

## Configuration

`~/.config/spellcode/config.toml` is created on first launch. It is optional,
and it is the single source of truth: right-clicking `+` lists exactly the
entries declared there, next to a plain shell.

```toml
[general]
# The shell launched by the "Shell" entry. Empty means $SHELL, then /bin/zsh.
shell = ""
# Empty means "try the bundled list below". A family that is not installed is
# skipped rather than silently falling back to a proportional font.
font_family = ""
font_size = 13
line_height = 1.4
scrollback = 10000
# Opacity of the black wash over the macOS blur, for the window chrome.
# Lower is more transparent.
window_tint = 0.45
# Opacity of the terminal background. Lower lets more of the blur through.
terminal_opacity = 0.42
# Even inset between the border of the terminal area and the grid. It absorbs
# the sub-cell remainder too, so the text never touches the border.
padding = 14

[[apps]]
name = "Tracker API"
command = "tracker-api"
args = []
cwd = "~/src/my-repo"           # optional, defaults to the current directory
```

When `font_family` is empty, the first installed family among `JetBrains Mono`,
`SF Mono`, `Menlo`, `Monaco`, `Cascadia Code`, `Fira Code` and `monospace` is
used. A family named in the config that is not installed is skipped rather than
silently falling back to a proportional font.

Entries whose executable is missing from `PATH` are marked *not installed* in
the menu rather than hidden, so you can keep a list that travels between
machines. Remove every `[[apps]]` block to only ever get a shell.

## Look

- The window is translucent and a single black wash covers the whole thing, so
  the blur reads as one surface behind the tab bar and the margins. The terminal
  card is fully opaque on top of it, so text stays readable.
- The chrome is strictly greyscale. The terminal keeps a real 256 colour
  palette, because full-screen programs rely on it.
- Tabs carry a Font Awesome icon on the left and a background colour, both
  chosen from the right-click menu. `Reset` puts them back.
- The dot on a tab reports its state: bright while the session is running, dim
  when the view is scrolled back, faint once the session has exited.

## Layout

```
crates/
  spellcode-term/   no UI: VT emulation and the PTY
    term.rs         TerminalCore over alacritty_terminal
    pty.rs          process spawn, reader thread, input
  spellcode-app/    GPUI
    main.rs         window setup, transparent title bar
    workspace.rs    tab bar, panes, global shortcuts
    overlay.rs      context menu, rename dialog, icon and colour pickers
    icon.rs         bundled Font Awesome face and the curated icon list
    terminal.rs     a session: grid rendering, cursor, key encoding
    keys.rs         keystroke to ANSI byte sequences
    theme.rs        monochrome chrome, 256 colour terminal palette
    config.rs       config.toml loading
```

## How the terminal stays fast

- **Never redraw when nothing happened.** A pane is only notified when the PTY
  produced output or the cursor blinked, so an idle terminal costs nothing. A
  full viewport repaint then costs about as much as a blinking cursor would.
- **One shape call per style run.** Adjacent cells that share a foreground,
  background and weight become a single string, so a 200x50 grid is a few
  hundred shaped lines rather than ten thousand glyphs. GPUI caches the
  resulting layouts, so a repaint of unchanged text is nearly free.
- **Output is drained in batches.** A reader thread fills 64 KiB chunks into a
  channel; the UI polls every 8 ms and feeds everything it finds to the
  emulator in one go, so a program spamming the PTY costs one frame, not one
  frame per write.
- **True 256 colour, 24 bit.** Named, indexed and direct-colour cells are all
  resolved through the full xterm palette, and OSC colour queries are answered
  so shells and prompts detect the real background. The chrome stays strictly
  black and white, the terminal keeps its colours.

## Contributing

Issues and pull requests are welcome. Before opening a pull request, run
`cargo build --release` and `cargo test --workspace`, and keep changes focused
on one thing. There is no contribution guide beyond that; ask in the issue if
anything is unclear.

## Security

Please report a security problem privately through this repository's
[security advisories](https://github.com/kawazoeh/Spellcode/security/advisories/new) rather than in a
public issue, so a fix can be prepared before the report is visible.

## License

MIT. See [LICENSE](LICENSE).
