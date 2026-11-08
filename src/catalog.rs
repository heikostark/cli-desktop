//! Built-in catalog of well-known command-line / text-UI programs that can
//! be put on the desktop as icons via the right-click menu (see `menu.rs`).
//!
//! Each entry has a short display `name` (used as the icon label and as the
//! tmux window name), the `bin` to look for, optional fixed `args`, and a
//! one-line `desc`. Categories carry the icon `emoji` and whether their
//! programs just print something and exit (`wait`): for those, the window
//! stays open until Enter is pressed instead of vanishing instantly.

use crate::config::{sh_c, shell_quote};
use std::path::PathBuf;

pub struct App {
    pub name: &'static str,
    pub bin: &'static str,
    pub args: &'static str,
    pub desc: &'static str,
}

pub struct Category {
    pub name: &'static str,
    /// Single-codepoint, emoji-presentation glyph (so its width is a clean
    /// 2 columns); on terminals without emoji the icon falls back to `[X]`.
    pub emoji: &'static str,
    /// Programs that print something and exit (system info, quick queries).
    pub wait: bool,
    pub apps: &'static [App],
}

const fn a(name: &'static str, bin: &'static str, args: &'static str, desc: &'static str) -> App {
    App { name, bin, args, desc }
}

pub static CATALOG: &[Category] = &[
    Category {
        name: "System monitors",
        emoji: "📊",
        wait: false,
        apps: &[
            a("htop", "htop", "", "Interactive process viewer"),
            a("btop", "btop", "", "Resource monitor with graphs"),
            a("top", "top", "", "Classic process viewer"),
            a("atop", "atop", "", "Advanced system & process monitor"),
            a("glances", "glances", "", "Cross-platform system monitor"),
            a("bottom", "btm", "", "Graphical process/system monitor"),
            a("gotop", "gotop", "", "Terminal system activity monitor"),
            a("zenith", "zenith", "", "System monitor with histograms"),
            a("bpytop", "bpytop", "", "Python resource monitor"),
            a("bashtop", "bashtop", "", "Bash resource monitor"),
            a("gtop", "gtop", "", "System monitoring dashboard (Node)"),
            a("vtop", "vtop", "", "Graphical activity monitor (Node)"),
            a("ytop", "ytop", "", "TUI system monitor (Rust)"),
            a("nmon", "nmon", "", "Performance monitor (CPU, disk, net)"),
            a("below", "below", "", "Time-traveling resource monitor"),
            a("s-tui", "s-tui", "", "CPU stress & temperature monitor"),
            a("nvtop", "nvtop", "", "GPU process monitor"),
            a("radeontop", "radeontop", "", "AMD GPU utilization"),
            a("intel_gpu_top", "intel_gpu_top", "", "Intel GPU utilization"),
            a("powertop", "powertop", "", "Power consumption analyzer"),
            a("iotop", "iotop", "", "Per-process disk I/O monitor"),
            a("slabtop", "slabtop", "", "Kernel slab cache monitor"),
            a("kmon", "kmon", "", "Kernel manager & activity monitor"),
            a("csysdig", "csysdig", "", "Sysdig interactive explorer"),
            a("perf top", "perf", "top", "Live Linux profiler"),
        ],
    },
    Category {
        name: "Files & disk usage",
        emoji: "📁",
        wait: false,
        apps: &[
            a("ranger", "ranger", "", "Vi-style file manager"),
            a("mc", "mc", "", "Midnight Commander"),
            a("nnn", "nnn", "", "Fast, tiny file manager"),
            a("lf", "lf", "", "Terminal file manager (Go)"),
            a("vifm", "vifm", "", "Vi-like file manager"),
            a("yazi", "yazi", "", "Async terminal file manager"),
            a("broot", "broot", "", "Tree navigator & fuzzy finder"),
            a("xplr", "xplr", "", "Hackable file explorer"),
            a("joshuto", "joshuto", "", "Ranger-like file manager (Rust)"),
            a("clifm", "clifm", "", "Shell-like file manager"),
            a("fff", "fff", "", "Fast, minimal file manager"),
            a("felix", "felix", "", "Fast TUI file manager (Rust)"),
            a("superfile", "spf", "", "Modern, pretty file manager"),
            a("walk", "walk", "", "Terminal file navigator"),
            a("tere", "tere", "", "Terminal file explorer"),
            a("lfm", "lfm", "", "Curses file manager (Python)"),
            a("cfiles", "cfiles", "", "Minimal file manager (C)"),
            a("ncdu", "ncdu", "", "NCurses disk usage"),
            a("gdu", "gdu", "", "Fast disk usage analyzer"),
            a("dua", "dua", "i", "Interactive disk usage (dua i)"),
            a("diskonaut", "diskonaut", "", "Disk space visualizer"),
        ],
    },
    Category {
        name: "Editors",
        emoji: "📝",
        wait: false,
        apps: &[
            a("vim", "vim", "", "Vi IMproved"),
            a("nvim", "nvim", "", "Neovim"),
            a("vi", "vi", "", "Classic vi"),
            a("nano", "nano", "", "Simple, friendly editor"),
            a("micro", "micro", "", "Modern, intuitive editor"),
            a("emacs", "emacs", "-nw", "GNU Emacs (terminal mode)"),
            a("helix", "hx", "", "Post-modern modal editor"),
            a("kakoune", "kak", "", "Modal editor inspired by Vim"),
            a("joe", "joe", "", "Joe's Own Editor"),
            a("jed", "jed", "", "Programmer's editor (S-Lang)"),
            a("ne", "ne", "", "The nice editor"),
            a("mg", "mg", "", "Micro GNU Emacs"),
            a("tilde", "tilde", "", "Intuitive text editor"),
            a("vis", "vis", "", "Vim-like editor"),
            a("amp", "amp", "", "Vim-inspired editor (Rust)"),
            a("zee", "zee", "", "Modern editor for the terminal"),
            a("pico", "pico", "", "Pine composer editor"),
            a("moe", "moe", "", "Modeless text editor"),
            a("jove", "jove", "", "Emacs-like editor"),
            a("nvi", "nvi", "", "Berkeley vi"),
            a("elvis", "elvis", "", "Vi clone"),
            a("lvim", "lvim", "", "LunarVim (Neovim distribution)"),
            a("dte", "dte", "", "Small, configurable editor"),
            a("ox", "ox", "", "Independent Rust text editor"),
            a("mcedit", "mcedit", "", "Midnight Commander editor"),
            a("textadept-curses", "textadept-curses", "", "Textadept (terminal version)"),
            a("vimtutor", "vimtutor", "", "Interactive Vim tutorial"),
        ],
    },
    Category {
        name: "Git & development",
        emoji: "🔧",
        wait: false,
        apps: &[
            a("lazygit", "lazygit", "", "Simple TUI for git"),
            a("gitui", "gitui", "", "Fast TUI for git (Rust)"),
            a("tig", "tig", "", "Text-mode interface for git"),
            a("gitu", "gitu", "", "Magit-inspired git TUI"),
            a("gh dash", "gh", "dash", "GitHub PR/issue dashboard"),
            a("lazynpm", "lazynpm", "", "Simple TUI for npm"),
            a("bacon", "bacon", "", "Background Rust code checker"),
            a("lnav", "lnav", "", "Advanced log file viewer"),
            a("multitail", "multitail", "", "Tail several files at once"),
            a("posting", "posting", "", "TUI HTTP client"),
            a("wuzz", "wuzz", "", "Interactive HTTP inspector"),
            a("mitmproxy", "mitmproxy", "", "Interactive HTTPS proxy"),
            a("mprocs", "mprocs", "", "Run multiple processes (TUI)"),
            a("tokei", "tokei", "", "Count lines of code"),
        ],
    },
    Category {
        name: "Containers & cloud",
        emoji: "🐳",
        wait: false,
        apps: &[
            a("k9s", "k9s", "", "Kubernetes TUI"),
            a("kubetui", "kubetui", "", "Kubernetes TUI (multi-pane)"),
            a("kdash", "kdash", "", "Kubernetes dashboard"),
            a("ktop", "ktop", "", "kubectl top as a TUI"),
            a("lazydocker", "lazydocker", "", "Docker & compose TUI"),
            a("ctop", "ctop", "", "Container metrics, top-like"),
            a("oxker", "oxker", "", "Docker TUI (Rust)"),
            a("dry", "dry", "", "Docker manager for the terminal"),
            a("docui", "docui", "", "Docker TUI (Go)"),
            a("podman-tui", "podman-tui", "", "Podman TUI"),
            a("wtfutil", "wtfutil", "", "Personal information dashboard"),
        ],
    },
    Category {
        name: "Databases",
        emoji: "💾",
        wait: false,
        apps: &[
            a("sqlite3", "sqlite3", "", "SQLite shell"),
            a("psql", "psql", "", "PostgreSQL shell"),
            a("mysql", "mysql", "", "MySQL shell"),
            a("mariadb", "mariadb", "", "MariaDB shell"),
            a("redis-cli", "redis-cli", "", "Redis shell"),
            a("mongosh", "mongosh", "", "MongoDB shell"),
            a("duckdb", "duckdb", "", "DuckDB shell"),
            a("pgcli", "pgcli", "", "Postgres CLI with autocomplete"),
            a("mycli", "mycli", "", "MySQL CLI with autocomplete"),
            a("litecli", "litecli", "", "SQLite CLI with autocomplete"),
            a("iredis", "iredis", "", "Redis CLI with autocomplete"),
            a("usql", "usql", "", "Universal SQL client"),
            a("harlequin", "harlequin", "", "SQL IDE for the terminal"),
            a("lazysql", "lazysql", "", "TUI database client"),
            a("rainfrog", "rainfrog", "", "Database management TUI"),
            a("gobang", "gobang", "", "Cross-platform TUI database viewer"),
            a("cqlsh", "cqlsh", "", "Cassandra shell"),
            a("influx", "influx", "", "InfluxDB shell"),
        ],
    },
    Category {
        name: "Debugging & RE",
        emoji: "🐛",
        wait: false,
        apps: &[
            a("gdb tui", "gdb", "-tui", "GNU debugger with TUI"),
            a("cgdb", "cgdb", "", "Curses front-end for gdb"),
            a("lldb", "lldb", "", "LLVM debugger"),
            a("radare2", "r2", "", "Reverse-engineering framework"),
            a("rizin", "rizin", "", "Reverse-engineering framework"),
            a("hexedit", "hexedit", "", "Hex editor"),
            a("hexcurse", "hexcurse", "", "Curses hex editor"),
            a("bvi", "bvi", "", "Binary vi-like editor"),
            a("heh", "heh", "", "Terminal hex editor (Rust)"),
        ],
    },
    Category {
        name: "Interpreters & REPLs",
        emoji: "🧪",
        wait: false,
        apps: &[
            a("python3", "python3", "", "Python interpreter"),
            a("ipython", "ipython", "", "Enhanced Python shell"),
            a("bpython", "bpython", "", "Python REPL with curses UI"),
            a("ptpython", "ptpython", "", "Advanced Python REPL"),
            a("node", "node", "", "Node.js REPL"),
            a("deno", "deno", "", "Deno runtime REPL"),
            a("bun repl", "bun", "repl", "Bun JavaScript REPL"),
            a("irb", "irb", "", "Interactive Ruby"),
            a("pry", "pry", "", "Ruby REPL for power users"),
            a("ghci", "ghci", "", "Haskell interactive"),
            a("lua", "lua", "", "Lua interpreter"),
            a("luajit", "luajit", "", "LuaJIT interpreter"),
            a("julia", "julia", "", "Julia language REPL"),
            a("R", "R", "", "R statistical language"),
            a("sbcl", "sbcl", "", "Steel Bank Common Lisp"),
            a("clisp", "clisp", "", "GNU CLISP"),
            a("guile", "guile", "", "GNU Guile Scheme"),
            a("racket", "racket", "", "Racket language REPL"),
            a("ocaml", "ocaml", "", "OCaml toplevel"),
            a("utop", "utop", "", "Universal OCaml toplevel"),
            a("erl", "erl", "", "Erlang shell"),
            a("iex", "iex", "", "Elixir shell"),
            a("scala", "scala", "", "Scala REPL"),
            a("jshell", "jshell", "", "Java REPL"),
            a("php -a", "php", "-a", "PHP interactive shell"),
            a("swipl", "swipl", "", "SWI-Prolog"),
            a("tclsh", "tclsh", "", "Tcl shell"),
            a("csi", "csi", "", "CHICKEN Scheme"),
            a("mit-scheme", "mit-scheme", "", "MIT/GNU Scheme"),
            a("gforth", "gforth", "", "GNU Forth"),
            a("evcxr", "evcxr", "", "Rust REPL"),
            a("perl -d", "perl", "-de0", "Perl debugger as a REPL"),
            a("fsi", "dotnet", "fsi", "F# interactive"),
            a("crystal", "crystal", "i", "Crystal REPL"),
            a("clj", "clj", "", "Clojure CLI REPL"),
            a("lein repl", "lein", "repl", "Clojure REPL (Leiningen)"),
            a("elm repl", "elm", "repl", "Elm REPL"),
            a("idris2", "idris2", "", "Idris 2 REPL"),
            a("raku", "raku", "", "Raku REPL"),
        ],
    },
    Category {
        name: "Shells",
        emoji: "🐚",
        wait: false,
        apps: &[
            a("bash", "bash", "", "GNU Bourne-Again shell"),
            a("zsh", "zsh", "", "Z shell"),
            a("fish", "fish", "", "Friendly interactive shell"),
            a("nushell", "nu", "", "Structured-data shell"),
            a("elvish", "elvish", "", "Expressive shell"),
            a("xonsh", "xonsh", "", "Python-powered shell"),
            a("ksh", "ksh", "", "KornShell"),
            a("mksh", "mksh", "", "MirBSD Korn shell"),
            a("tcsh", "tcsh", "", "Enhanced C shell"),
            a("dash", "dash", "", "Minimal POSIX shell"),
            a("yash", "yash", "", "Yet another shell"),
            a("ion", "ion", "", "Redox shell"),
            a("pwsh", "pwsh", "", "PowerShell"),
            a("osh", "osh", "", "Oils shell (OSH)"),
            a("murex", "murex", "", "Typed, scriptable shell"),
            a("busybox sh", "busybox", "sh", "BusyBox shell"),
        ],
    },
    Category {
        name: "Web browsers",
        emoji: "🌍",
        wait: false,
        apps: &[
            a("lynx", "lynx", "", "Classic text web browser"),
            a("links", "links", "", "Text web browser with menus"),
            a("w3m", "w3m", "", "Text browser with image support"),
            a("elinks", "elinks", "", "Feature-rich text browser"),
            a("browsh", "browsh", "", "Modern browser in the terminal"),
            a("carbonyl", "carbonyl", "", "Chromium in the terminal"),
            a("amfora", "amfora", "", "Gemini browser"),
            a("bombadillo", "bombadillo", "", "Gopher/Gemini client"),
            a("offpunk", "offpunk", "", "Offline-first Gemini/web reader"),
            a("circumflex", "clx", "", "Hacker News in the terminal"),
            a("edbrowse", "edbrowse", "", "Line-oriented browser/editor"),
        ],
    },
    Category {
        name: "Network & torrents",
        emoji: "📶",
        wait: false,
        apps: &[
            a("nmtui", "nmtui", "", "NetworkManager text UI"),
            a("wavemon", "wavemon", "", "Wireless network monitor"),
            a("bluetuith", "bluetuith", "", "Bluetooth manager TUI"),
            a("impala", "impala", "", "Wi-Fi manager TUI (iwd)"),
            a("iwctl", "iwctl", "", "iwd interactive client"),
            a("bluetoothctl", "bluetoothctl", "", "Bluetooth control shell"),
            a("connmanctl", "connmanctl", "", "ConnMan control shell"),
            a("mtr", "mtr", "", "Traceroute + ping combined"),
            a("trippy", "trip", "", "Network diagnostic tool"),
            a("gping", "gping", "", "Ping with a graph"),
            a("iftop", "iftop", "", "Bandwidth usage per connection"),
            a("nethogs", "nethogs", "", "Bandwidth usage per process"),
            a("nload", "nload", "", "Network traffic monitor"),
            a("bmon", "bmon", "", "Bandwidth monitor"),
            a("iptraf-ng", "iptraf-ng", "", "IP traffic monitor"),
            a("termshark", "termshark", "", "Wireshark-style packet TUI"),
            a("bandwhich", "bandwhich", "", "Bandwidth by process/host"),
            a("netscanner", "netscanner", "", "Network scanner TUI"),
            a("tcptrack", "tcptrack", "", "TCP connection tracker"),
            a("slurm", "slurm", "", "Network load monitor"),
            a("rtorrent", "rtorrent", "", "Ncurses BitTorrent client"),
            a("stig", "stig", "", "Transmission TUI"),
            a("tremc", "tremc", "", "Transmission curses client"),
            a("deluge-console", "deluge-console", "", "Deluge console client"),
        ],
    },
    Category {
        name: "Remote & transfer",
        emoji: "📡",
        wait: false,
        apps: &[
            a("lftp", "lftp", "", "Sophisticated FTP/SFTP client"),
            a("ncftp", "ncftp", "", "NcFTP client"),
            a("termscp", "termscp", "", "Terminal file transfer & explorer"),
            a("sshs", "sshs", "", "SSH host picker TUI"),
            a("tmate", "tmate", "", "Instant terminal sharing"),
            a("rclone config", "rclone", "config", "rclone setup wizard"),
        ],
    },
    Category {
        name: "Chat & social",
        emoji: "💬",
        wait: false,
        apps: &[
            a("irssi", "irssi", "", "IRC client"),
            a("weechat", "weechat", "", "Extensible chat client"),
            a("profanity", "profanity", "", "XMPP/Jabber client"),
            a("gomuks", "gomuks", "", "Matrix client"),
            a("iamb", "iamb", "", "Matrix client (Vim-like)"),
            a("nchat", "nchat", "", "Telegram & WhatsApp client"),
            a("tgt", "tgt", "", "Telegram TUI"),
            a("tut", "tut", "", "Mastodon TUI"),
            a("toot tui", "toot", "tui", "Mastodon TUI (toot)"),
            a("tuir", "tuir", "", "Reddit browser"),
            a("rtv", "rtv", "", "Reddit terminal viewer"),
            a("slack-term", "slack-term", "", "Slack client"),
            a("discordo", "discordo", "", "Discord client"),
            a("cordless", "cordless", "", "Discord client"),
            a("finch", "finch", "", "Pidgin console client"),
            a("senpai", "senpai", "", "Modern IRC client"),
            a("catgirl", "catgirl", "", "TLS-only IRC client"),
            a("scli", "scli", "", "Signal client"),
            a("gurk", "gurk", "", "Signal client (Rust)"),
        ],
    },
    Category {
        name: "Email & news",
        emoji: "📧",
        wait: false,
        apps: &[
            a("mutt", "mutt", "", "Classic mail client"),
            a("neomutt", "neomutt", "", "Mutt with extra features"),
            a("alpine", "alpine", "", "Friendly mail & news client"),
            a("pine", "pine", "", "Original Pine mail client"),
            a("aerc", "aerc", "", "Email client for the terminal"),
            a("meli", "meli", "", "Terminal email client"),
            a("himalaya", "himalaya", "", "Email client (CLI/TUI)"),
            a("tin", "tin", "", "Usenet newsreader"),
            a("slrn", "slrn", "", "Usenet newsreader"),
            a("newsboat", "newsboat", "", "RSS/Atom feed reader"),
            a("canto-curses", "canto-curses", "", "RSS reader (curses)"),
            a("nom", "nom", "", "RSS reader TUI"),
            a("russ", "russ", "", "RSS reader TUI"),
            a("snownews", "snownews", "", "Text-mode RSS reader"),
        ],
    },
    Category {
        name: "Music & audio",
        emoji: "🎵",
        wait: false,
        apps: &[
            a("cmus", "cmus", "", "Small, fast music player"),
            a("moc", "mocp", "", "Music on Console"),
            a("ncmpcpp", "ncmpcpp", "", "MPD client with many features"),
            a("ncmpc", "ncmpc", "", "Curses MPD client"),
            a("vimpc", "vimpc", "", "Vi-like MPD client"),
            a("musikcube", "musikcube", "", "Terminal audio player/streamer"),
            a("ncspot", "ncspot", "", "Spotify client"),
            a("spotify_player", "spotify_player", "", "Spotify client (Rust)"),
            a("spt", "spt", "", "Spotify TUI"),
            a("termusic", "termusic", "", "Music player (Rust)"),
            a("kew", "kew", "", "Music player for the terminal"),
            a("cava", "cava", "", "Audio visualizer"),
            a("pianobar", "pianobar", "", "Pandora client"),
            a("vlc ncurses", "vlc", "-I ncurses", "VLC with ncurses interface"),
            a("tizonia", "tizonia", "", "Cloud music streaming"),
            a("mpsyt", "mpsyt", "", "YouTube search & playback"),
            a("jellyfin-tui", "jellyfin-tui", "", "Jellyfin music client"),
            a("youtube-tui", "youtube-tui", "", "YouTube TUI"),
            a("ytermusic", "ytermusic", "", "YouTube Music TUI"),
            a("alsamixer", "alsamixer", "", "ALSA sound mixer"),
            a("pulsemixer", "pulsemixer", "", "PulseAudio mixer"),
            a("ncpamixer", "ncpamixer", "", "PulseAudio ncurses mixer"),
            a("wiremix", "wiremix", "", "PipeWire mixer TUI"),
            a("aumix", "aumix", "", "Ncurses audio mixer"),
        ],
    },
    Category {
        name: "Productivity & finance",
        emoji: "📅",
        wait: false,
        apps: &[
            a("calcurse", "calcurse", "", "Calendar & scheduling"),
            a("ikhal", "ikhal", "", "Calendar (khal interactive)"),
            a("calcure", "calcure", "", "Modern calendar & task manager"),
            a("wyrd", "wyrd", "", "Remind calendar front-end"),
            a("taskwarrior-tui", "taskwarrior-tui", "", "Taskwarrior TUI"),
            a("vit", "vit", "", "Visual Interactive Taskwarrior"),
            a("taskell", "taskell", "", "Kanban boards in the terminal"),
            a("dijo", "dijo", "", "Habit tracker"),
            a("tjournal", "tjournal", "", "Terminal journal"),
            a("jrnl", "jrnl", "", "Command-line journal"),
            a("sc-im", "sc-im", "", "Spreadsheet calculator"),
            a("visidata", "vd", "", "Data explorer (CSV, JSON, SQL)"),
            a("hledger-ui", "hledger-ui", "", "Plain-text accounting TUI"),
            a("cointop", "cointop", "", "Cryptocurrency tracker"),
            a("tickrs", "tickrs", "", "Realtime stock ticker"),
            a("ticker", "ticker", "", "Stock ticker in the terminal"),
            a("abook", "abook", "", "Address book"),
        ],
    },
    Category {
        name: "Calculators & math",
        emoji: "🧮",
        wait: false,
        apps: &[
            a("bc", "bc", "", "Arbitrary precision calculator"),
            a("dc", "dc", "", "Reverse-Polish calculator"),
            a("qalc", "qalc", "", "Qalculate! command line"),
            a("kalker", "kalker", "", "Scientific calculator"),
            a("calc", "calc", "", "Arbitrary precision calculator"),
            a("insect", "insect", "", "Calculator with units"),
            a("numbat", "numbat", "", "Calculator with physical units"),
            a("eva", "eva", "", "Simple calculator REPL"),
            a("units", "units", "", "Unit conversion"),
            a("wcalc", "wcalc", "", "Flexible command-line calculator"),
            a("maxima", "maxima", "", "Computer algebra system"),
            a("gnuplot", "gnuplot", "", "Plotting program"),
            a("octave", "octave-cli", "", "GNU Octave (numerical computing)"),
            a("gp", "gp", "", "PARI/GP number theory"),
            a("sage", "sage", "", "SageMath"),
            a("gap", "gap", "", "Computational discrete algebra"),
        ],
    },
    Category {
        name: "Reading & reference",
        emoji: "📖",
        wait: false,
        apps: &[
            a("info", "info", "", "GNU Info documentation browser"),
            a("pinfo", "pinfo", "", "Lynx-style info browser"),
            a("glow", "glow", "", "Markdown reader/browser"),
            a("navi", "navi", "", "Interactive cheatsheets"),
            a("wiki-tui", "wiki-tui", "", "Wikipedia in the terminal"),
            a("sdcv", "sdcv", "", "StarDict console dictionary"),
            a("trans", "trans", "-shell", "Translate-shell interactive"),
        ],
    },
    Category {
        name: "Games",
        emoji: "🎮",
        wait: false,
        apps: &[
            a("nethack", "nethack", "", "Classic roguelike"),
            a("crawl", "crawl", "", "Dungeon Crawl Stone Soup"),
            a("angband", "angband", "", "Tolkien-themed roguelike"),
            a("brogue", "brogue", "", "Roguelike dungeon crawler"),
            a("rogue", "rogue", "", "The original roguelike"),
            a("hack", "hack", "", "NetHack's ancestor"),
            a("moria", "moria", "", "Classic dungeon game"),
            a("cataclysm", "cataclysm", "", "Cataclysm: Dark Days Ahead"),
            a("dwarffortress", "dwarffortress", "", "Dwarf Fortress"),
            a("nsnake", "nsnake", "", "Snake game"),
            a("ninvaders", "ninvaders", "", "Space Invaders"),
            a("bastet", "bastet", "", "Tetris that picks bad pieces"),
            a("vitetris", "vitetris", "", "Tetris clone"),
            a("tint", "tint", "", "Tetris clone"),
            a("tetris-bsd", "tetris-bsd", "", "Tetris (BSD games)"),
            a("pacman4console", "pacman4console", "", "Pac-Man"),
            a("nudoku", "nudoku", "", "Sudoku"),
            a("gnuchess", "gnuchess", "", "GNU Chess"),
            a("gnugo", "gnugo", "", "GNU Go"),
            a("moon-buggy", "moon-buggy", "", "Drive a buggy on the moon"),
            a("greed", "greed", "", "Number grid game"),
            a("freesweep", "freesweep", "", "Minesweeper"),
            a("bs", "bs", "", "Battleship"),
            a("hangman", "hangman", "", "Guess the word"),
            a("adventure", "adventure", "", "Colossal Cave Adventure"),
            a("atc", "atc", "", "Air traffic controller"),
            a("backgammon", "backgammon", "", "Backgammon"),
            a("battlestar", "battlestar", "", "Adventure game"),
            a("boggle", "boggle", "", "Word search game"),
            a("canfield", "canfield", "", "Canfield solitaire"),
            a("cribbage", "cribbage", "", "Cribbage card game"),
            a("hunt", "hunt", "", "Multiplayer maze shooter"),
            a("mille", "mille", "", "Mille Bornes card game"),
            a("monop", "monop", "", "Monopoly"),
            a("phantasia", "phantasia", "", "Fantasy role-playing"),
            a("quiz", "quiz", "", "Quiz games"),
            a("robots", "robots", "", "Escape from the robots"),
            a("sail", "sail", "", "Naval warfare"),
            a("snake", "snake", "", "Snake (BSD games)"),
            a("trek", "trek", "", "Star Trek game"),
            a("worm", "worm", "", "Worm game"),
            a("wump", "wump", "", "Hunt the Wumpus"),
            a("gtypist", "gtypist", "", "Typing tutor"),
            a("ttyper", "ttyper", "", "Typing test"),
            a("typespeed", "typespeed", "", "Typing speed game"),
            a("toipe", "toipe", "", "Typing test"),
        ],
    },
    Category {
        name: "Fun & eye candy",
        emoji: "✨",
        wait: false,
        apps: &[
            a("cmatrix", "cmatrix", "", "Matrix digital rain"),
            a("neo", "neo", "", "Matrix digital rain (neo)"),
            a("unimatrix", "unimatrix", "", "Matrix rain (Python)"),
            a("sl", "sl", "", "Steam locomotive"),
            a("asciiquarium", "asciiquarium", "", "ASCII aquarium"),
            a("cbonsai", "cbonsai", "", "Growing bonsai tree"),
            a("pipes.sh", "pipes.sh", "", "Animated pipes screensaver"),
            a("tty-clock", "tty-clock", "", "Digital clock"),
            a("peaclock", "peaclock", "", "Customizable clock"),
            a("hollywood", "hollywood", "", "Hollywood hacker screen"),
            a("nyancat", "nyancat", "", "Nyan Cat"),
            a("genact", "genact", "", "Fake activity generator"),
            a("cacafire", "cacafire", "", "Fire demo (libcaca)"),
            a("aafire", "aafire", "", "Fire demo (aalib)"),
            a("bb", "bb", "", "Demo (aalib)"),
            a("lavat", "lavat", "", "Lava lamp"),
            a("termsaver", "termsaver", "", "Terminal screensavers"),
        ],
    },
    Category {
        name: "Terminal tools",
        emoji: "🧰",
        wait: false,
        apps: &[
            a("zellij", "zellij", "", "Terminal workspace"),
            a("screen", "screen", "", "GNU Screen multiplexer"),
            a("byobu", "byobu", "", "Enhanced tmux/screen wrapper"),
            a("dvtm", "dvtm", "", "Tiling terminal multiplexer"),
            a("mtm", "mtm", "", "Minimal terminal multiplexer"),
            a("fzf", "fzf", "", "Fuzzy finder (file picker)"),
            a("vtm", "vtm", "", "Text-based desktop environment"),
        ],
    },
    Category {
        name: "System administration",
        emoji: "🔩",
        wait: false,
        apps: &[
            a("aptitude", "aptitude", "", "Debian package manager (TUI)"),
            a("pacseek", "pacseek", "", "Arch package browser (TUI)"),
            a("cgdisk", "cgdisk", "", "GPT partition editor (curses)"),
            a("cfdisk", "cfdisk", "", "Partition editor (curses)"),
            a("raspi-config", "raspi-config", "", "Raspberry Pi configuration"),
            a("armbian-config", "armbian-config", "", "Armbian configuration"),
            a("tzselect", "tzselect", "", "Time zone selector"),
            a("systemctl-tui", "systemctl-tui", "", "systemd services TUI"),
            a("sysz", "sysz", "", "fzf front-end for systemd"),
            a("lazyjournal", "lazyjournal", "", "Journal & log browser TUI"),
            a("crontab", "crontab", "-e", "Edit your crontab"),
        ],
    },
    Category {
        name: "Quick info (prints & waits)",
        emoji: "💡",
        wait: true,
        apps: &[
            a("fastfetch", "fastfetch", "", "System information (fast)"),
            a("neofetch", "neofetch", "", "System information"),
            a("screenfetch", "screenfetch", "", "System information"),
            a("hyfetch", "hyfetch", "", "neofetch with pride flags"),
            a("macchina", "macchina", "", "Fast system info (Rust)"),
            a("cpufetch", "cpufetch", "", "CPU architecture info"),
            a("inxi", "inxi", "-F", "Detailed hardware & system info"),
            a("lscpu", "lscpu", "", "CPU details"),
            a("lsblk", "lsblk", "", "Block devices"),
            a("disk free", "df", "-h", "Disk space usage (df -h)"),
            a("memory", "free", "-h", "Memory usage (free -h)"),
            a("uptime", "uptime", "", "Uptime & load average"),
            a("net addrs", "ip", "-br address", "Network addresses (ip -br a)"),
            a("sockets", "ss", "-tulpn", "Listening sockets (ss -tulpn)"),
            a("lsusb", "lsusb", "", "USB devices"),
            a("lspci", "lspci", "", "PCI devices"),
            a("kernel", "uname", "-a", "Kernel & OS (uname -a)"),
            a("who", "w", "", "Who is logged in"),
            a("last logins", "last", "-n 20", "Recent logins"),
            a("calendar", "cal", "-3", "Three-month calendar"),
            a("fortune", "fortune", "", "Random quote"),
            a("weather", "curl", "-s wttr.in", "Weather via wttr.in (internet)"),
            a("timedate", "timedatectl", "", "Date, time & time zone"),
            a("hostnamectl", "hostnamectl", "", "Host & OS information"),
            a("failed units", "systemctl", "--failed", "Failed systemd units"),
        ],
    },
];

/// Total number of programs in the catalog.
pub fn total_apps() -> usize {
    CATALOG.iter().map(|c| c.apps.len()).sum()
}

/// Directories searched for programs: `$PATH` plus a few places tools are
/// commonly installed that often aren't on the PATH of a tmux server or a
/// non-login shell (BSD games, cargo/pip user installs, sbin tools).
/// The generated start commands extend `PATH` the same way, so what's
/// detected as installed is also what the icon can actually launch.
pub fn search_dirs() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    let home = std::env::var_os("HOME").map(PathBuf::from);
    for extra in ["/usr/games", "/usr/local/games", "/usr/sbin", "/sbin"] {
        dirs.push(PathBuf::from(extra));
    }
    if let Some(h) = home {
        dirs.push(h.join(".cargo/bin"));
        dirs.push(h.join(".local/bin"));
    }
    dirs
}

#[cfg(unix)]
fn is_executable_file(p: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(p)
        .map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(p: &std::path::Path) -> bool {
    p.is_file()
}

pub fn is_installed(bin: &str, dirs: &[PathBuf]) -> bool {
    dirs.iter().any(|d| is_executable_file(&d.join(bin)))
}

/// For every catalog entry, whether its program was found (same shape as `CATALOG`).
pub fn installed_flags() -> Vec<Vec<bool>> {
    let dirs = search_dirs();
    CATALOG
        .iter()
        .map(|c| c.apps.iter().map(|a| is_installed(a.bin, &dirs)).collect())
        .collect()
}

/// The shell command an icon for `app` runs in its tmux window.
///
/// - If the program doesn't exist, a clear message with an install hint is
///   shown and the window waits for Enter (instead of flashing shut).
/// - For programs that print and exit (`cat.wait`), the window waits for
///   Enter after they finish so the output can be read.
/// - Otherwise, if the program fails within 3 seconds (e.g. "must be root",
///   "no display"), its exit status is shown and the window waits for Enter,
///   so the error can actually be read. A normal exit just closes the window.
///
/// Everything is one `sh -c '...'` script, so it's parsed by POSIX `sh`
/// regardless of the user's login shell (see `config::sh_c`).
pub fn command_for(cat: &Category, app: &App) -> String {
    let run = if app.args.is_empty() {
        app.bin.to_string()
    } else {
        format!("{} {}", app.bin, app.args)
    };
    let after = if cat.wait {
        "echo; echo 'Press Enter to close.'; read x; ".to_string()
    } else {
        String::new()
    };
    let watch = if cat.wait {
        format!("{run}; {after}")
    } else {
        format!(
            "s=$(date +%s); {run}; rc=$?; \
             if [ \"$rc\" -ne 0 ] && [ $(( $(date +%s) - s )) -lt 3 ]; then \
             echo; echo \"Exited with status $rc.\"; echo 'Press Enter to close.'; read x; fi; "
        )
    };
    let script = format!(
        "PATH=\"$PATH:/usr/games:/usr/local/games:$HOME/.cargo/bin:$HOME/.local/bin:/usr/sbin:/sbin\"; export PATH; \
         if command -v {q} >/dev/null 2>&1; then {watch}else \
         echo {msg}; echo 'Install it with your package manager, then open this icon again.'; \
         echo 'Press Enter to close.'; read x; fi",
        q = shell_quote(app.bin),
        msg = shell_quote(&format!("{} is not installed.", app.name)),
    );
    sh_c(&script)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    use std::process::Command;

    #[test]
    fn catalog_is_large_and_well_formed() {
        assert!(total_apps() >= 400, "only {} programs", total_apps());
        let mut names = HashSet::new();
        for c in CATALOG {
            assert!(!c.apps.is_empty(), "empty category {}", c.name);
            assert!(!c.emoji.is_empty());
            for a in c.apps {
                assert!(!a.name.is_empty() && !a.bin.is_empty() && !a.desc.is_empty(), "{:?}", a.name);
                assert!(a.name.len() <= 16, "name too long: {}", a.name);
                assert!(a.desc.len() <= 36, "desc too long: {} ({})", a.desc, a.desc.len());
                assert!(!a.bin.contains(char::is_whitespace), "bin has spaces: {}", a.bin);
                assert!(names.insert(a.name), "duplicate name: {}", a.name);
            }
        }
    }

    #[test]
    fn every_generated_command_is_valid_shell_syntax() {
        for c in CATALOG {
            for a in c.apps {
                let cmd = command_for(c, a);
                // `sh -n` only parses, it doesn't run anything.
                let out = Command::new("sh").arg("-n").arg("-c").arg(&cmd).output().unwrap();
                assert!(
                    out.status.success(),
                    "syntax error for {}: {}\n{}",
                    a.name,
                    String::from_utf8_lossy(&out.stderr),
                    cmd
                );
            }
        }
    }
}
