# Security Policy

Spellcode is a pre-1.0 desktop terminal written in Rust. It is a native
application: it runs local PTYs, renders with the GPU, and reads a single
`config.toml`. There is no server component and no telemetry.

## Supported versions

Only the latest published release receives security fixes. Fixes are merged to
`main` and ship in the next release.

| Version | Supported |
| ------- | --------- |
| 0.1.x (latest release) | ✅ |
| < latest release | ❌ |
| `main` (development) | ✅ fix targets, not a distribution channel |

## Reporting a vulnerability

**Do not open a public issue for a security problem.** Public reports are
disclosed to everyone the moment they are posted.

Use **private vulnerability reporting** on this repository:

1. Open <https://github.com/kawazoeh/Spellcode/security/advisories/new>
   (or **Security → Report a vulnerability** in the repository menu).
2. Describe the issue as precisely as you can.
3. Do not publish details until a fix and a release exist.

If private reporting is unavailable for any reason, open a regular issue that
says only "security report available on request", with no technical detail, and
wait for contact.

### What to include

- Type of issue (memory safety, PTY escape, local privilege, path handling,
  dependency, build or release integrity).
- Affected version or commit, and the platform (macOS / Windows / Linux).
- Minimal reproduction: command, `config.toml` fragment, or steps.
- Impact: what an attacker gains, and what access they need beforehand.
- Logs, crash output, or a backtrace with symbols if you have one.

Please give us reasonable time to ship a fix before public disclosure.
Removing personally identifying information from logs before attaching them is
appreciated.

## Scope

**In scope**

- `crates/spellcode-term` and `crates/spellcode-app` (the workspace).
- The build and release workflows (`.github/workflows/`), including artefact
  signing, packaging and checksums.
- The `config.toml` parsing and any filesystem path it can influence.
- Vulnerabilities in the dependency tree that are reachable from the code
  shipped to users.

**Out of scope**

- Attacks that require an attacker to already control the user's account, or
  physical access to an unlocked, already-compromised machine.
- Reports about a shell that a user deliberately runs inside a Spellcode tab.
  Spellcode does not sandbox the programs it starts: running an untrusted
  binary gives that binary the user's own privileges. Use an operating-system
  sandbox, container or VM for that.
- Denial of service that only affects your own machine (a runaway process,
  a tab flooding its own screen).
- Missing hardening suggestions with no demonstrated impact, and automated
  scanner output with no reproduction.

## How a report is handled

1. **Acknowledgement** within 3 working days.
2. **Assessment**: impact and fix scope, with a target date within 14 days. If
   the report is out of scope or a duplicate, you get that answer in writing.
3. **Fix**: a patch on a private branch or a private fork, and a regression test
   where one is practical.
4. **Release**: the fix ships as a normal release, and you get credit in the
   release notes unless you prefer otherwise.
5. **Advisory**: a GitHub Security Advisory credits the reporter, unless you ask
   for it to stay private.

## For contributors

- Never commit a token, a signing certificate, or a personal key. Use a CI
  secret or an environment variable. GitHub secret scanning and push protection
  will block the obvious cases; they are not a substitute for care.
- Bump a dependency only when `cargo audit` and the CodeQL job are green, and
  mention the advisory or issue number in the pull request when the bump is a
  security fix.
- Do not add a dependency that parses untrusted input, spawns a process, or
  links native code without saying so in the pull request.
