# rust-desktop

A text-based "desktop" written in Rust. Window switching runs entirely
through **tmux**; the interface (icons, taskbar, trash, ASCII background
image) is operated with the mouse in plain text mode.

## How it works

- This process itself runs in **one** tmux window and draws the "desktop
  surface" there (wallpaper, icons, taskbar, trash).
- **Double-clicking** an icon → `tmux new-window` opens the associated
  program in a new tmux window and switches to it automatically. You get
  back to the desktop with `tmux next-window` / `Ctrl-b p,n` / by switching
  to it again from the taskbar.
- The **taskbar** at the bottom reads `tmux list-windows` and shows all open
  windows of the session. Click = `tmux select-window` (switch windows),
  right-click = `tmux kill-window` (close window).
- **Moving icons**: drag with the left mouse button held down. On release,
  the icon automatically snaps to an invisible grid and steps around other
  icons instead of overlapping them (snap-to-grid).
- **Keyboard control for icons**: `Tab`/arrow down/right selects the next
  icon, `Shift+Tab`/arrow up/left the previous one, `Enter` opens the
  selected icon – fully usable without a mouse.
- **Deleting**: drag an icon onto the trash (happens immediately, since
  it's a deliberate drag gesture), or right-click an icon (asks for
  confirmation first, as a safety measure: `j`/`y` confirms, `n`/`Esc`
  cancels). `e` likewise asks for confirmation before the trash is
  permanently emptied; `u` restores the most recently deleted icon.
- **Singleton icons**: an icon can be marked `"singleton": true` (e.g.
  "Processes" by default). When opening it, the program first checks
  whether a tmux window with the same name already exists – if so, it
  switches there instead of opening another window.
- A small **clock** runs in the top right corner.
- The **background image** is loaded via the `image` crate at startup,
  rasterized into brightness values, and translated into a character ramp –
  fitted exactly to the current terminal size. Without an image, a quiet
  procedural pattern is drawn as a fallback.

## Automatic detection: tty vs. terminal emulator

At startup the program checks whether it's running on a raw Linux
framebuffer console (`TERM=linux`, i.e. e.g. `Ctrl+Alt+F2` without
X11/Wayland) or inside a terminal emulator, and adjusts the character set
and colors accordingly:

| Environment | Character set | Colors | Icons |
|---|---|---|---|
| Linux console (tty) | plain ASCII ramp (` .:-=+*#%@`) | 16 standard colors, monochrome gray wallpaper | `[T]`, `[E]`, `[D]`, `[P]`, `[ ]`/`[#]` |
| Terminal emulator, ASCII locale | ASCII ramp | 256 colors/truecolor depending on `COLORTERM`, wallpaper in original colors | ASCII bracket icons |
| Terminal emulator, UTF-8 locale | extended ramp with block characters (` .:-=+*#░▒▓█`) | 256 colors/truecolor, wallpaper in original colors | Emoji (🖥️ 📝 📁 📊 🗑️) |

Detection logic (see `src/caps.rs`):
- **tty detection**: `$TERM` is `linux`, `hurd`, or similar → the
  framebuffer console has no color emoji font, so ASCII icons and only 16
  colors are used automatically there, regardless of what `COLORTERM` claims.
- **Unicode/emoji**: based on `$LC_ALL`/`$LC_CTYPE`/`$LANG` (needs a UTF-8
  locale) and only outside the tty console.
- **Color depth**: `COLORTERM=truecolor`/`24bit` → 24-bit RGB,
  `TERM=*256color*` → 256-color palette (nearest color is computed),
  otherwise 16 colors.

The default icons (Terminal/Editor/Files/Processes) are created on the
very first run to match the detected environment (emoji or ASCII brackets)
and then saved to `~/.config/cli-desktop/icons.json`.

Pressing **`i`** shows the detected environment in the status line at any
time. For testing, detection can be overridden via `--force-tty` (forces
tty behavior) or `--force-emoji` (forces emoji/Unicode).

## Build

```bash
cargo build --release
```

Requirements: Rust/Cargo, plus `tmux` as a runtime dependency (must be
present to run, but isn't compiled in).

## Running

The program **must** run inside a tmux session:

```bash
tmux new -s desktop ./target/release/rust-desktop

# with a custom background image:
tmux new -s desktop './target/release/rust-desktop --wallpaper /path/to/image.jpg'
```

If you're already inside a tmux session, simply run:

```bash
./target/release/rust-desktop
```

Additional options:

```bash
./target/release/rust-desktop --help                      # quick overview of all options/keys
./target/release/rust-desktop --config-dir /path/to/dir    # use a different config directory instead of ~/.config/cli-desktop
```

`--config-dir` is handy, for example, for several independent "profiles"
(different icon sets depending on context) or for testing without touching
your own configuration.

## Keyboard shortcuts

| Key | Action |
|---|---|
| `q` / `Esc` | quit the desktop (instead cancels an open confirmation, if any) |
| `Tab` / `↓` `→` | select the next icon |
| `Shift+Tab` / `↑` `←` | select the previous icon |
| `Enter` | open the selected icon |
| `j` / `y` | accept a pending confirmation (delete icon / empty trash) |
| `n` / `Esc` | cancel a pending confirmation |
| `r` | manually refresh the taskbar |
| `e` | empty the trash (asks for confirmation first) |
| `u` | restore the last deleted icon |
| `i` | show the detected environment (tty/terminal, colors, emoji) |
| `-h` / `--help` | brief help (command line, before starting) |

## Configuration

Settings live under `~/.config/cli-desktop/`, each part in its **own file**:

| File | Contents |
|---|---|
| `~/.config/cli-desktop/icons.json` | desktop icons (name, glyph, position, command, singleton flag) |
| `~/.config/cli-desktop/trash.json` | icons moved to the trash |
| `~/.config/cli-desktop/wallpaper.json` | path to the background image (`{"path": "..."}` or `{"path": null}`) |
| `~/.config/cli-desktop/settings.json` | time intervals (taskbar polling, clock, full redraw) |
| `~/.config/cli-desktop/colors.json` | UI color scheme |

Each file is only written when the relevant part actually changes (e.g.
double-clicking an icon only touches `icons.json`, emptying the trash only
touches `trash.json`). If a file is missing at startup, it's recreated with
a sensible default – `icons.json`, for example, with the four default icons
(Terminal, Editor, Files, Processes), matching the detected environment
(emoji or ASCII brackets).

This allows, for example, versioning/sharing `icons.json` independently, or
changing the wallpaper via a script without touching the icon positions:

```bash
echo '{"path": "/path/to/new/image.jpg"}' > ~/.config/cli-desktop/wallpaper.json
```

`--config-dir <path>` lets you use a completely different directory instead
of `~/.config/cli-desktop` (see the "Running" section above).

### Icon fields (`icons.json`)

```json
{
  "id": 4,
  "name": "Processes",
  "glyph": "📊",
  "x": 4,
  "y": 14,
  "command": "sh -c 'htop || top'",
  "singleton": true
}
```

`singleton: true` means: when opening it, the desktop first checks via
`tmux list-windows` whether a window with this icon's name already exists;
if so, it just switches there instead of opening another window. Useful for
programs you only ever want a single instance of anyway (process monitor,
possibly a file manager). If the field is missing in an older `icons.json`,
it's automatically treated as `false`.

### Time intervals (`settings.json`)

```json
{
  "window_poll_secs": 2,
  "clock_refresh_secs": 1,
  "full_redraw_secs": 5
}
```

- `window_poll_secs`: how often the taskbar automatically re-queries tmux.
- `clock_refresh_secs`: the minimum redraw interval (among other things, so
  the clock keeps ticking).
- `full_redraw_secs`: how often the screen is additionally cleared and
  redrawn from scratch (self-healing, see the "Rendering" section below).

Values are only read at startup; changes to the file only take effect after
restarting the program.

### Color scheme (`colors.json`)

```json
{
  "icon_fg": "white",
  "label_fg": "cyan",
  "selected_fg": "black",
  "selected_bg": "yellow",
  "danger_fg": "white",
  "danger_bg": "red",
  "trash_full_fg": "yellow",
  "taskbar_fg": "white",
  "taskbar_bg": "blue",
  "taskbar_active_fg": "black",
  "taskbar_active_bg": "cyan",
  "status_fg": "grey",
  "clock_fg": "white",
  "clock_bg": "black",
  "wallpaper_fallback_fg": "darkgrey"
}
```

Each value is either a color name (`black`, `red`, `green`, `yellow`,
`blue`, `magenta`, `cyan`, `white`, `grey`/`gray`, `darkgrey`/`darkgray`, as
well as `dark` variants of red/green/yellow/blue/magenta/cyan) or a hex
color `#rrggbb`. Hex colors are automatically converted to match the
detected terminal capability: truecolor directly, 256-color terminals round
to the nearest palette color, and a plain tty console (only 16 colors)
safely falls back to white instead of sending a color it can't display.

### Robustness

- **Atomic writes**: every file is first written as `*.tmp` in the same
  directory, then moved into its final place via `rename()`. `rename()` is
  atomic on the same filesystem – if the program crashes mid-write (power
  loss, `kill -9`), a half-written, corrupt `icons.json` is never left behind.
- **Broken config files don't cause a crash**: if `icons.json`, for
  example, is corrupted or incompatible after an update, it's automatically
  moved to `icons.bak-<unixtime>.json`, a warning is printed to stderr, and
  the program continues with default values – instead of aborting with a
  parse error.

## Security note

An icon's `command` field in `icons.json` is passed **unfiltered** to
`tmux new-window` and executed there as a shell command. For personal use
on your own machine this is uncritical – it's ultimately the same trust
level as your own shell configuration (`.bashrc` etc.). But if `icons.json`
is shared, synced (e.g. via a dotfiles repo), or taken from an untrusted
source, opening a crafted icon will run arbitrary code. `icons.json` should
therefore be treated with the same caution as a shell script from an
unknown source.

## Architecture

```
src/
  main.rs        Event loop (keyboard + mouse via crossterm), SIGTERM/SIGHUP handling
  desktop.rs     Rendering (wallpaper, icons, taskbar, trash) + hit testing,
                 snap-to-grid, confirmation dialogs, theme color resolution
  tmux.rs        Wrapper around the tmux CLI (list/new/select/kill-window)
  ascii_art.rs   Image → ASCII conversion (image crate) + fallback pattern
  caps.rs        Detection of tty vs. terminal emulator, Unicode/emoji/color depth
  config.rs      Configuration persistence (icons/trash/wallpaper/settings/colors, each its own JSON file)
```

## Known limitations

- The tmux session the program was started in is the one it uses;
  multi-monitor/multi-session setups aren't supported.
- Drag & drop and double-clicking need a terminal with working mouse
  reporting (SGR mouse mode); most modern terminals (Alacritty, kitty,
  iTerm2, GNOME Terminal, Windows Terminal via WSL) support this.
- The trash only deletes the desktop shortcut (the icon entry in the JSON
  configuration), not real files.
- **Snap-to-grid**: on release, the icon's position is rounded to a 10×4
  character grid; if it then overlaps another icon, the next free spot is
  searched for row by row (then column by column). With very small
  terminals or very many icons this search can hit the screen edges;
  there's currently no scrolling/multi-page view.
- **Shutting down via signal**: `SIGTERM`/`SIGHUP` (e.g. from
  `tmux kill-session` or a system shutdown) are caught and lead to an
  orderly shutdown through the normal event loop, so alternate-screen and
  raw mode are reliably restored. `SIGKILL` (`kill -9`) fundamentally can't
  be caught – in that case the terminal can be left in a broken state; the
  `reset` or `tput reset` command helps then.
- **Rendering**: the screen is fully cleared and redrawn on startup and on
  resize, plus every 5 seconds ("self-healing" against rendering glitches
  that could, for example, accumulate from stray escape sequences from
  other tmux windows). In between, every frame overwrites every cell
  anyway (the wallpaper covers the whole area, the taskbar and status line
  are padded to full width), so there's no visible flicker from
  mouse/keyboard actions between the full redraws.
