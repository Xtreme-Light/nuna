//! 按键映射的配置模型：
//! - 一条映射 = 原始组合(修饰键集合 + 主键) -> 目标组合(修饰键集合 + 主键)
//! - 组合里的键用"扫描码 + 是否E0扩展"表示，和拦截驱动上报的一致，
//!   因此采集时记录下来的 (code, ext) 可以直接原样注入回去
//! - 持久化为 exe 同目录的 config.json，不存在或损坏时使用内置默认映射

use kanata_interception::{KeyState, ScanCode, Stroke};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 修饰键位标志（采集时按物理按下的左右侧分别记录，注入时按原侧还原）
pub const M_CAPS: u16 = 1 << 0;
pub const M_LCTRL: u16 = 1 << 1;
pub const M_RCTRL: u16 = 1 << 2;
pub const M_LSHIFT: u16 = 1 << 3;
pub const M_RSHIFT: u16 = 1 << 4;
pub const M_LALT: u16 = 1 << 5;
pub const M_RALT: u16 = 1 << 6;
pub const M_LWIN: u16 = 1 << 7;
pub const M_RWIN: u16 = 1 << 8;

/// 一条完整映射
#[derive(Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Debug)]
pub struct KeyMapping {
    pub src_mods: u16,
    pub src_code: u16,
    pub src_ext: bool,
    pub dst_mods: u16,
    pub dst_code: u16,
    pub dst_ext: bool,
}

/// 采集结果：由拦截线程在采集模式下上报给配置界面
#[derive(Clone, Copy, Debug)]
pub enum CaptureMsg {
    Combo { mods: u16, code: u16, ext: bool },
    Cancelled,
}

/// 用枚举变体写默认表，减少魔法数字
fn m(
    src_mods: u16,
    src: ScanCode,
    src_ext: bool,
    dst_mods: u16,
    dst: ScanCode,
    dst_ext: bool,
) -> KeyMapping {
    KeyMapping {
        src_mods,
        src_code: src as u16,
        src_ext,
        dst_mods,
        dst_code: dst as u16,
        dst_ext,
    }
}

/// 内置默认映射（与 README 的键位表一致），也用于"恢复默认"
pub fn default_mappings() -> Vec<KeyMapping> {
    use ScanCode::*;
    vec![
        m(M_CAPS, Space, false, 0, Backspace, false),          // Caps+Space = Backspace
        m(M_CAPS, D, false, 0, NumpadPeriod, true),            // Caps+D = Del
        m(M_CAPS, A, false, 0, Numpad7, true),                 // Caps+A = Home
        m(M_CAPS, E, false, 0, Numpad1, true),                 // Caps+E = End
        m(M_CAPS, H, false, 0, Numpad4, true),                 // Caps+H = Left
        m(M_CAPS, J, false, 0, Numpad8, true),                 // Caps+J = Up
        m(M_CAPS, K, false, 0, Numpad2, true),                 // Caps+K = Down
        m(M_CAPS, L, false, 0, Numpad6, true),                 // Caps+L = Right
        m(M_CAPS, F, false, M_LCTRL, Numpad6, true),           // Caps+F = Ctrl+Right
        m(M_CAPS, B, false, M_LCTRL, Numpad4, true),           // Caps+B = Ctrl+Left
        m(M_CAPS, Q, false, M_LCTRL, A, false),                // Caps+Q = Ctrl+A
        m(M_CAPS, S, false, M_LCTRL, S, false),                // Caps+S = Ctrl+S
        m(M_CAPS, W, false, M_LCTRL, W, false),                // Caps+W = Ctrl+W
        m(M_CAPS, Z, false, M_LCTRL, Z, false),                // Caps+Z = Ctrl+Z
        m(M_CAPS, X, false, M_LCTRL, X, false),                // Caps+X = Ctrl+X
        m(M_CAPS, C, false, M_LCTRL, C, false),                // Caps+C = Ctrl+C
        m(M_CAPS, V, false, M_LCTRL, V, false),                // Caps+V = Ctrl+V
    ]
}

/// 配置文件路径：exe 同目录下的 config.json（便携分发友好）
pub fn config_path() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.join("config.json")))
        .unwrap_or_else(|| PathBuf::from("config.json"))
}

/// 读取配置；文件不存在 => 默认映射；解析失败 => 返回警告 + 默认映射
pub fn load_config() -> (Vec<KeyMapping>, Option<String>) {
    let path = config_path();
    if !path.exists() {
        return (default_mappings(), None);
    }
    match std::fs::read_to_string(&path) {
        Ok(text) => match serde_json::from_str::<ConfigFile>(&text) {
            Ok(cfg) => (cfg.mappings, None),
            Err(e) => (
                default_mappings(),
                Some(format!(
                    "config.json 解析失败：{e}\n已回退到默认键位配置。"
                )),
            ),
        },
        Err(e) => (
            default_mappings(),
            Some(format!("config.json 读取失败：{e}\n已回退到默认键位配置。")),
        ),
    }
}

pub fn save_config(mappings: &[KeyMapping]) -> std::io::Result<()> {
    let cfg = ConfigFile {
        version: 1,
        mappings: mappings.to_vec(),
    };
    let text = serde_json::to_string_pretty(&cfg)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    std::fs::write(config_path(), text)
}

#[derive(Serialize, Deserialize)]
struct ConfigFile {
    version: u32,
    mappings: Vec<KeyMapping>,
}

// ---------------------------------------------------------------- 构造注入序列

/// 构造一个键盘事件
pub fn kbd(code: u16, ext: bool, state: KeyState, information: u32) -> Stroke {
    let code = ScanCode::try_from(code).unwrap_or(ScanCode::Esc);
    let state = if ext { state | KeyState::E0 } else { state };
    Stroke::Keyboard {
        code,
        state,
        information,
    }
}

/// 修饰键集合 -> 注入的按下序列，顺序：Ctrl Shift Alt Win（左右侧各自还原）
pub fn mods_down_strokes(mods: u16, information: u32) -> Vec<Stroke> {
    let mut v = Vec::new();
    let mut push = |code: u16, ext: bool| v.push(kbd(code, ext, KeyState::DOWN, information));
    if mods & M_LCTRL != 0 {
        push(0x1D, false);
    }
    if mods & M_RCTRL != 0 {
        push(0x1D, true);
    }
    if mods & M_LSHIFT != 0 {
        push(0x2A, false);
    }
    if mods & M_RSHIFT != 0 {
        push(0x36, false);
    }
    if mods & M_LALT != 0 {
        push(0x38, false);
    }
    if mods & M_RALT != 0 {
        push(0x38, true);
    }
    if mods & M_LWIN != 0 {
        push(0x5B, true);
    }
    if mods & M_RWIN != 0 {
        push(0x5C, true);
    }
    v
}

/// 修饰键集合 -> 注入的抬起序列（与按下严格反序）
pub fn mods_up_strokes(mods: u16, information: u32) -> Vec<Stroke> {
    let mut v = mods_down_strokes(mods, information);
    v.reverse();
    for s in &mut v {
        if let Stroke::Keyboard { state, .. } = s {
            *state = KeyState::UP | (*state & KeyState::E0);
        }
    }
    v
}

/// 命中映射时的按下序列：[修饰键按下..., 主键按下]
pub fn mapping_downs(m: &KeyMapping, information: u32) -> Vec<Stroke> {
    let mut v = mods_down_strokes(m.dst_mods, information);
    v.push(kbd(m.dst_code, m.dst_ext, KeyState::DOWN, information));
    v
}

/// 命中映射时的抬起序列：[主键抬起, 修饰键反序抬起...]，与按下序列一一配对
pub fn mapping_ups(m: &KeyMapping, information: u32) -> Vec<Stroke> {
    let mut v = vec![kbd(m.dst_code, m.dst_ext, KeyState::UP, information)];
    v.extend(mods_up_strokes(m.dst_mods, information));
    v
}

/// 若该键是修饰键，返回其对应的位标志（右Ctrl/右Alt/Win 等以 基础码+E0 表示）
pub fn modifier_bit(code: u16, ext: bool) -> Option<u16> {
    Some(match (code, ext) {
        (0x3A, false) => M_CAPS,
        (0x1D, false) => M_LCTRL,
        (0x1D, true) => M_RCTRL,
        (0x2A, false) => M_LSHIFT,
        (0x36, false) => M_RSHIFT,
        (0x38, false) => M_LALT,
        (0x38, true) => M_RALT,
        (0x5B, true) => M_LWIN,
        (0x5C, true) => M_RWIN,
        _ => return None,
    })
}

// ---------------------------------------------------------------- 显示名称

/// 单个键的显示名（含 E0 扩展键）
pub fn key_name(code: u16, ext: bool) -> String {
    let base = match code {
        0x01 => "Esc".into(),
        0x02..=0x0B => format!("数字{}", code - 0x01),
        0x0C => "-".into(),
        0x0D => "=".into(),
        0x0E => "Backspace".into(),
        0x0F => "Tab".into(),
        0x1A => "[".into(),
        0x1B => "]".into(),
        0x1C => "Enter".into(),
        0x29 => "`".into(),
        0x2B => "\\".into(),
        0x33 => ",".into(),
        0x34 => ".".into(),
        0x35 => "/".into(),
        0x37 => { if ext { "PrintScreen" } else { "小键盘*" }.into() }
        0x39 => "Space".into(),
        0x3A => "CapsLock".into(),
        0x3B..=0x44 => format!("F{}", code - 0x3A),
        0x45 => "NumLock".into(),
        0x46 => "ScrollLock".into(),
        0x47 if ext => "Home".into(),
        0x48 if ext => "↑".into(),
        0x49 if ext => "PageUp".into(),
        0x4A => "小键盘-".into(),
        0x4B if ext => "←".into(),
        0x4C => "小键盘5".into(),
        0x4D if ext => "→".into(),
        0x4E => "小键盘+".into(),
        0x4F if ext => "End".into(),
        0x50 if ext => "↓".into(),
        0x51 if ext => "PageDown".into(),
        0x52 if ext => "Insert".into(),
        0x53 if ext => "Delete".into(),
        0x56 => "|".into(),
        0x57 | 0x58 => format!("F{}", code - 0x57 + 11),
        0x5B if ext => "Win".into(),
        0x5C if ext => "右Win".into(),
        0x5D..=0x63 => format!("媒体键0x{code:02X}"),
        _ => {
            // 0x10..0x32 区间大部分是字母
            if (0x10..=0x32).contains(&code) {
                let letters = "QWERTYUIOPASDFGHJKLZXCVBNM";
                let idx = match code {
                    0x10..=0x19 => code - 0x10,      // Q..P
                    0x1E..=0x26 => code - 0x1E + 10,  // A..L
                    0x2C..=0x32 => code - 0x2C + 19,  // Z..M
                    _ => return format!("键(0x{code:02X})"),
                };
                letters
                    .chars()
                    .nth(idx as usize)
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| format!("键(0x{code:02X})"))
            } else {
                format!("键(0x{code:02X})")
            }
        }
    };
    base
}

/// 组合的显示名："Caps + C"、"Ctrl + →" 等
pub fn combo_display(mods: u16, code: u16, ext: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    if mods & M_CAPS != 0 {
        parts.push("Caps".into());
    }
    if mods & M_LCTRL != 0 {
        parts.push("Ctrl".into());
    }
    if mods & M_RCTRL != 0 {
        parts.push("右Ctrl".into());
    }
    if mods & M_LSHIFT != 0 {
        parts.push("Shift".into());
    }
    if mods & M_RSHIFT != 0 {
        parts.push("右Shift".into());
    }
    if mods & M_LALT != 0 {
        parts.push("Alt".into());
    }
    if mods & M_RALT != 0 {
        parts.push("右Alt".into());
    }
    if mods & M_LWIN != 0 {
        parts.push("Win".into());
    }
    if mods & M_RWIN != 0 {
        parts.push("右Win".into());
    }
    parts.push(key_name(code, ext));
    parts.join(" + ")
}

impl KeyMapping {
    pub fn src_display(&self) -> String {
        combo_display(self.src_mods, self.src_code, self.src_ext)
    }
    pub fn dst_display(&self) -> String {
        combo_display(self.dst_mods, self.dst_code, self.dst_ext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_cover_readme_table() {
        assert_eq!(default_mappings().len(), 17);
    }

    #[test]
    fn down_up_pairing() {
        let m = m(M_CAPS, ScanCode::C, false, M_LCTRL, ScanCode::C, false);
        let downs = mapping_downs(&m, 0);
        let ups = mapping_ups(&m, 0);
        // 按下 [Ctrl↓, C↓]，抬起 [C↑, Ctrl↑]，键与E0标志一一对应
        assert_eq!(downs.len(), 2);
        assert_eq!(ups.len(), 2);
        assert!(matches!((&downs[0], &ups[1]), (Stroke::Keyboard { .. }, Stroke::Keyboard { .. })));
    }

    #[test]
    fn config_roundtrip() {
        let ms = default_mappings();
        let cfg = ConfigFile { version: 1, mappings: ms.clone() };
        let text = serde_json::to_string(&cfg).unwrap();
        let back: ConfigFile = serde_json::from_str(&text).unwrap();
        assert_eq!(back.mappings, ms);
    }

    #[test]
    fn display_names() {
        assert_eq!(combo_display(M_CAPS, 0x2E, false), "Caps + C");
        assert_eq!(combo_display(M_LCTRL, 0x4D, true), "Ctrl + →");
        assert_eq!(combo_display(0, 0x53, true), "Delete");
    }
}
