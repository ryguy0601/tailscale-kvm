#![windows_subsystem = "windows"]

mod vk_map;

use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

#[cfg(windows)]
use windows_sys::Win32::Foundation::*;
#[cfg(windows)]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::*;

static IS_REMOTE: AtomicBool = AtomicBool::new(false);
static IGNORE_WARP: AtomicBool = AtomicBool::new(false);
static STREAM_SENDER: Mutex<Option<TcpStream>> = Mutex::new(None);

static mut SCREEN_WIDTH: i32 = 1920;
static mut SCREEN_HEIGHT: i32 = 1080;
static mut CENTER_X: i32 = 960;
static mut CENTER_Y: i32 = 540;

#[cfg(windows)]
static mut MOUSE_HOOK: HHOOK = 0;
#[cfg(windows)]
static mut KEYBOARD_HOOK: HHOOK = 0;

static LBUTTON_DOWN: AtomicBool = AtomicBool::new(false);
static CTRL_DOWN: AtomicBool = AtomicBool::new(false);
static ALT_DOWN: AtomicBool = AtomicBool::new(false);

fn get_config_ip() -> String {
    let args: Vec<String> = env::args().collect();
    if args.len() > 1 && !args[1].trim().is_empty() {
        return args[1].trim().to_string();
    }

    if let Ok(ip) = env::var("OMARCHY_TAILSCALE_IP") {
        if !ip.trim().is_empty() && ip != "100.x.y.z" {
            return ip.trim().to_string();
        }
    }

    if let Ok(content) = fs::read_to_string("config.txt") {
        for line in content.lines() {
            let line = line.trim();
            if line.starts_with("OMARCHY_IP=") {
                let parts: Vec<&str> = line.split('=').collect();
                if parts.len() > 1 && !parts[1].trim().is_empty() {
                    return parts[1].trim().to_string();
                }
            }
        }
    }

    "100.99.142.70".to_string()
}

#[inline(always)]
fn send_fast_bytes(bytes: &[u8]) {
    if let Ok(mut lock) = STREAM_SENDER.lock() {
        if let Some(stream) = lock.as_mut() {
            let _ = stream.write_all(bytes);
        }
    }
}

/// Grabs the active browser URL using rapid Win32 keystrokes (Ctrl+L -> Ctrl+C)
fn try_get_active_browser_url() -> Option<String> {
    #[cfg(windows)]
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd == 0 {
            return None;
        }

        // Simulate Ctrl + L (Focus address bar in Chrome, Edge, Brave, Firefox)
        keybd_event(VK_CONTROL as u8, 0, 0, 0);
        keybd_event(b'L', 0, 0, 0);
        keybd_event(b'L', 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL as u8, 0, KEYEVENTF_KEYUP, 0);

        thread::sleep(Duration::from_millis(40));

        // Simulate Ctrl + C (Copy URL to clipboard)
        keybd_event(VK_CONTROL as u8, 0, 0, 0);
        keybd_event(b'C', 0, 0, 0);
        keybd_event(b'C', 0, KEYEVENTF_KEYUP, 0);
        keybd_event(VK_CONTROL as u8, 0, KEYEVENTF_KEYUP, 0);

        thread::sleep(Duration::from_millis(40));

        // Read URL from clipboard
        if let Ok(mut clip) = arboard::Clipboard::new() {
            if let Ok(text) = clip.get_text() {
                let trimmed = text.trim();
                if trimmed.starts_with("http://")
                    || trimmed.starts_with("https://")
                    || trimmed.starts_with("www.")
                    || (trimmed.contains('.') && !trimmed.contains(' '))
                {
                    return Some(trimmed.to_string());
                }
            }
        }
    }
    None
}

/// Fling active browser tab across to Omarchy
pub fn fling_browser_to_omarchy() {
    thread::spawn(|| {
        if let Some(url) = try_get_active_browser_url() {
            let pkt = format!("U {}\n", url);
            send_fast_bytes(pkt.as_bytes());
        }
    });
}

pub fn switch_to_remote() {
    // If the user was dragging a window across to Omarchy (holding Left Mouse Button),
    // automatically fling their active browser tab to Omarchy!
    if LBUTTON_DOWN.load(Ordering::SeqCst) {
        fling_browser_to_omarchy();
    }

    IS_REMOTE.store(true, Ordering::SeqCst);
    IGNORE_WARP.store(true, Ordering::SeqCst);
    #[cfg(windows)]
    unsafe {
        SetCursorPos(CENTER_X, CENTER_Y);
    }
}

pub fn switch_to_local() {
    IS_REMOTE.store(false, Ordering::SeqCst);
    IGNORE_WARP.store(true, Ordering::SeqCst);
    #[cfg(windows)]
    unsafe {
        ClipCursor(std::ptr::null());
        SetCursorPos(SCREEN_WIDTH - 80, CENTER_Y);
    }
}

#[cfg(windows)]
unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let mouse_data = *(lparam as *const MSLLHOOKSTRUCT);
        let x = mouse_data.pt.x;
        let y = mouse_data.pt.y;
        let msg = wparam as u32;

        if msg == WM_LBUTTONDOWN {
            LBUTTON_DOWN.store(true, Ordering::SeqCst);
        } else if msg == WM_LBUTTONUP {
            LBUTTON_DOWN.store(false, Ordering::SeqCst);
        }

        let remote = IS_REMOTE.load(Ordering::SeqCst);

        // Check if cursor hits the right screen edge of Windows
        if !remote {
            if x >= SCREEN_WIDTH - 2 {
                switch_to_remote();
                return 1;
            }
            return CallNextHookEx(MOUSE_HOOK, code, wparam, lparam);
        }

        // In Remote Mode:
        if msg == WM_MOUSEMOVE {
            if IGNORE_WARP.swap(false, Ordering::SeqCst) {
                return 1;
            }

            let dx = x - CENTER_X;
            let dy = y - CENTER_Y;

            if dx != 0 || dy != 0 {
                let pkt = format!("M {} {}\n", dx, dy);
                send_fast_bytes(pkt.as_bytes());

                IGNORE_WARP.store(true, Ordering::SeqCst);
                SetCursorPos(CENTER_X, CENTER_Y);
            }
            return 1;
        }

        // Clicks
        if msg == WM_LBUTTONDOWN || msg == WM_RBUTTONDOWN || msg == WM_MBUTTONDOWN {
            let btn = match msg {
                WM_LBUTTONDOWN => "left",
                WM_RBUTTONDOWN => "right",
                _ => "middle",
            };
            let pkt = format!("C {} down\n", btn);
            send_fast_bytes(pkt.as_bytes());
            return 1;
        }

        if msg == WM_LBUTTONUP || msg == WM_RBUTTONUP || msg == WM_MBUTTONUP {
            let btn = match msg {
                WM_LBUTTONUP => "left",
                WM_RBUTTONUP => "right",
                _ => "middle",
            };
            let pkt = format!("C {} up\n", btn);
            send_fast_bytes(pkt.as_bytes());
            return 1;
        }

        // Mouse Wheel Scroll
        if msg == WM_MOUSEWHEEL {
            let delta = (mouse_data.mouseData >> 16) as i16;
            let scroll_units = if delta > 0 { 1 } else { -1 };
            let pkt = format!("S 0 {}\n", scroll_units);
            send_fast_bytes(pkt.as_bytes());
            return 1;
        }
    }

    CallNextHookEx(MOUSE_HOOK, code, wparam, lparam)
}

#[cfg(windows)]
unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kbd_data = *(lparam as *const KBDLLHOOKSTRUCT);
        let vk = kbd_data.vkCode as u16;
        let msg = wparam as u32;
        let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;

        if vk == VK_CONTROL || vk == VK_LCONTROL || vk == VK_RCONTROL {
            CTRL_DOWN.store(is_down, Ordering::SeqCst);
        } else if vk == VK_MENU || vk == VK_LMENU || vk == VK_RMENU {
            ALT_DOWN.store(is_down, Ordering::SeqCst);
        }

        // Hotkey: Ctrl + Alt + RightArrow -> Instantly Fling Active Tab to Omarchy!
        if vk == VK_RIGHT && is_down && CTRL_DOWN.load(Ordering::SeqCst) && ALT_DOWN.load(Ordering::SeqCst) {
            fling_browser_to_omarchy();
            return 1;
        }

        // Emergency Release Hotkeys: F12 (0x7B), Pause (0x13), ScrollLock (0x91)
        if vk == 0x7B || vk == 0x13 || vk == 0x91 {
            if IS_REMOTE.load(Ordering::SeqCst) && is_down {
                switch_to_local();
            }
            return 1;
        }

        if !IS_REMOTE.load(Ordering::SeqCst) {
            return CallNextHookEx(KEYBOARD_HOOK, code, wparam, lparam);
        }

        // In Remote Mode: map and send to Omarchy, swallow on Windows
        if let Some(evdev_code) = vk_map::translate_vk(vk as u32) {
            let action = if is_down { "down" } else { "up" };
            let pkt = format!("K {} {}\n", evdev_code, action);
            send_fast_bytes(pkt.as_bytes());
        }
        return 1;
    }

    CallNextHookEx(KEYBOARD_HOOK, code, wparam, lparam)
}

fn start_clipboard_sync() {
    thread::spawn(|| {
        let mut clipboard = arboard::Clipboard::new().ok();
        let mut last_text = String::new();

        loop {
            if let Some(clip) = clipboard.as_mut() {
                if let Ok(text) = clip.get_text() {
                    if !text.is_empty() && text != last_text {
                        last_text = text.clone();
                        let json = serde_json::json!({
                            "type": "clipboard",
                            "text": text
                        });
                        let pkt = format!("{}\n", json);
                        send_fast_bytes(pkt.as_bytes());
                    }
                }
            }
            thread::sleep(Duration::from_millis(500));
        }
    });
}

fn start_receiver_listener(stream: TcpStream) {
    thread::spawn(move || {
        let reader = BufReader::new(stream);
        for line in reader.lines() {
            if let Ok(line_str) = line {
                let trimmed = line_str.trim();
                if trimmed == "switch_to_local" || trimmed == "{\"type\":\"switch_to_local\"}" {
                    switch_to_local();
                }
            }
        }
    });
}

fn main() {
    let target_ip = get_config_ip();

    #[cfg(windows)]
    unsafe {
        SCREEN_WIDTH = GetSystemMetrics(SM_CXSCREEN);
        SCREEN_HEIGHT = GetSystemMetrics(SM_CYSCREEN);
        CENTER_X = SCREEN_WIDTH / 2;
        CENTER_Y = SCREEN_HEIGHT / 2;
    }

    // Connect to Omarchy over Tailscale with auto-reconnect
    loop {
        match TcpStream::connect(format!("{}:9876", target_ip)) {
            Ok(stream) => {
                let _ = stream.set_nodelay(true);

                let stream_clone = stream.try_clone().expect("Failed to clone TCP stream");
                start_receiver_listener(stream_clone);

                if let Ok(mut lock) = STREAM_SENDER.lock() {
                    *lock = Some(stream);
                }
                break;
            }
            Err(_) => {
                thread::sleep(Duration::from_secs(3));
            }
        }
    }

    start_clipboard_sync();

    #[cfg(windows)]
    unsafe {
        MOUSE_HOOK = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), 0, 0);
        KEYBOARD_HOOK = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), 0, 0);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        UnhookWindowsHookEx(MOUSE_HOOK);
        UnhookWindowsHookEx(KEYBOARD_HOOK);
    }
}
