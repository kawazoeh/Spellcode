//! VT emulation state, wrapped so the UI never touches the underlying parser
//! directly.

use std::sync::{
    Arc,
    mpsc::{Receiver, Sender, channel},
};

use alacritty_terminal::{
    event::{Event, EventListener, WindowSize},
    grid::{Dimensions, Grid, Scroll},
    term::{Config, Term, TermMode},
    vte::ansi::{CursorShape, Processor, Rgb},
};

/// Smallest grid we allow: a 0-sized terminal makes the PTY misbehave.
const MIN_COLS: u16 = 2;
const MIN_ROWS: u16 = 1;

/// Terminal size in character cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TermSize {
    pub cols: u16,
    pub rows: u16,
}

impl TermSize {
    pub fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols: cols.max(MIN_COLS),
            rows: rows.max(MIN_ROWS),
        }
    }
}

impl Default for TermSize {
    fn default() -> Self {
        Self { cols: 80, rows: 24 }
    }
}

impl Dimensions for TermSize {
    fn total_lines(&self) -> usize {
        self.rows as usize
    }

    fn screen_lines(&self) -> usize {
        self.rows as usize
    }

    fn columns(&self) -> usize {
        self.cols as usize
    }
}

/// Something the emulator asks the host to do: answer a query the program made,
/// by writing the reply back to the PTY.
pub enum TermRequest {
    /// The program asked for the colour at `index`; call the responder with
    /// that colour to produce the reply.
    Color(usize, Arc<dyn Fn(Rgb) -> String + Send + Sync>),
    /// The emulator already built the exact bytes to send back — device status
    /// reports (cursor position), device attributes, mode reports. Write them
    /// to the PTY verbatim. A program that asks and gets no reply (`cmd.exe`
    /// sending `ESC[6n` under ConPTY) blocks forever.
    Write(String),
    /// The program asked for the text area size in pixels; call the responder
    /// with the current window size to produce the reply.
    TextAreaSize(Arc<dyn Fn(WindowSize) -> String + Send + Sync>),
}

/// The emulator's side of the request channel.
pub struct RequestSink {
    sender: Sender<TermRequest>,
}

impl RequestSink {
    /// Creates a sink and the matching receiver.
    pub fn channel() -> (Self, Requests) {
        let (sender, receiver) = channel();
        (Self { sender }, Requests { receiver })
    }
}

impl EventListener for RequestSink {
    fn send_event(&self, event: Event) {
        // Every event the host must act on becomes a request. Anything else the
        // emulator reports (titles, bell, wakeups) is intentionally dropped.
        let request = match event {
            Event::ColorRequest(index, responder) => TermRequest::Color(index, responder),
            Event::PtyWrite(text) => TermRequest::Write(text),
            Event::TextAreaSizeRequest(responder) => TermRequest::TextAreaSize(responder),
            _ => return,
        };
        let _ = self.sender.send(request);
    }
}

/// The host's side of the request channel.
pub struct Requests {
    receiver: Receiver<TermRequest>,
}

impl Requests {
    /// Hands every pending request to `handle`.
    pub fn drain(&mut self, mut handle: impl FnMut(TermRequest)) {
        while let Ok(request) = self.receiver.try_recv() {
            handle(request);
        }
    }
}

/// Owns the terminal emulator and tracks what needs repainting.
pub struct TerminalCore {
    term: Term<RequestSink>,
    processor: Processor,
    size: TermSize,
}

impl TerminalCore {
    pub fn new(size: TermSize, scrollback: usize, sink: RequestSink) -> Self {
        let config = Config {
            scrolling_history: scrollback,
            ..Default::default()
        };
        Self {
            term: Term::new(config, &size, sink),
            processor: Processor::new(),
            size,
        }
    }

    /// Feeds raw PTY output into the emulator.
    pub fn write(&mut self, bytes: &[u8]) {
        self.processor.advance(&mut self.term, bytes);
    }

    /// Resizes the grid. Returns `true` when the size actually changed.
    pub fn resize(&mut self, size: TermSize) -> bool {
        if size == self.size {
            return false;
        }
        self.size = size;
        self.term.resize(size);
        true
    }

    pub fn grid(&self) -> &Grid<alacritty_terminal::term::cell::Cell> {
        self.term.grid()
    }

    pub fn mode(&self) -> &TermMode {
        self.term.mode()
    }

    /// Colors set at runtime through OSC escape sequences.
    pub fn osc_colors(&self) -> &alacritty_terminal::term::color::Colors {
        self.term.colors()
    }

    /// Cursor position in viewport coordinates, or `None` when it is scrolled
    /// out of view.
    pub fn cursor_screen_point(&self) -> Option<(u16, u16)> {
        let grid = self.term.grid();
        let point = self.term.renderable_content().cursor.point;

        if point.column.0 >= self.size.cols as usize {
            return None;
        }

        let screen_lines = grid.screen_lines();
        let top = grid.total_lines() - grid.display_offset() - screen_lines;
        let row = point.line.0 as isize - top as isize;
        if row < 0 || row >= screen_lines as isize {
            return None;
        }

        Some((point.column.0 as u16, row as u16))
    }

    pub fn cursor_visible(&self) -> bool {
        self.term.renderable_content().cursor.shape != CursorShape::Hidden
    }

    pub fn cursor_shape(&self) -> CursorShape {
        self.term.renderable_content().cursor.shape
    }

    pub fn is_scrolled(&self) -> bool {
        self.term.grid().display_offset() > 0
    }

    pub fn scroll(&mut self, scroll: Scroll) {
        self.term.scroll_display(scroll);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.term.scroll_display(Scroll::Bottom);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn core() -> (TerminalCore, Requests) {
        let (sink, requests) = RequestSink::channel();
        (
            TerminalCore::new(TermSize::default(), 1_000, sink),
            requests,
        )
    }

    /// Collects the verbatim replies the emulator produced.
    fn writes(requests: &mut Requests) -> Vec<String> {
        let mut out = Vec::new();
        requests.drain(|request| {
            if let TermRequest::Write(text) = request {
                out.push(text);
            }
        });
        out
    }

    /// `ESC[6n` (DSR, cursor position) must produce a reply: this is the exact
    /// query `cmd.exe` sends under ConPTY and then waits for.
    #[test]
    fn cursor_position_query_is_answered() {
        let (mut core, mut requests) = core();
        core.write(b"\x1b[6n");
        assert_eq!(writes(&mut requests), vec!["\x1b[1;1R".to_string()]);
    }

    /// `ESC[c` (DA1, device attributes) must also produce a reply.
    #[test]
    fn device_attributes_query_is_answered() {
        let (mut core, mut requests) = core();
        core.write(b"\x1b[c");
        let replies = writes(&mut requests);
        assert_eq!(replies.len(), 1, "one DA reply expected, got {replies:?}");
        assert!(
            replies[0].starts_with("\x1b[?") && replies[0].ends_with('c'),
            "got {replies:?}"
        );
    }

    /// `ESC[14t` (text area size in pixels) needs host data, so it must arrive
    /// as a `TextAreaSize` request rather than be dropped.
    #[test]
    fn text_area_size_query_reaches_the_host() {
        let (mut core, mut requests) = core();
        core.write(b"\x1b[14t");
        let mut asked = false;
        requests.drain(|request| {
            if let TermRequest::TextAreaSize(_) = request {
                asked = true;
            }
        });
        assert!(asked, "the pixel size query must reach the host");
    }

    /// The existing colour path must stay intact.
    #[test]
    fn colour_query_still_reaches_the_host() {
        let (mut core, mut requests) = core();
        core.write(b"\x1b]10;?\x07");
        let mut colors = 0;
        requests.drain(|request| {
            if let TermRequest::Color(..) = request {
                colors += 1;
            }
        });
        assert_eq!(colors, 1, "the colour path must stay intact");
    }
}
