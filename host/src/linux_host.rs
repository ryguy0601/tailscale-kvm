//! Native Linux Host Backend (Wayland/Hyprland & X11 edge detection + evdev EVIOCGRAB capture)

use evdev::{Device, EventType, Key, RelativeAxisType};
use std::env;
use std::fs;
use std::io::{Cursor, Read, Write};
use std::os::unix::io::AsRawFd;
use std::os::unix::net::UnixStream;
use std::process::Command;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use crate::{
    monotonic_ms, send_to_active_remote, switch_to_local, switch_to_remote_slot, EdgeDirection,
    HOST_MAX_X, HOST_MAX_Y, HOST_MIN_X, HOST_MIN_Y, IS_REMOTE, LAST_LOCAL_RETURN_MS,
    SLOT_CONNECTED, SUPPRESS_CLIPBOARD_SYNC,
};

pub fn update_linux_screen_bounds() {
    if let Ok(out) = Command::new("hyprctl").args(["monitors", "-j"]).output() {
        if let Ok(json_val) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
            if let Some(arr) = json_val.as_array() {
                let mut min_x = i32::MAX;
                let mut max_x = i32::MIN;
                let mut min_y = i32::MAX;
                let mut max_y = i32::MIN;

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

                    min_x = min_x.min(x);
                    max_x = max_x.max(x + w);
                    min_y = min_y.min(y);
                    max_y = max_y.max(y + h);
                }

                if min_x != i32::MAX && max_x > min_x && max_y > min_y {
                    HOST_MIN_X.store(min_x, Ordering::SeqCst);
                    HOST_MAX_X.store(max_x, Ordering::SeqCst);
                    HOST_MIN_Y.store(min_y, Ordering::SeqCst);
                    HOST_MAX_Y.store(max_y, Ordering::SeqCst);
                    return;
                }
            }
        }
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
                            HOST_MIN_X.store(0, Ordering::SeqCst);
                            HOST_MAX_X.store(w, Ordering::SeqCst);
                            HOST_MIN_Y.store(0, Ordering::SeqCst);
                            HOST_MAX_Y.store(h, Ordering::SeqCst);
                            return;
                        }
                    }
                }
            }
        }
    }
}

pub fn warp_linux_cursor(x: i32, y: i32) {
    if let Ok(sig) = env::var("HYPRLAND_INSTANCE_SIGNATURE") {
        if !sig.is_empty() {
            let xdg = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".to_string());
            let sock_path = format!("{}/hypr/{}/.socket.sock", xdg, sig);
            if let Ok(mut stream) = UnixStream::connect(&sock_path) {
                let cmd = format!("dispatch movecursor {} {}", x, y);
                let _ = stream.write_all(cmd.as_bytes());
                return;
            }
        }
    }
    if env::var("DISPLAY").is_ok() {
        let _ = Command::new("xdotool")
            .args(["mousemove", &x.to_string(), &y.to_string()])
            .status();
    }
}

fn query_linux_cursor_pos() -> Option<(i32, i32)> {
    if let Ok(sig) = env::var("HYPRLAND_INSTANCE_SIGNATURE") {
        if !sig.is_empty() {
            let xdg = env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".to_string());
            let sock_path = format!("{}/hypr/{}/.socket.sock", xdg, sig);
            if let Ok(mut stream) = UnixStream::connect(&sock_path) {
                if stream.write_all(b"cursorpos").is_ok() {
                    let mut buf = [0u8; 128];
                    if let Ok(n) = stream.read(&mut buf) {
                        if n > 0 {
                            let s = String::from_utf8_lossy(&buf[..n]);
                            let mut parts = s.split(',');
                            if let (Some(xs), Some(ys)) = (parts.next(), parts.next()) {
                                if let (Ok(x), Ok(y)) =
                                    (xs.trim().parse::<i32>(), ys.trim().parse::<i32>())
                                {
                                    return Some((x, y));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if env::var("DISPLAY").is_ok() {
        if let Ok(out) = Command::new("xdotool")
            .args(["getmouselocation", "--shell"])
            .output()
        {
            let s = String::from_utf8_lossy(&out.stdout);
            let mut x_opt = None;
            let mut y_opt = None;
            for line in s.lines() {
                if let Some(v) = line.strip_prefix("X=") {
                    x_opt = v.trim().parse::<i32>().ok();
                } else if let Some(v) = line.strip_prefix("Y=") {
                    y_opt = v.trim().parse::<i32>().ok();
                }
            }
            if let (Some(x), Some(y)) = (x_opt, y_opt) {
                return Some((x, y));
            }
        }
    }

    None
}

fn open_physical_input_devices() -> Vec<Device> {
    let mut grabbed = Vec::new();
    let Ok(entries) = fs::read_dir("/dev/input") else {
        return grabbed;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name_str) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !name_str.starts_with("event") {
            continue;
        }
        let Ok(mut dev) = Device::open(&path) else {
            continue;
        };
        let dev_name = dev.name().unwrap_or("");
        if dev_name.contains("Tailscale-KVM") {
            continue;
        }

        let is_mouse = dev
            .supported_relative_axes()
            .map(|axes| axes.contains(RelativeAxisType::REL_X))
            .unwrap_or(false);
        let is_kbd = dev
            .supported_keys()
            .map(|keys| keys.contains(Key::KEY_A) && keys.contains(Key::KEY_ENTER))
            .unwrap_or(false);

        if is_mouse || is_kbd {
            // Set non-blocking fd for responsive multi-device polling
            unsafe {
                let fd = dev.as_raw_fd();
                let flags = libc_fcntl_getfl(fd);
                if flags >= 0 {
                    libc_fcntl_setfl(fd, flags | 0o4000); // O_NONBLOCK on Linux
                }
            }
            if dev.grab().is_ok() {
                grabbed.push(dev);
            }
        }
    }
    grabbed
}

extern "C" {
    fn fcntl(fd: i32, cmd: i32, ...) -> i32;
}

unsafe fn libc_fcntl_getfl(fd: i32) -> i32 {
    fcntl(fd, 3) // F_GETFL
}

unsafe fn libc_fcntl_setfl(fd: i32, flags: i32) -> i32 {
    fcntl(fd, 4, flags) // F_SETFL
}

pub fn start_linux_clipboard_sync() {
    thread::spawn(|| {
        let mut last_clip = String::new();
        loop {
            thread::sleep(Duration::from_millis(650));
            if SUPPRESS_CLIPBOARD_SYNC.load(Ordering::SeqCst) || !crate::any_slot_connected() {
                continue;
            }
            let text_opt = Command::new("wl-paste")
                .args(["--no-newline"])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .map(|o| String::from_utf8_lossy(&o.stdout).to_string());

            if let Some(text) = text_opt {
                if !text.is_empty() && text.len() <= 1_048_576 && text != last_clip {
                    last_clip = text.clone();
                    crate::broadcast_clipboard_text(&text);
                }
            }
        }
    });
}

pub fn write_linux_clipboard_text(text: &str) {
    let t = text.to_string();
    thread::spawn(move || {
        if let Ok(mut child) = Command::new("wl-copy")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(t.as_bytes());
            }
            let _ = child.wait();
            return;
        }
        if let Ok(mut child) = Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(t.as_bytes());
            }
            let _ = child.wait();
        }
    });
}

pub fn run_linux_host_loop() {
    update_linux_screen_bounds();
    start_linux_clipboard_sync();

    println!(
        "[*] Linux Host screen bounds: X=[{}, {}], Y=[{}, {}]",
        HOST_MIN_X.load(Ordering::SeqCst),
        HOST_MAX_X.load(Ordering::SeqCst),
        HOST_MIN_Y.load(Ordering::SeqCst),
        HOST_MAX_Y.load(Ordering::SeqCst)
    );

    let mut warned_permissions = false;
    let mut tick: u32 = 0;

    loop {
        tick = tick.wrapping_add(1);
        if tick % 200 == 0 {
            update_linux_screen_bounds();
        }

        if !IS_REMOTE.load(Ordering::SeqCst) {
            if monotonic_ms().saturating_sub(LAST_LOCAL_RETURN_MS.load(Ordering::Relaxed)) >= 150 {
                if let Some((x, y)) = query_linux_cursor_pos() {
                    let min_x = HOST_MIN_X.load(Ordering::Relaxed);
                    let max_x = HOST_MAX_X.load(Ordering::Relaxed);
                    let min_y = HOST_MIN_Y.load(Ordering::Relaxed);
                    let max_y = HOST_MAX_Y.load(Ordering::Relaxed);
                    let w = (max_x - min_x).max(1);
                    let h = (max_y - min_y).max(1);

                    let right_ok = SLOT_CONNECTED[0].load(Ordering::Relaxed);
                    let left_ok = SLOT_CONNECTED[1].load(Ordering::Relaxed);
                    let up_ok = SLOT_CONNECTED[2].load(Ordering::Relaxed);
                    let down_ok = SLOT_CONNECTED[3].load(Ordering::Relaxed);

                    let mut triggered: Option<EdgeDirection> = None;
                    if right_ok && x >= max_x - 3 && y > min_y + 50 && y < max_y - 50 {
                        triggered = Some(EdgeDirection::Right);
                    } else if left_ok && x <= min_x + 3 && y > min_y + 50 && y < max_y - 50 {
                        triggered = Some(EdgeDirection::Left);
                    } else if up_ok && y <= min_y + 3 && x > min_x + 50 && x < max_x - 50 {
                        triggered = Some(EdgeDirection::Up);
                    } else if down_ok && y >= max_y - 3 && x > min_x + 50 && x < max_x - 50 {
                        triggered = Some(EdgeDirection::Down);
                    }

                    if let Some(edge) = triggered {
                        let mut devices = open_physical_input_devices();
                        if devices.is_empty() {
                            if !warned_permissions {
                                warned_permissions = true;
                                eprintln!(
                                    "[!] Linux Host could not grab /dev/input/event* devices. Run `sudo usermod -aG input $USER` (and re-login) or run with sudo."
                                );
                            }
                        } else {
                            let slot_idx = edge.to_slot_index();
                            let (entry_coord, span) =
                                if edge == EdgeDirection::Up || edge == EdgeDirection::Down {
                                    ((x - min_x).max(0), w)
                                } else {
                                    ((y - min_y).max(0), h)
                                };
                            switch_to_remote_slot(slot_idx, false, entry_coord, span);

                            // Forward grabbed physical events while IS_REMOTE remains active
                            while IS_REMOTE.load(Ordering::SeqCst) {
                                let mut acc_dx = 0i32;
                                let mut acc_dy = 0i32;
                                let mut released = false;

                                for dev in &mut devices {
                                    if let Ok(events) = dev.fetch_events() {
                                        for ev in events {
                                            match ev.event_type() {
                                                EventType::RELATIVE => match RelativeAxisType(ev.code()) {
                                                    RelativeAxisType::REL_X => acc_dx += ev.value(),
                                                    RelativeAxisType::REL_Y => acc_dy += ev.value(),
                                                    RelativeAxisType::REL_WHEEL => {
                                                        let pkt = format!("S 0 {}\n", ev.value());
                                                        send_to_active_remote(pkt.as_bytes());
                                                    }
                                                    RelativeAxisType::REL_HWHEEL => {
                                                        let pkt = format!("S {} 0\n", ev.value());
                                                        send_to_active_remote(pkt.as_bytes());
                                                    }
                                                    _ => {}
                                                },
                                                EventType::KEY => {
                                                    let code = ev.code();
                                                    let val = ev.value();
                                                    if val == 2 {
                                                        // Skip autorepeat; receiver handles key hold natively
                                                        continue;
                                                    }
                                                    let is_down = val == 1;

                                                    // Emergency release hotkeys: F12 (88), Pause (119), ScrollLock (70)
                                                    if is_down && (code == 88 || code == 119 || code == 70) {
                                                        switch_to_local(slot_idx, None, None);
                                                        released = true;
                                                        break;
                                                    }

                                                    if code == Key::BTN_LEFT.code() {
                                                        send_to_active_remote(if is_down {
                                                            b"C left down\n"
                                                        } else {
                                                            b"C left up\n"
                                                        });
                                                    } else if code == Key::BTN_RIGHT.code() {
                                                        send_to_active_remote(if is_down {
                                                            b"C right down\n"
                                                        } else {
                                                            b"C right up\n"
                                                        });
                                                    } else if code == Key::BTN_MIDDLE.code() {
                                                        send_to_active_remote(if is_down {
                                                            b"C middle down\n"
                                                        } else {
                                                            b"C middle up\n"
                                                        });
                                                    } else if code < 248 {
                                                        let mut buf = [0u8; 24];
                                                        let mut cur = Cursor::new(&mut buf[..]);
                                                        let act = if is_down { "down" } else { "up" };
                                                        if writeln!(cur, "K {} {}", code, act).is_ok() {
                                                            let len = cur.position() as usize;
                                                            send_to_active_remote(&buf[..len]);
                                                        }
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                    if released {
                                        break;
                                    }
                                }

                                if !released && (acc_dx != 0 || acc_dy != 0) {
                                    let mut buf = [0u8; 32];
                                    let mut cur = Cursor::new(&mut buf[..]);
                                    if writeln!(cur, "M {} {}", acc_dx, acc_dy).is_ok() {
                                        let len = cur.position() as usize;
                                        send_to_active_remote(&buf[..len]);
                                    }
                                }

                                thread::sleep(Duration::from_millis(2));
                            }

                            for mut dev in devices {
                                let _ = dev.ungrab();
                            }
                        }
                    }
                }
            }
        }

        thread::sleep(Duration::from_millis(12));
    }
}
