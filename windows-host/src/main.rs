mod vk_map;

use serde::{Deserialize, Serialize};
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

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
enum HostMessage {
    #[serde(rename = "mouse_move")]
    MouseMove { dx: i32, dy: i32 },
    #[serde(rename = "mouse_click")]
    MouseClick { button: String, action: String },
    #[serde(rename = "mouse_scroll")]
    MouseScroll { dx: i32, dy: i32 },
    #[serde(rename = "key")]
    Key { code: u16, action: String },
    #[serde(rename = "clipboard")]
    Clipboard { text: String },
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
enum ReceiverMessage {
    #[serde(rename = "switch_to_local")]
    SwitchToLocal,
}

static IS_REMOTE: AtomicBool = AtomicBool::new(false);
static IGNORE_NEXT_WARP: AtomicBool = AtomicBool::new(false);
static STREAM_SENDER: Mutex<Option<TcpStream>> = Mutex::new(None);

static mut SCREEN_WIDTH: i32 = 1920;
static mut SCREEN_HEIGHT: i32 = 1080;
static mut CENTER_X: i32 = 960;
static mut CENTER_Y: i32 = 540;

static mut MOUSE_HOOK: HHOOK = std::ptr::null_mut();
static mut KEYBOARD_HOOK: HHOOK = std::ptr::null_mut();

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

fn send_payload(msg: &HostMessage) {
    if let Ok(mut lock) = STREAM_SENDER.lock() {
        if let Some(stream) = lock.as_mut() {
            if let Ok(json_str) = serde_json::to_string(msg) {
                let packet = format!("{}\n", json_str);
                let _ = stream.write_all(packet.as_bytes());
            }
        }
    }
}

pub fn switch_to_remote() {
    IS_REMOTE.store(true, Ordering::SeqCst);
    println!("[*] Control shifted -> Omarchy (Hyprland)");
    IGNORE_NEXT_WARP.store(true, Ordering::SeqCst);
    #[cfg(windows)]
    unsafe {
        SetCursorPos(CENTER_X, CENTER_Y);
    }
}

pub fn switch_to_local() {
    IS_REMOTE.store(false, Ordering::SeqCst);
    println!("[*] Control shifted -> Windows Local");
    IGNORE_NEXT_WARP.store(true, Ordering::SeqCst);
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

        let remote = IS_REMOTE.load(Ordering::SeqCst);

        // Check if cursor hits the right screen edge of Windows
        if !remote {
            if x >= SCREEN_WIDTH - 2 {
                switch_to_remote();
                return 1; // Swallow event
            }
            return CallNextHookEx(MOUSE_HOOK, code, wparam, lparam);
        }

        // In Remote Mode:
        if msg == WM_MOUSEMOVE {
            if IGNORE_NEXT_WARP.swap(false, Ordering::SeqCst) {
                return 1;
            }

            let dx = x - CENTER_X;
            let dy = y - CENTER_Y;

            if dx != 0 || dy != 0 {
                send_payload(&HostMessage::MouseMove { dx, dy });
                IGNORE_NEXT_WARP.store(true, Ordering::SeqCst);
                SetCursorPos(CENTER_X, CENTER_Y);
            }
            return 1;
        }

        // Clicks
        if msg == WM_LBUTTONDOWN || msg == WM_RBUTTONDOWN || msg == WM_MBUTTONDOWN {
            let button = match msg {
                WM_LBUTTONDOWN => "left",
                WM_RBUTTONDOWN => "right",
                _ => "middle",
            };
            send_payload(&HostMessage::MouseClick {
                button: button.to_string(),
                action: "down".to_string(),
            });
            return 1;
        }

        if msg == WM_LBUTTONUP || msg == WM_RBUTTONUP || msg == WM_MBUTTONUP {
            let button = match msg {
                WM_LBUTTONUP => "left",
                WM_RBUTTONUP => "right",
                _ => "middle",
            };
            send_payload(&HostMessage::MouseClick {
                button: button.to_string(),
                action: "up".to_string(),
            });
            return 1;
        }

        // Mouse Wheel Scroll
        if msg == WM_MOUSEWHEEL {
            let delta = (mouse_data.mouseData >> 16) as i16;
            let scroll_units = if delta > 0 { 1 } else { -1 };
            send_payload(&HostMessage::MouseScroll {
                dx: 0,
                dy: scroll_units,
            });
            return 1;
        }
    }

    CallNextHookEx(MOUSE_HOOK, code, wparam, lparam)
}

#[cfg(windows)]
unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kbd_data = *(lparam as *const KBDLLHOOKSTRUCT);
        let vk = kbd_data.vkCode;
        let msg = wparam as u32;

        // Emergency Release Hotkeys: F12 (0x7B), Pause (0x13), ScrollLock (0x91)
        if vk == 0x7B || vk == 0x13 || vk == 0x91 {
            if IS_REMOTE.load(Ordering::SeqCst) && (msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN) {
                switch_to_local();
            }
            return 1;
        }

        if !IS_REMOTE.load(Ordering::SeqCst) {
            return CallNextHookEx(KEYBOARD_HOOK, code, wparam, lparam);
        }

        // In Remote Mode: map and send to Omarchy, swallow on Windows
        if let Some(evdev_code) = vk_map::translate_vk(vk) {
            let action = if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
                "down"
            } else {
                "up"
            };
            send_payload(&HostMessage::Key {
                code: evdev_code,
                action: action.to_string(),
            });
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
                        send_payload(&HostMessage::Clipboard { text });
                    }
                }
            }
            thread::sleep(Duration::from_millis(500));
        }
    });
}

fn start_receiver_listener(mut stream: TcpStream) {
    thread::spawn(move || {
        let reader = BufReader::new(stream);
        for line in reader.lines() {
            if let Ok(line_str) = line {
                if let Ok(msg) = serde_json::from_str::<ReceiverMessage>(&line_str) {
                    match msg {
                        ReceiverMessage::SwitchToLocal => {
                            println!("[*] Received SwitchToLocal from Omarchy boundary!");
                            switch_to_local();
                        }
                    }
                }
            }
        }
    });
}

fn main() {
    let target_ip = get_config_ip();
    println!("=== Tailscale KVM Rust Host Controller ===");
    println!("[*] Target Omarchy Machine: {}:9876", target_ip);

    #[cfg(windows)]
    unsafe {
        SCREEN_WIDTH = GetSystemMetrics(SM_CXSCREEN);
        SCREEN_HEIGHT = GetSystemMetrics(SM_CYSCREEN);
        CENTER_X = SCREEN_WIDTH / 2;
        CENTER_Y = SCREEN_HEIGHT / 2;
        println!("[*] Windows Screen Metrics: {}x{}", SCREEN_WIDTH, SCREEN_HEIGHT);
    }

    // Connect to Omarchy over Tailscale
    loop {
        match TcpStream::connect(format!("{}:9876", target_ip)) {
            Ok(stream) => {
                let _ = stream.set_nodelay(true);
                println!("[+] Connected to Omarchy!");

                let stream_clone = stream.try_clone().expect("Failed to clone TCP stream");
                start_receiver_listener(stream_clone);

                if let Ok(mut lock) = STREAM_SENDER.lock() {
                    *lock = Some(stream);
                }
                break;
            }
            Err(e) => {
                eprintln!("[!] Connection failed: {}. Retrying in 4 seconds...", e);
                thread::sleep(Duration::from_secs(4));
            }
        }
    }

    start_clipboard_sync();

    #[cfg(windows)]
    unsafe {
        MOUSE_HOOK = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_hook_proc), std::ptr::null_mut(), 0);
        KEYBOARD_HOOK = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), std::ptr::null_mut(), 0);

        println!("[*] Windows Win32 Low-Level Hooks Active!");
        println!("    - Move cursor past right edge -> Enter Omarchy");
        println!("    - Move cursor past Omarchy left edge -> Seamless return to Windows");
        println!("    - Press F12, Pause, or ScrollLock -> Emergency Release");

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        UnhookWindowsHookEx(MOUSE_HOOK);
        UnhookWindowsHookEx(KEYBOARD_HOOK);
    }
}
