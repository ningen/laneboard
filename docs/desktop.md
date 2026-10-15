# Desktop (Tauri) notes

## Why Tauri

Board is meant to feel like a real desktop control surface for humans watching agents:

- Always-on window (not a buried browser tab)
- Future: tray, global shortcuts, notifications, deep links, file drops
- Still shares `board-core` + the same SQLite file as the CLI

Cross-compiling Tauri installers from Linux is not supported here — ship from macOS and Windows builders (or CI runners on those OSes).

## Dev

```bash
cd apps/desktop
npm install
npm run tauri dev
```

Frontend: Vite on `http://localhost:1420`  
Backend: Rust commands in `src-tauri/src/lib.rs` (`get_board`, `move_card`, …).

The React layer (`src/api.ts`) talks to Tauri `invoke` in the app, and to HTTP when opened via `board ui`.

## Release

```bash
cd apps/desktop
npm run tauri build
```

| OS | Typical outputs |
|---|---|
| macOS | `.app`, `.dmg` under `src-tauri/target/release/bundle/` |
| Windows | `.msi` / NSIS `.exe` under `bundle/msi` or `bundle/nsis` |

## Same DB as CLI

Both use `board-core::BoardStore::path_hint()` → `<data_dir>/board/board.db`.

After an agent runs `board move …`, hit **Refresh** in the desktop UI (live watch can come later).
