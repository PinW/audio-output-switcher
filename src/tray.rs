use std::ffi::c_void;
use std::sync::atomic::{AtomicPtr, Ordering};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Console::GetConsoleWindow;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::*;

const WM_TRAYICON: u32 = WM_APP + 1;
const TRAY_ICON_ID: u32 = 1;

// Embedded ICO files (multi-resolution, built from pixel art PNGs)
const SPEAKERS_ICO: &[u8] = include_bytes!("../assets/speakers.ico");
const HEADPHONES_ICO: &[u8] = include_bytes!("../assets/headphones.ico");

// Stored handles (HWND/HICON aren't Send+Sync, so use AtomicPtr)
static MSG_HWND: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static CONSOLE_HWND: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static SPEAKER_ICON: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static HEADPHONE_ICON: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

fn store_ptr(slot: &AtomicPtr<c_void>, ptr: *mut c_void) {
    slot.store(ptr, Ordering::Release);
}

fn load_ptr(slot: &AtomicPtr<c_void>) -> *mut c_void {
    slot.load(Ordering::Acquire)
}

fn load_msg_hwnd() -> HWND {
    HWND(load_ptr(&MSG_HWND))
}

fn load_console_hwnd() -> HWND {
    HWND(load_ptr(&CONSOLE_HWND))
}

/// Create tray icon with state indicators and hidden message window.
pub fn setup(is_speakers: bool) {
    // Cache console HWND and remove it from the taskbar
    let console = unsafe { GetConsoleWindow() };
    store_ptr(&CONSOLE_HWND, console.0);
    unsafe {
        // WS_EX_TOOLWINDOW hides the window from taskbar and Alt+Tab
        let style = GetWindowLongW(console, GWL_EXSTYLE);
        SetWindowLongW(console, GWL_EXSTYLE, style | WS_EX_TOOLWINDOW.0 as i32);
    }

    // Load icons from embedded ICO data
    let spk = load_icon_from_ico(SPEAKERS_ICO);
    let hp = load_icon_from_ico(HEADPHONES_ICO);
    store_ptr(&SPEAKER_ICON, spk.0);
    store_ptr(&HEADPHONE_ICON, hp.0);

    // Create message window and tray icon
    let hwnd = create_message_window();
    store_ptr(&MSG_HWND, hwnd.0);
    add_tray_icon(hwnd, is_speakers);
}

/// Remove tray icon and clean up.
pub fn cleanup() {
    let hwnd = load_msg_hwnd();
    if !hwnd.0.is_null() {
        remove_tray_icon(hwnd);
    }
}

/// Update tray icon and tooltip to reflect current device.
pub fn update_state(is_speakers: bool) {
    let hwnd = load_msg_hwnd();
    if hwnd.0.is_null() {
        return;
    }

    let icon = if is_speakers {
        HICON(load_ptr(&SPEAKER_ICON))
    } else {
        HICON(load_ptr(&HEADPHONE_ICON))
    };
    let tip_text = if is_speakers {
        "Audio: Speakers"
    } else {
        "Audio: Headphones"
    };

    let mut tip = [0u16; 128];
    let tip_utf16: Vec<u16> = tip_text.encode_utf16().collect();
    tip[..tip_utf16.len()].copy_from_slice(&tip_utf16);

    let nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ICON_ID,
        uFlags: NIF_ICON | NIF_TIP,
        hIcon: icon,
        szTip: tip,
        ..Default::default()
    };
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

/// Hide the console window.
pub fn hide_console() {
    unsafe {
        let _ = ShowWindow(load_console_hwnd(), SW_HIDE);
    }
}

/// Show and restore the console window, bringing it to the foreground.
pub fn show_console() {
    let hwnd = load_console_hwnd();
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = ShowWindow(hwnd, SW_RESTORE);

        // Briefly set topmost then remove — reliably brings window to front
        let topmost = HWND(-1isize as *mut c_void);
        let notopmost = HWND(-2isize as *mut c_void);
        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW;
        let _ = SetWindowPos(hwnd, Some(topmost), 0, 0, 0, 0, flags);
        let _ = SetWindowPos(hwnd, Some(notopmost), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE);
    }
}

/// Toggle console visibility: hide if visible, show if hidden.
fn toggle_console() {
    let hwnd = load_console_hwnd();
    if unsafe { IsWindowVisible(hwnd) }.as_bool() {
        hide_console();
    } else {
        show_console();
    }
}

/// Load an HICON from embedded ICO file bytes.
/// Picks the best size for the system tray (typically 16x16 or scaled).
fn load_icon_from_ico(ico_data: &[u8]) -> HICON {
    // ICO header: 2 reserved + 2 type + 2 count
    let count = u16::from_le_bytes([ico_data[4], ico_data[5]]) as usize;
    let target_size: u8 = 16; // System tray icon size

    // Find the 16x16 entry (or the smallest available)
    let mut best_offset: u32 = 0;
    let mut best_size: u32 = 0;
    let mut best_w: u8 = 255;

    for i in 0..count {
        let entry_base = 6 + i * 16;
        let w = ico_data[entry_base]; // 0 means 256
        let data_size = u32::from_le_bytes([
            ico_data[entry_base + 8],
            ico_data[entry_base + 9],
            ico_data[entry_base + 10],
            ico_data[entry_base + 11],
        ]);
        let data_offset = u32::from_le_bytes([
            ico_data[entry_base + 12],
            ico_data[entry_base + 13],
            ico_data[entry_base + 14],
            ico_data[entry_base + 15],
        ]);

        let actual_w = if w == 0 { 255 } else { w }; // treat 256 as largest
        // Prefer exact match, otherwise closest >= target, otherwise largest
        if actual_w == target_size
            || (best_w != target_size && actual_w < best_w && actual_w >= target_size)
            || (best_w != target_size && best_w < target_size && actual_w > best_w)
        {
            best_w = actual_w;
            best_offset = data_offset;
            best_size = data_size;
        }
    }

    let image_data = &ico_data[best_offset as usize..(best_offset + best_size) as usize];

    unsafe {
        CreateIconFromResourceEx(
            image_data,
            true, // fIcon
            0x00030000, // version (required)
            16,
            16,
            LR_DEFAULTCOLOR,
        )
        .expect("Failed to load icon from embedded ICO")
    }
}

fn create_message_window() -> HWND {
    unsafe {
        let class_name = wide_str("AudioSwitcherMsg");
        let wc = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            lpszClassName: PCWSTR(class_name.as_ptr()),
            ..Default::default()
        };
        RegisterClassExW(&wc);

        let parent = HWND(-3isize as *mut c_void); // HWND_MESSAGE
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WINDOW_STYLE::default(),
            0,
            0,
            0,
            0,
            Some(parent),
            None,
            None,
            None,
        )
        .expect("Failed to create message window")
    }
}

fn add_tray_icon(hwnd: HWND, is_speakers: bool) {
    let icon = if is_speakers {
        HICON(load_ptr(&SPEAKER_ICON))
    } else {
        HICON(load_ptr(&HEADPHONE_ICON))
    };
    let tip_text = if is_speakers {
        "Audio: Speakers"
    } else {
        "Audio: Headphones"
    };

    let mut tip = [0u16; 128];
    let tip_utf16: Vec<u16> = tip_text.encode_utf16().collect();
    tip[..tip_utf16.len()].copy_from_slice(&tip_utf16);

    let nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: TRAY_ICON_ID,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAYICON,
        hIcon: icon,
        szTip: tip,
        ..Default::default()
    };
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    }
}

fn remove_tray_icon(hwnd: HWND) {
    unsafe {
        let nid = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: TRAY_ICON_ID,
            ..Default::default()
        };
        let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    }
}

unsafe extern "system" fn wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TRAYICON => {
            let event = lparam.0 as u32;
            if event == WM_LBUTTONUP {
                toggle_console();
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn wide_str(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
