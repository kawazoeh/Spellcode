//! Regression test: a PTY child must inherit the parent's environment and only
//! have the terminal-specific overrides applied on top.
//!
//! The child is asked to dump its environment with the command the host OS
//! provides (`env` on Unix, `cmd /C set` on Windows), so the test runs on every
//! platform the CI builds.

use std::time::{Duration, Instant};

use spellcode_term::{PtySession, SpawnSpec, TermSize};

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
    loop {
        let (chunk, closed) = session.drain();
        out.extend_from_slice(&chunk);
        if closed || Instant::now() > deadline {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    String::from_utf8_lossy(&out).into_owned()
}

/// Reads `KEY=value` from an environment dump. Keys are compared
/// case-insensitively: Windows stores and prints them uppercased.
fn lookup(env: &str, key: &str) -> Option<String> {
    env.lines().find_map(|line| {
        let (name, value) = line.split_once('=')?;
        name.trim()
            .eq_ignore_ascii_case(key)
            .then(|| value.trim_end_matches('\r').to_string())
    })
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
