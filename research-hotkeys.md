# Research: Global Hotkey Implementation (from WhisperKeyLocal)

## Source Project
`/home/pin/whisper-key-local` — a Python-based whisper transcription app with global hotkey support on Windows and macOS.

## Library Used
**[`global-hotkeys`](https://pypi.org/project/global-hotkeys/)** — a Python library for system-wide hotkey registration.

### Key Import (Windows)
```python
# /home/pin/whisper-key-local/src/whisper_key/platform/windows/hotkeys.py
from global_hotkeys import register_hotkeys, start_checking_hotkeys, stop_checking_hotkeys
```

## Architecture Overview

The hotkey system has **3 layers**:

### 1. Platform Layer (`platform/windows/hotkeys.py`)
Thin wrapper around `global_hotkeys`. Handles key name normalization and exposes 3 functions:

```python
KEY_MAP = {
    'ctrl': 'control',
    'win': 'window',
    'windows': 'window',
    'cmd': 'window',
    'super': 'window',
    'esc': 'escape',
}

def _normalize_hotkey(hotkey_str: str) -> str:
    keys = hotkey_str.lower().split('+')
    converted = [KEY_MAP.get(k.strip(), k.strip()) for k in keys]
    return ' + '.join(converted)

def register(bindings: list):
    normalized = []
    for binding in bindings:
        hotkey_str = binding[0]
        normalized_binding = [_normalize_hotkey(hotkey_str)] + binding[1:]
        normalized.append(normalized_binding)
    register_hotkeys(normalized)

def start():
    start_checking_hotkeys()

def stop():
    stop_checking_hotkeys()
```

### 2. HotkeyListener Layer (`hotkey_listener.py`)
Manages hotkey lifecycle. Key pattern:

- **Binding format**: Each hotkey is a list `[combination_string, press_callback, release_callback, suppressed_flag]`
- **Registration flow**: `_setup_hotkeys()` builds a list of binding configs, sorts by specificity (most keys first), then calls `hotkeys.register(bindings)`
- **Start/stop**: `hotkeys.start()` / `hotkeys.stop()` control the listener thread
- **Dynamic reconfiguration**: `change_hotkey_config()` stops listening, rebuilds bindings, restarts

```python
# Binding format used by global_hotkeys:
binding = [
    "ctrl+shift+space",   # hotkey combination string
    callback_on_press,     # called when pressed
    callback_on_release,   # called when released (or None)
    False                  # whether to suppress the key
]
```

### 3. Config Layer (`config_manager.py`)
- Hotkeys defined in YAML config (`config.defaults.yaml`)
- User overrides stored in `user_settings.yaml` in AppData
- Config keys: `hotkey.recording_hotkey`, `hotkey.auto_enter_combination`, `hotkey.cancel_combination`
- Hotkey strings use `+` separator: `"ctrl+shift+space"`, `"ctrl+alt+s"`, etc.

## How to Reuse for Audio Output Switcher

### Minimal Implementation
```python
from global_hotkeys import register_hotkeys, start_checking_hotkeys, stop_checking_hotkeys

def on_toggle_pressed():
    print("Toggle audio output!")

# Format: [hotkey_string, press_callback, release_callback, suppress]
bindings = [
    ["control + shift + a", on_toggle_pressed, None, False]
]

register_hotkeys(bindings)
start_checking_hotkeys()

# Keep running...
import time
while True:
    time.sleep(0.1)
```

### Key Takeaways
1. **`global-hotkeys` is simple** — just `register`, `start`, `stop`. No complex setup.
2. **Hotkey format**: `"modifier + modifier + key"` with spaces around `+` (the library expects this format, WhisperKey normalizes to it)
3. **Thread-based**: `start_checking_hotkeys()` runs in a background thread, non-blocking
4. **Reconfigurable**: Can stop, re-register, and restart at runtime
5. **Python-only**: Pure Python, pip-installable, no compiled dependencies
6. **Supports release callbacks**: Can detect key-up events if needed (not needed for simple toggle)

### Dependencies
```
pip install global-hotkeys
```

### Considerations
- The library works on Windows. macOS uses a different implementation in WhisperKey (Quartz event taps).
- Since this project is Windows-only, we only need the Windows path.
- The `suppress` flag (4th element) can prevent the hotkey from reaching other apps — probably should be `False` for an audio switcher.
