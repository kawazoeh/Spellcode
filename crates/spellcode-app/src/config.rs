//! User configuration.
//!
//! Everything is optional. The sidebar is driven entirely by `config.toml`:
//! an entry that is not declared there is never launched.

use std::path::{Path, PathBuf};

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
        let Ok(path) = std::env::var("PATH") else {
            return false;
        };
        path.split(':').any(|dir| {
            let candidate = Path::new(dir).join(&self.command);
            candidate.is_file()
        })
    }
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
pub const MONOSPACE_FAMILIES: &[&str] = &[
    "JetBrains Mono",
    "SF Mono",
    "Menlo",
    "Monaco",
    "Cascadia Code",
    "Fira Code",
    "monospace",
];

pub fn config_dir() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .map(|base| base.join("spellcode"))
}

pub fn config_path() -> Option<PathBuf> {
    config_dir().map(|dir| dir.join("config.toml"))
}

pub fn default_shell() -> String {
    std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string())
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
    ///
    /// A GUI build launched from the Finder runs with `/` as its current
    /// directory. A shell opened there is useless and read-only, and tools
    /// that keep per-project state next to the cwd (e.g. `guren` creating
    /// `.guren`) fail on a read-only filesystem, so fall back to the home
    /// directory. Nothing is invented: when neither the cwd nor `HOME` gives a
    /// directory, `None` leaves the child with the inherited working
    /// directory.
    pub fn cwd(&self) -> Option<PathBuf> {
        pick_default_cwd(
            std::env::current_dir().ok(),
            std::env::var_os("HOME").map(PathBuf::from),
        )
    }

    /// Entries the sidebar should show, in configured order.
    pub fn visible_apps(&self) -> Vec<&AppEntry> {
        self.apps.iter().filter(|app| !app.hidden).collect()
    }
}

/// Picks the working directory for a session that does not declare one.
///
/// Keeps a meaningful [`std::env::current_dir`] result, rejects `/` (the
/// current directory of a Finder-launched GUI app), and otherwise defers to
/// `HOME`. Returns `None` only when neither is usable.
fn pick_default_cwd(current: Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    match current {
        Some(dir) if dir != Path::new("/") => Some(dir),
        _ => home,
    }
}

fn starter_config() -> String {
    r##"# Spellcode configuration
#
# The sidebar shows exactly the entries listed here, nothing else.

[general]
# Font families are tried in order; the first one installed wins. Leave it
# empty to use the macOS Terminal face (SF Mono).
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

    #[test]
    fn root_cwd_falls_back_to_home() {
        assert_eq!(
            pick_default_cwd(
                Some(PathBuf::from("/")),
                Some(PathBuf::from("/Users/kaito"))
            ),
            Some(PathBuf::from("/Users/kaito"))
        );
    }

    #[test]
    fn a_real_cwd_is_kept() {
        assert_eq!(
            pick_default_cwd(
                Some(PathBuf::from("/work/project")),
                Some(PathBuf::from("/Users/kaito"))
            ),
            Some(PathBuf::from("/work/project"))
        );
    }

    #[test]
    fn root_cwd_without_home_stays_none() {
        assert_eq!(pick_default_cwd(Some(PathBuf::from("/")), None), None);
    }
}
