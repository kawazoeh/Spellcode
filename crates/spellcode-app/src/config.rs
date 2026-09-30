//! User configuration.
//!
//! Everything is optional. The sidebar is driven entirely by `config.toml`:
//! an entry that is not declared there is never launched.
//!
//! The platform defaults (shell, config directory, home directory) are
//! resolved by pure functions that take the target OS and an environment map
//! as arguments, so the Windows branch is exercised by unit tests running on
//! any host.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::Deserialize;

/// One launchable program, listed in the new-tab menu.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AppEntry {
    /// Display name.
    pub name: String,
    /// Executable, plus optional arguments.
    pub command: String,
    /// Optional arguments appended to the command.
    #[serde(default)]
    pub args: Vec<String>,
    /// Working directory. Empty means "inherit".
    #[serde(default)]
    pub cwd: String,
    /// Kept in the config but not shown anywhere.
    #[serde(default)]
    pub hidden: bool,
}

impl AppEntry {
    /// Whether the executable can actually be found on `PATH`.
    pub fn is_available(&self) -> bool {
        let Some(path) = std::env::var_os("PATH") else {
            return false;
        };
        let candidates = command_candidates(&self.command);
        std::env::split_paths(&path)
            .any(|dir| candidates.iter().any(|name| dir.join(name).is_file()))
    }
}

/// File names to probe on `PATH` for `command`. `std::env::split_paths` already
/// handles the platform separator (`;` on Windows); a bare name such as `cmd`
/// additionally resolves through `PATHEXT` on Windows (`.exe`, `.cmd`, ...).
fn command_candidates(command: &str) -> Vec<String> {
    let mut names = vec![command.to_string()];
    if cfg!(windows) && Path::new(command).extension().is_none() {
        let pathext =
            std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
        for ext in pathext.split(';').filter(|ext| !ext.is_empty()) {
            names.push(format!("{command}{ext}"));
        }
    }
    names
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct General {
    /// Shell launched by the "Shell" entry.
    pub shell: String,
    /// Monospace font family. The first available family of
    /// [`MONOSPACE_FAMILIES`] is used when empty.
    pub font_family: String,
    pub font_size: f32,
    pub line_height: f32,
    pub scrollback: usize,
    /// Opacity of the black wash over the macOS blur. Lower is more
    /// transparent.
    pub window_tint: f32,
    /// Opacity of the terminal background over the blur.
    pub terminal_opacity: f32,
    /// Space kept between the edge of the pane and the first cell. Zero is what
    /// full-screen programs expect, and it is what keeps the rounded corners
    /// filled with the program's own background.
    pub padding: f32,
}

impl Default for General {
    fn default() -> Self {
        Self {
            shell: default_shell(),
            font_family: String::new(),
            font_size: 13.,
            line_height: 1.4,
            scrollback: 10_000,
            window_tint: 0.45,
            terminal_opacity: 0.42,
            padding: 14.,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: General,
    pub apps: Vec<AppEntry>,
}

/// Monospace families we try, in order, when none is configured.
///
/// The macOS-preferred faces keep their original order; the Windows-only
/// families are inserted after them and before the generic `monospace`
/// fallback, so a macOS machine with none of the Windows fonts is unaffected.
pub const MONOSPACE_FAMILIES: &[&str] = &[
    "JetBrains Mono",
    "SF Mono",
    "Menlo",
    "Monaco",
    "Cascadia Code",
    "Fira Code",
    "Cascadia Mono",
    "Consolas",
    "monospace",
];

/// The operating system a default is resolved for. It is an explicit parameter
/// so the platform logic is a pure function, unit-testable from any host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Platform {
    Macos,
    Windows,
    Other,
}

impl Platform {
    fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Macos
        } else {
            Self::Other
        }
    }
}

/// The environment variables the platform logic reads, injected so tests can
/// exercise the Windows branch from a macOS or Linux host.
type Env = BTreeMap<String, String>;

fn process_env() -> Env {
    std::env::vars_os()
        .filter_map(|(key, value)| Some((key.into_string().ok()?, value.into_string().ok()?)))
        .collect()
}

/// The shell a session starts when the config does not name one.
fn platform_shell(platform: Platform, env: &Env) -> String {
    let configured = match platform {
        Platform::Windows => env.get("COMSPEC"),
        Platform::Macos | Platform::Other => env.get("SHELL"),
    };
    configured
        .filter(|value| !value.is_empty())
        .cloned()
        .unwrap_or_else(|| fallback_shell(platform).to_string())
}

/// Fallback shell when the environment names none. macOS keeps `/bin/zsh`,
/// unchanged from the previous behaviour; other Unix systems use `/bin/sh`;
/// Windows uses `cmd.exe`.
fn fallback_shell(platform: Platform) -> &'static str {
    match platform {
        Platform::Macos => "/bin/zsh",
        Platform::Windows => "cmd.exe",
        Platform::Other => "/bin/sh",
    }
}

/// The user home directory. Windows stores it in `USERPROFILE` (with `HOME`,
/// set by some shells, as a secondary source); other systems use `HOME`.
fn platform_home(platform: Platform, env: &Env) -> Option<PathBuf> {
    let value = match platform {
        Platform::Windows => env.get("USERPROFILE").or_else(|| env.get("HOME")),
        Platform::Macos | Platform::Other => env.get("HOME"),
    };
    value.filter(|value| !value.is_empty()).map(PathBuf::from)
}

/// The directory holding `config.toml`.
///
/// Windows uses `%APPDATA%\spellcode` (falling back to
/// `%USERPROFILE%\AppData\Roaming\spellcode`); macOS and other systems keep the
/// XDG behaviour (`$XDG_CONFIG_HOME/spellcode`, else `$HOME/.config/spellcode`).
fn platform_config_dir(platform: Platform, env: &Env) -> Option<PathBuf> {
    let base = match platform {
        Platform::Windows => env
            .get("APPDATA")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                platform_home(Platform::Windows, env)
                    .map(|home| home.join("AppData").join("Roaming"))
            }),
        Platform::Macos | Platform::Other => env
            .get("XDG_CONFIG_HOME")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| platform_home(platform, env).map(|home| home.join(".config"))),
    };
    base.map(|base| base.join("spellcode"))
}

/// The working directory a session opens in when its entry does not set one.
///
/// A GUI build run from the Finder has `/` as its current directory: it is
/// read-only and useless, and tools that keep per-project state next to the
/// cwd (e.g. `guren` creating `.guren`) fail there. A filesystem root — `/` or
/// `C:\` — is therefore rejected and the home directory is used instead.
/// Nothing is invented: with no usable cwd and no home, `None` leaves the child
/// with the inherited directory.
fn platform_default_cwd(
    platform: Platform,
    current: Option<PathBuf>,
    env: &Env,
) -> Option<PathBuf> {
    match current {
        Some(dir) if dir.parent().is_some() => Some(dir),
        _ => platform_home(platform, env),
    }
}

pub fn config_dir() -> Option<PathBuf> {
    platform_config_dir(Platform::current(), &process_env())
}

pub fn config_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("config.toml"))
}

pub fn default_shell() -> String {
    platform_shell(Platform::current(), &process_env())
}

/// The user home directory for the current platform (`USERPROFILE` on Windows,
/// `HOME` elsewhere).
pub fn home_dir() -> Option<PathBuf> {
    platform_home(Platform::current(), &process_env())
}

impl Config {
    /// Loads the user config. The sidebar shows exactly what is configured,
    /// so there is no built-in launcher list to fall back to.
    pub fn load() -> Self {
        let mut config = Config::default();

        let Some(path) = config_path() else {
            return config;
        };

        match std::fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str::<Config>(&contents) {
                Ok(user) => {
                    config.general = user.general;
                    config.apps = user.apps;
                }
                Err(error) => eprintln!("spellcode: {}: {error}", path.display()),
            },
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                eprintln!("spellcode: {}: {error}", path.display());
            }
            Err(_) => {}
        }

        if config.general.shell.is_empty() {
            config.general.shell = default_shell();
        }

        config
    }

    /// Writes a commented starter config, leaving an existing file alone.
    pub fn ensure_starter_file() {
        let Some(path) = config_path() else { return };
        if path.exists() {
            return;
        }
        if let Some(parent) = path.parent()
            && std::fs::create_dir_all(parent).is_ok()
        {
            let _ = std::fs::write(&path, starter_config());
        }
    }

    /// Working directory a session opens in when its entry does not set one.
    pub fn cwd(&self) -> Option<PathBuf> {
        platform_default_cwd(
            Platform::current(),
            std::env::current_dir().ok(),
            &process_env(),
        )
    }

    /// Entries the sidebar should show, in configured order.
    pub fn visible_apps(&self) -> Vec<&AppEntry> {
        self.apps.iter().filter(|app| !app.hidden).collect()
    }
}

fn starter_config() -> String {
    r##"# Spellcode configuration
#
# The sidebar shows exactly the entries listed here, nothing else.

[general]
# Font families are tried in order; the first one installed wins. Leave it
# empty to use the default monospace face.
font_family = ""
font_size = 13
line_height = 1.4
scrollback = 10000
# Opacity of the black wash over the macOS blur (window chrome).
window_tint = 0.45
# Opacity of the terminal background. Lower lets more of the blur through.
terminal_opacity = 0.42
# Even inset between the border of the terminal area and the grid. It also
# absorbs the sub-cell remainder on the right and bottom, so the text never
# touches the border.
padding = 14

[[apps]]
name = "Tracker API"
# The program to run. It is looked up on $PATH unless it contains a slash.
command = "tracker-api"
"##
    .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> Env {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    // --- shell -----------------------------------------------------------

    #[test]
    fn shell_uses_the_platform_variable() {
        assert_eq!(
            platform_shell(
                Platform::Windows,
                &env(&[("COMSPEC", "C:\\Windows\\cmd.exe")])
            ),
            "C:\\Windows\\cmd.exe"
        );
        assert_eq!(
            platform_shell(Platform::Macos, &env(&[("SHELL", "/bin/zsh")])),
            "/bin/zsh"
        );
        assert_eq!(
            platform_shell(Platform::Other, &env(&[("SHELL", "/bin/bash")])),
            "/bin/bash"
        );
    }

    #[test]
    fn shell_falls_back_per_platform() {
        assert_eq!(platform_shell(Platform::Windows, &env(&[])), "cmd.exe");
        assert_eq!(platform_shell(Platform::Macos, &env(&[])), "/bin/zsh");
        assert_eq!(platform_shell(Platform::Other, &env(&[])), "/bin/sh");
    }

    #[test]
    fn empty_shell_variable_is_treated_as_unset() {
        assert_eq!(
            platform_shell(Platform::Windows, &env(&[("COMSPEC", "")])),
            "cmd.exe"
        );
        assert_eq!(
            platform_shell(Platform::Macos, &env(&[("SHELL", "")])),
            "/bin/zsh"
        );
    }

    // --- config directory ------------------------------------------------

    #[test]
    fn config_dir_macos_prefers_xdg_then_home() {
        assert_eq!(
            platform_config_dir(Platform::Macos, &env(&[("XDG_CONFIG_HOME", "/x")])),
            Some(PathBuf::from("/x").join("spellcode"))
        );
        assert_eq!(
            platform_config_dir(Platform::Macos, &env(&[("HOME", "/Users/kaito")])),
            Some(PathBuf::from("/Users/kaito/.config/spellcode"))
        );
    }

    #[test]
    fn config_dir_windows_uses_appdata() {
        let resolved = platform_config_dir(
            Platform::Windows,
            &env(&[("APPDATA", "C:\\Users\\Zenax\\AppData\\Roaming")]),
        )
        .expect("APPDATA resolves a config directory");
        assert_eq!(
            resolved,
            PathBuf::from("C:\\Users\\Zenax\\AppData\\Roaming").join("spellcode")
        );
    }

    #[test]
    fn config_dir_windows_falls_back_to_userprofile() {
        let resolved = platform_config_dir(
            Platform::Windows,
            &env(&[("USERPROFILE", "C:\\Users\\Zenax")]),
        )
        .expect("USERPROFILE resolves a config directory");
        assert_eq!(
            resolved,
            PathBuf::from("C:\\Users\\Zenax")
                .join("AppData")
                .join("Roaming")
                .join("spellcode")
        );
    }

    #[test]
    fn config_dir_linux_uses_home() {
        assert_eq!(
            platform_config_dir(Platform::Other, &env(&[("HOME", "/home/kaito")])),
            Some(PathBuf::from("/home/kaito/.config/spellcode"))
        );
    }

    #[test]
    fn config_dir_is_none_without_an_anchor() {
        assert_eq!(platform_config_dir(Platform::Windows, &env(&[])), None);
        assert_eq!(platform_config_dir(Platform::Macos, &env(&[])), None);
        assert_eq!(platform_config_dir(Platform::Other, &env(&[])), None);
    }

    // --- home directory --------------------------------------------------

    #[test]
    fn home_is_resolved_per_platform() {
        assert_eq!(
            platform_home(
                Platform::Windows,
                &env(&[("USERPROFILE", "C:\\Users\\Zenax")])
            ),
            Some(PathBuf::from("C:\\Users\\Zenax"))
        );
        // Windows falls back to HOME when a shell exports it.
        assert_eq!(
            platform_home(Platform::Windows, &env(&[("HOME", "C:\\Users\\Zenax")])),
            Some(PathBuf::from("C:\\Users\\Zenax"))
        );
        assert_eq!(
            platform_home(Platform::Macos, &env(&[("HOME", "/Users/kaito")])),
            Some(PathBuf::from("/Users/kaito"))
        );
        assert_eq!(platform_home(Platform::Other, &env(&[])), None);
    }

    // --- working directory -----------------------------------------------

    #[test]
    fn cwd_keeps_a_real_directory() {
        assert_eq!(
            platform_default_cwd(
                Platform::Macos,
                Some(PathBuf::from("/work/project")),
                &env(&[("HOME", "/Users/kaito")]),
            ),
            Some(PathBuf::from("/work/project"))
        );
    }

    #[test]
    fn root_cwd_falls_back_to_home() {
        assert_eq!(
            platform_default_cwd(
                Platform::Macos,
                Some(PathBuf::from("/")),
                &env(&[("HOME", "/Users/kaito")]),
            ),
            Some(PathBuf::from("/Users/kaito"))
        );
    }

    #[test]
    fn root_cwd_without_home_stays_none() {
        assert_eq!(
            platform_default_cwd(Platform::Macos, Some(PathBuf::from("/")), &env(&[])),
            None
        );
    }

    #[test]
    fn missing_cwd_uses_windows_home() {
        assert_eq!(
            platform_default_cwd(
                Platform::Windows,
                None,
                &env(&[("USERPROFILE", "C:\\Users\\Zenax")]),
            ),
            Some(PathBuf::from("C:\\Users\\Zenax"))
        );
    }

    /// A GUI build run from the Finder has `/` as its current directory. A new
    /// session must never open there: the directory is read-only, and programs
    /// that derive a per-project state directory from the cwd (e.g. `guren`
    /// creating `.guren`) fail immediately.
    #[test]
    fn default_cwd_is_never_root() {
        let previous = std::env::current_dir().expect("test needs a current directory");
        std::env::set_current_dir("/").expect("cannot enter /");
        let resolved = Config::default().cwd();
        std::env::set_current_dir(&previous).expect("cannot restore the working directory");

        assert!(
            resolved.is_some(),
            "a default working directory must resolve"
        );
        let resolved = resolved.unwrap();
        assert_ne!(
            resolved.as_path(),
            Path::new("/"),
            "a session must not open in /"
        );
        assert!(
            resolved.is_dir(),
            "the resolved working directory must exist"
        );
    }

    // --- command lookup --------------------------------------------------

    #[test]
    fn bare_command_expands_on_windows_only() {
        let candidates = command_candidates("tracker-api");
        #[cfg(not(windows))]
        assert_eq!(candidates, vec!["tracker-api".to_string()]);
        #[cfg(windows)]
        assert!(candidates.len() > 1, "PATHEXT extensions must be probed");
    }

    #[test]
    fn a_command_with_a_path_is_checked_verbatim() {
        // The command contains a separator: extensions must not be appended.
        let candidates = command_candidates("some/dir/tool");
        assert_eq!(candidates, vec!["some/dir/tool".to_string()]);
    }
}
