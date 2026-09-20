mod ascii_art;
mod caps;
mod config;
mod desktop;
mod tmux;

use anyhow::{Context, Result};
use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers, MouseButton,
    MouseEventKind,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::{execute, terminal};
use desktop::Desktop;
use std::io::{stdout, Stdout, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

const DOUBLE_CLICK_MS: u64 = 400;

struct Term {
    out: Stdout,
}

impl Term {
    fn new() -> Result<Self> {
        enable_raw_mode().context("Failed to put the terminal into raw mode")?;
        let mut out = stdout();
        execute!(out, EnterAlternateScreen, EnableMouseCapture)?;
        Ok(Self { out })
    }
}

impl Drop for Term {
    fn drop(&mut self) {
        let _ = execute!(self.out, DisableMouseCapture, LeaveAlternateScreen);
        let _ = disable_raw_mode();
    }
}

fn print_help_and_exit() {
    println!(
        "rust-desktop – a tmux-based text desktop\n\n\
         Usage:\n  \
         rust-desktop [--wallpaper <image>] [--config-dir <path>] [--force-tty] [--force-emoji] [--no-mouse-setup]\n\n\
         Options:\n  \
         --wallpaper, -w <image>   Set a background image once (saved to wallpaper.json)\n  \
         --config-dir <path>      Use this config directory instead of ~/.config/cli-desktop\n  \
         --force-tty              Force detection: plain tty console (ASCII, 16 colors, no emoji)\n  \
         --force-emoji            Force detection: enable Unicode/emoji\n  \
         --no-mouse-setup          Don't automatically run `tmux set-option -g mouse on`\n  \
         -h, --help                Show this help\n\n\
         Keys while running:\n  \
         Tab / Shift+Tab or arrow keys       switch icon selection\n  \
         Shift + arrow keys                  move the selected icon\n  \
         Enter                               open the selected icon\n  \
         q / Esc                             quit\n  \
         r                                   refresh the taskbar\n  \
         e                                   empty the trash (asks for confirmation)\n  \
         u                                   restore the last deleted icon\n  \
         i                                   show detected environment\n\n\
         Configuration lives under ~/.config/cli-desktop/ (icons.json, trash.json,\n\
         wallpaper.json, settings.json, colors.json) – see the README.\n\n\
         Note: on first run, this also enables tmux's global `mouse` option\n\
         (`tmux set-option -g mouse on`), since tmux otherwise won't forward\n\
         mouse clicks from your real terminal into the pane at all. Pass\n\
         --no-mouse-setup to skip this if you manage that setting yourself."
    );
    std::process::exit(0);
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "-h" || a == "--help") {
        print_help_and_exit();
    }

    // Without tmux this "desktop" makes no sense, since it relies on tmux
    // as the window manager for switching windows.
    if !tmux::is_inside_tmux() {
        eprintln!(
            "This program must run inside a tmux session.\n\
             Start it like this, for example:\n\n    tmux new -s desktop ./rust-desktop\n\n\
             (use the actual path to the binary — a bare 'rust-desktop' only\n\
             works if it's on your $PATH, e.g. after 'cargo install --path .')\n\n\
             or start tmux first and then run rust-desktop inside a session."
        );
        std::process::exit(1);
    }

    let wallpaper_arg = args.iter().position(|a| a == "--wallpaper" || a == "-w")
        .and_then(|i| args.get(i + 1).cloned());
    let config_dir_arg = args.iter().position(|a| a == "--config-dir")
        .and_then(|i| args.get(i + 1).cloned());
    // For testing/debugging only: override detection manually.
    let force_tty = args.iter().any(|a| a == "--force-tty");
    let force_emoji = args.iter().any(|a| a == "--force-emoji");
    let no_mouse_setup = args.iter().any(|a| a == "--no-mouse-setup");

    if let Some(dir) = config_dir_arg {
        config::set_config_dir_override(PathBuf::from(dir));
    }

    let capabilities = caps::Capabilities::detect().with_overrides(force_tty, force_emoji);
    eprintln!("Detected environment: {}", capabilities.summary());
    eprintln!("Config directory: {:?}", config::config_dir()?);

    // Without `tmux set-option -g mouse on`, tmux does not forward raw mouse
    // click events from the outer terminal into this pane at all, no matter
    // what this program requests via crossterm — clicking icons would then
    // silently do nothing. We turn this on automatically unless the user
    // explicitly opted out, and always report what we did (or didn't do).
    if no_mouse_setup {
        eprintln!("Skipping tmux mouse setup (--no-mouse-setup passed).");
    } else {
        match tmux::mouse_enabled() {
            Ok(true) => eprintln!("tmux mouse support: already on."),
            Ok(false) | Err(_) => match tmux::enable_mouse() {
                Ok(()) => eprintln!(
                    "tmux mouse support was off; enabled it now (tmux set-option -g mouse on)."
                ),
                Err(e) => eprintln!(
                    "Warning: could not enable tmux mouse support automatically ({}). \
                     If clicks don't do anything, add 'set -g mouse on' to your ~/.tmux.conf, \
                     or run 'tmux set-option -g mouse on' manually.",
                    e
                ),
            },
        }
    }

    let mut cfg = config::DesktopConfig::load_or_default(&capabilities)?;
    if let Some(w) = wallpaper_arg.clone() {
        cfg.wallpaper = Some(w);
        cfg.save_wallpaper()?;
    }

    // Time intervals come from settings.json (with sensible defaults)
    // instead of hardcoded constants – see config::Settings.
    let window_poll_interval = Duration::from_secs(cfg.settings.window_poll_secs.max(1));
    let clock_refresh_interval = Duration::from_secs(cfg.settings.clock_refresh_secs.max(1));
    let full_redraw_interval = Duration::from_secs(cfg.settings.full_redraw_secs.max(1));

    let (cols, rows) = terminal::size()?;
    let mut desktop = Desktop::new(cfg, cols, rows, wallpaper_arg, capabilities)?;

    // Warn (without blocking) if another rust-desktop instance already
    // seems to be running in this same tmux session — this is a common
    // source of confusion, and can trigger tmux errors like `create window
    // failed: index N in use` if both instances happen to open a window at
    // nearly the same moment.
    if let Some(warning) = config::check_and_write_instance_lock(&desktop.session, desktop.own_window) {
        eprintln!("Warning: {}", warning);
        desktop.status = format!("Warning: {}", warning);
    }

    // Clean shutdown on SIGTERM/SIGHUP (e.g. `tmux kill-session`, system
    // shutdown): instead of killing the process abruptly (which would leave
    // the terminal in alternate-screen/raw mode), this just sets a flag that
    // lets the event loop exit cleanly on its next iteration – the `Term`
    // guard then cleans up as usual via `Drop`.
    let shutdown_requested = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGTERM, Arc::clone(&shutdown_requested))
        .context("Could not register the SIGTERM handler")?;
    signal_hook::flag::register(signal_hook::consts::SIGHUP, Arc::clone(&shutdown_requested))
        .context("Could not register the SIGHUP handler")?;

    let _term = Term::new()?;
    let mut out = stdout();

    desktop.clear(&mut out)?;
    desktop.render(&mut out)?;

    let mut last_window_poll = Instant::now();
    let mut last_render = Instant::now();
    let mut last_full_redraw = Instant::now();

    loop {
        if shutdown_requested.load(Ordering::Relaxed) {
            break;
        }

        // Every few seconds, fully clear and redraw the screen, independent
        // of the otherwise cell-preserving rendering (see settings.json).
        if last_full_redraw.elapsed() > full_redraw_interval {
            desktop.clear(&mut out)?;
            desktop.render(&mut out)?;
            last_full_redraw = Instant::now();
            last_render = Instant::now();
        }

        // Automatically refresh the taskbar every few seconds so newly
        // opened/closed tmux windows show up even without a click.
        if last_window_poll.elapsed() > window_poll_interval {
            desktop.refresh_windows();
            desktop.render(&mut out)?;
            last_window_poll = Instant::now();
            last_render = Instant::now();
        }

        if !event::poll(Duration::from_millis(250))? {
            // Redraw periodically even without input, so the clock in the
            // top right corner keeps ticking.
            if last_render.elapsed() >= clock_refresh_interval {
                desktop.render(&mut out)?;
                last_render = Instant::now();
            }
            continue;
        }

        match event::read()? {
            Event::Resize(c, r) => {
                desktop.resize(c, r);
                desktop.clear(&mut out)?;
                desktop.render(&mut out)?;
                last_full_redraw = Instant::now();
                last_render = Instant::now();
            }
            Event::Key(k) => {
                // While a confirmation is pending (deleting an icon or
                // emptying the trash), key presses are interpreted
                // exclusively for that purpose.
                if desktop.has_pending() {
                    match k.code {
                        KeyCode::Char('j') | KeyCode::Char('y') | KeyCode::Enter => {
                            desktop.confirm_pending()
                        }
                        KeyCode::Char('n') | KeyCode::Esc => desktop.cancel_pending(),
                        _ => {}
                    }
                    desktop.render(&mut out)?;
                    continue;
                }

                let shift = k.modifiers.contains(KeyModifiers::SHIFT);
                match (shift, k.code) {
                    (_, KeyCode::Char('q')) | (_, KeyCode::Esc) => break,
                    (false, KeyCode::Tab) | (false, KeyCode::Down) | (false, KeyCode::Right) => {
                        desktop.select_next()
                    }
                    (false, KeyCode::BackTab) | (false, KeyCode::Up) | (false, KeyCode::Left) => {
                        desktop.select_prev()
                    }
                    // Shift+arrows: move the selected icon (keyboard
                    // equivalent of drag-and-drop, since mouse dragging
                    // generally doesn't work on the raw Linux console).
                    (true, KeyCode::Up) => desktop.move_selected(0, -1),
                    (true, KeyCode::Down) => desktop.move_selected(0, 1),
                    (true, KeyCode::Left) => desktop.move_selected(-1, 0),
                    (true, KeyCode::Right) => desktop.move_selected(1, 0),
                    (_, KeyCode::Enter) => desktop.open_selected(),
                    (_, KeyCode::Char('r')) => {
                        desktop.refresh_windows();
                        desktop.status = "Taskbar refreshed.".into();
                    }
                    (_, KeyCode::Char('e')) => desktop.request_empty_trash(),
                    (_, KeyCode::Char('u')) => desktop.restore_last_trash(),
                    (_, KeyCode::Char('i')) => desktop.show_system_info(),
                    _ => {}
                }
                desktop.render(&mut out)?;
                last_render = Instant::now();
            }
            Event::Mouse(m) => {
                match m.kind {
                    MouseEventKind::Down(MouseButton::Left) => {
                        if let Some(idx) = desktop.icon_at(m.column, m.row) {
                            // Double-click detection
                            let is_double = matches!(
                                desktop.last_click,
                                Some((last_idx, t)) if last_idx == idx
                                    && t.elapsed() < Duration::from_millis(DOUBLE_CLICK_MS)
                            );
                            desktop.selected_icon = Some(idx);
                            desktop.last_click = Some((idx, Instant::now()));

                            if is_double {
                                desktop.launch_icon(idx);
                                desktop.last_click = None;
                            } else {
                                let icon = &desktop.cfg.icons[idx];
                                let offset_x = m.column as i32 - icon.x as i32;
                                let offset_y = m.row as i32 - icon.y as i32;
                                desktop.dragging = Some(desktop::DragState {
                                    icon_idx: idx,
                                    offset_x,
                                    offset_y,
                                });
                            }
                        } else if let Some(win_idx) = desktop.taskbar_hit(m.column, m.row) {
                            desktop.select_taskbar(win_idx);
                        } else if desktop.trash_hit(m.column, m.row) {
                            desktop.status =
                                "Trash: 'e' to empty, 'u' to restore the last item."
                                    .into();
                        } else {
                            desktop.selected_icon = None;
                        }
                        desktop.render(&mut out)?;
                    }
                    MouseEventKind::Drag(MouseButton::Left) => {
                        if let Some(drag) = &desktop.dragging {
                            let idx = drag.icon_idx;
                            let (nx, ny) = desktop.clamp_icon_pos(
                                m.column as i32 - drag.offset_x,
                                m.row as i32 - drag.offset_y,
                            );
                            if let Some(icon) = desktop.cfg.icons.get_mut(idx) {
                                icon.x = nx;
                                icon.y = ny;
                            }
                            desktop.render(&mut out)?;
                        }
                    }
                    MouseEventKind::Up(MouseButton::Left) => {
                        if let Some(drag) = desktop.dragging.take() {
                            if desktop.trash_hit(m.column, m.row) {
                                // A deliberate drag action: no extra
                                // confirmation needed, unlike the quick
                                // right-click delete below.
                                desktop.move_icon_to_trash(drag.icon_idx);
                            } else {
                                // Snap to the grid on release and avoid
                                // overlapping other icons.
                                desktop.snap_and_resolve(drag.icon_idx);
                            }
                            desktop.render(&mut out)?;
                        }
                    }
                    MouseEventKind::Down(MouseButton::Right) => {
                        if let Some(win_idx) = desktop.taskbar_hit(m.column, m.row) {
                            desktop.close_taskbar(win_idx);
                        } else if desktop.trash_hit(m.column, m.row) {
                            desktop.request_empty_trash();
                        } else if let Some(idx) = desktop.icon_at(m.column, m.row) {
                            // A right-click doesn't delete immediately, it
                            // asks for confirmation first (protects against
                            // accidental clicks).
                            desktop.request_delete(idx);
                        }
                        desktop.render(&mut out)?;
                    }
                    _ => {}
                }
                last_render = Instant::now();
            }
            _ => {}
        }
    }

    let _ = out.flush();
    config::clear_instance_lock();
    Ok(())
}
