//! Thin wrapper around the `tmux` command line.
//!
//! The entire "window manager" for this desktop is tmux itself:
//! - every launched program runs in its own tmux window
//! - the taskbar reads `tmux list-windows`
//! - clicking a taskbar entry calls `tmux select-window`
//! - double-clicking a desktop icon calls `tmux new-window`

use anyhow::{anyhow, Context, Result};
use std::process::Command;

#[derive(Debug, Clone)]
pub struct TmuxWindow {
    pub index: u32,
    pub name: String,
    pub active: bool,
}

/// Is this process running inside a tmux session at all?
pub fn is_inside_tmux() -> bool {
    std::env::var("TMUX").is_ok()
}

fn run_tmux(args: &[&str]) -> Result<String> {
    let output = Command::new("tmux")
        .args(args)
        .output()
        .context("Failed to run tmux (is tmux installed?)")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow!("tmux {:?} failed: {}", args, stderr.trim()));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Determine the name of the current tmux session.
pub fn current_session() -> Result<String> {
    run_tmux(&["display-message", "-p", "#S"])
}

/// List all windows of the current session.
pub fn list_windows(session: &str) -> Result<Vec<TmuxWindow>> {
    let raw = run_tmux(&[
        "list-windows",
        "-t",
        session,
        "-F",
        "#{window_index}|#{window_name}|#{window_active}",
    ])?;

    let mut windows = Vec::new();
    for line in raw.lines() {
        let parts: Vec<&str> = line.splitn(3, '|').collect();
        if parts.len() != 3 {
            continue;
        }
        windows.push(TmuxWindow {
            index: parts[0].parse().unwrap_or(0),
            name: parts[1].to_string(),
            active: parts[2] == "1",
        });
    }
    Ok(windows)
}

/// Compute the lowest non-negative window index in `session` that is
/// currently free, by asking tmux for the windows that actually exist
/// right now (rather than trusting tmux's own implicit "next index"
/// selection, which is what we fall back to below after a conflict).
fn next_free_index(session: &str) -> Result<u32> {
    let used: std::collections::HashSet<u32> =
        list_windows(session)?.into_iter().map(|w| w.index).collect();
    let mut idx = 0;
    while used.contains(&idx) {
        idx += 1;
    }
    Ok(idx)
}

/// Open a new window with the given name + command and switch to it immediately.
///
/// If tmux reports the target index as already in use (`create window
/// failed: index N in use` — this can happen if another window was created
/// concurrently, e.g. from a second rust-desktop instance running in the
/// same session, or via tmux's own `Ctrl-b c` binding, right between us
/// listing windows and creating one), this retries a couple of times with
/// an index we compute ourselves from a fresh `list-windows`, instead of
/// surfacing a raw tmux error for what is essentially a transient race.
pub fn new_window(session: &str, name: &str, command: &str) -> Result<()> {
    match run_tmux(&["new-window", "-t", session, "-n", name, command]) {
        Ok(_) => Ok(()),
        Err(e) if e.to_string().contains("index") && e.to_string().contains("in use") => {
            for _ in 0..5 {
                let idx = next_free_index(session)?;
                let target = format!("{}:{}", session, idx);
                match run_tmux(&["new-window", "-t", &target, "-n", name, command]) {
                    Ok(_) => return Ok(()),
                    Err(e2) if e2.to_string().contains("index") && e2.to_string().contains("in use") => {
                        continue; // another race, try again with a freshly computed index
                    }
                    Err(e2) => return Err(e2),
                }
            }
            Err(e)
        }
        Err(e) => Err(e),
    }
}

/// Switch to a specific window (by index).
pub fn select_window(session: &str, index: u32) -> Result<()> {
    let target = format!("{}:{}", session, index);
    run_tmux(&["select-window", "-t", &target])?;
    Ok(())
}

/// Close a window (e.g. via right-click in the taskbar).
pub fn kill_window(session: &str, index: u32) -> Result<()> {
    let target = format!("{}:{}", session, index);
    run_tmux(&["kill-window", "-t", &target])?;
    Ok(())
}

/// Determine the index of the window this desktop process itself is running
/// in, so we don't accidentally "close" it from the taskbar.
pub fn own_window_index() -> Option<u32> {
    std::env::var("TMUX_PANE").ok()?;
    let raw = run_tmux(&["display-message", "-p", "#{window_index}"]).ok()?;
    raw.parse().ok()
}

/// The `$TERM` value of the *attached client's* outer terminal — i.e. the
/// real terminal tmux itself is running in (a GUI emulator, or the raw
/// Linux console), as opposed to the pane's own `$TERM`.
///
/// This distinction matters a lot: tmux always overrides `$TERM` *inside*
/// a pane to one of its own terminfo entries (typically `tmux-256color` or
/// `screen-256color`), completely independent of what the outer terminal
/// actually is. Checking the pane's own `$TERM` can therefore never tell
/// us whether we're really on the bare Linux console or in a full GUI
/// terminal emulator — both look identical from inside a pane. tmux does,
/// however, track the outer client's original `$TERM` and exposes it via
/// the `client_termname` format variable, which is what this reads.
pub fn client_termname() -> Option<String> {
    let raw = run_tmux(&["display-message", "-p", "#{client_termname}"]).ok()?;
    if raw.is_empty() {
        None
    } else {
        Some(raw)
    }
}

/// The device path of the attached client's outer terminal, e.g.
/// `/dev/pts/3` for anything running under a pseudo-terminal (every
/// terminal emulator, every SSH session, `script`, ...) or `/dev/tty2` /
/// `/dev/console` for a genuine Linux virtual console.
///
/// This is a second, independent signal for the same tty-vs-emulator
/// question `client_termname` answers, and a more reliable one in some
/// ways: it doesn't depend on `$TERM` being set to any particular string
/// (which can vary across distros/configurations), just on which kind of
/// device file the client is actually attached to — that's unambiguous.
pub fn client_tty() -> Option<String> {
    let raw = run_tmux(&["display-message", "-p", "#{client_tty}"]).ok()?;
    if raw.is_empty() {
        None
    } else {
        Some(raw)
    }
}

/// Read whether tmux's own `mouse` option is currently on.
pub fn mouse_enabled() -> Result<bool> {
    let raw = run_tmux(&["show-options", "-g", "mouse"])?;
    // Output looks like "mouse on" / "mouse off".
    Ok(raw.trim_end().ends_with("on"))
}

/// Turn tmux's global `mouse` option on.
///
/// This matters a great deal: by default (without a `set -g mouse on` in
/// the user's `~/.tmux.conf`), tmux does NOT forward raw mouse click events
/// from the outer terminal into a pane at all, even if the program running
/// inside that pane (like this one, via crossterm) has requested mouse
/// reporting. Without this, every click on an icon would silently do
/// nothing — the single most common reason a mouse-driven tmux program
/// "doesn't react to clicks". We turn it on automatically so the desktop
/// works out of the box regardless of the user's tmux configuration.
pub fn enable_mouse() -> Result<()> {
    run_tmux(&["set-option", "-g", "mouse", "on"])?;
    Ok(())
}
