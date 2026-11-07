use evdev::uinput::VirtualDeviceBuilder;
use evdev::{AttributeSet, EventType, InputEvent, Key, RelativeAxisType};
use std::env;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::fs::{self, File};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream, UnixStream};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

static CURSOR_X: AtomicI32 = AtomicI32::new(6000);
static CURRENT_LEFT_BOUND: AtomicI32 = AtomicI32::new(0);
static ACTIVE_CONTROL_CLIENTS: AtomicUsize = AtomicUsize::new(0);
static SWITCHED_SINGLE_MONITOR: AtomicBool = AtomicBool::new(false);

const STREAM_CHUNK_SIZE: usize = 65_536; // 64 KB fixed streaming buffer
const MAX_SINGLE_FILE_BYTES: u64 = 64 * 1024 * 1024 * 1024; // 64 GB sanity cap

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
                            if let Some(x_str) = s.split(',').next() {
                                if let Ok(x) = x_str.trim().parse::<i32>() {
                                    CURSOR_X.store(x, Ordering::Relaxed);
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

async fn handle_connection(
    stream: TcpStream,
    device: Arc<Mutex<evdev::uinput::VirtualDevice>>,
) {
    let peer = stream.peer_addr().ok();
    let (reader_half, mut writer) = stream.into_split();
    let mut reader = BufReader::with_capacity(STREAM_CHUNK_SIZE, reader_half);
    let mut line = String::with_capacity(256);
    let mut is_control = false;
    let mut received_files: Vec<String> = Vec::new();

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

                            if dx < 0 {
                                let cur_x = CURSOR_X.load(Ordering::Relaxed);
                                let left_bound = CURRENT_LEFT_BOUND.load(Ordering::Relaxed);
                                if cur_x <= left_bound + 2 {
                                    let _ = writer.write_all(b"switch_to_local\n").await;
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
                            _ => Key::BTN_LEFT.code(),
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

        // Release any temporary buffer growth from large clipboard strings immediately
        if line.capacity() > 8192 {
            line = String::with_capacity(256);
        }
    }

    if !received_files.is_empty() {
        notify_files_received(&received_files);
    }

    if is_control {
        println!("[-] KVM control session closed from: {:?}", peer);
        if ACTIVE_CONTROL_CLIENTS.fetch_sub(1, Ordering::SeqCst) == 1 {
            // Short debounce so restarting the Windows host doesn't flash docked monitors off and on
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

    let listener = TcpListener::bind("0.0.0.0:9876").await?;
    println!("[*] Rust Receiver listening on port 9876...");

    loop {
        let (stream, _) = listener.accept().await?;
        let _ = stream.set_nodelay(true);
        let dev_clone = Arc::clone(&device);
        tokio::spawn(handle_connection(stream, dev_clone));
    }
}
