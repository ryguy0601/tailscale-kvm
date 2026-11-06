#!/usr/bin/env python3
import asyncio
import json
import subprocess
import os
import sys

try:
    from evdev import UInput, ecodes as e
except ImportError:
    print("[!] Missing 'evdev' library. Run setup_receiver.sh first.")
    sys.exit(1)

# Initialize virtual hardware device
capabilities = {
    e.EV_KEY: [
        e.BTN_LEFT, e.BTN_RIGHT, e.BTN_MIDDLE,
        e.BTN_SIDE, e.BTN_EXTRA
    ],
    e.EV_REL: [
        e.REL_X, e.REL_Y, e.REL_WHEEL, e.REL_HWHEEL
    ]
}

# Register standard full-range keyboard keys
for code in range(1, 248):
    capabilities[e.EV_KEY].append(code)

try:
    uinput_device = UInput(capabilities, name="Virtual-Tailscale-KVM")
except PermissionError:
    print("[!] Permission denied accessing /dev/uinput.")
    print("    Ensure your user is in the 'input' group and udev rules are loaded.")
    print("    See setup_receiver.sh for setup instructions.")
    sys.exit(1)

DOWNLOAD_DIR = os.path.expanduser("~/Downloads")

async def handle_client(reader, writer):
    peer = writer.get_extra_info('peername')
    print(f"[+] Active connection from {peer}")
    
    while True:
        line = await reader.readline()
        if not line:
            break
        
        try:
            msg = json.loads(line.decode('utf-8'))
            msg_type = msg.get("type")
            
            if msg_type == "mouse_move":
                uinput_device.write(e.EV_REL, e.REL_X, msg["dx"])
                uinput_device.write(e.EV_REL, e.REL_Y, msg["dy"])
                uinput_device.syn()

            elif msg_type == "mouse_click":
                btn = {
                    "left": e.BTN_LEFT,
                    "right": e.BTN_RIGHT,
                    "middle": e.BTN_MIDDLE
                }.get(msg.get("button"), e.BTN_LEFT)
                action = 1 if msg.get("action") == "down" else 0
                uinput_device.write(e.EV_KEY, btn, action)
                uinput_device.syn()

            elif msg_type == "mouse_scroll":
                if "dy" in msg and msg["dy"] != 0:
                    uinput_device.write(e.EV_REL, e.REL_WHEEL, msg["dy"])
                if "dx" in msg and msg["dx"] != 0:
                    uinput_device.write(e.EV_REL, e.REL_HWHEEL, msg["dx"])
                uinput_device.syn()

            elif msg_type == "key":
                key_code = msg.get("code")
                action = 1 if msg.get("action") == "down" else 0
                if key_code:
                    uinput_device.write(e.EV_KEY, key_code, action)
                    uinput_device.syn()

            elif msg_type == "clipboard":
                text = msg.get("text", "")
                proc = subprocess.Popen(['wl-copy'], stdin=subprocess.PIPE)
                proc.communicate(input=text.encode('utf-8'))

            elif msg_type == "file_transfer":
                file_name = os.path.basename(msg.get("name", "transferred_file"))
                target_path = os.path.join(DOWNLOAD_DIR, file_name)
                file_size = msg.get("size", 0)
                
                writer.write(b"READY\n")
                await writer.drain()
                
                bytes_read = 0
                with open(target_path, "wb") as f:
                    while bytes_read < file_size:
                        chunk = await reader.read(min(65536, file_size - bytes_read))
                        if not chunk:
                            break
                        f.write(chunk)
                        bytes_read += len(chunk)
                
                subprocess.run(['notify-send', "KVM Transfer Complete", f"Received {file_name} in ~/Downloads"])

        except Exception as ex:
            print(f"[!] Processing error: {ex}")

    print(f"[-] Disconnected from {peer}")
    writer.close()
    await writer.wait_closed()

async def main():
    server = await asyncio.start_server(handle_client, '0.0.0.0', 9876)
    print("[*] Omarchy KVM Receiver listening on port 9876...")
    async with server:
        await server.serve_forever()

if __name__ == "__main__":
    try:
        asyncio.run(main())
    except KeyboardInterrupt:
        pass
    finally:
        try:
            uinput_device.close()
        except NameError:
            pass
