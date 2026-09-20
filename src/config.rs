//! Persistence for the desktop configuration.
//!
//! Each component lives in its own file under `~/.config/cli-desktop/`:
//!
//!   icons.json      – desktop icons (position, command, glyph, singleton flag)
//!   trash.json      – icons moved to the trash
//!   wallpaper.json  – path to the background image (optional)
//!   settings.json   – time intervals (taskbar, clock, full redraw)
//!   colors.json     – UI color scheme
//!
//! This allows, for example, versioning/sharing just `icons.json`, or
//! changing the wallpaper/color scheme independently of the icons via a script.

use crate::caps::Capabilities;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::OnceLock;

pub const CONFIG_DIR_NAME: &str = "cli-desktop";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Icon {
    pub id: u32,
    pub name: String,
    /// One or two characters representing the icon in text mode, e.g. "[T]".
    pub glyph: String,
    pub x: u16,
    pub y: u16,
    /// Shell command opened as a new tmux window on double-click.
    pub command: String,
    /// If `true`: before opening, first check whether a tmux window with
    /// this name already exists and switch to it instead of opening
    /// another one (useful for programs where you sensibly only want a
    /// single instance, e.g. a process monitor).
    #[serde(default)]
    pub singleton: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct WallpaperFile {
    path: Option<String>,
}

/// Time intervals that would otherwise be constants in the code. Values in seconds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// How often the taskbar automatically re-queries tmux.
    pub window_poll_secs: u64,
    /// The minimum redraw interval (used, among other things, for the clock).
    pub clock_refresh_secs: u64,
    /// How often the screen is additionally cleared and redrawn from
    /// scratch (self-healing against rendering artifacts).
    pub full_redraw_secs: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            window_poll_secs: 2,
            clock_refresh_secs: 1,
            full_redraw_secs: 5,
        }
    }
}

/// UI color scheme. Values are either a known color name (`white`, `black`,
/// `red`, `green`, `yellow`, `blue`, `magenta`, `cyan`, `grey`/`gray`,
/// `darkgrey`/`darkgray`) or a hex color `#rrggbb` (only visible if the
/// terminal supports truecolor/256 colors; a tty console automatically
/// falls back to the 16 basic colors, see `caps.rs`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Theme {
    pub icon_fg: String,
    pub label_fg: String,
    pub selected_fg: String,
    pub selected_bg: String,
    pub danger_fg: String,
    pub danger_bg: String,
    pub trash_full_fg: String,
    pub taskbar_fg: String,
    pub taskbar_bg: String,
    pub taskbar_active_fg: String,
    pub taskbar_active_bg: String,
    pub status_fg: String,
    pub clock_fg: String,
    pub clock_bg: String,
    /// Color of the wallpaper characters when there's no color support
    /// (tty console) or no original color could be computed.
    pub wallpaper_fallback_fg: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            icon_fg: "white".into(),
            label_fg: "cyan".into(),
            selected_fg: "black".into(),
            selected_bg: "yellow".into(),
            danger_fg: "white".into(),
            danger_bg: "red".into(),
            trash_full_fg: "yellow".into(),
            taskbar_fg: "white".into(),
            taskbar_bg: "blue".into(),
            taskbar_active_fg: "black".into(),
            taskbar_active_bg: "cyan".into(),
            status_fg: "grey".into(),
            clock_fg: "white".into(),
            clock_bg: "black".into(),
            wallpaper_fallback_fg: "darkgrey".into(),
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct DesktopConfig {
    pub icons: Vec<Icon>,
    pub trash: Vec<Icon>,
    pub wallpaper: Option<String>,
    pub settings: Settings,
    pub theme: Theme,
}

/// Allows the configuration directory to be set to something other than the
/// default (`~/.config/cli-desktop`), e.g. via the `--config-dir` CLI flag.
/// Must be set before the first access to `config_dir()`.
static CONFIG_DIR_OVERRIDE: OnceLock<PathBuf> = OnceLock::new();

pub fn set_config_dir_override(path: PathBuf) {
    let _ = CONFIG_DIR_OVERRIDE.set(path);
}

/// Determine the base directory (creating it if needed): either the
/// directory set via `set_config_dir_override`, or `~/.config/cli-desktop`
/// by default.
pub fn config_dir() -> Result<PathBuf> {
    let dir = if let Some(p) = CONFIG_DIR_OVERRIDE.get() {
        p.clone()
    } else {
        let base = dirs::config_dir().context("Could not determine the config directory (~/.config)")?;
        base.join(CONFIG_DIR_NAME)
    };
    std::fs::create_dir_all(&dir).with_context(|| format!("Could not create directory {:?}", dir))?;
    Ok(dir)
}

fn icons_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("icons.json"))
}

fn trash_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("trash.json"))
}

fn wallpaper_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("wallpaper.json"))
}

fn settings_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("settings.json"))
}

fn colors_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("colors.json"))
}

/// Read a JSON file, returning `None` if it doesn't exist.
///
/// If the file exists but is corrupted or incompatible (e.g. after a
/// program update changed the schema), the program does NOT crash: the
/// broken file is moved aside with a timestamp (`icons.bak-<unixtime>.json`),
/// a warning is printed, and `None` is returned so the caller falls back to
/// a default value.
fn read_json<T: for<'de> Deserialize<'de>>(path: &PathBuf) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("Could not read {:?}", path))?;
    match serde_json::from_str::<T>(&raw) {
        Ok(value) => Ok(Some(value)),
        Err(e) => {
            let backup = backup_path(path);
            if std::fs::rename(path, &backup).is_ok() {
                eprintln!(
                    "Warning: {:?} is invalid ({}). The file was moved to {:?}, using default values.",
                    path, e, backup
                );
            } else {
                eprintln!(
                    "Warning: {:?} is invalid ({}). Using default values (the file was left in place).",
                    path, e
                );
            }
            Ok(None)
        }
    }
}

/// Generate a backup file name with a Unix timestamp, e.g. `icons.bak-1732900000.json`.
fn backup_path(path: &PathBuf) -> PathBuf {
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("config");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("json");
    let mut backup = path.clone();
    backup.set_file_name(format!("{stem}.bak-{ts}.{ext}"));
    backup
}

/// Write atomically: first to a temporary file in the same directory, then
/// swap it into place via `rename`. `rename` is atomic on the same
/// filesystem, so a crash mid-write (power loss, `kill -9`) never leaves a
/// half-written/corrupt target file behind – at worst, an orphaned `.tmp`
/// file remains.
fn write_json<T: Serialize>(path: &PathBuf, value: &T) -> Result<()> {
    let raw = serde_json::to_string_pretty(value)?;
    let tmp = PathBuf::from(format!("{}.tmp", path.display()));
    std::fs::write(&tmp, raw).with_context(|| format!("Could not write {:?}", tmp))?;
    std::fs::rename(&tmp, path).with_context(|| format!("Could not atomically replace {:?}", path))?;
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct InstanceLock {
    pid: u32,
    session: String,
    window: Option<u32>,
}

fn lock_path() -> Result<PathBuf> {
    Ok(config_dir()?.join("instance.lock"))
}

/// Check whether another rust-desktop instance already seems to be running
/// in the same tmux session, then record ourselves as the current instance.
///
/// This doesn't block startup — running a second instance still works —
/// but it's a common source of confusion (and can trigger tmux errors like
/// `create window failed: index N in use` if both instances try to open a
/// window at nearly the same moment), so we warn about it clearly instead
/// of leaving the user to guess. The check is best-effort: if a previous
/// instance was killed with SIGKILL (which can't be caught to clean up
/// after itself) its lock file would otherwise linger forever, so a stale
/// lock is recognized by checking whether that pid is still alive via
/// `/proc/<pid>` and silently ignored/overwritten if not.
pub fn check_and_write_instance_lock(session: &str, window: Option<u32>) -> Option<String> {
    let path = lock_path().ok()?;
    let warning = read_json::<InstanceLock>(&path).ok().flatten().and_then(|prev| {
        let alive = std::path::Path::new(&format!("/proc/{}", prev.pid)).exists();
        if alive && prev.session == session {
            let win = prev
                .window
                .map(|w| w.to_string())
                .unwrap_or_else(|| "?".into());
            Some(format!(
                "another rust-desktop instance seems to already be running in this session \
                 (window {win}, pid {pid}). Running two instances that share the same \
                 configuration can interfere with each other (e.g. tmux 'index in use' errors, \
                 or icons.json being written by both at once) — consider switching to window \
                 {win} instead.",
                win = win,
                pid = prev.pid
            ))
        } else {
            None
        }
    });

    let mine = InstanceLock {
        pid: std::process::id(),
        session: session.to_string(),
        window,
    };
    let _ = write_json(&path, &mine);
    warning
}

/// Remove our own instance lock on clean shutdown, so a later run doesn't
/// have to rely on the `/proc/<pid>` staleness check at all.
pub fn clear_instance_lock() {
    if let Ok(path) = lock_path() {
        let _ = std::fs::remove_file(path);
    }
}

impl DesktopConfig {
    /// Loads every config file individually. If a file is missing, a
    /// sensible default is generated for that part and immediately written
    /// out as its own file (so this is typically only relevant on the very
    /// first run).
    pub fn load_or_default(caps: &Capabilities) -> Result<Self> {
        let icons = match read_json::<Vec<Icon>>(&icons_path()?)? {
            Some(icons) => icons,
            None => {
                let icons = default_icons(caps);
                write_json(&icons_path()?, &icons)?;
                icons
            }
        };

        let trash = read_json::<Vec<Icon>>(&trash_path()?)?.unwrap_or_default();

        let wallpaper = read_json::<WallpaperFile>(&wallpaper_path()?)?
            .and_then(|w| w.path);

        let settings = match read_json::<Settings>(&settings_path()?)? {
            Some(s) => s,
            None => {
                let s = Settings::default();
                write_json(&settings_path()?, &s)?;
                s
            }
        };

        let theme = match read_json::<Theme>(&colors_path()?)? {
            Some(t) => t,
            None => {
                let t = Theme::default();
                write_json(&colors_path()?, &t)?;
                t
            }
        };

        Ok(Self { icons, trash, wallpaper, settings, theme })
    }

    pub fn save_icons(&self) -> Result<()> {
        write_json(&icons_path()?, &self.icons)
    }

    pub fn save_trash(&self) -> Result<()> {
        write_json(&trash_path()?, &self.trash)
    }

    pub fn save_wallpaper(&self) -> Result<()> {
        write_json(&wallpaper_path()?, &WallpaperFile { path: self.wallpaper.clone() })
    }

    #[allow(dead_code)]
    pub fn save_settings(&self) -> Result<()> {
        write_json(&settings_path()?, &self.settings)
    }

    #[allow(dead_code)]
    pub fn save_theme(&self) -> Result<()> {
        write_json(&colors_path()?, &self.theme)
    }

    /// Convenience method: write every file at once.
    #[allow(dead_code)]
    pub fn save_all(&self) -> Result<()> {
        self.save_icons()?;
        self.save_trash()?;
        self.save_wallpaper()?;
        self.save_settings()?;
        self.save_theme()?;
        Ok(())
    }
}

/// Safely embed an arbitrary script as a single `sh -c '...'` argument, so
/// nested quotes (e.g. inside `$SHELL`/`$EDITOR` values, or the messages
/// below) can't break out of the generated shell command.
fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Build a `sh -c '<script>'` command as a single string, with `<script>`
/// fully quoted/escaped as one unit. Wrapping the *entire* fallback chain
/// (including all `||`/`()` control flow) inside one `sh -c` argument is
/// important: it guarantees the script is parsed by POSIX `sh`, regardless
/// of what the user's actual login shell is. If any part of the fallback
/// logic were left outside the `sh -c '...'` quoting, it would instead be
/// parsed by tmux's *default-shell* — whatever the user's login shell
/// happens to be — and shells like fish or csh/tcsh don't understand
/// `||`/`(...)` the POSIX way, which could silently break the command.
fn sh_c(script: &str) -> String {
    format!("sh -c {}", shell_quote(script))
}

/// A handful of sensible default icons with programs available on
/// virtually every Linux system (with fallbacks via `sh -c`).
/// Nicer glyphs are used when emoji support is detected; the plain tty
/// console gets the compatible ASCII bracket icons instead.
///
/// `$SHELL`/`$EDITOR` are resolved here in Rust rather than embedded as
/// `${SHELL:-bash}`-style shell syntax in the command string: tmux executes
/// an icon's `command` via the pane's *default-shell*, which is whatever
/// the user's login shell happens to be. Not every shell understands that
/// POSIX parameter-expansion syntax (fish and csh/tcsh notably don't), so
/// embedding it could silently fail to launch anything on those shells.
/// Resolving the value ourselves and passing the plain result avoids that
/// dependency entirely.
///
/// Every command below also has a `|| (... ; read x)` fallback, all inside
/// one `sh -c` call (see `sh_c` above). This matters because a tmux window
/// closes itself the instant its command exits — if the target program
/// doesn't exist on a given system (missing editor, missing process
/// monitor, ...), the window would otherwise flash open and vanish again
/// within milliseconds, which looks exactly like "nothing happened" when
/// double-clicking the icon. With the fallback, the window stays open with
/// a clear message instead.
fn default_icons(caps: &Capabilities) -> Vec<Icon> {
    let g = |emoji: &str, ascii: &str| -> String {
        if caps.emoji { emoji.to_string() } else { ascii.to_string() }
    };
    let resolved_shell = std::env::var("SHELL")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "bash".to_string());
    let resolved_editor = std::env::var("EDITOR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| "vi".to_string());

    let terminal_cmd = sh_c(&format!(
        "{shell} || (echo 'Shell not found: {shell}'; echo 'Set \\$SHELL or edit icons.json.'; read x)",
        shell = resolved_shell
    ));
    let editor_candidates: Vec<&str> = {
        let mut c = vec![resolved_editor.as_str()];
        for alt in ["vi", "nano"] {
            if !c.contains(&alt) {
                c.push(alt);
            }
        }
        c
    };
    let editor_cmd = sh_c(&format!(
        "{chain} || (echo 'No editor found ({tried} all missing).'; echo 'Install one, or edit icons.json.'; read x)",
        chain = editor_candidates.join(" || "),
        tried = editor_candidates.join(" / ")
    ));
    let files_cmd = sh_c(
        "ranger || mc || (echo 'No file manager found (ranger / mc missing).'; ls -la; echo; echo 'Press enter to close.'; read x)"
    );
    let processes_cmd = sh_c(
        "htop || top || (echo 'No process monitor found (htop / top missing).'; read x)"
    );

    vec![
        Icon {
            id: 1,
            name: "Terminal".into(),
            glyph: g("🖥️", "[T]"),
            x: 4,
            y: 2,
            command: terminal_cmd,
            singleton: false,
        },
        Icon {
            id: 2,
            name: "Editor".into(),
            glyph: g("📝", "[E]"),
            x: 4,
            y: 6,
            command: editor_cmd,
            singleton: false,
        },
        Icon {
            id: 3,
            name: "Files".into(),
            glyph: g("📁", "[D]"),
            x: 4,
            y: 10,
            command: files_cmd,
            singleton: false,
        },
        Icon {
            id: 4,
            name: "Processes".into(),
            glyph: g("📊", "[P]"),
            x: 4,
            y: 14,
            // A process monitor makes the most sense as a single instance:
            // double-clicking switches to an already-open "Processes"
            // window instead of opening another htop/top window.
            command: processes_cmd,
            singleton: true,
        },
    ]
}
