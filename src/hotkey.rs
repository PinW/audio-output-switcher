use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN, VIRTUAL_KEY, VK_F1, VK_F10, VK_F11, VK_F12, VK_F2, VK_F3, VK_F4, VK_F5,
    VK_F6, VK_F7, VK_F8, VK_F9, VK_SPACE,
};
pub const HOTKEY_ID: i32 = 1;

/// Parse a hotkey string like "Ctrl+Alt+S" into (modifiers, virtual_key).
pub fn parse_hotkey(s: &str) -> Result<(HOT_KEY_MODIFIERS, VIRTUAL_KEY), String> {
    let mut modifiers = MOD_NOREPEAT; // Prevent repeated firing when held
    let mut vk = VIRTUAL_KEY(0);

    for part in s.split('+') {
        match part.trim().to_uppercase().as_str() {
            "CTRL" | "CONTROL" => modifiers |= MOD_CONTROL,
            "ALT" => modifiers |= MOD_ALT,
            "SHIFT" => modifiers |= MOD_SHIFT,
            "WIN" | "WINDOWS" | "SUPER" => modifiers |= MOD_WIN,
            key => {
                if vk != VIRTUAL_KEY(0) {
                    return Err(format!("Multiple keys specified: already had one, got '{}'", key));
                }
                vk = key_name_to_vk(key)?;
            }
        }
    }

    if vk == VIRTUAL_KEY(0) {
        return Err("No key specified in hotkey string".to_string());
    }

    Ok((modifiers, vk))
}

fn key_name_to_vk(name: &str) -> Result<VIRTUAL_KEY, String> {
    // Single letter A-Z -> ASCII value (0x41-0x5A)
    if name.len() == 1 {
        let ch = name.chars().next().unwrap();
        if ch.is_ascii_alphabetic() {
            return Ok(VIRTUAL_KEY(ch.to_ascii_uppercase() as u16));
        }
        if ch.is_ascii_digit() {
            return Ok(VIRTUAL_KEY(ch as u16));
        }
    }

    // Function keys and special keys
    match name {
        "F1" => Ok(VK_F1),
        "F2" => Ok(VK_F2),
        "F3" => Ok(VK_F3),
        "F4" => Ok(VK_F4),
        "F5" => Ok(VK_F5),
        "F6" => Ok(VK_F6),
        "F7" => Ok(VK_F7),
        "F8" => Ok(VK_F8),
        "F9" => Ok(VK_F9),
        "F10" => Ok(VK_F10),
        "F11" => Ok(VK_F11),
        "F12" => Ok(VK_F12),
        "SPACE" => Ok(VK_SPACE),
        _ => Err(format!("Unknown key: '{}'", name)),
    }
}

/// Register a global hotkey parsed from a string like "Ctrl+Alt+S".
pub fn register(hotkey_str: &str) -> Result<(), String> {
    let (modifiers, vk) = parse_hotkey(hotkey_str)?;
    unsafe {
        RegisterHotKey(None, HOTKEY_ID, modifiers, vk.0 as u32)
            .map_err(|e| format!("Failed to register hotkey '{}': {}", hotkey_str, e))
    }
}

/// Unregister the global hotkey.
pub fn unregister() {
    unsafe {
        let _ = UnregisterHotKey(None, HOTKEY_ID);
    }
}
