use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;

pub static GUI_ACTIVE_RIGHT: OnceLock<Mutex<Option<String>>> = OnceLock::new();
pub static GUI_ACTIVE_LEFT: OnceLock<Mutex<Option<String>>> = OnceLock::new();
pub static GUI_ACTIVE_UP: OnceLock<Mutex<Option<String>>> = OnceLock::new();
pub static GUI_ACTIVE_DOWN: OnceLock<Mutex<Option<String>>> = OnceLock::new();

pub fn get_slot(slot: &str) -> &'static Mutex<Option<String>> {
    match slot {
        "left" => GUI_ACTIVE_LEFT.get_or_init(|| Mutex::new(None)),
        "up" | "top" => GUI_ACTIVE_UP.get_or_init(|| Mutex::new(None)),
        "down" | "bottom" => GUI_ACTIVE_DOWN.get_or_init(|| Mutex::new(None)),
        _ => GUI_ACTIVE_RIGHT.get_or_init(|| Mutex::new(None)),
    }
}

pub fn set_slot_peer(slot: &str, peer_ip: Option<String>) {
    if let Ok(mut lock) = get_slot(slot).lock() {
        *lock = peer_ip;
    }
}

pub fn get_slot_peer(slot: &str) -> Option<String> {
    get_slot(slot).lock().ok()?.clone()
}

pub fn start_gui_server(port: u16) {
    thread::spawn(move || {
        let addr = format!("0.0.0.0:{}", port);
        let Ok(listener) = TcpListener::bind(&addr) else {
            return;
        };
        println!("[*] KVM Visual Web Management Dashboard active at http://localhost:{}", port);

        for stream in listener.incoming().flatten() {
            thread::spawn(|| handle_http_client(stream));
        }
    });
}

fn handle_http_client(mut stream: TcpStream) {
    let mut reader = BufReader::new(&stream);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }

    let parts: Vec<&str> = request_line.split_whitespace().collect();
    if parts.len() < 2 {
        return;
    }
    let method = parts[0];
    let path = parts[1];

    if path == "/" || path == "/index.html" {
        let html = include_str!("../gui_static/index.html");
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=UTF-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            html.len(),
            html
        );
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    if path == "/api/status" {
        let right = get_slot_peer("right").unwrap_or_default();
        let left = get_slot_peer("left").unwrap_or_default();
        let up = get_slot_peer("up").unwrap_or_default();
        let down = get_slot_peer("down").unwrap_or_default();

        let peers_json = get_tailscale_peers_json();
        let json = format!(
            r#"{{"slots":{{"right":"{}","left":"{}","up":"{}","down":"{}"}},"peers":{}}}"#,
            right, left, up, down, peers_json
        );

        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            json.len(),
            json
        );
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    if path.starts_with("/api/assign") && method == "POST" {
        // Parse simple URL query params: /api/assign?slot=right&ip=100.x.y.z
        if let Some(query) = path.split('?').nth(1) {
            let mut slot = String::new();
            let mut ip = String::new();
            for pair in query.split('&') {
                if let Some(val) = pair.strip_prefix("slot=") {
                    slot = val.to_string();
                } else if let Some(val) = pair.strip_prefix("ip=") {
                    ip = val.to_string();
                }
            }
            if !slot.is_empty() {
                let target = if ip.is_empty() || ip == "none" { None } else { Some(ip) };
                set_slot_peer(&slot, target);
            }
        }
        let body = r#"{"status":"ok"}"#;
        let resp = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        );
        let _ = stream.write_all(resp.as_bytes());
        return;
    }

    let not_found = "HTTP/1.1 404 NOT FOUND\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
    let _ = stream.write_all(not_found.as_bytes());
}

fn get_tailscale_peers_json() -> String {
    if let Ok(output) = std::process::Command::new("tailscale").args(["status", "--json"]).output() {
        if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&output.stdout) {
            if let Some(peers) = val.get("Peer").and_then(|p| p.as_object()) {
                let mut list = Vec::new();
                for (_, peer) in peers {
                    let name = peer.get("HostName").and_then(|h| h.as_str()).unwrap_or("Unknown");
                    let os = peer.get("OS").and_then(|o| o.as_str()).unwrap_or("unknown");
                    let online = peer.get("Online").and_then(|o| o.as_bool()).unwrap_or(false);
                    let ip = peer.get("TailscaleIPs")
                        .and_then(|ips| ips.as_array())
                        .and_then(|arr| arr.get(0))
                        .and_then(|i| i.as_str())
                        .unwrap_or("");
                    if !ip.is_empty() {
                        list.push(format!(
                            r#"{{"name":"{}","os":"{}","online":{},"ip":"{}"}}"#,
                            name, os, online, ip
                        ));
                    }
                }
                return format!("[{}]", list.join(","));
            }
        }
    }
    "[]".to_string()
}
