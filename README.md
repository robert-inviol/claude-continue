# claude-sessions

Interactive TUI explorer for [Claude Code](https://docs.anthropic.com/en/docs/claude-code) sessions.

Browse, search, and resume your Claude Code sessions from the terminal.

![Rust](https://img.shields.io/badge/rust-%23000000.svg?style=flat&logo=rust&logoColor=white)

## Features

- **Browse by project** — sessions grouped by working directory
- **Recent view** — flat list of all sessions sorted by last activity
- **Fuzzy filter** — type `/` to filter by message content, session ID, or path
- **Session details** — model, message counts, timestamps, file size
- **Conversation viewer** — read the full conversation in-terminal
- **Resume sessions** — press Enter to launch `claude --resume`
- **Delete sessions** — clean up old sessions interactively
- **Clipboard** — copy session IDs (wl-copy/xclip/xsel)
- **CLI arguments** — `--search`, `--id`, or bare query for quick access
- **Catppuccin Mocha** colour scheme

## Install

### From source (Rust)

```sh
cargo build --release
cp target/release/claude-sessions-tui ~/.local/bin/claude-sessions
```

### Bash version (original)

The original bash script is included as `claude-sessions` for reference. It requires `gum`, `fzf`, and `python3`.

```sh
cp claude-sessions ~/.local/bin/claude-sessions
```

## Usage

```
claude-sessions              # interactive browser (scoped to cwd if applicable)
claude-sessions --search     # search all sessions
claude-sessions --search foo # search with initial query
claude-sessions --id abc123  # look up session by ID fragment
claude-sessions abc123       # shorthand for --id / --search
```

## Keybindings

| Key | Action |
|-----|--------|
| `j` / `k` or arrows | Navigate |
| `Enter` | Open project / Resume session |
| `Right` / `l` | Session details |
| `Delete` / `d` | Delete session |
| `/` | Filter |
| `Tab` | Switch Folders / Recent view |
| `Esc` / `q` | Back / Quit |
| `Ctrl+D` / `PageDown` | Page down (conversation) |
| `Ctrl+U` / `PageUp` | Page up (conversation) |

## How it works

Claude Code stores session logs as JSONL files under `~/.claude/projects/`. This tool scans those files, extracts metadata (first message, timestamps, model, message counts), and presents them in an interactive TUI built with [ratatui](https://ratatui.rs/).

When you resume a session, the process `exec`s into `claude --resume <session-id>` in the session's original working directory.
