#[cfg(target_os = "linux")]
use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
#[cfg(target_os = "linux")]
use evdev::{AttributeSet, EventType, InputEvent, Key, RelativeAxisType};

pub struct InputAdapter {
    #[cfg(target_os = "linux")]
    device: VirtualDevice,
    #[cfg(target_os = "macos")]
    left_down: bool,
    #[cfg(target_os = "macos")]
    right_down: bool,
}

impl InputAdapter {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        #[cfg(target_os = "linux")]
        {
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
                .name("Tailscale-KVM-Universal-Device")
                .with_keys(&key_set)?
                .with_relative_axes(&rel_set)?
                .build()?;

            Ok(Self {
                device: virtual_device,
            })
        }

        #[cfg(windows)]
        {
            Ok(Self {})
        }

        #[cfg(target_os = "macos")]
        {
            Ok(Self {
                left_down: false,
                right_down: false,
            })
        }

        #[cfg(not(any(target_os = "linux", windows, target_os = "macos")))]
        {
            Ok(Self {})
        }
    }

    pub fn warp_cursor(&mut self, x: i32, y: i32, min_x: i32, min_y: i32) {
        #[cfg(target_os = "linux")]
        {
            if warp_hyprland_cursor(x, y) {
                return;
            }
            if std::env::var("DISPLAY").is_ok() {
                if std::process::Command::new("xdotool")
                    .args(["mousemove", &x.to_string(), &y.to_string()])
                    .status()
                    .map(|s| s.success())
                    .unwrap_or(false)
                {
                    return;
                }
            }
            // Fallback for generic Wayland compositors without Hyprland/X11:
            // Home to top-left corner and step to relative target offset
            self.emit_mouse_move(-12000, -12000);
            let rel_x = (x - min_x).max(0);
            let rel_y = (y - min_y).max(0);
            if rel_x > 0 || rel_y > 0 {
                self.emit_mouse_move(rel_x, rel_y);
            }
        }

        #[cfg(windows)]
        {
            let _ = (min_x, min_y);
            unsafe {
                windows_sys::Win32::UI::WindowsAndMessaging::SetCursorPos(x, y);
            }
        }

        #[cfg(target_os = "macos")]
        {
            let _ = (min_x, min_y);
            macos_ffi::warp_cursor(x, y);
        }
    }

    pub fn emit_mouse_move(&mut self, dx: i32, dy: i32) {
        #[cfg(target_os = "linux")]
        {
            let ev_x = InputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_X.0, dx);
            let ev_y = InputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_Y.0, dy);
            let syn = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0);
            let _ = self.device.emit(&[ev_x, ev_y, syn]);
        }

        #[cfg(windows)]
        {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx,
                        dy,
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_MOVE,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            unsafe {
                SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
            }
        }

        #[cfg(target_os = "macos")]
        {
            macos_ffi::move_mouse_relative(dx, dy, self.left_down, self.right_down);
        }
    }

    pub fn emit_mouse_wheel(&mut self, dx: i32, dy: i32) {
        #[cfg(target_os = "linux")]
        {
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
                let _ = self.device.emit(&events);
            }
        }

        #[cfg(windows)]
        {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
            if dy != 0 {
                let input = INPUT {
                    r#type: INPUT_MOUSE,
                    Anonymous: INPUT_0 {
                        mi: MOUSEINPUT {
                            dx: 0,
                            dy: 0,
                            mouseData: (dy * 120) as u32,
                            dwFlags: MOUSEEVENTF_WHEEL,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                };
                unsafe {
                    SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
                }
            }
            if dx != 0 {
                let input = INPUT {
                    r#type: INPUT_MOUSE,
                    Anonymous: INPUT_0 {
                        mi: MOUSEINPUT {
                            dx: 0,
                            dy: 0,
                            mouseData: (dx * 120) as u32,
                            dwFlags: MOUSEEVENTF_HWHEEL,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                };
                unsafe {
                    SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
                }
            }
        }

        #[cfg(target_os = "macos")]
        {
            macos_ffi::scroll_wheel(dx, dy);
        }
    }

    pub fn emit_mouse_click(&mut self, btn: &str, is_down: bool) {
        #[cfg(target_os = "linux")]
        {
            let btn_code = match btn {
                "right" => Key::BTN_RIGHT.code(),
                "middle" => Key::BTN_MIDDLE.code(),
                _ => Key::BTN_LEFT.code(),
            };
            let act_val = if is_down { 1 } else { 0 };
            let ev = InputEvent::new(EventType::KEY, btn_code, act_val);
            let syn = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0);
            let _ = self.device.emit(&[ev, syn]);
        }

        #[cfg(windows)]
        {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
            let flag = match btn {
                "right" => {
                    if is_down {
                        MOUSEEVENTF_RIGHTDOWN
                    } else {
                        MOUSEEVENTF_RIGHTUP
                    }
                }
                "middle" => {
                    if is_down {
                        MOUSEEVENTF_MIDDLEDOWN
                    } else {
                        MOUSEEVENTF_MIDDLEUP
                    }
                }
                _ => {
                    if is_down {
                        MOUSEEVENTF_LEFTDOWN
                    } else {
                        MOUSEEVENTF_LEFTUP
                    }
                }
            };
            let input = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: 0,
                        dy: 0,
                        mouseData: 0,
                        dwFlags: flag,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            unsafe {
                SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
            }
        }

        #[cfg(target_os = "macos")]
        {
            match btn {
                "right" => self.right_down = is_down,
                "middle" => {}
                _ => self.left_down = is_down,
            }
            macos_ffi::mouse_click(btn, is_down);
        }
    }

    pub fn emit_key(&mut self, scancode: u16, is_down: bool) {
        #[cfg(target_os = "linux")]
        {
            let act_val = if is_down { 1 } else { 0 };
            let ev = InputEvent::new(EventType::KEY, scancode, act_val);
            let syn = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0);
            let _ = self.device.emit(&[ev, syn]);
        }

        #[cfg(windows)]
        {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
            let base_flags = if is_down { 0 } else { KEYEVENTF_KEYUP };
            let input = if let Some((vk, extended)) = evdev_to_windows_vk(scancode) {
                let ext_flag = if extended { KEYEVENTF_EXTENDEDKEY } else { 0 };
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: vk,
                            wScan: 0,
                            dwFlags: base_flags | ext_flag,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                }
            } else {
                INPUT {
                    r#type: INPUT_KEYBOARD,
                    Anonymous: INPUT_0 {
                        ki: KEYBDINPUT {
                            wVk: 0,
                            wScan: scancode,
                            dwFlags: base_flags | KEYEVENTF_SCANCODE,
                            time: 0,
                            dwExtraInfo: 0,
                        },
                    },
                }
            };
            unsafe {
                SendInput(1, &input, std::mem::size_of::<INPUT>() as i32);
            }
        }

        #[cfg(target_os = "macos")]
        {
            if let Some(mac_kc) = evdev_to_macos_keycode(scancode) {
                macos_ffi::key_event(mac_kc, is_down);
            }
        }
    }
}

#[cfg(target_os = "linux")]
pub fn resolve_hyprland_socket_path() -> Option<String> {
    let xdg = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/run/user/1000".to_string());
    if let Ok(sig) = std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
        let sig = sig.trim();
        if !sig.is_empty() {
            let p = format!("{}/hypr/{}/.socket.sock", xdg, sig);
            if std::path::Path::new(&p).exists() {
                return Some(p);
            }
        }
    }

    let hypr_dir = std::path::PathBuf::from(&xdg).join("hypr");
    let entries = std::fs::read_dir(hypr_dir).ok()?;
    let mut best: Option<(std::time::SystemTime, String)> = None;
    for entry in entries.flatten() {
        let sock = entry.path().join(".socket.sock");
        if sock.exists() {
            let mtime = entry
                .metadata()
                .and_then(|m| m.modified())
                .unwrap_or(std::time::UNIX_EPOCH);
            let sock_str = sock.to_string_lossy().into_owned();
            if best.as_ref().map(|(t, _)| mtime > *t).unwrap_or(true) {
                best = Some((mtime, sock_str));
            }
        }
    }
    best.map(|(_, p)| p)
}

#[cfg(target_os = "linux")]
fn warp_hyprland_cursor(x: i32, y: i32) -> bool {
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    let Some(sock_path) = resolve_hyprland_socket_path() else {
        return false;
    };
    if let Ok(mut stream) = UnixStream::connect(&sock_path) {
        let cmd = format!("dispatch movecursor {} {}", x, y);
        if stream.write_all(cmd.as_bytes()).is_ok() {
            return true;
        }
    }
    false
}

/// Inverse mapping from Linux evdev keycodes to Windows (VK_*, is_extended)
#[cfg(any(windows, test))]
pub fn evdev_to_windows_vk(code: u16) -> Option<(u16, bool)> {
    match code {
        1 => Some((0x1B, false)),   // KEY_ESC -> VK_ESCAPE
        2 => Some((0x31, false)),   // 1
        3 => Some((0x32, false)),   // 2
        4 => Some((0x33, false)),   // 3
        5 => Some((0x34, false)),   // 4
        6 => Some((0x35, false)),   // 5
        7 => Some((0x36, false)),   // 6
        8 => Some((0x37, false)),   // 7
        9 => Some((0x38, false)),   // 8
        10 => Some((0x39, false)),  // 9
        11 => Some((0x30, false)),  // 0
        12 => Some((0xBD, false)),  // -_
        13 => Some((0xBB, false)),  // =+
        14 => Some((0x08, false)),  // Backspace
        15 => Some((0x09, false)),  // Tab
        16 => Some((0x51, false)),  // Q
        17 => Some((0x57, false)),  // W
        18 => Some((0x45, false)),  // E
        19 => Some((0x52, false)),  // R
        20 => Some((0x54, false)),  // T
        21 => Some((0x59, false)),  // Y
        22 => Some((0x55, false)),  // U
        23 => Some((0x49, false)),  // I
        24 => Some((0x4F, false)),  // O
        25 => Some((0x50, false)),  // P
        26 => Some((0xDB, false)),  // [{
        27 => Some((0xDD, false)),  // ]}
        28 => Some((0x0D, false)),  // Enter
        29 => Some((0xA2, false)),  // Left Ctrl
        30 => Some((0x41, false)),  // A
        31 => Some((0x53, false)),  // S
        32 => Some((0x44, false)),  // D
        33 => Some((0x46, false)),  // F
        34 => Some((0x47, false)),  // G
        35 => Some((0x48, false)),  // H
        36 => Some((0x4A, false)),  // J
        37 => Some((0x4B, false)),  // K
        38 => Some((0x4C, false)),  // L
        39 => Some((0xBA, false)),  // ;:
        40 => Some((0xDE, false)),  // '"
        41 => Some((0xC0, false)),  // `~
        42 => Some((0xA0, false)),  // Left Shift
        43 => Some((0xDC, false)),  // \|
        44 => Some((0x5A, false)),  // Z
        45 => Some((0x58, false)),  // X
        46 => Some((0x43, false)),  // C
        47 => Some((0x56, false)),  // V
        48 => Some((0x42, false)),  // B
        49 => Some((0x4E, false)),  // N
        50 => Some((0x4D, false)),  // M
        51 => Some((0xBC, false)),  // ,<
        52 => Some((0xBE, false)),  // .>
        53 => Some((0xBF, false)),  // /?
        54 => Some((0xA1, false)),  // Right Shift
        55 => Some((0x6A, false)),  // Numpad *
        56 => Some((0xA4, false)),  // Left Alt
        57 => Some((0x20, false)),  // Space
        58 => Some((0x14, false)),  // CapsLock
        59 => Some((0x70, false)),  // F1
        60 => Some((0x71, false)),  // F2
        61 => Some((0x72, false)),  // F3
        62 => Some((0x73, false)),  // F4
        63 => Some((0x74, false)),  // F5
        64 => Some((0x75, false)),  // F6
        65 => Some((0x76, false)),  // F7
        66 => Some((0x77, false)),  // F8
        67 => Some((0x78, false)),  // F9
        68 => Some((0x79, false)),  // F10
        70 => Some((0x91, false)),  // ScrollLock
        71 => Some((0x67, false)),  // Numpad 7
        72 => Some((0x68, false)),  // Numpad 8
        73 => Some((0x69, false)),  // Numpad 9
        74 => Some((0x6D, false)),  // Numpad -
        75 => Some((0x64, false)),  // Numpad 4
        76 => Some((0x65, false)),  // Numpad 5
        77 => Some((0x66, false)),  // Numpad 6
        78 => Some((0x6B, false)),  // Numpad +
        79 => Some((0x61, false)),  // Numpad 1
        80 => Some((0x62, false)),  // Numpad 2
        81 => Some((0x63, false)),  // Numpad 3
        82 => Some((0x60, false)),  // Numpad 0
        83 => Some((0x6E, false)),  // Numpad .
        87 => Some((0x7A, false)),  // F11
        88 => Some((0x7B, false)),  // F12
        96 => Some((0x0D, true)),   // Numpad Enter
        97 => Some((0xA3, true)),   // Right Ctrl
        98 => Some((0x6F, true)),   // Numpad /
        99 => Some((0x2C, true)),   // PrintScreen
        100 => Some((0xA5, true)),  // Right Alt
        102 => Some((0x24, true)),  // Home
        103 => Some((0x26, true)),  // Up Arrow
        104 => Some((0x21, true)),  // PageUp
        105 => Some((0x25, true)),  // Left Arrow
        106 => Some((0x27, true)),  // Right Arrow
        107 => Some((0x23, true)),  // End
        108 => Some((0x28, true)),  // Down Arrow
        109 => Some((0x22, true)),  // PageDown
        110 => Some((0x2D, true)),  // Insert
        111 => Some((0x2E, true)),  // Delete
        119 => Some((0x13, false)), // Pause
        125 => Some((0x5B, true)),  // Left Win / Super
        126 => Some((0x5C, true)),  // Right Win / Super
        127 => Some((0x5D, true)),  // Menu / Apps
        _ => None,
    }
}

/// Mapping from Linux evdev keycodes to macOS CGKeyCode
#[cfg(any(target_os = "macos", test))]
pub fn evdev_to_macos_keycode(code: u16) -> Option<u16> {
    match code {
        1 => Some(53),   // Escape
        2 => Some(18),   // 1
        3 => Some(19),   // 2
        4 => Some(20),   // 3
        5 => Some(21),   // 4
        6 => Some(23),   // 5
        7 => Some(22),   // 6
        8 => Some(26),   // 7
        9 => Some(28),   // 8
        10 => Some(25),  // 9
        11 => Some(29),  // 0
        12 => Some(27),  // -
        13 => Some(24),  // =
        14 => Some(51),  // Backspace
        15 => Some(48),  // Tab
        16 => Some(12),  // Q
        17 => Some(13),  // W
        18 => Some(14),  // E
        19 => Some(15),  // R
        20 => Some(17),  // T
        21 => Some(16),  // Y
        22 => Some(32),  // U
        23 => Some(34),  // I
        24 => Some(31),  // O
        25 => Some(35),  // P
        26 => Some(33),  // [
        27 => Some(30),  // ]
        28 => Some(36),  // Return
        29 => Some(59),  // Left Control
        30 => Some(0),   // A
        31 => Some(1),   // S
        32 => Some(2),   // D
        33 => Some(3),   // F
        34 => Some(5),   // G
        35 => Some(4),   // H
        36 => Some(38),  // J
        37 => Some(40),  // K
        38 => Some(37),  // L
        39 => Some(41),  // ;
        40 => Some(39),  // '
        41 => Some(50),  // `
        42 => Some(56),  // Left Shift
        43 => Some(42),  // \
        44 => Some(6),   // Z
        45 => Some(7),   // X
        46 => Some(8),   // C
        47 => Some(9),   // V
        48 => Some(11),  // B
        49 => Some(45),  // N
        50 => Some(46),  // M
        51 => Some(43),  // ,
        52 => Some(47),  // .
        53 => Some(44),  // /
        54 => Some(60),  // Right Shift
        56 => Some(58),  // Left Option/Alt
        57 => Some(49),  // Space
        58 => Some(57),  // CapsLock
        59 => Some(122), // F1
        60 => Some(120), // F2
        61 => Some(99),  // F3
        62 => Some(118), // F4
        63 => Some(96),  // F5
        64 => Some(97),  // F6
        65 => Some(98),  // F7
        66 => Some(100), // F8
        67 => Some(101), // F9
        68 => Some(109), // F10
        87 => Some(103), // F11
        88 => Some(111), // F12
        97 => Some(62),  // Right Control
        100 => Some(61), // Right Option/Alt
        102 => Some(115),// Home
        103 => Some(126),// Up Arrow
        104 => Some(116),// PageUp
        105 => Some(123),// Left Arrow
        106 => Some(124),// Right Arrow
        107 => Some(119),// End
        108 => Some(125),// Down Arrow
        109 => Some(121),// PageDown
        111 => Some(117),// Forward Delete
        125 => Some(55), // Left Command (Meta/Super)
        126 => Some(54), // Right Command
        _ => None,
    }
}

#[cfg(target_os = "macos")]
pub mod macos_ffi {
    use std::ffi::c_void;

    #[repr(C)]
    #[derive(Clone, Copy)]
    pub struct CGPoint {
        pub x: f64,
        pub y: f64,
    }

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn CGMainDisplayID() -> u32;
        fn CGDisplayPixelsWide(display: u32) -> usize;
        fn CGDisplayPixelsHigh(display: u32) -> usize;
        fn CGEventCreate(source: *const c_void) -> *mut c_void;
        fn CGEventGetLocation(event: *const c_void) -> CGPoint;
        fn CGWarpMouseCursorPosition(new_cursor_position: CGPoint) -> i32;
        fn CGEventCreateMouseEvent(
            source: *const c_void,
            mouse_type: u32,
            mouse_cursor_position: CGPoint,
            mouse_button: u32,
        ) -> *mut c_void;
        fn CGEventCreateKeyboardEvent(
            source: *const c_void,
            virtual_key: u16,
            key_down: bool,
        ) -> *mut c_void;
        fn CGEventCreateScrollWheelEvent(
            source: *const c_void,
            units: u32,
            wheel_count: u32,
            wheel1: i32,
            wheel2: i32,
        ) -> *mut c_void;
        fn CGEventPost(tap: u32, event: *mut c_void);
        fn CFRelease(cf: *const c_void);
    }

    const K_CG_HID_EVENT_TAP: u32 = 0;
    const K_CG_EVENT_LEFT_MOUSE_DOWN: u32 = 1;
    const K_CG_EVENT_LEFT_MOUSE_UP: u32 = 2;
    const K_CG_EVENT_RIGHT_MOUSE_DOWN: u32 = 3;
    const K_CG_EVENT_RIGHT_MOUSE_UP: u32 = 4;
    const K_CG_EVENT_MOUSE_MOVED: u32 = 5;
    const K_CG_EVENT_LEFT_MOUSE_DRAGGED: u32 = 6;
    const K_CG_EVENT_RIGHT_MOUSE_DRAGGED: u32 = 7;
    const K_CG_EVENT_OTHER_MOUSE_DOWN: u32 = 25;
    const K_CG_EVENT_OTHER_MOUSE_UP: u32 = 26;

    pub fn display_bounds() -> (i32, i32, i32, i32) {
        unsafe {
            let id = CGMainDisplayID();
            let w = CGDisplayPixelsWide(id) as i32;
            let h = CGDisplayPixelsHigh(id) as i32;
            (0, w.max(1280), 0, h.max(720))
        }
    }

    pub fn cursor_pos() -> Option<(i32, i32)> {
        unsafe {
            let ev = CGEventCreate(std::ptr::null());
            if ev.is_null() {
                return None;
            }
            let pt = CGEventGetLocation(ev);
            CFRelease(ev);
            Some((pt.x.round() as i32, pt.y.round() as i32))
        }
    }

    pub fn warp_cursor(x: i32, y: i32) {
        unsafe {
            CGWarpMouseCursorPosition(CGPoint {
                x: x as f64,
                y: y as f64,
            });
        }
    }

    pub fn move_mouse_relative(dx: i32, dy: i32, left_down: bool, right_down: bool) {
        let (_, max_x, _, max_y) = display_bounds();
        let (cx, cy) = cursor_pos().unwrap_or((max_x / 2, max_y / 2));
        let nx = (cx + dx).clamp(0, max_x - 1);
        let ny = (cy + dy).clamp(0, max_y - 1);
        let pt = CGPoint {
            x: nx as f64,
            y: ny as f64,
        };
        let (ev_type, btn) = if left_down {
            (K_CG_EVENT_LEFT_MOUSE_DRAGGED, 0)
        } else if right_down {
            (K_CG_EVENT_RIGHT_MOUSE_DRAGGED, 1)
        } else {
            (K_CG_EVENT_MOUSE_MOVED, 0)
        };
        unsafe {
            let ev = CGEventCreateMouseEvent(std::ptr::null(), ev_type, pt, btn);
            if !ev.is_null() {
                CGEventPost(K_CG_HID_EVENT_TAP, ev);
                CFRelease(ev);
            }
        }
    }

    pub fn mouse_click(btn: &str, is_down: bool) {
        let (cx, cy) = cursor_pos().unwrap_or((500, 500));
        let pt = CGPoint {
            x: cx as f64,
            y: cy as f64,
        };
        let (ev_type, button_id) = match btn {
            "right" => (
                if is_down {
                    K_CG_EVENT_RIGHT_MOUSE_DOWN
                } else {
                    K_CG_EVENT_RIGHT_MOUSE_UP
                },
                1,
            ),
            "middle" => (
                if is_down {
                    K_CG_EVENT_OTHER_MOUSE_DOWN
                } else {
                    K_CG_EVENT_OTHER_MOUSE_UP
                },
                2,
            ),
            _ => (
                if is_down {
                    K_CG_EVENT_LEFT_MOUSE_DOWN
                } else {
                    K_CG_EVENT_LEFT_MOUSE_UP
                },
                0,
            ),
        };
        unsafe {
            let ev = CGEventCreateMouseEvent(std::ptr::null(), ev_type, pt, button_id);
            if !ev.is_null() {
                CGEventPost(K_CG_HID_EVENT_TAP, ev);
                CFRelease(ev);
            }
        }
    }

    pub fn scroll_wheel(dx: i32, dy: i32) {
        unsafe {
            let ev = CGEventCreateScrollWheelEvent(std::ptr::null(), 1, 2, dy * 3, dx * 3);
            if !ev.is_null() {
                CGEventPost(K_CG_HID_EVENT_TAP, ev);
                CFRelease(ev);
            }
        }
    }

    pub fn key_event(keycode: u16, is_down: bool) {
        unsafe {
            let ev = CGEventCreateKeyboardEvent(std::ptr::null(), keycode, is_down);
            if !ev.is_null() {
                CGEventPost(K_CG_HID_EVENT_TAP, ev);
                CFRelease(ev);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_evdev_to_windows_vk_navigation_and_letters() {
        assert_eq!(evdev_to_windows_vk(30), Some((0x41, false))); // A
        assert_eq!(evdev_to_windows_vk(103), Some((0x26, true))); // Up arrow (extended)
        assert_eq!(evdev_to_windows_vk(105), Some((0x25, true))); // Left arrow (extended)
        assert_eq!(evdev_to_windows_vk(106), Some((0x27, true))); // Right arrow (extended)
        assert_eq!(evdev_to_windows_vk(108), Some((0x28, true))); // Down arrow (extended)
        assert_eq!(evdev_to_windows_vk(125), Some((0x5B, true))); // Left Win
    }

    #[test]
    fn test_evdev_to_macos_keycode() {
        assert_eq!(evdev_to_macos_keycode(30), Some(0)); // A
        assert_eq!(evdev_to_macos_keycode(103), Some(126)); // Up arrow
        assert_eq!(evdev_to_macos_keycode(125), Some(55)); // Command
    }
}
