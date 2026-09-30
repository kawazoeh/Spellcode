//! The terminal pane: owns a session, renders its grid, and turns input into
//! bytes.
//!
//! A pane is only rendered when it is notified, and a notified pane repaints
//! its whole viewport. That is what makes a full-screen TUI cheap: the cost of
//! a frame is the cost of a blinking cursor, and an idle pane costs nothing.

use std::{cell::Cell, rc::Rc, time::Duration};

use alacritty_terminal::{
    event::WindowSize,
    grid::Scroll,
    term::{TermMode, cell::Flags},
    vte::ansi::{Color as VtColor, CursorShape, NamedColor},
};
use gpui::{
    App, Background, Bounds, ClipboardItem, Context, EventEmitter, FocusHandle, Font, KeyDownEvent,
    MouseButton, MouseDownEvent, Pixels, Point, Rgba, ScrollDelta, ScrollWheelEvent, SharedString,
    Size, StrikethroughStyle, TextRun, TextSystem, UnderlineStyle, Window, canvas, div, prelude::*,
    px,
};
use spellcode_term::{
    PtySession, RequestSink, Requests, SpawnSpec, TermRequest, TermSize, TerminalCore,
};

use crate::{
    keys,
    theme::{self, Palette, Theme},
};

/// Extra inset on the right, where the grid is measured in whole cells and the
/// leftover would otherwise crowd the border.
const EXTRA_RIGHT: f32 = 12.;

/// The first line sits closer to the tab bar than the rest of the grid sits to
/// the other edges, so the text reads as attached to its tab.
const EXTRA_TOP: f32 = -8.;

/// How often we look for new PTY output. Fast enough to feel instant, cheap
/// enough to stay off the CPU when nothing happens.
const PUMP_INTERVAL: Duration = Duration::from_millis(8);
const BLINK_INTERVAL: Duration = Duration::from_millis(530);

/// The area the workspace grants the pane, shared without going through an
/// entity so the pane can read it while it is rendering.
#[derive(Clone, Default)]
pub struct Viewport {
    cell: Rc<Cell<(f32, f32)>>,
}

impl Viewport {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, width: Pixels, height: Pixels) {
        self.cell.set((f32::from(width), f32::from(height)));
    }

    pub fn get(&self) -> (Pixels, Pixels) {
        let (width, height) = self.cell.get();
        (px(width), px(height))
    }
}

/// Resolved font handles for the four styles a terminal cell can use.
struct Fonts {
    regular: Font,
    bold: Font,
    italic: Font,
    bold_italic: Font,
    line_height: Pixels,
    cell_width: Pixels,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct CellStyle {
    fg: gpui::Hsla,
    bg: gpui::Hsla,
    bold: bool,
    italic: bool,
    underline: bool,
    strike: bool,
}

/// A run of adjacent cells sharing the same style, shaped as a single string.
struct StyledRun {
    start_column: u16,
    text: String,
    style: CellStyle,
}

impl StyledRun {
    /// Whether the run currently reaches up to `column`, so a cell of the same
    /// style can be appended instead of starting a new run.
    fn covers(&self, column: usize) -> bool {
        self.start_column as usize + self.text.chars().count() == column
    }
}

/// Everything the paint pass needs, computed once per frame during render.
struct FrameData {
    origin: Point<Pixels>,
    cell_width: Pixels,
    cell_height: Pixels,
    font_size: Pixels,
    fonts: [Font; 4],
    /// The terminal's default background, fully opaque: used to recognise the
    /// cells that do not paint a colour of their own.
    background: gpui::Hsla,
    cursor_color: gpui::Hsla,
    cursor_text: gpui::Hsla,
    lines: Vec<PaintedLine>,
    cursor: Option<PaintedCursor>,
}

struct PaintedLine {
    index: usize,
    runs: Vec<StyledRun>,
}

struct PaintedCursor {
    column: u16,
    line: u16,
    style: CellStyle,
    glyph: String,
    shape: CursorShape,
    focused: bool,
    visible: bool,
}

pub enum PaneEvent {
    Exited,
}

impl EventEmitter<PaneEvent> for TerminalPane {}

/// One running TUI or shell.
pub struct TerminalPane {
    core: TerminalCore,
    requests: Requests,
    pty: Option<PtySession>,
    spec: SpawnSpec,
    exited: bool,
    error: Option<String>,
    scrollback: usize,

    focus: FocusHandle,
    viewport: Viewport,
    fonts: Fonts,
    family: String,
    font_size: Pixels,
    line_height_factor: f32,
    padding: Pixels,
    /// Offset of the grid inside the pane. A grid is measured in whole cells,
    /// so the leftover pixels are distributed here rather than piling up on
    /// the right and bottom edges.
    origin: Point<Pixels>,
    size: TermSize,

    theme: Theme,
    palette: Palette,
    force_full: bool,
    focused: bool,
    cursor_visible: bool,
}

impl TerminalPane {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        spec: SpawnSpec,
        palette: Palette,
        theme: Theme,
        family: &str,
        font_size: f32,
        line_height_factor: f32,
        padding: f32,
        scrollback: usize,
        viewport: Viewport,
        cx: &mut Context<Self>,
    ) -> Self {
        let text_system = cx.text_system();
        let family = resolve_family(text_system, family);
        let fonts = resolve_fonts(text_system, &family, px(font_size), line_height_factor);
        let size = TermSize::default();
        let (sink, requests) = RequestSink::channel();

        let mut pane = Self {
            core: TerminalCore::new(size, scrollback, sink),
            requests,
            pty: None,
            spec,
            exited: false,
            error: None,
            scrollback,
            focus: cx.focus_handle(),
            viewport,
            fonts,
            family,
            font_size: px(font_size),
            line_height_factor,
            padding: px(padding),
            origin: point(px(0.), px(0.)),
            size,
            theme,
            palette,
            force_full: true,
            focused: true,
            cursor_visible: true,
        };
        pane.spec.size = size;

        match PtySession::spawn(pane.spec.clone()) {
            Ok(pty) => pane.pty = Some(pty),
            Err(error) => {
                pane.error = Some(format!("{error:#}"));
                pane.exited = true;
            }
        }

        pane.spawn_pump(cx);
        pane.spawn_blink(cx);
        pane
    }

    /// Drains PTY output into the emulator, repainting only on real changes.
    fn spawn_pump(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(PUMP_INTERVAL).await;
                if this.update(cx, |this, cx| this.pump(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn spawn_blink(&self, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor().timer(BLINK_INTERVAL).await;
                if this.update(cx, |this, cx| this.tick_blink(cx)).is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn pump(&mut self, cx: &mut Context<Self>) {
        let Some(pty) = self.pty.as_mut() else { return };
        let (output, closed) = pty.drain();

        if !output.is_empty() {
            self.core.write(&output);
            cx.notify();
        }

        // Answer the queries the program makes, over the same write path used
        // for keyboard input. The emulator turns a query into a request:
        // colours, verbatim replies (device status, device attributes — without
        // these `cmd.exe` blocks on Windows), and the pixel text area size.
        // Window titles are deliberately ignored: a tab is named after what it
        // runs, or after what the user called it, never after whatever the
        // program feels like calling itself this frame.
        let window_size = WindowSize {
            num_lines: self.size.rows,
            num_cols: self.size.cols,
            cell_width: u32::from(self.fonts.cell_width).min(u32::from(u16::MAX)) as u16,
            cell_height: u32::from(self.fonts.line_height).min(u32::from(u16::MAX)) as u16,
        };
        let mut reply = String::new();
        let background = self.palette.background;
        let foreground = self.palette.foreground;
        self.requests.drain(|request| match request {
            TermRequest::Color(index, responder) => {
                reply.push_str(&responder(palette_rgb(index, background, foreground)));
            }
            TermRequest::Write(text) => reply.push_str(&text),
            TermRequest::TextAreaSize(responder) => reply.push_str(&responder(window_size)),
        });

        if !reply.is_empty() {
            pty.send(reply.as_bytes());
        }

        if closed && !self.exited {
            self.exited = true;
            cx.emit(PaneEvent::Exited);
            cx.notify();
        }
    }

    fn tick_blink(&mut self, cx: &mut Context<Self>) {
        if self.exited || !self.focused {
            return;
        }
        self.cursor_visible = !self.cursor_visible;
        cx.notify();
    }

    /// Forces a full repaint on the next frame.
    pub fn invalidate(&mut self) {
        self.force_full = true;
    }

    pub fn focus(&self, window: &mut Window) {
        self.focus.focus(window);
    }

    pub fn is_exited(&self) -> bool {
        self.exited
    }

    pub fn is_scrolled(&self) -> bool {
        self.core.is_scrolled()
    }

    pub fn set_font_size(&mut self, font_size: f32, line_height_factor: f32, cx: &mut App) {
        self.font_size = px(font_size);
        self.line_height_factor = line_height_factor;
        self.fonts = resolve_fonts(
            cx.text_system(),
            &self.family,
            self.font_size,
            line_height_factor,
        );
        self.force_full = true;
    }

    pub fn scroll_to_bottom(&mut self) {
        self.core.scroll_to_bottom();
        self.force_full = true;
    }

    fn send(&mut self, bytes: &[u8]) {
        if let Some(pty) = self.pty.as_mut() {
            pty.send(bytes);
        }
        if self.core.is_scrolled() {
            self.core.scroll_to_bottom();
            self.force_full = true;
        }
    }

    fn paste(&mut self, text: &str) {
        let bracketed = self.core.mode().contains(TermMode::BRACKETED_PASTE);
        let bytes = keys::encode_paste(text, bracketed);
        self.send(&bytes);
    }

    fn resize(&mut self, width: Pixels, height: Pixels) {
        let top = self.padding + px(EXTRA_TOP);
        let usable_width = (width - self.padding * 2. - px(EXTRA_RIGHT)).max(px(1.));
        let usable_height = (height - self.padding - top).max(px(1.));

        let cols = ((usable_width / self.fonts.cell_width).floor() as i64).clamp(2, 2000) as u16;
        let rows = ((usable_height / self.fonts.line_height).floor() as i64).clamp(1, 2000) as u16;
        let size = TermSize::new(cols, rows);

        // The sub-cell remainder is split so the right and bottom margins stay
        // a little wider than the left and top ones.
        let slack_x = (usable_width - size.cols as f32 * self.fonts.cell_width).max(px(0.));
        let slack_y = (usable_height - size.rows as f32 * self.fonts.line_height).max(px(0.));
        self.origin = point(self.padding + slack_x * 0.4, top + slack_y * 0.5);

        if size != self.size {
            self.size = size;
            self.spec.size = size;
            if self.core.resize(size) {
                self.force_full = true;
            }
            if let Some(pty) = self.pty.as_mut() {
                pty.resize(size);
            }
        }
    }

    /// Builds everything the paint pass needs from the current grid.
    ///
    /// The pane is only ever rendered when it was notified, and a render
    /// re-emits the element background, so a frame always repaints the whole
    /// viewport. Idleness is what makes this cheap: with no output and no
    /// blink tick nothing is notified and nothing is drawn at all.
    fn build_frame(&mut self) -> FrameData {
        self.force_full = false;

        FrameData {
            origin: point(px(0.), px(0.)),
            cell_width: self.fonts.cell_width,
            cell_height: self.fonts.line_height,
            font_size: self.font_size,
            fonts: [
                self.fonts.regular.clone(),
                self.fonts.bold.clone(),
                self.fonts.italic.clone(),
                self.fonts.bold_italic.clone(),
            ],
            // The margin around the grid keeps the terminal background, so
            // the card stays black whatever the program paints.
            background: gpui::Hsla::from(self.palette.background),
            cursor_color: gpui::Hsla::from(self.palette.cursor),
            cursor_text: gpui::Hsla::from(self.palette.cursor_text),
            lines: self.paint_lines(),
            cursor: self.paint_cursor(),
        }
    }

    /// Converts the viewport into style-run segments, one shape call per run.
    fn paint_lines(&self) -> Vec<PaintedLine> {
        let rows = self.size.rows as usize;
        let cols = self.size.cols as usize;
        if rows == 0 || cols == 0 {
            return Vec::new();
        }

        let mut lines: Vec<PaintedLine> = Vec::with_capacity(rows);
        let mut runs: Vec<StyledRun> = Vec::new();
        let mut current_line: Option<usize> = None;

        for indexed in self.core.grid().display_iter() {
            let line = indexed.point.line.0 as usize;
            if line >= rows {
                continue;
            }

            if current_line != Some(line) {
                if let Some(previous) = current_line {
                    lines.push(PaintedLine {
                        index: previous,
                        runs: std::mem::take(&mut runs),
                    });
                }
                current_line = Some(line);
            }

            let style = self.style_of(indexed.cell.fg, indexed.cell.bg, indexed.cell.flags);
            let column = indexed.point.column.0;

            match runs.last_mut() {
                Some(run) if run.style == style && run.covers(column) => {
                    run.text.push(indexed.cell.c)
                }
                _ => runs.push(StyledRun {
                    start_column: column as u16,
                    text: indexed.cell.c.to_string(),
                    style,
                }),
            }
        }

        if let Some(previous) = current_line {
            lines.push(PaintedLine {
                index: previous,
                runs,
            });
        }

        lines
    }

    fn paint_cursor(&self) -> Option<PaintedCursor> {
        if self.exited || !self.core.cursor_visible() {
            return None;
        }

        let (column, line) = self.core.cursor_screen_point()?;
        let point = alacritty_terminal::index::Point::new(
            alacritty_terminal::index::Line(line as i32),
            alacritty_terminal::index::Column(column as usize),
        );
        let cell = &self.core.grid()[point];
        Some(PaintedCursor {
            column,
            line,
            style: self.style_of(cell.fg, cell.bg, cell.flags),
            glyph: if cell.c == ' ' {
                String::new()
            } else {
                cell.c.to_string()
            },
            shape: self.core.cursor_shape(),
            focused: self.focused,
            visible: self.cursor_visible || !self.focused,
        })
    }

    fn style_of(&self, fg: VtColor, bg: VtColor, flags: Flags) -> CellStyle {
        let bold = flags.intersects(Flags::BOLD | Flags::DIM_BOLD);
        let default_fg = if bold {
            self.palette.get(8)
        } else {
            self.palette.foreground
        };

        let mut fg = self.resolve_color(fg, default_fg);
        let mut bg = self.resolve_color(bg, self.palette.background);

        if flags.contains(Flags::INVERSE) {
            std::mem::swap(&mut fg, &mut bg);
        }
        if flags.contains(Flags::HIDDEN) {
            fg = bg;
        }
        if flags.contains(Flags::DIM) {
            fg = theme::mix(bg, fg, 0.6);
        }

        CellStyle {
            fg: gpui::Hsla::from(fg),
            bg: gpui::Hsla::from(bg),
            bold,
            italic: flags.intersects(Flags::ITALIC | Flags::BOLD_ITALIC),
            underline: flags.contains(Flags::UNDERLINE),
            strike: flags.contains(Flags::STRIKEOUT),
        }
    }

    /// Resolves a VT colour, preferring any OSC override the program set.
    fn resolve_color(&self, color: VtColor, fallback: Rgba) -> Rgba {
        let index = match color {
            VtColor::Named(NamedColor::Foreground) => return fallback,
            VtColor::Named(named) => named as usize,
            VtColor::Indexed(index) => index as usize,
            VtColor::Spec(spec) => {
                return Rgba {
                    r: spec.r as f32 / 255.,
                    g: spec.g as f32 / 255.,
                    b: spec.b as f32 / 255.,
                    a: 1.,
                };
            }
        };

        match self.core.osc_colors()[index] {
            Some(osc) => Rgba {
                r: osc.r as f32 / 255.,
                g: osc.g as f32 / 255.,
                b: osc.b as f32 / 255.,
                a: 1.,
            },
            None => self.palette.color_at(index),
        }
    }

    fn on_key(&mut self, event: &KeyDownEvent, cx: &mut Context<Self>) {
        let keystroke = &event.keystroke;

        if keystroke.modifiers.platform {
            match keystroke.key.as_str() {
                "v" => {
                    if let Some(text) = clipboard_text(cx) {
                        self.paste(&text);
                    }
                }
                "k" => {
                    self.core.write(b"\x0c");
                    cx.notify();
                }
                "l" => {
                    self.core.write(b"\x0b");
                    cx.notify();
                }
                "r" => {
                    self.core.scroll(Scroll::Top);
                    self.force_full = true;
                    cx.notify();
                }
                "end" | "arrowdown" => {
                    self.scroll_to_bottom();
                    cx.notify();
                }
                _ => {}
            }
            return;
        }

        if self.exited {
            if matches!(keystroke.key.as_str(), "enter" | "r") {
                self.restart(cx);
            }
            return;
        }

        let application_cursor = self.core.mode().contains(TermMode::APP_CURSOR);
        if let Some(bytes) = keys::encode(keystroke, application_cursor) {
            self.send(&bytes);
            cx.notify();
        }
    }

    fn restart(&mut self, cx: &mut Context<Self>) {
        self.exited = false;
        self.error = None;
        let (sink, requests) = RequestSink::channel();
        self.core = TerminalCore::new(self.size, self.scrollback, sink);
        self.requests = requests;

        let mut spec = self.spec.clone();
        spec.size = self.size;
        match PtySession::spawn(spec) {
            Ok(pty) => self.pty = Some(pty),
            Err(error) => {
                self.error = Some(format!("{error:#}"));
                self.exited = true;
            }
        }

        self.force_full = true;
        cx.notify();
    }
}

impl Render for TerminalPane {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (width, height) = self.viewport.get();
        self.focused = self.focus.is_focused(window);
        self.resize(width, height);

        let frame = self.build_frame();
        let origin = self.origin;
        let theme = self.theme;
        let exited = self.exited;
        let error = self.error.clone();

        let surface = div()
            .size_full()
            .relative()
            .flex()
            .cursor_text()
            .track_focus(&self.focus)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, _| {
                    this.focus.focus(window);
                }),
            )
            .on_scroll_wheel(cx.listener(|this, event: &ScrollWheelEvent, _, cx| {
                let delta = match event.delta {
                    ScrollDelta::Pixels(delta) => delta.y,
                    ScrollDelta::Lines(delta) => delta.y * this.fonts.line_height,
                };
                this.core
                    .scroll(Scroll::Delta(-(delta / this.fonts.line_height) as i32));
                this.force_full = true;
                cx.notify();
            }))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _, cx| this.on_key(event, cx)))
            .child(
                canvas(
                    move |bounds, _window, _cx| {
                        // Painting happens in window coordinates, so anchor the
                        // grid to wherever this element actually landed.
                        let mut frame = frame;
                        frame.origin = frame.origin + bounds.origin + origin;
                        frame
                    },
                    move |_bounds, frame: FrameData, window, cx| {
                        paint_frame(&frame, window, cx);
                    },
                )
                .size_full(),
            );

        if exited {
            return div()
                .size_full()
                .child(surface)
                .child(exit_overlay(error, theme));
        }

        surface
    }
}

fn paint_frame(frame: &FrameData, window: &mut Window, cx: &mut App) {
    // The card behind the grid already paints the terminal background at the
    // configured opacity, so this pass only draws the cells that ask for a
    // colour of their own. Painting the background again here would stack the
    // alphas and the terminal would end up opaque.
    //
    // Backgrounds first so glyphs are never clipped by a neighbour's quad.
    for line in &frame.lines {
        let top = frame.origin.y + line.index as f32 * frame.cell_height;
        for run in &line.runs {
            if run.style.bg == frame.background {
                continue;
            }
            let left = frame.origin.x + run.start_column as f32 * frame.cell_width;
            let width = frame.cell_width * run.text.chars().count() as f32;
            window.paint_quad(gpui::quad(
                Bounds::new(Point::new(left, top), Size::new(width, frame.cell_height)),
                px(0.),
                Background::from(run.style.bg),
                px(0.),
                gpui::transparent_black(),
                Default::default(),
            ));
        }
    }

    // Then the glyphs, one shaped line per style run.
    for line in &frame.lines {
        let top = frame.origin.y + line.index as f32 * frame.cell_height;
        for run in &line.runs {
            if run.text.trim().is_empty() {
                continue;
            }
            paint_text(
                window,
                cx,
                Point::new(
                    frame.origin.x + run.start_column as f32 * frame.cell_width,
                    top,
                ),
                &run.text,
                &run.style,
                frame,
            );
        }
    }

    if let Some(cursor) = &frame.cursor {
        paint_cursor(window, cx, frame, cursor);
    }
}

fn paint_text(
    window: &mut Window,
    cx: &mut App,
    origin: Point<Pixels>,
    text: &str,
    style: &CellStyle,
    frame: &FrameData,
) {
    let run = TextRun {
        len: text.len(),
        font: font_for(style, frame),
        color: style.fg,
        background_color: None,
        underline: style.underline.then_some(UnderlineStyle {
            thickness: px(1.),
            color: None,
            wavy: false,
        }),
        strikethrough: style.strike.then_some(StrikethroughStyle {
            thickness: px(1.),
            color: None,
        }),
    };

    let shaped = window.text_system().shape_line(
        SharedString::from(text.to_string()),
        frame.font_size,
        &[run],
        None,
    );
    let _ = shaped.paint(origin, frame.cell_height, window, cx);
}

fn paint_cursor(window: &mut Window, cx: &mut App, frame: &FrameData, cursor: &PaintedCursor) {
    if !cursor.visible {
        return;
    }

    let left = frame.origin.x + cursor.column as f32 * frame.cell_width;
    let top = frame.origin.y + cursor.line as f32 * frame.cell_height;
    let cell = Bounds::new(
        Point::new(left, top),
        Size::new(frame.cell_width, frame.cell_height),
    );

    if !cursor.focused {
        window.paint_quad(gpui::quad(
            cell,
            px(0.),
            Background::from(frame.background),
            px(1.5),
            cursor.style.fg,
            Default::default(),
        ));
        return;
    }

    let cursor_color = frame.cursor_color;
    let glyph_color = frame.cursor_text;

    match cursor.shape {
        CursorShape::Block => {
            window.paint_quad(gpui::quad(
                cell,
                px(0.),
                Background::from(cursor_color),
                px(0.),
                gpui::transparent_black(),
                Default::default(),
            ));
            if !cursor.glyph.is_empty() {
                let mut style = cursor.style;
                style.fg = glyph_color;
                paint_text(
                    window,
                    cx,
                    Point::new(left, top),
                    &cursor.glyph,
                    &style,
                    frame,
                );
            }
        }
        CursorShape::Underline => {
            window.paint_quad(gpui::quad(
                Bounds::new(
                    Point::new(left, top + frame.cell_height - px(2.)),
                    Size::new(frame.cell_width, px(2.)),
                ),
                px(0.),
                Background::from(cursor_color),
                px(0.),
                gpui::transparent_black(),
                Default::default(),
            ));
        }
        _ => {
            window.paint_quad(gpui::quad(
                cell,
                px(0.),
                Background::from(frame.background),
                px(1.5),
                cursor_color,
                Default::default(),
            ));
        }
    }
}

fn font_for(style: &CellStyle, frame: &FrameData) -> Font {
    match (style.bold, style.italic) {
        (true, true) => frame.fonts[3].clone(),
        (true, false) => frame.fonts[1].clone(),
        (false, true) => frame.fonts[2].clone(),
        (false, false) => frame.fonts[0].clone(),
    }
}

fn exit_overlay(error: Option<String>, theme: Theme) -> impl IntoElement {
    let message = match error {
        Some(error) => format!("Could not start\n\n{error}"),
        None => "Session ended".to_string(),
    };

    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .bg(theme::fade(theme.background, 0.9))
        .child(
            div()
                .max_w(px(560.))
                .mx_4()
                .flex()
                .flex_col()
                .gap_2()
                .px_5()
                .py_4()
                .rounded_lg()
                .border_1()
                .border_color(theme.border)
                .bg(theme.surface)
                .text_color(theme.text)
                .child(div().text_sm().whitespace_normal().child(message))
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.text_faint)
                        .child("Press enter to restart"),
                ),
        )
}

fn clipboard_text(cx: &App) -> Option<String> {
    let item: ClipboardItem = cx.read_from_clipboard()?;
    item.text()
}

/// Builds the reply to an OSC colour query from our palette.
fn palette_rgb(
    index: usize,
    background: Rgba,
    foreground: Rgba,
) -> alacritty_terminal::vte::ansi::Rgb {
    let color = match index {
        theme::FOREGROUND | theme::DIM_FOREGROUND => foreground,
        theme::BACKGROUND | theme::DIM_BACKGROUND => background,
        _ => background,
    };
    alacritty_terminal::vte::ansi::Rgb {
        r: (color.r * 255.) as u8,
        g: (color.g * 255.) as u8,
        b: (color.b * 255.) as u8,
    }
}

fn resolve_fonts(
    text_system: &TextSystem,
    family: &str,
    font_size: Pixels,
    line_height_factor: f32,
) -> Fonts {
    let regular = gpui::font(family.to_string());
    let regular_id = text_system.resolve_font(&regular);
    let cell_width = text_system
        .advance(regular_id, font_size, 'M')
        .map(|size| size.width)
        .unwrap_or(font_size * 0.6);

    Fonts {
        bold: regular.clone().bold(),
        italic: regular.clone().italic(),
        bold_italic: regular.clone().bold().italic(),
        regular,
        line_height: font_size * line_height_factor,
        cell_width,
    }
}

/// Picks the monospace face to use.
///
/// A configured family is only honoured when it is actually installed;
/// otherwise we fall back through the preference list, which starts with the
/// faces macOS Terminal ships with.
fn resolve_family(text_system: &TextSystem, configured: &str) -> String {
    let available = text_system.all_font_names();
    let installed = |name: &str| available.iter().any(|candidate| candidate == name);

    if !configured.is_empty() && installed(configured) {
        return configured.to_string();
    }

    crate::config::MONOSPACE_FAMILIES
        .iter()
        .copied()
        .find(|candidate| installed(candidate))
        .unwrap_or("monospace")
        .to_string()
}

fn point(x: Pixels, y: Pixels) -> Point<Pixels> {
    Point { x, y }
}
