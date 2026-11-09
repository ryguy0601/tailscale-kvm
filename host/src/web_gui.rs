use serde_json::json;
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct PeerInfo {
    pub name: String,
    pub os: String,
    pub online: bool,
    pub ip: String,
    pub kvm_ready: bool,
}

static GUI_SLOTS: OnceLock<Mutex<[Option<String>; 4]>> = OnceLock::new();
static CACHED_PEERS: OnceLock<Mutex<Vec<PeerInfo>>> = OnceLock::new();
static LAST_GUI_ACTIVITY_MS: AtomicU64 = AtomicU64::new(0);

fn mark_gui_active() {
    LAST_GUI_ACTIVITY_MS.store(crate::monotonic_ms(), Ordering::Relaxed);
}

fn slots_store() -> &'static Mutex<[Option<String>; 4]> {
    GUI_SLOTS.get_or_init(|| Mutex::new([None, None, None, None]))
}

fn peers_store() -> &'static Mutex<Vec<PeerInfo>> {
    CACHED_PEERS.get_or_init(|| Mutex::new(Vec::new()))
}

pub fn slot_name_to_index(slot: &str) -> Option<usize> {
    match slot.trim().to_ascii_lowercase().as_str() {
        "right" => Some(0),
        "left" => Some(1),
        "up" | "top" => Some(2),
        "down" | "bottom" => Some(3),
        _ => None,
    }
}

pub fn index_to_slot_name(idx: usize) -> &'static str {
    match idx {
        1 => "left",
        2 => "up",
        3 => "down",
        _ => "right",
    }
}

pub fn set_all_slots_mirror(slots: &[Option<String>; 4]) {
    if let Ok(mut guard) = slots_store().lock() {
        *guard = slots.clone();
    }
}

pub fn get_all_slots() -> [Option<String>; 4] {
    slots_store()
        .lock()
        .map(|g| g.clone())
        .unwrap_or([None, None, None, None])
}

pub fn probe_kvm_receiver_port(ip: &str, timeout_ms: u64) -> bool {
    let addr_str = format!("{}:9876", ip);
    let Ok(mut addrs) = addr_str.to_socket_addrs() else {
        return false;
    };
    let Some(addr) = addrs.next() else {
        return false;
    };
    if let Ok(stream) = TcpStream::connect_timeout(&addr, Duration::from_millis(timeout_ms)) {
        let _ = stream.shutdown(Shutdown::Both);
        true
    } else {
        false
    }
}

pub fn refresh_tailscale_peers() -> Vec<PeerInfo> {
    let mut cmd = std::process::Command::new("tailscale");
    cmd.args(["status", "--json"]);

    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut raw_peers: Vec<(String, String, bool, String)> = Vec::new();
    if let Ok(output) = cmd.output() {
        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
            if let Some(peers) = val.get("Peer").and_then(|p| p.as_object()) {
                for (_, peer) in peers {
                    let name = peer
                        .get("HostName")
                        .and_then(|h| h.as_str())
                        .unwrap_or("Unknown")
                        .to_string();
                    let os = peer
                        .get("OS")
                        .and_then(|o| o.as_str())
                        .unwrap_or("unknown")
                        .to_string();
                    let online = peer
                        .get("Online")
                        .and_then(|o| o.as_bool())
                        .unwrap_or(false);
                    let ip = peer
                        .get("TailscaleIPs")
                        .and_then(|ips| ips.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|i| i.as_str())
                        .unwrap_or("")
                        .to_string();
                    if !ip.is_empty() {
                        raw_peers.push((name, os, online, ip));
                    }
                }
            }
        }
    }

    // Probe TCP port 9876 concurrently on online peers so we know which ones run KVM Receiver
    let handles: Vec<_> = raw_peers
        .into_iter()
        .map(|(name, os, online, ip)| {
            thread::spawn(move || {
                let kvm_ready = if online {
                    probe_kvm_receiver_port(&ip, 220)
                } else {
                    false
                };
                PeerInfo {
                    name,
                    os,
                    online,
                    ip,
                    kvm_ready,
                }
            })
        })
        .collect();

    let mut list: Vec<PeerInfo> = handles
        .into_iter()
        .filter_map(|h| h.join().ok())
        .collect();

    // Sort: KVM-ready first, then online, then alphabetical by hostname
    list.sort_by(|a, b| {
        b.kvm_ready
            .cmp(&a.kvm_ready)
            .then_with(|| b.online.cmp(&a.online))
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    if let Ok(mut guard) = peers_store().lock() {
        *guard = list.clone();
    }
    list
}

fn start_peer_discovery_worker() {
    thread::spawn(|| loop {
        refresh_tailscale_peers();
        let last_gui = LAST_GUI_ACTIVITY_MS.load(Ordering::Relaxed);
        let gui_recently_active =
            last_gui > 0 && crate::monotonic_ms().saturating_sub(last_gui) < 20_000;
        let interval = if gui_recently_active {
            Duration::from_secs(3)
        } else {
            Duration::from_secs(20)
        };
        thread::sleep(interval);
    });
}

fn takeover_or_bind(port: u16) -> Option<TcpListener> {
    let bind_addr = format!("0.0.0.0:{}", port);
    if let Ok(listener) = TcpListener::bind(&bind_addr) {
        return Some(listener);
    }

    // Politely tell any stale background host instance on this port to quit cleanly
    let local_addr: SocketAddr = format!("127.0.0.1:{}", port).parse().ok()?;
    if let Ok(mut stream) = TcpStream::connect_timeout(&local_addr, Duration::from_millis(250)) {
        let req = "POST /api/quit HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let _ = stream.write_all(req.as_bytes());
        let _ = stream.flush();
        let mut sink = [0u8; 256];
        let _ = stream.read(&mut sink);
    }

    for _ in 0..12 {
        thread::sleep(Duration::from_millis(100));
        if let Ok(listener) = TcpListener::bind(&bind_addr) {
            return Some(listener);
        }
    }
    None
}

pub fn start_gui_server(port: u16, auto_open_browser: bool) {
    start_peer_discovery_worker();

    thread::spawn(move || {
        let Some(listener) = takeover_or_bind(port) else {
            eprintln!("[!] Could not bind Web GUI port {} (already in use)", port);
            return;
        };
        let url = format!("http://localhost:{}", port);
        println!("[*] KVM Visual Web Management Dashboard active at {}", url);
        if auto_open_browser {
            open_browser(&url);
        }

        for stream in listener.incoming().flatten() {
            thread::spawn(move || handle_http_client(stream));
        }
    });
}

pub fn build_status_json() -> String {
    let slots = get_all_slots();
    let right_conn = crate::SLOT_CONNECTED[0].load(Ordering::Relaxed);
    let left_conn = crate::SLOT_CONNECTED[1].load(Ordering::Relaxed);
    let up_conn = crate::SLOT_CONNECTED[2].load(Ordering::Relaxed);
    let down_conn = crate::SLOT_CONNECTED[3].load(Ordering::Relaxed);

    let active_remote = if crate::IS_REMOTE.load(Ordering::Relaxed) {
        let idx = crate::ACTIVE_REMOTE_SLOT.load(Ordering::Relaxed) as usize;
        if idx < 4 {
            Some(index_to_slot_name(idx))
        } else {
            None
        }
    } else {
        None
    };

    let peers = peers_store()
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();

    let peers_json: Vec<serde_json::Value> = peers
        .into_iter()
        .map(|p| {
            json!({
                "name": p.name,
                "os": p.os,
                "online": p.online,
                "ip": p.ip,
                "kvm_ready": p.kvm_ready,
            })
        })
        .collect();

    let filters = crate::get_edge_filters();
    let monitors = crate::detected_monitors()
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default();

    let payload = json!({
        "host_os": std::env::consts::OS,
        "slots": {
            "right": slots[0].clone().unwrap_or_default(),
            "left": slots[1].clone().unwrap_or_default(),
            "up": slots[2].clone().unwrap_or_default(),
            "down": slots[3].clone().unwrap_or_default(),
        },
        "connected": {
            "right": right_conn,
            "left": left_conn,
            "up": up_conn,
            "down": down_conn,
        },
        "edge_filters": {
            "right": filters[0],
            "left": filters[1],
            "up": filters[2],
            "down": filters[3],
        },
        "host_monitors": monitors,
        "rtt_ms": {
            "right": crate::get_slot_rtt_ms(0),
            "left": crate::get_slot_rtt_ms(1),
            "up": crate::get_slot_rtt_ms(2),
            "down": crate::get_slot_rtt_ms(3),
        },
        "active_remote_slot": active_remote,
        "peers": peers_json,
        "recording": crate::build_recording_status_json(),
    });

    payload.to_string()
}

fn normalize_slot_ip(raw: Option<&str>) -> Option<String> {
    let s = raw?.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("none") || s.eq_ignore_ascii_case("null") {
        None
    } else {
        Some(s.to_string())
    }
}

fn percent_decode(input: &str) -> String {
    let mut out = Vec::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(hex_str) = std::str::from_utf8(&bytes[i + 1..i + 3]) {
                if let Ok(val) = u8::from_str_radix(hex_str, 16) {
                    out.push(val);
                    i += 3;
                    continue;
                }
            }
        } else if bytes[i] == b'+' {
            out.push(b' ');
            i += 1;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn write_http_response(stream: &mut TcpStream, status: &str, content_type: &str, body: &[u8]) {
    let header = format!(
        "HTTP/1.1 {}\r\nContent-Type: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nCache-Control: no-store, no-cache, must-revalidate\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        status,
        content_type,
        body.len()
    );
    let _ = stream.write_all(header.as_bytes());
    if !body.is_empty() {
        let _ = stream.write_all(body);
    }
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Write);
}

pub fn handle_http_client(stream: TcpStream) {
    let _ = stream.set_nodelay(true);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(20)));

    let mut reader = BufReader::new(stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).unwrap_or(0) == 0 {
        return;
    }

    // Drain ALL HTTP request headers cleanly so Winsock never resets the TCP socket!
    let mut content_length: usize = 0;
    for _ in 0..64 {
        let mut hdr = String::new();
        match reader.read_line(&mut hdr) {
            Ok(0) => break,
            Ok(_) => {
                let t = hdr.trim_end_matches(['\r', '\n']);
                if t.is_empty() {
                    break;
                }
                if let Some((k, v)) = t.split_once(':') {
                    if k.trim().eq_ignore_ascii_case("content-length") {
                        content_length = v.trim().parse::<usize>().unwrap_or(0).min(65_536);
                    }
                }
            }
            Err(_) => break,
        }
    }

    let mut body_bytes = vec![0u8; content_length];
    if content_length > 0 && reader.read_exact(&mut body_bytes).is_err() {
        return;
    }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0];
    let full_path = parts[1];
    let (path_only, query_opt) = match full_path.split_once('?') {
        Some((p, q)) => (p, Some(q)),
        None => (full_path, None),
    };

    if method == "OPTIONS" {
        write_http_response(reader.get_mut(), "204 No Content", "text/plain", b"");
        return;
    }

    if (path_only == "/" || path_only == "/index.html") && method == "GET" {
        mark_gui_active();
        let html = include_str!("../gui_static/index.html");
        write_http_response(
            reader.get_mut(),
            "200 OK",
            "text/html; charset=UTF-8",
            html.as_bytes(),
        );
        return;
    }

    if path_only == "/api/status" && method == "GET" {
        mark_gui_active();
        let json_str = build_status_json();
        write_http_response(
            reader.get_mut(),
            "200 OK",
            "application/json; charset=UTF-8",
            json_str.as_bytes(),
        );
        return;
    }

    if path_only == "/api/edge-filter" && method == "POST" {
        mark_gui_active();
        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&body_bytes) {
            if let Some(slot_str) = val.get("slot").and_then(|v| v.as_str()) {
                if let Some(slot_idx) = slot_name_to_index(slot_str) {
                    let mon_idx = val.get("monitor").and_then(|v| v.as_u64()).unwrap_or(255) as u8;
                    crate::set_edge_filter(slot_idx, mon_idx);
                    let current = get_all_slots();
                    crate::apply_slot_layout(current, true);
                    let json_str = build_status_json();
                    write_http_response(
                        reader.get_mut(),
                        "200 OK",
                        "application/json; charset=UTF-8",
                        json_str.as_bytes(),
                    );
                    return;
                }
            }
        }
        write_http_response(
            reader.get_mut(),
            "400 Bad Request",
            "application/json",
            br#"{"error":"invalid edge-filter json"}"#,
        );
        return;
    }

    if path_only == "/api/layout" && method == "POST" {
        mark_gui_active();
        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&body_bytes) {
            let right = normalize_slot_ip(val.get("right").and_then(|v| v.as_str()));
            let left = normalize_slot_ip(val.get("left").and_then(|v| v.as_str()));
            let up = normalize_slot_ip(val.get("up").and_then(|v| v.as_str()));
            let down = normalize_slot_ip(val.get("down").and_then(|v| v.as_str()));

            crate::apply_slot_layout([right, left, up, down], true);
            let json_str = build_status_json();
            write_http_response(
                reader.get_mut(),
                "200 OK",
                "application/json; charset=UTF-8",
                json_str.as_bytes(),
            );
            return;
        }
        write_http_response(
            reader.get_mut(),
            "400 Bad Request",
            "application/json",
            br#"{"error":"invalid json"}"#,
        );
        return;
    }

    if path_only == "/api/assign" && method == "POST" {
        mark_gui_active();
        if let Some(query) = query_opt {
            let mut slot = String::new();
            let mut ip = String::new();
            for pair in query.split('&') {
                if let Some(val) = pair.strip_prefix("slot=") {
                    slot = percent_decode(val);
                } else if let Some(val) = pair.strip_prefix("ip=") {
                    ip = percent_decode(val);
                }
            }
            if let Some(slot_idx) = slot_name_to_index(&slot) {
                let target = normalize_slot_ip(Some(&ip));
                let mut next_slots = get_all_slots();
                if let Some(ref assigned_ip) = target {
                    // Deduplicate: clear any previous slot holding this exact IP
                    for existing in &mut next_slots {
                        if existing.as_deref() == Some(assigned_ip.as_str()) {
                            *existing = None;
                        }
                    }
                }
                next_slots[slot_idx] = target;
                crate::apply_slot_layout(next_slots, true);
            }
        }
        let json_str = build_status_json();
        write_http_response(
            reader.get_mut(),
            "200 OK",
            "application/json; charset=UTF-8",
            json_str.as_bytes(),
        );
        return;
    }

    if path_only == "/api/record/status" && method == "GET" {
        mark_gui_active();
        let json_str = crate::build_recording_status_json().to_string();
        write_http_response(
            reader.get_mut(),
            "200 OK",
            "application/json; charset=UTF-8",
            json_str.as_bytes(),
        );
        return;
    }

    if path_only == "/api/record/start" && method == "POST" {
        mark_gui_active();
        let val = serde_json::from_slice::<serde_json::Value>(&body_bytes).unwrap_or_else(|_| json!({}));
        let slot_opt = val
            .get("slot")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());
        let auto_export = val
            .get("auto_export")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        match crate::start_dual_recording(slot_opt.as_deref(), auto_export) {
            Ok(rec_json) => {
                let body = json!({ "ok": true, "recording": rec_json }).to_string();
                write_http_response(
                    reader.get_mut(),
                    "200 OK",
                    "application/json; charset=UTF-8",
                    body.as_bytes(),
                );
            }
            Err(err) => {
                let body = json!({ "ok": false, "error": err }).to_string();
                write_http_response(
                    reader.get_mut(),
                    "400 Bad Request",
                    "application/json; charset=UTF-8",
                    body.as_bytes(),
                );
            }
        }
        return;
    }

    if path_only == "/api/record/stop" && method == "POST" {
        mark_gui_active();
        match crate::stop_dual_recording() {
            Ok(rec_json) => {
                let body = json!({ "ok": true, "recording": rec_json }).to_string();
                write_http_response(
                    reader.get_mut(),
                    "200 OK",
                    "application/json; charset=UTF-8",
                    body.as_bytes(),
                );
            }
            Err(err) => {
                let body = json!({ "ok": false, "error": err }).to_string();
                write_http_response(
                    reader.get_mut(),
                    "400 Bad Request",
                    "application/json; charset=UTF-8",
                    body.as_bytes(),
                );
            }
        }
        return;
    }

    if path_only == "/api/record/export" && method == "POST" {
        mark_gui_active();
        let val = serde_json::from_slice::<serde_json::Value>(&body_bytes).unwrap_or_else(|_| json!({}));
        let fname_opt = val
            .get("filename")
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        match crate::export_latest_recording(fname_opt.as_deref()) {
            Ok(dest) => {
                let body = json!({
                    "ok": true,
                    "exported": dest.to_string_lossy(),
                    "recording": crate::build_recording_status_json(),
                })
                .to_string();
                write_http_response(
                    reader.get_mut(),
                    "200 OK",
                    "application/json; charset=UTF-8",
                    body.as_bytes(),
                );
            }
            Err(err) => {
                let body = json!({ "ok": false, "error": err }).to_string();
                write_http_response(
                    reader.get_mut(),
                    "400 Bad Request",
                    "application/json; charset=UTF-8",
                    body.as_bytes(),
                );
            }
        }
        return;
    }

    if path_only == "/api/record/file" && method == "GET" {
        mark_gui_active();
        let mut raw_session = String::new();
        let mut raw_name = String::new();
        if let Some(query) = query_opt {
            for pair in query.split('&') {
                if let Some(val) = pair.strip_prefix("session=") {
                    raw_session = percent_decode(val);
                } else if let Some(val) = pair.strip_prefix("name=") {
                    raw_name = percent_decode(val);
                }
            }
        }
        let safe_session = crate::screen_record::sanitize_token(&raw_session, "");
        let safe_name = crate::screen_record::sanitize_filename(&raw_name, "");
        if safe_session.is_empty() || safe_name.is_empty() {
            write_http_response(
                reader.get_mut(),
                "400 Bad Request",
                "application/json",
                br#"{"error":"invalid recording file params"}"#,
            );
            return;
        }

        let base_dir = crate::screen_record::recordings_base_dir().join(&safe_session);
        let file_path = base_dir.join(&safe_name);
        if !file_path.is_file() {
            write_http_response(
                reader.get_mut(),
                "404 Not Found",
                "application/json",
                br#"{"error":"recording file not found"}"#,
            );
            return;
        }

        let content_type = match file_path.extension().and_then(|e| e.to_str()).unwrap_or("") {
            "webp" => "image/webp",
            "gif" => "image/gif",
            "mp4" => "video/mp4",
            "avi" => "video/x-msvideo",
            "mov" => "video/quicktime",
            _ => "application/octet-stream",
        };
        match std::fs::read(&file_path) {
            Ok(bytes) => {
                write_http_response(reader.get_mut(), "200 OK", content_type, &bytes);
            }
            Err(_) => {
                write_http_response(
                    reader.get_mut(),
                    "500 Internal Server Error",
                    "application/json",
                    br#"{"error":"failed reading recording file"}"#,
                );
            }
        }
        return;
    }

    if path_only == "/api/lock" && method == "POST" {
        mark_gui_active();
        crate::broadcast_sys_lock();
        write_http_response(
            reader.get_mut(),
            "200 OK",
            "application/json",
            br#"{"status":"locked"}"#,
        );
        return;
    }

    if path_only == "/api/quit" && method == "POST" {
        write_http_response(
            reader.get_mut(),
            "200 OK",
            "application/json",
            br#"{"status":"quitting"}"#,
        );
        thread::spawn(|| {
            thread::sleep(Duration::from_millis(80));
            crate::shutdown_and_exit();
        });
        return;
    }

    write_http_response(
        reader.get_mut(),
        "404 Not Found",
        "application/json",
        br#"{"error":"not found"}"#,
    );
}

fn open_browser(url: &str) {
    let u = url.to_string();
    thread::spawn(move || {
        thread::sleep(Duration::from_millis(350));
        #[cfg(target_os = "windows")]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            let mut cmd = std::process::Command::new("cmd");
            cmd.args(["/C", "start", "", &u]);
            cmd.creation_flags(CREATE_NO_WINDOW);
            let _ = cmd.spawn();
        }
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("open").arg(&u).spawn();
        }
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        {
            let _ = std::process::Command::new("xdg-open").arg(&u).spawn();
        }
    });
}
