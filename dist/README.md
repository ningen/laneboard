# Prebuilt binaries

**Primary desktop app** = Tauri (build on macOS/Windows: see `docs/desktop.md`).

This folder holds the **CLI** helper for agents (and a browser fallback UI):

| File | Platform | Notes |
|---|---|---|
| `board-windows-x64.exe` | Windows x64 | `board` CLI + optional `board ui` fallback |

```powershell
.\board-windows-x64.exe ls --json
.\board-windows-x64.exe ui   # fallback browser UI @ http://127.0.0.1:7841
```

Tauri installer (recommended for humans):

```bash
cd apps/desktop && npm install && npm run tauri build
```