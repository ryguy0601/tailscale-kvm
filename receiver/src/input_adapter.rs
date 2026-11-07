#[cfg(unix)]
use evdev::uinput::{VirtualDevice, VirtualDeviceBuilder};
#[cfg(unix)]
use evdev::{AttributeSet, EventType, InputEvent, Key, RelativeAxisType};

pub struct InputAdapter {
    #[cfg(unix)]
    device: VirtualDevice,
}

impl InputAdapter {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        #[cfg(unix)]
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

            Ok(Self { device: virtual_device })
        }

        #[cfg(windows)]
        {
            Ok(Self {})
        }
    }

    pub fn emit_mouse_move(&mut self, dx: i32, dy: i32) {
        #[cfg(unix)]
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
    }

    pub fn emit_mouse_wheel(&mut self, _dx: i32, dy: i32) {
        #[cfg(unix)]
        let dx = _dx;
        #[cfg(unix)]
        {
            let mut events = Vec::with_capacity(3);
            if dy != 0 {
                events.push(InputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_WHEEL.0, dy));
            }
            if dx != 0 {
                events.push(InputEvent::new(EventType::RELATIVE, RelativeAxisType::REL_HWHEEL.0, dx));
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
        }
    }

    pub fn emit_mouse_click(&mut self, btn: &str, is_down: bool) {
        #[cfg(unix)]
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
                "right" => if is_down { MOUSEEVENTF_RIGHTDOWN } else { MOUSEEVENTF_RIGHTUP },
                "middle" => if is_down { MOUSEEVENTF_MIDDLEDOWN } else { MOUSEEVENTF_MIDDLEUP },
                _ => if is_down { MOUSEEVENTF_LEFTDOWN } else { MOUSEEVENTF_LEFTUP },
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
    }

    pub fn emit_key(&mut self, scancode: u16, is_down: bool) {
        #[cfg(unix)]
        {
            let act_val = if is_down { 1 } else { 0 };
            let ev = InputEvent::new(EventType::KEY, scancode, act_val);
            let syn = InputEvent::new(EventType::SYNCHRONIZATION, 0, 0);
            let _ = self.device.emit(&[ev, syn]);
        }

        #[cfg(windows)]
        {
            use windows_sys::Win32::UI::Input::KeyboardAndMouse::*;
            let flags = if is_down { 0 } else { KEYEVENTF_KEYUP };
            let input = INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: 0,
                        wScan: scancode,
                        dwFlags: flags | KEYEVENTF_SCANCODE,
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
}
