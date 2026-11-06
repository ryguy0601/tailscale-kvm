# vk_to_evdev.py
# Comprehensive map from Windows Virtual-Key (VK) codes to Linux evdev scancodes

VK_MAP = {
    # Control / Editing Keys
    0x08: 14,   # VK_BACK -> KEY_BACKSPACE
    0x09: 15,   # VK_TAB -> KEY_TAB
    0x0D: 28,   # VK_RETURN -> KEY_ENTER
    0x10: 42,   # VK_SHIFT (Generic) -> KEY_LEFTSHIFT
    0x11: 29,   # VK_CONTROL (Generic) -> KEY_LEFTCTRL
    0x12: 56,   # VK_MENU (Alt Generic) -> KEY_LEFTALT
    0x13: 119,  # VK_PAUSE -> KEY_PAUSE
    0x14: 58,   # VK_CAPITAL -> KEY_CAPSLOCK
    0x1B: 1,    # VK_ESCAPE -> KEY_ESC
    0x20: 57,   # VK_SPACE -> KEY_SPACE

    # Navigation / Cursor Keys
    0x21: 104,  # VK_PRIOR -> KEY_PAGEUP
    0x22: 109,  # VK_NEXT -> KEY_PAGEDOWN
    0x23: 107,  # VK_END -> KEY_END
    0x24: 102,  # VK_HOME -> KEY_HOME
    0x25: 105,  # VK_LEFT -> KEY_LEFT
    0x26: 103,  # VK_UP -> KEY_UP
    0x27: 106,  # VK_RIGHT -> KEY_RIGHT
    0x28: 108,  # VK_DOWN -> KEY_DOWN
    0x2C: 99,   # VK_SNAPSHOT -> KEY_SYSRQ (PrintScreen)
    0x2D: 110,  # VK_INSERT -> KEY_INSERT
    0x2E: 111,  # VK_DELETE -> KEY_DELETE

    # Left / Right Explicit Modifiers & Windows / Super Keys (CRITICAL FOR HYPRLAND HOTKEYS)
    0x5B: 125,  # VK_LWIN -> KEY_LEFTMETA (Super / Windows Key)
    0x5C: 126,  # VK_RWIN -> KEY_RIGHTMETA
    0x5D: 127,  # VK_APPS -> KEY_COMPOSE
    0xA0: 42,   # VK_LSHIFT -> KEY_LEFTSHIFT
    0xA1: 54,   # VK_RSHIFT -> KEY_RIGHTSHIFT
    0xA2: 29,   # VK_LCONTROL -> KEY_LEFTCTRL
    0xA3: 97,   # VK_RCONTROL -> KEY_RIGHTCTRL
    0xA4: 56,   # VK_LMENU -> KEY_LEFTALT
    0xA5: 100,  # VK_RMENU -> KEY_RIGHTALT

    # Number Row (0-9)
    0x30: 11,   # 0
    0x31: 2,    # 1
    0x32: 3,    # 2
    0x33: 4,    # 3
    0x34: 5,    # 4
    0x35: 6,    # 5
    0x36: 7,    # 6
    0x37: 8,    # 7
    0x38: 9,    # 8
    0x39: 10,   # 9

    # Function Keys (F1 - F12)
    0x70: 59,   # F1
    0x71: 60,   # F2
    0x72: 61,   # F3
    0x73: 62,   # F4
    0x74: 63,   # F5
    0x75: 64,   # F6
    0x76: 65,   # F7
    0x77: 66,   # F8
    0x78: 67,   # F9
    0x79: 68,   # F10
    0x7A: 87,   # F11
    0x7B: 88,   # F12

    # OEM Punctuation & Symbols (US Layout)
    0xBA: 39,   # VK_OEM_1 (;: / Semicolon) -> KEY_SEMICOLON
    0xBB: 13,   # VK_OEM_PLUS (=+) -> KEY_EQUAL
    0xBC: 51,   # VK_OEM_COMMA (,<) -> KEY_COMMA
    0xBD: 12,   # VK_OEM_MINUS (-_) -> KEY_MINUS
    0xBE: 52,   # VK_OEM_PERIOD (.>) -> KEY_DOT
    0xBF: 53,   # VK_OEM_2 (/?) -> KEY_SLASH
    0xC0: 41,   # VK_OEM_3 (`~) -> KEY_GRAVE
    0xDB: 26,   # VK_OEM_4 ([{) -> KEY_LEFTBRACE
    0xDC: 43,   # VK_OEM_5 (\|) -> KEY_BACKSLASH
    0xDD: 27,   # VK_OEM_6 (]}) -> KEY_RIGHTBRACE
    0xDE: 40,   # VK_OEM_7 ('") -> KEY_APOSTROPHE

    # Numpad
    0x60: 82,   # NUMPAD 0
    0x61: 79,   # NUMPAD 1
    0x62: 80,   # NUMPAD 2
    0x63: 81,   # NUMPAD 3
    0x64: 75,   # NUMPAD 4
    0x65: 76,   # NUMPAD 5
    0x66: 77,   # NUMPAD 6
    0x67: 71,   # NUMPAD 7
    0x68: 72,   # NUMPAD 8
    0x69: 73,   # NUMPAD 9
    0x6A: 55,   # NUMPAD *
    0x6B: 78,   # NUMPAD +
    0x6D: 74,   # NUMPAD -
    0x6E: 83,   # NUMPAD .
    0x6F: 98,   # NUMPAD /
}

# Alphanumeric (A-Z)
for code, ev in zip(range(0x41, 0x5B), [
    30, 48, 46, 32, 18, 33, 34, 35, 23, 36, 37, 38, 50, 49, 24, 25, 16, 19, 31, 20, 22, 47, 17, 45, 21, 44
]):
    VK_MAP[code] = ev

def translate_vk(vk_code):
    return VK_MAP.get(vk_code, None)
