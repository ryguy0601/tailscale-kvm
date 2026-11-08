#[cfg(windows)]
mod vk_map;
mod web_gui;
#[path = "../../shared/screen_record.rs"]
pub mod screen_record;

#[cfg(target_os = "linux")]
mod linux_host;
#[cfg(target_os = "macos")]
mod macos_host;

use serde_json::json;
use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
#[cfg(windows)]
use std::io::Cursor;
use std::net::{Shutdown, SocketAddr, TcpStream, ToSocketAddrs};
#[cfg(windows)]
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
#[cfg(windows)]
use std::sync::atomic::AtomicIsize;
use std::sync::atomic::{AtomicI32, AtomicU32, AtomicU64, AtomicU8, Ordering};
use std::sync::{Condvar, Mutex, OnceLock};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use windows_sys::Win32::Foundation::*;
#[cfg(windows)]
use windows_sys::Win32::Graphics::Gdi::*;
#[cfg(windows)]
use windows_sys::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardSequenceNumber,
    IsClipboardFormatAvailable, OpenClipboard, SetClipboardData,
};
#[cfg(windows)]
use windows_sys::Win32::System::Memory::{
    GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE,
};
#[cfg(windows)]
use windows_sys::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};
#[cfg(windows)]
use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
#[cfg(windows)]
use windows_sys::Win32::UI::Shell::DragQueryFileW;
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::*;

pub const SLOT_RIGHT: usize = 0;
pub const SLOT_LEFT: usize = 1;
pub const SLOT_UP: usize = 2;
pub const SLOT_DOWN: usize = 3;

pub static SLOT_CONNECTED: [AtomicBool; 4] = [
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
    AtomicBool::new(false),
];
pub static ACTIVE_REMOTE_SLOT: AtomicU8 = AtomicU8::new(255); // 0=Right, 1=Left, 2=Up, 3=Down, 255=Local
pub static IS_REMOTE: AtomicBool = AtomicBool::new(false);
pub static LAST_LOCAL_RETURN_MS: AtomicU64 = AtomicU64::new(0);

static IGNORE_WARP: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static HANDOFF_BUSY: AtomicBool = AtomicBool::new(false);
pub static SUPPRESS_CLIPBOARD_SYNC: AtomicBool = AtomicBool::new(false);
static LAST_CLIP_SEQ: AtomicU32 = AtomicU32::new(0);

pub static HOST_MIN_X: AtomicI32 = AtomicI32::new(0);
pub static HOST_MAX_X: AtomicI32 = AtomicI32::new(1920);
pub static HOST_MIN_Y: AtomicI32 = AtomicI32::new(0);
pub static HOST_MAX_Y: AtomicI32 = AtomicI32::new(1080);
static CENTER_X: AtomicI32 = AtomicI32::new(960);
static CENTER_Y: AtomicI32 = AtomicI32::new(540);

#[cfg(windows)]
static LBUTTON_DOWN: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static DRAG_START_X: AtomicI32 = AtomicI32::new(0);
#[cfg(windows)]
static DRAG_START_Y: AtomicI32 = AtomicI32::new(0);
#[cfg(windows)]
static DRAG_CLICK_HWND: AtomicIsize = AtomicIsize::new(0);

#[cfg(windows)]
static CTRL_DOWN: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static ALT_DOWN: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static WIN_DOWN: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static OVERLAY_HWND: AtomicIsize = AtomicIsize::new(0);
#[cfg(windows)]
static OVERLAY_ACTIVE: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static OVERLAY_DIR: AtomicU8 = AtomicU8::new(255);

#[cfg(windows)]
static mut MOUSE_HOOK: HHOOK = 0;
#[cfg(windows)]
static mut KEYBOARD_HOOK: HHOOK = 0;

#[cfg(windows)]
const CF_DIB_FMT: u32 = 8;
#[cfg(windows)]
const CF_DIBV5_FMT: u32 = 17;
#[cfg(windows)]
const CF_UNICODETEXT_FMT: u32 = 13;
#[cfg(windows)]
const CF_HDROP_FMT: u32 = 15;

#[cfg(windows)]
const MAX_CLIPBOARD_TEXT_BYTES: usize = 1_048_576; // 1 MB cap for text clipboard sync
#[cfg(windows)]
const MAX_IMAGE_BYTES: usize = 67_108_864; // 64 MB cap for image clipboard sync
const FILE_CHUNK_SIZE: usize = 65_536; // 64 KB streaming chunk buffer
const LAYOUT_FILE_NAME: &str = "kvm_layout.json";

static START_INSTANT: OnceLock<Instant> = OnceLock::new();

pub struct SlotChannel {
    pub target_ip: Mutex<Option<String>>,
    pub writer: Mutex<Option<TcpStream>>,
    pub generation: AtomicU64,
    pub wake_mutex: Mutex<u64>,
    pub wake_cv: Condvar,
}

impl SlotChannel {
    fn new() -> Self {
        Self {
            target_ip: Mutex::new(None),
            writer: Mutex::new(None),
            generation: AtomicU64::new(0),
            wake_mutex: Mutex::new(0),
            wake_cv: Condvar::new(),
        }
    }

    fn notify_change(&self) {
        if let Ok(mut g) = self.wake_mutex.lock() {
            *g = g.wrapping_add(1);
            self.wake_cv.notify_all();
        }
    }

    fn wait_timeout(&self, dur: Duration) {
        if let Ok(g) = self.wake_mutex.lock() {
            let _ = self.wake_cv.wait_timeout(g, dur);
        }
    }
}

static SLOT_POOL: OnceLock<[SlotChannel; 4]> = OnceLock::new();

fn pool() -> &'static [SlotChannel; 4] {
    SLOT_POOL.get_or_init(|| {
        [
            SlotChannel::new(),
            SlotChannel::new(),
            SlotChannel::new(),
            SlotChannel::new(),
        ]
    })
}

pub fn monotonic_ms() -> u64 {
    START_INSTANT
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis() as u64
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EdgeDirection {
    Right,
    Left,
    Up,
    Down,
}

impl EdgeDirection {
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().trim() {
            "left" => EdgeDirection::Left,
            "up" | "top" => EdgeDirection::Up,
            "down" | "bottom" => EdgeDirection::Down,
            _ => EdgeDirection::Right,
        }
    }

    pub fn to_slot_index(self) -> usize {
        match self {
            EdgeDirection::Right => SLOT_RIGHT,
            EdgeDirection::Left => SLOT_LEFT,
            EdgeDirection::Up => SLOT_UP,
            EdgeDirection::Down => SLOT_DOWN,
        }
    }

    pub fn reciprocal_return_edge(slot_idx: usize) -> &'static str {
        match slot_idx {
            SLOT_LEFT => "right",
            SLOT_UP => "bottom",
            SLOT_DOWN => "top",
            _ => "left",
        }
    }
}

#[cfg(windows)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum DragKind {
    Browser,
    Explorer,
    Other,
}

pub fn any_slot_connected() -> bool {
    SLOT_CONNECTED.iter().any(|c| c.load(Ordering::Relaxed))
}

pub fn first_connected_slot() -> Option<usize> {
    (0..4).find(|&i| SLOT_CONNECTED[i].load(Ordering::Relaxed))
}

#[cfg(windows)]
fn slot_target_ip(slot_idx: usize) -> Option<String> {
    pool()[slot_idx].target_ip.lock().ok().and_then(|g| g.clone())
}

pub fn send_bytes_to_slot(slot_idx: usize, bytes: &[u8]) -> bool {
    if slot_idx >= 4 {
        return false;
    }
    let mut disconnected = false;
    if let Ok(mut lock) = pool()[slot_idx].writer.lock() {
        if let Some(stream) = lock.as_mut() {
            if stream.write_all(bytes).is_err() {
                let _ = stream.shutdown(Shutdown::Both);
                *lock = None;
                disconnected = true;
            }
        } else {
            return false;
        }
    }
    if disconnected {
        SLOT_CONNECTED[slot_idx].store(false, Ordering::SeqCst);
        pool()[slot_idx].notify_change();
        if IS_REMOTE.load(Ordering::SeqCst)
            && ACTIVE_REMOTE_SLOT.load(Ordering::SeqCst) == slot_idx as u8
        {
            switch_to_local(slot_idx, None, None);
        }
        return false;
    }
    true
}

pub fn send_to_active_remote(bytes: &[u8]) {
    let active = ACTIVE_REMOTE_SLOT.load(Ordering::Relaxed) as usize;
    if active < 4 {
        send_bytes_to_slot(active, bytes);
    } else if let Some(first) = first_connected_slot() {
        send_bytes_to_slot(first, bytes);
    }
}

pub fn broadcast_clipboard_text(text: &str) {
    let mut pkt = String::with_capacity(text.len() + 4);
    pkt.push_str("T ");
    for ch in text.chars() {
        match ch {
            '\\' => pkt.push_str("\\\\"),
            '\n' => pkt.push_str("\\n"),
            '\r' => pkt.push_str("\\r"),
            c => pkt.push(c),
        }
    }
    pkt.push('\n');
    let raw = pkt.as_bytes();
    for idx in 0..4 {
        if SLOT_CONNECTED[idx].load(Ordering::Relaxed) {
            let _ = send_bytes_to_slot(idx, raw);
        }
    }
}

fn unescape_text_line(escaped: &str) -> String {
    let mut out = String::with_capacity(escaped.len());
    let mut chars = escaped.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn save_layout_file(slots: &[Option<String>; 4]) {
    let data = json!({
        "right": slots[0].clone().unwrap_or_default(),
        "left": slots[1].clone().unwrap_or_default(),
        "up": slots[2].clone().unwrap_or_default(),
        "down": slots[3].clone().unwrap_or_default(),
    });
    if let Ok(pretty) = serde_json::to_string_pretty(&data) {
        let _ = fs::write(LAYOUT_FILE_NAME, pretty);
    }
}

fn load_layout_file() -> Option<[Option<String>; 4]> {
    let raw = fs::read_to_string(LAYOUT_FILE_NAME).ok()?;
    let val: serde_json::Value = serde_json::from_str(&raw).ok()?;
    let norm = |k: &str| -> Option<String> {
        let s = val.get(k)?.as_str()?.trim();
        if s.is_empty() || s.eq_ignore_ascii_case("none") {
            None
        } else {
            Some(s.to_string())
        }
    };
    Some([norm("right"), norm("left"), norm("up"), norm("down")])
}

/// Atomically applies a 4-slot topology layout across Host & connected Receivers!
pub fn apply_slot_layout(mut desired: [Option<String>; 4], save_to_disk: bool) {
    // Sanitize & deduplicate so a single IP cannot occupy two edges simultaneously
    for i in 0..4 {
        if let Some(s) = desired[i].take() {
            let trimmed = s.trim().to_string();
            if !trimmed.is_empty() && !trimmed.eq_ignore_ascii_case("none") {
                let already_used = (0..i).any(|prev| desired[prev].as_deref() == Some(&trimmed));
                if !already_used {
                    desired[i] = Some(trimmed);
                }
            }
        }
    }

    let slots = pool();

    // Pass 1: Immediately tear down any slot whose IP was cleared or changed
    // (guarantees old connection closes BEFORE the same IP reconnects on a new edge!)
    for idx in 0..4 {
        let old_ip = slots[idx].target_ip.lock().ok().and_then(|g| g.clone());
        if old_ip != desired[idx] {
            if IS_REMOTE.load(Ordering::SeqCst)
                && ACTIVE_REMOTE_SLOT.load(Ordering::SeqCst) == idx as u8
            {
                switch_to_local(idx, None, None);
            }

            if let Ok(mut g) = slots[idx].target_ip.lock() {
                *g = None;
            }
            slots[idx].generation.fetch_add(1, Ordering::SeqCst);
            SLOT_CONNECTED[idx].store(false, Ordering::SeqCst);

            if let Ok(mut w) = slots[idx].writer.lock() {
                if let Some(stream) = w.take() {
                    let _ = stream.shutdown(Shutdown::Both);
                }
            }
            slots[idx].notify_change();
        }
    }

    // Pass 2: Activate newly assigned slots (or re-sync return edge on unchanged live slots)
    for idx in 0..4 {
        if let Some(ref target_ip) = desired[idx] {
            let cur_ip = slots[idx].target_ip.lock().ok().and_then(|g| g.clone());
            if cur_ip.as_deref() != Some(target_ip.as_str()) {
                println!(
                    "[*] Assigned slot [{}] -> {} (return edge: {})",
                    web_gui::index_to_slot_name(idx),
                    target_ip,
                    EdgeDirection::reciprocal_return_edge(idx)
                );
                if let Ok(mut g) = slots[idx].target_ip.lock() {
                    *g = Some(target_ip.clone());
                }
                slots[idx].generation.fetch_add(1, Ordering::SeqCst);
                SLOT_CONNECTED[idx].store(false, Ordering::SeqCst);
                slots[idx].notify_change();
            } else if SLOT_CONNECTED[idx].load(Ordering::SeqCst) {
                let pkt = format!("E {}\n", EdgeDirection::reciprocal_return_edge(idx));
                let _ = send_bytes_to_slot(idx, pkt.as_bytes());
            }
        }
    }

    web_gui::set_all_slots_mirror(&desired);
    if save_to_disk {
        save_layout_file(&desired);
    }
}

fn explicit_cli_target() -> Option<String> {
    let args: Vec<String> = env::args().collect();
    for i in 1..args.len() {
        if args[i] == "--target" || args[i] == "-t" || args[i] == "--ip" {
            if i + 1 < args.len() && !args[i + 1].trim().is_empty() {
                return Some(args[i + 1].trim().to_string());
            }
        }
        if let Some(val) = args[i].strip_prefix("--target=") {
            if !val.trim().is_empty() {
                return Some(val.trim().to_string());
            }
        }
        if let Some(val) = args[i].strip_prefix("--ip=") {
            if !val.trim().is_empty() {
                return Some(val.trim().to_string());
            }
        }
    }
    if args.len() > 1 && !args[1].starts_with('-') && !args[1].trim().is_empty() {
        return Some(args[1].trim().to_string());
    }
    None
}

fn legacy_env_or_config_target() -> Option<String> {
    if let Ok(ip) = env::var("OMARCHY_TAILSCALE_IP") {
        if !ip.trim().is_empty() && ip != "100.x.y.z" {
            return Some(ip.trim().to_string());
        }
    }

    if let Ok(content) = fs::read_to_string("config.txt") {
        for line in content.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("OMARCHY_IP=") {
                if !rest.trim().is_empty() {
                    return Some(rest.trim().to_string());
                }
            }
        }
    }

    None
}

fn get_config_direction() -> EdgeDirection {
    let args: Vec<String> = env::args().collect();
    for i in 1..args.len() {
        if args[i] == "--direction" || args[i] == "-d" || args[i] == "--edge" || args[i] == "-e" {
            if i + 1 < args.len() {
                return EdgeDirection::parse(&args[i + 1]);
            }
        }
        if let Some(val) = args[i].strip_prefix("--direction=") {
            return EdgeDirection::parse(val);
        }
        if let Some(val) = args[i].strip_prefix("--edge=") {
            return EdgeDirection::parse(val);
        }
    }

    if let Ok(dir) = env::var("KVM_DIRECTION") {
        if !dir.trim().is_empty() {
            return EdgeDirection::parse(&dir);
        }
    }

    if let Ok(content) = fs::read_to_string("config.txt") {
        for line in content.lines() {
            let line = line.trim();
            if let Some(rest) = line.strip_prefix("DIRECTION=") {
                return EdgeDirection::parse(rest);
            }
            if let Some(rest) = line.strip_prefix("EDGE=") {
                return EdgeDirection::parse(rest);
            }
        }
    }

    EdgeDirection::Right
}

#[cfg(windows)]
unsafe fn make_key_input(vk: u16, key_up: bool) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: vk,
                wScan: 0,
                dwFlags: if key_up { KEYEVENTF_KEYUP } else { 0 },
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

#[cfg(windows)]
unsafe fn send_inputs(inputs: &[INPUT]) {
    if !inputs.is_empty() {
        let _ = SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
    }
}

#[cfg(windows)]
unsafe fn tap_key(vk: u16) {
    let seq = [make_key_input(vk, false), make_key_input(vk, true)];
    send_inputs(&seq);
}

#[cfg(windows)]
unsafe fn tap_chord(modifier_vk: u16, key_vk: u16) {
    let seq = [
        make_key_input(modifier_vk, false),
        make_key_input(key_vk, false),
        make_key_input(key_vk, true),
        make_key_input(modifier_vk, true),
    ];
    send_inputs(&seq);
}

#[cfg(windows)]
unsafe fn cancel_active_drag_and_release_mouse() {
    tap_key(VK_ESCAPE);
    thread::sleep(Duration::from_millis(20));

    let up = [INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_LEFTUP,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }];
    send_inputs(&up);
    thread::sleep(Duration::from_millis(35));
}

#[cfg(windows)]
unsafe fn classify_window(hwnd: HWND) -> (HWND, DragKind) {
    if hwnd == 0 {
        return (0, DragKind::Other);
    }

    let root = GetAncestor(hwnd, GA_ROOT);
    let target = if root != 0 { root } else { hwnd };

    let mut class_buf = [0u16; 128];
    let class_len = GetClassNameW(target, class_buf.as_mut_ptr(), class_buf.len() as i32);
    let class_name = if class_len > 0 {
        String::from_utf16_lossy(&class_buf[..class_len as usize])
    } else {
        String::new()
    };

    let mut pid: u32 = 0;
    GetWindowThreadProcessId(target, &mut pid);
    if pid == 0 {
        return (target, DragKind::Other);
    }

    let h_proc = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
    if h_proc == 0 {
        return (target, DragKind::Other);
    }

    let mut exe_buf = [0u16; 512];
    let mut exe_len = exe_buf.len() as u32;
    let ok = QueryFullProcessImageNameW(h_proc, 0, exe_buf.as_mut_ptr(), &mut exe_len);
    let _ = CloseHandle(h_proc);

    if ok == 0 || exe_len == 0 {
        return (target, DragKind::Other);
    }

    let full_path = String::from_utf16_lossy(&exe_buf[..exe_len as usize]);
    let exe_name = full_path
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();

    match exe_name.as_str() {
        "firefox.exe" | "zen.exe" | "waterfox.exe" | "librewolf.exe" | "floorp.exe"
        | "chrome.exe" | "msedge.exe" | "brave.exe" | "vivaldi.exe" | "opera.exe" | "arc.exe"
        | "chromium.exe" | "thorium.exe" => (target, DragKind::Browser),

        "explorer.exe" => {
            if matches!(
                class_name.as_str(),
                "CabinetWClass" | "ExploreWClass" | "Progman" | "WorkerW"
            ) {
                (target, DragKind::Explorer)
            } else {
                (target, DragKind::Other)
            }
        }

        "files.exe" | "onecommander.exe" | "totalcmd64.exe" | "xyplorer.exe" => {
            (target, DragKind::Explorer)
        }

        _ => (target, DragKind::Other),
    }
}

#[cfg(windows)]
fn wait_for_clipboard_change(seq_before: u32, timeout_ms: u64) -> bool {
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    while Instant::now() < deadline {
        let cur = unsafe { GetClipboardSequenceNumber() };
        if cur != seq_before {
            return true;
        }
        thread::sleep(Duration::from_millis(10));
    }
    false
}

#[cfg(windows)]
fn read_clipboard_unicode_text() -> Option<String> {
    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT_FMT) == 0 {
            return None;
        }

        let mut opened = false;
        for _ in 0..4 {
            if OpenClipboard(0) != 0 {
                opened = true;
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        if !opened {
            return None;
        }

        let mut result = None;
        let h_data = GetClipboardData(CF_UNICODETEXT_FMT);
        if h_data != 0 {
            let h_global = h_data as HGLOBAL;
            let byte_size = GlobalSize(h_global);
            if byte_size > 0 && byte_size <= MAX_CLIPBOARD_TEXT_BYTES {
                let ptr = GlobalLock(h_global) as *const u16;
                if !ptr.is_null() {
                    let max_chars = byte_size / 2;
                    let mut len = 0usize;
                    while len < max_chars && *ptr.add(len) != 0 {
                        len += 1;
                    }
                    let slice = std::slice::from_raw_parts(ptr, len);
                    result = Some(String::from_utf16_lossy(slice));
                    let _ = GlobalUnlock(h_global);
                }
            }
        }

        let _ = CloseClipboard();
        result
    }
}

#[cfg(windows)]
fn write_clipboard_unicode_text(text: &str) -> bool {
    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    let byte_len = wide.len() * 2;

    unsafe {
        let mut opened = false;
        for _ in 0..4 {
            if OpenClipboard(0) != 0 {
                opened = true;
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        if !opened {
            return false;
        }

        EmptyClipboard();
        let h_mem = GlobalAlloc(GMEM_MOVEABLE, byte_len);
        if !h_mem.is_null() {
            let ptr = GlobalLock(h_mem) as *mut u16;
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len());
                let _ = GlobalUnlock(h_mem);
                SetClipboardData(CF_UNICODETEXT_FMT, h_mem as _);
            }
        }
        let _ = CloseClipboard();
        true
    }
}

fn write_host_clipboard_text(text: &str) {
    if text.is_empty() {
        return;
    }
    SUPPRESS_CLIPBOARD_SYNC.store(true, Ordering::SeqCst);
    #[cfg(windows)]
    {
        write_clipboard_unicode_text(text);
        LAST_CLIP_SEQ.store(unsafe { GetClipboardSequenceNumber() }, Ordering::SeqCst);
    }
    #[cfg(target_os = "linux")]
    {
        linux_host::write_linux_clipboard_text(text);
    }
    #[cfg(target_os = "macos")]
    {
        macos_host::write_macos_clipboard_text(text);
    }
    SUPPRESS_CLIPBOARD_SYNC.store(false, Ordering::SeqCst);
}

#[cfg(windows)]
fn read_clipboard_files() -> Vec<PathBuf> {
    unsafe {
        if IsClipboardFormatAvailable(CF_HDROP_FMT) == 0 {
            return Vec::new();
        }

        let mut opened = false;
        for _ in 0..4 {
            if OpenClipboard(0) != 0 {
                opened = true;
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        if !opened {
            return Vec::new();
        }

        let mut paths = Vec::new();
        let h_drop = GetClipboardData(CF_HDROP_FMT);
        if h_drop != 0 {
            let file_count = DragQueryFileW(h_drop as _, 0xFFFF_FFFF, std::ptr::null_mut(), 0);
            for i in 0..file_count.min(500) {
                let char_len = DragQueryFileW(h_drop as _, i, std::ptr::null_mut(), 0);
                if char_len == 0 {
                    continue;
                }
                let mut buffer: Vec<u16> = vec![0; (char_len + 1) as usize];
                let copied =
                    DragQueryFileW(h_drop as _, i, buffer.as_mut_ptr(), buffer.len() as u32);
                if copied > 0 {
                    buffer.truncate(copied as usize);
                    let os_str = std::ffi::OsString::from_wide(&buffer);
                    paths.push(PathBuf::from(os_str));
                }
            }
        }

        let _ = CloseClipboard();
        paths
    }
}

#[cfg(windows)]
fn read_clipboard_dib_bytes() -> Option<Vec<u8>> {
    unsafe {
        let fmt = if IsClipboardFormatAvailable(CF_DIBV5_FMT) != 0 {
            CF_DIBV5_FMT
        } else if IsClipboardFormatAvailable(CF_DIB_FMT) != 0 {
            CF_DIB_FMT
        } else {
            return None;
        };

        if OpenClipboard(0) == 0 {
            return None;
        }

        let mut result = None;
        let h_data = GetClipboardData(fmt);
        if h_data != 0 {
            let h_global = h_data as HGLOBAL;
            let byte_size = GlobalSize(h_global);
            if byte_size > 0 && byte_size <= MAX_IMAGE_BYTES {
                let ptr = GlobalLock(h_global) as *const u8;
                if !ptr.is_null() {
                    let slice = std::slice::from_raw_parts(ptr, byte_size);
                    result = Some(slice.to_vec());
                    let _ = GlobalUnlock(h_global);
                }
            }
        }
        let _ = CloseClipboard();
        result
    }
}

#[cfg(windows)]
fn write_clipboard_dib_bytes(dib_bytes: &[u8]) -> bool {
    unsafe {
        if OpenClipboard(0) == 0 {
            return false;
        }
        EmptyClipboard();

        let h_mem = GlobalAlloc(GMEM_MOVEABLE, dib_bytes.len());
        if !h_mem.is_null() {
            let ptr = GlobalLock(h_mem) as *mut u8;
            if !ptr.is_null() {
                std::ptr::copy_nonoverlapping(dib_bytes.as_ptr(), ptr, dib_bytes.len());
                let _ = GlobalUnlock(h_mem);
                SetClipboardData(CF_DIB_FMT, h_mem as _);
            }
        }
        let _ = CloseClipboard();
        true
    }
}

#[cfg(not(windows))]
fn write_clipboard_dib_bytes(_dib_bytes: &[u8]) -> bool {
    let _ = LAST_CLIP_SEQ.load(Ordering::Relaxed);
    false
}

#[cfg(windows)]
fn collect_transfer_entries(
    base: &Path,
    current: &Path,
    depth: usize,
    out: &mut Vec<(PathBuf, String)>,
) {
    if depth > 6 || out.len() >= 500 {
        return;
    }

    if current.is_file() {
        let rel = if base == current {
            current
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("transferred_file")
                .to_string()
        } else {
            let parent_name = base
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("folder");
            let sub = current
                .strip_prefix(base)
                .ok()
                .and_then(|p| p.to_str())
                .unwrap_or("file");
            format!("{}/{}", parent_name, sub.replace('\\', "/"))
        };
        let clean_rel: String = rel.chars().filter(|&c| c != '\r' && c != '\n').collect();
        if !clean_rel.is_empty() {
            out.push((current.to_path_buf(), clean_rel));
        }
    } else if current.is_dir() {
        if let Ok(entries) = fs::read_dir(current) {
            for entry in entries.flatten() {
                collect_transfer_entries(base, &entry.path(), depth + 1, out);
            }
        }
    }
}

fn resolve_target_addr(target_ip: &str) -> Option<SocketAddr> {
    let addr_str = format!("{}:9876", target_ip);
    addr_str.to_socket_addrs().ok()?.next()
}

fn get_host_downloads_dir() -> PathBuf {
    if let Ok(home) = env::var("USERPROFILE").or_else(|_| env::var("HOME")) {
        let p = PathBuf::from(home).join("Downloads");
        return p;
    }
    PathBuf::from("Downloads")
}

fn sanitize_relative_path(raw: &str) -> PathBuf {
    let normalized = raw.replace('\\', "/");
    let mut safe = PathBuf::new();
    for comp in Path::new(&normalized).components() {
        if let std::path::Component::Normal(seg) = comp {
            if !seg.is_empty() {
                safe.push(seg);
            }
        }
    }
    if safe.as_os_str().is_empty() {
        safe.push("transferred_file");
    }
    safe
}

fn receive_binary_file_host<R: Read>(
    reader: &mut R,
    file_size: u64,
    raw_rel_path: &str,
) -> Result<String, std::io::Error> {
    let normalized = raw_rel_path.replace('\\', "/");
    let (is_rec_internal, effective_rel) = if let Some(rest) = normalized.strip_prefix("__kvm_rec__/")
    {
        (true, rest)
    } else {
        (false, raw_rel_path)
    };

    let safe_rel = sanitize_relative_path(effective_rel);
    let base_dir = if is_rec_internal {
        screen_record::recordings_base_dir()
    } else {
        get_host_downloads_dir()
    };
    let target_path = base_dir.join(&safe_rel);

    if let Some(parent) = target_path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let mut out_file = fs::File::create(&target_path)?;
    let mut chunk = [0u8; FILE_CHUNK_SIZE];
    let mut remaining = file_size;

    while remaining > 0 {
        let to_read = FILE_CHUNK_SIZE.min(remaining as usize);
        let n = reader.read(&mut chunk[..to_read])?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "stream closed mid-file",
            ));
        }
        out_file.write_all(&chunk[..n])?;
        remaining -= n as u64;
    }

    out_file.flush()?;
    let rel_str = safe_rel.to_string_lossy().into_owned();
    if is_rec_internal {
        on_rec_binary_received(&rel_str, &target_path);
    }
    Ok(rel_str)
}

#[derive(Clone, Debug, Default)]
struct RecvRecCaps {
    backend: String,
    has_ffmpeg: bool,
    os: String,
}

#[derive(Clone, Debug)]
struct HostRecState {
    status: String,
    session_id: String,
    started_ms: u64,
    slot_idx: usize,
    slot_name: String,
    peer_ip: String,
    host_backend: String,
    host_file: Option<String>,
    recv_backend: String,
    recv_has_ffmpeg: bool,
    recv_os: String,
    recv_file: Option<PathBuf>,
    recv_err: Option<String>,
    stitched_webp: Option<String>,
    stitched_gif: Option<String>,
    stitched_mp4: Option<String>,
    exported_demo_pic: Option<String>,
    stitch_done: bool,
    auto_export: bool,
    message: String,
}

impl Default for HostRecState {
    fn default() -> Self {
        Self {
            status: "idle".to_string(),
            session_id: String::new(),
            started_ms: 0,
            slot_idx: 0,
            slot_name: "right".to_string(),
            peer_ip: String::new(),
            host_backend: screen_record::detect_capture_backend().to_string(),
            host_file: None,
            recv_backend: String::new(),
            recv_has_ffmpeg: false,
            recv_os: String::new(),
            recv_file: None,
            recv_err: None,
            stitched_webp: None,
            stitched_gif: None,
            stitched_mp4: None,
            exported_demo_pic: None,
            stitch_done: false,
            auto_export: true,
            message: "Ready to record synchronized Host + Receiver demo".to_string(),
        }
    }
}

static SLOT_REC_CAPS: OnceLock<Mutex<[Option<RecvRecCaps>; 4]>> = OnceLock::new();
static HOST_REC_COORD: OnceLock<(Mutex<HostRecState>, Condvar)> = OnceLock::new();

fn slot_caps_store() -> &'static Mutex<[Option<RecvRecCaps>; 4]> {
    SLOT_REC_CAPS.get_or_init(|| Mutex::new([None, None, None, None]))
}

fn rec_coord() -> &'static (Mutex<HostRecState>, Condvar) {
    HOST_REC_COORD.get_or_init(|| (Mutex::new(HostRecState::default()), Condvar::new()))
}

fn on_rec_binary_received(rel_after_prefix: &str, saved_path: &Path) {
    let norm = rel_after_prefix.replace('\\', "/");
    let mut parts = norm.splitn(2, '/');
    let sess = parts.next().unwrap_or("");
    let fname = parts.next().unwrap_or("");
    if sess.is_empty() || fname.is_empty() {
        return;
    }

    let (lock, cv) = rec_coord();
    if let Ok(mut st) = lock.lock() {
        if st.session_id == sess {
            if fname.starts_with("receiver_") {
                st.recv_file = Some(saved_path.to_path_buf());
            } else if fname == "dual_demo.webp" {
                st.stitched_webp = Some(fname.to_string());
            } else if fname == "dual_demo.gif" {
                st.stitched_gif = Some(fname.to_string());
            } else if fname == "dual_demo.mp4" {
                st.stitched_mp4 = Some(fname.to_string());
            }
            cv.notify_all();
        }
    }
}

fn on_rec_control_line(slot_idx: usize, trimmed: &str) {
    if let Some(rest) = trimmed.strip_prefix("REC_CAPS ") {
        let mut parts = rest.split_whitespace();
        let backend = parts.next().unwrap_or("unknown").to_string();
        let has_ffmpeg = parts.next().unwrap_or("0") == "1";
        let os = parts.next().unwrap_or("unknown").to_string();
        if let Ok(mut store) = slot_caps_store().lock() {
            if slot_idx < 4 {
                store[slot_idx] = Some(RecvRecCaps {
                    backend,
                    has_ffmpeg,
                    os,
                });
            }
        }
        return;
    }

    if let Some(rest) = trimmed.strip_prefix("REC_STARTED ") {
        let mut parts = rest.split_whitespace();
        let sess = parts.next().unwrap_or("");
        let backend = parts.next().unwrap_or("");
        let (lock, cv) = rec_coord();
        if let Ok(mut st) = lock.lock() {
            if st.session_id == sess && !backend.is_empty() {
                st.recv_backend = backend.to_string();
                cv.notify_all();
            }
        }
        return;
    }

    if let Some(rest) = trimmed.strip_prefix("REC_STITCH_DONE ") {
        let sess = rest.trim();
        let (lock, cv) = rec_coord();
        if let Ok(mut st) = lock.lock() {
            if st.session_id == sess {
                st.stitch_done = true;
                cv.notify_all();
            }
        }
        return;
    }

    if let Some(rest) = trimmed.strip_prefix("REC_ERR ") {
        let mut parts = rest.splitn(2, ' ');
        let sess = parts.next().unwrap_or("");
        let msg = parts.next().unwrap_or("Receiver recording error");
        let (lock, cv) = rec_coord();
        if let Ok(mut st) = lock.lock() {
            if st.session_id == sess {
                st.recv_err = Some(msg.to_string());
                cv.notify_all();
            }
        }
    }
}

fn stream_recording_file_to_receiver(
    target_ip: &str,
    local_path: &Path,
    session_id: &str,
    remote_filename: &str,
) -> Result<(), String> {
    let addr = resolve_target_addr(target_ip)
        .ok_or_else(|| format!("Failed resolving Receiver IP {}", target_ip))?;
    let mut tx = TcpStream::connect_timeout(&addr, Duration::from_secs(5))
        .map_err(|e| format!("Failed connecting to Receiver {}: {}", addr, e))?;
    let _ = tx.set_nodelay(true);
    let _ = tx.set_write_timeout(Some(Duration::from_secs(60)));

    let mut file = fs::File::open(local_path)
        .map_err(|e| format!("Failed opening {:?}: {}", local_path, e))?;
    let total_size = file
        .metadata()
        .map_err(|e| format!("Failed reading metadata {:?}: {}", local_path, e))?
        .len();

    let clean_sess = screen_record::sanitize_token(session_id, "demo_session");
    let clean_name = screen_record::sanitize_filename(remote_filename, "host.mp4");
    let header = format!("B {} __kvm_rec__/{}/{}\n", total_size, clean_sess, clean_name);
    tx.write_all(header.as_bytes())
        .map_err(|e| format!("Failed writing stream header: {}", e))?;

    let mut chunk = [0u8; FILE_CHUNK_SIZE];
    let mut remaining = total_size;
    while remaining > 0 {
        let to_read = FILE_CHUNK_SIZE.min(remaining as usize);
        match file.read(&mut chunk[..to_read]) {
            Ok(0) => break,
            Ok(n) => {
                tx.write_all(&chunk[..n])
                    .map_err(|e| format!("Failed streaming recording byte chunk: {}", e))?;
                remaining -= n as u64;
            }
            Err(e) => return Err(format!("Failed reading host file: {}", e)),
        }
    }
    let _ = tx.flush();
    let _ = tx.shutdown(Shutdown::Write);
    Ok(())
}

pub fn build_recording_status_json() -> serde_json::Value {
    let (lock, _) = rec_coord();
    let st = lock.lock().map(|g| g.clone()).unwrap_or_default();

    let elapsed_secs = if st.status == "recording" && st.started_ms > 0 {
        monotonic_ms().saturating_sub(st.started_ms) / 1000
    } else {
        0
    };

    let connected_caps = first_connected_slot().and_then(|idx| {
        slot_caps_store()
            .lock()
            .ok()
            .and_then(|s| s[idx].clone())
    });

    json!({
        "status": st.status,
        "session_id": st.session_id,
        "elapsed_secs": elapsed_secs,
        "slot_name": st.slot_name,
        "host_backend": if !st.host_backend.is_empty() {
            st.host_backend
        } else {
            screen_record::detect_capture_backend().to_string()
        },
        "host_has_ffmpeg": screen_record::has_ffmpeg(),
        "recv_backend": if !st.recv_backend.is_empty() {
            st.recv_backend
        } else {
            connected_caps.as_ref().map(|c| c.backend.clone()).unwrap_or_default()
        },
        "recv_os": connected_caps.as_ref().map(|c| c.os.clone()).unwrap_or(st.recv_os),
        "recv_has_ffmpeg": connected_caps.as_ref().map(|c| c.has_ffmpeg).unwrap_or(st.recv_has_ffmpeg),
        "host_file": st.host_file,
        "stitched_webp": st.stitched_webp,
        "stitched_gif": st.stitched_gif,
        "stitched_mp4": st.stitched_mp4,
        "exported_demo_pic": st.exported_demo_pic,
        "auto_export": st.auto_export,
        "demo_pics_available": screen_record::demo_pics_dir().is_some(),
        "message": st.message,
    })
}

pub fn start_dual_recording(
    requested_slot: Option<&str>,
    auto_export: bool,
) -> Result<serde_json::Value, String> {
    let slot_idx = if let Some(s) = requested_slot {
        let idx = web_gui::slot_name_to_index(s)
            .ok_or_else(|| format!("Invalid slot '{}'", s))?;
        if !SLOT_CONNECTED[idx].load(Ordering::Relaxed) {
            return Err(format!("Slot '{}' is not currently connected.", s));
        }
        idx
    } else {
        first_connected_slot().ok_or_else(|| {
            "Connect a Tailscale Receiver slot first so both screens can record together."
                .to_string()
        })?
    };

    let peer_ip = pool()[slot_idx]
        .target_ip
        .lock()
        .ok()
        .and_then(|g| g.clone())
        .ok_or_else(|| "No Receiver IP assigned to selected slot.".to_string())?;

    let slot_name = web_gui::index_to_slot_name(slot_idx).to_string();

    {
        let (lock, _) = rec_coord();
        let st = lock.lock().map_err(|_| "Recorder lock error".to_string())?;
        if st.status == "recording" || st.status == "stopping" || st.status == "stitching" {
            return Err(format!(
                "Recording already in progress (status: {}).",
                st.status
            ));
        }
    }

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let session_id = format!("demo_{}", ts);

    let caps = slot_caps_store()
        .lock()
        .ok()
        .and_then(|s| s[slot_idx].clone())
        .unwrap_or_default();

    {
        let (lock, cv) = rec_coord();
        if let Ok(mut st) = lock.lock() {
            *st = HostRecState {
                status: "recording".to_string(),
                session_id: session_id.clone(),
                started_ms: monotonic_ms(),
                slot_idx,
                slot_name: slot_name.clone(),
                peer_ip: peer_ip.clone(),
                host_backend: screen_record::detect_capture_backend().to_string(),
                host_file: None,
                recv_backend: caps.backend.clone(),
                recv_has_ffmpeg: caps.has_ffmpeg,
                recv_os: caps.os.clone(),
                recv_file: None,
                recv_err: None,
                stitched_webp: None,
                stitched_gif: None,
                stitched_mp4: None,
                exported_demo_pic: None,
                stitch_done: false,
                auto_export,
                message: format!(
                    "Recording Host ({}) + {} slot Receiver ({}) simultaneously...",
                    std::env::consts::OS,
                    slot_name,
                    peer_ip
                ),
            };
            cv.notify_all();
        }
    }

    let start_cmd = format!("REC_START {}\n", session_id);
    if !send_bytes_to_slot(slot_idx, start_cmd.as_bytes()) {
        let (lock, _) = rec_coord();
        if let Ok(mut st) = lock.lock() {
            st.status = "error".to_string();
            st.message = "Failed sending REC_START to connected Receiver.".to_string();
        }
        return Err("Failed sending REC_START to Receiver.".to_string());
    }

    match screen_record::start_capture(&session_id, "host") {
        Ok(info) => {
            if let Ok(mut st) = rec_coord().0.lock() {
                if st.session_id == session_id {
                    st.host_backend = info.backend.clone();
                }
            }
            println!(
                "[+] Synchronized Dual Recording STARTED: session={} (host={}, receiver={})",
                session_id, info.backend, peer_ip
            );
            Ok(build_recording_status_json())
        }
        Err(err) => {
            let _ = send_bytes_to_slot(slot_idx, format!("REC_STOP {}\n", session_id).as_bytes());
            let (lock, _) = rec_coord();
            if let Ok(mut st) = lock.lock() {
                st.status = "error".to_string();
                st.message = format!("Host recording failed to start: {}", err);
            }
            Err(err)
        }
    }
}

pub fn stop_dual_recording() -> Result<serde_json::Value, String> {
    let (session_id, slot_idx, slot_name, peer_ip, auto_export, recv_caps) = {
        let (lock, cv) = rec_coord();
        let mut st = lock.lock().map_err(|_| "Recorder lock error".to_string())?;
        if st.status != "recording" {
            return Err(format!(
                "Cannot stop recording while state is '{}'",
                st.status
            ));
        }
        st.status = "stopping".to_string();
        st.message = "Stopping Host & Receiver captures and syncing files...".to_string();
        cv.notify_all();
        let caps = slot_caps_store()
            .lock()
            .ok()
            .and_then(|s| s[st.slot_idx].clone())
            .unwrap_or_default();
        (
            st.session_id.clone(),
            st.slot_idx,
            st.slot_name.clone(),
            st.peer_ip.clone(),
            st.auto_export,
            caps,
        )
    };

    // Signal Receiver first so both stop cleanly in lockstep
    let _ = send_bytes_to_slot(slot_idx, format!("REC_STOP {}\n", session_id).as_bytes());

    thread::spawn(move || {
        let host_path = match screen_record::stop_capture(&session_id) {
            Ok(p) => p,
            Err(e) => {
                let (lock, cv) = rec_coord();
                if let Ok(mut st) = lock.lock() {
                    st.status = "error".to_string();
                    st.message = format!("Host capture stop error: {}", e);
                    cv.notify_all();
                }
                return;
            }
        };

        let host_fname = host_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("host.mp4")
            .to_string();

        {
            let (lock, cv) = rec_coord();
            if let Ok(mut st) = lock.lock() {
                st.host_file = Some(host_fname.clone());
                st.message = format!(
                    "Host clip saved ({}); waiting for Receiver upload...",
                    host_fname
                );
                cv.notify_all();
            }
        }

        // Wait up to 25 seconds for Receiver upload or error
        let wait_deadline = Instant::now() + Duration::from_secs(25);
        let mut recv_path_opt: Option<PathBuf> = None;
        let mut recv_err_opt: Option<String> = None;
        {
            let (lock, cv) = rec_coord();
            if let Ok(mut st) = lock.lock() {
                while Instant::now() < wait_deadline {
                    if let Some(ref p) = st.recv_file {
                        recv_path_opt = Some(p.clone());
                        break;
                    }
                    if let Some(ref err) = st.recv_err {
                        recv_err_opt = Some(err.clone());
                        break;
                    }
                    let rem = wait_deadline.saturating_duration_since(Instant::now());
                    let (guard, _) = cv
                        .wait_timeout(st, rem.min(Duration::from_millis(250)))
                        .unwrap();
                    st = guard;
                }
            }
        }

        let Some(recv_path) = recv_path_opt else {
            let (lock, cv) = rec_coord();
            if let Ok(mut st) = lock.lock() {
                st.status = "ready".to_string();
                st.message = if let Some(err) = recv_err_opt {
                    format!(
                        "Host clip saved ({}), Receiver recording failed: {}",
                        host_fname, err
                    )
                } else {
                    format!(
                        "Host clip saved ({}); Receiver upload timed out (make sure Receiver was rebuilt/updated).",
                        host_fname
                    )
                };
                cv.notify_all();
            }
            return;
        };

        let session_dir = screen_record::recordings_base_dir().join(&session_id);
        let host_lbl = format!("HOST ({})", std::env::consts::OS.to_uppercase());
        let recv_lbl = if recv_caps.os.is_empty() {
            "RECEIVER".to_string()
        } else {
            format!("RECEIVER ({})", recv_caps.os.to_uppercase())
        };

        {
            let (lock, cv) = rec_coord();
            if let Ok(mut st) = lock.lock() {
                st.status = "stitching".to_string();
                st.message = format!(
                    "Stitching Host + Receiver screens side-by-side ({} slot)...",
                    slot_name
                );
                cv.notify_all();
            }
        }

        if screen_record::has_ffmpeg() {
            match screen_record::stitch_dual_demo(
                &session_dir,
                &host_path,
                &recv_path,
                &slot_name,
                &host_lbl,
                &recv_lbl,
            ) {
                Ok(out) => {
                    let mut exported_name = None;
                    if auto_export {
                        if let Ok(dest) = screen_record::export_to_demo_pics(
                            &out.webp_path,
                            "02-dual-screen-demo.webp",
                        ) {
                            exported_name = dest
                                .file_name()
                                .and_then(|n| n.to_str())
                                .map(|s| s.to_string());
                        }
                    }
                    let (lock, cv) = rec_coord();
                    if let Ok(mut st) = lock.lock() {
                        st.status = "ready".to_string();
                        st.stitched_webp = Some("dual_demo.webp".to_string());
                        st.stitched_gif = if out.gif_path.is_file() {
                            Some("dual_demo.gif".to_string())
                        } else {
                            None
                        };
                        st.stitched_mp4 = Some("dual_demo.mp4".to_string());
                        st.exported_demo_pic = exported_name.clone();
                        st.message = match exported_name {
                            Some(n) => format!(
                                "Synced dual-PC demo stitched & exported to demoPics/{}!",
                                n
                            ),
                            None => "Synced dual-PC demo stitched (.webp / .gif / .mp4 ready)!"
                                .to_string(),
                        };
                        cv.notify_all();
                    }
                }
                Err(err) => {
                    let (lock, cv) = rec_coord();
                    if let Ok(mut st) = lock.lock() {
                        st.status = "error".to_string();
                        st.message = format!("Stitch error: {}", err);
                        cv.notify_all();
                    }
                }
            }
            return;
        }

        // Host lacks ffmpeg: delegate stitch to Linux/ffmpeg-capable Receiver!
        if recv_caps.has_ffmpeg {
            let recv_fname = recv_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("receiver.mp4")
                .to_string();
            if let Err(err) =
                stream_recording_file_to_receiver(&peer_ip, &host_path, &session_id, &host_fname)
            {
                let (lock, cv) = rec_coord();
                if let Ok(mut st) = lock.lock() {
                    st.status = "error".to_string();
                    st.message = format!("Remote stitch upload error: {}", err);
                    cv.notify_all();
                }
                return;
            }

            let host_lbl_tok = host_lbl.replace(' ', "_");
            let stitch_cmd = format!(
                "REC_STITCH {} {} {} {} {} {}\n",
                session_id,
                slot_name,
                host_fname,
                recv_fname,
                if auto_export { "1" } else { "0" },
                host_lbl_tok
            );
            let _ = send_bytes_to_slot(slot_idx, stitch_cmd.as_bytes());

            let stitch_deadline = Instant::now() + Duration::from_secs(60);
            let mut done = false;
            let mut done_seen_at: Option<Instant> = None;
            {
                let (lock, cv) = rec_coord();
                if let Ok(mut st) = lock.lock() {
                    while Instant::now() < stitch_deadline {
                        if (st.stitch_done && st.stitched_webp.is_some()) || st.recv_err.is_some() {
                            done = st.stitch_done;
                            break;
                        }
                        if st.stitch_done {
                            let seen = *done_seen_at.get_or_insert_with(Instant::now);
                            if seen.elapsed() >= Duration::from_secs(3) {
                                done = true;
                                break;
                            }
                        }
                        let rem = stitch_deadline.saturating_duration_since(Instant::now());
                        let (guard, _) = cv
                            .wait_timeout(st, rem.min(Duration::from_millis(250)))
                            .unwrap();
                        st = guard;
                    }
                    if !done && st.stitch_done {
                        done = true;
                    }
                }
            }

            let mut exported_local = None;
            if done && auto_export {
                let webp_local = session_dir.join("dual_demo.webp");
                if webp_local.is_file() {
                    if let Ok(dest) = screen_record::export_to_demo_pics(
                        &webp_local,
                        "02-dual-screen-demo.webp",
                    ) {
                        exported_local = dest
                            .file_name()
                            .and_then(|n| n.to_str())
                            .map(|s| s.to_string());
                    }
                }
            }

            let (lock, cv) = rec_coord();
            if let Ok(mut st) = lock.lock() {
                if done {
                    st.status = "ready".to_string();
                    st.exported_demo_pic = exported_local;
                    st.message = "Receiver stitched both screens & returned dual_demo.webp / .gif / .mp4!".to_string();
                } else {
                    st.status = "ready".to_string();
                    st.message = st
                        .recv_err
                        .clone()
                        .unwrap_or_else(|| "Remote stitch timed out; raw clips saved locally.".to_string());
                }
                cv.notify_all();
            }
        } else {
            let (lock, cv) = rec_coord();
            if let Ok(mut st) = lock.lock() {
                st.status = "ready".to_string();
                st.message = format!(
                    "Saved both clips to recordings/{} (install ffmpeg on either PC to auto-stitch .webp/.gif).",
                    session_id
                );
                cv.notify_all();
            }
        }
    });

    Ok(build_recording_status_json())
}

pub fn export_latest_recording(dest_name: Option<&str>) -> Result<PathBuf, String> {
    let (session_id, webp_opt, gif_opt) = {
        let (lock, _) = rec_coord();
        let st = lock.lock().map_err(|_| "Recorder lock error".to_string())?;
        (
            st.session_id.clone(),
            st.stitched_webp.clone(),
            st.stitched_gif.clone(),
        )
    };
    if session_id.is_empty() {
        return Err("No finished dual-screen recording session yet.".to_string());
    }
    let sdir = screen_record::recordings_base_dir().join(&session_id);
    let src = if let Some(w) = webp_opt {
        sdir.join(w)
    } else if let Some(g) = gif_opt {
        sdir.join(g)
    } else {
        sdir.join("dual_demo.webp")
    };
    let default_target = if src.extension().and_then(|e| e.to_str()) == Some("gif") {
        "02-dual-screen-demo.gif"
    } else {
        "02-dual-screen-demo.webp"
    };
    let dest = screen_record::export_to_demo_pics(&src, dest_name.unwrap_or(default_target))?;
    if let Ok(mut st) = rec_coord().0.lock() {
        st.exported_demo_pic = dest.file_name().and_then(|n| n.to_str()).map(|s| s.to_string());
    }
    Ok(dest)
}

fn open_url_on_host(url: &str) {
    let u = url.trim().to_string();
    if u.is_empty() {
        return;
    }
    thread::spawn(move || {
        let formatted = if !u.starts_with("http://")
            && !u.starts_with("https://")
            && !u.starts_with("about:")
            && !u.starts_with("file://")
        {
            format!("https://{}", u)
        } else {
            u
        };

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            let ff_candidates = [
                "C:\\Program Files\\Mozilla Firefox\\firefox.exe",
                "C:\\Program Files (x86)\\Mozilla Firefox\\firefox.exe",
            ];
            for candidate in ff_candidates {
                if Path::new(candidate).exists()
                    && std::process::Command::new(candidate)
                        .arg(&formatted)
                        .spawn()
                        .is_ok()
                {
                    return;
                }
            }
            let mut cmd = std::process::Command::new("cmd");
            cmd.args(["/C", "start", "", &formatted]);
            cmd.creation_flags(CREATE_NO_WINDOW);
            let _ = cmd.spawn();
        }

        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg(&formatted).spawn();
        }

        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let _ = std::process::Command::new("xdg-open")
                .arg(&formatted)
                .spawn();
        }
    });
}

fn open_terminal_on_host(dir_path: &str) {
    let d = dir_path.trim().to_string();
    thread::spawn(move || {
        #[cfg(windows)]
        {
            let default_dir = env::var("USERPROFILE").unwrap_or_else(|_| "C:\\".to_string());
            let win_path = if d.contains("/Work") {
                let sub = d.split("/Work").nth(1).unwrap_or("");
                format!(
                    "{}\\{}",
                    default_dir,
                    sub.replace('/', "\\").trim_start_matches('\\')
                )
            } else if Path::new(&d).exists() {
                d
            } else {
                default_dir
            };

            let _ = std::process::Command::new("wt.exe")
                .args(["-d", &win_path])
                .spawn()
                .or_else(|_| {
                    std::process::Command::new("powershell.exe")
                        .args(["-NoExit", "-Command", &format!("Set-Location '{}'", win_path)])
                        .spawn()
                });
        }

        #[cfg(target_os = "macos")]
        {
            let target = if Path::new(&d).exists() { &d } else { "." };
            let _ = std::process::Command::new("open")
                .args(["-a", "Terminal", target])
                .spawn();
        }

        #[cfg(not(any(windows, target_os = "macos")))]
        {
            let home = env::var("HOME").unwrap_or_else(|_| ".".to_string());
            let target = if Path::new(&d).exists() { &d } else { &home };
            let _ = std::process::Command::new("foot")
                .args(["-D", target])
                .spawn();
        }
    });
}

/// Incoming reverse transfer listener on Host port 9876 (for binary file/image reverse streams)
fn start_reverse_transfer_listener() {
    thread::spawn(|| {
        let Ok(listener) = std::net::TcpListener::bind("0.0.0.0:9876") else {
            return;
        };

        for stream_res in listener.incoming() {
            if let Ok(stream) = stream_res {
                let _ = stream.set_nodelay(true);
                thread::spawn(move || {
                    let mut reader = BufReader::with_capacity(FILE_CHUNK_SIZE, stream);
                    let mut line = String::new();
                    loop {
                        line.clear();
                        let bytes_read = match reader.read_line(&mut line) {
                            Ok(n) => n,
                            Err(_) => break,
                        };
                        if bytes_read == 0 {
                            break;
                        }

                        let trimmed = line.trim_end_matches(['\r', '\n']);
                        let bytes = trimmed.as_bytes();
                        if bytes.is_empty() {
                            continue;
                        }

                        match bytes[0] {
                            b'B' => {
                                if trimmed.len() > 2 {
                                    let rest = &trimmed[2..];
                                    if let Some(space_idx) = rest.find(' ') {
                                        let size_str = &rest[..space_idx];
                                        let rel_path = rest[space_idx + 1..].trim();
                                        if let Ok(file_size) = size_str.parse::<u64>() {
                                            let _ = receive_binary_file_host(
                                                &mut reader,
                                                file_size,
                                                rel_path,
                                            );
                                        }
                                    }
                                }
                            }
                            b'I' => {
                                if trimmed.len() > 2 {
                                    if let Ok(img_size) = trimmed[2..].trim().parse::<usize>() {
                                        let mut buf = vec![0u8; img_size];
                                        if reader.read_exact(&mut buf).is_ok() {
                                            SUPPRESS_CLIPBOARD_SYNC.store(true, Ordering::SeqCst);
                                            write_clipboard_dib_bytes(&buf);
                                            #[cfg(windows)]
                                            LAST_CLIP_SEQ.store(
                                                unsafe { GetClipboardSequenceNumber() },
                                                Ordering::SeqCst,
                                            );
                                            SUPPRESS_CLIPBOARD_SYNC.store(false, Ordering::SeqCst);
                                        }
                                    }
                                }
                            }
                            b'U' => {
                                if trimmed.len() > 2 {
                                    open_url_on_host(trimmed[2..].trim());
                                }
                            }
                            b'D' => {
                                if trimmed.len() > 2 {
                                    open_terminal_on_host(trimmed[2..].trim());
                                }
                            }
                            _ => {}
                        }
                    }
                });
            }
        }
    });
}

#[cfg(windows)]
fn stream_files_to_receiver(slot_idx: usize, paths: Vec<PathBuf>) {
    let Some(target_ip) = slot_target_ip(slot_idx) else {
        return;
    };

    let mut entries = Vec::new();
    for p in &paths {
        collect_transfer_entries(p, p, 0, &mut entries);
    }
    if entries.is_empty() {
        return;
    }

    let Some(addr) = resolve_target_addr(&target_ip) else {
        return;
    };

    let Ok(mut tx) = TcpStream::connect_timeout(&addr, Duration::from_secs(4)) else {
        return;
    };
    let _ = tx.set_nodelay(true);
    let _ = tx.set_write_timeout(Some(Duration::from_secs(60)));

    let mut chunk_buf = [0u8; FILE_CHUNK_SIZE];

    for (full_path, rel_path) in entries {
        let Ok(mut file) = fs::File::open(&full_path) else {
            continue;
        };
        let Ok(meta) = file.metadata() else {
            continue;
        };
        let total_size = meta.len();

        let header = format!("B {} {}\n", total_size, rel_path);
        if tx.write_all(header.as_bytes()).is_err() {
            break;
        }

        let mut remaining = total_size;
        let mut write_failed = false;
        while remaining > 0 {
            let to_read = FILE_CHUNK_SIZE.min(remaining as usize);
            match file.read(&mut chunk_buf[..to_read]) {
                Ok(0) => {
                    let zeros = [0u8; 4096];
                    while remaining > 0 {
                        let pad = zeros.len().min(remaining as usize);
                        if tx.write_all(&zeros[..pad]).is_err() {
                            write_failed = true;
                            break;
                        }
                        remaining -= pad as u64;
                    }
                    break;
                }
                Ok(n) => {
                    if tx.write_all(&chunk_buf[..n]).is_err() {
                        write_failed = true;
                        break;
                    }
                    remaining -= n as u64;
                }
                Err(_) => {
                    write_failed = true;
                    break;
                }
            }
        }

        if write_failed {
            break;
        }
    }

    let _ = tx.flush();
}

#[cfg(windows)]
fn stream_image_to_connected_receivers(dib_bytes: Vec<u8>) {
    let bytes = std::sync::Arc::new(dib_bytes);
    for idx in 0..4 {
        if !SLOT_CONNECTED[idx].load(Ordering::Relaxed) {
            continue;
        }
        let Some(target_ip) = slot_target_ip(idx) else {
            continue;
        };
        let Some(addr) = resolve_target_addr(&target_ip) else {
            continue;
        };
        let payload = std::sync::Arc::clone(&bytes);
        thread::spawn(move || {
            if let Ok(mut tx) = TcpStream::connect_timeout(&addr, Duration::from_secs(4)) {
                let _ = tx.set_nodelay(true);
                let header = format!("I {}\n", payload.len());
                if tx.write_all(header.as_bytes()).is_ok() {
                    let _ = tx.write_all(&payload);
                    let _ = tx.flush();
                }
            }
        });
    }
}

#[cfg(windows)]
fn looks_like_url(s: &str) -> bool {
    let t = s.trim();
    if t.len() < 4 || t.chars().any(char::is_whitespace) {
        return false;
    }
    t.starts_with("http://")
        || t.starts_with("https://")
        || t.starts_with("about:")
        || t.starts_with("file://")
        || t.starts_with("www.")
        || t.contains('.')
        || t.starts_with("localhost:")
}

#[cfg(windows)]
fn execute_handoff(slot_idx: usize, source_hwnd: HWND, fallback_hwnd: HWND, from_drag: bool) {
    if HANDOFF_BUSY
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    thread::spawn(move || {
        unsafe {
            if from_drag {
                cancel_active_drag_and_release_mouse();
            } else {
                let deadline = Instant::now() + Duration::from_millis(600);
                while Instant::now() < deadline {
                    if !WIN_DOWN.load(Ordering::Relaxed)
                        && !CTRL_DOWN.load(Ordering::Relaxed)
                        && !ALT_DOWN.load(Ordering::Relaxed)
                    {
                        break;
                    }
                    thread::sleep(Duration::from_millis(12));
                }
                thread::sleep(Duration::from_millis(20));
            }

            let (mut target_hwnd, mut kind) = classify_window(source_hwnd);
            if kind == DragKind::Other && fallback_hwnd != 0 && fallback_hwnd != source_hwnd {
                let (fb_hwnd, fb_kind) = classify_window(fallback_hwnd);
                if fb_kind != DragKind::Other {
                    target_hwnd = fb_hwnd;
                    kind = fb_kind;
                }
            }

            match kind {
                DragKind::Browser => {
                    if GetForegroundWindow() != target_hwnd {
                        let _ = SetForegroundWindow(target_hwnd);
                        thread::sleep(Duration::from_millis(30));
                    }

                    SUPPRESS_CLIPBOARD_SYNC.store(true, Ordering::SeqCst);
                    let seq_before = GetClipboardSequenceNumber();

                    tap_chord(VK_CONTROL, b'L' as u16);
                    thread::sleep(Duration::from_millis(45));
                    tap_chord(VK_CONTROL, b'C' as u16);

                    let mut changed = wait_for_clipboard_change(seq_before, 240);
                    if !changed {
                        tap_chord(VK_MENU, b'D' as u16);
                        thread::sleep(Duration::from_millis(45));
                        tap_chord(VK_CONTROL, b'C' as u16);
                        changed = wait_for_clipboard_change(seq_before, 200);
                    }

                    let copied_url = if changed {
                        read_clipboard_unicode_text()
                    } else {
                        None
                    };

                    LAST_CLIP_SEQ.store(GetClipboardSequenceNumber(), Ordering::SeqCst);
                    SUPPRESS_CLIPBOARD_SYNC.store(false, Ordering::SeqCst);

                    tap_key(VK_ESCAPE);
                    thread::sleep(Duration::from_millis(30));

                    if from_drag {
                        tap_chord(VK_CONTROL, b'W' as u16);
                    }

                    let final_url = copied_url
                        .as_deref()
                        .map(str::trim)
                        .filter(|u| looks_like_url(u))
                        .unwrap_or("about:newtab");

                    let pkt = format!("U {}\n", final_url);
                    let _ = send_bytes_to_slot(slot_idx, pkt.as_bytes());
                }

                DragKind::Explorer => {
                    if GetForegroundWindow() != target_hwnd {
                        let _ = SetForegroundWindow(target_hwnd);
                        thread::sleep(Duration::from_millis(30));
                    }

                    SUPPRESS_CLIPBOARD_SYNC.store(true, Ordering::SeqCst);
                    let seq_before = GetClipboardSequenceNumber();

                    tap_chord(VK_CONTROL, b'C' as u16);
                    let mut changed = wait_for_clipboard_change(seq_before, 250);
                    if !changed {
                        thread::sleep(Duration::from_millis(35));
                        tap_chord(VK_CONTROL, b'C' as u16);
                        changed = wait_for_clipboard_change(seq_before, 250);
                    }

                    let files = if changed || !from_drag {
                        read_clipboard_files()
                    } else {
                        Vec::new()
                    };

                    LAST_CLIP_SEQ.store(GetClipboardSequenceNumber(), Ordering::SeqCst);
                    SUPPRESS_CLIPBOARD_SYNC.store(false, Ordering::SeqCst);

                    if !files.is_empty() {
                        stream_files_to_receiver(slot_idx, files);
                    }
                }

                DragKind::Other => {
                    if !from_drag {
                        let files = read_clipboard_files();
                        if !files.is_empty() {
                            stream_files_to_receiver(slot_idx, files);
                        }
                    }
                }
            }
        }

        HANDOFF_BUSY.store(false, Ordering::SeqCst);
    });
}

pub fn switch_to_remote_slot(slot_idx: usize, was_dragging: bool, entry_coord: i32, host_span: i32) {
    #[cfg(windows)]
    let fg_hwnd = unsafe { GetForegroundWindow() };
    #[cfg(windows)]
    let click_hwnd = DRAG_CLICK_HWND.load(Ordering::Relaxed);
    #[cfg(not(windows))]
    let _ = was_dragging;

    hide_edge_overlay();

    ACTIVE_REMOTE_SLOT.store(slot_idx as u8, Ordering::SeqCst);

    // Ensure receiver has up-to-date return edge + proportional entry alignment
    let edge_pkt = format!("E {}\n", EdgeDirection::reciprocal_return_edge(slot_idx));
    let _ = send_bytes_to_slot(slot_idx, edge_pkt.as_bytes());

    let align_pkt = if slot_idx == SLOT_UP || slot_idx == SLOT_DOWN {
        format!("X {} {}\n", entry_coord, host_span.max(1))
    } else {
        format!("Y {} {}\n", entry_coord, host_span.max(1))
    };
    if !send_bytes_to_slot(slot_idx, align_pkt.as_bytes()) {
        ACTIVE_REMOTE_SLOT.store(255, Ordering::SeqCst);
        return;
    }

    #[cfg(windows)]
    LBUTTON_DOWN.store(false, Ordering::SeqCst);

    IS_REMOTE.store(true, Ordering::SeqCst);
    IGNORE_WARP.store(true, Ordering::SeqCst);

    #[cfg(any(windows, target_os = "macos"))]
    let cx = CENTER_X.load(Ordering::Relaxed);
    #[cfg(any(windows, target_os = "macos"))]
    let cy = CENTER_Y.load(Ordering::Relaxed);
    #[cfg(not(any(windows, target_os = "macos")))]
    let _ = (CENTER_X.load(Ordering::Relaxed), CENTER_Y.load(Ordering::Relaxed));

    #[cfg(windows)]
    unsafe {
        SetCursorPos(cx, cy);
    }

    #[cfg(target_os = "macos")]
    macos_host::set_macos_remote_capture(true, cx, cy);

    #[cfg(windows)]
    if was_dragging {
        execute_handoff(slot_idx, fg_hwnd, click_hwnd, true);
    }
}

pub fn switch_to_local(slot_idx: usize, rel_coord: Option<i32>, remote_span: Option<i32>) {
    LAST_LOCAL_RETURN_MS.store(monotonic_ms(), Ordering::SeqCst);
    IS_REMOTE.store(false, Ordering::SeqCst);
    ACTIVE_REMOTE_SLOT.store(255, Ordering::SeqCst);
    IGNORE_WARP.store(true, Ordering::SeqCst);
    hide_edge_overlay();

    let min_x = HOST_MIN_X.load(Ordering::Relaxed);
    let max_x = HOST_MAX_X.load(Ordering::Relaxed);
    let min_y = HOST_MIN_Y.load(Ordering::Relaxed);
    let max_y = HOST_MAX_Y.load(Ordering::Relaxed);
    let host_w = (max_x - min_x).max(1);
    let host_h = (max_y - min_y).max(1);

    let scale_coord = |host_min: i32, host_len: i32| -> i32 {
        match (rel_coord, remote_span) {
            (Some(c), Some(s)) if s > 0 => {
                (host_min + ((c as i64 * host_len as i64) / s as i64) as i32)
                    .clamp(host_min + 20, (host_min + host_len - 20).max(host_min + 20))
            }
            (Some(c), _) => c.clamp(host_min + 20, (host_min + host_len - 20).max(host_min + 20)),
            (None, _) => host_min + host_len / 2,
        }
    };

    let (x, y) = match slot_idx {
        SLOT_LEFT => ((min_x + 55).min(max_x - 10), scale_coord(min_y, host_h)),
        SLOT_UP => (scale_coord(min_x, host_w), (min_y + 55).min(max_y - 10)),
        SLOT_DOWN => (scale_coord(min_x, host_w), (max_y - 55).max(min_y + 10)),
        _ => ((max_x - 55).max(min_x + 10), scale_coord(min_y, host_h)),
    };

    #[cfg(windows)]
    unsafe {
        ClipCursor(std::ptr::null());
        // Snap (x, y) inside the nearest real physical monitor's bounds so staggered
        // multi-monitor desktops never place the cursor in invisible dead space!
        let near_mon = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        if near_mon != 0 && GetMonitorInfoW(near_mon, &mut mi) != 0 {
            let rc = mi.rcMonitor;
            let mw = (rc.right - rc.left).max(1);
            let mh = (rc.bottom - rc.top).max(1);
            let scale_mon = |m_min: i32, m_len: i32| -> i32 {
                match (rel_coord, remote_span) {
                    (Some(c), Some(s)) if s > 0 => {
                        (m_min + ((c as i64 * m_len as i64) / s as i64) as i32)
                            .clamp(m_min + 20, (m_min + m_len - 20).max(m_min + 20))
                    }
                    (Some(c), _) => c.clamp(m_min + 20, (m_min + m_len - 20).max(m_min + 20)),
                    (None, _) => m_min + m_len / 2,
                }
            };
            let (mx, my) = match slot_idx {
                SLOT_LEFT => ((rc.left + 55).min(rc.right - 10), scale_mon(rc.top, mh)),
                SLOT_UP => (scale_mon(rc.left, mw), (rc.top + 55).min(rc.bottom - 10)),
                SLOT_DOWN => (scale_mon(rc.left, mw), (rc.bottom - 55).max(rc.top + 10)),
                _ => ((rc.right - 55).max(rc.left + 10), scale_mon(rc.top, mh)),
            };
            SetCursorPos(mx, my);
        } else {
            SetCursorPos(x, y);
        }
    }

    #[cfg(target_os = "linux")]
    linux_host::warp_linux_cursor(x, y);

    #[cfg(target_os = "macos")]
    macos_host::set_macos_remote_capture(false, x, y);
}

#[cfg(windows)]
fn update_windows_screen_bounds() {
    unsafe {
        let sw = GetSystemMetrics(SM_CXSCREEN).max(1280);
        let sh = GetSystemMetrics(SM_CYSCREEN).max(720);
        CENTER_X.store(sw / 2, Ordering::SeqCst);
        CENTER_Y.store(sh / 2, Ordering::SeqCst);

        let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
        let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN);
        if vw > 0 && vh > 0 {
            let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
            let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
            HOST_MIN_X.store(vx, Ordering::SeqCst);
            HOST_MAX_X.store(vx + vw, Ordering::SeqCst);
            HOST_MIN_Y.store(vy, Ordering::SeqCst);
            HOST_MAX_Y.store(vy + vh, Ordering::SeqCst);
        } else {
            HOST_MIN_X.store(0, Ordering::SeqCst);
            HOST_MAX_X.store(sw, Ordering::SeqCst);
            HOST_MIN_Y.store(0, Ordering::SeqCst);
            HOST_MAX_Y.store(sh, Ordering::SeqCst);
        }
    }
}

#[cfg(windows)]
fn start_windows_bounds_refresher() {
    thread::spawn(|| loop {
        thread::sleep(Duration::from_secs(3));
        update_windows_screen_bounds();
    });
}

// -------------------------------------------------------------
// Visual Edge Glow Barrier Window (Win32 Layered Window)
// -------------------------------------------------------------

#[cfg(windows)]
unsafe extern "system" fn overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = std::mem::zeroed();
            let hdc = BeginPaint(hwnd, &mut ps);
            let brush = CreateSolidBrush(0x00BE_D400); // Cyan/Teal BGR accent glow
            let mut rc: RECT = std::mem::zeroed();
            GetClientRect(hwnd, &mut rc);
            FillRect(hdc, &rc, brush);
            DeleteObject(brush as _);
            EndPaint(hwnd, &ps);
            0
        }
        WM_ERASEBKGND => 1,
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

#[cfg(windows)]
fn init_edge_overlay() {
    thread::spawn(|| unsafe {
        let class_name: Vec<u16> = "TailscaleKVMEdgeOverlay\0".encode_utf16().collect();
        let wnd_class = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(overlay_wnd_proc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: 0,
            hIcon: 0,
            hCursor: 0,
            hbrBackground: 0,
            lpszMenuName: std::ptr::null(),
            lpszClassName: class_name.as_ptr(),
        };
        RegisterClassW(&wnd_class);

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_LAYERED | WS_EX_NOACTIVATE,
            class_name.as_ptr(),
            std::ptr::null(),
            WS_POPUP,
            0,
            0,
            3,
            1080,
            0,
            0,
            0,
            std::ptr::null(),
        );

        if hwnd != 0 {
            SetLayeredWindowAttributes(hwnd, 0, 195, LWA_ALPHA);
            OVERLAY_HWND.store(hwnd as isize, Ordering::SeqCst);

            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, hwnd, 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    });
}

#[cfg(windows)]
fn show_edge_overlay(slot_idx: u8, rc: RECT) {
    let was_active = OVERLAY_ACTIVE.swap(true, Ordering::SeqCst);
    let prev_dir = OVERLAY_DIR.swap(slot_idx, Ordering::SeqCst);
    if !was_active || prev_dir != slot_idx {
        let hwnd = OVERLAY_HWND.load(Ordering::SeqCst) as HWND;
        if hwnd != 0 {
            unsafe {
                let w = (rc.right - rc.left).max(1);
                let h = (rc.bottom - rc.top).max(1);

                let (x_pos, y_pos, w_size, h_size) = match slot_idx as usize {
                    SLOT_LEFT => (rc.left, rc.top, 3, h),
                    SLOT_UP => (rc.left, rc.top, w, 3),
                    SLOT_DOWN => (rc.left, rc.bottom - 3, w, 3),
                    _ => (rc.right - 3, rc.top, 3, h),
                };

                SetWindowPos(
                    hwnd,
                    HWND_TOPMOST,
                    x_pos,
                    y_pos,
                    w_size,
                    h_size,
                    SWP_NOACTIVATE | SWP_SHOWWINDOW,
                );
                InvalidateRect(hwnd, std::ptr::null(), 1);
            }
        }
    }
}

fn hide_edge_overlay() {
    #[cfg(windows)]
    if OVERLAY_ACTIVE.swap(false, Ordering::SeqCst) {
        OVERLAY_DIR.store(255, Ordering::SeqCst);
        let hwnd = OVERLAY_HWND.load(Ordering::SeqCst) as HWND;
        if hwnd != 0 {
            unsafe {
                ShowWindow(hwnd, SW_HIDE);
            }
        }
    }
}

#[cfg(windows)]
unsafe extern "system" fn mouse_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let mouse_data = *(lparam as *const MSLLHOOKSTRUCT);
        let x = mouse_data.pt.x;
        let y = mouse_data.pt.y;
        let msg = wparam as u32;
        let injected = (mouse_data.flags & LLMHF_INJECTED) != 0;

        if injected && msg != WM_MOUSEMOVE {
            if msg == WM_LBUTTONUP {
                LBUTTON_DOWN.store(false, Ordering::SeqCst);
            }
            return CallNextHookEx(MOUSE_HOOK, code, wparam, lparam);
        }

        let remote = IS_REMOTE.load(Ordering::SeqCst);

        if !remote {
            if msg == WM_LBUTTONDOWN {
                LBUTTON_DOWN.store(true, Ordering::SeqCst);
                DRAG_START_X.store(x, Ordering::Relaxed);
                DRAG_START_Y.store(y, Ordering::Relaxed);
                DRAG_CLICK_HWND.store(WindowFromPoint(mouse_data.pt), Ordering::Relaxed);
            } else if msg == WM_LBUTTONUP {
                LBUTTON_DOWN.store(false, Ordering::SeqCst);
            }

            if monotonic_ms().saturating_sub(LAST_LOCAL_RETURN_MS.load(Ordering::Relaxed)) < 150 {
                return CallNextHookEx(MOUSE_HOOK, code, wparam, lparam);
            }

            let cur_mon = MonitorFromPoint(mouse_data.pt, MONITOR_DEFAULTTONEAREST);
            let mut mi: MONITORINFO = std::mem::zeroed();
            mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            let rc = if cur_mon != 0 && GetMonitorInfoW(cur_mon, &mut mi) != 0 {
                mi.rcMonitor
            } else {
                RECT {
                    left: HOST_MIN_X.load(Ordering::Relaxed),
                    top: HOST_MIN_Y.load(Ordering::Relaxed),
                    right: HOST_MAX_X.load(Ordering::Relaxed),
                    bottom: HOST_MAX_Y.load(Ordering::Relaxed),
                }
            };
            let span_w = (rc.right - rc.left).max(1);
            let span_h = (rc.bottom - rc.top).max(1);

            // Lock-free O(1) atomic checks per physical edge + MonitorFromPoint outer-edge check
            let right_ready = SLOT_CONNECTED[SLOT_RIGHT].load(Ordering::Relaxed);
            let left_ready = SLOT_CONNECTED[SLOT_LEFT].load(Ordering::Relaxed);
            let up_ready = SLOT_CONNECTED[SLOT_UP].load(Ordering::Relaxed);
            let down_ready = SLOT_CONNECTED[SLOT_DOWN].load(Ordering::Relaxed);

            let near_slot: Option<u8> = if right_ready
                && x >= rc.right - 45
                && MonitorFromPoint(POINT { x: rc.right + 4, y }, MONITOR_DEFAULTTONULL) == 0
            {
                Some(SLOT_RIGHT as u8)
            } else if left_ready
                && x <= rc.left + 45
                && MonitorFromPoint(POINT { x: rc.left - 4, y }, MONITOR_DEFAULTTONULL) == 0
            {
                Some(SLOT_LEFT as u8)
            } else if up_ready
                && y <= rc.top + 45
                && MonitorFromPoint(POINT { x, y: rc.top - 4 }, MONITOR_DEFAULTTONULL) == 0
            {
                Some(SLOT_UP as u8)
            } else if down_ready
                && y >= rc.bottom - 45
                && MonitorFromPoint(POINT { x, y: rc.bottom + 4 }, MONITOR_DEFAULTTONULL) == 0
            {
                Some(SLOT_DOWN as u8)
            } else {
                None
            };

            if let Some(s) = near_slot {
                show_edge_overlay(s, rc);
            } else {
                hide_edge_overlay();
            }

            // Detect outer boundary crossing (excluding 50px corner deadzones)
            let mut triggered_slot: Option<usize> = None;
            if near_slot == Some(SLOT_RIGHT as u8)
                && x >= rc.right - 3
                && y > rc.top + 50
                && y < rc.bottom - 50
            {
                triggered_slot = Some(SLOT_RIGHT);
            } else if near_slot == Some(SLOT_LEFT as u8)
                && x <= rc.left + 3
                && y > rc.top + 50
                && y < rc.bottom - 50
            {
                triggered_slot = Some(SLOT_LEFT);
            } else if near_slot == Some(SLOT_UP as u8)
                && y <= rc.top + 3
                && x > rc.left + 50
                && x < rc.right - 50
            {
                triggered_slot = Some(SLOT_UP);
            } else if near_slot == Some(SLOT_DOWN as u8)
                && y >= rc.bottom - 3
                && x > rc.left + 50
                && x < rc.right - 50
            {
                triggered_slot = Some(SLOT_DOWN);
            }

            if let Some(slot_idx) = triggered_slot {
                let held = LBUTTON_DOWN.load(Ordering::SeqCst);
                let was_dragging = if slot_idx == SLOT_UP || slot_idx == SLOT_DOWN {
                    let dy_drag = (y - DRAG_START_Y.load(Ordering::Relaxed)).abs();
                    held && dy_drag >= 35
                } else {
                    let dx_drag = (x - DRAG_START_X.load(Ordering::Relaxed)).abs();
                    held && dx_drag >= 35
                };

                let (entry_coord, span) = if slot_idx == SLOT_UP || slot_idx == SLOT_DOWN {
                    ((x - rc.left).max(0), span_w)
                } else {
                    ((y - rc.top).max(0), span_h)
                };

                switch_to_remote_slot(slot_idx, was_dragging, entry_coord, span);
                if IS_REMOTE.load(Ordering::SeqCst) {
                    return 1;
                }
            }
            return CallNextHookEx(MOUSE_HOOK, code, wparam, lparam);
        }

        // In Remote Mode:
        if msg == WM_MOUSEMOVE {
            if IGNORE_WARP.swap(false, Ordering::SeqCst) {
                return 1;
            }

            let cx = CENTER_X.load(Ordering::Relaxed);
            let cy = CENTER_Y.load(Ordering::Relaxed);
            let dx = x - cx;
            let dy = y - cy;

            if dx != 0 || dy != 0 {
                let mut buf = [0u8; 32];
                let mut cur = Cursor::new(&mut buf[..]);
                if writeln!(cur, "M {} {}", dx, dy).is_ok() {
                    let len = cur.position() as usize;
                    send_to_active_remote(&buf[..len]);
                }

                IGNORE_WARP.store(true, Ordering::SeqCst);
                SetCursorPos(cx, cy);
            }
            return 1;
        }

        if msg == WM_LBUTTONDOWN || msg == WM_RBUTTONDOWN || msg == WM_MBUTTONDOWN {
            let pkt: &[u8] = match msg {
                WM_LBUTTONDOWN => b"C left down\n",
                WM_RBUTTONDOWN => b"C right down\n",
                _ => b"C middle down\n",
            };
            send_to_active_remote(pkt);
            return 1;
        }

        if msg == WM_LBUTTONUP || msg == WM_RBUTTONUP || msg == WM_MBUTTONUP {
            let pkt: &[u8] = match msg {
                WM_LBUTTONUP => b"C left up\n",
                WM_RBUTTONUP => b"C right up\n",
                _ => b"C middle up\n",
            };
            send_to_active_remote(pkt);
            return 1;
        }

        if msg == WM_MOUSEWHEEL {
            let delta = (mouse_data.mouseData >> 16) as i16;
            let pkt: &[u8] = if delta > 0 { b"S 0 1\n" } else { b"S 0 -1\n" };
            send_to_active_remote(pkt);
            return 1;
        }

        if msg == WM_MOUSEHWHEEL {
            let delta = (mouse_data.mouseData >> 16) as i16;
            let pkt: &[u8] = if delta > 0 { b"S 1 0\n" } else { b"S -1 0\n" };
            send_to_active_remote(pkt);
            return 1;
        }
    }

    CallNextHookEx(MOUSE_HOOK, code, wparam, lparam)
}

#[cfg(windows)]
unsafe extern "system" fn keyboard_hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kbd_data = *(lparam as *const KBDLLHOOKSTRUCT);

        if (kbd_data.flags & LLKHF_INJECTED) != 0 {
            return CallNextHookEx(KEYBOARD_HOOK, code, wparam, lparam);
        }

        let vk = kbd_data.vkCode as u16;
        let msg = wparam as u32;
        let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;

        if vk == VK_CONTROL || vk == VK_LCONTROL || vk == VK_RCONTROL {
            CTRL_DOWN.store(is_down, Ordering::SeqCst);
        } else if vk == VK_MENU || vk == VK_LMENU || vk == VK_RMENU {
            ALT_DOWN.store(is_down, Ordering::SeqCst);
        } else if vk == VK_LWIN || vk == VK_RWIN {
            WIN_DOWN.store(is_down, Ordering::SeqCst);
        }

        // Working Directory Teleport (Win + Alt + T)
        if !IS_REMOTE.load(Ordering::Relaxed)
            && is_down
            && vk == b'T' as u16
            && WIN_DOWN.load(Ordering::Relaxed)
            && ALT_DOWN.load(Ordering::Relaxed)
        {
            if let Some(slot_idx) = first_connected_slot() {
                let _ = send_bytes_to_slot(slot_idx, b"D .\n");
                return 1;
            }
        }

        // Hotkey to fling active Browser Tab or selected Explorer File(s): Win + Arrow Key matching slot
        if !IS_REMOTE.load(Ordering::Relaxed) && is_down && WIN_DOWN.load(Ordering::Relaxed) {
            let hotkey_slot = match vk {
                VK_RIGHT if SLOT_CONNECTED[SLOT_RIGHT].load(Ordering::Relaxed) => Some(SLOT_RIGHT),
                VK_LEFT if SLOT_CONNECTED[SLOT_LEFT].load(Ordering::Relaxed) => Some(SLOT_LEFT),
                VK_UP if SLOT_CONNECTED[SLOT_UP].load(Ordering::Relaxed) => Some(SLOT_UP),
                VK_DOWN if SLOT_CONNECTED[SLOT_DOWN].load(Ordering::Relaxed) => Some(SLOT_DOWN),
                VK_RIGHT => first_connected_slot(),
                _ => None,
            };
            if let Some(slot_idx) = hotkey_slot {
                let fg = GetForegroundWindow();
                execute_handoff(slot_idx, fg, 0, false);
                return 1;
            }
        }

        // Emergency Release Hotkeys: F12 (0x7B), Pause (0x13), ScrollLock (0x91)
        if vk == 0x7B || vk == 0x13 || vk == 0x91 {
            if IS_REMOTE.load(Ordering::SeqCst) && is_down {
                let cur_slot = (ACTIVE_REMOTE_SLOT.load(Ordering::Relaxed) as usize).min(3);
                switch_to_local(cur_slot, None, None);
                return 1;
            }
        }

        if !IS_REMOTE.load(Ordering::SeqCst) {
            return CallNextHookEx(KEYBOARD_HOOK, code, wparam, lparam);
        }

        // In Remote Mode: map VK -> evdev and send to active remote slot
        if let Some(evdev_code) = vk_map::translate_vk(vk as u32) {
            let action = if is_down { "down" } else { "up" };
            let mut buf = [0u8; 24];
            let mut cur = Cursor::new(&mut buf[..]);
            if writeln!(cur, "K {} {}", evdev_code, action).is_ok() {
                let len = cur.position() as usize;
                send_to_active_remote(&buf[..len]);
            }
        }
        return 1;
    }

    CallNextHookEx(KEYBOARD_HOOK, code, wparam, lparam)
}

#[cfg(windows)]
fn start_windows_clipboard_sync() {
    thread::spawn(|| {
        LAST_CLIP_SEQ.store(unsafe { GetClipboardSequenceNumber() }, Ordering::SeqCst);

        loop {
            thread::sleep(Duration::from_millis(500));

            if SUPPRESS_CLIPBOARD_SYNC.load(Ordering::SeqCst) || !any_slot_connected() {
                continue;
            }

            let cur_seq = unsafe { GetClipboardSequenceNumber() };
            let prev_seq = LAST_CLIP_SEQ.load(Ordering::SeqCst);
            if cur_seq == prev_seq {
                continue;
            }
            LAST_CLIP_SEQ.store(cur_seq, Ordering::SeqCst);

            if let Some(dib_bytes) = read_clipboard_dib_bytes() {
                if !dib_bytes.is_empty() {
                    stream_image_to_connected_receivers(dib_bytes);
                    continue;
                }
            }

            if let Some(text) = read_clipboard_unicode_text() {
                if !text.is_empty() {
                    broadcast_clipboard_text(&text);
                }
            }
        }
    });
}

fn spawn_slot_worker(slot_idx: usize) {
    thread::spawn(move || {
        let slot_name = web_gui::index_to_slot_name(slot_idx);
        let chan = &pool()[slot_idx];
        let mut retry_delay_ms: u64 = 1000;

        loop {
            let gen_snapshot = chan.generation.load(Ordering::SeqCst);
            let target_opt = chan.target_ip.lock().ok().and_then(|g| g.clone());

            let Some(target_ip) = target_opt else {
                retry_delay_ms = 1000;
                SLOT_CONNECTED[slot_idx].store(false, Ordering::SeqCst);
                chan.wait_timeout(Duration::from_secs(30));
                continue;
            };

            if let Some(addr) = resolve_target_addr(&target_ip) {
                if let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(1400))
                {
                    let _ = stream.set_nodelay(true);
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

                    // Abort immediately if slot assignment changed during connect handshake
                    if chan.generation.load(Ordering::SeqCst) != gen_snapshot {
                        let _ = stream.shutdown(Shutdown::Both);
                        retry_delay_ms = 1000;
                        continue;
                    }

                    let reciprocal = EdgeDirection::reciprocal_return_edge(slot_idx);
                    let handshake = format!("H control\nE {}\nREC_PING\n", reciprocal);

                    if stream.write_all(handshake.as_bytes()).is_ok() {
                        retry_delay_ms = 1000;
                        if let Ok(reader_stream) = stream.try_clone() {
                            if let Ok(mut w_lock) = chan.writer.lock() {
                                *w_lock = Some(stream);
                            }
                            SLOT_CONNECTED[slot_idx].store(true, Ordering::SeqCst);
                            println!(
                                "[+] Slot [{}] connected to {} (Port 9876, receiver return edge: {})",
                                slot_name, target_ip, reciprocal
                            );

                            let reader = BufReader::new(reader_stream);
                            for line in reader.lines() {
                                if chan.generation.load(Ordering::SeqCst) != gen_snapshot {
                                    break;
                                }
                                match line {
                                    Ok(line_str) => {
                                        let trimmed = line_str.trim_end_matches(['\r', '\n']);
                                        if trimmed.starts_with("switch_to_local") {
                                            let mut parts = trimmed.split_whitespace().skip(1);
                                            let coord =
                                                parts.next().and_then(|v| v.parse::<i32>().ok());
                                            let span =
                                                parts.next().and_then(|v| v.parse::<i32>().ok());
                                            if IS_REMOTE.load(Ordering::SeqCst)
                                                && (ACTIVE_REMOTE_SLOT.load(Ordering::SeqCst)
                                                    == slot_idx as u8
                                                    || ACTIVE_REMOTE_SLOT.load(Ordering::SeqCst)
                                                        == 255)
                                            {
                                                switch_to_local(slot_idx, coord, span);
                                            }
                                        } else if trimmed == "{\"type\":\"switch_to_local\"}" {
                                            if IS_REMOTE.load(Ordering::SeqCst) {
                                                switch_to_local(slot_idx, None, None);
                                            }
                                        } else if let Some(escaped_clip) =
                                            trimmed.strip_prefix("T ")
                                        {
                                            let text = unescape_text_line(escaped_clip);
                                            write_host_clipboard_text(&text);
                                        } else if let Some(url) = trimmed.strip_prefix("U ") {
                                            open_url_on_host(url.trim());
                                        } else if let Some(dir_p) = trimmed.strip_prefix("D ") {
                                            open_terminal_on_host(dir_p.trim());
                                        } else if trimmed.starts_with("REC_") {
                                            on_rec_control_line(slot_idx, trimmed);
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }
                        }
                    }
                }
            }

            if chan.generation.load(Ordering::SeqCst) == gen_snapshot {
                SLOT_CONNECTED[slot_idx].store(false, Ordering::SeqCst);
                if let Ok(mut w_lock) = chan.writer.lock() {
                    *w_lock = None;
                }
                if IS_REMOTE.load(Ordering::SeqCst)
                    && ACTIVE_REMOTE_SLOT.load(Ordering::SeqCst) == slot_idx as u8
                {
                    switch_to_local(slot_idx, None, None);
                }
                chan.wait_timeout(Duration::from_millis(retry_delay_ms));
                retry_delay_ms = (retry_delay_ms * 2).min(15_000);
            } else {
                retry_delay_ms = 1000;
            }
        }
    });
}

fn start_all_slot_workers() {
    for idx in 0..4 {
        spawn_slot_worker(idx);
    }
}

pub fn shutdown_and_exit() {
    println!("[*] Shutting down Tailscale KVM Host cleanly...");
    hide_edge_overlay();
    #[cfg(windows)]
    unsafe {
        ClipCursor(std::ptr::null());
        if MOUSE_HOOK != 0 {
            UnhookWindowsHookEx(MOUSE_HOOK);
        }
        if KEYBOARD_HOOK != 0 {
            UnhookWindowsHookEx(KEYBOARD_HOOK);
        }
    }
    #[cfg(target_os = "macos")]
    {
        let cx = CENTER_X.load(Ordering::Relaxed);
        let cy = CENTER_Y.load(Ordering::Relaxed);
        macos_host::set_macos_remote_capture(false, cx, cy);
    }
    std::process::exit(0);
}

fn resolve_initial_layout() -> ([Option<String>; 4], bool) {
    let dir = get_config_direction();
    // 1. Explicit CLI flags (--target / -t) take highest priority
    if let Some(cli_ip) = explicit_cli_target() {
        let mut slots = [None, None, None, None];
        slots[dir.to_slot_index()] = Some(cli_ip);
        return (slots, false);
    }

    // 2. Saved GUI layout (kvm_layout.json) takes priority over legacy config.txt
    if let Some(saved) = load_layout_file() {
        return (saved, false);
    }

    // 3. Legacy env / config.txt fallback
    if let Some(cfg_ip) = legacy_env_or_config_target() {
        let mut slots = [None, None, None, None];
        slots[dir.to_slot_index()] = Some(cfg_ip);
        return (slots, true);
    }

    // 4. Smart zero-config startup: scan Tailscale peers and ONLY auto-assign a peer
    // that actually has KVM Receiver listening on TCP port 9876!
    let peers = web_gui::refresh_tailscale_peers();
    let mut slots = [None, None, None, None];
    if let Some(ready_peer) = peers.iter().find(|p| p.kvm_ready) {
        println!(
            "[*] Smart auto-discovery found active KVM Receiver: {} ({}) -> placed on {:?} screen",
            ready_peer.name, ready_peer.ip, dir
        );
        slots[dir.to_slot_index()] = Some(ready_peer.ip.clone());
        return (slots, true);
    }

    (slots, false)
}

fn print_usage() {
    println!("Tailscale Universal KVM — Host Controller");
    println!("Usage: tailscale-kvm-host [OPTIONS]");
    println!();
    println!("Options:");
    println!("  -t, --target <IP>       Optional initial receiver Tailscale IP (can also be arranged in Web GUI)");
    println!("  -d, --direction <SIDE>  Screen side for --target: right | left | up | down (default: right)");
    println!("      --no-browser        Do not auto-open http://localhost:9877 in browser on startup");
    println!("  -h, --help              Show this help message");
}

fn main() {
    let _ = monotonic_ms();
    let args: Vec<String> = env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_usage();
        return;
    }
    let auto_open_browser = !args.iter().any(|a| a == "--no-browser");

    println!("=== Tailscale KVM Universal Host Controller ===");

    start_all_slot_workers();
    let (initial_slots, save_initial) = resolve_initial_layout();
    apply_slot_layout(initial_slots, save_initial);

    // Launch Web GUI server on http://localhost:9877
    web_gui::start_gui_server(9877, auto_open_browser);
    start_reverse_transfer_listener();

    #[cfg(windows)]
    {
        update_windows_screen_bounds();
        println!(
            "[*] Windows Host virtual screen bounds: X=[{}, {}], Y=[{}, {}] (Emergency release: F12)",
            HOST_MIN_X.load(Ordering::SeqCst),
            HOST_MAX_X.load(Ordering::SeqCst),
            HOST_MIN_Y.load(Ordering::SeqCst),
            HOST_MAX_Y.load(Ordering::SeqCst)
        );
        start_windows_bounds_refresher();
        init_edge_overlay();
        start_windows_clipboard_sync();

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

    #[cfg(target_os = "linux")]
    {
        linux_host::run_linux_host_loop();
    }

    #[cfg(target_os = "macos")]
    {
        macos_host::run_macos_host_loop();
    }

    #[cfg(not(any(windows, target_os = "linux", target_os = "macos")))]
    {
        loop {
            thread::sleep(Duration::from_secs(3600));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edge_direction_and_reciprocals() {
        assert_eq!(EdgeDirection::parse("right").to_slot_index(), SLOT_RIGHT);
        assert_eq!(EdgeDirection::parse("left").to_slot_index(), SLOT_LEFT);
        assert_eq!(EdgeDirection::parse("top").to_slot_index(), SLOT_UP);
        assert_eq!(EdgeDirection::parse("bottom").to_slot_index(), SLOT_DOWN);

        assert_eq!(EdgeDirection::reciprocal_return_edge(SLOT_RIGHT), "left");
        assert_eq!(EdgeDirection::reciprocal_return_edge(SLOT_LEFT), "right");
        assert_eq!(EdgeDirection::reciprocal_return_edge(SLOT_UP), "bottom");
        assert_eq!(EdgeDirection::reciprocal_return_edge(SLOT_DOWN), "top");
    }

    #[test]
    fn test_layout_deduplication() {
        apply_slot_layout(
            [
                Some("100.99.142.70".to_string()),
                Some("100.99.142.70".to_string()),
                None,
                None,
            ],
            false,
        );
        let mirrored = web_gui::get_all_slots();
        assert_eq!(mirrored[0].as_deref(), Some("100.99.142.70"));
        assert_eq!(mirrored[1], None);
        apply_slot_layout([None, None, None, None], false);
    }
}
