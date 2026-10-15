# Board

Human-visible **Tauri desktop** kanban + agent-friendly **CLI**, sharing one SQLite store.

Agents move cards with `board … --json`. People use the native Board window on macOS and Windows.

## Direction

**Tauri is the primary desktop surface.** Native window, tray/notifications/OS integration later — that’s the long-term bet. The same React UI also runs via `board ui` (embedded HTTP) as a headless/dev fallback; it is not the product north star.

## What’s in the box

| Piece | Role |
|---|---|
| `crates/core` | Cards, links, append-only events, FTS5 search |
| `crates/cli` (`board`) | Create / list / move / search / link / timeline (+ optional `ui`) |
| `apps/desktop` | **Tauri 2** desktop app (React UI + Rust commands) |

Default DB: platform data dir `/board/board.db`  
Override: `--db` or `BOARD_DB` (CLI and desktop share this when using the default path).

## Desktop app (macOS & Windows) — primary

Build **on each OS** (Tauri needs the local WebView: WebKit on macOS, WebView2 on Windows):

```bash
cd apps/desktop
npm install
npm run tauri dev      # development window
npm run tauri build    # .dmg / .app (macOS), .msi / .exe (Windows)
```

Installers land under:

`apps/desktop/src-tauri/target/release/bundle/`

Requirements:

- Node 20+
- Rust stable
- **macOS**: Xcode CLT
- **Windows**: WebView2 runtime (usually present on Win10/11), MSVC build tools for release

## CLI (agents)

```bash
cargo run -p board -- seed
cargo run -p board -- ls
cargo run -p board -- search auth --json
cargo run -p board -- move CARD-1 --to doing --actor agent --actor-name cursor --json
cargo run -p board -- get CARD-1 --json
cargo run -p board -- link CARD-1 CARD-3 --type blocks --json
cargo run -p board -- timeline --json
```

### Windows CLI (prebuilt)

- [`dist/board-windows-x64.exe`](dist/board-windows-x64.exe) — CLI (+ embedded `ui` fallback)

```powershell
.\board-windows-x64.exe seed
.\board-windows-x64.exe ls --json
.\board-windows-x64.exe move CARD-1 --to doing --actor agent --json
```

Rebuild CLI:

```bash
./scripts/build-windows-cli.sh   # also embeds UI assets for `board ui`
# or on Windows: cargo build -p board --release
```

### Fallback: `board ui`

Same kanban in the browser when you don’t want to launch Tauri (CI, SSH, quick look):

```bash
board ui   # http://127.0.0.1:7841
```

## Data model (v0)

- **cards** — current state (status, labels, human/agent assignee)
- **card_links** — blocks / relates / parent / …
- **events** — append-only timeline
- **cards_fts** — SQLite FTS5 for search

JSON is an export/view (`board export`), not the runtime source of truth.

## Layout

```
Cargo.toml
crates/core          domain + SQLite
crates/cli           board binary
apps/desktop         Tauri app (primary UI)
dist/                prebuilt Windows CLI
docs/                sketches / notes
```
