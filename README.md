<p align="center">
  <img src="assets/spellcode01.png" alt="The Spellcode icon: a four-pointed star with bracket-like marks above, below and to either side, drawn as faint outlines on white." width="96">
</p>

<h1 align="center">Spellcode</h1>

<p align="center"><strong>A fast, monochrome terminal for macOS and Windows.</strong></p>

<p align="center">
  <a href="https://github.com/kawazoeh/Spellcode/actions"><img src="https://img.shields.io/github/actions/workflow/status/kawazoeh/Spellcode/ci.yml?branch=main&label=build" alt="Build status"></a>
  <a href="https://github.com/kawazoeh/Spellcode/releases"><img src="https://img.shields.io/github/v/release/kawazoeh/Spellcode" alt="Latest release"></a>
  <img src="https://img.shields.io/badge/macOS-11%2B-black?logo=apple&logoColor=white" alt="macOS 11 or newer">
  <img src="https://img.shields.io/badge/Windows-10%2B-0078D6?logo=windows&logoColor=white" alt="Windows 10 or newer">
  <a href="LICENSE"><img src="https://img.shields.io/github/license/kawazoeh/Spellcode" alt="MIT licensed"></a>
</p>

<p align="center">English · <a href="README.fr.md">Français</a></p>

Spellcode is a tabbed terminal that stays out of your way. It's written in Rust
and built on [GPUI](https://gpui.rs), the GPU-accelerated UI framework from the
makers of Zed, and it draws every pane on the GPU. The app's own chrome is black
and white; your shell keeps all 256 of its colours, because full-screen programs
expect nothing less.

The tab bar *is* the title bar. There's no extra strip of grey sitting above
your terminal: the window buttons sit inline with the wordmark, the bar is
translucent over the system blur, and `+` opens a new shell (or any program
you've added to the config). A shell is already running when the window appears.

It also feels quick, because it refuses to do work it doesn't need. A pane only
repaints when the PTY actually produces output or the cursor blinks, so an idle
tab costs nothing. The details are [under the hood](#under-the-hood), if you
like that sort of thing.

## Install

Every release ships the same six files. Grab the one for your machine from the
[releases page](https://github.com/kawazoeh/Spellcode/releases):

| Platform             | File                                 |
| -------------------- | ------------------------------------ |
| macOS · Apple silicon | `Spellcode-macos-arm64.dmg`         |
| macOS · Apple silicon | `Spellcode-macos-arm64.zip`         |
| macOS · Intel        | `Spellcode-macos-x86_64.dmg`         |
| macOS · Intel        | `Spellcode-macos-x86_64.zip`         |
| Windows · x64        | `Setup-Spellcode-<version>-x64.exe`  |
| Windows · x64        | `Spellcode-windows-x86_64.zip`       |

**macOS.** Take the `.dmg`, open it, and drag Spellcode onto Applications. The
`.zip` is the same app bundle without the disk image, if you'd rather have that.
The build is signed ad-hoc, not with a Developer ID, so Gatekeeper will ask once:
right-click the app, choose **Open**, and confirm. After that, a normal
double-click works. Opening it silently on the very first launch would need a
paid Apple Developer ID and notarisation; [docs/macos.md](docs/macos.md) spells
out exactly what is and isn't signed.

**Windows.** The `Setup-…-x64.exe` installer gives you a Start-menu entry, an
uninstaller and a line in *Programs and Features*; the `.zip` is a portable
build if you'd rather not install anything. Neither is code-signed, so
SmartScreen shows "Windows protected your PC" on first launch: click **More
info**, then **Run anyway**. Build notes live in [docs/windows.md](docs/windows.md).

## Build it yourself

You'll need Rust 1.85 or newer, and that's most of it. There's no full Xcode
requirement on macOS: the default `macos-blade` renderer compiles GPUI's shaders
through Rust instead of the Xcode `metal` toolchain.

```sh
git clone https://github.com/kawazoeh/Spellcode spellcode
cd spellcode
cargo build --release
```

To get a real macOS app bundle (sealed and ad-hoc signed, so Finder doesn't call
it damaged), and then a disk image:

```sh
scripts/bundle.sh   # target/Spellcode.app
scripts/dmg.sh      # Spellcode.dmg
```

On Windows, drop the macOS-only renderer and build the workspace. GPUI compiles
its HLSL shaders with `fxc.exe` at build time, so the Windows SDK has to be
installed and discoverable; [docs/windows.md](docs/windows.md) covers the rest.

```sh
cargo build --release --no-default-features
```

The binary lands at `target/release/spellcode-app` (`spellcode-app.exe` on
Windows). Run the tests with:

```sh
cargo test --workspace
```

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

`Enter` or `r` restarts a session that has exited. The right-click menus mirror
`cmd+w`, so you never have to memorise a shortcut if you'd rather not. The list
above is the macOS set.

## Configuration

Everything lives in `~/.config/spellcode/config.toml`, which is created on first
launch and is entirely optional. It's also the only place the new-tab menu looks:
right-click `+` and you get a plain shell plus exactly the entries you declared
there, nothing more.

```toml
[general]
# The shell launched by the "Shell" entry. Empty means $SHELL, then /bin/zsh.
shell = ""
# Empty means "try the list below". A family that is not installed is skipped
# rather than silently falling back to a proportional font.
font_family = ""
font_size = 13
line_height = 1.4
scrollback = 10000
# Opacity of the black wash over the macOS blur, for the window chrome.
# Lower is more transparent.
window_tint = 0.45
# Opacity of the terminal background. Lower lets more of the blur through.
terminal_opacity = 0.42
# Even inset between the edge of the terminal area and the grid. It absorbs the
# sub-cell remainder too, so the text never touches the border.
padding = 14

[[apps]]
name = "Tracker API"
command = "tracker-api"
args = []
cwd = "~/src/my-repo"           # optional, defaults to the current directory
```

If `font_family` is left empty, the first installed family out of `JetBrains
Mono`, `SF Mono`, `Menlo`, `Monaco`, `Cascadia Code`, `Fira Code` and
`monospace` wins. A name that isn't installed is skipped, never swapped for a
proportional font behind your back. Entries whose executable isn't on `PATH` show
up greyed out as *not installed* rather than hidden, so a config file can travel
between machines. Delete every `[[apps]]` block and you'll only ever get a shell.

## Under the hood

### Layout

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

### Why it feels fast

A pane is only notified when the PTY produced output or the cursor blinked, so a
quiet terminal does no work at all, and a full repaint costs about as much as a
blinking cursor would. Adjacent cells that share a foreground, background and
weight are shaped as one run, so a 200×50 grid becomes a few hundred shaped
lines instead of ten thousand glyphs, and GPUI caches the resulting layouts.
Output is drained in batches too: a reader thread fills 64 KiB chunks, the UI
polls every 8 ms, and a program that spams the PTY costs one frame instead of
one frame per write.

As for colour, named, indexed and direct-colour cells all resolve through the
full xterm palette, and OSC colour queries are answered so shells and prompts
detect the real background. The chrome stays strictly black and white while the
terminal keeps every colour.

## Contributing

Issues and pull requests are welcome. Run `cargo build --release` and
`cargo test --workspace` before opening a PR, and keep each change to one thing.
There's no contribution guide beyond that; just ask in the issue if something is
unclear.

## Security

Found a security problem? Report it privately through the repo's
[security advisories](https://github.com/kawazoeh/Spellcode/security/advisories/new)
rather than in a public issue, so a fix can land before the report is visible.

## License

MIT. See [LICENSE](LICENSE).
