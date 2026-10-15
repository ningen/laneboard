# Prebuilt binaries

| File | Platform | Notes |
|---|---|---|
| `board-windows-x64.exe` | Windows x64 | CLI **and** embedded kanban UI (`board ui`). |

```powershell
.\board-windows-x64.exe ui
# open http://127.0.0.1:7841
```

Optional native Tauri installers: build on each OS with `cd apps/desktop && npm run tauri build`.