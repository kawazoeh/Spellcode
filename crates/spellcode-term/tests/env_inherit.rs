//! Regression test: a PTY child must inherit the parent's environment and only
//! have the terminal-specific overrides applied on top.
//!
//! The child dumps its environment with the command the host OS provides
//! (`env` on Unix, `cmd /C set` on Windows), so the test runs everywhere the CI
//! builds. A bare `pty` has no terminal emulator behind it, so the test answers
//! the terminal queries the child makes — notably the cursor-position report
//! (DSR) that `cmd.exe` sends on startup and then waits for.

use std::time::{Duration, Instant};

use spellcode_term::{PtySession, SpawnSpec, TermSize};

/// Terminal queries a real emulator answers, with the reply to send back. A
/// pty with no emulator would leave the child blocked waiting for them.
const QUERIES: &[(&[u8], &[u8])] = &[
    // DSR: report the cursor position. `cmd.exe` sends this under ConPTY at
    // startup and waits for the reply before printing anything.
    (b"\x1b[6n", b"\x1b[1;1R"),
    // DSR: report the terminal is operating.
    (b"\x1b[5n", b"\x1b[0n"),
    // DA1 / DA2: device attributes.
    (b"\x1b[c", b"\x1b[?1;0c"),
    (b"\x1b[>c", b"\x1b[>0;0;0c"),
    // CSI 18 t: report the text area size in cells (answered from the grid).
    (b"\x1b[18t", b"\x1b[8;24;80t"),
];

/// A command that prints the environment of the process it runs in, as
/// `KEY=VALUE` lines.
fn env_command() -> (String, Vec<String>) {
    if cfg!(windows) {
        let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string());
        (comspec, vec!["/C".to_string(), "set".to_string()])
    } else {
        ("env".to_string(), Vec::new())
    }
}

/// First index of `needle` in `haystack`.
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Answers the queries found in `pending`, consuming each exactly once. Keeps
/// only a short tail, long enough to hold the prefix of a query split across
/// two reads.
fn answer_queries(session: &mut PtySession, pending: &mut Vec<u8>) {
    loop {
        let mut earliest: Option<(usize, usize, &'static [u8])> = None;
        for (pattern, reply) in QUERIES {
            if let Some(index) = find_subslice(pending, pattern)
                && earliest.is_none_or(|(start, _, _)| index < start)
            {
                earliest = Some((index, pattern.len(), reply));
            }
        }
        match earliest {
            Some((index, length, reply)) => {
                if !reply.is_empty() {
                    session.send(reply);
                }
                pending.drain(..index + length);
            }
            None => break,
        }
    }
    const TAIL: usize = 8;
    if pending.len() > TAIL {
        let drop = pending.len() - TAIL;
        pending.drain(..drop);
    }
}

/// Runs the environment-dumping command in a PTY and returns its output.
fn child_env() -> String {
    let (program, args) = env_command();
    let mut spec = SpawnSpec::new(program, TermSize::default());
    spec.args = args;
    // Mirror what workspace.rs injects, so the test exercises the same path.
    spec.env = vec![
        ("TERM".into(), "xterm-256color".into()),
        ("COLORTERM".into(), "truecolor".into()),
        ("TERM_PROGRAM".into(), "Spellcode".into()),
    ];

    let mut session = PtySession::spawn(spec).expect("failed to spawn pty child");

    // Drain until the child closes or the deadline passes. Stopping at the
    // deadline without panicking keeps the test meaningful on platforms where
    // the pty does not report EOF promptly; the assertions below still check
    // whatever the child printed.
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut out = Vec::new();
    let mut pending = Vec::new();
    loop {
        let (chunk, closed) = session.drain();
        out.extend_from_slice(&chunk);
        pending.extend_from_slice(&chunk);
        answer_queries(&mut session, &mut pending);
        if closed || Instant::now() > deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    String::from_utf8_lossy(&out).into_owned()
}

/// Parses `KEY=VALUE` lines. Keys are matched case-insensitively: Windows
/// stores and prints them uppercased.
fn parse_env(env: &str) -> Vec<(String, String)> {
    env.lines()
        .filter_map(|line| {
            let (name, value) = line.split_once('=')?;
            let name = name.trim();
            if name.is_empty() {
                return None;
            }
            Some((name.to_string(), value.trim_end_matches('\r').to_string()))
        })
        .collect()
}

fn lookup(env: &str, key: &str) -> Option<String> {
    parse_env(env)
        .into_iter()
        .find_map(|(name, value)| name.eq_ignore_ascii_case(key).then_some(value))
}

/// A variable that exists in this process on every platform, used as the
/// inheritance probe.
fn probe_variable() -> &'static str {
    // `TERM` is excluded on purpose: it is one of the overridden variables.
    const CANDIDATES: &[&str] = &["HOME", "USERPROFILE", "USERNAME", "USER", "PATH"];
    CANDIDATES
        .iter()
        .copied()
        .find(|key| std::env::var_os(key).is_some_and(|value| !value.is_empty()))
        .expect("the test process has at least one environment variable")
}

#[test]
fn pty_child_inherits_the_parent_environment() {
    let env = child_env();

    // Guard the real failure mode first: a child that printed nothing (for
    // example because it is blocked on an unanswered terminal query) must say
    // so, instead of a misleading "must be inherited" on an empty dump.
    assert!(
        !parse_env(&env).is_empty(),
        "the PTY child printed no environment variables; captured output was:\n{env:?}"
    );

    let probe = probe_variable();
    let expected = std::env::var(probe).expect("the probe variable is set");
    assert_eq!(
        lookup(&env, probe).as_deref(),
        Some(expected.as_str()),
        "{probe} must be inherited verbatim by the PTY child.\nChild env:\n{env}"
    );
    assert!(
        lookup(&env, "PATH").is_some_and(|value| !value.is_empty()),
        "PATH must be inherited by the PTY child.\nChild env:\n{env}"
    );

    // The terminal-specific overrides must still win over anything inherited.
    assert_eq!(lookup(&env, "TERM").as_deref(), Some("xterm-256color"));
    assert_eq!(lookup(&env, "COLORTERM").as_deref(), Some("truecolor"));
    assert_eq!(lookup(&env, "TERM_PROGRAM").as_deref(), Some("Spellcode"));
}
