import socket
import json
import ctypes
import os
import sys
import time
import threading
import pyperclip
from pynput import mouse, keyboard
from vk_to_evdev import translate_vk

# TARGET CONFIGURATION
CONFIG_FILE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "config.txt")

def get_target_ip():
    if len(sys.argv) > 1 and sys.argv[1].strip():
        return sys.argv[1].strip()
    
    env_ip = os.environ.get("OMARCHY_TAILSCALE_IP", "").strip()
    if env_ip and env_ip != "100.x.y.z":
        return env_ip
    
    if os.path.isfile(CONFIG_FILE):
        try:
            with open(CONFIG_FILE, "r") as f:
                for line in f:
                    line = line.strip()
                    if line and not line.startswith("#"):
                        if "=" in line:
                            k, v = line.split("=", 1)
                            if k.strip() == "OMARCHY_IP" and v.strip():
                                return v.strip()
                        else:
                            return line
        except Exception:
            pass

    return "100.99.142.70"

OMARCHY_TAILSCALE_IP = get_target_ip()
PORT = int(os.environ.get("OMARCHY_PORT", 9876))

user32 = ctypes.windll.user32
user32.SetProcessDPIAware()

screen_width = user32.GetSystemMetrics(0)
screen_height = user32.GetSystemMetrics(1)

is_remote = False
last_mouse_pos = None
last_clipboard_text = ""
sock = None
sock_lock = threading.Lock()
running = True

mouse_listener = None
keyboard_listener = None

def connect_to_omarchy():
    global sock
    while running:
        try:
            s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
            s.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            s.settimeout(5.0)
            s.connect((OMARCHY_TAILSCALE_IP, PORT))
            s.settimeout(None)
            with sock_lock:
                sock = s
            print(f"[+] Connected to Omarchy at {OMARCHY_TAILSCALE_IP}:{PORT}")
            return
        except Exception as e:
            print(f"[!] Connecting to Omarchy ({OMARCHY_TAILSCALE_IP}:{PORT}) failed: {e}. Retrying in 5s...")
            time.sleep(5)

def send_msg(payload):
    global sock, is_remote
    with sock_lock:
        if sock is None:
            return
        try:
            data = (json.dumps(payload) + "\n").encode('utf-8')
            sock.sendall(data)
        except Exception as ex:
            print(f"[!] Socket communication broken: {ex}")
            try:
                sock.close()
            except Exception:
                pass
            sock = None
            if is_remote:
                switch_to_local()
            threading.Thread(target=connect_to_omarchy, daemon=True).start()

def clipboard_monitor():
    global last_clipboard_text
    while running:
        try:
            current = pyperclip.paste()
            if current and current != last_clipboard_text:
                last_clipboard_text = current
                send_msg({"type": "clipboard", "text": current})
        except Exception:
            pass
        time.sleep(0.5)

def switch_to_remote():
    global is_remote, last_mouse_pos
    if sock is None:
        return
    is_remote = True
    print("[*] Control shifted -> Omarchy (Hyprland)")
    # Pin cursor to the rightmost boundary edge so it stays active
    rect = (ctypes.c_long * 4)(screen_width - 1, 0, screen_width, screen_height)
    user32.ClipCursor(ctypes.byref(rect))

def switch_to_local():
    global is_remote
    is_remote = False
    print("[*] Control shifted -> Windows Local")
    user32.ClipCursor(None)
    user32.SetCursorPos(screen_width - 60, screen_height // 2)

# --- Win32 Low-Level Event Interceptors (Suppresses Local Windows Duplication) ---

# Windows Message IDs
WM_MOUSEMOVE   = 0x0200
WM_LBUTTONDOWN = 0x0201
WM_LBUTTONUP   = 0x0202
WM_RBUTTONDOWN = 0x0204
WM_RBUTTONUP   = 0x0205
WM_MBUTTONDOWN = 0x0207
WM_MBUTTONUP   = 0x0208
WM_MOUSEWHEEL  = 0x020A
WM_MOUSEHWHEEL = 0x020E

WM_KEYDOWN     = 0x0100
WM_KEYUP       = 0x0101
WM_SYSKEYDOWN  = 0x0104
WM_SYSKEYUP    = 0x0105

VK_F12         = 0x7B

def win32_mouse_filter(msg, data):
    global is_remote, last_mouse_pos

    x = data.pt.x
    y = data.pt.y

    # Check for crossing to remote boundary
    if not is_remote:
        if x >= screen_width - 2:
            last_mouse_pos = (x, y)
            switch_to_remote()
            if mouse_listener:
                mouse_listener.suppress_event()
            return False
        return True

    # When in remote mode:
    # 1. Dispatch mouse moves
    if msg == WM_MOUSEMOVE:
        if last_mouse_pos is not None:
            dx = x - last_mouse_pos[0]
            dy = y - last_mouse_pos[1]
            if dx != 0 or dy != 0:
                send_msg({"type": "mouse_move", "dx": dx, "dy": dy})
        last_mouse_pos = (x, y)
        return True

    # 2. Dispatch clicks and SUPPRESS them locally so Windows doesn't click
    if msg in (WM_LBUTTONDOWN, WM_RBUTTONDOWN, WM_MBUTTONDOWN):
        btn = "left" if msg == WM_LBUTTONDOWN else ("right" if msg == WM_RBUTTONDOWN else "middle")
        send_msg({"type": "mouse_click", "button": btn, "action": "down"})
        if mouse_listener:
            mouse_listener.suppress_event()
        return False

    if msg in (WM_LBUTTONUP, WM_RBUTTONUP, WM_MBUTTONUP):
        btn = "left" if msg == WM_LBUTTONUP else ("right" if msg == WM_RBUTTONUP else "middle")
        send_msg({"type": "mouse_click", "button": btn, "action": "up"})
        if mouse_listener:
            mouse_listener.suppress_event()
        return False

    # 3. Dispatch scrolls and SUPPRESS them locally
    if msg == WM_MOUSEWHEEL:
        # High word of mouseData contains wheel delta (standard increment is 120)
        wheel_delta = ctypes.c_short(data.mouseData >> 16).value
        scroll_units = 1 if wheel_delta > 0 else -1
        send_msg({"type": "mouse_scroll", "dx": 0, "dy": scroll_units})
        if mouse_listener:
            mouse_listener.suppress_event()
        return False

    return True

def win32_keyboard_filter(msg, data):
    global is_remote

    vk = data.vkCode

    # Hotkey F12 toggles focus back to Windows
    if vk == VK_F12:
        if is_remote and msg in (WM_KEYDOWN, WM_SYSKEYDOWN):
            switch_to_local()
        if keyboard_listener:
            keyboard_listener.suppress_event()
        return False

    # If local, let all keystrokes pass through untouched
    if not is_remote:
        return True

    # If remote, map scancode, send to Omarchy, and SUPPRESS on Windows
    evdev_code = translate_vk(vk)
    action = "down" if msg in (WM_KEYDOWN, WM_SYSKEYDOWN) else "up"

    if evdev_code is not None:
        send_msg({"type": "key", "code": evdev_code, "action": action})

    # SUPPRESS keystroke so Windows doesn't execute hotkeys, type letters, or backspace locally
    if keyboard_listener:
        keyboard_listener.suppress_event()
    return False

if __name__ == "__main__":
    print(f"[*] Starting Windows KVM Host for Omarchy ({OMARCHY_TAILSCALE_IP}:{PORT})...")
    connect_to_omarchy()
    
    threading.Thread(target=clipboard_monitor, daemon=True).start()

    mouse_listener = mouse.Listener(win32_event_filter=win32_mouse_filter)
    keyboard_listener = keyboard.Listener(win32_event_filter=win32_keyboard_filter)

    mouse_listener.start()
    keyboard_listener.start()

    print("[*] Windows KVM Host active.")
    print("    - Hit right screen edge to enter Omarchy.")
    print("    - Press F12 to return control to Windows.")
    
    try:
        mouse_listener.join()
        keyboard_listener.join()
    except KeyboardInterrupt:
        print("\n[*] Shutting down...")
        running = False
        switch_to_local()
