# Windows Audio Output Switcher -- Architecture Research

**Date:** 2026-02-27

## Core Requirements Recap

- Detect available audio output devices on the system
- Let the user configure two preset outputs to toggle between
- Toggle between them via a global hotkey

---

## 1. Language / Runtime Options

### Option A: Python (running on native Windows Python, NOT WSL)

**Pros:**
- Rapid prototyping; fastest path to a working prototype
- Rich ecosystem: `pycaw` for Windows Core Audio, `keyboard` or `pynput` for global hotkeys, `pystray` for system tray
- JSON/TOML config is trivial with stdlib
- Large community with many examples of this exact use case
- Easy to iterate on and modify

**Cons:**
- Requires a Windows-native Python install (pycaw uses COM; will not work under WSL)
- Packaging to a standalone `.exe` adds a step (PyInstaller or Nuitka)
- Resulting `.exe` is relatively large (~15-40 MB) due to bundled interpreter
- Startup time is slower than compiled languages (~1-2 seconds cold start)
- The `pycaw` library's "set default device" feature is experimental and has reported COM errors; may require calling the undocumented `IPolicyConfig` COM interface directly via `comtypes`, or shelling out to PowerShell/NirCmd

**Key libraries:**
| Purpose | Library | Notes |
|---|---|---|
| Audio device enumeration | `pycaw` | `AudioUtilities.GetAllDevices()` |
| Set default device | `pycaw` (experimental), or shell to `PowerShell` / `nircmd` | See Section 4 |
| Global hotkey | `keyboard` (zero deps, pure Python) or `pynput` | `keyboard.add_hotkey('ctrl+shift+a', callback)` |
| System tray | `pystray` + `Pillow` | Mature, well-documented |
| Packaging | PyInstaller (simpler) or Nuitka (faster runtime) | See Section 7 |

### Option B: PowerShell (+ AudioDeviceCmdlets module)

**Pros:**
- Zero install footprint -- ships with Windows
- `AudioDeviceCmdlets` module provides `Get-AudioDevice -List` and `Set-AudioDevice` cmdlets that directly wrap the Windows Core Audio API
- A toggle script can be written in ~20 lines
- No packaging step needed; just a `.ps1` file

**Cons:**
- Global hotkey registration is awkward in pure PowerShell (no native support; must P/Invoke `RegisterHotKey` or use a helper tool)
- No built-in system tray support
- Limited GUI options
- Users must set execution policy (`Set-ExecutionPolicy`) to run scripts
- Harder to distribute as a "polished app"

**Best fit:** Quick personal tool, or as the audio-switching backend called by another language's hotkey layer.

### Option C: C# (.NET)

**Pros:**
- First-class Windows citizen; direct COM interop with Core Audio APIs
- `NAudio` and `AudioSwitcher.AudioApi` NuGet packages provide high-level wrappers
- Native system tray / WinForms / WPF support
- Compiles to a small, fast executable; .NET 8+ supports single-file publish and AOT
- Existing open-source reference: [SoundSwitch](https://github.com/Belphemur/SoundSwitch) (full-featured C# audio switcher)

**Cons:**
- More boilerplate than Python or PowerShell for a simple toggle
- Requires .NET SDK to build
- Steeper learning curve if unfamiliar with C#
- Heavier toolchain for a small utility

**Best fit:** If you want a polished, low-footprint Windows app and are comfortable with C#.

### Option D: Rust

**Pros:**
- Single static binary, no runtime dependencies, tiny file size (~1-3 MB)
- Excellent Windows API crates: `windows-rs` (official Microsoft bindings), `wasapi-rs`, `cpal`
- Global hotkey crates: `global-hotkey`, `win-hotkeys`
- Tray icon crate: `trayicon-rs`
- Fast startup, minimal memory usage

**Cons:**
- Highest development effort for this scope of project
- Windows COM interop in Rust is verbose
- Smaller community for this specific use case (fewer examples/Stack Overflow answers)
- Compile times are longer during development

**Best fit:** If you want a tiny, dependency-free binary and enjoy Rust.

### Option E: AutoHotkey (AHK)

**Pros:**
- Purpose-built for Windows hotkeys and automation
- Can call `nircmd.exe setdefaultsounddevice "Device Name"` in one line
- Entire toggle script is ~10-15 lines
- Compiles to standalone `.exe` via Ahk2Exe
- Large community with many audio-switching scripts already written

**Cons:**
- AHK is a niche scripting language; less transferable skill
- Relies on external tool (NirCmd) for the actual device switching
- Limited extensibility if requirements grow
- AHK v2 syntax differs significantly from v1; community examples are split

**Best fit:** Absolute minimum-effort solution if you just want it working today.

### Summary Matrix

| Criterion | Python | PowerShell | C# | Rust | AHK |
|---|---|---|---|---|---|
| Time to prototype | Fast | Fastest | Medium | Slow | Fastest |
| Packaging simplicity | Medium | High (no pkg) | High | High | High |
| Binary size | Large | N/A | Small-Med | Small | Small |
| Windows API access | Via COM/libs | Via cmdlets | Native | Via crates | Via NirCmd |
| Hotkey support | Good | Poor | Good | Good | Excellent |
| System tray support | Good | None | Excellent | Good | Basic |
| Extensibility | High | Low | High | High | Low |

---

## 2. Project Structure

Regardless of language, the project decomposes into these modules/concerns:

```
audio-output-switcher/
  |-- main.py (or main.rs, Program.cs, etc.)  # Entry point, wires everything together
  |-- audio.py          # Audio device enumeration and switching
  |-- hotkey.py          # Global hotkey registration and event handling
  |-- config.py          # Load/save user configuration (presets, hotkey binding)
  |-- tray.py            # (Optional) System tray icon and menu
  |-- config.json        # User's saved presets
  |-- assets/
  |     |-- icon.ico     # Tray icon
  |-- tests/
  |     |-- test_audio.py
  |     |-- test_config.py
```

### Key architectural boundaries:

1. **`audio` module** -- Knows how to enumerate devices and set the default. Exposes:
   - `list_output_devices() -> List[Device]`
   - `get_current_default() -> Device`
   - `set_default_device(device_id: str) -> None`

2. **`hotkey` module** -- Registers a global hotkey and calls a callback when pressed. Exposes:
   - `register_hotkey(key_combo: str, callback: Callable) -> None`
   - `start_listening() -> None` (blocking or threaded)

3. **`config` module** -- Reads/writes the config file. Exposes:
   - `load_config() -> Config`
   - `save_config(config: Config) -> None`

4. **`tray` module (optional)** -- Shows a system tray icon with a menu for configuration.

5. **`main`** -- Glue: loads config, registers hotkey with a toggle callback, optionally starts tray, enters event loop.

---

## 3. Configuration Storage

### What needs to be stored

- Device A identifier (name or ID string)
- Device B identifier (name or ID string)
- Hotkey binding (e.g., `"Ctrl+Alt+S"`)
- (Optional) Last active device, notification preferences, etc.

### Format Options

| Format | Pros | Cons |
|---|---|---|
| **JSON** | Stdlib support in every language; human-readable; widely understood | No comments allowed (unless using JSONC); verbose for nested config |
| **TOML** | Human-friendly; supports comments; great for config files; Python has `tomllib` in stdlib (3.11+) | Less common in Windows ecosystem; requires `tomli` for Python <3.11 |
| **INI** | Native to Windows; `configparser` in Python stdlib | Flat structure; no nested data; feels dated |
| **Windows Registry** | "The Windows way"; survives app reinstall; no file to manage | Harder to inspect/debug; overkill for 3 values; requires elevated access for HKLM |
| **YAML** | Very human-readable; supports comments | Requires third-party library; quirky parsing edge cases |

**Recommendation:** JSON or TOML. Both are simple, human-editable, and well-supported. JSON is the safe default; TOML is nicer if you want comments in the config.

### Storage Location Options

| Location | Path Example | Pros | Cons |
|---|---|---|---|
| **AppData/Roaming** | `%APPDATA%\AudioSwitcher\config.json` | Standard Windows convention; roams with user profile; survives app updates | Harder to find manually |
| **AppData/Local** | `%LOCALAPPDATA%\AudioSwitcher\config.json` | Same as above but machine-specific (no roaming) | Same discoverability issue |
| **Alongside executable** | `.\config.json` | Portable; easy to find and edit | Breaks if installed in `Program Files` (write-protected); not conventional |
| **User home** | `~\.audio-switcher\config.json` | Simple; cross-platform convention | Clutters home directory |

**Recommendation:** `%APPDATA%\AudioSwitcher\config.json` is the most Windows-conventional choice. For a personal tool, alongside the executable is fine and simpler.

---

## 4. Dynamic Device Detection

### The core problem

Windows does not expose a simple, documented API to **set** the default audio endpoint. It exposes APIs to **enumerate** devices and **get** the current default, but setting the default requires the undocumented `IPolicyConfig` COM interface.

### Enumeration approaches

**A. Windows Core Audio API (IMMDeviceEnumerator)**
- The "proper" way. Available via COM in any language.
- In Python: `pycaw.AudioUtilities.GetAllDevices()` wraps this.
- In C#: `NAudio.CoreAudioApi.MMDeviceEnumerator`.
- In Rust: `windows-rs` crate provides bindings to `IMMDeviceEnumerator`.
- Returns device ID (GUID-like string), friendly name, state (active/disabled/unplugged).

**B. PowerShell AudioDeviceCmdlets**
- `Get-AudioDevice -List` returns all devices with Index, Name, ID, and Default status.
- Can be called from Python via `subprocess`: `powershell.exe -Command "Get-AudioDevice -List"` and parse output.

**C. NirCmd / SoundVolumeView (NirSoft utilities)**
- `nircmd.exe showsounddevices` lists devices.
- Free, but requires bundling a third-party binary.

### Setting the default device

| Method | Language | Reliability | Notes |
|---|---|---|---|
| `IPolicyConfig` COM interface | C/C++/C#/Rust/Python (via comtypes) | High | Undocumented but stable since Vista; used by SoundSwitch, AudioSwitcher, and others |
| `Set-AudioDevice` (AudioDeviceCmdlets) | PowerShell (callable from any language) | High | Wraps `IPolicyConfig` internally |
| `nircmd setdefaultsounddevice "Name"` | CLI (callable from any language) | High | Requires bundling `nircmd.exe`; matches by device name |
| `pycaw` experimental API | Python | Medium | Some users report COM errors; active development |

### Handling device changes

Devices can be plugged/unplugged at any time. Options:
- **Re-enumerate on each toggle** -- simplest; slight latency (~50ms) but reliable
- **Register for device change notifications** -- `IMMNotificationClient` callback interface; more complex but real-time
- **Enumerate once at startup, re-enumerate on error** -- good middle ground

**Recommendation for a simple tool:** Re-enumerate on each toggle press. It is fast enough and handles plug/unplug gracefully.

---

## 5. Connecting Hotkey to Audio Switching

### Architecture: Event Loop

The application needs to:
1. Run persistently in the background
2. Listen for the global hotkey
3. On hotkey press: determine current default device, switch to the other preset

### Option A: Library-managed event loop (simplest)

```python
# Python example using `keyboard` library
import keyboard

def toggle():
    current = audio.get_current_default()
    target = config.device_b if current == config.device_a else config.device_a
    audio.set_default_device(target)

keyboard.add_hotkey('ctrl+alt+s', toggle)
keyboard.wait()  # Blocks forever, listening for hotkeys
```

The `keyboard` library runs its own listener thread. `keyboard.wait()` blocks the main thread. This is the simplest approach.

### Option B: Win32 message loop (more control)

```python
# Using RegisterHotKey + GetMessage loop
import ctypes

# Register hotkey with Windows
ctypes.windll.user32.RegisterHotKey(None, 1, MOD_CTRL | MOD_ALT, VK_S)

# Message pump
msg = ctypes.wintypes.MSG()
while ctypes.windll.user32.GetMessageW(ctypes.byref(msg), None, 0, 0):
    if msg.message == WM_HOTKEY:
        toggle()
```

More low-level but gives full control. Required if combining with a system tray (which also needs a message pump).

### Option C: System tray library manages the loop

If using `pystray`, it runs its own event loop. The hotkey listener runs in a separate thread:

```python
import pystray
import keyboard
import threading

def setup(icon):
    # Start hotkey listener in background thread
    keyboard.add_hotkey('ctrl+alt+s', toggle)
    icon.visible = True

icon = pystray.Icon("AudioSwitcher", image, menu=menu)
icon.run(setup=setup)  # Blocks on tray event loop
```

### Option D: Async / event-driven (Rust / C#)

In Rust or C#, the Win32 message loop is the standard pattern. Both `global-hotkey` (Rust) and `RegisterHotKey` (C#) integrate with the Windows message pump naturally.

**Recommendation:** For Python, Option A (`keyboard` library) is simplest for a no-tray version. Option C if you want a system tray. For C#/Rust, the Win32 message loop is idiomatic.

---

## 6. GUI vs CLI vs System Tray

### Option A: Pure CLI (minimal)

- Run from terminal, configure via command-line args or config file edit
- Hotkey listener runs as a blocking foreground process
- User starts it manually or adds to Windows Startup folder
- **Effort:** Lowest
- **UX:** Functional but rough; visible console window unless hidden

### Option B: System Tray App (recommended sweet spot)

- Runs silently in the background with a small icon in the system tray
- Right-click menu: "Device A", "Device B", "Settings", "Quit"
- Tooltip shows current active device
- Optional: toast notification on switch
- Can be added to Windows Startup
- **Effort:** Moderate (adds ~50-100 lines for tray setup)
- **UX:** Clean, unobtrusive, professional feel

### Option C: Full GUI Window

- Settings window with dropdowns for device selection, hotkey picker
- Overkill for this scope; most time spent on UI that is rarely opened
- Frameworks: Tkinter, PyQt, WinForms, WPF, egui (Rust)
- **Effort:** Highest
- **UX:** Most polished but unnecessary for a toggle utility

### Option D: Hybrid -- System Tray + Settings Dialog

- Tray for day-to-day use; a simple settings dialog (opened from tray menu) for initial configuration
- Best of both worlds
- **Effort:** Moderate-High
- **UX:** Best

**Recommendation:** Start with **CLI** (get the core working), then graduate to **System Tray** for daily use. A settings dialog can come later.

---

## 7. Packaging / Distribution

### Python Packaging

| Tool | Approach | Output Size | Startup Speed | Notes |
|---|---|---|---|---|
| **PyInstaller** | Bundles interpreter + deps into `.exe` | ~15-40 MB | ~1-2s | Most popular; `--onefile` or `--onedir` modes; `--noconsole` hides terminal |
| **Nuitka** | Compiles Python to C, then to native binary | ~10-25 MB | ~0.5-1s | Faster runtime; more complex build setup |
| **cx_Freeze** | Similar to PyInstaller | ~15-35 MB | ~1-2s | Less popular but stable |

For PyInstaller, a typical build command:
```
pyinstaller --onefile --noconsole --icon=assets/icon.ico --name=AudioSwitcher main.py
```

### C# Packaging

```
dotnet publish -c Release -r win-x64 --self-contained -p:PublishSingleFile=true
```
- Produces a single `.exe` (~10-20 MB with .NET runtime, or ~1-5 MB for AOT/trimmed)
- No installer needed; just the `.exe`

### Rust Packaging

```
cargo build --release
```
- Produces a single `.exe` (~1-3 MB)
- No runtime dependencies
- Smallest and fastest option

### AutoHotkey Packaging

- Ahk2Exe compiles `.ahk` to `.exe`
- Tiny output (~1-2 MB)
- Requires bundling `nircmd.exe` alongside

### Distribution Options

| Method | Effort | UX |
|---|---|---|
| **Single `.exe` in a GitHub Release** | Low | User downloads and runs; may trigger SmartScreen warning |
| **`.zip` with `.exe` + config + README** | Low | Slightly more organized |
| **Inno Setup / NSIS installer** | Medium | Professional; can add Start Menu shortcut, startup entry |
| **Windows Package Manager (`winget`)** | Medium-High | `winget install AudioSwitcher`; requires publishing manifest |
| **Scoop bucket** | Medium | `scoop install audio-switcher`; popular with power users |
| **Microsoft Store (MSIX)** | High | Maximum reach; significant packaging overhead |

**Recommendation for a personal tool:** Single `.exe` in a GitHub Release. If you want broader distribution later, add a Scoop manifest or Inno Setup installer.

### Auto-start on Windows login

Regardless of language, add a shortcut or registry entry:
- **Startup folder:** `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup\`
- **Registry:** `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` with path to `.exe`
- **Task Scheduler:** Create a task triggered at logon

---

## 8. Existing Tools (for reference / avoid reinventing)

Before building from scratch, consider whether an existing tool meets the need:

| Tool | Language | Hotkey | Tray | Open Source | Notes |
|---|---|---|---|---|---|
| [SoundSwitch](https://soundswitch.aaflalo.me/) | C# | Yes | Yes | Yes | Full-featured; mature; may be more than needed |
| [AudioSwitcher](https://audioswit.ch/er) | C# | Yes | Yes | Yes | Similar to SoundSwitch |
| [NirCmd](https://nircmd.nirsoft.net/setdefaultsounddevice.html) | C++ | No (CLI) | No | No | CLI tool; pair with AHK for hotkey |
| [AudioDeviceCmdlets](https://github.com/frgnca/AudioDeviceCmdlets) | PowerShell | No | No | Yes | PowerShell module; pair with a hotkey layer |

Building your own is justified if: you want something minimal, you want to learn, or you want precise control over behavior.

---

## 9. Recommended Starting Points (by complexity preference)

### Path 1: Minimal Effort (~30 minutes)
- **AutoHotkey v2 + NirCmd**
- Write a 15-line AHK script that toggles between two named devices via `nircmd.exe`
- Compile to `.exe` with Ahk2Exe

### Path 2: Simple but Flexible (~2-4 hours)
- **Python + `keyboard` + subprocess to `PowerShell AudioDeviceCmdlets`**
- Enumerate devices via `Get-AudioDevice -List`
- Switch via `Set-AudioDevice -Index N`
- Global hotkey via `keyboard.add_hotkey()`
- Config in `config.json`
- Package with PyInstaller

### Path 3: Polished Utility (~1-2 days)
- **Python + `pycaw`/comtypes + `keyboard` + `pystray`**
- Native COM calls for device enumeration and switching
- System tray with device menu
- Toast notifications on switch
- Config in `%APPDATA%`
- Package with PyInstaller `--noconsole`

### Path 4: Professional Native App (~3-5 days)
- **C# .NET 8 + AudioSwitcher.AudioApi + WinForms tray**
- Full COM interop; no external dependencies
- Native system tray and settings dialog
- Single-file publish; small binary
- Auto-start via registry

### Path 5: Minimal Binary, Maximum Performance (~3-5 days)
- **Rust + `windows-rs` + `global-hotkey` + `trayicon-rs`**
- Direct Windows API calls
- ~2 MB standalone binary
- Zero runtime dependencies

---

## Sources

- [SoundSwitch -- GitHub (C# audio switcher)](https://github.com/Belphemur/SoundSwitch)
- [AudioDeviceCmdlets -- GitHub (PowerShell)](https://github.com/frgnca/AudioDeviceCmdlets)
- [pycaw -- Python Core Audio Windows Library](https://github.com/AndreMiras/pycaw)
- [keyboard -- Python global hotkey library](https://github.com/boppreh/keyboard)
- [pynput -- Python keyboard/mouse library](https://pynput.readthedocs.io/en/latest/keyboard.html)
- [pystray -- Python system tray library](https://pypi.org/project/pystray/)
- [NirCmd setdefaultsounddevice reference](https://nircmd.nirsoft.net/setdefaultsounddevice.html)
- [AudioSwitcher .NET library](https://github.com/xenolightning/AudioSwitcher)
- [NAudio -- .NET audio library](https://github.com/naudio/NAudio)
- [global-hotkey Rust crate](https://crates.io/crates/global-hotkey)
- [win-hotkeys Rust crate](https://crates.io/crates/win-hotkeys)
- [trayicon-rs -- Rust tray icon](https://github.com/Ciantic/trayicon-rs)
- [Windows IPolicyConfig COM interface (community docs)](https://github.com/tartakynov/audioswitch/blob/master/IPolicyConfig.h)
- [PyInstaller vs Nuitka comparison](https://krrt7.dev/en/blog/nuitka-vs-pyinstaller)
- [AudioDeviceCmdlets Toggle example script](https://github.com/frgnca/AudioDeviceCmdlets/blob/master/EXAMPLE/Toggle-AudioDevice.ps1)
- [josetr/AudioDeviceSwitcher -- Windows 10 hotkey switcher](https://github.com/josetr/AudioDeviceSwitcher)
