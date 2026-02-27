mod audio;
mod config;
mod hotkey;

use std::io::{self, Write};

use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::WindowsAndMessaging::{GetMessageW, MSG, WM_HOTKEY};

fn main() {
    // Initialize COM (required for audio APIs)
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .expect("Failed to initialize COM");
    }

    // Load config or run first-time setup
    let cfg = match config::load() {
        Some(cfg) => {
            println!("Loaded config from {}", config::config_path().display());
            println!("  Device A: {}", cfg.device_a);
            println!("  Device B: {}", cfg.device_b);
            println!("  Hotkey:   {}", cfg.hotkey);
            cfg
        }
        None => {
            println!("No config found. Running first-time setup...\n");
            match first_time_setup() {
                Some(cfg) => cfg,
                None => {
                    eprintln!("Setup cancelled.");
                    return;
                }
            }
        }
    };

    // Register global hotkey
    if let Err(e) = hotkey::register(&cfg.hotkey) {
        eprintln!("Error: {}", e);
        return;
    }
    println!("\nHotkey [{}] registered. Press it to toggle audio output.", cfg.hotkey);
    println!("Press Ctrl+C to exit.\n");

    // Message loop — waits for WM_HOTKEY messages
    unsafe {
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            if msg.message == WM_HOTKEY {
                toggle_device(&cfg);
            }
        }
    }

    hotkey::unregister();
}

fn toggle_device(cfg: &config::Config) {
    let current_id = match audio::get_default_device_id() {
        Ok(id) => id,
        Err(e) => {
            eprintln!("Failed to get current device: {}", e);
            return;
        }
    };

    // Determine which device to switch to
    let target_id = if current_id == cfg.device_a {
        &cfg.device_b
    } else {
        &cfg.device_a
    };

    // Find the target device name for display
    let target_name = audio::list_devices()
        .ok()
        .and_then(|devices| {
            devices
                .into_iter()
                .find(|d| d.id == *target_id)
                .map(|d| d.name)
        })
        .unwrap_or_else(|| target_id.clone());

    match audio::set_default_device(target_id) {
        Ok(()) => println!("Switched to: {}", target_name),
        Err(e) => eprintln!("Failed to switch device: {}", e),
    }
}

fn first_time_setup() -> Option<config::Config> {
    let devices = audio::list_devices().expect("Failed to enumerate audio devices");

    if devices.len() < 2 {
        eprintln!("Need at least 2 audio output devices. Found {}.", devices.len());
        return None;
    }

    println!("Available audio output devices:");
    for (i, dev) in devices.iter().enumerate() {
        println!("  [{}] {}", i + 1, dev.name);
    }
    println!();

    let a = prompt_device_choice("Select Device A (number): ", devices.len())?;
    let b = prompt_device_choice("Select Device B (number): ", devices.len())?;

    if a == b {
        eprintln!("Device A and B must be different.");
        return None;
    }

    let cfg = config::Config {
        device_a: devices[a].id.clone(),
        device_b: devices[b].id.clone(),
        hotkey: "Ctrl+Alt+S".to_string(),
    };

    config::save(&cfg);
    println!(
        "\nConfig saved. Device A = '{}', Device B = '{}'",
        devices[a].name, devices[b].name
    );
    println!("Default hotkey: Ctrl+Alt+S (edit config to change)");

    Some(cfg)
}

fn prompt_device_choice(prompt: &str, max: usize) -> Option<usize> {
    print!("{}", prompt);
    io::stdout().flush().ok()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input).ok()?;

    let n: usize = input.trim().parse().ok()?;
    if n < 1 || n > max {
        eprintln!("Invalid choice: {}", n);
        return None;
    }

    Some(n - 1) // Convert to 0-indexed
}
