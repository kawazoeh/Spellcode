//! Headless terminal core for Spellcode.
//!
//! This crate is intentionally free of any UI dependency: it owns the VT
//! emulation ([`term`]) and the process side of things ([`pty`]).

pub mod pty;
pub mod term;

pub use pty::{PtyEvent, PtySession, SpawnSpec};
pub use term::{RequestSink, Requests, TermRequest, TermSize, TerminalCore};
