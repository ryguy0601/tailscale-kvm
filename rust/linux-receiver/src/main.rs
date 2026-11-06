use evdev::uinput::VirtualDeviceBuilder;
use evdev::{AttributeSet, EventType, InputEvent, Key, RelativeAxisCode};
use notify_rust::Notification;
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::Mutex;

#[derive(Deserialize, Debug)]
#[serde(tag = "type")]
enum IncomingMessage {
    #[serde(rename = "mouse_move")]
    MouseMove { dx: i32, dy: i32 },
    #[serde(rename = "mouse_click")]
    MouseClick { button: String, action: String },
    #[serde(rename = "mouse_scroll")]
    MouseScroll {
        #[serde(default)]
        dx: i32,
        #[serde(default)]
        dy: i32,
    },
    #[serde(rename = "key")]
    Key { code: u16, action: String },
    #[serde(rename = "clipboard")]
    Clipboard { text: String },
}

#[derive(Serialize, Debug)]
#[serde(tag = "type")]
enum OutgoingMessage {
    #[serde(rename = "switch_to_local")]
    SwitchToLocal,
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

fn get_cursor_x() -> Option<i32> {
    let output = Command::new("hyprctl").arg("cursorpos").output().ok()?;
    let text = String::from_utf8_lossy(&output.stdout);
    let parts: Vec<&str> = text.trim().split(',').collect();
    if !parts.is_empty() {
        return parts[0].trim().parse::<i32>().ok();
    }
    None
}

async fn handle_connection(
    mut stream: TcpStream,
    device: Arc<Mutex<evdev::uinput::VirtualDevice>>,
    left_bound: i32,
) {
    let peer = stream.peer_addr().ok();
    println!("[+] Connection accepted from: {:?}", peer);

    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    let mut check_counter = 0;

    while let Ok(Some(line)) = lines.next_line().await {
        if let Ok(msg) = serde_json::from_str::<IncomingMessage>(&line) {
            match msg {
                IncomingMessage::MouseMove { dx, dy } => {
                    let mut dev = device.lock().await;
                    let ev_x = InputEvent::new(EventType::RELATIVE, RelativeAxisCode::REL_X.0, dx);
                    let ev_y = InputEvent::new(EventType::RELATIVE, RelativeAxisCode::REL_Y.0, dy);
                    let syn = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0);
                    let _ = dev.emit(&[ev_x, ev_y, syn]);

                    // Seamless left-edge monitor boundary detection
                    if dx < 0 {
                        check_counter += 1;
                        if check_counter % 3 == 0 {
                            if let Some(cur_x) = get_cursor_x() {
                                if cur_x <= left_bound + 2 {
                                    let packet = serde_json::to_string(&OutgoingMessage::SwitchToLocal).unwrap();
                                    let _ = writer.write_all(format!("{}\n", packet).as_bytes()).await;
                                }
                            }
                        }
                    }
                }
                IncomingMessage::MouseClick { button, action } => {
                    let btn_code = match button.as_str() {
                        "right" => Key::BTN_RIGHT.code(),
                        "middle" => Key::BTN_MIDDLE.code(),
                        _ => Key::BTN_LEFT.code(),
                    };
                    let act_val = if action == "down" { 1 } else { 0 };
                    let mut dev = device.lock().await;
                    let ev = InputEvent::new(EventType::KEY, btn_code, act_val);
                    let syn = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0);
                    let _ = dev.emit(&[ev, syn]);
                }
                IncomingMessage::MouseScroll { dx, dy } => {
                    let mut dev = device.lock().await;
                    let mut events = Vec::new();
                    if dy != 0 {
                        events.push(InputEvent::new(EventType::RELATIVE, RelativeAxisCode::REL_WHEEL.0, dy));
                    }
                    if dx != 0 {
                        events.push(InputEvent::new(EventType::RELATIVE, RelativeAxisCode::REL_HWHEEL.0, dx));
                    }
                    if !events.is_empty() {
                        events.push(InputEvent::new(EventType::SYNCHRONIZATION, 0, 0));
                        let _ = dev.emit(&events);
                    }
                }
                IncomingMessage::Key { code, action } => {
                    let act_val = if action == "down" { 1 } else { 0 };
                    let mut dev = device.lock().await;
                    let ev = InputEvent::new(EventType::KEY, code, act_val);
                    let syn = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0);
                    let _ = dev.emit(&[ev, syn]);
                }
                IncomingMessage::Clipboard { text } => {
                    tokio::spawn(async move {
                        if let Ok(mut child) = tokio::process::Command::new("wl-copy")
                            .stdin(std::process::Stdio::piped())
                            .spawn()
                        {
                            if let Some(mut stdin) = child.stdin.take() {
                                let _ = stdin.write_all(text.as_bytes()).await;
                            }
                        }
                    });
                }
            }
        }
    }

    println!("[-] Connection closed from: {:?}", peer);
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Tailscale KVM Rust Receiver Daemon ===");

    // Initialize virtual keyboard keys
    let mut key_set = AttributeSet::<Key>::new();
    for code in 1..248 {
        key_set.insert(Key::new(code));
    }
    key_set.insert(Key::BTN_LEFT);
    key_set.insert(Key::BTN_RIGHT);
    key_set.insert(Key::BTN_MIDDLE);
    key_set.insert(Key::BTN_SIDE);
    key_set.insert(Key::BTN_EXTRA);

    // Initialize relative axes (mouse movement and scrolling)
    let mut rel_set = AttributeSet::<RelativeAxisCode>::new();
    rel_set.insert(RelativeAxisCode::REL_X);
    rel_set.insert(RelativeAxisCode::REL_Y);
    rel_set.insert(RelativeAxisCode::REL_WHEEL);
    rel_set.insert(RelativeAxisCode::REL_HWHEEL);

    let virtual_device = VirtualDeviceBuilder::new()?
        .name("Tailscale-KVM-Rust-Device")
        .with_keys(&key_set)?
        .with_relative_axes(&rel_set)?
        .build()?;

    let device = Arc::new(Mutex::new(virtual_device));
    let left_bound = get_hyprland_left_bound();
    println!("[*] Detected Hyprland leftmost coordinate: {}", left_bound);

    let listener = TcpListener::bind("0.0.0.0:9876").await?;
    println!("[*] Rust Receiver listening on port 9876...");

    loop {
        let (stream, _) = listener.accept().await?;
        let _ = stream.set_nodelay(true);
        let dev_clone = Arc::clone(&device);
        tokio::spawn(handle_connection(stream, dev_clone, left_bound));
    }
}
