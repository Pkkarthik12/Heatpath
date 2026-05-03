# Heatpath

> Watch your filesystem. See where you actually work.

![Rust](https://img.shields.io/badge/Rust-1.78-orange?style=flat-square&logo=rust)
![License](https://img.shields.io/badge/license-MIT-green?style=flat-square)
![Platform](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-lightgrey?style=flat-square)
![Status](https://img.shields.io/badge/status-beta-yellow?style=flat-square)

---

Most complexity metrics measure lines of code. Heatpath measures something more honest: **where you actually spend your time**.

It watches your filesystem in real time, logs every file open, edit, and save, and renders a live color-coded heatmap of your project directly in the terminal. After a week of use you'll see exactly which files are load-bearing and which are just big.

```
╔══════════════════════════════════════════════════════════════════╗
║  heatpath  ·  ~/projects/myapp  ·  session: 4h 12m             ║
╠══════════════════════════════════════════════════════════════════╣
║                                                                  ║
║  src/                                                            ║
║  ├── [██████████] handlers/auth.rs         · 47 touches  HOT    ║
║  ├── [████████░░] handlers/payments.rs     · 38 touches         ║
║  ├── [█████░░░░░] models/user.rs           · 24 touches         ║
║  ├── [██░░░░░░░░] utils/retry.rs           ·  9 touches         ║
║  ├── [░░░░░░░░░░] config/defaults.rs       ·  1 touch   COLD    ║
║  └── [░░░░░░░░░░] migrations/001_init.sql  ·  0 touches         ║
║                                                                  ║
║  [s] session  [l] lifetime  [g] git-weighted  [q] quit          ║
╚══════════════════════════════════════════════════════════════════╝
```

---

## Why this exists

Code coverage tells you what's tested. LOC counters tell you what's big. Neither tells you what's *risky* — the file that breaks everything when touched, the one three people edit every sprint, the one nobody has opened since 2022.

Heatpath gives you that missing signal, passively, just by watching you work.

---

## Features

- **Real-time filesystem watcher** — uses OS-native events (FSEvents on macOS, inotify on Linux) via `notify-rs`. Zero polling, zero CPU overhead.
- **Terminal treemap UI** — built with Ratatui. Colour-coded from cold blue (untouched) to hot red (hammered constantly). Resizes with your terminal.
- **Session vs. lifetime mode** — toggle between today's activity and your all-time heatmap. Useful for spotting when a "finished" area of code suddenly gets hot again.
- **Git-aware decay** — files untouched for 30 days gradually fade. Files with recent commits get a boost. Makes the heatmap reflect current reality, not ancient history.
- **Respects .gitignore** — auto-excludes build artifacts, `node_modules`, `target/`, `.git/`, and any custom patterns you add.
- **JSON and CSV export** — pipe data into Grafana, a spreadsheet, or your own scripts.
- **Low overhead** — written in Rust, uses SQLite as its backing store. Runs permanently in the background on 0–1% CPU.

---

## Installation

### From source (recommended for now)

```bash
git clone https://github.com/yourusername/heatpath.git
cd heatpath
cargo build --release
cp target/release/heatpath ~/.local/bin/
```

Requires Rust 1.78+. Install via [rustup](https://rustup.rs) if needed.

### Homebrew (coming soon)

```bash
brew install yourusername/tap/heatpath
```

### Cargo

```bash
cargo install heatpath
```

---

## Quick start

```bash
# Start watching the current directory
heatpath watch .

# In another terminal, open the live dashboard
heatpath ui

# Or watch and open UI in one command
heatpath watch . --ui
```

That's it. Go work normally. The heatmap builds itself.

---

## Commands

### `heatpath watch <path>`

Start the background watcher for a directory.

```bash
heatpath watch ~/projects/myapp
heatpath watch . --ignore "*.test.ts" --ignore "docs/"
heatpath watch . --no-gitignore   # disable auto gitignore parsing
```

| Flag | Description |
|------|-------------|
| `--ignore <pattern>` | Extra glob patterns to exclude. Repeatable. |
| `--no-gitignore` | Don't auto-read `.gitignore` files. |
| `--db <path>` | Custom path to the SQLite database file. |
| `--ui` | Open the terminal UI immediately after starting. |

---

### `heatpath ui`

Open the live terminal dashboard.

```bash
heatpath ui
heatpath ui --mode lifetime    # start in lifetime view instead of session
heatpath ui --sort touches     # sort by touch count (default)
heatpath ui --sort recency     # sort by most recently touched
heatpath ui --depth 3          # max directory depth to show
```

**Keyboard shortcuts in the UI:**

| Key | Action |
|-----|--------|
| `s` | Switch to session view |
| `l` | Switch to lifetime view |
| `g` | Switch to git-weighted view |
| `↑ / ↓` | Navigate file list |
| `→` | Expand directory |
| `←` | Collapse directory |
| `r` | Reset session data |
| `e` | Export current view to JSON |
| `q` | Quit |

---

### `heatpath stats`

Print a summary to stdout — useful for scripts and CI.

```bash
heatpath stats
heatpath stats --top 10           # show top 10 hottest files
heatpath stats --since "7 days"   # restrict to the last week
heatpath stats --format json      # machine-readable output
heatpath stats --format csv       # spreadsheet-friendly
```

Example output:

```
Top files (last 7 days)
───────────────────────────────────────────
 1.  src/handlers/auth.rs          47 touches
 2.  src/handlers/payments.rs      38 touches
 3.  src/models/user.rs            24 touches
 4.  src/utils/retry.rs             9 touches
 5.  src/config/defaults.rs         1 touch
───────────────────────────────────────────
Total files tracked: 84
Session started: 2026-04-25 09:12
```

---

### `heatpath stop`

Stop the background watcher.

```bash
heatpath stop
heatpath stop --purge   # stop and delete all recorded data
```

---

## How the scoring works

Each file has a **heat score** computed from three signals:

```
heat = (touch_count × recency_weight) + git_commit_boost − decay
```

- **touch_count** — raw number of times the file was opened or saved in the window.
- **recency_weight** — touches in the last 24 hours count double; the last 7 days count 1.5×; older touches count 1×.
- **git_commit_boost** — if the file has been committed to in the last 14 days, it gets a +20% boost. This catches files that are touched in bursts rather than constantly.
- **decay** — files not touched for 30+ days lose 10% of their score per week until they reach a floor.

You can disable git weighting (`--no-git`) or change the decay window (`--decay-days 60`) if the defaults don't fit your workflow.

---

## Project structure

```
heatpath/
├── src/
│   ├── main.rs            
│   ├── watcher/
│   │   ├── mod.rs          
│   │   ├── events.rs     
│   │   └── filter.rs      
│   ├── db/
│   │   ├── mod.rs       
│   │   ├── schema.sql      
│   │   └── queries.rs      
│   ├── scoring/
│   │   ├── mod.rs         
│   │   ├── decay.rs        
│   │   └── git.rs         
│   ├── ui/
│   │   ├── mod.rs         
│   │   ├── treemap.rs      
│   │   └── colours.rs      # Heat to colour mapping
│   └── export.rs           # JSON / CSV serialisation
├── tests/
│   ├── watcher_test.rs
│   ├── scoring_test.rs
│   └── fixtures/           # Sample file trees for tests
├── Cargo.toml
├── Cargo.lock
└── README.md
```

---

## Configuration

Heatpath reads from `~/.config/heatpath/config.toml` if it exists. All values are optional.

```toml
[defaults]
depth = 4              # default tree depth in UI
sort = "touches"       # "touches" | "recency" | "git"
mode = "session"       # "session" | "lifetime"

[decay]
enabled = true
window_days = 30       # start decaying after this many days untouched
rate = 0.10            # decay 10% per week after the window

[git]
enabled = true
commit_boost = 0.20    # percentage boost for recently committed files
lookback_days = 14     # how far back to check git log

[ignore]
patterns = [
  "*.lock",
  "*.log",
  "dist/",
  ".DS_Store",
]
```

---

## Data storage

All data is stored in a local SQLite database at `~/.local/share/heatpath/data.db` by default. Nothing leaves your machine.

Schema overview:

```sql
CREATE TABLE projects (
  id         INTEGER PRIMARY KEY,
  path       TEXT NOT NULL UNIQUE,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE events (
  id          INTEGER PRIMARY KEY,
  project_id  INTEGER REFERENCES projects(id),
  filepath    TEXT NOT NULL,
  event_type  TEXT NOT NULL,  -- 'open' | 'save' | 'delete'
  occurred_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE scores (
  project_id INTEGER REFERENCES projects(id),
  filepath   TEXT NOT NULL,
  score      REAL NOT NULL,
  updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
  PRIMARY KEY (project_id, filepath)
);
```

Scores are recomputed on a background thread every 60 seconds, not on every event, so the UI stays fast even in large projects.

---

## Export examples

```bash
# Export JSON for a Grafana dashboard
heatpath stats --format json --since "30 days" > heatmap.json

# Pipe into jq to find files touched every day this week
heatpath stats --format json | jq '[.files[] | select(.daily_touches >= 5)]'

# CSV for a spreadsheet
heatpath stats --format csv --top 50 > heatmap.csv
```

---

## Roadmap

- [ ] VS Code extension — show heat score inline next to file names in the explorer
- [ ] Team mode — aggregate heatmaps from multiple developers (opt-in, self-hosted)
- [ ] Heatpath diff — compare heatmaps between two time windows ("what changed this sprint?")
- [ ] Windows support (ReadDirectoryChangesW)
- [ ] Web UI option for people who prefer a browser view
- [ ] `heatpath explain <file>` — narrate why a file is hot or cold

---

## Contributing

1. Fork and clone the repo
2. `cargo build` to confirm it compiles
3. `cargo test` — all tests must pass before opening a PR
4. `cargo clippy -- -D warnings` — zero clippy warnings policy
5. Open a PR with a clear description

For larger changes, open an issue first to discuss the approach. The codebase is deliberately small and focused — features that belong in a separate tool should be separate tools.

---

## Similar tools (and why Heatpath is different)

| Tool | What it does | What's missing |
|------|-------------|----------------|
| `git log --stat` | Shows commit frequency per file | Only captures commits, not all editing activity |
| Code coverage tools | Shows tested lines | Measures tests, not development effort |
| IDE heatmaps | Some IDEs show edit frequency | Locked to one editor, no terminal view, no history |
| `fatigue` (npm) | Identifies complex files by LOC | Static analysis, not runtime activity |

Heatpath is editor-agnostic (watches the filesystem, not a plugin), terminal-native, and captures the full picture of where your hands actually go.

---

## License

MIT. See [LICENSE](LICENSE).

---


