mod input_adapter;
#[path = "../../shared/screen_record.rs"]
mod screen_record;
use input_adapter::InputAdapter;

use std::env;
use std::path::{Component, Path, PathBuf};
#[cfg(target_os = "linux")]
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex, OnceLock};
use std::time::Instant;
use tokio::fs::{self, File};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
#[cfg(target_os = "linux")]
use tokio::net::UnixStream;
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReturnEdge {
    Left,
    Right,
    Top,
    Bottom,
}

impl ReturnEdge {
    pub fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().trim() {
            "right" => ReturnEdge::Right,
            "top" | "up" => ReturnEdge::Top,
            "bottom" | "down" => ReturnEdge::Bottom,
            _ => ReturnEdge::Left,
        }
    }

    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => ReturnEdge::Right,
            2 => ReturnEdge::Top,
            3 => ReturnEdge::Bottom,
            _ => ReturnEdge::Left,
        }
    }

    pub fn to_u8(self) -> u8 {
        match self {
            ReturnEdge::Left => 0,
            ReturnEdge::Right => 1,
            ReturnEdge::Top => 2,
            ReturnEdge::Bottom => 3,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct MonitorBounds {
    pub min_x: i32,
    pub max_x: i32,
    pub min_y: i32,
    pub max_y: i32,
}

static CURSOR_X: AtomicI32 = AtomicI32::new(960);
static CURSOR_Y: AtomicI32 = AtomicI32::new(540);
static CURRENT_BOUND_MIN_X: AtomicI32 = AtomicI32::new(0);
static CURRENT_BOUND_MAX_X: AtomicI32 = AtomicI32::new(1920);
static CURRENT_BOUND_MIN_Y: AtomicI32 = AtomicI32::new(0);
static CURRENT_BOUND_MAX_Y: AtomicI32 = AtomicI32::new(1080);
static RETURN_EDGE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0); // 0=Left, 1=Right, 2=Top, 3=Bottom
static LAST_ENTRY_MS: AtomicU64 = AtomicU64::new(0);

static ACTIVE_CONTROL_CLIENTS: AtomicUsize = AtomicUsize::new(0);
static NEXT_SESSION_ID: AtomicU64 = AtomicU64::new(0);
static ACTIVE_SESSION_ID: AtomicU64 = AtomicU64::new(0);
static LATEST_EDGE_SESSION_ID: AtomicU64 = AtomicU64::new(0);
static SWITCHED_SINGLE_MONITOR: AtomicBool = AtomicBool::new(false);

static REMOTE_MOUSE_LBUTTON: AtomicBool = AtomicBool::new(false);
static REMOTE_DRAG_START_X: AtomicI32 = AtomicI32::new(0);
static REMOTE_DRAG_START_Y: AtomicI32 = AtomicI32::new(0);
static REMOTE_HANDOFF_BUSY: AtomicBool = AtomicBool::new(false);

static ACTIVE_MONITORS: OnceLock<StdMutex<Vec<MonitorBounds>>> = OnceLock::new();
static LAST_SYNCED_CLIP_TEXT: StdMutex<String> = StdMutex::new(String::new());
static ACTIVE_CONTROL_WRITER: StdMutex<Option<tokio::sync::mpsc::UnboundedSender<Vec<u8>>>> =
    StdMutex::new(None);
static ACTIVE_HOST_PEER: StdMutex<Option<std::net::IpAddr>> = StdMutex::new(None);

static START_INSTANT: OnceLock<Instant> = OnceLock::new();
static CONTROL_ACTIVE_NOTIFY: OnceLock<tokio::sync::Notify> = OnceLock::new();

fn control_active_notify() -> &'static tokio::sync::Notify {
    CONTROL_ACTIVE_NOTIFY.get_or_init(tokio::sync::Notify::new)
}

async fn wait_until_control_active() {
    let notify = control_active_notify();
    loop {
        let notified = notify.notified();
        if ACTIVE_CONTROL_CLIENTS.load(Ordering::SeqCst) > 0 {
            return;
        }
        notified.await;
    }
}

const STREAM_CHUNK_SIZE: usize = 65_536; // 64 KB fixed streaming buffer
const MAX_SINGLE_FILE_BYTES: u64 = 64 * 1024 * 1024 * 1024; // 64 GB sanity cap
const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024; // 64 MB image cap

fn monitors_list() -> &'static StdMutex<Vec<MonitorBounds>> {
    ACTIVE_MONITORS.get_or_init(|| {
        StdMutex::new(vec![MonitorBounds {
            min_x: 0,
            max_x: 1920,
            min_y: 0,
            max_y: 1080,
        }])
    })
}

fn monotonic_ms() -> u64 {
    START_INSTANT
        .get_or_init(Instant::now)
        .elapsed()
        .as_millis() as u64
}

#[cfg(target_os = "linux")]
fn monitor_counts() -> (usize, usize) {
    let output = Command::new("hyprctl")
        .args(["monitors", "all", "-j"])
        .output();

    if let Ok(out) = output {
        if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            if let Some(arr) = json_val.as_array() {
                let mut physical = 0usize;
                let mut enabled = 0usize;
                for m in arr {
                    let name = m["name"].as_str().unwrap_or("");
                    if name == "FALLBACK" {
                        continue;
                    }
                    physical += 1;
                    if m["disabled"].as_bool() != Some(true) {
                        enabled += 1;
                    }
                }
                return (physical, enabled);
            }
        }
    }
    (1, 1)
}

fn apply_single_monitor_if_docked_async(is_loopback: bool) {
    if is_loopback {
        return;
    }
    #[cfg(target_os = "linux")]
    tokio::task::spawn_blocking(|| {
        let (physical, enabled) = monitor_counts();
        if physical > 1 {
            SWITCHED_SINGLE_MONITOR.store(true, Ordering::SeqCst);
            if enabled > 1 {
                println!(
                    "[*] Docked multi-monitor detected ({} enabled / {} connected). Switching to docked-single profile...",
                    enabled, physical
                );
                let _ = Command::new("hyprmoncfg")
                    .args(["apply", "docked-single", "--confirm-timeout", "0"])
                    .status();
                update_current_monitor_bounds();
            }
        }
    });
}

fn restore_docked_monitors_if_needed_async() {
    #[cfg(target_os = "linux")]
    tokio::task::spawn_blocking(|| {
        if SWITCHED_SINGLE_MONITOR.swap(false, Ordering::SeqCst) {
            let (physical, _) = monitor_counts();
            if physical > 1 {
                println!("[*] KVM client disconnected. Restoring full docked monitor layout...");
                let _ = Command::new("hyprmoncfg")
                    .args(["apply", "docked", "--confirm-timeout", "0"])
                    .status();
                update_current_monitor_bounds();
            }
        }
    });

    #[cfg(not(target_os = "linux"))]
    {
        let _ = SWITCHED_SINGLE_MONITOR.swap(false, Ordering::SeqCst);
    }
}

fn parse_return_edge() -> ReturnEdge {
    let args: Vec<String> = env::args().collect();
    for i in 1..args.len() {
        if args[i] == "--edge"
            || args[i] == "-e"
            || args[i] == "--return-edge"
            || args[i] == "--direction"
            || args[i] == "-d"
        {
            if i + 1 < args.len() {
                return ReturnEdge::parse(&args[i + 1]);
            }
        }
        if let Some(val) = args[i].strip_prefix("--edge=") {
            return ReturnEdge::parse(val);
        }
        if let Some(val) = args[i].strip_prefix("--direction=") {
            return ReturnEdge::parse(val);
        }
    }
    if let Ok(env_val) = env::var("KVM_RETURN_EDGE") {
        return ReturnEdge::parse(&env_val);
    }
    if let Ok(env_val) = env::var("KVM_EDGE") {
        return ReturnEdge::parse(&env_val);
    }
    if let Ok(content) = std::fs::read_to_string("config.txt") {
        for line in content.lines() {
            let t = line.trim();
            if let Some(rest) = t.strip_prefix("RETURN_EDGE=") {
                return ReturnEdge::parse(rest);
            }
            if let Some(rest) = t.strip_prefix("EDGE=") {
                return ReturnEdge::parse(rest);
            }
            if let Some(rest) = t.strip_prefix("DIRECTION=") {
                return ReturnEdge::parse(rest);
            }
        }
    }
    ReturnEdge::Left
}

#[cfg(target_os = "linux")]
fn get_linux_monitors() -> Vec<MonitorBounds> {
    let mut list = Vec::new();
    if let Ok(out) = Command::new("hyprctl").args(["monitors", "-j"]).output() {
        if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            if let Some(arr) = json_val.as_array() {
                for m in arr {
                    if m["disabled"].as_bool() == Some(true) {
                        continue;
                    }
                    let x = m["x"].as_i64().unwrap_or(0) as i32;
                    let y = m["y"].as_i64().unwrap_or(0) as i32;
                    let w_raw = m["width"].as_i64().unwrap_or(1920) as f64;
                    let h_raw = m["height"].as_i64().unwrap_or(1080) as f64;
                    let scale = m["scale"].as_f64().unwrap_or(1.0).max(0.1);
                    let w = (w_raw / scale).round() as i32;
                    let h = (h_raw / scale).round() as i32;
                    if w > 0 && h > 0 {
                        list.push(MonitorBounds {
                            min_x: x,
                            max_x: x + w,
                            min_y: y,
                            max_y: y + h,
                        });
                    }
                }
            }
        }
    }
    if !list.is_empty() {
        return list;
    }

    if let Ok(out) = Command::new("xrandr").arg("--current").output() {
        let s = String::from_utf8_lossy(&out.stdout);
        for line in s.lines() {
            if let Some(idx) = line.find("current ") {
                let rest = &line[idx + 8..];
                let parts: Vec<&str> = rest
                    .split(',')
                    .next()
                    .unwrap_or("")
                    .split_whitespace()
                    .collect();
                if parts.len() >= 3 && parts[1] == "x" {
                    if let (Ok(w), Ok(h)) = (parts[0].parse::<i32>(), parts[2].parse::<i32>()) {
                        if w > 0 && h > 0 {
                            return vec![MonitorBounds {
                                min_x: 0,
                                max_x: w,
                                min_y: 0,
                                max_y: h,
                            }];
                        }
                    }
                }
            }
        }
    }

    vec![MonitorBounds {
        min_x: 0,
        max_x: 1920,
        min_y: 0,
        max_y: 1080,
    }]
}

pub fn update_current_monitor_bounds() {
    #[cfg(target_os = "linux")]
    {
        let mons = get_linux_monitors();
        let min_x = mons.iter().map(|m| m.min_x).min().unwrap_or(0);
        let max_x = mons.iter().map(|m| m.max_x).max().unwrap_or(1920);
        let min_y = mons.iter().map(|m| m.min_y).min().unwrap_or(0);
        let max_y = mons.iter().map(|m| m.max_y).max().unwrap_or(1080);
        CURRENT_BOUND_MIN_X.store(min_x, Ordering::SeqCst);
        CURRENT_BOUND_MAX_X.store(max_x, Ordering::SeqCst);
        CURRENT_BOUND_MIN_Y.store(min_y, Ordering::SeqCst);
        CURRENT_BOUND_MAX_Y.store(max_y, Ordering::SeqCst);
        if let Ok(mut g) = monitors_list().lock() {
            *g = mons;
        }
    }
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            GetSystemMetrics, SM_CXSCREEN, SM_CXVIRTUALSCREEN, SM_CYSCREEN, SM_CYVIRTUALSCREEN,
            SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN,
        };
        unsafe {
            let sw = GetSystemMetrics(SM_CXSCREEN).max(1280);
            let sh = GetSystemMetrics(SM_CYSCREEN).max(720);
            let vw = GetSystemMetrics(SM_CXVIRTUALSCREEN);
            let vh = GetSystemMetrics(SM_CYVIRTUALSCREEN);
            let (min_x, max_x, min_y, max_y) = if vw > 0 && vh > 0 {
                let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
                let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
                (vx, vx + vw, vy, vy + vh)
            } else {
                (0, sw, 0, sh)
            };
            CURRENT_BOUND_MIN_X.store(min_x, Ordering::SeqCst);
            CURRENT_BOUND_MAX_X.store(max_x, Ordering::SeqCst);
            CURRENT_BOUND_MIN_Y.store(min_y, Ordering::SeqCst);
            CURRENT_BOUND_MAX_Y.store(max_y, Ordering::SeqCst);
            if let Ok(mut g) = monitors_list().lock() {
                *g = vec![MonitorBounds {
                    min_x: 0,
                    max_x: sw,
                    min_y: 0,
                    max_y: sh,
                }];
            }
        }
    }
    #[cfg(target_os = "macos")]
    {
        let (min_x, max_x, min_y, max_y) = input_adapter::macos_ffi::display_bounds();
        CURRENT_BOUND_MIN_X.store(min_x, Ordering::SeqCst);
        CURRENT_BOUND_MAX_X.store(max_x, Ordering::SeqCst);
        CURRENT_BOUND_MIN_Y.store(min_y, Ordering::SeqCst);
        CURRENT_BOUND_MAX_Y.store(max_y, Ordering::SeqCst);
        if let Ok(mut g) = monitors_list().lock() {
            *g = vec![MonitorBounds {
                min_x,
                max_x,
                min_y,
                max_y,
            }];
        }
    }
}

/// Checks if `(cur_x, cur_y)` is pressing against the true outer boundary of `edge`
/// across arbitrary multi-monitor arrangements!
fn is_at_outer_return_edge(
    cur_x: i32,
    cur_y: i32,
    dx: i32,
    dy: i32,
    edge: ReturnEdge,
) -> Option<(i32, i32)> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Foundation::POINT;
        use windows_sys::Win32::Graphics::Gdi::{
            GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
            MONITOR_DEFAULTTONULL,
        };
        unsafe {
            let cur_mon = MonitorFromPoint(POINT { x: cur_x, y: cur_y }, MONITOR_DEFAULTTONEAREST);
            let mut mi: MONITORINFO = std::mem::zeroed();
            mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
            if cur_mon != 0 && GetMonitorInfoW(cur_mon, &mut mi) != 0 {
                let rc = mi.rcMonitor;
                let w = (rc.right - rc.left).max(1);
                let h = (rc.bottom - rc.top).max(1);
                match edge {
                    ReturnEdge::Left => {
                        if dx < 0
                            && cur_x <= rc.left + 3
                            && MonitorFromPoint(
                                POINT {
                                    x: rc.left - 4,
                                    y: cur_y,
                                },
                                MONITOR_DEFAULTTONULL,
                            ) == 0
                        {
                            return Some(((cur_y - rc.top).max(0), h));
                        }
                    }
                    ReturnEdge::Right => {
                        if dx > 0
                            && cur_x >= rc.right - 4
                            && MonitorFromPoint(
                                POINT {
                                    x: rc.right + 4,
                                    y: cur_y,
                                },
                                MONITOR_DEFAULTTONULL,
                            ) == 0
                        {
                            return Some(((cur_y - rc.top).max(0), h));
                        }
                    }
                    ReturnEdge::Top => {
                        if dy < 0
                            && cur_y <= rc.top + 3
                            && MonitorFromPoint(
                                POINT {
                                    x: cur_x,
                                    y: rc.top - 4,
                                },
                                MONITOR_DEFAULTTONULL,
                            ) == 0
                        {
                            return Some(((cur_x - rc.left).max(0), w));
                        }
                    }
                    ReturnEdge::Bottom => {
                        if dy > 0
                            && cur_y >= rc.bottom - 4
                            && MonitorFromPoint(
                                POINT {
                                    x: cur_x,
                                    y: rc.bottom + 4,
                                },
                                MONITOR_DEFAULTTONULL,
                            ) == 0
                        {
                            return Some(((cur_x - rc.left).max(0), w));
                        }
                    }
                }
                return None;
            }
        }
    }

    let mons = monitors_list()
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();
    if mons.is_empty() {
        return None;
    }

    let contains = |px: i32, py: i32| -> bool {
        mons.iter()
            .any(|m| px >= m.min_x && px <= m.max_x && py >= m.min_y && py <= m.max_y)
    };

    // Find monitor containing (cur_x, cur_y) or closest monitor
    let cur_m = mons
        .iter()
        .copied()
        .find(|m| cur_x >= m.min_x && cur_x <= m.max_x && cur_y >= m.min_y && cur_y <= m.max_y)
        .unwrap_or(mons[0]);

    let w = (cur_m.max_x - cur_m.min_x).max(1);
    let h = (cur_m.max_y - cur_m.min_y).max(1);

    match edge {
        ReturnEdge::Left => {
            if dx < 0 && cur_x <= cur_m.min_x + 3 && !contains(cur_m.min_x - 5, cur_y) {
                return Some(((cur_y - cur_m.min_y).max(0), h));
            }
        }
        ReturnEdge::Right => {
            if dx > 0 && cur_x >= cur_m.max_x - 3 && !contains(cur_m.max_x + 5, cur_y) {
                return Some(((cur_y - cur_m.min_y).max(0), h));
            }
        }
        ReturnEdge::Top => {
            if dy < 0 && cur_y <= cur_m.min_y + 3 && !contains(cur_x, cur_m.min_y - 5) {
                return Some(((cur_x - cur_m.min_x).max(0), w));
            }
        }
        ReturnEdge::Bottom => {
            if dy > 0 && cur_y >= cur_m.max_y - 3 && !contains(cur_x, cur_m.max_y + 5) {
                return Some(((cur_x - cur_m.min_x).max(0), w));
            }
        }
    }
    None
}

/// Selects the monitor on the `return_edge` side so cursor entry always lands on valid pixels.
fn select_entry_monitor(return_edge: ReturnEdge) -> MonitorBounds {
    if let Ok(mons) = monitors_list().lock() {
        if !mons.is_empty() {
            let chosen = match return_edge {
                ReturnEdge::Left => mons.iter().copied().min_by_key(|m| m.min_x),
                ReturnEdge::Right => mons.iter().copied().max_by_key(|m| m.max_x),
                ReturnEdge::Top => mons.iter().copied().min_by_key(|m| m.min_y),
                ReturnEdge::Bottom => mons.iter().copied().max_by_key(|m| m.max_y),
            };
            if let Some(m) = chosen {
                return m;
            }
        }
    }
    MonitorBounds {
        min_x: CURRENT_BOUND_MIN_X.load(Ordering::Relaxed),
        max_x: CURRENT_BOUND_MAX_X.load(Ordering::Relaxed),
        min_y: CURRENT_BOUND_MIN_Y.load(Ordering::Relaxed),
        max_y: CURRENT_BOUND_MAX_Y.load(Ordering::Relaxed),
    }
}

fn start_bounds_refresh_worker() {
    tokio::spawn(async {
        loop {
            wait_until_control_active().await;
            let _ = tokio::task::spawn_blocking(update_current_monitor_bounds).await;
            sleep(Duration::from_secs(3)).await;
        }
    });
}

fn start_cursorpos_worker() {
    #[cfg(target_os = "linux")]
    tokio::spawn(async {
        let mut buf = [0u8; 128];
        loop {
            if ACTIVE_CONTROL_CLIENTS.load(Ordering::Relaxed) == 0 {
                wait_until_control_active().await;
            }
            if monotonic_ms().saturating_sub(LAST_ENTRY_MS.load(Ordering::Relaxed)) >= 75 {
                let Some(sock_path) = input_adapter::resolve_hyprland_socket_path() else {
                    sleep(Duration::from_millis(500)).await;
                    continue;
                };
                if let Ok(mut stream) = UnixStream::connect(&sock_path).await {
                    if stream.write_all(b"cursorpos").await.is_ok() {
                        if let Ok(n) = stream.read(&mut buf).await {
                            if n > 0 {
                                let s = String::from_utf8_lossy(&buf[..n]);
                                let mut parts = s.split(',');
                                if let (Some(x_str), Some(y_str)) = (parts.next(), parts.next()) {
                                    if let (Ok(x), Ok(y)) =
                                        (x_str.trim().parse::<i32>(), y_str.trim().parse::<i32>())
                                    {
                                        CURSOR_X.store(x, Ordering::Relaxed);
                                        CURSOR_Y.store(y, Ordering::Relaxed);
                                    }
                                }
                            }
                        }
                    }
                }
            }
            sleep(Duration::from_millis(15)).await;
        }
    });

    #[cfg(windows)]
    tokio::spawn(async {
        use windows_sys::Win32::Foundation::POINT;
        use windows_sys::Win32::UI::WindowsAndMessaging::GetCursorPos;
        loop {
            if ACTIVE_CONTROL_CLIENTS.load(Ordering::Relaxed) == 0 {
                wait_until_control_active().await;
            }
            if monotonic_ms().saturating_sub(LAST_ENTRY_MS.load(Ordering::Relaxed)) >= 75 {
                let mut pt = POINT { x: 0, y: 0 };
                unsafe {
                    if GetCursorPos(&mut pt) != 0 {
                        CURSOR_X.store(pt.x, Ordering::Relaxed);
                        CURSOR_Y.store(pt.y, Ordering::Relaxed);
                    }
                }
            }
            sleep(Duration::from_millis(15)).await;
        }
    });

    #[cfg(target_os = "macos")]
    tokio::spawn(async {
        loop {
            if ACTIVE_CONTROL_CLIENTS.load(Ordering::Relaxed) == 0 {
                wait_until_control_active().await;
            }
            if monotonic_ms().saturating_sub(LAST_ENTRY_MS.load(Ordering::Relaxed)) >= 75 {
                if let Some((x, y)) = input_adapter::macos_ffi::cursor_pos() {
                    CURSOR_X.store(x, Ordering::Relaxed);
                    CURSOR_Y.store(y, Ordering::Relaxed);
                }
            }
            sleep(Duration::from_millis(15)).await;
        }
    });
}

fn open_url_in_browser(url: String) {
    tokio::spawn(async move {
        let formatted = if !url.starts_with("http://")
            && !url.starts_with("https://")
            && !url.starts_with("about:")
            && !url.starts_with("file://")
        {
            format!("https://{}", url)
        } else {
            url
        };

        #[cfg(target_os = "linux")]
        {
            let launched = tokio::process::Command::new("uwsm-app")
                .args(["--", "firefox", &formatted])
                .spawn()
                .is_ok();

            if !launched {
                let direct = tokio::process::Command::new("firefox")
                    .arg(&formatted)
                    .spawn()
                    .is_ok();
                if !direct {
                    let _ = tokio::process::Command::new("xdg-open")
                        .arg(&formatted)
                        .spawn();
                }
            }

            sleep(Duration::from_millis(250)).await;
            let _ = tokio::process::Command::new("omarchy-hyprland-focus-app")
                .arg("firefox")
                .status()
                .await;
        }

        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            let mut cmd = std::process::Command::new("cmd");
            cmd.args(["/C", "start", "", &formatted]);
            cmd.creation_flags(CREATE_NO_WINDOW);
            let _ = cmd.spawn();
        }

        #[cfg(target_os = "macos")]
        {
            let _ = tokio::process::Command::new("open")
                .arg(&formatted)
                .spawn();
        }
    });
}

fn lock_receiver_workstation() {
    tokio::spawn(async move {
        #[cfg(target_os = "linux")]
        {
            if tokio::process::Command::new("hyprlock").spawn().is_err() {
                if tokio::process::Command::new("swaylock").spawn().is_err() {
                    let _ = tokio::process::Command::new("loginctl")
                        .arg("lock-session")
                        .spawn();
                }
            }
        }
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Shutdown::LockWorkStation;
            unsafe {
                let _ = LockWorkStation();
            }
        }
        #[cfg(target_os = "macos")]
        {
            let _ = tokio::process::Command::new("/System/Library/CoreServices/Menu Extras/User.menu/Contents/Resources/CGSession")
                .arg("-suspend")
                .spawn();
        }
    });
}

fn sanitize_relative_path(raw: &str) -> PathBuf {
    let normalized = raw.replace('\\', "/");
    let mut safe = PathBuf::new();
    for comp in Path::new(&normalized).components() {
        if let Component::Normal(seg) = comp {
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

fn downloads_dir() -> PathBuf {
    let home = env::var("HOME")
        .or_else(|_| env::var("USERPROFILE"))
        .unwrap_or_else(|_| "/tmp".to_string());
    PathBuf::from(home).join("Downloads")
}

async fn receive_binary_file<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
    file_size: u64,
    raw_rel_path: &str,
) -> Result<(String, bool), std::io::Error> {
    if file_size > MAX_SINGLE_FILE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file exceeds maximum size",
        ));
    }

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
        downloads_dir()
    };
    let target_path = base_dir.join(&safe_rel);

    if let Some(parent) = target_path.parent() {
        fs::create_dir_all(parent).await?;
    }

    let mut out_file = File::create(&target_path).await?;
    let mut chunk = [0u8; STREAM_CHUNK_SIZE];
    let mut remaining = file_size;

    while remaining > 0 {
        let to_read = STREAM_CHUNK_SIZE.min(remaining as usize);
        let n = reader.read(&mut chunk[..to_read]).await?;
        if n == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "stream closed mid-file",
            ));
        }
        out_file.write_all(&chunk[..n]).await?;
        remaining -= n as u64;
    }

    out_file.flush().await?;
    Ok((safe_rel.to_string_lossy().into_owned(), is_rec_internal))
}

async fn stream_recording_file_to_host(
    host_ip: std::net::IpAddr,
    local_path: &Path,
    session_id: &str,
    remote_filename: &str,
) -> Result<(), String> {
    let addr = std::net::SocketAddr::new(host_ip, 9875);
    let mut tx = TcpStream::connect(addr)
        .await
        .map_err(|e| format!("Failed connecting to host {} to upload recording: {}", addr, e))?;
    let _ = tx.set_nodelay(true);

    let mut file = File::open(local_path)
        .await
        .map_err(|e| format!("Failed opening {:?}: {}", local_path, e))?;
    let meta = file
        .metadata()
        .await
        .map_err(|e| format!("Failed reading metadata {:?}: {}", local_path, e))?;
    let total_size = meta.len();

    let clean_sess = screen_record::sanitize_token(session_id, "demo_session");
    let clean_name = screen_record::sanitize_filename(remote_filename, "recording.mp4");
    let header = format!("B {} __kvm_rec__/{}/{}\n", total_size, clean_sess, clean_name);
    tx.write_all(header.as_bytes())
        .await
        .map_err(|e| format!("Failed writing recording header: {}", e))?;

    let mut chunk_buf = vec![0u8; STREAM_CHUNK_SIZE];
    let mut remaining = total_size;
    while remaining > 0 {
        let to_read = STREAM_CHUNK_SIZE.min(remaining as usize);
        match file.read(&mut chunk_buf[..to_read]).await {
            Ok(0) => break,
            Ok(n) => {
                tx.write_all(&chunk_buf[..n])
                    .await
                    .map_err(|e| format!("Failed streaming recording chunk: {}", e))?;
                remaining -= n as u64;
            }
            Err(e) => return Err(format!("Failed reading recording chunk: {}", e)),
        }
    }
    let _ = tx.flush().await;
    let _ = tx.shutdown().await;
    Ok(())
}

fn send_receiver_rec_caps(msg_tx: &tokio::sync::mpsc::UnboundedSender<Vec<u8>>) {
    let backend = screen_record::detect_capture_backend();
    let has_ffmpeg = if screen_record::has_ffmpeg() { "1" } else { "0" };
    let os = std::env::consts::OS;
    let pkt = format!("REC_CAPS {} {} {}\n", backend, has_ffmpeg, os);
    let _ = msg_tx.send(pkt.into_bytes());
}

fn handle_receiver_record_command(
    trimmed: &str,
    msg_tx: tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
    peer_ip: Option<std::net::IpAddr>,
) {
    if trimmed == "REC_PING" {
        send_receiver_rec_caps(&msg_tx);
        return;
    }

    if let Some(raw_sess) = trimmed.strip_prefix("REC_START ") {
        let session_id = screen_record::sanitize_token(raw_sess.trim(), "demo_session");
        println!("[*] Synchronized demo recording START: session={}", session_id);
        tokio::spawn(async move {
            let sid = session_id.clone();
            let res =
                tokio::task::spawn_blocking(move || screen_record::start_capture(&sid, "receiver"))
                    .await;
            match res {
                Ok(Ok(info)) => {
                    let pkt = format!("REC_STARTED {} {}\n", info.session_id, info.backend);
                    let _ = msg_tx.send(pkt.into_bytes());
                }
                Ok(Err(err)) => {
                    let clean_err = err.replace(['\r', '\n'], " ");
                    eprintln!("[!] Receiver screen capture error: {}", clean_err);
                    let pkt = format!("REC_ERR {} {}\n", session_id, clean_err);
                    let _ = msg_tx.send(pkt.into_bytes());
                }
                Err(e) => {
                    let pkt = format!("REC_ERR {} join_error:{}\n", session_id, e);
                    let _ = msg_tx.send(pkt.into_bytes());
                }
            }
        });
        return;
    }

    if let Some(raw_sess) = trimmed.strip_prefix("REC_STOP ") {
        let session_id = screen_record::sanitize_token(raw_sess.trim(), "demo_session");
        println!("[*] Synchronized demo recording STOP: session={}", session_id);
        tokio::spawn(async move {
            let sid = session_id.clone();
            let stop_res =
                tokio::task::spawn_blocking(move || screen_record::stop_capture(&sid)).await;
            match stop_res {
                Ok(Ok(file_path)) => {
                    let fname = file_path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("receiver.mp4")
                        .to_string();
                    let target_ip = peer_ip
                        .or_else(|| ACTIVE_HOST_PEER.lock().ok().and_then(|l| *l));
                    if let Some(host_ip) = target_ip {
                        match stream_recording_file_to_host(host_ip, &file_path, &session_id, &fname)
                            .await
                        {
                            Ok(()) => {
                                println!(
                                    "[+] Uploaded Receiver recording {} to Host {}",
                                    fname, host_ip
                                );
                                let pkt = format!("REC_SENT {} {}\n", session_id, fname);
                                let _ = msg_tx.send(pkt.into_bytes());
                            }
                            Err(err) => {
                                let clean = err.replace(['\r', '\n'], " ");
                                eprintln!("[!] Failed uploading recording: {}", clean);
                                let pkt = format!("REC_ERR {} {}\n", session_id, clean);
                                let _ = msg_tx.send(pkt.into_bytes());
                            }
                        }
                    } else {
                        let pkt = format!(
                            "REC_ERR {} Unknown Host IP for recording upload\n",
                            session_id
                        );
                        let _ = msg_tx.send(pkt.into_bytes());
                    }
                }
                Ok(Err(err)) => {
                    let clean_err = err.replace(['\r', '\n'], " ");
                    eprintln!("[!] Receiver stop_capture failed: {}", clean_err);
                    let pkt = format!("REC_ERR {} {}\n", session_id, clean_err);
                    let _ = msg_tx.send(pkt.into_bytes());
                }
                Err(e) => {
                    let pkt = format!("REC_ERR {} join_error:{}\n", session_id, e);
                    let _ = msg_tx.send(pkt.into_bytes());
                }
            }
        });
        return;
    }

    if let Some(rest) = trimmed.strip_prefix("REC_STITCH ") {
        let mut parts = rest.split_whitespace();
        let session_id =
            screen_record::sanitize_token(parts.next().unwrap_or(""), "demo_session");
        let slot_name = screen_record::sanitize_token(parts.next().unwrap_or("right"), "right");
        let host_fname =
            screen_record::sanitize_filename(parts.next().unwrap_or(""), "host_windows.avi");
        let recv_fname =
            screen_record::sanitize_filename(parts.next().unwrap_or(""), "receiver_linux.mp4");
        let auto_export = parts.next().unwrap_or("0") == "1";
        let host_label = parts.next().unwrap_or("HOST").replace('_', " ");

        tokio::spawn(async move {
            let sid = session_id.clone();
            let slot_copy = slot_name.clone();
            let h_name = host_fname.clone();
            let r_name = recv_fname.clone();
            let h_lbl = host_label.clone();
            let r_lbl = format!("RECEIVER ({})", std::env::consts::OS.to_uppercase());

            let res = tokio::task::spawn_blocking(move || {
                let sdir = screen_record::recordings_base_dir().join(&sid);
                let host_path = sdir.join(&h_name);
                let recv_path = sdir.join(&r_name);
                let out = screen_record::stitch_dual_demo(
                    &sdir,
                    &host_path,
                    &recv_path,
                    &slot_copy,
                    &h_lbl,
                    &r_lbl,
                )?;
                if auto_export {
                    let _ = screen_record::export_to_demo_pics(
                        &out.webp_path,
                        "02-dual-screen-demo.webp",
                    );
                }
                Ok::<_, String>(out)
            })
            .await;

            match res {
                Ok(Ok(outputs)) => {
                    let target_ip = peer_ip
                        .or_else(|| ACTIVE_HOST_PEER.lock().ok().and_then(|l| *l));
                    if let Some(host_ip) = target_ip {
                        for p in [&outputs.webp_path, &outputs.gif_path, &outputs.mp4_path] {
                            if !p.is_file() {
                                continue;
                            }
                            if let Some(name) = p.file_name().and_then(|n| n.to_str()) {
                                let _ = stream_recording_file_to_host(
                                    host_ip,
                                    p,
                                    &session_id,
                                    name,
                                )
                                .await;
                            }
                        }
                    }
                    let pkt = format!("REC_STITCH_DONE {}\n", session_id);
                    let _ = msg_tx.send(pkt.into_bytes());
                }
                Ok(Err(err)) => {
                    let clean = err.replace(['\r', '\n'], " ");
                    let pkt = format!("REC_ERR {} {}\n", session_id, clean);
                    let _ = msg_tx.send(pkt.into_bytes());
                }
                Err(e) => {
                    let pkt = format!("REC_ERR {} stitch_join:{}\n", session_id, e);
                    let _ = msg_tx.send(pkt.into_bytes());
                }
            }
        });
    }
}

async fn receive_image_to_clipboard<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
    image_size: u64,
) -> Result<(), std::io::Error> {
    if image_size > MAX_IMAGE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "image exceeds maximum size",
        ));
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(mut child) = tokio::process::Command::new("wl-copy")
            .args(["-t", "image/png"])
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let mut chunk = [0u8; STREAM_CHUNK_SIZE];
                let mut remaining = image_size;
                while remaining > 0 {
                    let to_read = STREAM_CHUNK_SIZE.min(remaining as usize);
                    let n = reader.read(&mut chunk[..to_read]).await?;
                    if n == 0 {
                        break;
                    }
                    let _ = stdin.write_all(&chunk[..n]).await;
                    remaining -= n as u64;
                }
                let _ = stdin.flush().await;
            }
            let _ = child.wait().await;
            return Ok(());
        }
    }

    let mut chunk = [0u8; STREAM_CHUNK_SIZE];
    let mut remaining = image_size;
    while remaining > 0 {
        let to_read = STREAM_CHUNK_SIZE.min(remaining as usize);
        let n = reader.read(&mut chunk[..to_read]).await?;
        if n == 0 {
            break;
        }
        remaining -= n as u64;
    }
    Ok(())
}

fn notify_files_received(files: &[String]) {
    if files.is_empty() {
        return;
    }
    let body = if files.len() == 1 {
        format!("Saved {} to Downloads", files[0])
    } else {
        format!(
            "Saved {} files (e.g. {}) to Downloads",
            files.len(),
            files[0]
        )
    };
    println!("[+] {}", body);
    #[cfg(target_os = "linux")]
    {
        let _ = Command::new("notify-send")
            .args(["-a", "Tailscale KVM", "File Received", &body])
            .spawn();
    }
}

#[cfg(windows)]
fn write_windows_clipboard_text(text: &str) {
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, OpenClipboard, SetClipboardData,
    };
    use windows_sys::Win32::System::Memory::{
        GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE,
    };
    const CF_UNICODETEXT_FMT: u32 = 13;

    let mut wide: Vec<u16> = text.encode_utf16().collect();
    wide.push(0);
    let byte_len = wide.len() * 2;

    unsafe {
        for _ in 0..4 {
            if OpenClipboard(0) != 0 {
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
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}

#[cfg(windows)]
fn read_windows_clipboard_text() -> Option<String> {
    use windows_sys::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows_sys::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
    const CF_UNICODETEXT_FMT: u32 = 13;

    unsafe {
        if IsClipboardFormatAvailable(CF_UNICODETEXT_FMT) == 0 {
            return None;
        }
        if OpenClipboard(0) == 0 {
            return None;
        }
        let mut out = None;
        let h_data = GetClipboardData(CF_UNICODETEXT_FMT);
        if h_data != 0 {
            let h_global = h_data as *mut std::ffi::c_void;
            let byte_size = GlobalSize(h_global);
            if byte_size > 0 && byte_size <= 1_048_576 {
                let ptr = GlobalLock(h_global) as *const u16;
                if !ptr.is_null() {
                    let max_chars = byte_size / 2;
                    let mut len = 0usize;
                    while len < max_chars && *ptr.add(len) != 0 {
                        len += 1;
                    }
                    let slice = std::slice::from_raw_parts(ptr, len);
                    out = Some(String::from_utf16_lossy(slice));
                    let _ = GlobalUnlock(h_global);
                }
            }
        }
        let _ = CloseClipboard();
        out
    }
}

fn copy_text_to_system_clipboard(text: String) {
    {
        if let Ok(mut last) = LAST_SYNCED_CLIP_TEXT.lock() {
            *last = text.clone();
        }
    }

    #[cfg(target_os = "linux")]
    tokio::spawn(async move {
        if let Ok(mut child) = tokio::process::Command::new("wl-copy")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes()).await;
            }
            let _ = child.wait().await;
            return;
        }
        if let Ok(mut child) = tokio::process::Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes()).await;
            }
            let _ = child.wait().await;
        }
    });

    #[cfg(windows)]
    {
        write_windows_clipboard_text(&text);
    }

    #[cfg(target_os = "macos")]
    tokio::spawn(async move {
        if let Ok(mut child) = tokio::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(text.as_bytes()).await;
            }
            let _ = child.wait().await;
        }
    });
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

fn escape_text_for_transmission(text: &str) -> String {
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
    pkt
}

fn decode_base64(s: &str) -> Result<Vec<u8>, ()> {
    if s.len() > 16 * 1024 * 1024 {
        return Err(());
    }
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0;

    for &b in s.as_bytes() {
        if b == b'=' {
            break;
        }
        let val = match b {
            b'A'..=b'Z' => b - b'A',
            b'a'..=b'z' => b - b'a' + 26,
            b'0'..=b'9' => b - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        };
        buf = (buf << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}

fn mark_control_session(
    is_control: &mut bool,
    session_id: u64,
    msg_tx: &tokio::sync::mpsc::UnboundedSender<Vec<u8>>,
    peer_ip: Option<std::net::IpAddr>,
) {
    if !*is_control {
        *is_control = true;
        let prev = ACTIVE_SESSION_ID.fetch_max(session_id, Ordering::SeqCst);
        if session_id >= prev {
            if let Ok(mut lock) = ACTIVE_CONTROL_WRITER.lock() {
                *lock = Some(msg_tx.clone());
            }
            if let Some(ip) = peer_ip {
                if let Ok(mut lock) = ACTIVE_HOST_PEER.lock() {
                    *lock = Some(ip);
                }
            }
        }
        let is_loopback = peer_ip.map(|ip| ip.is_loopback()).unwrap_or(false);
        if ACTIVE_CONTROL_CLIENTS.fetch_add(1, Ordering::SeqCst) == 0 {
            control_active_notify().notify_waiters();
            apply_single_monitor_if_docked_async(is_loopback);
        }
    }
}

#[cfg(target_os = "linux")]
#[derive(Clone, Debug)]
enum ActiveApp {
    Browser,
    FileManager,
    Terminal(String),
    Other,
}

#[cfg(target_os = "linux")]
fn get_active_hyprland_app() -> ActiveApp {
    let output = Command::new("hyprctl").args(["activewindow", "-j"]).output();
    if let Ok(out) = output {
        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            let class = v["class"].as_str().unwrap_or("").to_lowercase();
            let initial_class = v["initialClass"].as_str().unwrap_or("").to_lowercase();
            let title = v["title"].as_str().unwrap_or("").to_lowercase();

            if class.contains("firefox")
                || class.contains("zen")
                || class.contains("chromium")
                || class.contains("chrome")
                || class.contains("brave")
                || initial_class.contains("firefox")
            {
                return ActiveApp::Browser;
            }
            if class.contains("nautilus")
                || class.contains("thunar")
                || class.contains("dolphin")
                || class.contains("nemo")
                || title.contains("files")
            {
                return ActiveApp::FileManager;
            }

            if class.contains("foot")
                || class.contains("kitty")
                || class.contains("alacritty")
                || class.contains("ghostty")
                || class.contains("terminal")
                || initial_class.contains("org.omarchy.agent")
            {
                if let Some(pid) = v["pid"].as_i64() {
                    if let Ok(link) = std::fs::read_link(format!("/proc/{}/cwd", pid)) {
                        return ActiveApp::Terminal(link.to_string_lossy().into_owned());
                    }
                }
                return ActiveApp::Terminal(String::new());
            }
        }
    }
    ActiveApp::Other
}

#[cfg(target_os = "linux")]
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
        if let Ok(entries) = std::fs::read_dir(current) {
            for entry in entries.flatten() {
                collect_transfer_entries(base, &entry.path(), depth + 1, out);
            }
        }
    }
}

#[cfg(target_os = "linux")]
async fn stream_files_to_host(host_ip: std::net::IpAddr, paths: Vec<PathBuf>) {
    let mut entries = Vec::new();
    for p in &paths {
        collect_transfer_entries(p, p, 0, &mut entries);
    }
    if entries.is_empty() {
        return;
    }

    let addr = std::net::SocketAddr::new(host_ip, 9875);
    let Ok(mut tx) = TcpStream::connect(addr).await else {
        eprintln!("[!] Failed to connect to host file transfer port: {}", addr);
        return;
    };
    let _ = tx.set_nodelay(true);

    let mut chunk_buf = [0u8; STREAM_CHUNK_SIZE];
    for (full_path, rel_path) in entries {
        let Ok(mut file) = File::open(&full_path).await else {
            continue;
        };
        let Ok(meta) = file.metadata().await else {
            continue;
        };
        let total_size = meta.len();

        let header = format!("B {} {}\n", total_size, rel_path);
        if tx.write_all(header.as_bytes()).await.is_err() {
            break;
        }

        let mut remaining = total_size;
        let mut failed = false;
        while remaining > 0 {
            let to_read = STREAM_CHUNK_SIZE.min(remaining as usize);
            match file.read(&mut chunk_buf[..to_read]).await {
                Ok(0) => break,
                Ok(n) => {
                    if tx.write_all(&chunk_buf[..n]).await.is_err() {
                        failed = true;
                        break;
                    }
                    remaining -= n as u64;
                }
                Err(_) => {
                    failed = true;
                    break;
                }
            }
        }
        if failed {
            break;
        }
    }
    let _ = tx.flush().await;
}

#[cfg(target_os = "linux")]
fn parse_file_uris(raw: &str) -> Vec<PathBuf> {
    let mut paths = Vec::new();
    for line in raw.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix("file://") {
            let decoded = percent_decode_str(rest);
            let p = PathBuf::from(decoded);
            if p.exists() {
                paths.push(p);
            }
        } else if t.starts_with('/') {
            let p = PathBuf::from(t);
            if p.exists() {
                paths.push(p);
            }
        }
    }
    paths
}

#[cfg(target_os = "linux")]
fn percent_decode_str(s: &str) -> String {
    let mut out = Vec::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let h1 = bytes[i + 1] as char;
            let h2 = bytes[i + 2] as char;
            if let Ok(byte) = u8::from_str_radix(&format!("{}{}", h1, h2), 16) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(target_os = "linux")]
async fn get_local_tailscale_ip() -> Option<String> {
    if let Ok(out) = tokio::process::Command::new("tailscale")
        .args(["ip", "-4"])
        .output()
        .await
    {
        let ip = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if !ip.is_empty() && !ip.contains(':') && ip.chars().all(|c| c.is_ascii_digit() || c == '.') {
            return Some(ip);
        }
    }
    None
}

#[cfg(target_os = "linux")]
fn rewrite_localhost_to_tailscale(url: &str, tailscale_ip: Option<&str>) -> String {
    let Some(ts_ip) = tailscale_ip else {
        return url.to_string();
    };
    if ts_ip.is_empty() {
        return url.to_string();
    }

    let mut u = url.trim().to_string();
    if !u.starts_with("http://")
        && !u.starts_with("https://")
        && !u.starts_with("about:")
        && !u.starts_with("file://")
    {
        if u.starts_with("localhost") || u.starts_with("127.0.0.1") {
            u = format!("http://{}", u);
        } else {
            u = format!("https://{}", u);
        }
    }

    let patterns = [
        ("://localhost:", format!("://{}:", ts_ip)),
        ("://localhost/", format!("://{}/", ts_ip)),
        ("://localhost?", format!("://{}?", ts_ip)),
        ("://localhost#", format!("://{}#", ts_ip)),
        ("://127.0.0.1:", format!("://{}:", ts_ip)),
        ("://127.0.0.1/", format!("://{}/", ts_ip)),
        ("://127.0.0.1?", format!("://{}?", ts_ip)),
        ("://127.0.0.1#", format!("://{}#", ts_ip)),
    ];

    for (pat, repl) in &patterns {
        if u.contains(pat) {
            return u.replace(pat, repl);
        }
    }

    if u.ends_with("://localhost") {
        return u.replace("://localhost", &format!("://{}", ts_ip));
    }
    if u.ends_with("://127.0.0.1") {
        return u.replace("://127.0.0.1", &format!("://{}", ts_ip));
    }

    u
}

#[cfg(target_os = "linux")]
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
        || t.starts_with("localhost")
        || t.starts_with("127.0.0.1")
}

async fn execute_reverse_handoff() {
    #[cfg(target_os = "linux")]
    {
        if REMOTE_HANDOFF_BUSY
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        tokio::spawn(async move {
            let host_ip = ACTIVE_HOST_PEER.lock().ok().and_then(|l| *l);

            if let Some(target_ip) = host_ip {
                let app = get_active_hyprland_app();

                let _ = tokio::process::Command::new("wtype")
                    .args(["-k", "Escape"])
                    .status()
                    .await;
                sleep(Duration::from_millis(40)).await;

                match app {
                    ActiveApp::Browser => {
                        let _ = tokio::process::Command::new("wtype")
                            .args(["-M", "ctrl", "-k", "l", "-m", "ctrl"])
                            .status()
                            .await;
                        sleep(Duration::from_millis(50)).await;

                        let _ = tokio::process::Command::new("wtype")
                            .args(["-M", "ctrl", "-k", "c", "-m", "ctrl"])
                            .status()
                            .await;
                        sleep(Duration::from_millis(80)).await;

                        if let Ok(out) = tokio::process::Command::new("wl-paste")
                            .args(["--no-newline"])
                            .output()
                            .await
                        {
                            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
                            let raw_url = if looks_like_url(&text) {
                                text
                            } else {
                                "about:newtab".to_string()
                            };

                            let local_ts_ip = get_local_tailscale_ip().await;
                            let final_url = rewrite_localhost_to_tailscale(&raw_url, local_ts_ip.as_deref());

                            let pkt = format!("U {}\n", final_url);
                            let sent_via_control = ACTIVE_CONTROL_WRITER
                                .lock()
                                .ok()
                                .and_then(|l| l.clone())
                                .map(|tx| tx.send(pkt.clone().into_bytes()).is_ok())
                                .unwrap_or(false);

                            if !sent_via_control {
                                let addr = std::net::SocketAddr::new(target_ip, 9876);
                                if let Ok(mut tx) = TcpStream::connect(addr).await {
                                    let _ = tx.set_nodelay(true);
                                    let _ = tx.write_all(pkt.as_bytes()).await;
                                    let _ = tx.flush().await;
                                }
                            }
                        }

                        let _ = tokio::process::Command::new("wtype")
                            .args(["-k", "Escape"])
                            .status()
                            .await;
                        sleep(Duration::from_millis(30)).await;

                        let _ = tokio::process::Command::new("wtype")
                            .args(["-M", "ctrl", "-k", "w", "-m", "ctrl"])
                            .status()
                            .await;
                    }

                    ActiveApp::FileManager => {
                        let _ = tokio::process::Command::new("wtype")
                            .args(["-M", "ctrl", "-k", "c", "-m", "ctrl"])
                            .status()
                            .await;
                        sleep(Duration::from_millis(80)).await;

                        let mut file_paths = Vec::new();
                        if let Ok(out) = tokio::process::Command::new("wl-paste")
                            .args(["-t", "text/uri-list"])
                            .output()
                            .await
                        {
                            let s = String::from_utf8_lossy(&out.stdout);
                            file_paths = parse_file_uris(&s);
                        }
                        if file_paths.is_empty() {
                            if let Ok(out) = tokio::process::Command::new("wl-paste")
                                .args(["--no-newline"])
                                .output()
                                .await
                            {
                                let s = String::from_utf8_lossy(&out.stdout);
                                file_paths = parse_file_uris(&s);
                            }
                        }

                        if !file_paths.is_empty() {
                            stream_files_to_host(target_ip, file_paths).await;
                        }
                    }

                    ActiveApp::Terminal(cwd) => {
                        if !cwd.is_empty() {
                            let pkt = format!("D {}\n", cwd);
                            let sent_via_control = ACTIVE_CONTROL_WRITER
                                .lock()
                                .ok()
                                .and_then(|l| l.clone())
                                .map(|tx| tx.send(pkt.clone().into_bytes()).is_ok())
                                .unwrap_or(false);
                            if !sent_via_control {
                                let addr = std::net::SocketAddr::new(target_ip, 9876);
                                if let Ok(mut tx) = TcpStream::connect(addr).await {
                                    let _ = tx.set_nodelay(true);
                                    let _ = tx.write_all(pkt.as_bytes()).await;
                                    let _ = tx.flush().await;
                                }
                            }
                        }
                    }

                    ActiveApp::Other => {
                        if let Ok(out) = tokio::process::Command::new("wl-paste")
                            .args(["-t", "text/uri-list"])
                            .output()
                            .await
                        {
                            let s = String::from_utf8_lossy(&out.stdout);
                            let file_paths = parse_file_uris(&s);
                            if !file_paths.is_empty() {
                                stream_files_to_host(target_ip, file_paths).await;
                            }
                        }
                    }
                }
            }

            REMOTE_HANDOFF_BUSY.store(false, Ordering::SeqCst);
        });
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = REMOTE_HANDOFF_BUSY.load(Ordering::Relaxed);
    }
}

/// Outgoing cross-platform clipboard monitor for Receiver -> Host sync
fn start_clipboard_monitor() {
    tokio::spawn(async {
        loop {
            if ACTIVE_CONTROL_CLIENTS.load(Ordering::Relaxed) == 0 {
                wait_until_control_active().await;
            }
            sleep(Duration::from_millis(650)).await;

            if ACTIVE_CONTROL_CLIENTS.load(Ordering::Relaxed) == 0 {
                continue;
            }

            #[cfg(target_os = "linux")]
            {
                let host_ip = ACTIVE_HOST_PEER.lock().ok().and_then(|l| *l);
                if let Some(target_ip) = host_ip {
                    if let Ok(types_out) = tokio::process::Command::new("wl-paste")
                        .args(["-l"])
                        .output()
                        .await
                    {
                        let types_str = String::from_utf8_lossy(&types_out.stdout);
                        if types_str.lines().any(|l| l.trim() == "image/png") {
                            if let Ok(out) = tokio::process::Command::new("wl-paste")
                                .args(["-t", "image/png"])
                                .output()
                                .await
                            {
                                let png_bytes = out.stdout;
                                if !png_bytes.is_empty()
                                    && png_bytes.len() < MAX_IMAGE_BYTES as usize
                                {
                                    let img_hash = format!("img:{}", png_bytes.len());
                                    let changed = {
                                        let mut last = LAST_SYNCED_CLIP_TEXT.lock().unwrap();
                                        if *last != img_hash {
                                            *last = img_hash;
                                            true
                                        } else {
                                            false
                                        }
                                    };
                                    if changed {
                                        let addr = std::net::SocketAddr::new(target_ip, 9876);
                                        if let Ok(mut tx) = TcpStream::connect(addr).await {
                                            let _ = tx.set_nodelay(true);
                                            let header = format!("I {}\n", png_bytes.len());
                                            if tx.write_all(header.as_bytes()).await.is_ok() {
                                                let _ = tx.write_all(&png_bytes).await;
                                                let _ = tx.flush().await;
                                            }
                                        }
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                }
            }

            let maybe_text: Option<String> = {
                #[cfg(target_os = "linux")]
                {
                    if let Ok(out) = tokio::process::Command::new("wl-paste")
                        .args(["--no-newline"])
                        .output()
                        .await
                    {
                        if out.status.success() {
                            Some(String::from_utf8_lossy(&out.stdout).to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                #[cfg(windows)]
                {
                    read_windows_clipboard_text()
                }
                #[cfg(target_os = "macos")]
                {
                    if let Ok(out) = tokio::process::Command::new("pbpaste").output().await {
                        if out.status.success() {
                            Some(String::from_utf8_lossy(&out.stdout).to_string())
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                }
                #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
                {
                    None
                }
            };

            if let Some(text) = maybe_text {
                if !text.is_empty() && text.len() <= 1_048_576 {
                    let changed = {
                        let mut last = LAST_SYNCED_CLIP_TEXT.lock().unwrap();
                        if *last != text {
                            *last = text.clone();
                            true
                        } else {
                            false
                        }
                    };
                    if changed {
                        let pkt = escape_text_for_transmission(&text);
                        let maybe_tx = ACTIVE_CONTROL_WRITER.lock().ok().and_then(|l| l.clone());
                        if let Some(tx) = maybe_tx {
                            let _ = tx.send(pkt.into_bytes());
                        }
                    }
                }
            }
        }
    });
}

fn open_terminal_at_path(path: String) {
    tokio::spawn(async move {
        let p = path.trim();
        let home = env::var("HOME")
            .or_else(|_| env::var("USERPROFILE"))
            .unwrap_or_else(|_| ".".to_string());
        let target_dir = if !p.is_empty() && p != "." && Path::new(p).exists() {
            p.to_string()
        } else {
            home
        };

        #[cfg(target_os = "linux")]
        {
            if tokio::process::Command::new("foot")
                .args(["-D", &target_dir])
                .spawn()
                .is_err()
            {
                let _ = tokio::process::Command::new("kitty")
                    .args(["--directory", &target_dir])
                    .spawn();
            }
        }

        #[cfg(windows)]
        {
            let _ = std::process::Command::new("wt.exe")
                .args(["-d", &target_dir])
                .spawn()
                .or_else(|_| {
                    std::process::Command::new("powershell.exe")
                        .args(["-NoExit", "-Command", &format!("Set-Location '{}'", target_dir)])
                        .spawn()
                });
        }

        #[cfg(target_os = "macos")]
        {
            let _ = tokio::process::Command::new("open")
                .args(["-a", "Terminal", &target_dir])
                .spawn();
        }
    });
}

/// Computes exact entry pixel coordinates on the Receiver screen so the cursor enters
/// directly from the physical edge adjacent to the Host!
pub fn compute_entry_position(
    is_horizontal_axis: bool,
    raw_coord: i32,
    host_span: Option<i32>,
    bounds: MonitorBounds,
    return_edge: ReturnEdge,
) -> (i32, i32) {
    let recv_w = (bounds.max_x - bounds.min_x).max(1);
    let recv_h = (bounds.max_y - bounds.min_y).max(1);

    if is_horizontal_axis {
        let scaled_x = match host_span {
            Some(span) if span > 0 => {
                bounds.min_x + ((raw_coord as i64 * recv_w as i64) / span as i64) as i32
            }
            _ => raw_coord,
        };
        let target_x = scaled_x.clamp(
            bounds.min_x + 16,
            (bounds.max_x - 16).max(bounds.min_x + 16),
        );
        let entry_y = match return_edge {
            ReturnEdge::Bottom => (bounds.max_y - 18).max(bounds.min_y + 4),
            _ => (bounds.min_y + 18).min(bounds.max_y - 4),
        };
        (target_x, entry_y)
    } else {
        let scaled_y = match host_span {
            Some(span) if span > 0 => {
                bounds.min_y + ((raw_coord as i64 * recv_h as i64) / span as i64) as i32
            }
            _ => raw_coord,
        };
        let target_y = scaled_y.clamp(
            bounds.min_y + 16,
            (bounds.max_y - 16).max(bounds.min_y + 16),
        );
        let entry_x = match return_edge {
            ReturnEdge::Right => (bounds.max_x - 18).max(bounds.min_x + 4),
            _ => (bounds.min_x + 18).min(bounds.max_x - 4),
        };
        (entry_x, target_y)
    }
}

async fn handle_connection(stream: TcpStream, device: Arc<Mutex<InputAdapter>>, session_id: u64) {
    let peer = stream.peer_addr().ok();
    let peer_ip = peer.map(|p| p.ip());

    let (reader_half, mut raw_writer) = stream.into_split();
    let mut reader = BufReader::with_capacity(STREAM_CHUNK_SIZE, reader_half);
    let mut line = String::with_capacity(256);
    let mut is_control = false;
    let mut received_files: Vec<String> = Vec::new();

    let (msg_tx, mut msg_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

    tokio::spawn(async move {
        while let Some(msg) = msg_rx.recv().await {
            if raw_writer.write_all(&msg).await.is_err() {
                break;
            }
            let _ = raw_writer.flush().await;
        }
    });

    loop {
        line.clear();
        let bytes_read = match reader.read_line(&mut line).await {
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
            // "H control" -> Primary KVM control channel handshake
            b'H' => {
                println!(
                    "[+] KVM control session #{} established from: {:?}",
                    session_id, peer
                );
                tokio::task::spawn_blocking(update_current_monitor_bounds);
                mark_control_session(&mut is_control, session_id, &msg_tx, peer_ip);
                send_receiver_rec_caps(&msg_tx);
            }

            // "REC_*" -> Synchronized dual-computer screen recording commands
            b'R' => {
                if trimmed.starts_with("REC_") {
                    handle_receiver_record_command(trimmed, msg_tx.clone(), peer_ip);
                }
            }

            // "E <edge>" -> Dynamic return edge configuration from Host GUI
            // Guarded by monotonic session_id so an older closing socket can NEVER overwrite a newer socket!
            b'E' => {
                if trimmed.len() > 2 {
                    let prev_edge_sess =
                        LATEST_EDGE_SESSION_ID.fetch_max(session_id, Ordering::SeqCst);
                    if session_id >= prev_edge_sess {
                        let edge_str = trimmed[2..].trim();
                        let new_edge = ReturnEdge::parse(edge_str);
                        RETURN_EDGE.store(new_edge.to_u8(), Ordering::SeqCst);
                        println!(
                            "[*] Session #{} synced return edge to: {:?} (exit {:?} edge to return to host)",
                            session_id, new_edge, new_edge
                        );
                    }
                }
            }

            // "B <byte_len> <relative_path>\n<raw_bytes>" -> Zero-RAM-spike binary file stream
            b'B' => {
                if trimmed.len() > 2 {
                    let rest = &trimmed[2..];
                    if let Some(space_idx) = rest.find(' ') {
                        let size_str = &rest[..space_idx];
                        let rel_path = rest[space_idx + 1..].trim();
                        if let Ok(file_size) = size_str.parse::<u64>() {
                            match receive_binary_file(&mut reader, file_size, rel_path).await {
                                Ok((saved_name, is_rec_internal)) => {
                                    if is_rec_internal {
                                        println!(
                                            "[+] Saved synchronized recording asset: {} ({} bytes)",
                                            saved_name, file_size
                                        );
                                    } else {
                                        println!(
                                            "[+] Saved streamed file: {} ({} bytes)",
                                            saved_name, file_size
                                        );
                                        received_files.push(saved_name);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("[!] File stream error: {}", e);
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            // "I <byte_len>\n<raw_png_bytes>" -> Bitmap image clipboard sync
            b'I' => {
                if trimmed.len() > 2 {
                    if let Ok(img_size) = trimmed[2..].trim().parse::<u64>() {
                        let _ = receive_image_to_clipboard(&mut reader, img_size).await;
                    }
                }
            }

            // "D <cwd_path>" -> Terminal teleport
            b'D' => {
                if trimmed.len() > 2 {
                    open_terminal_at_path(trimmed[2..].trim().to_string());
                }
            }

            // "X <x_coord> [host_width]" -> Seamless cursor horizontal entry alignment (Top/Bottom)
            b'X' => {
                mark_control_session(&mut is_control, session_id, &msg_tx, peer_ip);
                if trimmed.len() > 2 {
                    let mut parts = trimmed[2..].split_whitespace();
                    if let Some(Ok(raw_x)) = parts.next().map(|s| s.parse::<i32>()) {
                        let host_span = parts.next().and_then(|s| s.parse::<i32>().ok());
                        let ret_edge = ReturnEdge::from_u8(RETURN_EDGE.load(Ordering::Relaxed));
                        let bounds = select_entry_monitor(ret_edge);
                        let (entry_x, entry_y) =
                            compute_entry_position(true, raw_x, host_span, bounds, ret_edge);

                        LAST_ENTRY_MS.store(monotonic_ms(), Ordering::SeqCst);
                        CURSOR_X.store(entry_x, Ordering::SeqCst);
                        CURSOR_Y.store(entry_y, Ordering::SeqCst);
                        let mut dev = device.lock().await;
                        dev.warp_cursor(entry_x, entry_y, bounds.min_x, bounds.min_y);
                    }
                }
            }

            // "Y <y_coord> [host_height]" -> Seamless cursor vertical entry alignment (Left/Right)
            b'Y' => {
                mark_control_session(&mut is_control, session_id, &msg_tx, peer_ip);
                if trimmed.len() > 2 {
                    let mut parts = trimmed[2..].split_whitespace();
                    if let Some(Ok(raw_y)) = parts.next().map(|s| s.parse::<i32>()) {
                        let host_span = parts.next().and_then(|s| s.parse::<i32>().ok());
                        let ret_edge = ReturnEdge::from_u8(RETURN_EDGE.load(Ordering::Relaxed));
                        let bounds = select_entry_monitor(ret_edge);
                        let (entry_x, entry_y) =
                            compute_entry_position(false, raw_y, host_span, bounds, ret_edge);

                        LAST_ENTRY_MS.store(monotonic_ms(), Ordering::SeqCst);
                        CURSOR_X.store(entry_x, Ordering::SeqCst);
                        CURSOR_Y.store(entry_y, Ordering::SeqCst);
                        let mut dev = device.lock().await;
                        dev.warp_cursor(entry_x, entry_y, bounds.min_x, bounds.min_y);
                    }
                }
            }

            // "M dx dy" -> Mouse Move + Reciprocal Return-Edge Detection
            b'M' => {
                mark_control_session(&mut is_control, session_id, &msg_tx, peer_ip);
                if trimmed.len() > 2 {
                    let mut parts = trimmed[2..].split_whitespace();
                    if let (Some(dx_s), Some(dy_s)) = (parts.next(), parts.next()) {
                        if let (Ok(dx), Ok(dy)) = (dx_s.parse::<i32>(), dy_s.parse::<i32>()) {
                            {
                                let mut dev = device.lock().await;
                                dev.emit_mouse_move(dx, dy);
                            }

                            let min_x = CURRENT_BOUND_MIN_X.load(Ordering::Relaxed);
                            let max_x = CURRENT_BOUND_MAX_X.load(Ordering::Relaxed);
                            let min_y = CURRENT_BOUND_MIN_Y.load(Ordering::Relaxed);
                            let max_y = CURRENT_BOUND_MAX_Y.load(Ordering::Relaxed);

                            let cur_x = (CURSOR_X.load(Ordering::Relaxed) + dx).clamp(min_x, max_x);
                            let cur_y = (CURSOR_Y.load(Ordering::Relaxed) + dy).clamp(min_y, max_y);
                            CURSOR_X.store(cur_x, Ordering::Relaxed);
                            CURSOR_Y.store(cur_y, Ordering::Relaxed);

                            if monotonic_ms()
                                .saturating_sub(LAST_ENTRY_MS.load(Ordering::Relaxed))
                                >= 140
                            {
                                let ret_edge =
                                    ReturnEdge::from_u8(RETURN_EDGE.load(Ordering::Relaxed));

                                if let Some((rel_coord, span_len)) =
                                    is_at_outer_return_edge(cur_x, cur_y, dx, dy, ret_edge)
                                {
                                    LAST_ENTRY_MS.store(monotonic_ms(), Ordering::SeqCst);

                                    let held = REMOTE_MOUSE_LBUTTON.load(Ordering::SeqCst);
                                    let drag_dist_x =
                                        (REMOTE_DRAG_START_X.load(Ordering::Relaxed) - cur_x).abs();
                                    let drag_dist_y =
                                        (REMOTE_DRAG_START_Y.load(Ordering::Relaxed) - cur_y).abs();
                                    let was_dragging =
                                        held && (drag_dist_x >= 35 || drag_dist_y >= 35);

                                    if was_dragging {
                                        execute_reverse_handoff().await;
                                    }

                                    REMOTE_MOUSE_LBUTTON.store(false, Ordering::SeqCst);

                                    let switch_msg =
                                        format!("switch_to_local {} {}\n", rel_coord, span_len);
                                    let _ = msg_tx.send(switch_msg.into_bytes());
                                }
                            }
                        }
                    }
                }
            }

            // "U <url>" -> Instant Browser Handoff
            b'U' => {
                mark_control_session(&mut is_control, session_id, &msg_tx, peer_ip);
                if trimmed.len() > 2 {
                    let url = trimmed[2..].trim().to_string();
                    if !url.is_empty() {
                        open_url_in_browser(url);
                    }
                }
            }

            // "C left/right/middle down/up" -> Click
            b'C' => {
                mark_control_session(&mut is_control, session_id, &msg_tx, peer_ip);
                if trimmed.len() > 2 {
                    let mut parts = trimmed[2..].split_whitespace();
                    if let (Some(btn), Some(act)) = (parts.next(), parts.next()) {
                        let is_down = act == "down";
                        if btn == "left" {
                            if is_down {
                                REMOTE_MOUSE_LBUTTON.store(true, Ordering::SeqCst);
                                REMOTE_DRAG_START_X
                                    .store(CURSOR_X.load(Ordering::Relaxed), Ordering::Relaxed);
                                REMOTE_DRAG_START_Y
                                    .store(CURSOR_Y.load(Ordering::Relaxed), Ordering::Relaxed);
                            } else {
                                REMOTE_MOUSE_LBUTTON.store(false, Ordering::SeqCst);
                            }
                        }
                        let mut dev = device.lock().await;
                        dev.emit_mouse_click(btn, is_down);
                    }
                }
            }

            // "S dx dy" -> Scroll
            b'S' => {
                mark_control_session(&mut is_control, session_id, &msg_tx, peer_ip);
                if trimmed.len() > 2 {
                    let mut parts = trimmed[2..].split_whitespace();
                    if let (Some(dx_s), Some(dy_s)) = (parts.next(), parts.next()) {
                        if let (Ok(dx), Ok(dy)) = (dx_s.parse::<i32>(), dy_s.parse::<i32>()) {
                            let mut dev = device.lock().await;
                            dev.emit_mouse_wheel(dx, dy);
                        }
                    }
                }
            }

            // "K code down/up" -> Key
            b'K' => {
                mark_control_session(&mut is_control, session_id, &msg_tx, peer_ip);
                if trimmed.len() > 2 {
                    let mut parts = trimmed[2..].split_whitespace();
                    if let (Some(code_s), Some(act)) = (parts.next(), parts.next()) {
                        if let Ok(code) = code_s.parse::<u16>() {
                            let is_down = act == "down";
                            let mut dev = device.lock().await;
                            dev.emit_key(code, is_down);
                        }
                    }
                }
            }

            // "T <escaped_text>" -> Lightweight text clipboard sync
            b'T' => {
                mark_control_session(&mut is_control, session_id, &msg_tx, peer_ip);
                if trimmed.len() > 2 {
                    let text = unescape_text_line(&trimmed[2..]);
                    if !text.is_empty() {
                        copy_text_to_system_clipboard(text);
                    }
                }
            }

            // Legacy "F <filename> <b64>" fallback
            b'F' => {
                mark_control_session(&mut is_control, session_id, &msg_tx, peer_ip);
                if trimmed.len() > 2 {
                    let rest = trimmed[2..].trim();
                    if let Some(idx) = rest.find(' ') {
                        let safe_rel = sanitize_relative_path(&rest[..idx]);
                        let b64_data = rest[idx + 1..].trim();
                        if let Ok(data) = decode_base64(b64_data) {
                            let target = downloads_dir().join(&safe_rel);
                            if let Some(parent) = target.parent() {
                                let _ = fs::create_dir_all(parent).await;
                            }
                            if fs::write(&target, &data).await.is_ok() {
                                notify_files_received(&[safe_rel.to_string_lossy().into_owned()]);
                            }
                        }
                    }
                }
            }

            // "PING <ts>" -> Round-trip telemetry probe
            b'P' => {
                if let Some(rest) = trimmed.strip_prefix("PING ") {
                    let ts = rest.trim();
                    let pong = format!("PONG {}\n", ts);
                    let _ = msg_tx.send(pong.into_bytes());
                }
            }

            // "SYS_LOCK" -> Synchronized lock screen broadcast
            b'L' => {
                if trimmed == "LOCK" || trimmed == "SYS_LOCK" {
                    lock_receiver_workstation();
                }
            }

            // Legacy JSON clipboard fallback
            b'{' => {
                mark_control_session(&mut is_control, session_id, &msg_tx, peer_ip);
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    if json_val["type"] == "clipboard" {
                        if let Some(text) = json_val["text"].as_str() {
                            copy_text_to_system_clipboard(text.to_string());
                        }
                    }
                }
            }

            _ => {}
        }

        if line.capacity() > 8192 {
            line = String::with_capacity(256);
        }
    }

    if !received_files.is_empty() {
        notify_files_received(&received_files);
    }

    if is_control {
        println!(
            "[-] KVM control session #{} closed from: {:?}",
            session_id, peer
        );
        if ACTIVE_SESSION_ID.load(Ordering::SeqCst) == session_id {
            if let Ok(mut lock) = ACTIVE_CONTROL_WRITER.lock() {
                *lock = None;
            }
            if let Ok(mut lock) = ACTIVE_HOST_PEER.lock() {
                *lock = None;
            }
        }
        if ACTIVE_CONTROL_CLIENTS.fetch_sub(1, Ordering::SeqCst) == 1 {
            tokio::spawn(async {
                sleep(Duration::from_millis(1500)).await;
                if ACTIVE_CONTROL_CLIENTS.load(Ordering::SeqCst) == 0 {
                    restore_docked_monitors_if_needed_async();
                }
            });
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = monotonic_ms();
    let return_edge = parse_return_edge();
    RETURN_EDGE.store(return_edge.to_u8(), Ordering::SeqCst);
    println!("=== Tailscale KVM Universal Receiver Daemon ===");
    println!(
        "[*] Initial return edge trigger: {:?} (auto-syncs dynamically with Host GUI)",
        return_edge
    );

    let mut warned_input = false;
    let adapter = loop {
        match InputAdapter::new() {
            Ok(a) => break a,
            Err(e) => {
                if !warned_input {
                    warned_input = true;
                    eprintln!(
                        "[!] Waiting for virtual input device access (/dev/uinput): {}. Checking gently every 20s...",
                        e
                    );
                }
                sleep(Duration::from_secs(20)).await;
            }
        }
    };
    let device = Arc::new(Mutex::new(adapter));
    update_current_monitor_bounds();
    println!(
        "[*] Detected display bounds: X=[{}, {}], Y=[{}, {}]",
        CURRENT_BOUND_MIN_X.load(Ordering::SeqCst),
        CURRENT_BOUND_MAX_X.load(Ordering::SeqCst),
        CURRENT_BOUND_MIN_Y.load(Ordering::SeqCst),
        CURRENT_BOUND_MAX_Y.load(Ordering::SeqCst)
    );

    start_bounds_refresh_worker();
    start_cursorpos_worker();
    start_clipboard_monitor();

    let listener = TcpListener::bind("0.0.0.0:9876").await?;
    println!("[*] Universal Receiver listening on 0.0.0.0:9876...");

    loop {
        let (stream, _) = listener.accept().await?;
        let _ = stream.set_nodelay(true);
        // Assign session ID strictly in TCP accept arrival order!
        let session_id = NEXT_SESSION_ID.fetch_add(1, Ordering::SeqCst) + 1;
        let dev_clone = Arc::clone(&device);
        tokio::spawn(handle_connection(stream, dev_clone, session_id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_return_edge_parsing() {
        assert_eq!(ReturnEdge::parse("left"), ReturnEdge::Left);
        assert_eq!(ReturnEdge::parse("RIGHT"), ReturnEdge::Right);
        assert_eq!(ReturnEdge::parse("top"), ReturnEdge::Top);
        assert_eq!(ReturnEdge::parse("up"), ReturnEdge::Top);
        assert_eq!(ReturnEdge::parse("bottom"), ReturnEdge::Bottom);
        assert_eq!(ReturnEdge::parse("down"), ReturnEdge::Bottom);
    }

    #[test]
    fn test_compute_entry_position_all_four_sides() {
        let bounds = MonitorBounds {
            min_x: 0,
            max_x: 1920,
            min_y: 0,
            max_y: 1200,
        };

        let (x, y) = compute_entry_position(false, 720, Some(1440), bounds, ReturnEdge::Left);
        assert_eq!(x, 18);
        assert_eq!(y, 600);

        let (x, y) = compute_entry_position(false, 720, Some(1440), bounds, ReturnEdge::Right);
        assert_eq!(x, 1902);
        assert_eq!(y, 600);

        let (x, y) = compute_entry_position(true, 1280, Some(2560), bounds, ReturnEdge::Bottom);
        assert_eq!(x, 960);
        assert_eq!(y, 1182);

        let (x, y) = compute_entry_position(true, 1280, Some(2560), bounds, ReturnEdge::Top);
        assert_eq!(x, 960);
        assert_eq!(y, 18);
    }
}
