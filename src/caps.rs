//! Detects whether we're running on a raw Linux console (tty, framebuffer
//! console) or inside a "real" terminal emulator, and derives from that
//! which character sets and colors can sensibly be used.
//!
//! Reason: the Linux console (TERM=linux) generally has no color emoji font
//! and often only 16 colors, whereas terminal emulators such as Alacritty,
//! kitty, iTerm2, GNOME Terminal, Windows Terminal, etc. usually render
//! truecolor, full Unicode, and emoji cleanly.
//!
//! Crucially, this can *not* simply check the pane's own `$TERM`: tmux
//! always overrides that to one of its own terminfo entries (typically
//! `tmux-256color`), regardless of what the real outer terminal is — the
//! bare Linux console and a full GUI terminal emulator look identical from
//! inside a pane's own `$TERM`. See `tmux::client_termname()` for the
//! actual outer-terminal value this uses instead.

use crate::tmux;
use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSupport {
    /// Only the 16 basic tty colors (e.g. Linux framebuffer console).
    Basic16,
    /// 256-color palette (e.g. TERM=xterm-256color, tmux-256color).
    Ansi256,
    /// 24-bit RGB ("truecolor").
    TrueColor,
}

#[derive(Debug, Clone)]
pub struct Capabilities {
    /// Is the process running on the raw Linux framebuffer console instead
    /// of inside a terminal emulator?
    pub is_console_tty: bool,
    /// Locale is UTF-8 -> Unicode block characters (░▒▓█ etc.) can be used.
    pub unicode: bool,
    /// Colored, multi-part emoji glyphs are probably rendered cleanly.
    pub emoji: bool,
    pub color: ColorSupport,
    /// The raw values detection was based on, kept around purely so `i`
    /// can show *why* a given call was made — makes it possible to tell
    /// whether an unexpected result is because tmux reported something
    /// this program doesn't recognize, or a genuine bug elsewhere.
    pub debug_outer_term: String,
    pub debug_client_tty: String,
}

fn is_console_term_name(term: &str) -> bool {
    // The Linux framebuffer console (real tty, no emulator) practically
    // always reports itself as TERM=linux (or "linux-16color" etc.).
    // GNU Hurd uses "hurd", some BSD consoles use "cons25".
    term == "linux" || term.starts_with("linux-") || term == "hurd" || term == "cons25"
}

/// Whether a tty device path refers to a genuine Linux virtual console
/// rather than a pseudo-terminal. Every terminal emulator, every SSH
/// session, `screen`/`tmux` itself, `script`, etc. attaches through a
/// pseudo-terminal (`/dev/pts/N`); only a real virtual console shows up as
/// `/dev/ttyN` (a plain number, not e.g. `/dev/ttyUSB0` or `/dev/ttyS0`,
/// which are serial ports) or `/dev/console`. This doesn't depend on
/// `$TERM` being set to any particular string at all, which makes it a
/// good second, independent signal alongside `is_console_term_name`.
fn is_console_tty_device(path: &str) -> bool {
    if path == "/dev/console" {
        return true;
    }
    for prefix in ["/dev/tty", "/dev/vc/"] {
        if let Some(rest) = path.strip_prefix(prefix) {
            if !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit()) {
                return true;
            }
        }
    }
    false
}

impl Capabilities {
    pub fn detect() -> Self {
        // The *outer* terminal's $TERM, as tmux saw it when the client
        // attached — this is what actually tells us whether the real
        // terminal is the bare console or a GUI emulator. Falls back to
        // this process's own (tmux-overridden) $TERM if that can't be
        // determined for some reason (e.g. not actually running under
        // tmux yet, such as during very early manual testing).
        let outer_term = tmux::client_termname().unwrap_or_else(|| env::var("TERM").unwrap_or_default());
        let client_tty = tmux::client_tty().unwrap_or_default();
        let colorterm = env::var("COLORTERM").unwrap_or_default();
        let lang = env::var("LC_ALL")
            .or_else(|_| env::var("LC_CTYPE"))
            .or_else(|_| env::var("LANG"))
            .unwrap_or_default()
            .to_lowercase();

        // Two independent signals, either one being true is enough: the
        // outer $TERM naming convention, and (more reliably, since it
        // doesn't depend on $TERM at all) the actual device path of the
        // attached client's terminal.
        let is_console_tty =
            is_console_term_name(&outer_term) || is_console_tty_device(&client_tty);

        let unicode = lang.contains("utf-8") || lang.contains("utf8");

        // Colored emoji glyphs need an emoji font; the Linux console doesn't
        // have one (and the glyphs are often missing from the console font
        // entirely) -> disable on a real tty, allow it otherwise.
        let emoji = unicode && !is_console_tty;

        let color = if is_console_tty {
            // Framebuffer console: stick to the safe 16 standard colors,
            // regardless of what COLORTERM claims.
            ColorSupport::Basic16
        } else if colorterm.eq_ignore_ascii_case("truecolor") || colorterm.eq_ignore_ascii_case("24bit") {
            ColorSupport::TrueColor
        } else if outer_term.contains("256color") {
            ColorSupport::Ansi256
        } else {
            ColorSupport::Basic16
        };

        Self {
            is_console_tty,
            unicode,
            emoji,
            color,
            debug_outer_term: outer_term,
            debug_client_tty: client_tty,
        }
    }

    /// Can be overridden manually via CLI flag for testing/debugging.
    pub fn with_overrides(mut self, force_tty: bool, force_emoji: bool) -> Self {
        if force_tty {
            self.is_console_tty = true;
            self.unicode = false;
            self.emoji = false;
            self.color = ColorSupport::Basic16;
        }
        if force_emoji {
            self.unicode = true;
            self.emoji = true;
        }
        self
    }

    pub fn summary(&self) -> String {
        let env_kind = if self.is_console_tty { "Linux console (tty)" } else { "Terminal emulator" };
        let color = match self.color {
            ColorSupport::Basic16 => "16 colors",
            ColorSupport::Ansi256 => "256 colors",
            ColorSupport::TrueColor => "Truecolor",
        };
        let charset = if self.unicode { "Unicode" } else { "ASCII" };
        let emoji = if self.emoji { "emoji on" } else { "emoji off" };
        format!(
            "{} · {} · {} · {} (term={:?} tty={:?})",
            env_kind, color, charset, emoji, self.debug_outer_term, self.debug_client_tty
        )
    }
}

/// Compute the nearest 256-color code for an RGB color (for terminals
/// without truecolor but with a 256-color palette).
pub fn rgb_to_ansi256(r: u8, g: u8, b: u8) -> u8 {
    // Map near-gray tones onto the finer grayscale ramp (232-255); this
    // looks noticeably cleaner for a grayscale wallpaper than the 6x6x6
    // color cube would.
    let max = r.max(g).max(b) as i16;
    let min = r.min(g).min(b) as i16;
    if max - min < 10 {
        let gray = ((r as u16 + g as u16 + b as u16) / 3) as u8;
        return if gray < 8 {
            16
        } else if gray > 248 {
            231
        } else {
            232 + (((gray as u16 - 8) * 24) / 240) as u8
        };
    }
    let r6 = (r as u16 * 6 / 256) as u8;
    let g6 = (g as u16 * 6 / 256) as u8;
    let b6 = (b as u16 * 6 / 256) as u8;
    16 + 36 * r6 + 6 * g6 + b6
}
