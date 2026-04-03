# claude-archiver

Archives [Claude Code](https://claude.ai/code) conversations and plan files to a local SQLite database for search, analysis, and long-term storage.

## What it does

Claude Code stores conversations as `.jsonl` files under `~/.claude/projects/`. This tool scans those files, parses the messages and events, and writes them into a structured SQLite database. Plan files (`~/.claude/plans/*.md`) are also archived with full revision history.

### Database schema

| Table | Contents |
|---|---|
| `projects` | One row per project slug |
| `sessions` | One row per conversation, with AI-generated title and start time |
| `messages` | Every message (human and assistant), with token counts |
| `raw_events` | Unrecognised event types stored verbatim as JSON |
| `plan_files` | Plan file revisions keyed by `(slug, mtime)` |
| `processed_files` | Tracks which `.jsonl` files have been ingested (by mtime) |

## Installation

```sh
cargo install --path .
```

Requires Rust 1.85+ (edition 2024).

## Usage

### One-shot

```sh
claude-archiver
```

Scans `~/.claude/projects/`, archives any new or changed sessions, then exits.

### Watch mode

```sh
claude-archiver --watch 30
```

Polls continuously, starting at 30-second intervals. After 5 consecutive polls with no changes the interval doubles, up to a maximum idle interval (default 20 minutes). The interval resets to the base as soon as a change is detected.

```sh
claude-archiver --watch 10 --max-idle-interval 600
```

### Options

| Flag | Default | Description |
|---|---|---|
| `--db PATH` | `~/claude-archive.db` | Path to the SQLite database |
| `--source PATH` | `~/.claude/projects/` | Root directory to scan for `.jsonl` files |
| `--project SLUG` | _(all)_ | Only process a single project |
| `--force` | false | Re-process files even if mtime is unchanged |
| `--watch SECONDS` | _(disabled)_ | Enable watch mode with this base interval |
| `--max-idle-interval SECONDS` | `1200` | Maximum poll interval when idle (watch mode only) |

### Logging

Log level is controlled via the `RUST_LOG` environment variable:

```sh
RUST_LOG=debug claude-archiver --watch 30
```

## License

[![](https://www.gnu.org/graphics/agplv3-155x51.png)](https://www.gnu.org/licenses/agpl-3.0.txt)

Copyright (C) 2026 KunoiSayami

This program is free software: you can redistribute it and/or modify it under the terms of the GNU Affero General Public License as published by the Free Software Foundation, either version 3 of the License, or any later version.

This program is distributed in the hope that it will be useful, but WITHOUT ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the GNU Affero General Public License for more details.

You should have received a copy of the GNU Affero General Public License along with this program. If not, see <https://www.gnu.org/licenses/>.
