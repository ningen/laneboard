# Laneboard

Human-visible **Tauri desktop** kanban + agent-friendly **CLI**, sharing one SQLite store.

Agents move cards with `board … --json`. People use the native Board window on macOS and Windows.

![Board desktop — kanban with detail panel](docs/images/desktop.png)

Design: Linear / Plane–style board UX (dense cards, status color, detail drawer) — see [`docs/design-system.md`](docs/design-system.md).

## Direction

**Tauri is the primary desktop surface.** Native window first (tray / notifications / OS integration later). `board ui` remains a headless/dev browser fallback.

## What’s in the box

| Piece | Role |
|---|---|
| `crates/core` | Cards, links, append-only events, FTS5 search |
| `crates/cli` (`board`) | Agent CLI (+ optional embedded UI) |
| `apps/desktop` | **Tauri 2** desktop app |

Default DB: `<data_dir>/board/board.db` (`BOARD_DB` / `--db` to override).

## Desktop (macOS & Windows)

```bash
./scripts/dev-desktop.sh
# or: cd apps/desktop && npm i && npm run tauri dev
```

Release installers (build **on each OS**):

```bash
cd apps/desktop && npm run tauri build
# → apps/desktop/src-tauri/target/release/bundle/
```

CI: [`.github/workflows/desktop.yml`](.github/workflows/desktop.yml) builds macOS + Windows artifacts when this repo is on GitHub.

This environment also produces a Linux binary smoke build: `dist/board-desktop-linux-x64`.

## CLI (agents)

```bash
cargo run -p board -- seed
cargo run -p board -- ls --json
cargo run -p board -- move CARD-1 --to doing --actor agent --actor-name cursor --json
```

### Nix

```bash
# one-shot
nix run github:ningen/laneboard -- ls --json

# install into your profile
nix profile install github:ningen/laneboard

# from a checkout
nix build
./result/bin/board --help
```

Windows CLI (prebuilt): [`dist/board-windows-x64.exe`](dist/board-windows-x64.exe)

```powershell
.\board-windows-x64.exe ls --json
.\board-windows-x64.exe ui   # fallback UI @ http://127.0.0.1:7841
```

## Data model (v0)

- **cards** — status, labels, human/agent assignee  
- **card_links** — blocks / relates / parent / …  
- **events** — append-only timeline  
- **cards_fts** — SQLite FTS5  

## GitLab

Pull GitLab Issues into the board (local SQLite; no webhooks).

**UI:** Desktop / `board ui` → **Settings** (⌘,) → GitLab (token, project, import / sync).

**CLI:**

```bash
export GITLAB_TOKEN=glpat-…          # read_api (or save token in Settings)
# export GITLAB_URL=https://gitlab.example.com   # self-managed
board gitlab doctor
board gitlab import group/project
board gitlab sync group/project
```

Details: [`docs/gitlab.md`](docs/gitlab.md). CI smoke on GitLab runners: [`.gitlab-ci.yml`](.gitlab-ci.yml).

## Agent path (MCP)

For Claude Code (especially with sandbox), prefer MCP over Bash `board …`.
Tool surface: [`docs/mcp-tools.md`](docs/mcp-tools.md).

```bash
BOARD_ACTOR_NAME=claude cargo run -p board -- mcp
# or after install: board mcp
```

Claude Code `mcpServers` example:

```json
{
  "mcpServers": {
    "laneboard": {
      "command": "board",
      "args": ["mcp"],
      "env": {
        "BOARD_DB": "/absolute/path/to/board.db",
        "BOARD_ACTOR_NAME": "claude"
      }
    }
  }
}
```

Session loop: `board_list` → `board_claim` → `board_get` → work → `board_note` → `board_move`.

## Layout

```
crates/core          domain + SQLite
crates/cli           board binary
apps/desktop         Tauri app
nix/                 Nix package (`board` CLI)
docs/                design system + desktop notes
dist/                prebuilt CLI / smoke binaries
```
