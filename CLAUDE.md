# Audio Output Switcher

Windows system tray utility to toggle between two preset audio output devices via a global hotkey. Written in Rust.

## Build & Run

Source lives on WSL, builds target Windows. The exe must live on the Windows filesystem (WSL doesn't support lock files for cargo).

```bash
# Build
powershell.exe -Command "cd '\\\\wsl$\\Ubuntu\\home\\pin\\audio-output-switcher'; cargo build 2>&1"

# Run (from WSL)
powershell.exe -Command "Start-Process 'C:\Users\pinwa\dev\audio-output-switcher\build\debug\audio-output-switcher.exe'"

# Kill running instance before rebuilding
powershell.exe -Command "Stop-Process -Name 'audio-output-switcher' -Force -ErrorAction SilentlyContinue"
```

PowerShell stderr from cargo is informational — check for "Finished" to confirm success.

Build output dir configured in `.cargo/config.toml` → `C:\Users\pinwa\dev\audio-output-switcher\build`.

## Architecture

- **Portable app** — no installer, single exe, can live anywhere. Config in `%APPDATA%\AudioSwitcher\config.json`. Startup shortcut (not registry) for auto-start.
- **`#![windows_subsystem = "windows"]`** — hides console. Uses `AllocConsole`/`FreeConsole` for temporary setup console. Do NOT use console subsystem + ShowWindow(SW_HIDE).
- **No official API** for setting default audio device — uses undocumented `IPolicyConfig` COM interface (stable since Vista).
- **No third-party hotkey crate** — uses `RegisterHotKey` from Windows API directly.

## Source Files

| File | Purpose |
|------|---------|
| `src/main.rs` | Entry point, message loop, CLI mode (`toggle`/`speakers`/`headphones`), setup wizard |
| `src/audio.rs` | Audio device enumeration and switching via `IPolicyConfig` COM |
| `src/config.rs` | JSON config load/save to `%APPDATA%\AudioSwitcher\config.json` |
| `src/hotkey.rs` | Global hotkey registration/parsing via `RegisterHotKey` |
| `src/tray.rs` | System tray icon, context menu, message window, autostart shortcut |
| `build.rs` | Windows resource embedding (exe icon, file description) |

## Key Dependencies

- `windows` (0.61) — Microsoft's official Windows API bindings
- `serde` + `serde_json` — config serialization
- `dirs` — find `%APPDATA%`
- `winresource` (build) — embed exe icon/metadata
