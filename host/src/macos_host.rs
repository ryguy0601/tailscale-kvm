//! Native macOS Host Backend (CoreGraphics CGEventTap capture & suppression)

use std::ffi::c_void;
use std::io::{Cursor, Write};
use std::process::Command;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use crate::{
    monotonic_ms, send_to_active_remote, switch_to_local, switch_to_remote_slot,
    ACTIVE_REMOTE_SLOT, HOST_MAX_X, HOST_MAX_Y, HOST_MIN_X, HOST_MIN_Y, IS_REMOTE,
    LAST_LOCAL_RETURN_MS, SLOT_CONNECTED, SUPPRESS_CLIPBOARD_SYNC,
};

#[repr(C)]
#[derive(Clone, Copy)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn CGMainDisplayID() -> u32;
    fn CGDisplayPixelsWide(display: u32) -> usize;
    fn CGDisplayPixelsHigh(display: u32) -> usize;
    fn CGWarpMouseCursorPosition(new_cursor_position: CGPoint) -> i32;
    fn CGAssociateMouseAndMouseCursorPosition(connected: bool) -> i32;
    fn CGEventGetLocation(event: *const c_void) -> CGPoint;
    fn CGEventGetIntegerValueField(event: *const c_void, field: u32) -> i64;
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: unsafe extern "C" fn(*mut c_void, u32, *mut c_void, *mut c_void) -> *mut c_void,
        user_info: *mut c_void,
    ) -> *mut c_void;
    fn CGEventTapEnable(tap: *mut c_void, enable: bool);
    fn CFMachPortCreateRunLoopSource(
        allocator: *const c_void,
        port: *mut c_void,
        order: isize,
    ) -> *mut c_void;
    fn CFRunLoopGetCurrent() -> *mut c_void;
    fn CFRunLoopAddSource(rl: *mut c_void, source: *mut c_void, mode: *const c_void);
    fn CFRunLoopRun();
    static kCFRunLoopCommonModes: *const c_void;
}

const K_CG_SESSION_EVENT_TAP: u32 = 1;
const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
const K_CG_EVENT_TAP_OPTION_DEFAULT: u32 = 0;

const K_CG_EVENT_LEFT_MOUSE_DOWN: u32 = 1;
const K_CG_EVENT_LEFT_MOUSE_UP: u32 = 2;
const K_CG_EVENT_RIGHT_MOUSE_DOWN: u32 = 3;
const K_CG_EVENT_RIGHT_MOUSE_UP: u32 = 4;
const K_CG_EVENT_MOUSE_MOVED: u32 = 5;
const K_CG_EVENT_LEFT_MOUSE_DRAGGED: u32 = 6;
const K_CG_EVENT_RIGHT_MOUSE_DRAGGED: u32 = 7;
const K_CG_EVENT_KEY_DOWN: u32 = 10;
const K_CG_EVENT_KEY_UP: u32 = 11;
const K_CG_EVENT_SCROLL_WHEEL: u32 = 22;
const K_CG_EVENT_OTHER_MOUSE_DOWN: u32 = 25;
const K_CG_EVENT_OTHER_MOUSE_UP: u32 = 26;
const K_CG_EVENT_OTHER_MOUSE_DRAGGED: u32 = 27;
const K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT: u32 = 0xFFFFFFFE;

const K_CG_MOUSE_EVENT_DELTA_X: u32 = 4;
const K_CG_MOUSE_EVENT_DELTA_Y: u32 = 5;
const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;
const K_CG_SCROLL_WHEEL_EVENT_DELTA_AXIS_1: u32 = 11;
const K_CG_SCROLL_WHEEL_EVENT_DELTA_AXIS_2: u32 = 12;

static mut EVENT_TAP_PORT: *mut c_void = std::ptr::null_mut();

pub fn update_macos_screen_bounds() {
    unsafe {
        let id = CGMainDisplayID();
        let w = (CGDisplayPixelsWide(id) as i32).max(1280);
        let h = (CGDisplayPixelsHigh(id) as i32).max(720);
        HOST_MIN_X.store(0, Ordering::SeqCst);
        HOST_MAX_X.store(w, Ordering::SeqCst);
        HOST_MIN_Y.store(0, Ordering::SeqCst);
        HOST_MAX_Y.store(h, Ordering::SeqCst);
    }
}

pub fn set_macos_remote_capture(remote: bool, warp_x: i32, warp_y: i32) {
    unsafe {
        CGAssociateMouseAndMouseCursorPosition(!remote);
        CGWarpMouseCursorPosition(CGPoint {
            x: warp_x as f64,
            y: warp_y as f64,
        });
    }
}

pub fn write_macos_clipboard_text(text: &str) {
    let t = text.to_string();
    thread::spawn(move || {
        if let Ok(mut child) = Command::new("pbcopy")
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

fn start_macos_clipboard_sync() {
    thread::spawn(|| {
        let mut last_clip = String::new();
        loop {
            thread::sleep(Duration::from_millis(650));
            if SUPPRESS_CLIPBOARD_SYNC.load(Ordering::SeqCst) || !crate::any_slot_connected() {
                continue;
            }
            if let Ok(out) = Command::new("pbpaste").output() {
                if out.status.success() {
                    let text = String::from_utf8_lossy(&out.stdout).to_string();
                    if !text.is_empty() && text.len() <= 1_048_576 && text != last_clip {
                        last_clip = text.clone();
                        crate::broadcast_clipboard_text(&text);
                    }
                }
            }
        }
    });
}

fn macos_keycode_to_evdev(kc: u16) -> Option<u16> {
    match kc {
        0 => Some(30),   // A
        1 => Some(31),   // S
        2 => Some(32),   // D
        3 => Some(33),   // F
        4 => Some(35),   // H
        5 => Some(34),   // G
        6 => Some(44),   // Z
        7 => Some(45),   // X
        8 => Some(46),   // C
        9 => Some(47),   // V
        11 => Some(48),  // B
        12 => Some(16),  // Q
        13 => Some(17),  // W
        14 => Some(18),  // E
        15 => Some(19),  // R
        16 => Some(21),  // Y
        17 => Some(20),  // T
        18 => Some(2),   // 1
        19 => Some(3),   // 2
        20 => Some(4),   // 3
        21 => Some(5),   // 4
        22 => Some(7),   // 6
        23 => Some(6),   // 5
        24 => Some(13),  // =
        25 => Some(10),  // 9
        26 => Some(8),   // 7
        27 => Some(12),  // -
        28 => Some(9),   // 8
        29 => Some(11),  // 0
        30 => Some(27),  // ]
        31 => Some(24),  // O
        32 => Some(22),  // U
        33 => Some(26),  // [
        34 => Some(23),  // I
        35 => Some(25),  // P
        36 => Some(28),  // Return
        37 => Some(38),  // L
        38 => Some(36),  // J
        39 => Some(40),  // '
        40 => Some(37),  // K
        41 => Some(39),  // ;
        42 => Some(43),  // \
        43 => Some(51),  // ,
        44 => Some(53),  // /
        45 => Some(49),  // N
        46 => Some(50),  // M
        47 => Some(52),  // .
        48 => Some(15),  // Tab
        49 => Some(57),  // Space
        50 => Some(41),  // `
        51 => Some(14),  // Backspace
        53 => Some(1),   // Escape
        54 => Some(126), // Right Command
        55 => Some(125), // Left Command
        56 => Some(42),  // Left Shift
        57 => Some(58),  // CapsLock
        58 => Some(56),  // Left Option
        59 => Some(29),  // Left Control
        60 => Some(54),  // Right Shift
        61 => Some(100), // Right Option
        62 => Some(97),  // Right Control
        96 => Some(63),  // F5
        97 => Some(64),  // F6
        98 => Some(65),  // F7
        99 => Some(61),  // F3
        100 => Some(66), // F8
        101 => Some(67), // F9
        103 => Some(87), // F11
        109 => Some(68), // F10
        111 => Some(88), // F12
        115 => Some(102),// Home
        116 => Some(104),// PageUp
        117 => Some(111),// Delete
        118 => Some(62), // F4
        119 => Some(107),// End
        120 => Some(60), // F2
        121 => Some(109),// PageDown
        122 => Some(59), // F1
        123 => Some(105),// Left
        124 => Some(106),// Right
        125 => Some(108),// Down
        126 => Some(103),// Up
        _ => None,
    }
}

unsafe extern "C" fn macos_tap_callback(
    _proxy: *mut c_void,
    event_type: u32,
    event: *mut c_void,
    _user_info: *mut c_void,
) -> *mut c_void {
    if event_type == K_CG_EVENT_TAP_DISABLED_BY_TIMEOUT {
        if !EVENT_TAP_PORT.is_null() {
            CGEventTapEnable(EVENT_TAP_PORT, true);
        }
        return event;
    }

    let remote = IS_REMOTE.load(Ordering::SeqCst);

    if !remote {
        if event_type == K_CG_EVENT_MOUSE_MOVED
            || event_type == K_CG_EVENT_LEFT_MOUSE_DRAGGED
            || event_type == K_CG_EVENT_RIGHT_MOUSE_DRAGGED
        {
            if monotonic_ms().saturating_sub(LAST_LOCAL_RETURN_MS.load(Ordering::Relaxed)) >= 150 {
                let pt = CGEventGetLocation(event);
                let x = pt.x.round() as i32;
                let y = pt.y.round() as i32;
                let w = HOST_MAX_X.load(Ordering::Relaxed).max(1);
                let h = HOST_MAX_Y.load(Ordering::Relaxed).max(1);

                let right_ok = SLOT_CONNECTED[0].load(Ordering::Relaxed);
                let left_ok = SLOT_CONNECTED[1].load(Ordering::Relaxed);
                let up_ok = SLOT_CONNECTED[2].load(Ordering::Relaxed);
                let down_ok = SLOT_CONNECTED[3].load(Ordering::Relaxed);

                if right_ok && x >= w - 3 && y > 50 && y < h - 50 {
                    switch_to_remote_slot(0, false, y, h);
                    return std::ptr::null_mut();
                } else if left_ok && x <= 3 && y > 50 && y < h - 50 {
                    switch_to_remote_slot(1, false, y, h);
                    return std::ptr::null_mut();
                } else if up_ok && y <= 3 && x > 50 && x < w - 50 {
                    switch_to_remote_slot(2, false, x, w);
                    return std::ptr::null_mut();
                } else if down_ok && y >= h - 3 && x > 50 && x < w - 50 {
                    switch_to_remote_slot(3, false, x, w);
                    return std::ptr::null_mut();
                }
            }
        }
        return event;
    }

    // Remote mode: forward and suppress locally
    match event_type {
        K_CG_EVENT_MOUSE_MOVED
        | K_CG_EVENT_LEFT_MOUSE_DRAGGED
        | K_CG_EVENT_RIGHT_MOUSE_DRAGGED
        | K_CG_EVENT_OTHER_MOUSE_DRAGGED => {
            let dx = CGEventGetIntegerValueField(event, K_CG_MOUSE_EVENT_DELTA_X) as i32;
            let dy = CGEventGetIntegerValueField(event, K_CG_MOUSE_EVENT_DELTA_Y) as i32;
            if dx != 0 || dy != 0 {
                let mut buf = [0u8; 32];
                let mut cur = Cursor::new(&mut buf[..]);
                if writeln!(cur, "M {} {}", dx, dy).is_ok() {
                    let len = cur.position() as usize;
                    send_to_active_remote(&buf[..len]);
                }
            }
            return std::ptr::null_mut();
        }
        K_CG_EVENT_LEFT_MOUSE_DOWN => {
            send_to_active_remote(b"C left down\n");
            return std::ptr::null_mut();
        }
        K_CG_EVENT_LEFT_MOUSE_UP => {
            send_to_active_remote(b"C left up\n");
            return std::ptr::null_mut();
        }
        K_CG_EVENT_RIGHT_MOUSE_DOWN => {
            send_to_active_remote(b"C right down\n");
            return std::ptr::null_mut();
        }
        K_CG_EVENT_RIGHT_MOUSE_UP => {
            send_to_active_remote(b"C right up\n");
            return std::ptr::null_mut();
        }
        K_CG_EVENT_OTHER_MOUSE_DOWN => {
            send_to_active_remote(b"C middle down\n");
            return std::ptr::null_mut();
        }
        K_CG_EVENT_OTHER_MOUSE_UP => {
            send_to_active_remote(b"C middle up\n");
            return std::ptr::null_mut();
        }
        K_CG_EVENT_SCROLL_WHEEL => {
            let dy = CGEventGetIntegerValueField(event, K_CG_SCROLL_WHEEL_EVENT_DELTA_AXIS_1) as i32;
            let dx = CGEventGetIntegerValueField(event, K_CG_SCROLL_WHEEL_EVENT_DELTA_AXIS_2) as i32;
            if dx != 0 || dy != 0 {
                let pkt = format!("S {} {}\n", dx.signum(), dy.signum());
                send_to_active_remote(pkt.as_bytes());
            }
            return std::ptr::null_mut();
        }
        K_CG_EVENT_KEY_DOWN | K_CG_EVENT_KEY_UP => {
            let is_down = event_type == K_CG_EVENT_KEY_DOWN;
            let kc = CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) as u16;
            if kc == 111 && is_down {
                // F12 emergency release
                let slot = ACTIVE_REMOTE_SLOT.load(Ordering::Relaxed) as usize;
                switch_to_local(slot.min(3), None, None);
                return std::ptr::null_mut();
            }
            if let Some(evdev_code) = macos_keycode_to_evdev(kc) {
                let act = if is_down { "down" } else { "up" };
                let pkt = format!("K {} {}\n", evdev_code, act);
                send_to_active_remote(pkt.as_bytes());
            }
            return std::ptr::null_mut();
        }
        _ => {}
    }

    event
}

pub fn run_macos_host_loop() {
    update_macos_screen_bounds();
    start_macos_clipboard_sync();

    unsafe {
        let mask: u64 = (1 << K_CG_EVENT_LEFT_MOUSE_DOWN)
            | (1 << K_CG_EVENT_LEFT_MOUSE_UP)
            | (1 << K_CG_EVENT_RIGHT_MOUSE_DOWN)
            | (1 << K_CG_EVENT_RIGHT_MOUSE_UP)
            | (1 << K_CG_EVENT_MOUSE_MOVED)
            | (1 << K_CG_EVENT_LEFT_MOUSE_DRAGGED)
            | (1 << K_CG_EVENT_RIGHT_MOUSE_DRAGGED)
            | (1 << K_CG_EVENT_KEY_DOWN)
            | (1 << K_CG_EVENT_KEY_UP)
            | (1 << K_CG_EVENT_SCROLL_WHEEL)
            | (1 << K_CG_EVENT_OTHER_MOUSE_DOWN)
            | (1 << K_CG_EVENT_OTHER_MOUSE_UP)
            | (1 << K_CG_EVENT_OTHER_MOUSE_DRAGGED);

        let tap = CGEventTapCreate(
            K_CG_SESSION_EVENT_TAP,
            K_CG_HEAD_INSERT_EVENT_TAP,
            K_CG_EVENT_TAP_OPTION_DEFAULT,
            mask,
            macos_tap_callback,
            std::ptr::null_mut(),
        );

        if tap.is_null() {
            eprintln!(
                "[!] macOS Accessibility permission required for KVM input capture (System Settings -> Privacy & Security -> Accessibility)."
            );
            loop {
                thread::sleep(Duration::from_secs(3600));
            }
        }

        EVENT_TAP_PORT = tap;
        let source = CFMachPortCreateRunLoopSource(std::ptr::null(), tap, 0);
        let rl = CFRunLoopGetCurrent();
        CFRunLoopAddSource(rl, source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap, true);
        CFRunLoopRun();
    }
}
