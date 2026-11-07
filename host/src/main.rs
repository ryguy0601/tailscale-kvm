#![windows_subsystem = "windows"]
#![cfg_attr(not(windows), allow(dead_code, unused_imports, unused_variables))]

mod vk_map;
mod web_gui;

use std::env;
use std::fs;
use std::io::{BufRead, BufReader, Cursor, Read, Write};
use std::net::{SocketAddr, TcpStream, ToSocketAddrs};
#[cfg(windows)]
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicIsize, AtomicU32, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::{Duration, Instant};

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

static IS_CONNECTED: AtomicBool = AtomicBool::new(false);
static IS_REMOTE: AtomicBool = AtomicBool::new(false);
static IGNORE_WARP: AtomicBool = AtomicBool::new(false);
static HANDOFF_BUSY: AtomicBool = AtomicBool::new(false);
static SUPPRESS_CLIPBOARD_SYNC: AtomicBool = AtomicBool::new(false);
static LAST_CLIP_SEQ: AtomicU32 = AtomicU32::new(0);

static STREAM_SENDER: Mutex<Option<TcpStream>> = Mutex::new(None);
static TARGET_PEER_IP: Mutex<Option<String>> = Mutex::new(None);

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
}

static EDGE_DIRECTION: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0); // 0=Right, 1=Left, 2=Up, 3=Down
static SCREEN_WIDTH: AtomicI32 = AtomicI32::new(1920);
static SCREEN_HEIGHT: AtomicI32 = AtomicI32::new(1080);
static CENTER_X: AtomicI32 = AtomicI32::new(960);
static CENTER_Y: AtomicI32 = AtomicI32::new(540);

static LBUTTON_DOWN: AtomicBool = AtomicBool::new(false);
static DRAG_START_X: AtomicI32 = AtomicI32::new(0);
static DRAG_START_Y: AtomicI32 = AtomicI32::new(0);
static DRAG_CLICK_HWND: AtomicIsize = AtomicIsize::new(0);

static CTRL_DOWN: AtomicBool = AtomicBool::new(false);
static ALT_DOWN: AtomicBool = AtomicBool::new(false);
static WIN_DOWN: AtomicBool = AtomicBool::new(false);

#[cfg(windows)]
static OVERLAY_HWND: AtomicIsize = AtomicIsize::new(0);
static OVERLAY_ACTIVE: AtomicBool = AtomicBool::new(false);

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

const MAX_CLIPBOARD_TEXT_BYTES: usize = 1_048_576; // 1 MB cap for text clipboard sync
const MAX_IMAGE_BYTES: usize = 67_108_864; // 64 MB cap for image clipboard sync
const FILE_CHUNK_SIZE: usize = 65_536; // 64 KB streaming chunk buffer

#[derive(Clone, Copy, PartialEq, Eq)]
enum DragKind {
    Browser,
    Explorer,
    Other,
}

fn auto_discover_peer_ip() -> Option<String> {
    let mut cmd = std::process::Command::new("tailscale");
    cmd.args(["status", "--json"]);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    if let Ok(output) = cmd.output() {
        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
            if let Some(peers) = val.get("Peer").and_then(|p| p.as_object()) {
                for (_, peer) in peers {
                    let online = peer.get("Online").and_then(|o| o.as_bool()).unwrap_or(false);
                    if online {
                        if let Some(ips) = peer.get("TailscaleIPs").and_then(|i| i.as_array()) {
                            if let Some(first_ip) = ips.get(0).and_then(|ip| ip.as_str()) {
                                return Some(first_ip.to_string());
                            }
                        }
                    }
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

fn get_config_ip() -> String {
    let args: Vec<String> = env::args().collect();
    for i in 1..args.len() {
        if args[i] == "--target" || args[i] == "-t" || args[i] == "--ip" {
            if i + 1 < args.len() {
                return args[i + 1].trim().to_string();
            }
        }
        if let Some(val) = args[i].strip_prefix("--target=") {
            return val.trim().to_string();
        }
        if let Some(val) = args[i].strip_prefix("--ip=") {
            return val.trim().to_string();
        }
    }
    if args.len() > 1 && !args[1].starts_with('-') && !args[1].trim().is_empty() {
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
            if let Some(rest) = line.strip_prefix("OMARCHY_IP=") {
                if !rest.trim().is_empty() {
                    return rest.trim().to_string();
                }
            }
        }
    }

    auto_discover_peer_ip().unwrap_or_else(|| "127.0.0.1".to_string())
}

#[inline(always)]
fn send_fast_bytes(bytes: &[u8]) {
    let mut disconnected = false;
    if let Ok(mut lock) = STREAM_SENDER.lock() {
        if let Some(stream) = lock.as_mut() {
            if stream.write_all(bytes).is_err() {
                *lock = None;
                disconnected = true;
            }
        }
    }
    if disconnected {
        IS_CONNECTED.store(false, Ordering::SeqCst);
        if IS_REMOTE.load(Ordering::SeqCst) {
            switch_to_local(None);
        }
    }
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
    false
}

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

fn get_windows_downloads_dir() -> PathBuf {
    if let Ok(profile) = env::var("USERPROFILE") {
        let p = PathBuf::from(profile).join("Downloads");
        if p.exists() {
            return p;
        }
    }
    PathBuf::from("C:\\Users\\Default\\Downloads")
}

fn sanitize_relative_path(raw: &str) -> PathBuf {
    let normalized = raw.replace('/', "\\");
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

fn receive_binary_file_windows<R: Read>(
    reader: &mut R,
    file_size: u64,
    raw_rel_path: &str,
) -> Result<String, std::io::Error> {
    let safe_rel = sanitize_relative_path(raw_rel_path);
    let base_downloads = get_windows_downloads_dir();
    let target_path = base_downloads.join(&safe_rel);

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
    Ok(safe_rel.to_string_lossy().into_owned())
}

fn open_url_on_windows(url: &str) {
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

        let ff_candidates = [
            "C:\\Program Files\\Mozilla Firefox\\firefox.exe",
            "C:\\Program Files (x86)\\Mozilla Firefox\\firefox.exe",
        ];
        let mut opened = false;
        for candidate in ff_candidates {
            if Path::new(candidate).exists() {
                if std::process::Command::new(candidate)
                    .arg(&formatted)
                    .spawn()
                    .is_ok()
                {
                    opened = true;
                    break;
                }
            }
        }
        if !opened {
            let _ = std::process::Command::new("cmd")
                .args(["/C", "start", "", &formatted])
                .spawn();
        }
    });
}

fn open_terminal_on_windows(dir_path: &str) {
    let d = dir_path.trim().to_string();
    thread::spawn(move || {
        let default_dir = get_windows_downloads_dir()
            .parent()
            .unwrap_or(Path::new("C:\\"))
            .to_string_lossy()
            .to_string();
        let win_path = if d.contains("/Work") {
            let userprofile = env::var("USERPROFILE").unwrap_or_else(|_| default_dir);
            let sub = d.split("/Work").nth(1).unwrap_or("");
            format!("{}\\{}", userprofile, sub.replace('/', "\\").trim_start_matches('\\'))
        } else {
            env::var("USERPROFILE").unwrap_or_else(|_| default_dir)
        };

        let _ = std::process::Command::new("wt.exe")
            .args(["-d", &win_path])
            .spawn()
            .or_else(|_| {
                std::process::Command::new("powershell.exe")
                    .args(["-NoExit", "-Command", &format!("Set-Location '{}'", win_path)])
                    .spawn()
            });
    });
}

/// Incoming transfer listener on Windows port 9876 so Omarchy can stream files, images, tabs & teleports
fn start_windows_server_listener() {
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
                            // "B <size> <rel_path>\n" -> Binary file stream from Omarchy
                            b'B' => {
                                if trimmed.len() > 2 {
                                    let rest = &trimmed[2..];
                                    if let Some(space_idx) = rest.find(' ') {
                                        let size_str = &rest[..space_idx];
                                        let rel_path = rest[space_idx + 1..].trim();
                                        if let Ok(file_size) = size_str.parse::<u64>() {
                                            let _ = receive_binary_file_windows(
                                                &mut reader,
                                                file_size,
                                                rel_path,
                                            );
                                        }
                                    }
                                }
                            }

                            // "I <size>\n" -> Image clipboard stream from Omarchy
                            b'I' => {
                                if trimmed.len() > 2 {
                                    if let Ok(img_size) = trimmed[2..].trim().parse::<usize>() {
                                        let mut buf = vec![0u8; img_size];
                                        if reader.read_exact(&mut buf).is_ok() {
                                            SUPPRESS_CLIPBOARD_SYNC.store(true, Ordering::SeqCst);
                                            write_clipboard_dib_bytes(&buf);
                                            #[cfg(windows)]
                                            LAST_CLIP_SEQ.store(unsafe { GetClipboardSequenceNumber() }, Ordering::SeqCst);
                                            SUPPRESS_CLIPBOARD_SYNC.store(false, Ordering::SeqCst);
                                        }
                                    }
                                }
                            }

                            // "U <url>\n" -> Tab fling from Omarchy
                            b'U' => {
                                if trimmed.len() > 2 {
                                    let url = trimmed[2..].trim();
                                    open_url_on_windows(url);
                                }
                            }

                            // "D <dir_path>\n" -> Terminal directory teleport
                            b'D' => {
                                if trimmed.len() > 2 {
                                    let path = trimmed[2..].trim();
                                    open_terminal_on_windows(path);
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

/// Streams files over a dedicated secondary TCP connection using a constant 64 KB stack buffer.
fn stream_files_to_omarchy(paths: Vec<PathBuf>) {
    let target_ip_opt = TARGET_PEER_IP.lock().ok().and_then(|l| l.clone());
    let Some(ref target_ip) = target_ip_opt else {
        return;
    };

    let mut entries = Vec::new();
    for p in &paths {
        collect_transfer_entries(p, p, 0, &mut entries);
    }
    if entries.is_empty() {
        return;
    }

    let Some(addr) = resolve_target_addr(target_ip) else {
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

fn stream_image_to_omarchy(dib_bytes: Vec<u8>) {
    let target_ip_opt = TARGET_PEER_IP.lock().ok().and_then(|l| l.clone());
    let Some(ref target_ip) = target_ip_opt else {
        return;
    };
    let Some(addr) = resolve_target_addr(target_ip) else {
        return;
    };

    thread::spawn(move || {
        if let Ok(mut tx) = TcpStream::connect_timeout(&addr, Duration::from_secs(4)) {
            let _ = tx.set_nodelay(true);
            let header = format!("I {}\n", dib_bytes.len());
            if tx.write_all(header.as_bytes()).is_ok() {
                let _ = tx.write_all(&dib_bytes);
                let _ = tx.flush();
            }
        }
    });
}

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
fn execute_handoff(source_hwnd: HWND, fallback_hwnd: HWND, from_drag: bool) {
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
                    send_fast_bytes(pkt.as_bytes());
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
                        stream_files_to_omarchy(files);
                    }
                }

                DragKind::Other => {
                    if !from_drag {
                        let files = read_clipboard_files();
                        if !files.is_empty() {
                            stream_files_to_omarchy(files);
                        }
                    }
                }
            }
        }

        HANDOFF_BUSY.store(false, Ordering::SeqCst);
    });
}

pub fn switch_to_remote(was_dragging: bool, entry_coord: i32) {
    #[cfg(windows)]
    let fg_hwnd = unsafe { GetForegroundWindow() };
    #[cfg(windows)]
    let click_hwnd = DRAG_CLICK_HWND.load(Ordering::Relaxed);

    hide_edge_overlay();

    let dir_val = EDGE_DIRECTION.load(Ordering::Relaxed);
    // When switching from Up/Down, align horizontally (X); otherwise align vertically (Y)
    let pkt = if dir_val == 2 || dir_val == 3 {
        format!("X {}\n", entry_coord)
    } else {
        format!("Y {}\n", entry_coord)
    };
    send_fast_bytes(pkt.as_bytes());

    LBUTTON_DOWN.store(false, Ordering::SeqCst);
    IS_REMOTE.store(true, Ordering::SeqCst);
    IGNORE_WARP.store(true, Ordering::SeqCst);

    #[cfg(windows)]
    unsafe {
        SetCursorPos(
            CENTER_X.load(Ordering::Relaxed),
            CENTER_Y.load(Ordering::Relaxed),
        );
    }

    #[cfg(windows)]
    if was_dragging {
        execute_handoff(fg_hwnd, click_hwnd, true);
    }
}

pub fn switch_to_local(target_coord: Option<i32>) {
    IS_REMOTE.store(false, Ordering::SeqCst);
    IGNORE_WARP.store(true, Ordering::SeqCst);
    hide_edge_overlay();

    #[cfg(windows)]
    unsafe {
        ClipCursor(std::ptr::null());
        let sw = SCREEN_WIDTH.load(Ordering::Relaxed);
        let sh = SCREEN_HEIGHT.load(Ordering::Relaxed);
        let dir_val = EDGE_DIRECTION.load(Ordering::Relaxed);

        let (x, y) = match dir_val {
            1 => (80, target_coord.unwrap_or_else(|| CENTER_Y.load(Ordering::Relaxed))), // Host Left -> Re-enter near Left edge
            2 => (target_coord.unwrap_or_else(|| CENTER_X.load(Ordering::Relaxed)), 80), // Host Up -> Re-enter near Top edge
            3 => (target_coord.unwrap_or_else(|| CENTER_X.load(Ordering::Relaxed)), sh - 80), // Host Down -> Re-enter near Bottom edge
            _ => (sw - 80, target_coord.unwrap_or_else(|| CENTER_Y.load(Ordering::Relaxed))), // Default: Host Right -> Re-enter near Right edge
        };
        SetCursorPos(x, y);
    }
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
            let brush = CreateSolidBrush(0x00D77838); // Cyan/Teal accent border glow
            let dir = EDGE_DIRECTION.load(Ordering::Relaxed);
            let sw = SCREEN_WIDTH.load(Ordering::Relaxed);
            let sh = SCREEN_HEIGHT.load(Ordering::Relaxed);
            let mut rc = if dir == 2 || dir == 3 {
                RECT { left: 0, top: 0, right: sw, bottom: 3 }
            } else {
                RECT { left: 0, top: 0, right: 3, bottom: sh }
            };
            FillRect(hdc, &mut rc, brush);
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

        let sw = SCREEN_WIDTH.load(Ordering::Relaxed);
        let sh = SCREEN_HEIGHT.load(Ordering::Relaxed);

        let dir = EDGE_DIRECTION.load(Ordering::Relaxed);
        let (x_pos, y_pos, w_size, h_size) = match dir {
            1 => (0, 0, 3, sh),
            2 => (0, 0, sw, 3),
            3 => (0, sh - 3, sw, 3),
            _ => (sw - 3, 0, 3, sh),
        };

        let hwnd = CreateWindowExW(
            WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_TRANSPARENT | WS_EX_LAYERED,
            class_name.as_ptr(),
            std::ptr::null(),
            WS_POPUP,
            x_pos,
            y_pos,
            w_size,
            h_size,
            0,
            0,
            0,
            std::ptr::null(),
        );

        if hwnd != 0 {
            SetLayeredWindowAttributes(hwnd, 0, 180, LWA_ALPHA);
            OVERLAY_HWND.store(hwnd as isize, Ordering::SeqCst);

            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, hwnd, 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
    });
}

fn show_edge_overlay() {
    #[cfg(windows)]
    if !OVERLAY_ACTIVE.swap(true, Ordering::SeqCst) {
        let hwnd = OVERLAY_HWND.load(Ordering::SeqCst) as HWND;
        if hwnd != 0 {
            unsafe {
                ShowWindow(hwnd, SW_SHOW);
                let sw = SCREEN_WIDTH.load(Ordering::Relaxed);
                let sh = SCREEN_HEIGHT.load(Ordering::Relaxed);
                let dir = EDGE_DIRECTION.load(Ordering::Relaxed);
                let (x_pos, y_pos, w_size, h_size) = match dir {
                    1 => (0, 0, 3, sh),
                    2 => (0, 0, sw, 3),
                    3 => (0, sh - 3, sw, 3),
                    _ => (sw - 3, 0, 3, sh),
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
            }
        }
    }
}

fn hide_edge_overlay() {
    #[cfg(windows)]
    if OVERLAY_ACTIVE.swap(false, Ordering::SeqCst) {
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

            let screen_w = SCREEN_WIDTH.load(Ordering::Relaxed);
            let screen_h = SCREEN_HEIGHT.load(Ordering::Relaxed);

            // Check active GUI multi-receiver slots or fall back to CLI configured direction
            let has_left_peer = web_gui::get_slot_peer("left").is_some() || EDGE_DIRECTION.load(Ordering::Relaxed) == 1;
            let has_up_peer = web_gui::get_slot_peer("up").is_some() || EDGE_DIRECTION.load(Ordering::Relaxed) == 2;
            let has_down_peer = web_gui::get_slot_peer("down").is_some() || EDGE_DIRECTION.load(Ordering::Relaxed) == 3;
            let has_right_peer = web_gui::get_slot_peer("right").is_some() || EDGE_DIRECTION.load(Ordering::Relaxed) == 0;

            let near_edge = (has_right_peer && x >= screen_w - 45)
                || (has_left_peer && x <= 45)
                || (has_up_peer && y <= 45)
                || (has_down_peer && y >= screen_h - 45);

            if near_edge && IS_CONNECTED.load(Ordering::Relaxed) {
                show_edge_overlay();
            } else {
                hide_edge_overlay();
            }

            // Detect which boundary is crossed
            let mut triggered_dir: Option<EdgeDirection> = None;
            if has_right_peer && x >= screen_w - 3 && !(y <= 50 || y >= screen_h - 50) {
                triggered_dir = Some(EdgeDirection::Right);
            } else if has_left_peer && x <= 3 && !(y <= 50 || y >= screen_h - 50) {
                triggered_dir = Some(EdgeDirection::Left);
            } else if has_up_peer && y <= 3 && !(x <= 50 || x >= screen_w - 50) {
                triggered_dir = Some(EdgeDirection::Up);
            } else if has_down_peer && y >= screen_h - 3 && !(x <= 50 || x >= screen_w - 50) {
                triggered_dir = Some(EdgeDirection::Down);
            }

            if let Some(edge) = triggered_dir {
                if IS_CONNECTED.load(Ordering::Relaxed) {
                    let dir_val = match edge {
                        EdgeDirection::Right => 0,
                        EdgeDirection::Left => 1,
                        EdgeDirection::Up => 2,
                        EdgeDirection::Down => 3,
                    };
                    EDGE_DIRECTION.store(dir_val, Ordering::SeqCst);

                    // Reconnect to slot peer if dynamically re-routed via GUI
                    let slot_key = match edge {
                        EdgeDirection::Right => "right",
                        EdgeDirection::Left => "left",
                        EdgeDirection::Up => "up",
                        EdgeDirection::Down => "down",
                    };
                    if let Some(target_peer_ip) = web_gui::get_slot_peer(slot_key) {
                        let cur = TARGET_PEER_IP.lock().ok().and_then(|l| l.clone());
                    if cur.as_deref() != Some(&target_peer_ip) {
                            switch_active_peer(target_peer_ip);
                        }
                    }

                    let held = LBUTTON_DOWN.load(Ordering::SeqCst);
                    let was_dragging = if edge == EdgeDirection::Up || edge == EdgeDirection::Down {
                        let dy_drag = (y - DRAG_START_Y.load(Ordering::Relaxed)).abs();
                        held && dy_drag >= 35
                    } else {
                        let dx_drag = (x - DRAG_START_X.load(Ordering::Relaxed)).abs();
                        held && dx_drag >= 35
                    };

                    let entry_coord = if edge == EdgeDirection::Up || edge == EdgeDirection::Down { x } else { y };
                    switch_to_remote(was_dragging, entry_coord);
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
                    send_fast_bytes(&buf[..len]);
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
            send_fast_bytes(pkt);
            return 1;
        }

        if msg == WM_LBUTTONUP || msg == WM_RBUTTONUP || msg == WM_MBUTTONUP {
            let pkt: &[u8] = match msg {
                WM_LBUTTONUP => b"C left up\n",
                WM_RBUTTONUP => b"C right up\n",
                _ => b"C middle up\n",
            };
            send_fast_bytes(pkt);
            return 1;
        }

        if msg == WM_MOUSEWHEEL {
            let delta = (mouse_data.mouseData >> 16) as i16;
            let pkt: &[u8] = if delta > 0 { b"S 0 1\n" } else { b"S 0 -1\n" };
            send_fast_bytes(pkt);
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

        // Feature 5: Working Directory Teleport (Win + Alt + T)
        if !IS_REMOTE.load(Ordering::Relaxed)
            && is_down
            && vk == b'T' as u16
            && WIN_DOWN.load(Ordering::Relaxed)
            && ALT_DOWN.load(Ordering::Relaxed)
            && IS_CONNECTED.load(Ordering::Relaxed)
        {
            // Directory teleport hotkey
            let pkt = "D .\n";
            send_fast_bytes(pkt.as_bytes());
            return 1;
        }

        // Hotkey to fling active Browser Tab or selected Explorer File(s): Win + Right Arrow
        if !IS_REMOTE.load(Ordering::Relaxed)
            && vk == VK_RIGHT
            && is_down
            && WIN_DOWN.load(Ordering::Relaxed)
            && IS_CONNECTED.load(Ordering::Relaxed)
        {
            let fg = GetForegroundWindow();
            execute_handoff(fg, 0, false);
            return 1;
        }

        // Emergency Release Hotkeys: F12 (0x7B), Pause (0x13), ScrollLock (0x91)
        if vk == 0x7B || vk == 0x13 || vk == 0x91 {
            if IS_REMOTE.load(Ordering::SeqCst) && is_down {
                switch_to_local(None);
                return 1;
            }
        }

        if !IS_REMOTE.load(Ordering::SeqCst) {
            return CallNextHookEx(KEYBOARD_HOOK, code, wparam, lparam);
        }

        // In Remote Mode: map and send to Omarchy, swallow on Windows
        if let Some(evdev_code) = vk_map::translate_vk(vk as u32) {
            let action = if is_down { "down" } else { "up" };
            let mut buf = [0u8; 24];
            let mut cur = Cursor::new(&mut buf[..]);
            if writeln!(cur, "K {} {}", evdev_code, action).is_ok() {
                let len = cur.position() as usize;
                send_fast_bytes(&buf[..len]);
            }
        }
        return 1;
    }

    CallNextHookEx(KEYBOARD_HOOK, code, wparam, lparam)
}

/// Zero-allocation idle clipboard monitor using GetClipboardSequenceNumber().
/// Supports both text and image bitmap clipboard synchronization.
fn start_clipboard_sync() {
    #[cfg(windows)]
    thread::spawn(|| {
        LAST_CLIP_SEQ.store(unsafe { GetClipboardSequenceNumber() }, Ordering::SeqCst);

        loop {
            thread::sleep(Duration::from_millis(500));

            if SUPPRESS_CLIPBOARD_SYNC.load(Ordering::SeqCst) || !IS_CONNECTED.load(Ordering::Relaxed) {
                continue;
            }

            let cur_seq = unsafe { GetClipboardSequenceNumber() };
            let prev_seq = LAST_CLIP_SEQ.load(Ordering::SeqCst);
            if cur_seq == prev_seq {
                continue;
            }
            LAST_CLIP_SEQ.store(cur_seq, Ordering::SeqCst);

            // Feature 2: Image Clipboard Synchronization (Win32 CF_DIB / CF_DIBV5)
            if let Some(dib_bytes) = read_clipboard_dib_bytes() {
                if !dib_bytes.is_empty() {
                    stream_image_to_omarchy(dib_bytes);
                    continue;
                }
            }

            // Text clipboard sync
            if let Some(text) = read_clipboard_unicode_text() {
                if !text.is_empty() {
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
                    send_fast_bytes(pkt.as_bytes());
                }
            }
        }
    });
}

/// Persistent TCP connection manager with automatic reconnect on disconnect or receiver restart.
fn switch_active_peer(new_ip: String) {
    println!("[*] Switching active KVM target peer to: {}", new_ip);
    if let Ok(mut lock) = TARGET_PEER_IP.lock() {
        *lock = Some(new_ip);
    }
    if let Ok(mut lock) = STREAM_SENDER.lock() {
        *lock = None;
    }
    IS_CONNECTED.store(false, Ordering::SeqCst);
}

fn start_connection_manager() {
    thread::spawn(move || loop {
        let current_target = TARGET_PEER_IP.lock().ok().and_then(|l| l.clone());

        if let Some(target_ip) = current_target {
            if let Some(addr) = resolve_target_addr(&target_ip) {
                if let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_secs(2)) {
                    let _ = stream.set_nodelay(true);
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

                    if stream.write_all(b"H control\n").is_ok() {
                        if let Ok(reader_stream) = stream.try_clone() {
                            if let Ok(mut lock) = STREAM_SENDER.lock() {
                                *lock = Some(stream);
                            }
                            IS_CONNECTED.store(true, Ordering::SeqCst);
                            println!("[+] Successfully connected to receiver at {} (Port 9876)", target_ip);

                            let reader = BufReader::new(reader_stream);
                            for line in reader.lines() {
                                // Check if target IP has changed while connected
                                let active_now = TARGET_PEER_IP.lock().ok().and_then(|l| l.clone());
                                if active_now.as_deref() != Some(&target_ip) {
                                    break;
                                }

                                match line {
                                    Ok(line_str) => {
                                        let trimmed = line_str.trim();
                                        if trimmed.starts_with("switch_to_local") {
                                            let target_y = trimmed
                                                .split_whitespace()
                                                .nth(1)
                                                .and_then(|y| y.parse::<i32>().ok());
                                            switch_to_local(target_y);
                                        } else if trimmed == "{\"type\":\"switch_to_local\"}" {
                                            switch_to_local(None);
                                        }
                                    }
                                    Err(_) => break,
                                }
                            }
                        }
                    }
                }
            }
        }

        IS_CONNECTED.store(false, Ordering::SeqCst);
        if let Ok(mut lock) = STREAM_SENDER.lock() {
            *lock = None;
        }
        if IS_REMOTE.load(Ordering::SeqCst) {
            switch_to_local(None);
        }

        thread::sleep(Duration::from_millis(800));
    });
}

fn main() {
    let dir = get_config_direction();
    let dir_val = match dir {
        EdgeDirection::Left => 1,
        EdgeDirection::Up => 2,
        EdgeDirection::Down => 3,
        EdgeDirection::Right => 0,
    };
    EDGE_DIRECTION.store(dir_val, Ordering::SeqCst);

    let target_ip = get_config_ip();
    if let Ok(mut lock) = TARGET_PEER_IP.lock() {
        *lock = Some(target_ip.clone());
    }

    println!("=== Tailscale KVM Universal Host Controller ===");
    println!("[*] Configured Host Screen Edge Trigger: {:?} (move cursor past this edge to control remote)", dir);
    println!("[*] Target Receiver Peer: {}", target_ip);

    // Seed initial slot in web GUI
    match dir {
        EdgeDirection::Left => web_gui::set_slot_peer("left", Some(target_ip.clone())),
        EdgeDirection::Up => web_gui::set_slot_peer("up", Some(target_ip.clone())),
        EdgeDirection::Down => web_gui::set_slot_peer("down", Some(target_ip.clone())),
        EdgeDirection::Right => web_gui::set_slot_peer("right", Some(target_ip.clone())),
    }

    // Launch multi-receiver web management GUI dashboard on port 9877
    web_gui::start_gui_server(9877);

    #[cfg(windows)]
    unsafe {
        let sw = GetSystemMetrics(SM_CXSCREEN);
        let sh = GetSystemMetrics(SM_CYSCREEN);
        SCREEN_WIDTH.store(sw, Ordering::SeqCst);
        SCREEN_HEIGHT.store(sh, Ordering::SeqCst);
        CENTER_X.store(sw / 2, Ordering::SeqCst);
        CENTER_Y.store(sh / 2, Ordering::SeqCst);
    }

    #[cfg(windows)]
    init_edge_overlay();

    start_windows_server_listener();
    start_connection_manager();
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

    #[cfg(not(windows))]
    {
        // On non-Windows platforms, keep background threads and Web GUI active
        loop {
            thread::sleep(Duration::from_secs(3600));
        }
    }
}
