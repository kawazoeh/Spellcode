//! PTY lifecycle: spawn a command, stream its output, feed it input.

use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::mpsc::{Receiver, TryRecvError, channel},
    thread,
};

use anyhow::{Context, Result};
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize};

use crate::term::TermSize;

/// Read chunk size. Large enough to keep syscalls low, small enough that a
/// burst of output is picked up quickly.
const READ_BUFFER: usize = 64 * 1024;

/// What a running session reports back to the UI.
pub enum PtyEvent {
    /// Raw bytes produced by the child process.
    Output(Vec<u8>),
    /// The child's output stream reached EOF.
    Closed,
}

/// Everything needed to spawn a session.
#[derive(Clone)]
pub struct SpawnSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: Option<PathBuf>,
    pub env: Vec<(String, String)>,
    pub size: TermSize,
}

impl SpawnSpec {
    pub fn new(program: impl Into<String>, size: TermSize) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: None,
            env: Vec::new(),
            size,
        }
    }
}

/// A live child process attached to a pseudo terminal.
pub struct PtySession {
    writer: Box<dyn Write + Send>,
    master: Box<dyn MasterPty + Send>,
    killer: Box<dyn ChildKiller + Send + Sync>,
    events: Receiver<PtyEvent>,
    size: TermSize,
    closed: bool,
}

impl PtySession {
    pub fn spawn(spec: SpawnSpec) -> Result<Self> {
        let pty_system = portable_pty::native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: spec.size.rows,
                cols: spec.size.cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .context("failed to allocate a pty")?;

        let mut command = CommandBuilder::new(&spec.program);
        command.args(&spec.args);
        if let Some(cwd) = &spec.cwd {
            command.cwd(cwd);
        }
        // `spec.env` is an override layer, not the whole environment: the child
        // inherits the parent's variables (HOME, PATH, USER, LANG, ...) and the
        // terminal-specific entries are applied on top. Seeding the base here
        // keeps that contract explicit instead of relying on the pty backend to
        // carry the parent environment across. Nothing is invented — a variable
        // the parent does not have stays absent in the child.
        for (key, value) in std::env::vars_os() {
            command.env(key, value);
        }
        for (key, value) in &spec.env {
            command.env(key, value);
        }

        let child = pair
            .slave
            .spawn_command(command)
            .context("failed to spawn command")?;
        // Dropping the slave end is what makes the master read return EOF once
        // the child is gone.
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .context("failed to clone pty reader")?;
        let writer = pair
            .master
            .take_writer()
            .context("failed to take pty writer")?;
        let killer = child.clone_killer();

        let (tx, events) = channel();
        thread::Builder::new()
            .name("spellcode-pty-reader".into())
            .spawn(move || {
                let mut buf = [0u8; READ_BUFFER];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            if tx.send(PtyEvent::Output(buf[..n].to_vec())).is_err() {
                                break;
                            }
                        }
                    }
                }
                let _ = tx.send(PtyEvent::Closed);
            })
            .context("failed to spawn the pty reader thread")?;

        // The master has to outlive the reader thread, so it stays owned here
        // and is used to propagate window size changes.
        Ok(Self {
            writer,
            master: pair.master,
            killer,
            events,
            size: spec.size,
            closed: false,
        })
    }

    /// Drains every pending event without blocking.
    ///
    /// Returns the concatenated output and whether the child just closed.
    pub fn drain(&mut self) -> (Vec<u8>, bool) {
        let mut output = Vec::new();
        let mut closed = self.closed;

        loop {
            match self.events.try_recv() {
                Ok(PtyEvent::Output(bytes)) => output.extend_from_slice(&bytes),
                Ok(PtyEvent::Closed) => closed = true,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    closed = true;
                    break;
                }
            }
        }

        self.closed = closed;
        (output, closed)
    }

    pub fn send(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    pub fn resize(&mut self, size: TermSize) {
        if size == self.size {
            return;
        }
        self.size = size;
        let _ = self.master.resize(PtySize {
            rows: size.rows,
            cols: size.cols,
            pixel_width: 0,
            pixel_height: 0,
        });
    }
}

impl Drop for PtySession {
    fn drop(&mut self) {
        let _ = self.killer.kill();
    }
}
