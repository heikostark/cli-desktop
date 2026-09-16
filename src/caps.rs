//! Detects whether we're running on a raw Linux console (tty, framebuffer
//! console) or inside a "real" terminal emulator, and derives from that
//! which character sets and colors can sensibly be used.
//!
//! Reason: the Linux console (TERM=linux) generally has no color emoji font
//! and often only 16 colors, whereas terminal emulators such as Alacritty,
//! kitty, iTerm2, GNOME Terminal, Windows Terminal, etc. usually render
//! truecolor, full Unicode, and emoji cleanly.

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
}

impl Capabilities {
    pub fn detect() -> Self {
        let term = env::var("TERM").unwrap_or_default();
        let colorterm = env::var("COLORTERM").unwrap_or_default();
        let lang = env::var("LC_ALL")
            .or_else(|_| env::var("LC_CTYPE"))
            .or_else(|_| env::var("LANG"))
            .unwrap_or_default()
            .to_lowercase();

        // The Linux framebuffer console (real tty, no emulator) practically
        // always reports itself as TERM=linux (or "linux-16color" etc.).
        // GNU Hurd uses "hurd", some BSD consoles use "cons25".
        let is_console_tty = term == "linux"
            || term.starts_with("linux-")
            || term == "hurd"
            || term == "cons25";

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
        } else if term.contains("256color") {
            ColorSupport::Ansi256
        } else {
            ColorSupport::Basic16
        };

        Self { is_console_tty, unicode, emoji, color }
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
        format!("{} · {} · {} · {}", env_kind, color, charset, emoji)
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
