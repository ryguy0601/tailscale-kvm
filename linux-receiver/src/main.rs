use evdev::uinput::VirtualDeviceBuilder;
use evdev::{AttributeSet, EventType, InputEvent, Key, RelativeAxisType};
use std::env;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use tokio::fs::{self, File};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream, UnixStream};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

static CURSOR_X: AtomicI32 = AtomicI32::new(6000);
static CURSOR_Y: AtomicI32 = AtomicI32::new(540);
static CURRENT_LEFT_BOUND: AtomicI32 = AtomicI32::new(0);
static ACTIVE_CONTROL_CLIENTS: AtomicUsize = AtomicUsize::new(0);
static SWITCHED_SINGLE_MONITOR: AtomicBool = AtomicBool::new(false);

static REMOTE_MOUSE_LBUTTON: AtomicBool = AtomicBool::new(false);
static REMOTE_DRAG_START_X: AtomicI32 = AtomicI32::new(0);
static REMOTE_HANDOFF_BUSY: AtomicBool = AtomicBool::new(false);

static LAST_WAYLAND_CLIP_TEXT: StdMutex<String> = StdMutex::new(String::new());
static ACTIVE_CONTROL_WRITER: StdMutex<Option<tokio::sync::mpsc::UnboundedSender<Vec<u8>>>> =
    StdMutex::new(None);
static ACTIVE_WINDOWS_PEER: StdMutex<Option<std::net::IpAddr>> = StdMutex::new(None);

const STREAM_CHUNK_SIZE: usize = 65_536; // 64 KB fixed streaming buffer
const MAX_SINGLE_FILE_BYTES: u64 = 64 * 1024 * 1024 * 1024; // 64 GB sanity cap
const MAX_IMAGE_BYTES: u64 = 64 * 1024 * 1024; // 64 MB image cap

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

fn apply_single_monitor_if_docked() {
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
            let new_bound = get_hyprland_left_bound();
            CURRENT_LEFT_BOUND.store(new_bound, Ordering::SeqCst);
        }
    }
}

fn restore_docked_monitors_if_needed() {
    if SWITCHED_SINGLE_MONITOR.swap(false, Ordering::SeqCst) {
        let (physical, _) = monitor_counts();
        if physical > 1 {
            println!("[*] KVM client disconnected. Restoring full docked monitor layout...");
            let _ = Command::new("hyprmoncfg")
                .args(["apply", "docked", "--confirm-timeout", "0"])
                .status();
            let new_bound = get_hyprland_left_bound();
            CURRENT_LEFT_BOUND.store(new_bound, Ordering::SeqCst);
        }
    }
}

fn get_hyprland_left_bound() -> i32 {
    let output = Command::new("hyprctl").args(["monitors", "-j"]).output();

    if let Ok(out) = output {
        if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            if let Some(arr) = json_val.as_array() {
                let mut leftmost = 0;
                for (i, m) in arr.iter().enumerate() {
                    if let Some(x) = m["x"].as_i64() {
                        if i == 0 || (x as i32) < leftmost {
                            leftmost = x as i32;
                        }
                    }
                }
                return leftmost;
            }
        }
    }
    0
}

fn start_cursorpos_worker() {
    tokio::spawn(async {
        let xdg = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".to_string());
        let sig = env::var("HYPRLAND_INSTANCE_SIGNATURE").unwrap_or_default();
        if sig.is_empty() {
            return;
        }
        let sock_path = format!("{}/hypr/{}/.socket.sock", xdg, sig);

        let mut buf = [0u8; 128];
        loop {
            if let Ok(mut stream) = UnixStream::connect(&sock_path).await {
                if stream.write_all(b"cursorpos").await.is_ok() {
                    if let Ok(n) = stream.read(&mut buf).await {
                        if n > 0 {
                            let s = String::from_utf8_lossy(&buf[..n]);
                            let mut parts = s.split(',');
                            if let (Some(x_str), Some(y_str)) = (parts.next(), parts.next()) {
                                if let (Ok(x), Ok(y)) = (x_str.trim().parse::<i32>(), y_str.trim().parse::<i32>()) {
                                    CURSOR_X.store(x, Ordering::Relaxed);
                                    CURSOR_Y.store(y, Ordering::Relaxed);
                                }
                            }
                        }
                    }
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

        // Launch inside the proper Hyprland UWSM session scope so Firefox outlives service restarts
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

        // Bring Firefox into focus on Hyprland so the handed-off tab is immediately visible
        sleep(Duration::from_millis(250)).await;
        let _ = tokio::process::Command::new("omarchy-hyprland-focus-app")
            .arg("firefox")
            .status()
            .await;
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
    let home = env::var("HOME").unwrap_or_else(|_| "/home/ryguy".to_string());
    PathBuf::from(home).join("Downloads")
}

async fn receive_binary_file<R: tokio::io::AsyncRead + Unpin>(
    reader: &mut R,
    file_size: u64,
    raw_rel_path: &str,
) -> Result<String, std::io::Error> {
    if file_size > MAX_SINGLE_FILE_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "file exceeds maximum size",
        ));
    }

    let safe_rel = sanitize_relative_path(raw_rel_path);
    let base_downloads = downloads_dir();
    let target_path = base_downloads.join(&safe_rel);

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
    Ok(safe_rel.to_string_lossy().into_owned())
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

    let mut child = tokio::process::Command::new("wl-copy")
        .args(["-t", "image/png"])
        .stdin(std::process::Stdio::piped())
        .spawn()?;

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
    Ok(())
}

fn notify_files_received(files: &[String]) {
    if files.is_empty() {
        return;
    }
    let body = if files.len() == 1 {
        format!("Saved {} to ~/Downloads", files[0])
    } else {
        format!("Saved {} files (e.g. {}) to ~/Downloads", files.len(), files[0])
    };
    let _ = Command::new("notify-send")
        .args(["-a", "Tailscale KVM", "File Received", &body])
        .spawn();
}

fn copy_text_to_wayland_clipboard(text: String) {
    {
        let mut last = LAST_WAYLAND_CLIP_TEXT.lock().unwrap();
        *last = text.clone();
    }
    tokio::spawn(async move {
        if let Ok(mut child) = tokio::process::Command::new("wl-copy")
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

fn mark_control_session(is_control: &mut bool) {
    if !*is_control {
        *is_control = true;
        if ACTIVE_CONTROL_CLIENTS.fetch_add(1, Ordering::SeqCst) == 0 {
            apply_single_monitor_if_docked();
        }
    }
}

// -------------------------------------------------------------
// Two-Way Reverse Handoff: Linux (Omarchy) -> Windows 11 Host
// -------------------------------------------------------------

#[derive(Clone, Debug)]
enum ActiveApp {
    Browser,
    FileManager,
    Terminal(String),
    Other,
}

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

            // Detect active terminal directory (foot, kitty, alacritty, etc.)
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

async fn stream_files_to_windows(win_ip: std::net::IpAddr, paths: Vec<PathBuf>) {
    let mut entries = Vec::new();
    for p in &paths {
        collect_transfer_entries(p, p, 0, &mut entries);
    }
    if entries.is_empty() {
        return;
    }

    let addr = std::net::SocketAddr::new(win_ip, 9876);
    let Ok(mut tx) = TcpStream::connect(addr).await else {
        eprintln!("[!] Failed to connect to Windows host file transfer port: {}", addr);
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

async fn execute_reverse_handoff() {
    if REMOTE_HANDOFF_BUSY
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return;
    }

    tokio::spawn(async move {
        let win_ip = {
            let lock = ACTIVE_WINDOWS_PEER.lock().unwrap();
            *lock
        };

        if let Some(target_ip) = win_ip {
            let app = get_active_hyprland_app();

            // 1. Send Escape via wtype to cancel any ongoing Wayland drag/tab detach loop
            let _ = tokio::process::Command::new("wtype")
                .args(["-k", "Escape"])
                .status()
                .await;
            sleep(Duration::from_millis(40)).await;

            match app {
                ActiveApp::Browser => {
                    // Focus address bar with Ctrl+L and copy with Ctrl+C
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

                    // Read URL from Wayland clipboard via wl-paste
                    if let Ok(out) = tokio::process::Command::new("wl-paste")
                        .args(["--no-newline"])
                        .output()
                        .await
                    {
                        let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
                        let final_url = if looks_like_url(&text) {
                            text
                        } else {
                            "about:newtab".to_string()
                        };

                        // Send "U <url>\n" over a short TCP socket to Windows port 9876
                        let addr = std::net::SocketAddr::new(target_ip, 9876);
                        if let Ok(mut tx) = TcpStream::connect(addr).await {
                            let _ = tx.set_nodelay(true);
                            let pkt = format!("U {}\n", final_url);
                            let _ = tx.write_all(pkt.as_bytes()).await;
                            let _ = tx.flush().await;
                        }
                    }

                    // Press Escape to dismiss the browser URL bar dropdown
                    let _ = tokio::process::Command::new("wtype")
                        .args(["-k", "Escape"])
                        .status()
                        .await;
                    sleep(Duration::from_millis(30)).await;

                    // Close the originating tab/window on Linux so it continues seamlessly on Windows
                    let _ = tokio::process::Command::new("wtype")
                        .args(["-M", "ctrl", "-k", "w", "-m", "ctrl"])
                        .status()
                        .await;
                }

                ActiveApp::FileManager => {
                    // Copy selection in Nautilus / file manager
                    let _ = tokio::process::Command::new("wtype")
                        .args(["-M", "ctrl", "-k", "c", "-m", "ctrl"])
                        .status()
                        .await;
                    sleep(Duration::from_millis(80)).await;

                    // Read text/uri-list from Wayland clipboard
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
                        stream_files_to_windows(target_ip, file_paths).await;
                    }
                }

                ActiveApp::Terminal(cwd) => {
                    if !cwd.is_empty() {
                        let addr = std::net::SocketAddr::new(target_ip, 9876);
                        if let Ok(mut tx) = TcpStream::connect(addr).await {
                            let _ = tx.set_nodelay(true);
                            let pkt = format!("D {}\n", cwd);
                            let _ = tx.write_all(pkt.as_bytes()).await;
                            let _ = tx.flush().await;
                        }
                    }
                }

                ActiveApp::Other => {
                    // Check if clipboard contains file URIs or plain text
                    if let Ok(out) = tokio::process::Command::new("wl-paste")
                        .args(["-t", "text/uri-list"])
                        .output()
                        .await
                    {
                        let s = String::from_utf8_lossy(&out.stdout);
                        let file_paths = parse_file_uris(&s);
                        if !file_paths.is_empty() {
                            stream_files_to_windows(target_ip, file_paths).await;
                        }
                    }
                }
            }
        }

        REMOTE_HANDOFF_BUSY.store(false, Ordering::SeqCst);
    });
}

/// Outgoing Wayland clipboard monitor for Omarchy -> Windows image and text sync
fn start_wayland_clipboard_monitor() {
    tokio::spawn(async {
        loop {
            sleep(Duration::from_millis(600)).await;

            let win_ip = {
                let lock = ACTIVE_WINDOWS_PEER.lock().unwrap();
                *lock
            };
            let Some(target_ip) = win_ip else {
                continue;
            };

            // Check if clipboard offers image/png (e.g. from hyprshot or screenshot snippet)
            if let Ok(types_out) = tokio::process::Command::new("wl-paste")
                .args(["-l"])
                .output()
                .await
            {
                let types_str = String::from_utf8_lossy(&types_out.stdout);
                if types_str.lines().any(|l| l.trim() == "image/png") {
                    // Check if it's already transmitted
                    if let Ok(out) = tokio::process::Command::new("wl-paste")
                        .args(["-t", "image/png"])
                        .output()
                        .await
                    {
                        let png_bytes = out.stdout;
                        if !png_bytes.is_empty() && png_bytes.len() < MAX_IMAGE_BYTES as usize {
                            let img_hash = format!("img:{}", png_bytes.len());
                            let changed = {
                                let mut last = LAST_WAYLAND_CLIP_TEXT.lock().unwrap();
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

            // Text clipboard sync
            if let Ok(out) = tokio::process::Command::new("wl-paste")
                .args(["--no-newline"])
                .output()
                .await
            {
                let text = String::from_utf8_lossy(&out.stdout).to_string();
                if !text.is_empty() && text.len() <= 1_048_576 {
                    let changed = {
                        let mut last = LAST_WAYLAND_CLIP_TEXT.lock().unwrap();
                        if *last != text {
                            *last = text.clone();
                            true
                        } else {
                            false
                        }
                    };
                    if changed {
                        let pkt = escape_text_for_transmission(&text);
                        let maybe_tx = {
                            let lock = ACTIVE_CONTROL_WRITER.lock().unwrap();
                            lock.clone()
                        };
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
        let target_dir = if !p.is_empty() && Path::new(p).exists() {
            p
        } else {
            "/home/ryguy"
        };
        let _ = tokio::process::Command::new("foot")
            .arg("-D")
            .arg(target_dir)
            .spawn();
    });
}

async fn handle_connection(
    stream: TcpStream,
    device: Arc<Mutex<evdev::uinput::VirtualDevice>>,
) {
    let peer = stream.peer_addr().ok();
    if let Some(ref p) = peer {
        let mut lock = ACTIVE_WINDOWS_PEER.lock().unwrap();
        *lock = Some(p.ip());
    }

    let (reader_half, mut raw_writer) = stream.into_split();
    let mut reader = BufReader::with_capacity(STREAM_CHUNK_SIZE, reader_half);
    let mut line = String::with_capacity(256);
    let mut is_control = false;
    let mut received_files: Vec<String> = Vec::new();

    let (msg_tx, mut msg_rx) = tokio::sync::mpsc::unbounded_channel::<Vec<u8>>();

    // Spawn a writer worker so asynchronous clipboard transmissions can cleanly share the socket
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
                println!("[+] KVM control session established from: {:?}", peer);
                mark_control_session(&mut is_control);
                {
                    let mut lock = ACTIVE_CONTROL_WRITER.lock().unwrap();
                    *lock = Some(msg_tx.clone());
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
                                Ok(saved_name) => {
                                    println!("[+] Saved streamed file: {} ({} bytes)", saved_name, file_size);
                                    received_files.push(saved_name);
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

            // "Y <y_coord>" -> Seamless cursor vertical alignment
            b'Y' => {
                mark_control_session(&mut is_control);
                if trimmed.len() > 2 {
                    if let Ok(target_y) = trimmed[2..].trim().parse::<i32>() {
                        let cur_y = CURSOR_Y.load(Ordering::Relaxed);
                        let dy = target_y - cur_y;
                        if dy != 0 {
                            let mut dev = device.lock().await;
                            let ev_y = InputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_Y.0, dy);
                            let syn = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0);
                            let _ = dev.emit(&[ev_y, syn]);
                        }
                    }
                }
            }

            // "M dx dy" -> Mouse Move
            b'M' => {
                mark_control_session(&mut is_control);
                if trimmed.len() > 2 {
                    let mut parts = trimmed[2..].split_whitespace();
                    if let (Some(dx_s), Some(dy_s)) = (parts.next(), parts.next()) {
                        if let (Ok(dx), Ok(dy)) = (dx_s.parse::<i32>(), dy_s.parse::<i32>()) {
                            let mut dev = device.lock().await;
                            let ev_x =
                                InputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_X.0, dx);
                            let ev_y =
                                InputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_Y.0, dy);
                            let syn = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0);
                            let _ = dev.emit(&[ev_x, ev_y, syn]);

                            // When cursor moves left and hits the left edge bound, switch back to Windows with Y coord
                            if dx < 0 {
                                let cur_x = CURSOR_X.load(Ordering::Relaxed);
                                let left_bound = CURRENT_LEFT_BOUND.load(Ordering::Relaxed);
                                if cur_x <= left_bound + 2 {
                                    let was_dragging = REMOTE_MOUSE_LBUTTON.load(Ordering::SeqCst)
                                        && (REMOTE_DRAG_START_X.load(Ordering::Relaxed) - cur_x).abs() >= 35;

                                    if was_dragging {
                                        execute_reverse_handoff().await;
                                    }

                                    REMOTE_MOUSE_LBUTTON.store(false, Ordering::SeqCst);
                                    let cur_y = CURSOR_Y.load(Ordering::Relaxed);
                                    let switch_msg = format!("switch_to_local {}\n", cur_y);
                                    let _ = msg_tx.send(switch_msg.into_bytes());
                                }
                            }
                        }
                    }
                }
            }

            // "U <url>" -> Instant Browser Handoff / Open URL in Firefox
            b'U' => {
                mark_control_session(&mut is_control);
                if trimmed.len() > 2 {
                    let url = trimmed[2..].trim().to_string();
                    if !url.is_empty() {
                        open_url_in_browser(url);
                    }
                }
            }

            // "C left/right down/up" -> Click
            b'C' => {
                mark_control_session(&mut is_control);
                if trimmed.len() > 2 {
                    let mut parts = trimmed[2..].split_whitespace();
                    if let (Some(btn), Some(act)) = (parts.next(), parts.next()) {
                        let btn_code = match btn {
                            "right" => Key::BTN_RIGHT.code(),
                            "middle" => Key::BTN_MIDDLE.code(),
                            _ => {
                                if act == "down" {
                                    REMOTE_MOUSE_LBUTTON.store(true, Ordering::SeqCst);
                                    REMOTE_DRAG_START_X.store(CURSOR_X.load(Ordering::Relaxed), Ordering::Relaxed);
                                } else {
                                    REMOTE_MOUSE_LBUTTON.store(false, Ordering::SeqCst);
                                }
                                Key::BTN_LEFT.code()
                            }
                        };
                        let act_val = if act == "down" { 1 } else { 0 };
                        let mut dev = device.lock().await;
                        let ev = InputEvent::new(EventType::KEY, btn_code, act_val);
                        let syn = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0);
                        let _ = dev.emit(&[ev, syn]);
                    }
                }
            }

            // "S dx dy" -> Scroll
            b'S' => {
                mark_control_session(&mut is_control);
                if trimmed.len() > 2 {
                    let mut parts = trimmed[2..].split_whitespace();
                    if let (Some(dx_s), Some(dy_s)) = (parts.next(), parts.next()) {
                        if let (Ok(dx), Ok(dy)) = (dx_s.parse::<i32>(), dy_s.parse::<i32>()) {
                            let mut dev = device.lock().await;
                            let mut events = Vec::with_capacity(3);
                            if dy != 0 {
                                events.push(InputEvent::new(
                                    EventType::RELATIVE,
                                    RelativeAxisType::REL_WHEEL.0,
                                    dy,
                                ));
                            }
                            if dx != 0 {
                                events.push(InputEvent::new(
                                    EventType::RELATIVE,
                                    RelativeAxisType::REL_HWHEEL.0,
                                    dx,
                                ));
                            }
                            if !events.is_empty() {
                                events.push(InputEvent::new(EventType::SYNCHRONIZATION, 0, 0));
                                let _ = dev.emit(&events);
                            }
                        }
                    }
                }
            }

            // "K code down/up" -> Key
            b'K' => {
                mark_control_session(&mut is_control);
                if trimmed.len() > 2 {
                    let mut parts = trimmed[2..].split_whitespace();
                    if let (Some(code_s), Some(act)) = (parts.next(), parts.next()) {
                        if let Ok(code) = code_s.parse::<u16>() {
                            let act_val = if act == "down" { 1 } else { 0 };
                            let mut dev = device.lock().await;
                            let ev = InputEvent::new(EventType::KEY, code, act_val);
                            let syn = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0);
                            let _ = dev.emit(&[ev, syn]);
                        }
                    }
                }
            }

            // "T <escaped_text>" -> Lightweight Wayland clipboard sync
            b'T' => {
                mark_control_session(&mut is_control);
                if trimmed.len() > 2 {
                    let text = unescape_text_line(&trimmed[2..]);
                    if !text.is_empty() {
                        copy_text_to_wayland_clipboard(text);
                    }
                }
            }

            // Legacy "F <filename> <b64>" fallback (capped for memory safety)
            b'F' => {
                mark_control_session(&mut is_control);
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

            // Legacy JSON clipboard fallback
            b'{' => {
                mark_control_session(&mut is_control);
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                    if json_val["type"] == "clipboard" {
                        if let Some(text) = json_val["text"].as_str() {
                            copy_text_to_wayland_clipboard(text.to_string());
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
        println!("[-] KVM control session closed from: {:?}", peer);
        {
            let mut lock = ACTIVE_CONTROL_WRITER.lock().unwrap();
            *lock = None;
        }
        {
            let mut lock = ACTIVE_WINDOWS_PEER.lock().unwrap();
            *lock = None;
        }
        if ACTIVE_CONTROL_CLIENTS.fetch_sub(1, Ordering::SeqCst) == 1 {
            tokio::spawn(async {
                sleep(Duration::from_millis(1500)).await;
                if ACTIVE_CONTROL_CLIENTS.load(Ordering::SeqCst) == 0 {
                    restore_docked_monitors_if_needed();
                }
            });
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Tailscale KVM Rust Receiver Daemon ===");

    let mut key_set = AttributeSet::<Key>::new();
    for code in 1..248 {
        key_set.insert(Key::new(code));
    }
    key_set.insert(Key::BTN_LEFT);
    key_set.insert(Key::BTN_RIGHT);
    key_set.insert(Key::BTN_MIDDLE);
    key_set.insert(Key::BTN_SIDE);
    key_set.insert(Key::BTN_EXTRA);

    let mut rel_set = AttributeSet::<RelativeAxisType>::new();
    rel_set.insert(RelativeAxisType::REL_X);
    rel_set.insert(RelativeAxisType::REL_Y);
    rel_set.insert(RelativeAxisType::REL_WHEEL);
    rel_set.insert(RelativeAxisType::REL_HWHEEL);

    let virtual_device = VirtualDeviceBuilder::new()?
        .name("Tailscale-KVM-Rust-Device")
        .with_keys(&key_set)?
        .with_relative_axes(&rel_set)?
        .build()?;

    let device = Arc::new(Mutex::new(virtual_device));
    let left_bound = get_hyprland_left_bound();
    CURRENT_LEFT_BOUND.store(left_bound, Ordering::SeqCst);
    println!("[*] Detected Hyprland leftmost coordinate: {}", left_bound);

    start_cursorpos_worker();
    start_wayland_clipboard_monitor();

    let listener = TcpListener::bind("0.0.0.0:9876").await?;
    println!("[*] Rust Receiver listening on port 9876...");

    loop {
        let (stream, _) = listener.accept().await?;
        let _ = stream.set_nodelay(true);
        let dev_clone = Arc::clone(&device);
        tokio::spawn(handle_connection(stream, dev_clone));
    }
}
