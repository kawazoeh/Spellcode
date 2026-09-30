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

        let child = match pair.slave.spawn_command(command) {
            Ok(child) => child,
            // The OS error for a missing program (Windows `CreateProcessW`,
            // with a NUL-terminated path) is unreadable. Replace it with a
            // plain message when the program really cannot be resolved.
            Err(_) if !program_exists(&spec.program) => {
                anyhow::bail!("{}", not_found_message(&spec.program));
            }
            Err(error) => return Err(error.context("failed to spawn command")),
        };
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

/// Whether `program` can be resolved to something the OS will run: an existing
/// file when it contains a path separator, otherwise an entry on `PATH`
/// (expanded through `PATHEXT` on Windows for a bare name).
fn program_exists(program: &str) -> bool {
    if program.contains('/') || program.contains('\\') {
        return std::path::Path::new(program).is_file();
    }
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    let mut names = vec![program.to_string()];
    if cfg!(windows) && std::path::Path::new(program).extension().is_none() {
        let pathext =
            std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
        for ext in pathext.split(';').filter(|ext| !ext.is_empty()) {
            names.push(format!("{program}{ext}"));
        }
    }
    std::env::split_paths(&path).any(|dir| names.iter().any(|name| dir.join(name).is_file()))
}

/// Readable "cannot start" message. A shell keeps the wording the user
/// expects; anything else is reported as a command.
fn not_found_message(program: &str) -> String {
    let kind = if is_shell(program) {
        "shell"
    } else {
        "command"
    };
    format!("{kind} not found: {program}")
}

/// Whether the program name looks like an interactive shell. Only used to pick
/// the wording of an error message, never to change behaviour.
fn is_shell(program: &str) -> bool {
    let name = program
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(program)
        .to_ascii_lowercase();
    let stem = name.strip_suffix(".exe").unwrap_or(name.as_str());
    matches!(
        stem,
        "zsh"
            | "bash"
            | "sh"
            | "fish"
            | "dash"
            | "ksh"
            | "csh"
            | "tcsh"
            | "cmd"
            | "powershell"
            | "pwsh"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_shell_gets_a_readable_message() {
        assert_eq!(not_found_message("/bin/zsh"), "shell not found: /bin/zsh");
        assert_eq!(
            not_found_message("C:\\Windows\\cmd.exe"),
            "shell not found: C:\\Windows\\cmd.exe"
        );
        assert_eq!(not_found_message("pwsh"), "shell not found: pwsh");
    }

    #[test]
    fn missing_command_is_reported_as_a_command() {
        assert_eq!(
            not_found_message("tracker-api"),
            "command not found: tracker-api"
        );
    }

    #[test]
    fn existing_programs_are_found() {
        // `/bin/sh` exists on every Unix host; the path separator branch does
        // not consult PATH.
        #[cfg(unix)]
        assert!(program_exists("/bin/sh"));
        assert!(!program_exists("/definitely/not/here/tool"));
    }
}
