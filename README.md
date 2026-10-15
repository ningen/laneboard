# Board

Human-visible kanban + agent-friendly CLI, sharing one SQLite store.

Agents move cards with `board … --json`. People drag the same cards in the Tauri desktop app (macOS & Windows).

## What’s in the box

| Piece | Role |
|---|---|
| `crates/core` | Cards, links, append-only events, FTS5 search |
| `crates/cli` (`board`) | Create / list / move / search / link / timeline |
| `apps/desktop` | Tauri 2 + React kanban UI |

Default DB path: platform data dir `/board/board.db`  
Override with `--db` or `BOARD_DB`.

## CLI quick start

```bash
# Linux / macOS (from repo)
cargo run -p board -- seed
cargo run -p board -- ls
cargo run -p board -- search auth --json
cargo run -p board -- move CARD-1 --to doing --actor agent --actor-name cursor --json
cargo run -p board -- get CARD-1 --json
cargo run -p board -- link CARD-1 CARD-3 --type blocks --json
cargo run -p board -- timeline --json
```

### Windows executable (prebuilt in this repo)

Release binary (cross-compiled):

- [`dist/board-windows-x64.exe`](dist/board-windows-x64.exe)

```powershell
.\board-windows-x64.exe seed
.\board-windows-x64.exe ls --json
$env:BOARD_DB = "C:\path\to\board.db"
.\board-windows-x64.exe board --json
```

Rebuild yourself (from Linux with mingw, or on Windows):

```bash
# Linux → Windows
rustup target add x86_64-pc-windows-gnu
cargo build -p board --release --target x86_64-pc-windows-gnu
# → target/x86_64-pc-windows-gnu/release/board.exe
```

```powershell
# On Windows (MSVC toolchain)
cargo build -p board --release
```

## Desktop app (macOS & Windows)

Requires Node 20+ and Rust. On each OS, build natively (Tauri needs the local WebView):

```bash
cd apps/desktop
npm install
npm run tauri dev          # development
npm run tauri build        # installers under src-tauri/target/release/bundle/
```

- **Windows**: produces `.msi` / `.exe` installer (WebView2).
- **macOS**: produces `.app` / `.dmg`.

The desktop app and CLI share the same default database, so an agent can `board move …` while you watch the board update after Refresh (or reopen detail).

## Data model (v0)

- **cards** — current state (status, labels, human/agent assignee)
- **card_links** — blocks / relates / parent / …
- **events** — append-only timeline
- **cards_fts** — SQLite FTS5 for search

JSON is an export/view (`board export`), not the runtime source of truth.

## Project layout

```
Cargo.toml                 workspace
crates/core                domain + SQLite
crates/cli                 `board` binary
apps/desktop               Tauri UI
dist/board-windows-x64.exe Windows CLI build artifact
```
