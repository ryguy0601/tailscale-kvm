# vk_to_evdev.py
# Maps Windows Virtual Key codes to Linux evdev scancodes
VK_MAP = {
    0x08: 14,   # Backspace
    0x09: 15,   # Tab
    0x0D: 28,   # Enter
    0x10: 42,   # Shift
    0x11: 29,   # Ctrl
    0x12: 56,   # Alt
    0x14: 58,   # CapsLock
    0x1B: 1,    # Escape
    0x20: 57,   # Space
    0x25: 105,  # Left
    0x26: 103,  # Up
    0x27: 106,  # Right
    0x28: 108,  # Down
    0x2E: 111,  # Delete
    # Alphanumeric (A-Z)
    **{code: ev for code, ev in zip(range(0x41, 0x5B), [
        30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17, 45, 21, 44
    ])},
    # Number Row (0-9)
    **{code: ev for code, ev in zip(range(0x30, 0x3A), [11, 2, 3, 4, 5, 6, 7, 8, 9, 10])},
}

def translate_vk(vk_code):
    return VK_MAP.get(vk_code, None)
