// Comprehensive map from Windows Virtual-Key (VK) codes to Linux evdev scancodes

pub fn translate_vk(vk: u32) -> Option<u16> {
    match vk {
        // Control & Editing Keys
        0x08 => Some(14),  // VK_BACK -> KEY_BACKSPACE
        0x09 => Some(15),  // VK_TAB -> KEY_TAB
        0x0D => Some(28),  // VK_RETURN -> KEY_ENTER
        0x10 => Some(42),  // VK_SHIFT -> KEY_LEFTSHIFT
        0x11 => Some(29),  // VK_CONTROL -> KEY_LEFTCTRL
        0x12 => Some(56),  // VK_MENU -> KEY_LEFTALT
        0x13 => Some(119), // VK_PAUSE -> KEY_PAUSE
        0x14 => Some(58),  // VK_CAPITAL -> KEY_CAPSLOCK
        0x1B => Some(1),   // VK_ESCAPE -> KEY_ESC
        0x20 => Some(57),  // VK_SPACE -> KEY_SPACE

        // Navigation
        0x21 => Some(104), // VK_PRIOR -> KEY_PAGEUP
        0x22 => Some(109), // VK_NEXT -> KEY_PAGEDOWN
        0x23 => Some(107), // VK_END -> KEY_END
        0x24 => Some(102), // VK_HOME -> KEY_HOME
        0x25 => Some(105), // VK_LEFT -> KEY_LEFT
        0x26 => Some(103), // VK_UP -> KEY_UP
        0x27 => Some(106), // VK_RIGHT -> KEY_RIGHT
        0x28 => Some(108), // VK_DOWN -> KEY_DOWN
        0x2C => Some(99),  // VK_SNAPSHOT -> KEY_SYSRQ (PrintScreen)
        0x2D => Some(110), // VK_INSERT -> KEY_INSERT
        0x2E => Some(111), // VK_DELETE -> KEY_DELETE

        // Windows / Super Keys (Hyprland Hotkeys) & Explicit Left/Right Modifiers
        0x5B => Some(125), // VK_LWIN -> KEY_LEFTMETA (Super)
        0x5C => Some(126), // VK_RWIN -> KEY_RIGHTMETA
        0x5D => Some(127), // VK_APPS -> KEY_COMPOSE
        0xA0 => Some(42),  // VK_LSHIFT -> KEY_LEFTSHIFT
        0xA1 => Some(54),  // VK_RSHIFT -> KEY_RIGHTSHIFT
        0xA2 => Some(29),  // VK_LCONTROL -> KEY_LEFTCTRL
        0xA3 => Some(97),  // VK_RCONTROL -> KEY_RIGHTCTRL
        0xA4 => Some(56),  // VK_LMENU -> KEY_LEFTALT
        0xA5 => Some(100), // VK_RMENU -> KEY_RIGHTALT

        // Numbers 0-9
        0x30 => Some(11), // 0
        0x31 => Some(2),  // 1
        0x32 => Some(3),  // 2
        0x33 => Some(4),  // 3
        0x34 => Some(5),  // 4
        0x35 => Some(6),  // 5
        0x36 => Some(7),  // 6
        0x37 => Some(8),  // 7
        0x38 => Some(9),  // 8
        0x39 => Some(10), // 9

        // Function Keys (F1-F12)
        0x70 => Some(59), // F1
        0x71 => Some(60), // F2
        0x72 => Some(61), // F3
        0x73 => Some(62), // F4
        0x74 => Some(63), // F5
        0x75 => Some(64), // F6
        0x76 => Some(65), // F7
        0x77 => Some(66), // F8
        0x78 => Some(67), // F9
        0x79 => Some(68), // F10
        0x7A => Some(87), // F11
        0x7B => Some(88), // F12

        // OEM Punctuation & Symbols
        0xBA => Some(39), // ;:
        0xBB => Some(13), // =+
        0xBC => Some(51), // ,<
        0xBD => Some(12), // -_
        0xBE => Some(52), // .>
        0xBF => Some(53), // /?
        0xC0 => Some(41), // `~
        0xDB => Some(26), // [{
        0xDC => Some(43), // \|
        0xDD => Some(27), // ]}
        0xDE => Some(40), // '"

        // Numpad
        0x60 => Some(82),
        0x61 => Some(79),
        0x62 => Some(80),
        0x63 => Some(81),
        0x64 => Some(75),
        0x65 => Some(76),
        0x66 => Some(77),
        0x67 => Some(71),
        0x68 => Some(72),
        0x69 => Some(73),
        0x6A => Some(55),
        0x6B => Some(78),
        0x6D => Some(74),
        0x6E => Some(83),
        0x6F => Some(98),

        // Letters A-Z (0x41 - 0x5A)
        0x41 => Some(30), // A
        0x42 => Some(48), // B
        0x43 => Some(46), // C
        0x44 => Some(32), // D
        0x45 => Some(18), // E
        0x46 => Some(33), // F
        0x47 => Some(34), // G
        0x48 => Some(35), // H
        0x49 => Some(23), // I
        0x4A => Some(36), // J
        0x4B => Some(37), // K
        0x4C => Some(38), // L
        0x4D => Some(50), // M
        0x4E => Some(49), // N
        0x4F => Some(24), // O
        0x50 => Some(25), // P
        0x51 => Some(16), // Q
        0x52 => Some(19), // R
        0x53 => Some(31), // S
        0x54 => Some(20), // T
        0x55 => Some(22), // U
        0x56 => Some(47), // V
        0x57 => Some(17), // W
        0x58 => Some(45), // X
        0x59 => Some(21), // Y
        0x5A => Some(44), // Z

        _ => None,
    }
}
