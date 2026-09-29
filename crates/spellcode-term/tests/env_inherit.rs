//! Regression test: a PTY child must inherit the parent's environment
//! (HOME, PATH, USER, LANG, ...) and only have the terminal-specific
//! overrides applied on top.
//!
//! Before the fix, the child saw an almost-empty environment, so `HOME` was
//! missing and any program resolving the home directory failed.

use std::time::{Duration, Instant};

use spellcode_term::{PtySession, SpawnSpec, TermSize};

/// Spawns `/usr/bin/env` in a PTY and returns the child's environment as text.
fn child_env() -> String {
    let mut spec = SpawnSpec::new("/usr/bin/env", TermSize::default());
    // Mirror what workspace.rs injects, so the test exercises the same path.
    spec.env = vec![
        ("TERM".into(), "xterm-256color".into()),
        ("COLORTERM".into(), "truecolor".into()),
        ("TERM_PROGRAM".into(), "Spellcode".into()),
    ];

    let mut session = PtySession::spawn(spec).expect("failed to spawn pty child");

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut out = Vec::new();
    loop {
        let (chunk, closed) = session.drain();
        out.extend_from_slice(&chunk);
        if closed {
            break;
        }
        if Instant::now() > deadline {
            panic!(
                "PTY child never closed; captured output so far:\n{}",
                String::from_utf8_lossy(&out)
            );
        }
        std::thread::sleep(Duration::from_millis(20));
    }

    String::from_utf8_lossy(&out).into_owned()
}

/// Reads `KEY=value` for an exact key from an `env` dump.
fn lookup(env: &str, key: &str) -> Option<String> {
    env.lines()
        .find_map(|line| line.strip_prefix(&format!("{key}=")).map(str::to_owned))
}

#[test]
fn pty_child_inherits_home_and_path() {
    let env = child_env();

    assert!(
        lookup(&env, "HOME").is_some_and(|v| !v.is_empty()),
        "HOME must be inherited by the PTY child.\nChild env:\n{env}"
    );
    assert!(
        lookup(&env, "PATH").is_some_and(|v| !v.is_empty()),
        "PATH must be inherited by the PTY child.\nChild env:\n{env}"
    );

    // The terminal-specific overrides must still win over anything inherited.
    assert_eq!(lookup(&env, "TERM").as_deref(), Some("xterm-256color"));
    assert_eq!(lookup(&env, "COLORTERM").as_deref(), Some("truecolor"));
    assert_eq!(lookup(&env, "TERM_PROGRAM").as_deref(), Some("Spellcode"));
}
