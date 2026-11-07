use evdev::uinput::VirtualDeviceBuilder;
use evdev::{AttributeSet, EventType, InputEvent, Key, RelativeAxisType};
use std::env;
use std::fs::File;
use std::io::Write as StdWrite;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream, UnixStream};
use tokio::sync::Mutex;
use tokio::time::{sleep, Duration};

static CURSOR_X: AtomicI32 = AtomicI32::new(6000);
static CURRENT_LEFT_BOUND: AtomicI32 = AtomicI32::new(0);

fn apply_single_monitor_if_docked() {
    let output = Command::new("hyprctl")
        .args(["monitors", "-j"])
        .output();

    if let Ok(out) = output {
        if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            if let Some(arr) = json_val.as_array() {
                // If more than 1 monitor is connected, laptop is docked with external monitors
                if arr.len() > 1 {
                    println!("[*] Docked multi-monitor detected ({} outputs). Switching to docked-single profile...", arr.len());
                    let _ = Command::new("hyprmoncfg")
                        .args(["apply", "docked-single", "--confirm-timeout", "0"])
                        .status();
                    let new_bound = get_hyprland_left_bound();
                    CURRENT_LEFT_BOUND.store(new_bound, Ordering::SeqCst);
                }
            }
        }
    }
}

fn restore_docked_monitors_if_needed() {
    println!("[*] KVM client disconnected. Restoring full docked monitor layout...");
    let _ = Command::new("hyprmoncfg")
        .args(["apply", "docked", "--confirm-timeout", "0"])
        .status();
    let new_bound = get_hyprland_left_bound();
    CURRENT_LEFT_BOUND.store(new_bound, Ordering::SeqCst);
}

fn get_hyprland_left_bound() -> i32 {
    let output = Command::new("hyprctl")
        .args(["monitors", "-j"])
        .output();

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
        let formatted = if !url.starts_with("http://") && !url.starts_with("https://") {
            format!("https://{}", url)
        } else {
            url
        };
        // Prefer firefox if present, fallback to xdg-open
        let mut cmd = tokio::process::Command::new("firefox");
        cmd.arg(&formatted);
        if cmd.spawn().is_err() {
            let _ = tokio::process::Command::new("xdg-open")
                .arg(&formatted)
                .spawn();
        }
    });
}

fn save_transferred_file(filename: String, b64_data: String) {
    tokio::spawn(async move {
        let home = env::var("HOME").unwrap_or_else(|_| "/home/ryguy".to_string());
        let downloads_dir = PathBuf::from(&home).join("Downloads");
        let _ = std::fs::create_dir_all(&downloads_dir);
        let target_path = downloads_dir.join(&filename);

        // Simple base64 decode
        if let Ok(data) = decode_base64(&b64_data) {
            if let Ok(mut f) = File::create(&target_path) {
                let _ = f.write_all(&data);
                let _ = Command::new("notify-send")
                    .args([
                        "-a",
                        "Tailscale KVM",
                        "File Received",
                        &format!("Saved {} to Downloads", filename),
                    ])
                    .spawn();
            }
        }
    });
}

fn decode_base64(s: &str) -> Result<Vec<u8>, ()> {
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

async fn handle_connection(
    stream: TcpStream,
    device: Arc<Mutex<evdev::uinput::VirtualDevice>>,
) {
    let peer = stream.peer_addr().ok();
    println!("[+] Connection accepted from: {:?}", peer);

    // If docked with multiple monitors, enforce single internal monitor while running
    apply_single_monitor_if_docked();

    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let bytes = line.as_bytes();
        if bytes.is_empty() {
            continue;
        }

        match bytes[0] {
            // "M dx dy" -> Mouse Move
            b'M' => {
                let mut parts = line[2..].split_whitespace();
                if let (Some(dx_s), Some(dy_s)) = (parts.next(), parts.next()) {
                    if let (Ok(dx), Ok(dy)) = (dx_s.parse::<i32>(), dy_s.parse::<i32>()) {
                        let mut dev = device.lock().await;
                        let ev_x = InputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_X.0, dx);
                        let ev_y = InputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_Y.0, dy);
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

            // "U <url>" -> Instant Browser Handoff / Open URL
            b'U' => {
                if line.len() > 2 {
                    let url = line[2..].trim().to_string();
                    if !url.is_empty() {
                        open_url_in_browser(url);
                    }
                }
            }

            // "F <filename> <base64_data>" -> Incoming File Transfer
            b'F' => {
                let rest = line[2..].trim();
                if let Some(idx) = rest.find(' ') {
                    let filename = rest[..idx].to_string();
                    let b64_data = rest[idx + 1..].trim().to_string();
                    if !filename.is_empty() && !b64_data.is_empty() {
                        save_transferred_file(filename, b64_data);
                    }
                }
            }

            // "C left/right down/up" -> Click
            b'C' => {
                let mut parts = line[2..].split_whitespace();
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

            // "S dx dy" -> Scroll
            b'S' => {
                let mut parts = line[2..].split_whitespace();
                if let (Some(dx_s), Some(dy_s)) = (parts.next(), parts.next()) {
                    if let (Ok(dx), Ok(dy)) = (dx_s.parse::<i32>(), dy_s.parse::<i32>()) {
                        let mut dev = device.lock().await;
                        let mut events = Vec::with_capacity(3);
                        if dy != 0 {
                            events.push(InputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_WHEEL.0, dy));
                        }
                        if dx != 0 {
                            events.push(InputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_HWHEEL.0, dx));
                        }
                        if !events.is_empty() {
                            events.push(InputEvent::new(EventType::SYNCHRONIZATION, 0, 0));
                            let _ = dev.emit(&events);
                        }
                    }
                }
            }

            // "K code down/up" -> Key
            b'K' => {
                let mut parts = line[2..].split_whitespace();
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

            // JSON fallback for clipboard
            b'{' => {
                if let Ok(json_val) = serde_json::from_str::<serde_json::Value>(&line) {
                    if json_val["type"] == "clipboard" {
                        if let Some(text) = json_val["text"].as_str() {
                            let text_owned = text.to_string();
                            tokio::spawn(async move {
                                if let Ok(mut child) = tokio::process::Command::new("wl-copy")
                                    .stdin(std::process::Stdio::piped())
                                    .spawn()
                                {
                                    if let Some(mut stdin) = child.stdin.take() {
                                        let _ = stdin.write_all(text_owned.as_bytes()).await;
                                    }
                                }
                            });
                        }
                    }
                }
            }

            _ => {}
        }
    }

    println!("[-] Connection closed from: {:?}", peer);
    restore_docked_monitors_if_needed();
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
