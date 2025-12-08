
#![allow(unused)]

use kanata_interception::ScanCode;

// Taken from:
// https://github.com/retep998/winapi-rs/blob/0.3/src/um/winuser.rs#L253
pub const VK_LBUTTON: u16 = 0x01;
pub const VK_RBUTTON: u16 = 0x02;
pub const VK_CANCEL: u16 = 0x03;
pub const VK_MBUTTON: u16 = 0x04;
pub const VK_XBUTTON1: u16 = 0x05;
pub const VK_XBUTTON2: u16 = 0x06;
pub const VK_BACK: u16 = 0x08;
pub const VK_TAB: u16 = 0x09;
pub const VK_CLEAR: u16 = 0x0C;
pub const VK_RETURN: u16 = 0x0D;
pub const VK_SHIFT: u16 = 0x10;
pub const VK_CONTROL: u16 = 0x11;
pub const VK_MENU: u16 = 0x12;
pub const VK_PAUSE: u16 = 0x13;
pub const VK_CAPITAL: u16 = 0x14;
pub const VK_KANA: u16 = 0x15;
pub const VK_HANGEUL: u16 = 0x15;
pub const VK_HANGUL: u16 = 0x15;
pub const VK_JUNJA: u16 = 0x17;
pub const VK_FINAL: u16 = 0x18;
pub const VK_HANJA: u16 = 0x19;
pub const VK_KANJI: u16 = 0x19;
pub const VK_ESCAPE: u16 = 0x1B;
pub const VK_CONVERT: u16 = 0x1C;
pub const VK_NONCONVERT: u16 = 0x1D;
pub const VK_ACCEPT: u16 = 0x1E;
pub const VK_MODECHANGE: u16 = 0x1F;
pub const VK_SPACE: u16 = 0x20;
pub const VK_PRIOR: u16 = 0x21;
pub const VK_NEXT: u16 = 0x22;
pub const VK_END: u16 = 0x23;
pub const VK_HOME: u16 = 0x24;
pub const VK_LEFT: u16 = 0x25;
pub const VK_UP: u16 = 0x26;
pub const VK_RIGHT: u16 = 0x27;
pub const VK_DOWN: u16 = 0x28;
pub const VK_SELECT: u16 = 0x29;
pub const VK_PRINT: u16 = 0x2A;
pub const VK_EXECUTE: u16 = 0x2B;
pub const VK_SNAPSHOT: u16 = 0x2C;
pub const VK_INSERT: u16 = 0x2D;
pub const VK_DELETE: u16 = 0x2E;
pub const VK_HELP: u16 = 0x2F;
pub const VK_LWIN: u16 = 0x5B;
pub const VK_RWIN: u16 = 0x5C;
pub const VK_APPS: u16 = 0x5D;
pub const VK_SLEEP: u16 = 0x5F;
pub const VK_NUMPAD0: u16 = 0x60;
pub const VK_NUMPAD1: u16 = 0x61;
pub const VK_NUMPAD2: u16 = 0x62;
pub const VK_NUMPAD3: u16 = 0x63;
pub const VK_NUMPAD4: u16 = 0x64;
pub const VK_NUMPAD5: u16 = 0x65;
pub const VK_NUMPAD6: u16 = 0x66;
pub const VK_NUMPAD7: u16 = 0x67;
pub const VK_NUMPAD8: u16 = 0x68;
pub const VK_NUMPAD9: u16 = 0x69;
pub const VK_MULTIPLY: u16 = 0x6A;
pub const VK_ADD: u16 = 0x6B;
pub const VK_SEPARATOR: u16 = 0x6C;
pub const VK_SUBTRACT: u16 = 0x6D;
pub const VK_DECIMAL: u16 = 0x6E;
pub const VK_DIVIDE: u16 = 0x6F;
pub const VK_F1: u16 = 0x70;
pub const VK_F2: u16 = 0x71;
pub const VK_F3: u16 = 0x72;
pub const VK_F4: u16 = 0x73;
pub const VK_F5: u16 = 0x74;
pub const VK_F6: u16 = 0x75;
pub const VK_F7: u16 = 0x76;
pub const VK_F8: u16 = 0x77;
pub const VK_F9: u16 = 0x78;
pub const VK_F10: u16 = 0x79;
pub const VK_F11: u16 = 0x7A;
pub const VK_F12: u16 = 0x7B;
pub const VK_F13: u16 = 0x7C;
pub const VK_F14: u16 = 0x7D;
pub const VK_F15: u16 = 0x7E;
pub const VK_F16: u16 = 0x7F;
pub const VK_F17: u16 = 0x80;
pub const VK_F18: u16 = 0x81;
pub const VK_F19: u16 = 0x82;
pub const VK_F20: u16 = 0x83;
pub const VK_F21: u16 = 0x84;
pub const VK_F22: u16 = 0x85;
pub const VK_F23: u16 = 0x86;
pub const VK_F24: u16 = 0x87;
pub const VK_NAVIGATION_VIEW: u16 = 0x88;
pub const VK_NAVIGATION_MENU: u16 = 0x89;
pub const VK_NAVIGATION_UP: u16 = 0x8A;
pub const VK_NAVIGATION_DOWN: u16 = 0x8B;
pub const VK_NAVIGATION_LEFT: u16 = 0x8C;
pub const VK_NAVIGATION_RIGHT: u16 = 0x8D;
pub const VK_NAVIGATION_ACCEPT: u16 = 0x8E;
pub const VK_NAVIGATION_CANCEL: u16 = 0x8F;
pub const VK_NUMLOCK: u16 = 0x90;
pub const VK_SCROLL: u16 = 0x91;
pub const VK_OEM_NEC_EQUAL: u16 = 0x92;
pub const VK_OEM_FJ_JISHO: u16 = 0x92;
pub const VK_OEM_FJ_MASSHOU: u16 = 0x93;
pub const VK_OEM_FJ_TOUROKU: u16 = 0x94;
pub const VK_OEM_FJ_LOYA: u16 = 0x95;
pub const VK_OEM_FJ_ROYA: u16 = 0x96;
pub const VK_LSHIFT: u16 = 0xA0;
pub const VK_RSHIFT: u16 = 0xA1;
pub const VK_LCONTROL: u16 = 0xA2;
pub const VK_RCONTROL: u16 = 0xA3;
pub const VK_LMENU: u16 = 0xA4;
pub const VK_RMENU: u16 = 0xA5;
pub const VK_BROWSER_BACK: u16 = 0xA6;
pub const VK_BROWSER_FORWARD: u16 = 0xA7;
pub const VK_BROWSER_REFRESH: u16 = 0xA8;
pub const VK_BROWSER_STOP: u16 = 0xA9;
pub const VK_BROWSER_SEARCH: u16 = 0xAA;
pub const VK_BROWSER_FAVORITES: u16 = 0xAB;
pub const VK_BROWSER_HOME: u16 = 0xAC;
pub const VK_VOLUME_MUTE: u16 = 0xAD;
pub const VK_VOLUME_DOWN: u16 = 0xAE;
pub const VK_VOLUME_UP: u16 = 0xAF;
pub const VK_MEDIA_NEXT_TRACK: u16 = 0xB0;
pub const VK_MEDIA_PREV_TRACK: u16 = 0xB1;
pub const VK_MEDIA_STOP: u16 = 0xB2;
pub const VK_MEDIA_PLAY_PAUSE: u16 = 0xB3;
pub const VK_LAUNCH_MAIL: u16 = 0xB4;
pub const VK_LAUNCH_MEDIA_SELECT: u16 = 0xB5;
pub const VK_LAUNCH_APP1: u16 = 0xB6;
pub const VK_LAUNCH_APP2: u16 = 0xB7;
pub const VK_OEM_1: u16 = 0xBA;
pub const VK_OEM_PLUS: u16 = 0xBB;
pub const VK_OEM_COMMA: u16 = 0xBC;
pub const VK_OEM_MINUS: u16 = 0xBD;
pub const VK_OEM_PERIOD: u16 = 0xBE;
pub const VK_OEM_2: u16 = 0xBF;
pub const VK_OEM_3: u16 = 0xC0;
pub const VK_GAMEPAD_A: u16 = 0xC3;
pub const VK_GAMEPAD_B: u16 = 0xC4;
pub const VK_GAMEPAD_X: u16 = 0xC5;
pub const VK_GAMEPAD_Y: u16 = 0xC6;
pub const VK_GAMEPAD_RIGHT_SHOULDER: u16 = 0xC7;
pub const VK_GAMEPAD_LEFT_SHOULDER: u16 = 0xC8;
pub const VK_GAMEPAD_LEFT_TRIGGER: u16 = 0xC9;
pub const VK_GAMEPAD_RIGHT_TRIGGER: u16 = 0xCA;
pub const VK_GAMEPAD_DPAD_UP: u16 = 0xCB;
pub const VK_GAMEPAD_DPAD_DOWN: u16 = 0xCC;
pub const VK_GAMEPAD_DPAD_LEFT: u16 = 0xCD;
pub const VK_GAMEPAD_DPAD_RIGHT: u16 = 0xCE;
pub const VK_GAMEPAD_MENU: u16 = 0xCF;
pub const VK_GAMEPAD_VIEW: u16 = 0xD0;
pub const VK_GAMEPAD_LEFT_THUMBSTICK_BUTTON: u16 = 0xD1;
pub const VK_GAMEPAD_RIGHT_THUMBSTICK_BUTTON: u16 = 0xD2;
pub const VK_GAMEPAD_LEFT_THUMBSTICK_UP: u16 = 0xD3;
pub const VK_GAMEPAD_LEFT_THUMBSTICK_DOWN: u16 = 0xD4;
pub const VK_GAMEPAD_LEFT_THUMBSTICK_RIGHT: u16 = 0xD5;
pub const VK_GAMEPAD_LEFT_THUMBSTICK_LEFT: u16 = 0xD6;
pub const VK_GAMEPAD_RIGHT_THUMBSTICK_UP: u16 = 0xD7;
pub const VK_GAMEPAD_RIGHT_THUMBSTICK_DOWN: u16 = 0xD8;
pub const VK_GAMEPAD_RIGHT_THUMBSTICK_RIGHT: u16 = 0xD9;
pub const VK_GAMEPAD_RIGHT_THUMBSTICK_LEFT: u16 = 0xDA;
pub const VK_OEM_4: u16 = 0xDB;
pub const VK_OEM_5: u16 = 0xDC;
pub const VK_OEM_6: u16 = 0xDD;
pub const VK_OEM_7: u16 = 0xDE;
pub const VK_OEM_8: u16 = 0xDF;
pub const VK_OEM_AX: u16 = 0xE1;
pub const VK_OEM_102: u16 = 0xE2;
pub const VK_ICO_HELP: u16 = 0xE3;
pub const VK_ICO_00: u16 = 0xE4;
pub const VK_PROCESSKEY: u16 = 0xE5;
pub const VK_ICO_CLEAR: u16 = 0xE6;
pub const VK_PACKET: u16 = 0xE7;
pub const VK_OEM_RESET: u16 = 0xE9;
pub const VK_OEM_JUMP: u16 = 0xEA;
pub const VK_OEM_PA1: u16 = 0xEB;
pub const VK_OEM_PA2: u16 = 0xEC;
pub const VK_OEM_PA3: u16 = 0xED;
pub const VK_OEM_WSCTRL: u16 = 0xEE;
pub const VK_OEM_CUSEL: u16 = 0xEF;
pub const VK_OEM_ATTN: u16 = 0xF0;
pub const VK_OEM_FINISH: u16 = 0xF1;
pub const VK_OEM_COPY: u16 = 0xF2;
pub const VK_OEM_AUTO: u16 = 0xF3;
pub const VK_OEM_ENLW: u16 = 0xF4;
pub const VK_OEM_BACKTAB: u16 = 0xF5;
pub const VK_ATTN: u16 = 0xF6;
pub const VK_CRSEL: u16 = 0xF7;
pub const VK_EXSEL: u16 = 0xF8;
pub const VK_EREOF: u16 = 0xF9;
pub const VK_PLAY: u16 = 0xFA;
pub const VK_ZOOM: u16 = 0xFB;
pub const VK_NONAME: u16 = 0xFC;
pub const VK_PA1: u16 = 0xFD;
pub const VK_OEM_CLEAR: u16 = 0xFE;


pub fn all_scan_codes()->Vec<ScanCode>{
    vec![
        ScanCode::Esc,
        ScanCode::Num1,
        ScanCode::Num2,
        ScanCode::Num3,
        ScanCode::Num4,
        ScanCode::Num5,
        ScanCode::Num6,
        ScanCode::Num7,
        ScanCode::Num8,
        ScanCode::Num9,
        ScanCode::Num0,
        ScanCode::Minus,
        ScanCode::Equals,
        ScanCode::Backspace,
        ScanCode::Tab,
        ScanCode::Q,
        ScanCode::W,
        ScanCode::E,
        ScanCode::R,
        ScanCode::T,
        ScanCode::Y,
        ScanCode::U,
        ScanCode::I,
        ScanCode::O,
        ScanCode::P,
        ScanCode::LeftBracket,
        ScanCode::RightBracket,
        ScanCode::Enter,
        ScanCode::LeftControl,
        ScanCode::A,
        ScanCode::S,
        ScanCode::D,
        ScanCode::F,
        ScanCode::G,
        ScanCode::H,
        ScanCode::J,
        ScanCode::K,
        ScanCode::L,
        ScanCode::SemiColon,
        ScanCode::Apostrophe,
        ScanCode::Grave,
        ScanCode::LeftShift,
        ScanCode::BackSlash,
        ScanCode::Z,
        ScanCode::X,
        ScanCode::C,
        ScanCode::V,
        ScanCode::B,
        ScanCode::N,
        ScanCode::M,
        ScanCode::Comma,
        ScanCode::Period,
        ScanCode::Slash,
        ScanCode::RightShift,
        ScanCode::NumpadMultiply,
        ScanCode::LeftAlt,
        ScanCode::Space,
        ScanCode::CapsLock,
        ScanCode::F1,
        ScanCode::F2,
        ScanCode::F3,
        ScanCode::F4,
        ScanCode::F5,
        ScanCode::F6,
        ScanCode::F7,
        ScanCode::F8,
        ScanCode::F9,
        ScanCode::F10,
        ScanCode::NumLock,
        ScanCode::ScrollLock,
        ScanCode::Numpad7,
        ScanCode::Numpad8,
        ScanCode::Numpad9,
        ScanCode::NumpadMinus,
        ScanCode::Numpad4,
        ScanCode::Numpad5,
        ScanCode::Numpad6,
        ScanCode::NumpadPlus,
        ScanCode::Numpad1,
        ScanCode::Numpad2,
        ScanCode::Numpad3,
        ScanCode::Numpad0,
        ScanCode::NumpadPeriod,
        ScanCode::AltPrintScreen,
        ScanCode::SC_55,
        ScanCode::Int1,
        ScanCode::F11,
        ScanCode::F12,
        ScanCode::SC_59,
        ScanCode::Oem1,
        ScanCode::Oem2,
        ScanCode::Oem3,
        ScanCode::EraseEOF,
        ScanCode::Oem4,
        ScanCode::Oem5,
        ScanCode::SC_60,
        ScanCode::SC_61,
        ScanCode::Zoom,
        ScanCode::Help,
        ScanCode::F13,
        ScanCode::F14,
        ScanCode::F15,
        ScanCode::F16,
        ScanCode::F17,
        ScanCode::F18,
        ScanCode::F19,
        ScanCode::F20,
        ScanCode::F21,
        ScanCode::F22,
        ScanCode::F23,
        ScanCode::Oem6,
        ScanCode::Katakana,
        ScanCode::Oem7,
        ScanCode::SC_72,
        ScanCode::SC_73,
        ScanCode::SC_74,
        ScanCode::SC_75,
        ScanCode::F24,
        ScanCode::SBCSChar,
        ScanCode::SC_78,
        ScanCode::Convert,
        ScanCode::SC_7A,
        ScanCode::NonConvert,
        ScanCode::SC_7C,
        ScanCode::SC_7D,
        ScanCode::SC_7E,
        ScanCode::SC_7F,
        ScanCode::SC_80,
        ScanCode::SC_81,
        ScanCode::SC_82,
        ScanCode::SC_83,
        ScanCode::SC_84,
        ScanCode::SC_85,
        ScanCode::SC_86,
        ScanCode::SC_87,
        ScanCode::SC_88,
        ScanCode::SC_89,
        ScanCode::SC_8A,
        ScanCode::SC_8B,
        ScanCode::SC_8C,
        ScanCode::SC_8D,
        ScanCode::SC_8E,
        ScanCode::SC_8F,
        ScanCode::SC_90,
        ScanCode::SC_91,
        ScanCode::SC_92,
        ScanCode::SC_93,
        ScanCode::SC_94,
        ScanCode::SC_95,
        ScanCode::SC_96,
        ScanCode::SC_97,
        ScanCode::SC_98,
        ScanCode::SC_99,
        ScanCode::SC_9A,
        ScanCode::SC_9B,
        ScanCode::SC_9C,
        ScanCode::SC_9D,
        ScanCode::SC_9E,
        ScanCode::SC_9F,
        ScanCode::SC_A0,
        ScanCode::SC_A1,
        ScanCode::SC_A2,
        ScanCode::SC_A3,
        ScanCode::SC_A4,
        ScanCode::SC_A5,
        ScanCode::SC_A6,
        ScanCode::SC_A7,
        ScanCode::SC_A8,
        ScanCode::SC_A9,
        ScanCode::SC_AA,
        ScanCode::SC_AB,
        ScanCode::SC_AC,
        ScanCode::SC_AD,
        ScanCode::SC_AE,
        ScanCode::SC_AF,
        ScanCode::SC_B0,
        ScanCode::SC_B1,
        ScanCode::SC_B2,
        ScanCode::SC_B3,
        ScanCode::SC_B4,
        ScanCode::SC_B5,
        ScanCode::SC_B6,
        ScanCode::SC_B7,
        ScanCode::SC_B8,
        ScanCode::SC_B9,
        ScanCode::SC_BA,
        ScanCode::SC_BB,
        ScanCode::SC_BC,
        ScanCode::SC_BD,
        ScanCode::SC_BE,
        ScanCode::SC_BF,
        ScanCode::SC_C0,
        ScanCode::SC_C1,
        ScanCode::SC_C2,
        ScanCode::SC_C3,
        ScanCode::SC_C4,
        ScanCode::SC_C5,
        ScanCode::SC_C6,
        ScanCode::SC_C7,
        ScanCode::SC_C8,
        ScanCode::SC_C9,
        ScanCode::SC_CA,
        ScanCode::SC_CB,
        ScanCode::SC_CC,
        ScanCode::SC_CD,
        ScanCode::SC_CE,
        ScanCode::SC_CF,
        ScanCode::SC_D0,
        ScanCode::SC_D1,
        ScanCode::SC_D2,
        ScanCode::SC_D3,
        ScanCode::SC_D4,
        ScanCode::SC_D5,
        ScanCode::SC_D6,
        ScanCode::SC_D7,
        ScanCode::SC_D8,
        ScanCode::SC_D9,
        ScanCode::SC_DA,
        ScanCode::SC_DB,
        ScanCode::SC_DC,
        ScanCode::SC_DD,
        ScanCode::SC_DE,
        ScanCode::SC_DF,
        ScanCode::SC_E0,
        ScanCode::SC_E1,
        ScanCode::SC_E2,
        ScanCode::SC_E3,
        ScanCode::SC_E4,
        ScanCode::SC_E5,
        ScanCode::SC_E6,
        ScanCode::SC_E7,
        ScanCode::SC_E8,
        ScanCode::SC_E9,
        ScanCode::SC_EA,
        ScanCode::SC_EB,
        ScanCode::SC_EC,
        ScanCode::SC_ED,
        ScanCode::SC_EE,
        ScanCode::SC_EF,
        ScanCode::SC_F0,
        ScanCode::SC_F1,
        ScanCode::SC_F2,
        ScanCode::SC_F3,
        ScanCode::SC_F4,
        ScanCode::SC_F5,
        ScanCode::SC_F6,
        ScanCode::SC_F7,
        ScanCode::SC_F8,
        ScanCode::SC_F9,
        ScanCode::SC_FA,
        ScanCode::SC_FB,
        ScanCode::SC_FC,
        ScanCode::SC_FD,
        ScanCode::SC_FE,
        ScanCode::SC_NonExtendMax,
    ]
}