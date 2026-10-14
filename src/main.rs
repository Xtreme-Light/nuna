#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! 该程序使用 kanata_interception 库拦截键盘事件，实现 CapsLock 组合键的增强映射。
//! 映射表可通过托盘菜单"按键配置"打开的配置窗口按按键采集（类似 SharpKeys），
//! 保存到 exe 同目录的 config.json 并即时生效；无配置文件时使用内置默认映射。
//!
//! 注入采用"按下/抬起严格配对"的方式：每个注入的 DOWN 都保证有配对的 UP，
//! 避免本机或云桌面/远程桌面这类只透传按键流的会话出现按键卡住（影子按键）的问题。

// 导入模块
mod config_ui;
mod keys;
mod mapping;
mod oscode;
mod tray;

// 导入所需的外部库和模块
use crate::mapping::{CaptureMsg, KeyMapping};
use crate::tray::init_tray;
use anyhow::Result;
use crossbeam_channel::{Receiver, Sender, unbounded};
use kanata_interception as ic;
use kanata_interception::{Device, Interception, KeyState, ScanCode, Stroke};
use log::LevelFilter;
use simplelog::{ColorChoice, CombinedLogger, ConfigBuilder, TermLogger, TerminalMode};
use single_instance::SingleInstance;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use windows::core::PCWSTR;
use windows::Win32::Foundation::LPARAM;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyNameTextW, VK_DELETE, VK_DOWN, VK_END, VK_HOME, VK_INSERT, VK_LCONTROL,
    VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_NEXT, VK_PRIOR, VK_RCONTROL, VK_RIGHT, VK_RMENU,
    VK_RSHIFT, VK_RWIN, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

/// 全局共享状态：拦截线程、托盘、配置窗口三者之间的通信载体
pub struct AppShared {
    /// 当前生效的映射表（配置窗口保存后更新，拦截线程每个按键事件读取）
    pub mappings: Mutex<Vec<KeyMapping>>,
    /// 采集模式：开启后拦截线程只记录按下的组合并上报，不下发任何按键
    pub capture_on: AtomicBool,
    /// 采集结果通道（拦截线程 -> 配置窗口）
    pub capture_tx: Sender<CaptureMsg>,
    pub capture_rx: Receiver<CaptureMsg>,
    /// 配置窗口是否已打开（避免重复开窗）
    pub ui_open: AtomicBool,
}

/// 程序入口函数
fn main() -> Result<()> {
    // 无控制台的 GUI 程序：任何 panic 都弹窗展示，而不是无声退出只剩退出码
    std::panic::set_hook(Box::new(|info| {
        fatal_dialog("nuna 发生内部错误", &info.to_string());
    }));

    let instance = SingleInstance::new("nuna.exe")?;
    if !instance.is_single() {
        fatal_dialog("nuna", "已有实例正在运行，请勿重复启动。");
        return Err(anyhow::anyhow!("已有单例正在执行，请勿重复启动"));
    };

    init_log();

    log::info!("程序启动中...");

    // 加载键位映射配置（无文件时使用默认）
    let (mappings, warn) = mapping::load_config();
    if let Some(w) = warn {
        log::warn!("{}", w);
        fatal_dialog("nuna 配置文件异常", &w);
    }
    log::info!("已加载 {} 条按键映射", mappings.len());

    let (capture_tx, capture_rx) = unbounded();
    let shared = Arc::new(AppShared {
        mappings: Mutex::new(mappings),
        capture_on: AtomicBool::new(false),
        capture_tx,
        capture_rx,
        ui_open: AtomicBool::new(false),
    });

    // 创建退出信号通道
    let (exit_tx, exit_rx) = unbounded();

    // 启动键盘拦截线程
    {
        let shared = shared.clone();
        std::thread::spawn(move || {
            if let Err(e) = keyboard_interceptor(exit_rx, shared) {
                log::error!("键盘拦截线程出错: {}", e);
                // 无控制台的 GUI 程序，致命错误必须弹窗告知并退出，
                // 否则只会留下一个无意义的退出码（如 0xC0000409）
                fatal_dialog("nuna 运行出错", &format!("{e:#}"));
                std::process::exit(1);
            }
        });
    }

    // 初始化系统托盘
    let _tray_icon = init_tray(exit_tx, shared)?;
    log::info!("系统托盘初始化完成");

    Ok(())
}

/// caps 层内一条已注入的映射：记录原始键码、命中的映射以及需要抵消的真实修饰键，
/// 用于在任意松开顺序下构造与按下序列严格配对的抬起序列
struct InjectedKey {
    dev: Device,
    /// 原始物理键（扫描码 + E0），用于把后续 UP/重复 DOWN 路由回来
    code: u16,
    ext: bool,
    /// 命中的映射（抬起序列由它生成）
    mapping: KeyMapping,
    /// 命中时被抵消抬起的真实修饰键位（Caps 不经过系统，无需抵消）
    suppress_bits: u16,
    /// 按下序列（重复 DOWN 时重发）
    downs: Vec<Stroke>,
}

impl InjectedKey {
    /// 抬起序列：目标组合反序抬起 + 把仍被物理按住的抵消修饰键恢复按下。
    /// restore_bits 由调用方根据"此刻仍按住的修饰键"计算
    fn ups(&self, restore_bits: u16, information: u32) -> Vec<Stroke> {
        let mut v = mapping::mapping_ups(&self.mapping, information);
        if restore_bits != 0 {
            v.extend(mapping::mods_down_strokes(restore_bits, information));
        }
        v
    }
}

fn keyboard_interceptor(exit_rx: Receiver<()>, shared: Arc<AppShared>) -> Result<()> {
    log::info!("等待所有的键释放");
    // 动态等待直到所有按键释放
    init_keyboard_state(); // Call once
    loop {
        if are_all_keys_released() {
            log::info!("所有按键均已释放，程序开始");
            break;
        }
        // 短间隔轮询，减少 CPU 占用
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // 初始化 Interception 驱动
    // 若驱动未安装则弹窗提示用户安装方法（返回 Err 由线程统一弹窗）
    let intercept = match Interception::new() {
        Some(intercept) => intercept,
        None => {
            return Err(anyhow::anyhow!(
                "未找到 interception 驱动（本机未安装或未重启生效）。\n\
                 安装步骤：\n\
                 1. 下载 https://github.com/oblitum/Interception\n\
                 2. 使用管理员权限打开命令行工具\n\
                 3. 在 interception 所在文件夹运行 install-interception /install\n\
                 4. 重启电脑生效"
            ));
        }
    };

    // 设置拦截过滤器：拦截所有键盘事件
    intercept.set_filter(ic::is_keyboard, ic::Filter::KeyFilter(ic::KeyFilter::all()));

    // 初始化键盘事件缓冲区
    let mut strokes = [Stroke::Keyboard {
        code: ScanCode::Esc,
        state: KeyState::empty(),
        information: 0,
    }; 32];

    log::info!("interception 驱动已加载，开始监听键盘事件...");
    // 当前物理按住的修饰键位集合
    let mut held_mods: u16 = 0;
    // 已注入且尚未抬起的键（DOWN/UP 配对表）
    let mut injected: Vec<InjectedKey> = Vec::new();

    loop {
        // 检查退出信号
        if exit_rx.try_recv().is_ok() {
            log::info!("收到退出信号，停止键盘拦截");
            // 退出前抬掉所有仍按下的注入键，避免退出后残留卡键
            release_all_injected(&intercept, &mut injected);
            return Ok(());
        }
        // 等待键盘事件，超时时间为 1 毫秒（避免阻塞过久）
        let dev = intercept.wait_with_timeout(std::time::Duration::from_millis(1));

        // 若检测到有效设备（dev > 0 表示有键盘事件）
        if dev > 0 {
            let num_strokes = intercept.receive(dev, &mut strokes) as usize;

            for i in 0..num_strokes {
                let original_stroke = strokes[i];
                let (code, ext, state, information) = match original_stroke {
                    Stroke::Keyboard {
                        code,
                        state,
                        information,
                    } => (code as u16, state.contains(KeyState::E0), state, information),
                    _ => {
                        // 非键盘事件原样放行
                        intercept.send(dev, &[original_stroke]);
                        continue;
                    }
                };
                let is_up = state.contains(KeyState::UP);

                // ---- 1) 修饰键：维护按住状态；采集模式下吞掉不下发 ----
                if let Some(bit) = mapping::modifier_bit(code, ext) {
                    if shared.capture_on.load(Ordering::Relaxed) {
                        // 采集期间修饰键不下发（避免误触发 CapsLock 切换/系统快捷键），只记录状态
                        if is_up {
                            held_mods &= !bit;
                        } else {
                            held_mods |= bit;
                        }
                        continue;
                    }
                    if is_up {
                        held_mods &= !bit;
                        // 命中组合里包含该修饰键的注入条目必须立即抬起：
                        // 用户可能先松修饰键再松主键（例如先松 Caps 再松 C），
                        // 若等主键自己的 UP，注入的 Ctrl/方向键就永远停在按下状态
                        flush_entries_with_mod(&intercept, &mut injected, bit, held_mods);
                    } else {
                        held_mods |= bit;
                    }
                    // 修饰键本身原样放行
                    intercept.send(dev, &[original_stroke]);
                    continue;
                }

                // ---- 2) 采集模式：非修饰键即采集结果（Esc 取消），期间全部吞掉 ----
                if shared.capture_on.load(Ordering::Relaxed) {
                    if !is_up {
                        let msg = if code == ScanCode::Esc as u16 {
                            CaptureMsg::Cancelled
                        } else {
                            CaptureMsg::Combo {
                                mods: held_mods,
                                code,
                                ext,
                            }
                        };
                        let _ = shared.capture_tx.send(msg);
                        shared.capture_on.store(false, Ordering::Relaxed);
                    }
                    continue;
                }

                // ---- 3) 已注入过的键：UP 配对补发 / 重复 DOWN 重发 ----
                if let Some(pos) = injected
                    .iter()
                    .position(|e| e.code == code && e.ext == ext)
                {
                    if is_up {
                        let entry = injected.remove(pos);
                        // 仍被物理按住的抵消修饰键要恢复按下（如 Ctrl+X→Home 松开 X 时 Ctrl 还按着）
                        let restore_bits = entry.suppress_bits & held_mods;
                        intercept.send(entry.dev, &entry.ups(restore_bits, information));
                    } else {
                        // 硬件自动重复产生的 DOWN：重发按下序列，保留连续触发的手感
                        let entry = &injected[pos];
                        intercept.send(entry.dev, &entry.downs);
                    }
                    continue;
                }

                // ---- 4) 查映射表：按下的组合(修饰键集合+主键)精确匹配 ----
                if !is_up {
                    let hit = shared
                        .mappings
                        .lock()
                        .unwrap()
                        .iter()
                        .find(|m| m.src_mods == held_mods && m.src_code == code && m.src_ext == ext)
                        .copied();
                    if let Some(m) = hit {
                        // 原始组合里的真实修饰键（非 Caps）此前已放行、系统认为处于按下态，
                        // 注入目标组合前要先抵消抬起，否则会变成 修饰键+目标键 的叠加（如 Ctrl+Home）
                        let suppress_bits = m.src_mods & !mapping::M_CAPS;
                        if suppress_bits != 0 {
                            intercept.send(
                                dev,
                                &mapping::mods_up_strokes(suppress_bits, information),
                            );
                        }
                        let downs = mapping::mapping_downs(&m, information);
                        intercept.send(dev, &downs);
                        injected.push(InjectedKey {
                            dev,
                            code,
                            ext,
                            mapping: m,
                            suppress_bits,
                            downs,
                        });
                        continue;
                    }
                }

                // ---- 5) 其余事件原样放行 ----
                intercept.send(dev, &[original_stroke]);
            }
        }
    }
}

/// 按后按先抬的顺序，把所有仍处于按下状态的注入键全部抬起
fn release_all_injected(intercept: &Interception, injected: &mut Vec<InjectedKey>) {
    if !injected.is_empty() {
        log::info!("抬起 {} 个仍处于按下状态的注入键", injected.len());
    }
    while let Some(entry) = injected.pop() {
        // 全量抬起时不恢复抵消修饰键（通常来自退出/兜底路径）
        intercept.send(entry.dev, &mapping::mapping_ups(&entry.mapping, 0));
    }
}

/// 抬起所有"命中组合包含指定修饰键位"的注入条目（该修饰键被松开时调用）。
/// held_mods 为松开该键后的剩余按住集合，用于决定是否恢复其它抵消修饰键
fn flush_entries_with_mod(
    intercept: &Interception,
    injected: &mut Vec<InjectedKey>,
    bit: u16,
    held_mods: u16,
) {
    let mut i = injected.len();
    while i > 0 {
        i -= 1;
        if injected[i].mapping.src_mods & bit != 0 {
            let entry = injected.remove(i);
            let restore_bits = entry.suppress_bits & held_mods;
            intercept.send(entry.dev, &entry.ups(restore_bits, 0));
        }
    }
}

fn init_log() {
    // 配置日志系统
    // 尝试将日志时间设置为本地时间，若失败则输出警告
    let mut log_config = ConfigBuilder::new();
    if let Err(e) = log_config.set_time_offset_to_local() {
        eprintln!("警告: 无法进行日期本地化: {e:?}");
    };

    // 初始化组合日志器，使用终端日志输出
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Info,   // 日志级别为 Info
        log_config.build(),  // 使用上述配置
        TerminalMode::Mixed, // 混合模式（ stdout 和 stderr）
        ColorChoice::Auto,   // 自动选择颜色（根据终端支持）
    )])
    .expect("日志初始化失败"); // 若初始化失败则终止程序并提示
    log::info!("日志初始化成功");
}

/// 无控制台的 GUI 程序，致命错误必须弹窗告知用户，
/// 否则 panic/abort 只会留下一个无意义的退出码
pub(crate) fn fatal_dialog(title: &str, text: &str) {
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    let title = wide(title);
    let text = wide(text);
    unsafe {
        MessageBoxW(
            None,
            PCWSTR::from_raw(text.as_ptr()),
            PCWSTR::from_raw(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

/// 检查当前是否所有按键都处于释放状态
static CLEARED_WEIRD: std::sync::Once = std::sync::Once::new();

pub fn init_keyboard_state() {
    CLEARED_WEIRD.call_once(|| {
        let weird = [
            VK_HOME,
            VK_END,
            VK_PRIOR,
            VK_NEXT,
            VK_INSERT,
            VK_DELETE,
            VK_LEFT,
            VK_RIGHT,
            VK_UP,
            VK_DOWN,
            VK_LCONTROL,
            VK_RCONTROL,
            VK_LSHIFT,
            VK_RSHIFT,
            VK_LMENU,
            VK_RMENU, // Alt
            VK_LWIN,
            VK_RWIN, // Win
        ];
        for &vk in &weird {
            unsafe {
                windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(i32::from(vk.0));
            }
        }
    });
}

/// 启动等待所有按键释放时使用的快速检测
pub fn are_all_keys_released() -> bool {
    const WEIRD: [u16; 18] = [
        0x21,
        0x22,
        0x23,
        0x24, // Prior, Next, End, Home
        0x25,
        0x26,
        0x27,
        0x28, // Left, Up, Right, Down
        0x2D,
        0x2E,
        VK_LCONTROL.0,
        VK_RCONTROL.0,
        VK_LSHIFT.0,
        VK_RSHIFT.0,
        VK_LMENU.0,
        VK_RMENU.0,
        VK_LWIN.0,
        VK_RWIN.0,
    ];

    for code in 1u16..=255u16 {
        let state = unsafe {
            windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(i32::from(code))
        };
        let pressed = if WEIRD.contains(&code) {
            (state & 1) != 0 // "was pressed" bit
        } else {
            state < 0 // normal "currently down"
        };
        if pressed {
            return false;
        }
    }
    true
}

//  获取键位状态，false没有按下，true 按下
//  卡键修复已改为确定性记账（InjectedKey 配对表），不再依赖 GetAsyncKeyState 探测，
//  该函数目前仅测试使用
#[allow(unused)]
pub fn is_key_down(vk: windows::Win32::UI::Input::KeyboardAndMouse::VIRTUAL_KEY) -> bool {
    const WEIRD_KEYS: [u16; 8] = [
        VK_HOME.0,
        VK_END.0,
        VK_PRIOR.0,
        VK_NEXT.0,
        VK_INSERT.0,
        VK_DELETE.0,
        VK_LEFT.0,
        VK_RIGHT.0,
    ];

    let code = vk.0;
    unsafe {
        let state = GetAsyncKeyState(i32::from(code));
        if WEIRD_KEYS.contains(&code) {
            state & 1 != 0
        } else {
            state < 0
        }
    }
}
#[cfg(test)]
mod tests {
    use crate::is_key_down;
    use windows::Win32::UI::Input::KeyboardAndMouse::*;

    #[test]
    fn test_key_states() {
        // Give you a moment to release any keys
        std::thread::sleep(std::time::Duration::from_millis(200));

        let home = is_key_down(VK_HOME);
        let shift = is_key_down(VK_LSHIFT);
        let a = is_key_down(VK_A);

        println!("HOME: {home}   LSHIFT: {shift}   A: {a}");

        assert!(!home, "Home key is reported as down but it should be up");
    }
}

/// 通过虚拟键码获取键的名称（如 "A", "Left Ctrl", "Mouse Left" 等）
#[allow(unused)]
fn get_key_name(vk_code: u16) -> String {
    // 构造 lParam 参数（低 16 位为虚拟键码，高 16 位为扩展键标志）
    let lparam = LPARAM((vk_code as isize) << 16);
    let mut buffer = [0u16; 256]; // 存储宽字符结果

    // 调用 Windows API 获取键名
    let length = unsafe { GetKeyNameTextW(lparam.0 as i32, &mut buffer) };

    if length > 0 {
        // 将宽字符串转换为 Rust 字符串
        String::from_utf16_lossy(&buffer[..length as usize])
    } else {
        format!("未知键 (VK_CODE: 0x{:02X})", vk_code)
    }
}
