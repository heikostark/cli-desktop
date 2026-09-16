use crate::ascii_art::{self, AsciiFrame};
use crate::caps::{Capabilities, ColorSupport};
use crate::config::{DesktopConfig, Icon};
use crate::tmux::{self, TmuxWindow};
use anyhow::Result;
use crossterm::style::{Color, ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::{cursor, queue, terminal};
use std::io::Write;
use std::time::Instant;
use unicode_width::UnicodeWidthStr;

const TRASH_LABEL: &str = "Trash";
const TRASH_EMOJI: &str = "🗑️";
const TRASH_ASCII_EMPTY: &str = "[ ]";
const TRASH_ASCII_FULL: &str = "[#]";

/// Grid size for snapping icons into place after being moved (snap-to-grid),
/// so icons don't overlap each other and clicks don't hit the wrong icon.
const GRID_W: u16 = 10;
const GRID_H: u16 = 4;

/// What the user is currently being asked to confirm with 'j'/'n' (if anything).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PendingConfirm {
    None,
    DeleteIcon(usize),
    EmptyTrash,
}

pub struct Desktop {
    pub cfg: DesktopConfig,
    pub caps: Capabilities,
    pub session: String,
    pub own_window: Option<u32>,
    pub cols: u16,
    pub rows: u16,
    pub wallpaper: AsciiFrame,
    pub windows: Vec<TmuxWindow>,
    pub selected_icon: Option<usize>,
    pub dragging: Option<DragState>,
    pub last_click: Option<(usize, Instant)>,
    pub status: String,
    /// Action currently awaiting 'j'/'n' confirmation in the status line.
    /// Only used for "risky" quick actions (right-click delete, emptying
    /// the trash) – deliberately dragging an icon onto the trash doesn't
    /// ask for extra confirmation.
    pub pending: PendingConfirm,
    // Hit boxes are recomputed on every render so clicks land exactly where
    // things were actually drawn.
    pub taskbar_boxes: Vec<(u16, u16, u32)>, // (x_start, x_end, window_index)
    pub trash_box: (u16, u16, u16, u16),     // (x, y, w, h)
}

pub struct DragState {
    pub icon_idx: usize,
    pub offset_x: i32,
    pub offset_y: i32,
}

impl Desktop {
    pub fn new(
        cfg: DesktopConfig,
        cols: u16,
        rows: u16,
        wallpaper_path: Option<String>,
        caps: Capabilities,
    ) -> Result<Self> {
        let session = tmux::current_session().unwrap_or_else(|_| "?".into());
        let own_window = tmux::own_window_index();
        let wallpaper = load_wallpaper(
            wallpaper_path.as_deref().or(cfg.wallpaper.as_deref()),
            cols,
            rows.saturating_sub(2),
            &caps,
        );
        let windows = tmux::list_windows(&session).unwrap_or_default();
        let status = format!(
            "{} · 'i' = system info · q = quit",
            "Double-click = open · Drag = move · drag onto trash = delete"
        );

        Ok(Self {
            cfg,
            caps,
            session,
            own_window,
            cols,
            rows,
            wallpaper,
            windows,
            selected_icon: None,
            dragging: None,
            last_click: None,
            status,
            pending: PendingConfirm::None,
            taskbar_boxes: Vec::new(),
            trash_box: (0, 0, 0, 0),
        })
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        self.wallpaper = load_wallpaper(self.cfg.wallpaper.as_deref(), cols, rows.saturating_sub(2), &self.caps);
    }

    pub fn show_system_info(&mut self) {
        self.status = format!("Detected environment: {}", self.caps.summary());
    }

    pub fn refresh_windows(&mut self) {
        if let Ok(w) = tmux::list_windows(&self.session) {
            self.windows = w;
        }
    }

    fn trash_pos(&self) -> (u16, u16) {
        let x = self.cols.saturating_sub(14);
        let y = self.rows.saturating_sub(6);
        (x, y)
    }

    /// Convert a color scheme value (`colors.json`) into a `Color` this
    /// terminal can actually display (hex colors are automatically
    /// downgraded to a safe basic color on tty consoles).
    fn theme_color(&self, key: &str) -> Color {
        theme_color(key, self.caps.color)
    }

    /// Fully clear the screen once. Not called on every `render()` (that
    /// caused visible flicker, e.g. while dragging icons), but only at
    /// startup, on resize, and periodically as self-healing (see
    /// `FULL_REDRAW_INTERVAL` in main.rs), since `render()` rewrites every
    /// cell of the screen anyway.
    pub fn clear(&self, out: &mut impl Write) -> Result<()> {
        queue!(out, terminal::Clear(terminal::ClearType::All))?;
        out.flush()?;
        Ok(())
    }

    pub fn render(&mut self, out: &mut impl Write) -> Result<()> {
        let wallpaper_fallback = self.theme_color(&self.cfg.theme.wallpaper_fallback_fg);

        // 1) Wallpaper (ASCII art). Monochrome without color support (tty),
        // in the image's original colors with 256 colors/truecolor.
        for (y, row) in self.wallpaper.rows.iter().enumerate() {
            queue!(out, cursor::MoveTo(0, y as u16))?;
            let mut buf = [0u8; 4];
            let mut last_color: Option<Color> = None;
            for cell in row {
                let color = match cell.rgb {
                    Some((r, g, b)) => Some(terminal_color(r, g, b, self.caps.color)),
                    None => None,
                };
                if color != last_color {
                    match color {
                        Some(c) => queue!(out, SetForegroundColor(c))?,
                        None => queue!(out, SetForegroundColor(wallpaper_fallback))?,
                    }
                    last_color = color;
                }
                out.write_all(cell.ch.encode_utf8(&mut buf).as_bytes())?;
            }
        }
        queue!(out, ResetColor)?;

        // 1b) Show the clock in the top right corner (overwrites a few
        // wallpaper characters there, but noticeably improves the desktop)
        let clock = chrono::Local::now().format("%H:%M:%S").to_string();
        let clock_x = self.cols.saturating_sub(clock.width() as u16 + 1);
        queue!(out, cursor::MoveTo(clock_x, 0))?;
        queue!(
            out,
            SetForegroundColor(self.theme_color(&self.cfg.theme.clock_fg)),
            SetBackgroundColor(self.theme_color(&self.cfg.theme.clock_bg))
        )?;
        out.write_all(format!(" {} ", clock).as_bytes())?;
        queue!(out, ResetColor)?;

        // 2) Desktop icons
        let palette = IconPalette {
            normal_fg: self.theme_color(&self.cfg.theme.icon_fg),
            label_fg: self.theme_color(&self.cfg.theme.label_fg),
            selected_fg: self.theme_color(&self.cfg.theme.selected_fg),
            selected_bg: self.theme_color(&self.cfg.theme.selected_bg),
            danger_fg: self.theme_color(&self.cfg.theme.danger_fg),
            danger_bg: self.theme_color(&self.cfg.theme.danger_bg),
            full_fg: self.theme_color(&self.cfg.theme.trash_full_fg),
        };
        for (i, icon) in self.cfg.icons.iter().enumerate() {
            let style = match self.pending {
                PendingConfirm::DeleteIcon(pi) if pi == i => IconStyle::PendingDelete,
                _ if self.selected_icon == Some(i) => IconStyle::Selected,
                _ => IconStyle::Normal,
            };
            draw_icon(out, icon, style, &palette)?;
        }

        // 3) Trash (emoji if available, otherwise ASCII brackets; a
        // non-empty trash is additionally highlighted with a color)
        let (tx, ty) = self.trash_pos();
        let trash_glyph = if self.caps.emoji {
            TRASH_EMOJI.to_string()
        } else if self.cfg.trash.is_empty() {
            TRASH_ASCII_EMPTY.to_string()
        } else {
            TRASH_ASCII_FULL.to_string()
        };
        let trash_w = trash_glyph.width().max(TRASH_LABEL.width()) as u16 + 2;
        self.trash_box = (tx, ty, trash_w, 2);
        let trash_icon = Icon {
            id: 0,
            name: TRASH_LABEL.into(),
            glyph: trash_glyph,
            x: tx,
            y: ty,
            command: String::new(),
            singleton: false,
        };
        let trash_style = if self.pending == PendingConfirm::EmptyTrash {
            IconStyle::PendingDelete
        } else if self.cfg.trash.is_empty() {
            IconStyle::Normal
        } else {
            IconStyle::Full
        };
        draw_icon(out, &trash_icon, trash_style, &palette)?;

        // 4) Taskbar at the bottom
        self.taskbar_boxes.clear();
        let bar_y = self.rows.saturating_sub(2);
        let taskbar_fg = self.theme_color(&self.cfg.theme.taskbar_fg);
        let taskbar_bg = self.theme_color(&self.cfg.theme.taskbar_bg);
        let taskbar_active_fg = self.theme_color(&self.cfg.theme.taskbar_active_fg);
        let taskbar_active_bg = self.theme_color(&self.cfg.theme.taskbar_active_bg);

        queue!(out, cursor::MoveTo(0, bar_y))?;
        queue!(out, SetBackgroundColor(taskbar_bg), SetForegroundColor(taskbar_fg))?;
        let bar = " ".repeat(self.cols as usize);
        out.write_all(bar.as_bytes())?;
        queue!(out, cursor::MoveTo(0, bar_y))?;

        let mut cursor_x: u16 = 1;
        for w in &self.windows {
            let label = format!(" {}:{} ", w.index, w.name);
            let start = cursor_x;
            let end = start + label.len() as u16;
            if end >= self.cols {
                break;
            }
            queue!(out, cursor::MoveTo(start, bar_y))?;
            if w.active {
                queue!(out, SetBackgroundColor(taskbar_active_bg), SetForegroundColor(taskbar_active_fg))?;
            } else {
                queue!(out, SetBackgroundColor(taskbar_bg), SetForegroundColor(taskbar_fg))?;
            }
            out.write_all(label.as_bytes())?;
            self.taskbar_boxes.push((start, end, w.index));
            cursor_x = end;
        }
        queue!(out, ResetColor)?;

        // 5) Status line at the very bottom. If a confirmation is pending,
        // an eye-catching prompt is shown here instead of the normal status message.
        queue!(out, cursor::MoveTo(0, self.rows.saturating_sub(1)))?;
        let danger_fg = self.theme_color(&self.cfg.theme.danger_fg);
        let status_fg = self.theme_color(&self.cfg.theme.status_fg);
        let (status_text, status_color) = match self.pending {
            PendingConfirm::DeleteIcon(idx) => {
                let name = self.cfg.icons.get(idx).map(|i| i.name.as_str()).unwrap_or("icon");
                (
                    format!("Really delete \"{}\"? j = yes, n/Esc = cancel", name),
                    danger_fg,
                )
            }
            PendingConfirm::EmptyTrash => (
                format!(
                    "Really empty the trash permanently ({} item(s))? j = yes, n/Esc = cancel",
                    self.cfg.trash.len()
                ),
                danger_fg,
            ),
            PendingConfirm::None => (self.status.clone(), status_fg),
        };
        queue!(out, SetForegroundColor(status_color))?;
        let mut status = truncate_to_width(&status_text, self.cols);
        pad_to_width(&mut status, self.cols);
        out.write_all(status.as_bytes())?;
        queue!(out, ResetColor)?;

        out.flush()?;
        Ok(())
    }

    // ---- Hit testing ----

    pub fn icon_at(&self, x: u16, y: u16) -> Option<usize> {
        self.cfg.icons.iter().position(|icon| hit_icon(icon, x, y))
    }

    pub fn trash_hit(&self, x: u16, y: u16) -> bool {
        let (tx, ty, w, h) = self.trash_box;
        x >= tx && x < tx + w && y >= ty && y < ty + h
    }

    pub fn taskbar_hit(&self, x: u16, y: u16) -> Option<u32> {
        let bar_y = self.rows.saturating_sub(2);
        if y != bar_y {
            return None;
        }
        self.taskbar_boxes
            .iter()
            .find(|(s, e, _)| x >= *s && x < *e)
            .map(|(_, _, idx)| *idx)
    }

    // ---- Keyboard navigation ----

    /// Select the next icon (Tab / arrow down/right), wrapping at the end.
    pub fn select_next(&mut self) {
        if self.cfg.icons.is_empty() {
            return;
        }
        let next = match self.selected_icon {
            Some(i) => (i + 1) % self.cfg.icons.len(),
            None => 0,
        };
        self.selected_icon = Some(next);
    }

    /// Select the previous icon (Shift+Tab / arrow up/left), wrapping at the start.
    pub fn select_prev(&mut self) {
        if self.cfg.icons.is_empty() {
            return;
        }
        let prev = match self.selected_icon {
            Some(0) | None => self.cfg.icons.len() - 1,
            Some(i) => i - 1,
        };
        self.selected_icon = Some(prev);
    }

    /// Open the currently selected icon (Enter).
    pub fn open_selected(&mut self) {
        if let Some(idx) = self.selected_icon {
            self.launch_icon(idx);
        }
    }

    // ---- Confirmation dialogs ----

    /// Whether a confirmation ('j'/'n') is currently pending. Key presses
    /// should be interpreted exclusively for that purpose while this is true.
    pub fn has_pending(&self) -> bool {
        self.pending != PendingConfirm::None
    }

    /// Trigger deleting an icon: instead of removing it immediately, a
    /// confirmation is shown in the status line first (via 'j'/'n' on the keyboard).
    pub fn request_delete(&mut self, idx: usize) {
        if idx < self.cfg.icons.len() {
            self.pending = PendingConfirm::DeleteIcon(idx);
        }
    }

    /// Trigger emptying the trash (confirmation required since it's permanent).
    /// If the trash is already empty, a message is shown immediately instead.
    pub fn request_empty_trash(&mut self) {
        if self.cfg.trash.is_empty() {
            self.status = "The trash is already empty.".into();
        } else {
            self.pending = PendingConfirm::EmptyTrash;
        }
    }

    pub fn confirm_pending(&mut self) {
        match std::mem::replace(&mut self.pending, PendingConfirm::None) {
            PendingConfirm::DeleteIcon(idx) => self.move_icon_to_trash(idx),
            PendingConfirm::EmptyTrash => self.empty_trash(),
            PendingConfirm::None => {}
        }
    }

    pub fn cancel_pending(&mut self) {
        if self.pending != PendingConfirm::None {
            self.pending = PendingConfirm::None;
            self.status = "Cancelled.".into();
        }
    }

    // ---- Actions ----

    /// Open an icon. If it's marked "singleton" and a tmux window with the
    /// same name already exists, switch to it instead of opening another one.
    pub fn launch_icon(&mut self, idx: usize) {
        let icon = match self.cfg.icons.get(idx) {
            Some(icon) => icon.clone(),
            None => return,
        };

        if icon.singleton {
            self.refresh_windows();
            if let Some(existing) = self.windows.iter().find(|w| w.name == icon.name) {
                let index = existing.index;
                self.select_taskbar(index);
                self.status = format!("\"{}\" is already open, switched to it.", icon.name);
                return;
            }
        }

        match tmux::new_window(&self.session, &icon.name, &icon.command) {
            Ok(_) => {
                self.status = format!("\"{}\" opened.", icon.name);
                self.refresh_windows();
            }
            Err(e) => self.status = format!("Error opening {}: {}", icon.name, e),
        }
    }

    pub fn select_taskbar(&mut self, index: u32) {
        match tmux::select_window(&self.session, index) {
            Ok(_) => self.status = format!("Switched to window {}.", index),
            Err(e) => self.status = format!("Error switching windows: {}", e),
        }
        self.refresh_windows();
    }

    pub fn close_taskbar(&mut self, index: u32) {
        if Some(index) == self.own_window {
            self.status = "The desktop itself can't be closed.".into();
            return;
        }
        match tmux::kill_window(&self.session, index) {
            Ok(_) => self.status = format!("Window {} closed.", index),
            Err(e) => self.status = format!("Error closing window: {}", e),
        }
        self.refresh_windows();
    }

    pub fn move_icon_to_trash(&mut self, idx: usize) {
        if idx < self.cfg.icons.len() {
            let icon = self.cfg.icons.remove(idx);
            self.status = format!("\"{}\" moved to the trash.", icon.name);
            self.cfg.trash.push(icon);
            let _ = self.cfg.save_icons();
            let _ = self.cfg.save_trash();
            self.selected_icon = None;
        }
    }

    pub fn empty_trash(&mut self) {
        let n = self.cfg.trash.len();
        self.cfg.trash.clear();
        let _ = self.cfg.save_trash();
        self.status = format!("Trash emptied ({} item(s) permanently deleted).", n);
    }

    pub fn restore_last_trash(&mut self) {
        if let Some(icon) = self.cfg.trash.pop() {
            self.status = format!("\"{}\" restored.", icon.name);
            self.cfg.icons.push(icon);
            let _ = self.cfg.save_icons();
            let _ = self.cfg.save_trash();
        } else {
            self.status = "The trash is empty.".into();
        }
    }

    pub fn clamp_icon_pos(&self, x: i32, y: i32) -> (u16, u16) {
        let max_x = self.cols.saturating_sub(6) as i32;
        let max_y = self.rows.saturating_sub(5) as i32;
        (x.clamp(0, max_x.max(0)) as u16, y.clamp(1, max_y.max(1)) as u16)
    }

    /// After dragging (drag & drop), snap the icon's position to the grid
    /// and, if that makes it overlap another icon, look for a free spot
    /// nearby (collision avoidance). Saves `icons.json` afterwards.
    pub fn snap_and_resolve(&mut self, idx: usize) {
        if idx >= self.cfg.icons.len() {
            return;
        }
        let max_x = self.cols.saturating_sub(6).max(1);
        let max_y = self.rows.saturating_sub(5).max(1);

        // 1) Snap to the grid
        {
            let icon = &mut self.cfg.icons[idx];
            let gx = ((icon.x + GRID_W / 2) / GRID_W).saturating_mul(GRID_W);
            let gy = ((icon.y + GRID_H / 2) / GRID_H).saturating_mul(GRID_H);
            icon.x = gx.clamp(1, max_x);
            icon.y = gy.clamp(1, max_y);
        }

        // 2) Avoid colliding with other icons: first move down within the
        // column, then jump to the next column. Bounded search (max 80
        // attempts) so this can never get stuck.
        for _ in 0..80 {
            let rect = icon_rect(&self.cfg.icons[idx]);
            let collides = self
                .cfg
                .icons
                .iter()
                .enumerate()
                .any(|(j, other)| j != idx && rects_overlap(rect, icon_rect(other)));
            if !collides {
                break;
            }
            let icon = &mut self.cfg.icons[idx];
            icon.y += GRID_H;
            if icon.y > max_y {
                icon.y = 1;
                icon.x += GRID_W;
                if icon.x > max_x {
                    icon.x = 1;
                }
            }
        }

        let _ = self.cfg.save_icons();
    }

    /// Save icon positions without snapping/collision resolution
    /// (currently unused since `snap_and_resolve` handles this on drop;
    /// kept around as a simple alternative for future callers).
    #[allow(dead_code)]
    pub fn persist(&self) {
        let _ = self.cfg.save_icons();
    }
}

fn hit_icon(icon: &Icon, x: u16, y: u16) -> bool {
    // An icon occupies: the glyph line + the label line below it, width =
    // max(glyph, label) + 2 (unicode-width instead of char count, since
    // emoji often take up 2 terminal columns).
    let w = icon.glyph.width().max(icon.name.width()) as u16 + 2;
    x >= icon.x && x < icon.x + w && y >= icon.y && y <= icon.y + 1
}

/// The rectangle an icon occupies on screen (x, y, width, height) – used
/// for collision detection during snap-to-grid.
fn icon_rect(icon: &Icon) -> (u16, u16, u16, u16) {
    let w = icon.glyph.width().max(icon.name.width()) as u16 + 1;
    (icon.x, icon.y, w.max(3), 2)
}

fn rects_overlap(a: (u16, u16, u16, u16), b: (u16, u16, u16, u16)) -> bool {
    let (ax, ay, aw, ah) = a;
    let (bx, by, bw, bh) = b;
    ax < bx + bw && bx < ax + aw && ay < by + bh && by < ay + ah
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum IconStyle {
    Normal,
    Selected,
    /// Proposed for deletion, awaiting confirmation ('j'/'n').
    PendingDelete,
    /// The trash is not empty (only relevant for the trash icon).
    Full,
}

/// Colors pre-resolved from the theme, so `draw_icon` doesn't have to
/// parse/look up strings again for every icon.
struct IconPalette {
    normal_fg: Color,
    label_fg: Color,
    selected_fg: Color,
    selected_bg: Color,
    danger_fg: Color,
    danger_bg: Color,
    full_fg: Color,
}

fn draw_icon(out: &mut impl Write, icon: &Icon, style: IconStyle, palette: &IconPalette) -> Result<()> {
    let (fg, bg) = match style {
        IconStyle::Normal => (palette.normal_fg, None),
        IconStyle::Selected => (palette.selected_fg, Some(palette.selected_bg)),
        IconStyle::PendingDelete => (palette.danger_fg, Some(palette.danger_bg)),
        IconStyle::Full => (palette.full_fg, None),
    };

    queue!(out, cursor::MoveTo(icon.x, icon.y))?;
    queue!(out, SetForegroundColor(fg))?;
    if let Some(bg) = bg {
        queue!(out, SetBackgroundColor(bg))?;
    }
    out.write_all(icon.glyph.as_bytes())?;
    queue!(out, ResetColor)?;

    let label_fg = match style {
        IconStyle::Selected | IconStyle::PendingDelete => fg,
        _ => palette.label_fg,
    };
    queue!(out, cursor::MoveTo(icon.x, icon.y + 1))?;
    queue!(out, SetForegroundColor(label_fg))?;
    if let Some(bg) = bg {
        queue!(out, SetBackgroundColor(bg))?;
    }
    out.write_all(icon.name.as_bytes())?;
    queue!(out, ResetColor)?;
    Ok(())
}

/// Pad a string with spaces up to `width` terminal columns, so that
/// redrawing never leaves remnants of a previous, longer line behind
/// (relevant because we deliberately no longer clear the whole screen every frame).
fn pad_to_width(s: &mut String, width: u16) {
    let current: u16 = s.chars().map(|c| c.to_string().width() as u16).sum();
    if current < width {
        s.push_str(&" ".repeat((width - current) as usize));
    }
}

/// Safely truncate a string to `max_width` terminal columns without cutting
/// in the middle of a multi-byte character (e.g. accented letters, emoji).
fn truncate_to_width(s: &str, max_width: u16) -> String {
    let mut out = String::new();
    let mut used = 0u16;
    for ch in s.chars() {
        let w = ch.to_string().width() as u16;
        if used + w > max_width {
            break;
        }
        out.push(ch);
        used += w;
    }
    out
}

fn load_wallpaper(path: Option<&str>, cols: u16, rows: u16, caps: &Capabilities) -> AsciiFrame {
    let unicode = caps.unicode;
    let colorize = caps.color != ColorSupport::Basic16;
    if let Some(p) = path {
        match ascii_art::from_image(p, cols, rows, unicode, colorize) {
            Ok(frame) => return frame,
            Err(e) => {
                eprintln!("Warning: could not load the wallpaper ({}), using the fallback pattern instead.", e);
            }
        }
    }
    ascii_art::procedural(cols, rows, unicode)
}

/// Convert the image's original RGB color into a color this terminal can
/// actually display (truecolor directly, otherwise rounded to the
/// 256-color palette).
fn terminal_color(r: u8, g: u8, b: u8, support: ColorSupport) -> Color {
    match support {
        ColorSupport::TrueColor => Color::Rgb { r, g, b },
        ColorSupport::Ansi256 => Color::AnsiValue(crate::caps::rgb_to_ansi256(r, g, b)),
        ColorSupport::Basic16 => Color::DarkGrey,
    }
}

/// Convert a theme color value (from `colors.json`, e.g. "yellow" or
/// "#ff8800") into a `Color` this terminal can actually display. Hex colors
/// are automatically downgraded to white on a plain tty console (only 16
/// colors), and rounded to the nearest palette color on 256-color terminals.
fn theme_color(value: &str, support: ColorSupport) -> Color {
    let s = value.trim().to_lowercase();
    if let Some(hex) = s.strip_prefix('#') {
        if hex.len() == 6 {
            if let Ok(v) = u32::from_str_radix(hex, 16) {
                let r = ((v >> 16) & 0xFF) as u8;
                let g = ((v >> 8) & 0xFF) as u8;
                let b = (v & 0xFF) as u8;
                return match support {
                    ColorSupport::TrueColor => Color::Rgb { r, g, b },
                    ColorSupport::Ansi256 => Color::AnsiValue(crate::caps::rgb_to_ansi256(r, g, b)),
                    ColorSupport::Basic16 => Color::White,
                };
            }
        }
        // Invalid hex format -> safe fallback
        return Color::White;
    }
    match s.as_str() {
        "black" => Color::Black,
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::White,
        "grey" | "gray" => Color::Grey,
        "darkgrey" | "darkgray" => Color::DarkGrey,
        "darkred" => Color::DarkRed,
        "darkgreen" => Color::DarkGreen,
        "darkyellow" => Color::DarkYellow,
        "darkblue" => Color::DarkBlue,
        "darkmagenta" => Color::DarkMagenta,
        "darkcyan" => Color::DarkCyan,
        _ => Color::White,
    }
}
