#![windows_subsystem = "windows"]

mod audio;
mod config;
mod hotkey;
mod tray;

use std::io::{self, Write};
use std::sync::atomic::{AtomicBool, Ordering};

use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::System::Console::{AllocConsole, FreeConsole};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, WM_HOTKEY,
};

// Hotkey IDs
const HOTKEY_TOGGLE: i32 = 1;
const HOTKEY_OPTIONS: i32 = 2;

// Flag to signal reconfigure request from the message loop
static RECONFIGURE: AtomicBool = AtomicBool::new(false);

fn main() {
    // Initialize COM
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .expect("Failed to initialize COM");
    }

    // Load or create config (allocate a temporary console for first-time setup)
    let mut cfg = match config::load() {
        Some(cfg) => cfg,
        None => {
            unsafe { let _ = AllocConsole(); }
            println!("No config found. Running first-time setup...\n");
            let result = run_setup();
            unsafe { let _ = FreeConsole(); }
            match result {
                Some(cfg) => cfg,
                None => return,
            }
        }
    };

    // Determine initial state (which device is currently default)
    let is_speakers = is_current_speakers(&cfg);

    // Register toggle hotkey
    if hotkey::register(&cfg.hotkey).is_err() {
        return;
    }
    hotkey::register_options();

    // Set up tray with initial state
    tray::setup(is_speakers);

    // Message loop
    loop {
        let exited = unsafe {
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                match msg.message {
                    WM_HOTKEY => match msg.wParam.0 as i32 {
                        HOTKEY_TOGGLE => toggle_device(&cfg),
                        HOTKEY_OPTIONS => {
                            RECONFIGURE.store(true, Ordering::Release);
                            break;
                        }
                        _ => {}
                    },
                    tray::WM_APP_TOGGLE => toggle_device(&cfg),
                    tray::WM_APP_RECONFIGURE => {
                        RECONFIGURE.store(true, Ordering::Release);
                        break;
                    }
                    _ => {
                        DispatchMessageW(&msg);
                    }
                }
            }
            !RECONFIGURE.load(Ordering::Acquire)
        };

        if exited {
            break; // WM_QUIT — app is closing
        }

        // Reconfigure: allocate temporary console, re-run setup
        RECONFIGURE.store(false, Ordering::Release);
        hotkey::unregister();

        unsafe { let _ = AllocConsole(); }
        println!("\n--- Reconfigure ---\n");
        let result = run_setup();
        unsafe { let _ = FreeConsole(); }

        match result {
            Some(new_cfg) => {
                cfg = new_cfg;
                let is_spk = is_current_speakers(&cfg);
                if hotkey::register(&cfg.hotkey).is_err() {
                    break;
                }
                hotkey::register_options();
                tray::update_state(is_spk);
            }
            None => {
                break;
            }
        }
    }

    tray::cleanup();
    hotkey::unregister();
}

fn is_current_speakers(cfg: &config::Config) -> bool {
    audio::get_default_device_id()
        .map(|id| id == cfg.speakers)
        .unwrap_or(true)
}

fn toggle_device(cfg: &config::Config) {
    let current_id = match audio::get_default_device_id() {
        Ok(id) => id,
        Err(e) => {
            eprintln!("Failed to get current device: {}", e);
            return;
        }
    };

    let (target_id, switching_to_speakers) = if current_id == cfg.speakers {
        (&cfg.headphones, false)
    } else {
        (&cfg.speakers, true)
    };

    let label = if switching_to_speakers {
        "Speakers"
    } else {
        "Headphones"
    };

    match audio::set_default_device(target_id) {
        Ok(()) => {
            println!("Switched to: {}", label);
            tray::update_state(switching_to_speakers);
        }
        Err(e) => eprintln!("Failed to switch device: {}", e),
    }
}

fn run_setup() -> Option<config::Config> {
    let devices = audio::list_devices().expect("Failed to enumerate audio devices");

    if devices.len() < 2 {
        eprintln!(
            "Need at least 2 audio output devices. Found {}.",
            devices.len()
        );
        return None;
    }

    println!("Available audio output devices:");
    for (i, dev) in devices.iter().enumerate() {
        println!("  [{}] {}", i + 1, dev.name);
    }
    println!();

    let a = prompt_device_choice("Select Speakers (number): ", devices.len())?;
    let b = prompt_device_choice("Select Headphones (number): ", devices.len())?;

    if a == b {
        eprintln!("Speakers and Headphones must be different devices.");
        return None;
    }

    let hotkey_str = prompt_hotkey()?;

    let cfg = config::Config {
        speakers: devices[a].id.clone(),
        headphones: devices[b].id.clone(),
        hotkey: hotkey_str,
    };

    config::save(&cfg);
    println!(
        "\nConfig saved. Speakers = '{}', Headphones = '{}'",
        devices[a].name, devices[b].name
    );
    println!("Hotkey: {}", cfg.hotkey);

    Some(cfg)
}

fn prompt_hotkey() -> Option<String> {
    loop {
        print!("Enter hotkey (default: Ctrl+Alt+S): ");
        io::stdout().flush().ok()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input).ok()?;
        let input = input.trim();

        let hotkey_str = if input.is_empty() {
            "Ctrl+Alt+S".to_string()
        } else {
            input.to_string()
        };

        match hotkey::parse_hotkey(&hotkey_str) {
            Ok(_) => return Some(hotkey_str),
            Err(e) => {
                eprintln!("Invalid hotkey '{}': {}", hotkey_str, e);
                eprintln!("Format: Modifier+Modifier+Key (e.g. Ctrl+Alt+S, Ctrl+Shift+F1)");
            }
        }
    }
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

    Some(n - 1)
}
