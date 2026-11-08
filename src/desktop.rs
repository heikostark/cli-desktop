use crate::ascii_art::{self, AsciiFrame};
use crate::catalog;
use crate::caps::{Capabilities, ColorSupport};
use crate::config::{DesktopConfig, Icon};
use crate::menu::{Menu, MenuAction, MenuPalette};
use crate::tmux::{self, TmuxWindow};
use anyhow::Result;
use crossterm::event::{KeyEvent, MouseEvent};
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
    /// The open right-click "add program" menu, if any. While it's open it
    /// receives all keyboard and mouse input.
    pub menu: Option<Menu>,
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
        let (wallpaper, wallpaper_warning) = load_wallpaper(
            wallpaper_path.as_deref().or(cfg.wallpaper.as_deref()),
            cols,
            rows.saturating_sub(2),
            &caps,
        );
        let windows = tmux::list_windows(&session).unwrap_or_default();
        // On the raw Linux console, xterm-style mouse reporting generally
        // doesn't work at all (it needs gpm with specific configuration,
        // which we have no way to set up or detect) — so default straight
        // into keyboard mode: pre-select the first icon, and lead with the
        // keyboard controls instead of the mouse-centric message.
        let (status, selected_icon) = if caps.is_console_tty {
            (
                "Mouse may not work on a console: Tab/arrows = select · Enter = open · \
                 a = add program · Shift+arrows = move · d = delete · q = quit"
                    .to_string(),
                if cfg.icons.is_empty() { None } else { Some(0) },
            )
        } else {
            (
                "Double-click = open · Right-click = add program · Drag = move · \
                 drag onto trash = delete · q = quit"
                    .to_string(),
                None,
            )
        };

        // A wallpaper that failed to load is easy to miss on stderr (which the
        // alternate screen hides), so surface it in the status line as well.
        let status = match wallpaper_warning {
            Some(w) => format!("{} {}", w, status),
            None => status,
        };

        let mut desktop = Self {
            cfg,
            caps,
            session,
            own_window,
            cols,
            rows,
            wallpaper,
            windows,
            selected_icon,
            dragging: None,
            last_click: None,
            status,
            pending: PendingConfirm::None,
            menu: None,
            taskbar_boxes: Vec::new(),
            trash_box: (0, 0, 0, 0),
        };

        // Icons from an older icons.json have no relative position yet:
        // derive it once from their absolute x/y, then persist it.
        if desktop.migrate_missing_rel() {
            let _ = desktop.cfg.save_icons();
        }
        desktop.apply_layout();
        Ok(desktop)
    }

    pub fn resize(&mut self, cols: u16, rows: u16) {
        self.cols = cols;
        self.rows = rows;
        let (wallpaper, warning) =
            load_wallpaper(self.cfg.wallpaper.as_deref(), cols, rows.saturating_sub(2), &self.caps);
        self.wallpaper = wallpaper;
        if let Some(w) = warning {
            self.status = w;
        }
        // Icon positions are stored relative to the window edges, so they
        // follow the new size instead of ending up off-screen.
        self.apply_layout();
        self.close_menu_if_too_small();
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
            draw_icon(out, icon, style, &palette, &self.caps)?;
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
            rel_x: None,
            rel_y: None,
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
        draw_icon(out, &trash_icon, trash_style, &palette, &self.caps)?;

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
            let end = start + label.width() as u16;
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

        // 6) The "add program" menu, on top of everything else.
        self.close_menu_if_too_small();
        if self.menu.is_some() {
            let pal = self.menu_palette();
            let (cols, rows, unicode) = (self.cols, self.rows, self.caps.unicode);
            if let Some(menu) = self.menu.as_mut() {
                menu.ensure_visible(cols, rows);
                menu.render(out, cols, rows, &pal, unicode)?;
            }
        }

        out.flush()?;
        Ok(())
    }

    // ---- Hit testing ----

    pub fn icon_at(&self, x: u16, y: u16) -> Option<usize> {
        self.cfg.icons.iter().position(|icon| hit_icon(icon, x, y, &self.caps))
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

    // ---- "Add program" menu ----

    fn menu_palette(&self) -> MenuPalette {
        let t = &self.cfg.theme;
        MenuPalette {
            fg: self.theme_color(&t.menu_fg),
            bg: self.theme_color(&t.menu_bg),
            border: self.theme_color(&t.menu_border_fg),
            sel_fg: self.theme_color(&t.menu_selected_fg),
            sel_bg: self.theme_color(&t.menu_selected_bg),
            dim: self.theme_color(&t.menu_dim_fg),
            ok: self.theme_color(&t.menu_ok_fg),
        }
    }

    /// Open the menu. `anchor` is where the popup's corner goes and
    /// `place_at` the desktop cell the new icon should get (both the
    /// right-click position); with `None` the popup is centered and the new
    /// icon goes to the first free spot (keyboard-opened menu).
    pub fn open_menu(&mut self, anchor: Option<(u16, u16)>, place_at: Option<(u16, u16)>) {
        let menu = Menu::new(anchor, place_at);
        if menu.geometry(self.cols, self.rows).is_none() {
            self.status = "Window too small for the program menu (needs at least 44x11).".into();
            return;
        }
        self.selected_icon = None;
        self.menu = Some(menu);
    }

    fn close_menu_if_too_small(&mut self) {
        let too_small = self
            .menu
            .as_ref()
            .map(|m| m.geometry(self.cols, self.rows).is_none())
            .unwrap_or(false);
        if too_small {
            self.menu = None;
            self.status = "Program menu closed: window too small.".into();
        }
    }

    pub fn menu_key(&mut self, k: KeyEvent) {
        let (cols, rows) = (self.cols, self.rows);
        let action = match self.menu.as_mut() {
            Some(m) => m.handle_key(k, cols, rows),
            None => return,
        };
        self.apply_menu_action(action);
    }

    pub fn menu_mouse(&mut self, m: MouseEvent) {
        let (cols, rows) = (self.cols, self.rows);
        let action = match self.menu.as_mut() {
            Some(menu) => menu.handle_mouse(m, cols, rows),
            None => return,
        };
        self.apply_menu_action(action);
    }

    fn apply_menu_action(&mut self, action: MenuAction) {
        match action {
            MenuAction::None => {}
            MenuAction::Close => self.menu = None,
            MenuAction::Add { cat, app } => {
                let place_at = self.menu.as_ref().and_then(|m| m.place_at);
                self.menu = None;
                self.add_app(cat, app, place_at);
            }
        }
    }

    /// Put a catalog program on the desktop as a new icon: at `place_at`
    /// if given (then snapped to the grid and kept clear of other icons,
    /// exactly like a drop after dragging), otherwise at the first free spot.
    fn add_app(&mut self, cat: usize, app: usize, place_at: Option<(u16, u16)>) {
        let category = &catalog::CATALOG[cat];
        let program = &category.apps[app];
        let command = catalog::command_for(category, program);

        // Already on the desktop? Select it instead of adding a duplicate.
        if let Some(i) = self
            .cfg
            .icons
            .iter()
            .position(|ic| ic.name == program.name && ic.command == command)
        {
            self.selected_icon = Some(i);
            self.status = format!("\"{}\" is already on the desktop.", program.name);
            return;
        }

        // Ids stay unique even across icons currently in the trash.
        let id = self
            .cfg
            .icons
            .iter()
            .chain(self.cfg.trash.iter())
            .map(|i| i.id)
            .max()
            .unwrap_or(0)
            + 1;
        self.cfg.icons.push(Icon {
            id,
            name: program.name.to_string(),
            // Stored as the category's emoji; terminals that can't show
            // emoji draw `[X]` instead (see `effective_glyph`).
            glyph: category.emoji.to_string(),
            x: 1,
            y: 1,
            rel_x: None,
            rel_y: None,
            command,
            singleton: false,
        });
        let idx = self.cfg.icons.len() - 1;
        let (x, y) = place_at.unwrap_or((1, 1));
        let (cx, cy) = self.clamp_icon_pos(idx, x as i32, y as i32);
        self.cfg.icons[idx].x = cx;
        self.cfg.icons[idx].y = cy;
        // Snaps to the grid, resolves collisions (with (1,1) as the start
        // this finds the first free spot), stores the relative position
        // and saves icons.json.
        self.snap_and_resolve(idx);
        self.selected_icon = Some(idx);

        let installed = catalog::is_installed(program.bin, &catalog::search_dirs());
        self.status = if installed {
            format!("\"{}\" added to the desktop.", program.name)
        } else {
            format!(
                "\"{}\" added, but it doesn't seem to be installed (the icon will say so).",
                program.name
            )
        };
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

    /// Move the currently selected icon by one grid step in the given
    /// direction (Shift+arrow keys), then snap/resolve collisions exactly
    /// like a mouse drag-and-drop would. This is the keyboard equivalent of
    /// dragging an icon, needed because mouse dragging generally doesn't
    /// work at all on the raw Linux console (no gpm / xterm mouse protocol
    /// support there).
    pub fn move_selected(&mut self, dx: i32, dy: i32) {
        let Some(idx) = self.selected_icon else {
            self.status = "No icon selected — press Tab first.".into();
            return;
        };
        let Some(icon) = self.cfg.icons.get(idx) else { return };
        let nx = icon.x as i32 + dx * GRID_W as i32;
        let ny = icon.y as i32 + dy * GRID_H as i32;
        let (cx, cy) = self.clamp_icon_pos(idx, nx, ny);
        if let Some(icon) = self.cfg.icons.get_mut(idx) {
            icon.x = cx;
            icon.y = cy;
        }
        self.snap_and_resolve(idx);
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

    /// Keyboard equivalent of right-clicking an icon: ask to delete the
    /// currently selected one (needed on the console, where there's no mouse).
    pub fn request_delete_selected(&mut self) {
        match self.selected_icon {
            Some(idx) => self.request_delete(idx),
            None => self.status = "No icon selected — press Tab first.".into(),
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
            self.migrate_missing_rel();
            self.apply_layout();
            let _ = self.cfg.save_icons();
            let _ = self.cfg.save_trash();
        } else {
            self.status = "The trash is empty.".into();
        }
    }

    /// Allowed range for an icon's top-left cell: `(min_x, max_x, min_y, max_y)`.
    /// The right edge depends on the icon's own width, so a wide label like
    /// "Processes" can never run past the last column (where the terminal
    /// would wrap it onto the next line); the bottom edge leaves room for
    /// the taskbar and status line.
    fn bounds_for(&self, icon: &Icon) -> (u16, u16, u16, u16) {
        let w = icon_rect(icon, &self.caps).2;
        let max_x = self.cols.saturating_sub(w);
        let max_y = self.rows.saturating_sub(5).max(1);
        (0, max_x, 1, max_y)
    }

    /// The trash icon's on-screen rectangle (x, y, width, height). Its width
    /// is `max(glyph, "Trash") + 2` = 7 in both the emoji and ASCII variants.
    fn trash_rect(&self) -> (u16, u16, u16, u16) {
        let (tx, ty) = self.trash_pos();
        (tx, ty, 7, 2)
    }

    pub fn clamp_icon_pos(&self, idx: usize, x: i32, y: i32) -> (u16, u16) {
        let Some(icon) = self.cfg.icons.get(idx) else { return (0, 1) };
        let (min_x, max_x, min_y, max_y) = self.bounds_for(icon);
        (
            x.clamp(min_x as i32, max_x as i32) as u16,
            y.clamp(min_y as i32, max_y as i32) as u16,
        )
    }

    /// Compute `rel_x`/`rel_y` for icon `idx` from its current cell position.
    fn store_rel(&mut self, idx: usize) {
        let Some(icon) = self.cfg.icons.get(idx) else { return };
        let (min_x, max_x, min_y, max_y) = self.bounds_for(icon);
        let frac = |v: u16, min: u16, max: u16| -> f32 {
            let span = max.saturating_sub(min);
            if span == 0 {
                return 0.0;
            }
            let f = (v.saturating_sub(min) as f32 / span as f32).clamp(0.0, 1.0);
            // Round so icons.json stays tidy (0.0455 rather than 0.045454543).
            (f * 10000.0).round() / 10000.0
        };
        let (rx, ry) = (frac(icon.x, min_x, max_x), frac(icon.y, min_y, max_y));
        let icon = &mut self.cfg.icons[idx];
        icon.rel_x = Some(rx);
        icon.rel_y = Some(ry);
    }

    /// Give every icon that has no relative position yet (older config
    /// files) one, derived from its absolute x/y. Returns whether anything changed.
    fn migrate_missing_rel(&mut self) -> bool {
        let mut changed = false;
        for idx in 0..self.cfg.icons.len() {
            let icon = &self.cfg.icons[idx];
            if icon.rel_x.is_none() || icon.rel_y.is_none() {
                self.store_rel(idx);
                changed = true;
            }
        }
        changed
    }

    /// Set icon `idx`'s cell position from its stored relative position
    /// for the current window size.
    fn place_from_rel(&mut self, idx: usize) {
        let Some(icon) = self.cfg.icons.get(idx) else { return };
        let (min_x, max_x, min_y, max_y) = self.bounds_for(icon);
        let rx = icon.rel_x.unwrap_or(0.0).clamp(0.0, 1.0);
        let ry = icon.rel_y.unwrap_or(0.0).clamp(0.0, 1.0);
        let x = min_x + (rx * max_x.saturating_sub(min_x) as f32).round() as u16;
        let y = min_y + (ry * max_y.saturating_sub(min_y) as f32).round() as u16;
        let icon = &mut self.cfg.icons[idx];
        icon.x = x;
        icon.y = y;
    }

    /// Recompute every icon's cell position from its relative position for
    /// the current window size, then keep them from piling up on each other.
    /// Called at startup and on every resize.
    pub fn apply_layout(&mut self) {
        for idx in 0..self.cfg.icons.len() {
            self.place_from_rel(idx);
        }
        self.resolve_overlaps();
    }

    /// Nudge icons that overlap an earlier icon or the trash out of the way.
    ///
    /// This only changes where icons are *drawn*, never their stored
    /// `rel_x`/`rel_y`: shrinking the window squeezes the proportional
    /// layout together (and could stack icons on top of each other, where
    /// they'd look like they vanished), but enlarging it again restores
    /// exactly the layout you set up. Bounded, so it can't spin forever
    /// when the window is simply too small to fit everything.
    fn resolve_overlaps(&mut self) {
        let trash = self.trash_rect();
        for i in 0..self.cfg.icons.len() {
            let (_, max_x, min_y, max_y) = self.bounds_for(&self.cfg.icons[i]);
            for _ in 0..500 {
                let rect = icon_rect(&self.cfg.icons[i], &self.caps);
                let collides = rects_overlap(rect, trash)
                    || (0..i).any(|j| rects_overlap(rect, icon_rect(&self.cfg.icons[j], &self.caps)));
                if !collides {
                    break;
                }
                let icon = &mut self.cfg.icons[i];
                if icon.y < max_y {
                    icon.y += 1;
                } else if icon.x + rect.2 < max_x {
                    icon.x += rect.2 + 1;
                    icon.y = min_y;
                } else {
                    break; // no free spot in a window this small
                }
            }
        }
    }

    /// After dragging (drag & drop), snap the icon's position to the grid
    /// and, if that makes it overlap another icon, look for a free spot
    /// nearby (collision avoidance). Then store the result as the icon's
    /// new relative position and save `icons.json`.
    pub fn snap_and_resolve(&mut self, idx: usize) {
        if idx >= self.cfg.icons.len() {
            return;
        }
        let (_, bx, _, by) = self.bounds_for(&self.cfg.icons[idx]);
        let max_x = bx.max(1);
        let max_y = by.max(1);
        let trash = self.trash_rect();

        // 1) Snap to the grid
        {
            let icon = &mut self.cfg.icons[idx];
            let gx = ((icon.x + GRID_W / 2) / GRID_W).saturating_mul(GRID_W);
            let gy = ((icon.y + GRID_H / 2) / GRID_H).saturating_mul(GRID_H);
            icon.x = gx.clamp(1, max_x);
            icon.y = gy.clamp(1, max_y);
        }

        // 2) Avoid colliding with other icons or the trash: first move down
        // within the column, then jump to the next column. Bounded search
        // (max 80 attempts) so this can never get stuck.
        for _ in 0..80 {
            let rect = icon_rect(&self.cfg.icons[idx], &self.caps);
            let collides = rects_overlap(rect, trash)
                || self
                    .cfg
                    .icons
                    .iter()
                    .enumerate()
                    .any(|(j, other)| j != idx && rects_overlap(rect, icon_rect(other, &self.caps)));
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

        self.store_rel(idx);
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

/// The glyph actually used for display and hit-testing, adapted to the
/// *current* run's capabilities.
///
/// `icons.json` stores one fixed glyph per icon (e.g. "🖥️"), which is only
/// chosen based on capabilities the first time the file is created. If the
/// same config is later reused on a different terminal that can't render
/// emoji (most notably the raw Linux console/tty, which typically has no
/// color-emoji font and is missing the glyph entirely), rendering that
/// stored glyph verbatim would draw something invisible/garbled — and,
/// worse, `unicode-width`'s assumed column width (e.g. 2 columns for many
/// emoji) would no longer match how many columns the terminal actually
/// advanced the cursor by, silently shifting every click hit-box out of
/// alignment. Both symptoms ("icon not visible" and "icon not clickable")
/// stem from this same mismatch. Falling back to a plain ASCII glyph
/// whenever the current environment can't do emoji — regardless of what's
/// stored in the config file — keeps what's drawn and what's clickable
/// consistent, and keeps the fallback human-readable (first letter of the
/// icon's name) instead of raw bytes.
fn effective_glyph<'a>(icon: &'a Icon, caps: &Capabilities) -> std::borrow::Cow<'a, str> {
    if caps.emoji || icon.glyph.is_ascii() {
        return std::borrow::Cow::Borrowed(&icon.glyph);
    }
    let letter = icon
        .name
        .chars()
        .find(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .unwrap_or('?');
    std::borrow::Cow::Owned(format!("[{}]", letter))
}

fn hit_icon(icon: &Icon, x: u16, y: u16, caps: &Capabilities) -> bool {
    // An icon occupies: the glyph line + the label line below it, width =
    // max(glyph, label) + 2 (unicode-width instead of char count, since
    // emoji often take up 2 terminal columns).
    let glyph = effective_glyph(icon, caps);
    let w = glyph.width().max(icon.name.width()) as u16 + 2;
    x >= icon.x && x < icon.x + w && y >= icon.y && y <= icon.y + 1
}

/// The rectangle an icon occupies on screen (x, y, width, height) – used
/// for collision detection during snap-to-grid.
fn icon_rect(icon: &Icon, caps: &Capabilities) -> (u16, u16, u16, u16) {
    let glyph = effective_glyph(icon, caps);
    let w = glyph.width().max(icon.name.width()) as u16 + 1;
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

fn draw_icon(out: &mut impl Write, icon: &Icon, style: IconStyle, palette: &IconPalette, caps: &Capabilities) -> Result<()> {
    let (fg, bg) = match style {
        IconStyle::Normal => (palette.normal_fg, None),
        IconStyle::Selected => (palette.selected_fg, Some(palette.selected_bg)),
        IconStyle::PendingDelete => (palette.danger_fg, Some(palette.danger_bg)),
        IconStyle::Full => (palette.full_fg, None),
    };
    let glyph = effective_glyph(icon, caps);

    queue!(out, cursor::MoveTo(icon.x, icon.y))?;
    queue!(out, SetForegroundColor(fg))?;
    if let Some(bg) = bg {
        queue!(out, SetBackgroundColor(bg))?;
    }
    out.write_all(glyph.as_bytes())?;
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

/// Load the wallpaper, falling back to the procedural pattern if it can't
/// be loaded. Returns the frame plus, if loading failed, a short warning.
///
/// The warning is returned instead of printed: this also runs on every
/// terminal resize, i.e. while the alternate screen and raw mode are
/// active, where a stray `eprintln!` would be drawn straight into the UI.
/// The caller decides where to show it (the status line).
fn load_wallpaper(path: Option<&str>, cols: u16, rows: u16, caps: &Capabilities) -> (AsciiFrame, Option<String>) {
    let unicode = caps.unicode;
    let colorize = caps.color != ColorSupport::Basic16;
    let mut warning = None;
    if let Some(p) = path {
        match ascii_art::from_image(p, cols, rows, unicode, colorize) {
            Ok(frame) => return (frame, None),
            Err(e) => {
                warning = Some(format!("Wallpaper not loaded ({}) - using fallback pattern.", e));
            }
        }
    }
    (ascii_art::procedural(cols, rows, unicode), warning)
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
