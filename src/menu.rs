//! The right-click "add program" menu: a two-pane popup (categories on the
//! left, programs on the right) listing the catalog in `catalog.rs`.
//!
//! Operation:
//! - Mouse: click a category to show its programs, click a program to add
//!   it as an icon, scroll wheel to scroll, click outside (or right-click)
//!   to close.
//! - Keyboard (needed on the console, where there's no mouse): `Up`/`Down`
//!   move, `Tab`/`Left`/`Right` switch pane, `Enter` adds, `Esc` closes.
//!   Typing filters *all* programs by name, command, description or
//!   category; `Backspace` edits the filter and `Esc` clears it.
//!
//! The menu is pure state + drawing; adding the actual icon is up to the
//! caller (it gets a `MenuAction::Add`).

use crate::catalog::{self, CATALOG};
use anyhow::Result;
use crossterm::cursor;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use crossterm::queue;
use crossterm::style::{Color, ResetColor, SetBackgroundColor, SetForegroundColor};
use std::io::Write;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Focus {
    Categories,
    Items,
}

pub struct MenuPalette {
    pub fg: Color,
    pub bg: Color,
    pub border: Color,
    pub sel_fg: Color,
    pub sel_bg: Color,
    pub dim: Color,
    pub ok: Color,
}

/// Screen rectangle of the popup (`x`, `y`, `w`, `h`) and the width of the
/// category pane.
#[derive(Clone, Copy, Debug)]
pub struct Geo {
    pub x: u16,
    pub y: u16,
    pub w: u16,
    pub h: u16,
    pub left_w: u16,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MenuAction {
    None,
    Close,
    Add { cat: usize, app: usize },
}

enum Hit {
    Outside,
    Inside,
    Category(usize),
    Item(usize),
}

pub struct Menu {
    /// Desktop cell the new icon should be placed at (the right-click
    /// position); `None` = automatically pick a free spot.
    pub place_at: Option<(u16, u16)>,
    /// Requested top-left of the popup (the right-click position);
    /// `None` = centered.
    anchor: Option<(u16, u16)>,
    cat: usize,
    cat_scroll: usize,
    item: usize,
    scroll: usize,
    focus: Focus,
    filter: String,
    installed: Vec<Vec<bool>>,
}

/// Number of list rows inside the popup: everything except the top border,
/// the header row, the filter row and the bottom border.
fn list_rows(geo: &Geo) -> usize {
    geo.h as usize - 4
}

/// Truncate/pad `s` to exactly `w` display columns.
fn fit(s: &str, w: usize) -> String {
    let mut out = String::new();
    let mut used = 0;
    for ch in s.chars() {
        let cw = ch.width().unwrap_or(0);
        if used + cw > w {
            break;
        }
        out.push(ch);
        used += cw;
    }
    out.push_str(&" ".repeat(w - used));
    out
}

fn put(out: &mut impl Write, x: u16, y: u16, text: &str, fg: Color, bg: Color) -> Result<()> {
    queue!(out, cursor::MoveTo(x, y), SetForegroundColor(fg), SetBackgroundColor(bg))?;
    out.write_all(text.as_bytes())?;
    Ok(())
}

impl Menu {
    pub fn new(anchor: Option<(u16, u16)>, place_at: Option<(u16, u16)>) -> Self {
        Self {
            place_at,
            anchor,
            cat: 0,
            cat_scroll: 0,
            item: 0,
            scroll: 0,
            focus: Focus::Categories,
            filter: String::new(),
            installed: catalog::installed_flags(),
        }
    }

    /// Where the popup goes for a `cols` x `rows` screen, or `None` if the
    /// screen is too small to show it usefully. The popup stays above the
    /// taskbar and status line (the bottom two rows).
    pub fn geometry(&self, cols: u16, rows: u16) -> Option<Geo> {
        let avail_h = rows.saturating_sub(2);
        let w = cols.min(92);
        let h = avail_h.min(26);
        if w < 44 || h < 9 {
            return None;
        }
        let (x, y) = match self.anchor {
            Some((ax, ay)) => (ax.min(cols - w), ay.min(avail_h - h)),
            None => ((cols - w) / 2, (avail_h - h) / 2),
        };
        let widest = CATALOG.iter().map(|c| c.name.width()).max().unwrap_or(10) as u16;
        let left_w = (widest + 3).min(w / 3);
        Some(Geo { x, y, w, h, left_w })
    }

    /// The programs currently listed on the right as `(category, app)` indices.
    pub fn visible(&self) -> Vec<(usize, usize)> {
        if self.filter.is_empty() {
            return (0..CATALOG[self.cat].apps.len()).map(|a| (self.cat, a)).collect();
        }
        let q = self.filter.to_lowercase();
        let mut v = Vec::new();
        for (c, cat) in CATALOG.iter().enumerate() {
            let cat_hit = cat.name.to_lowercase().contains(&q);
            for (a, app) in cat.apps.iter().enumerate() {
                if cat_hit
                    || app.name.to_lowercase().contains(&q)
                    || app.bin.to_lowercase().contains(&q)
                    || app.desc.to_lowercase().contains(&q)
                {
                    v.push((c, a));
                }
            }
        }
        v
    }

    fn reset_items(&mut self) {
        self.item = 0;
        self.scroll = 0;
    }

    /// Keep the selected category/program inside the visible rows.
    pub fn ensure_visible(&mut self, cols: u16, rows: u16) {
        let Some(geo) = self.geometry(cols, rows) else { return };
        let n = list_rows(&geo);
        let len = self.visible().len();
        self.item = self.item.min(len.saturating_sub(1));
        if self.item < self.scroll {
            self.scroll = self.item;
        } else if self.item >= self.scroll + n {
            self.scroll = self.item + 1 - n;
        }
        self.scroll = self.scroll.min(len.saturating_sub(n));

        if self.cat < self.cat_scroll {
            self.cat_scroll = self.cat;
        } else if self.cat >= self.cat_scroll + n {
            self.cat_scroll = self.cat + 1 - n;
        }
        self.cat_scroll = self.cat_scroll.min(CATALOG.len().saturating_sub(n));
    }

    fn step(&mut self, delta: i64) {
        if self.focus == Focus::Categories && self.filter.is_empty() {
            let n = CATALOG.len() as i64;
            self.cat = (self.cat as i64 + delta).clamp(0, n - 1) as usize;
            self.reset_items();
        } else {
            self.step_items(delta);
        }
    }

    fn step_items(&mut self, delta: i64) {
        let n = self.visible().len() as i64;
        self.item = if n == 0 { 0 } else { (self.item as i64 + delta).clamp(0, n - 1) as usize };
    }

    pub fn handle_key(&mut self, k: KeyEvent, cols: u16, rows: u16) -> MenuAction {
        let Some(geo) = self.geometry(cols, rows) else { return MenuAction::Close };
        let page = list_rows(&geo) as i64;
        let ctrl = k.modifiers.contains(KeyModifiers::CONTROL);
        let alt = k.modifiers.contains(KeyModifiers::ALT);
        let filtering = !self.filter.is_empty();
        match k.code {
            KeyCode::Char('c') if ctrl => return MenuAction::Close,
            KeyCode::Esc => {
                if !filtering {
                    return MenuAction::Close;
                }
                self.filter.clear();
                self.reset_items();
            }
            KeyCode::Enter => {
                if self.focus == Focus::Categories && !filtering {
                    self.focus = Focus::Items;
                } else if let Some(&(cat, app)) = self.visible().get(self.item) {
                    return MenuAction::Add { cat, app };
                }
            }
            KeyCode::Up => self.step(-1),
            KeyCode::Down => self.step(1),
            KeyCode::PageUp => self.step(-page),
            KeyCode::PageDown => self.step(page),
            KeyCode::Home => self.step(-1_000_000),
            KeyCode::End => self.step(1_000_000),
            KeyCode::Tab | KeyCode::BackTab if !filtering => {
                self.focus = match self.focus {
                    Focus::Categories => Focus::Items,
                    Focus::Items => Focus::Categories,
                };
            }
            KeyCode::Left if !filtering => self.focus = Focus::Categories,
            KeyCode::Right if !filtering => self.focus = Focus::Items,
            KeyCode::Backspace => {
                self.filter.pop();
                self.reset_items();
            }
            KeyCode::Char(ch) if !ctrl && !alt => {
                self.filter.push(ch);
                self.reset_items();
                self.focus = Focus::Items;
            }
            _ => {}
        }
        self.ensure_visible(cols, rows);
        MenuAction::None
    }

    fn hit(&self, x: u16, y: u16, geo: &Geo) -> Hit {
        if x < geo.x || x >= geo.x + geo.w || y < geo.y || y >= geo.y + geo.h {
            return Hit::Outside;
        }
        let (rx, ry) = (x - geo.x, y - geo.y);
        let rows = list_rows(geo) as u16;
        if ry >= 2 && ry < 2 + rows {
            let r = (ry - 2) as usize;
            if rx >= 1 && rx <= geo.left_w {
                let ci = self.cat_scroll + r;
                if ci < CATALOG.len() {
                    return Hit::Category(ci);
                }
            } else if rx > geo.left_w + 1 && rx < geo.w - 1 {
                let ii = self.scroll + r;
                if ii < self.visible().len() {
                    return Hit::Item(ii);
                }
            }
        }
        Hit::Inside
    }

    pub fn handle_mouse(&mut self, m: MouseEvent, cols: u16, rows: u16) -> MenuAction {
        let Some(geo) = self.geometry(cols, rows) else { return MenuAction::Close };
        let action = match m.kind {
            MouseEventKind::Down(MouseButton::Left) => match self.hit(m.column, m.row, &geo) {
                Hit::Outside => MenuAction::Close,
                Hit::Category(i) => {
                    self.cat = i;
                    self.filter.clear();
                    self.reset_items();
                    self.focus = Focus::Categories;
                    MenuAction::None
                }
                Hit::Item(i) => match self.visible().get(i) {
                    Some(&(cat, app)) => MenuAction::Add { cat, app },
                    None => MenuAction::None,
                },
                Hit::Inside => MenuAction::None,
            },
            // Any other button closes it; right-clicking again then opens
            // a fresh menu wherever you like.
            MouseEventKind::Down(_) => MenuAction::Close,
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let dir: i64 = if m.kind == MouseEventKind::ScrollUp { -1 } else { 1 };
                let over_left = m.column > geo.x
                    && m.column <= geo.x + geo.left_w
                    && m.row >= geo.y
                    && m.row < geo.y + geo.h;
                if over_left && self.filter.is_empty() {
                    let n = CATALOG.len() as i64;
                    self.cat = (self.cat as i64 + dir).clamp(0, n - 1) as usize;
                    self.reset_items();
                } else {
                    self.step_items(dir * 3);
                }
                MenuAction::None
            }
            _ => MenuAction::None,
        };
        self.ensure_visible(cols, rows);
        action
    }

    pub fn render(&self, out: &mut impl Write, cols: u16, rows: u16, pal: &MenuPalette, unicode: bool) -> Result<()> {
        let Some(geo) = self.geometry(cols, rows) else { return Ok(()) };
        let (tl, tr, bl, br, hz, vt) = if unicode {
            ('┌', '┐', '└', '┘', '─', '│')
        } else {
            ('+', '+', '+', '+', '-', '|')
        };
        let iw = geo.w as usize - 2;
        let left_w = geo.left_w as usize;
        let rw = geo.w as usize - left_w - 3;
        let list_n = list_rows(&geo);
        let vis = self.visible();
        let filtering = !self.filter.is_empty();
        let vt_s = vt.to_string();

        // Top border with title
        let title = " Add program to the desktop ";
        let mut top = String::new();
        top.push(tl);
        top.push_str(title);
        top.push_str(&hz.to_string().repeat(iw - title.len()));
        top.push(tr);
        put(out, geo.x, geo.y, &top, pal.border, pal.bg)?;

        // Header row
        let hy = geo.y + 1;
        let header_right = if filtering {
            format!(" Search results: {} of {}", vis.len(), catalog::total_apps())
        } else {
            let c = &CATALOG[self.cat];
            let inst = self.installed[self.cat].iter().filter(|b| **b).count();
            format!(" {}: {} programs, {} installed (*)", c.name, c.apps.len(), inst)
        };
        put(out, geo.x, hy, &vt_s, pal.border, pal.bg)?;
        put(out, geo.x + 1, hy, &fit(" Categories", left_w), pal.border, pal.bg)?;
        put(out, geo.x + 1 + geo.left_w, hy, &vt_s, pal.border, pal.bg)?;
        put(out, geo.x + geo.left_w + 2, hy, &fit(&header_right, rw), pal.border, pal.bg)?;
        put(out, geo.x + geo.w - 1, hy, &vt_s, pal.border, pal.bg)?;

        // List rows
        for r in 0..list_n {
            let y = geo.y + 2 + r as u16;
            put(out, geo.x, y, &vt_s, pal.border, pal.bg)?;

            // Category pane
            let ci = self.cat_scroll + r;
            let (txt, fg, bg) = if ci < CATALOG.len() {
                let sel = ci == self.cat;
                let t = format!("{} {}", if sel { '>' } else { ' ' }, CATALOG[ci].name);
                if sel && !filtering && self.focus == Focus::Categories {
                    (t, pal.sel_fg, pal.sel_bg)
                } else if sel && !filtering {
                    (t, pal.border, pal.bg)
                } else {
                    (t, pal.fg, pal.bg)
                }
            } else {
                (String::new(), pal.fg, pal.bg)
            };
            put(out, geo.x + 1, y, &fit(&txt, left_w), fg, bg)?;
            put(out, geo.x + 1 + geo.left_w, y, &vt_s, pal.border, pal.bg)?;

            // Program pane
            let x0 = geo.x + geo.left_w + 2;
            let ii = self.scroll + r;
            if let Some(&(c, ap)) = vis.get(ii) {
                let app = &CATALOG[c].apps[ap];
                let inst = self.installed[c][ap];
                let sel = ii == self.item;
                let hl = sel && (self.focus == Focus::Items || filtering);
                let (fg, bg) = if hl {
                    (pal.sel_fg, pal.sel_bg)
                } else if sel {
                    (pal.border, pal.bg)
                } else if inst {
                    (pal.fg, pal.bg)
                } else {
                    (pal.dim, pal.bg)
                };
                let marker_fg = if hl || !inst { fg } else { pal.ok };
                put(out, x0, y, if inst { "* " } else { "  " }, marker_fg, bg)?;
                let body = format!("{:<16} {}", app.name, app.desc);
                put(out, x0 + 2, y, &fit(&body, rw - 2), fg, bg)?;
            } else {
                put(out, x0, y, &fit("", rw), pal.fg, pal.bg)?;
            }
            put(out, geo.x + geo.w - 1, y, &vt_s, pal.border, pal.bg)?;
        }

        // Filter / help row
        let fy = geo.y + geo.h - 2;
        let left = format!(" Filter: {}_", self.filter);
        let hint = if filtering { "Enter: add - Esc: clear " } else { "type to filter - Enter: add - Esc: close " };
        let gap = iw.saturating_sub(left.width() + hint.width());
        let line = if gap > 0 { format!("{}{}{}", left, " ".repeat(gap), hint) } else { left };
        put(out, geo.x, fy, &vt_s, pal.border, pal.bg)?;
        put(out, geo.x + 1, fy, &fit(&line, iw), pal.fg, pal.bg)?;
        put(out, geo.x + geo.w - 1, fy, &vt_s, pal.border, pal.bg)?;

        // Bottom border
        let mut bottom = String::new();
        bottom.push(bl);
        bottom.push_str(&hz.to_string().repeat(iw));
        bottom.push(br);
        put(out, geo.x, geo.y + geo.h - 1, &bottom, pal.border, pal.bg)?;

        queue!(out, ResetColor)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent { code, modifiers: KeyModifiers::NONE, kind: KeyEventKind::Press, state: KeyEventState::NONE }
    }

    fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent { kind, column, row, modifiers: KeyModifiers::NONE }
    }

    #[test]
    fn too_small_screens_have_no_geometry() {
        let m = Menu::new(None, None);
        assert!(m.geometry(40, 30).is_none());
        assert!(m.geometry(100, 10).is_none());
        assert!(m.geometry(100, 28).is_some());
    }

    #[test]
    fn popup_is_clamped_inside_the_screen_above_the_taskbar() {
        let m = Menu::new(Some((95, 27)), None);
        let g = m.geometry(100, 28).unwrap();
        assert!(g.x + g.w <= 100);
        assert!(g.y + g.h <= 26, "must not cover taskbar/status rows");
    }

    #[test]
    fn typing_filters_across_all_categories() {
        let mut m = Menu::new(None, None);
        for ch in "htop".chars() {
            m.handle_key(key(KeyCode::Char(ch)), 100, 28);
        }
        let v = m.visible();
        assert!(!v.is_empty());
        assert!(v.iter().any(|&(c, a)| CATALOG[c].apps[a].name == "htop"));
        // Esc clears the filter first, a second Esc closes.
        assert_eq!(m.handle_key(key(KeyCode::Esc), 100, 28), MenuAction::None);
        assert_eq!(m.handle_key(key(KeyCode::Esc), 100, 28), MenuAction::Close);
    }

    #[test]
    fn keyboard_flow_adds_the_selected_program() {
        let mut m = Menu::new(None, None);
        // Enter on the category pane moves to the program pane ...
        assert_eq!(m.handle_key(key(KeyCode::Enter), 100, 28), MenuAction::None);
        // ... Down selects the 2nd program, Enter adds it.
        m.handle_key(key(KeyCode::Down), 100, 28);
        assert_eq!(m.handle_key(key(KeyCode::Enter), 100, 28), MenuAction::Add { cat: 0, app: 1 });
    }

    #[test]
    fn selection_scrolls_and_stays_in_bounds() {
        let mut m = Menu::new(None, None);
        m.handle_key(key(KeyCode::Down), 100, 28); // category 1 (Files...)
        m.handle_key(key(KeyCode::Right), 100, 28);
        for _ in 0..500 {
            m.handle_key(key(KeyCode::Down), 100, 28);
        }
        assert_eq!(m.item, CATALOG[1].apps.len() - 1);
        let g = m.geometry(100, 28).unwrap();
        assert!(m.item >= m.scroll && m.item < m.scroll + list_rows(&g));
    }

    #[test]
    fn mouse_click_on_item_adds_and_outside_closes() {
        let mut m = Menu::new(None, None);
        let g = m.geometry(100, 28).unwrap();
        // first program row, right pane
        let (x, y) = (g.x + g.left_w + 4, g.y + 2);
        assert_eq!(m.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), x, y), 100, 28), MenuAction::Add { cat: 0, app: 0 });
        assert_eq!(m.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), 0, 0), 100, 28), MenuAction::Close);
        assert_eq!(m.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Right), x, y), 100, 28), MenuAction::Close);
    }

    #[test]
    fn clicking_a_category_switches_the_program_list() {
        let mut m = Menu::new(None, None);
        let g = m.geometry(100, 28).unwrap();
        m.handle_mouse(mouse(MouseEventKind::Down(MouseButton::Left), g.x + 3, g.y + 2 + 2), 100, 28);
        assert_eq!(m.cat, 2);
        assert!(m.visible().iter().all(|&(c, _)| c == 2));
    }
}
