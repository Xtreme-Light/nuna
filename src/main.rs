#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! 该程序使用 kanata_interception 库拦截键盘事件，实现将 CapsLock 键映射为 Left Ctrl 键的功能。
//! 映射采用"按下/抬起严格配对"的方式注入：每个注入的 DOWN 都保证有配对的 UP，
//! 避免本机或云桌面/远程桌面这类只透传按键流的会话出现按键卡住（影子按键）的问题。

// 导入模块
mod keys;
mod oscode;
mod tray;

// 导入所需的外部库和模块
use crate::tray::init_tray;
use anyhow::Result;
use crossbeam_channel::{Receiver, unbounded};
use kanata_interception as ic;
use kanata_interception::{Device, Interception, KeyState, ScanCode, Stroke};
use log::LevelFilter;
use simplelog::{ColorChoice, CombinedLogger, ConfigBuilder, TermLogger, TerminalMode};
use single_instance::SingleInstance;
use windows::core::PCWSTR;
use windows::Win32::Foundation::LPARAM;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyNameTextW, VK_DELETE, VK_DOWN, VK_END, VK_HOME, VK_INSERT, VK_LCONTROL,
    VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_NEXT, VK_PRIOR, VK_RCONTROL, VK_RIGHT, VK_RMENU,
    VK_RSHIFT, VK_RWIN, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONERROR, MB_OK};

/// 程序入口函数
/// 初始化日志系统、拦截驱动，然后进入事件循环处理键盘事件
fn main() -> Result<()> {
    let instance = SingleInstance::new("nuna.exe")?;
    if !instance.is_single() {
        fatal_dialog("nuna", "已有实例正在运行，请勿重复启动。");
        return Err(anyhow::anyhow!("已有单例正在执行，请勿重复启动"));
    };

    init_log();

    log::info!("程序启动中...");

    // 创建退出信号通道
    let (exit_tx, exit_rx) = unbounded();

    // 启动键盘拦截线程
    std::thread::spawn(move || {
        if let Err(e) = keyboard_interceptor(exit_rx) {
            log::error!("键盘拦截线程出错: {}", e);
            // 无控制台的 GUI 程序，致命错误必须弹窗告知并退出，
            // 否则只会留下一个无意义的退出码（如 0xC0000409）
            fatal_dialog("nuna 运行出错", &format!("{e:#}"));
            std::process::exit(1);
        }
    });

    // 初始化系统托盘
    let _tray_icon = init_tray(exit_tx)?;
    log::info!("系统托盘初始化完成");

    Ok(())
}

fn keyboard_interceptor(exit_rx: Receiver<()>) -> Result<()> {
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

    // 初始化键盘事件缓冲区，用于接收拦截到的事件
    // 缓冲区大小为 32，初始值为 Esc 键的空状态（仅用于初始化，实际会被覆盖）
    let mut strokes = [Stroke::Keyboard {
        code: ScanCode::Esc,
        state: KeyState::empty(),
        information: 0,
    }; 32];

    log::info!("interception 驱动已加载，开始监听键盘事件...");
    // 用于跟踪CapsLock(已映射为虚拟键)的按下状态
    let mut caps_down = false;
    // caps 层内已注入且尚未抬起的键：保证每个注入的 DOWN 最终都有配对的 UP
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
            // 接收设备发送的键盘事件，存储到缓冲区
            let num_strokes = intercept.receive(dev, &mut strokes) as usize;

            // 遍历处理每个接收到的事件
            for i in 0..num_strokes {
                let original_stroke = strokes[i]; // 复制当前事件（用于可能的修改）
                if let Stroke::Keyboard {
                    code,
                    state,
                    information,
                } = original_stroke
                {
                    let is_up = state.contains(KeyState::UP);

                    // CapsLock 本身不下发，只用来维护 caps 层的开关状态
                    if code == ScanCode::CapsLock {
                        // 松开（或再次按下）caps 时，立即抬掉该层所有仍按着的注入键。
                        // 不能依赖各按键自己的 UP 事件：用户可能先松 caps 再松按键，
                        // 那时 caps_down 已变回 false，UP 会被原样放行，之前注入的
                        // Ctrl/方向键就永远停在按下状态 —— 这就是影子按键/卡 Ctrl 的根源
                        release_all_injected(&intercept, &mut injected);
                        caps_down = !is_up;
                        continue;
                    }

                    // 按下时被映射注入过的键：无论此刻 caps 是否还按着，
                    // UP/重复 DOWN 一律按当初注入的内容配对处理
                    if let Some(pos) = injected.iter().position(|e| e.code == code as u8) {
                        if is_up {
                            let entry = injected.remove(pos);
                            intercept.send(entry.dev, &entry.ups);
                        } else {
                            // 硬件自动重复产生的 DOWN：重发按下序列，保留连续触发的手感
                            let entry = &injected[pos];
                            intercept.send(entry.dev, &entry.downs);
                        }
                        continue;
                    }

                    // caps 层内新按下的可映射键：成对生成 按下/抬起 序列并记录
                    if caps_down && !is_up {
                        if let Some((downs, ups)) = build_mapping(code, information) {
                            intercept.send(dev, &downs);
                            injected.push(InjectedKey {
                                dev,
                                code: code as u8,
                                downs,
                                ups,
                            });
                            continue;
                        }
                    }
                }

                // 其余事件（未映射的按键、以及各种未被上面命中的情况）原样放行
                intercept.send(dev, &[original_stroke]);
            }
        }
    }
}
/// 无控制台的 GUI 程序，致命错误必须弹窗告知用户，
/// 否则 panic/abort 只会留下一个无意义的退出码
fn fatal_dialog(title: &str, text: &str) {
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

fn init_log() {    // 配置日志系统
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

/// caps 层内一条已注入的映射：记录原始键码、注入所用的设备，
/// 以及和按下序列严格配对的抬起序列
struct InjectedKey {
    dev: Device,
    code: u8,
    downs: Vec<Stroke>,
    ups: Vec<Stroke>,
}

/// 按后按先抬的顺序，把所有仍处于按下状态的注入键全部抬起
fn release_all_injected(intercept: &Interception, injected: &mut Vec<InjectedKey>) {
    if !injected.is_empty() {
        log::info!("抬起 {} 个仍处于按下状态的注入键", injected.len());
    }
    while let Some(entry) = injected.pop() {
        intercept.send(entry.dev, &entry.ups);
    }
}

/// 为 caps 层内按下的键生成 (按下序列, 抬起序列)。
/// 两条序列必须一一配对，否则接收端——无论是本机应用，还是云桌面/远程桌面
/// 这类只透传按键流的会话——都会认为按键一直被按住，表现为 Ctrl 卡键等影子按键
fn build_mapping(code: ScanCode, information: u32) -> Option<(Vec<Stroke>, Vec<Stroke>)> {
    // E0 扩展序列（小键盘扫描码 + E0 = 方向/编辑键），抬起状态按实测日志补上 E0|E1
    let e0_down = e0_extra_key_state(KeyState::DOWN); // => E0
    let e0_up = e0_extra_key_state(KeyState::UP); // => UP|E0|E1

    // 单键替换：按下和抬起使用同一个扫描码
    let single =
        |mapped: ScanCode, down: KeyState, up: KeyState| -> (Vec<Stroke>, Vec<Stroke>) {
            (
                vec![Stroke::Keyboard {
                    code: mapped,
                    state: down,
                    information,
                }],
                vec![Stroke::Keyboard {
                    code: mapped,
                    state: up,
                    information,
                }],
            )
        };
    // ctrl 组合键：按下 [ctrl, key]，抬起 [key, ctrl]，顺序严格相反。
    // 注意：左 Ctrl 的扫描码(0x1D)不能带 E0，E0|1D 是右 Ctrl；
    // 混用会导致本机与远程会话两侧的修饰键状态对不上，是卡 Ctrl 的诱因之一
    let with_ctrl =
        |mapped: ScanCode, down: KeyState, up: KeyState| -> (Vec<Stroke>, Vec<Stroke>) {
            (
                vec![
                    Stroke::Keyboard {
                        code: ScanCode::LeftControl,
                        state: KeyState::DOWN,
                        information,
                    },
                    Stroke::Keyboard {
                        code: mapped,
                        state: down,
                        information,
                    },
                ],
                vec![
                    Stroke::Keyboard {
                        code: mapped,
                        state: up,
                        information,
                    },
                    Stroke::Keyboard {
                        code: ScanCode::LeftControl,
                        state: KeyState::UP,
                        information,
                    },
                ],
            )
        };

    match code {
        // Caps + Space = Backspace
        ScanCode::Space => Some(single(ScanCode::Backspace, KeyState::DOWN, KeyState::UP)),
        // Caps + D = Del
        ScanCode::D => Some(single(ScanCode::NumpadPeriod, e0_down, e0_up)),
        // Caps + A = Home
        ScanCode::A => Some(single(ScanCode::Numpad7, e0_down, e0_up)),
        // Caps + E = End
        ScanCode::E => Some(single(ScanCode::Numpad1, e0_down, e0_up)),
        // 方向左键
        ScanCode::H => Some(single(ScanCode::Numpad4, e0_down, e0_up)),
        // 方向右键
        ScanCode::L => Some(single(ScanCode::Numpad6, e0_down, e0_up)),
        // 方向上键
        ScanCode::J => Some(single(ScanCode::Numpad8, e0_down, e0_up)),
        // 方向下键
        ScanCode::K => Some(single(ScanCode::Numpad2, e0_down, e0_up)),
        // Caps + B = Ctrl + Left
        ScanCode::B => Some(with_ctrl(ScanCode::Numpad4, e0_down, e0_up)),
        // Caps + F = Ctrl + Right
        ScanCode::F => Some(with_ctrl(ScanCode::Numpad6, e0_down, e0_up)),
        // Caps + Z/X/C/V/W = Ctrl + 对应键
        ScanCode::Z => Some(with_ctrl(ScanCode::Z, KeyState::DOWN, KeyState::UP)),
        ScanCode::X => Some(with_ctrl(ScanCode::X, KeyState::DOWN, KeyState::UP)),
        ScanCode::C => Some(with_ctrl(ScanCode::C, KeyState::DOWN, KeyState::UP)),
        ScanCode::V => Some(with_ctrl(ScanCode::V, KeyState::DOWN, KeyState::UP)),
        ScanCode::W => Some(with_ctrl(ScanCode::W, KeyState::DOWN, KeyState::UP)),
        // Caps + Q = Ctrl + A
        ScanCode::Q => Some(with_ctrl(ScanCode::A, KeyState::DOWN, KeyState::UP)),
        // Caps + S = Ctrl + S
        ScanCode::S => Some(with_ctrl(ScanCode::S, KeyState::DOWN, KeyState::UP)),
        _ => None,
    }
}

// 2. 处理E0扩展键序列（左方向键的核心逻辑）
// 日志显示：E0序列以LeftShift(0x2a, state含E0)开头，Numpad4(0x4b, state含E0)跟进
fn e0_extra_key_state(state: KeyState) -> KeyState {
    let new_state = if state.contains(KeyState::UP) {
        // 释放事件：保留E0和UP标志（匹配日志中的state格式）
        KeyState::UP | KeyState::E0 | KeyState::E1
    } else {
        // 按下事件：保留E0标志
        KeyState::E0
    };
    new_state
}
/// 检查当前是否所有按键都处于释放状态
static CLEARED_WEIRD: std::sync::Once = std::sync::Once::new();

pub fn init_keyboard_state() {
    CLEARED_WEIRD.call_once(|| {
        // Existing weird keys...
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
            // NEW: Flush ALL modifiers to prevent ghosting
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
        // Extra: Force-send UP for all modifiers (safe, as they're likely up)
        // You'll need access to Interception here—move this to main() post-init if preferred
    });
}

/// Ultra-fast version after init_keyboard_state() was called once
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
        // NEW: Modifiers (treat as "weird" to avoid false positives)
        VK_LCONTROL.0,
        VK_RCONTROL.0,
        VK_LSHIFT.0,
        VK_RSHIFT.0,
        VK_LMENU.0,
        VK_RMENU.0,
        VK_LWIN.0,
        VK_RWIN.0, // Insert, Delete
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
    // These keys have bit 15 permanently set when up
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
            // For these keys, bit 0 = was pressed since last call
            // (you usually need to call it twice or clear it manually)
            state & 1 != 0
        } else {
            state < 0 // normal keys: bit 15 = currently down
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

        // This will now PASS when Home is not pressed
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
