# rust-desktop

A text-based "desktop" written in Rust. Window switching runs entirely
through **tmux**; the interface (icons, taskbar, trash, ASCII background
image) is operated with the mouse in plain text mode.

## How it works

- This process itself runs in **one** tmux window and draws the "desktop
  surface" there (wallpaper, icons, taskbar, trash). On startup, it also
  automatically runs `tmux set-option -g mouse on` if it's currently off,
  since tmux otherwise won't forward mouse clicks from your real terminal
  into the pane at all (see "Troubleshooting" below; pass `--no-mouse-setup`
  to skip this).
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
  icon, `Shift+Tab`/arrow up/left selects the previous one, `Enter` opens the
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
  rasterised into brightness values, and translated into a character ramp –
  fitted exactly to the current terminal size. Without an image, a quiet
  procedural pattern is drawn as a fallback.

## Automatic detection: tty vs. terminal emulator

At startup, the program checks whether it's running on a raw Linux
framebuffer console (`TERM=linux`, i.e. e.g. `Ctrl+Alt+F2` without
X11/Wayland) or inside a terminal emulator, and adjusts the character set
and colours accordingly:

| Environment | Character set | Colors | Icons |
|---|---|---|---|
| Linux console (tty) | plain ASCII ramp (` .:-=+*#%@`) | 16 standard colors, monochrome gray wallpaper | `[T]`, `[E]`, `[D]`, `[P]`, `[ ]`/`[#]` |
| Terminal emulator, ASCII locale | ASCII ramp | 256 colors/truecolor depending on `COLORTERM`, wallpaper in original colours | ASCII bracket icons |
| Terminal emulator, UTF-8 locale | extended ramp with block characters (` .:-=+*#░▒▓█`) | 256 colors/truecolor, wallpaper in original colors | Emoji (🖥️ 📝 📁 📊 🗑️) |

Detection logic (see `src/caps.rs`), either signal being true is enough:
- **Outer `$TERM` name**: as tmux saw it when the attached client
  connected, via `tmux display-message -p '#{client_termname}'` — checked
  against `linux`, `hurd`, `cons25` and similar. This deliberately does
  **not** check the pane's own `$TERM`, since tmux always overrides that to
  its own terminfo entry (e.g. `tmux-256color`) regardless of the real
  outer terminal; checking it would never be able to tell a console and a
  terminal emulator apart.
- **Outer client's tty device path**: via `tmux display-message -p
  '#{client_tty}'`. A genuine Linux virtual console shows up as
  `/dev/ttyN` or `/dev/console`; a terminal emulator, SSH session, `screen`,
  or anything else always shows up as a pseudo-terminal (`/dev/pts/N`).
  This doesn't depend on `$TERM` being set to any particular string at
  all, which makes it a good second, independent signal in case the first
  one doesn't recognise a given system's `$TERM` value.

  On the console (either signal), ASCII icons and only 16 colours are used
  automatically, regardless of what `COLORTERM` claims — and, separately,
  mouse support is generally unavailable there at all (see
  "Troubleshooting").
- **Unicode/emoji**: based on `$LC_ALL`/`$LC_CTYPE`/`$LANG` (needs a UTF-8
  locale) and only outside the tty console.
- **Color depth**: `COLORTERM=truecolor`/`24bit` → 24-bit RGB,
  outer `$TERM` containing `256color` → 256-color palette (nearest color is
  computed), otherwise 16 colours.

Press `i` to see not just the result but the *raw* values detection was
based on (`term=... tty=...`) — handy for figuring out why a given system
wasn't recognised as expected. If it's still wrong for your setup for any
reason, `--force-tty` sidesteps all of this detection entirely.

The default icons (Terminal/Editor/Files/Processes) are created on the
very first run to match the detected environment (emoji or ASCII brackets)
and then saved to `~/.config/cli-desktop/icons.json`.

**Reusing the same config across environments:** `icons.json` stores
whatever glyph was chosen on first run (e.g. an emoji), and that file is
then reused as-is on later runs — including in a *different* environment,
such as switching from a terminal emulator to the raw tty console. Since
the tty console typically can't render emoji at all (no colour-emoji font,
and the glyph is often missing from the console font entirely), the
program does **not** blindly draw whatever is stored in `icons.json`:
every icon's glyph is re-evaluated against the *current* run's detected
capabilities before drawing. If the current environment can't do emoji but
a stored glyph isn't plain ASCII, it's displayed (and, importantly, its
click hit-box is sized) as `[X]` instead, `X` being the icon name's first
letter — so an icon is never invisible or unclickable just because it was
last configured somewhere else. `icons.json` itself is left untouched, so
switching back to a terminal emulator later still shows the original emoji.

Pressing **`i`** shows the detected environment in the status line at any
time. For testing, detection can be overridden via `--force-tty` (forces
tty behaviour) or `--force-emoji` (forces emoji/Unicode).

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
./target/release/rust-desktop --no-mouse-setup             # don't auto-enable tmux's 'mouse' option (see Troubleshooting)
```

`--config-dir` is handy, for example, for several independent "profiles"
(different icon sets depending on context) or for testing without touching
your own configuration.

Optionally, install the binary onto your `$PATH` so a bare `rust-desktop`
works too (this is what `cargo install` does):

```bash
cargo install --path .
tmux new -s desktop rust-desktop
```

## Troubleshooting

**Icons don't open anything when I double-click them.**
By default, tmux does *not* forward mouse clicks from your real terminal
into a pane at all — this needs `set -g mouse on` somewhere in tmux's
configuration. Without it, this program (or any other mouse-driven program
running inside tmux, e.g. a mouse-enabled vim) simply never receives click
events; nothing appears broken, clicks just never arrive. To avoid this
common trap, rust-desktop automatically runs
`tmux set-option -g mouse on` once at startup (you'll see a line about this
on stderr, e.g. in `tmux new -s desktop -- sh -c './target/release/rust-desktop; read'`
if you want to keep the window open to read it). If clicks still don't do
anything after that:
- Confirm it actually got turned on: `tmux show-options -g mouse` should print `mouse on`.
- Make sure your terminal emulator itself supports and isn't blocking mouse
  reporting (this is virtually always on by default in modern terminals).
- As a full workaround, the desktop is also completely usable from the
  keyboard: `Tab`/`Shift+Tab`/arrow keys to select an icon, `Enter` to open it.
- Pass `--no-mouse-setup` if you manage the `mouse` tmux option yourself and
  don't want this program to touch it.

**An icon opens a window that immediately disappears again.**
A tmux window closes itself the instant its command exits. If the program
an icon tries to run isn't installed (e.g. `htop`/`top` both missing, or no
editor available), the window can flash open and vanish within
milliseconds, which looks just like the click did nothing. The bundled
default icons already fall back through a few common alternatives and print
a clear message instead of silently vanishing if none of them is
available (see the icon fields below) — if you see this with a custom icon
you added yourself, wrap its `command` in a similar
`sh -c '<program> || (echo not found; read x)'` pattern so the window stays
open to show what went wrong.

**`tmux new -s desktop rust-desktop` exits immediately / does nothing.**
This almost always means the `rust-desktop` binary isn't on your `$PATH` —
tmux can't find it, fails to launch it, and the session closes again before
you even see anything. Either use the actual path to the binary
(`tmux new -s desktop ./target/release/rust-desktop`) or install it onto
your `$PATH` first with `cargo install --path .` as shown above.

**On the raw Linux console (tty): icons aren't shown / are marked with "?" / can't be clicked or dragged, and the mouse only selects background text.**
This has two separate causes, both specific to the bare console (a real
terminal emulator doesn't have either problem):

1. *Detecting the console at all.* tmux always overrides `$TERM` **inside**
   a pane to one of its own terminfo entries (typically `tmux-256color`),
   regardless of what the real outer terminal is — so simply checking the
   pane's own `$TERM` can never tell console and terminal-emulator apart;
   both look identical from inside a pane. This version instead asks tmux
   for two things about the *attached client's* real outer terminal: its
   original terminal type (`#{client_termname}`) and the device path it's
   actually attached to (`#{client_tty}`, which is `/dev/ttyN` on a real
   console vs. `/dev/pts/N` for literally anything else, regardless of
   `$TERM`) — either one being recognised is enough. Press `i` to see not
   just the result but the raw values this was based on
   (`term=... tty=...`), which is the fastest way to tell whether tmux is
   reporting something this program doesn't recognise (in which case,
   please report the exact values shown) versus something else being wrong.
   **If detection is wrong for any reason, `--force-tty` sidesteps it
   completely** and is guaranteed to switch on the console-safe rendering
   (ASCII icons, 16 colours) and keyboard-first controls described below —
   worth trying immediately if you're not sure what's going on. A `[?]` on
   an icon specifically means the *opposite* problem — emoji support was
   detected but the icon's name doesn't start with a recognisable ASCII
   letter to fall back to; this doesn't affect clickability.
2. *Mouse input on the console itself.* Even with correct detection, the
   bare Linux console generally has **no support at all** for the
   xterm/SGR mouse-reporting protocol this program (and most other
   mouse-driven terminal programs) relies on — that protocol is a feature
   terminal emulators implement, not the kernel's own framebuffer console
   driver. Getting a mouse to do anything on the console at all normally
   requires the `gpm` daemon running with a configuration that emulates
   this protocol, which isn't something this program can set up or detect,
   and isn't present on most systems by default. Without it, dragging with
   the mouse just falls back to the console's own plain text selection —
   exactly the "only the background gets selected" behaviour described
   above. **This is why the program leads with keyboard controls whenever
   it detects the console**: the first icon is already selected on
   startup, and every mouse action has a keyboard equivalent —
   `Tab`/`Shift+Tab`/arrow keys to select, `Enter` to open, and
   `Shift+arrow keys` to move the selected icon (the equivalent of
   dragging it). None of this needs a working mouse.

**`tmux` reports `create window failed: index N in use` when opening an icon.**
This is a tmux-level race over which window index to use next, most likely
because more than one thing is creating tmux windows in the same session
around the same time — commonly a second rust-desktop instance running
somewhere else in that session (see the warning below), or tmux's own
`Ctrl-b c` binding firing at the same moment. `new_window` in `src/tmux.rs`
already retries automatically with a freshly computed free index if this
happens, so a single occurrence should just work transparently; if it
persists, check whether you have more than one rust-desktop instance
running in this session (`tmux list-windows`, or watch for the "another
rust-desktop instance seems to already be running" warning shown at
startup and in the status line).

## Keyboard shortcuts

| Key | Action |
|---|---|
| `q` / `Esc` | quit the desktop (instead cancels an open confirmation, if any) |
| `Tab` / `↓` `→` | select the next icon |
| `Shift+Tab` / `↑` `←` | select the previous icon |
| `Shift+↑↓←→` | move the selected icon (keyboard equivalent of dragging it) |
| `Enter` | open the selected icon |
| `j` / `y` | accept a pending confirmation (delete icon / empty trash) |
| `n` / `Esc` | cancel a pending confirmation |
| `r` | manually refresh the taskbar |
| `e` | empty the trash (asks for confirmation first) |
| `u` | restore the last deleted icon |
| `i` | show the detected environment (tty/terminal, colours, emoji) |
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
| `~/.config/cli-desktop/instance.lock` | pid/session/window of the currently running instance (see "Troubleshooting") |

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

The bundled default icons wrap their `command` in a `sh -c '<program> ||
<alternative> || (echo ...; read x)'` fallback chain (see
`default_icons()` in `src/config.rs`), so that if none of the preferred
programs are installed, the tmux window stays open with a clear message
instead of flashing open and closing again instantly (see "Troubleshooting"
above). Custom icons you add yourself don't get this automatically — if you
want the same safety net, wrap your own `command` the same way.

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

### Colour scheme (`colors.json`)

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

Each value is either a colour name (`black`, `red`, `green`, `yellow`,
`blue`, `magenta`, `cyan`, `white`, `grey`/`gray`, `darkgrey`/`darkgray`, as
well as `dark` variants of red/green/yellow/blue/magenta/cyan) or a hex
colour `#rrggbb`. Hex colours are automatically converted to match the
detected terminal capability: truecolor directly, 256-colour terminals round
to the nearest palette colour, and a plain tty console (only 16 colours)
safely falls back to white instead of sending a colour it can't display.

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
on your own machine, this is uncritical – it's ultimately the same trust
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
                 snap-to-grid, confirmation dialogues, theme colour resolution
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
  terminals or very many icons, this search can hit the screen edges;
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
