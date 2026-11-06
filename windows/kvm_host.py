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

# TARGET CONFIGURATION: Environment variable or prompt fallback
DEFAULT_OMARCHY_IP = os.environ.get("OMARCHY_TAILSCALE_IP", "100.x.y.z")
PORT = int(os.environ.get("OMARCHY_PORT", 9876))

OMARCHY_TAILSCALE_IP = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_OMARCHY_IP

if OMARCHY_TAILSCALE_IP == "100.x.y.z":
    print("[!] Please specify your Omarchy Tailscale IP:")
    print("    python kvm_host.py <omarchy_ip>")
    print("    Or set OMARCHY_TAILSCALE_IP environment variable.")
    sys.exit(1)

sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)

try:
    sock.connect((OMARCHY_TAILSCALE_IP, PORT))
    print(f"[+] Connected to Omarchy at {OMARCHY_TAILSCALE_IP}:{PORT}")
except Exception as e:
    print(f"[!] Connection failed: {e}")
    sys.exit(1)

user32 = ctypes.windll.user32
user32.SetProcessDPIAware()

screen_width = user32.GetSystemMetrics(0)
screen_height = user32.GetSystemMetrics(1)

is_remote = False
last_mouse_pos = None
last_clipboard_text = ""

def send_msg(payload):
    try:
        data = (json.dumps(payload) + "\n").encode('utf-8')
        sock.sendall(data)
    except Exception as ex:
        print(f"[!] Socket send error: {ex}")

def send_file(filepath):
    if not os.path.isfile(filepath):
        print(f"[!] Invalid file path: {filepath}")
        return
    filename = os.path.basename(filepath)
    filesize = os.path.getsize(filepath)
    
    header = {"type": "file_transfer", "name": filename, "size": filesize}
    send_msg(header)
    
    ack = sock.recv(1024).decode().strip()
    if ack == "READY":
        with open(filepath, "rb") as f:
            while chunk := f.read(65536):
                sock.sendall(chunk)
        print(f"[+] Successfully sent {filename}")

def clipboard_monitor():
    global last_clipboard_text
    while True:
        try:
            current = pyperclip.paste()
            if current and current != last_clipboard_text:
                last_clipboard_text = current
                send_msg({"type": "clipboard", "text": current})
        except Exception:
            pass
        time.sleep(0.5)

threading.Thread(target=clipboard_monitor, daemon=True).start()

def switch_to_remote():
    global is_remote, last_mouse_pos
    is_remote = True
    print("[*] Control shifted -> Omarchy (Hyprland)")
    rect = (ctypes.c_long * 4)(screen_width - 1, 0, screen_width, screen_height)
    user32.ClipCursor(ctypes.byref(rect))

def switch_to_local():
    global is_remote
    is_remote = False
    print("[*] Control shifted -> Windows Local")
    user32.ClipCursor(None)
    user32.SetCursorPos(screen_width - 50, screen_height // 2)

def on_move(x, y):
    global is_remote, last_mouse_pos
    if not is_remote and x >= screen_width - 2:
        last_mouse_pos = (x, y)
        switch_to_remote()
        return

    if is_remote:
        if last_mouse_pos:
            dx = x - last_mouse_pos[0]
            dy = y - last_mouse_pos[1]
            if dx != 0 or dy != 0:
                send_msg({"type": "mouse_move", "dx": dx, "dy": dy})
        last_mouse_pos = (x, y)

def on_click(x, y, button, pressed):
    if is_remote:
        btn_name = "left" if button == mouse.Button.left else "right" if button == mouse.Button.right else "middle"
        action = "down" if pressed else "up"
        send_msg({"type": "mouse_click", "button": btn_name, "action": action})

def on_scroll(x, y, dx, dy):
    if is_remote:
        send_msg({"type": "mouse_scroll", "dx": int(dx), "dy": int(dy)})

def on_press(key):
    if key == keyboard.Key.f12:
        if is_remote:
            switch_to_local()
        return

    if is_remote:
        vk = getattr(key, 'vk', None)
        if vk is not None:
            evdev_code = translate_vk(vk)
            if evdev_code:
                send_msg({"type": "key", "code": evdev_code, "action": "down"})

def on_release(key):
    if is_remote:
        vk = getattr(key, 'vk', None)
        if vk is not None:
            evdev_code = translate_vk(vk)
            if evdev_code:
                send_msg({"type": "key", "code": evdev_code, "action": "up"})

mouse_listener = mouse.Listener(on_move=on_move, on_click=on_click, on_scroll=on_scroll)
keyboard_listener = keyboard.Listener(on_press=on_press, on_release=on_release)

mouse_listener.start()
keyboard_listener.start()

print("[*] Windows KVM Host active. Hit right screen edge to enter Omarchy. F12 to return.")
mouse_listener.join()
keyboard_listener.join()
