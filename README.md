# Audio Output Switcher

A lightweight Windows system tray utility that toggles between two audio output devices with a global hotkey. Built in Rust as a single portable executable — no installer needed.

## Features

- **Global hotkey** to instantly switch between speakers and headphones (default: `Ctrl+Alt+S`)
- **System tray icon** showing the active device (left-click to toggle, right-click for menu)
- **Start with Windows** option via the tray menu
- **CLI mode** for scripting: `audio-output-switcher.exe [toggle|speakers|headphones]`
- **Audio feedback** — plays a switch sound on toggle
- **First-time setup wizard** — interactive device and hotkey selection

## Installation

1. Download `audio-output-switcher.exe` from [Releases](https://github.com/PinW/audio-output-switcher/releases)
2. Place it anywhere you like
3. Run it — the setup wizard will guide you through selecting your two devices and a hotkey

Configuration is stored in `%APPDATA%\AudioSwitcher\config.json`. To reconfigure, right-click the tray icon and select **Reconfigure**, or delete the config file and restart.

## Usage

### Tray

- **Left-click** the tray icon to toggle devices
- **Right-click** for the context menu:
  - **Reconfigure** — re-run the setup wizard
  - **Start with Windows** — toggle auto-start on login
  - **Exit**

### CLI

```
audio-output-switcher.exe toggle       # switch to the other device
audio-output-switcher.exe speakers     # switch to speakers
audio-output-switcher.exe headphones   # switch to headphones
```

The CLI notifies any running tray instance to update its icon.

## Building from Source

Requires the Rust toolchain with the MSVC target and Visual Studio C++ Build Tools.

```
cargo build --release
```

## How It Works

Windows has no public API for changing the default audio output device. This utility uses the undocumented `IPolicyConfig` COM interface, which has been stable since Windows Vista and is used by all major audio switcher tools.
