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

/// Open a new window with the given name + command and switch to it immediately.
pub fn new_window(session: &str, name: &str, command: &str) -> Result<()> {
    run_tmux(&[
        "new-window",
        "-t",
        session,
        "-n",
        name,
        command,
    ])?;
    Ok(())
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
