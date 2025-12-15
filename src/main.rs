#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! 该程序使用 kanata_interception 库拦截键盘事件，实现将 CapsLock 键映射为 Left Ctrl 键的功能，
//! 并通过日志记录拦截到的键盘事件信息。

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
use windows::Win32::Foundation::LPARAM;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyNameTextW, VK_DELETE, VK_DOWN, VK_END, VK_HOME, VK_INSERT, VK_LCONTROL,
    VK_LEFT, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_NEXT, VK_PRIOR, VK_RCONTROL, VK_RIGHT, VK_RMENU,
    VK_RSHIFT, VK_RWIN, VK_UP,
};

/// 程序入口函数
/// 初始化日志系统、拦截驱动，然后进入事件循环处理键盘事件
fn main() -> Result<()> {
    let instance = SingleInstance::new("nuna.exe")?;
    if !instance.is_single() {
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
    // 若驱动未安装则提示用户安装方法
    let intercept = Interception::new().expect(
        "interception driver 未安装，请下载安装。\n
        下载地址： https://github.com/oblitum/Interception \n
        安装步骤：\n
        1. 下载完成后，使用管理员权限打开命令行工具\n
        2. 在 interception 所在文件夹运行 install-interception /install 进行安装\n
        3. 使用 install-interception /uninstall 进行卸载\n
        注意：安装后需要重启电脑生效",
    );

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
    // NEW: Track if we expect Ctrl to be down (prevents ghosting)
    let mut expected_ctrl_down = false;

    loop {
        // 检查退出信号
        if exit_rx.try_recv().is_ok() {
            log::info!("收到退出信号，停止键盘拦截");
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
                // 处理 CapsLock 键映射：将 CapsLock 替换为 Left Ctrl
                if let Stroke::Keyboard {
                    code,
                    state,
                    information,
                } = original_stroke
                {
                    // 如果是按下了Capslock键位，则激活标识位
                    if code == ScanCode::CapsLock {
                        caps_down = !state.contains(KeyState::UP); // DOWN = true
                        continue;
                    }
                    //  下一个键位过来的时候，此时caps是否被激活了，被激活了，则触发组合键的功能
                    if caps_down {
                        let mapped_stroke = match code {
                            // Ctrl +Space = Backspace
                            ScanCode::Space => Stroke::Keyboard {
                                code: ScanCode::Backspace,
                                state,
                                information,
                            },
                            // Ctrl +D = Del
                            ScanCode::D => Stroke::Keyboard {
                                code: ScanCode::NumpadPeriod,
                                state: e0_extra_key_state(state),
                                information,
                            },
                            // Ctrl +A = Home
                            ScanCode::A => Stroke::Keyboard {
                                code: ScanCode::Numpad7,
                                state: e0_extra_key_state(state),
                                information,
                            },
                            // Ctrl + E = End
                            ScanCode::E => Stroke::Keyboard {
                                code: ScanCode::Numpad1,
                                state: e0_extra_key_state(state),
                                information,
                            },
                            // 方向左键
                            ScanCode::H => Stroke::Keyboard {
                                code: ScanCode::Numpad4,
                                state: e0_extra_key_state(state),
                                information,
                            },
                            // 方向右键
                            ScanCode::L => Stroke::Keyboard {
                                code: ScanCode::Numpad6,
                                state: e0_extra_key_state(state),
                                information,
                            },
                            // 方向上键
                            ScanCode::J => Stroke::Keyboard {
                                code: ScanCode::Numpad8,
                                state: e0_extra_key_state(state),
                                information,
                            },
                            // 方向下键
                            ScanCode::K => Stroke::Keyboard {
                                code: ScanCode::Numpad2,
                                state: e0_extra_key_state(state),
                                information,
                            },
                            // ctrl + left
                            ScanCode::B => {
                                // 开始模拟ctrl键位
                                let ctrl_simulating = Stroke::Keyboard {
                                    code: ScanCode::LeftControl,
                                    state,
                                    information,
                                };
                                let left_simulating = Stroke::Keyboard {
                                    code: ScanCode::Numpad4,
                                    state: e0_extra_key_state(state),
                                    information,
                                };
                                intercept.send(dev, &[ctrl_simulating, left_simulating]);
                                continue;
                            }
                            // ctrl + right
                            ScanCode::F => {
                                // 开始模拟ctrl键位
                                let ctrl_simulating = Stroke::Keyboard {
                                    code: ScanCode::LeftControl,
                                    state,
                                    information,
                                };
                                let right_simulating = Stroke::Keyboard {
                                    code: ScanCode::Numpad6,
                                    state: e0_extra_key_state(state),
                                    information,
                                };
                                intercept.send(dev, &[ctrl_simulating, right_simulating]);
                                continue;
                            }
                            ScanCode::Z => {
                                // ctrl + c  ,ctrl + v , ctrl + z ,ctrl + x
                                ctrl_simulating(
                                    ScanCode::Z,
                                    &intercept,
                                    dev,
                                    state,
                                    information,
                                    &mut expected_ctrl_down,
                                );
                                continue;
                            }
                            ScanCode::X => {
                                // ctrl + c  ,ctrl + v , ctrl + z ,ctrl + x
                                ctrl_simulating(
                                    ScanCode::X,
                                    &intercept,
                                    dev,
                                    state,
                                    information,
                                    &mut expected_ctrl_down,
                                );
                                continue;
                            }
                            ScanCode::C => {
                                // ctrl + c  ,ctrl + v , ctrl + z ,ctrl + x
                                ctrl_simulating(
                                    ScanCode::C,
                                    &intercept,
                                    dev,
                                    state,
                                    information,
                                    &mut expected_ctrl_down,
                                );
                                continue;
                            }
                            ScanCode::V => {
                                // ctrl + c  ,ctrl + v , ctrl + z ,ctrl + x
                                ctrl_simulating(
                                    ScanCode::V,
                                    &intercept,
                                    dev,
                                    state,
                                    information,
                                    &mut expected_ctrl_down,
                                );
                                continue;
                            }
                            ScanCode::W => {
                                // ctrl + c  ,ctrl + v , ctrl + z ,ctrl + x
                                ctrl_simulating(
                                    ScanCode::W,
                                    &intercept,
                                    dev,
                                    state,
                                    information,
                                    &mut expected_ctrl_down,
                                );
                                continue;
                            }
                            ScanCode::Q => {
                                // caps + q = ctl + a
                                ctrl_simulating(
                                    ScanCode::A,
                                    &intercept,
                                    dev,
                                    state,
                                    information,
                                    &mut expected_ctrl_down,
                                );
                                continue;
                            }
                            ScanCode::S => {
                                // caps + s = ctl + s
                                ctrl_simulating(
                                    ScanCode::S,
                                    &intercept,
                                    dev,
                                    state,
                                    information,
                                    &mut expected_ctrl_down,
                                );
                                continue;
                            }
                            _ => original_stroke,
                        };

                        // log::info!(
                        //     "拦截到键盘事件: {:?}，映射成为了： {:?}",
                        //     original_stroke,
                        //     mapped_stroke,
                        // );
                        intercept.send(dev, &[mapped_stroke]);
                        continue;
                    }
                }

                if expected_ctrl_down && !is_key_down(VK_LCONTROL) {
                    // Mismatch: Force Ctrl UP to resync
                    let ctrl_up = Stroke::Keyboard {
                        code: ScanCode::LeftControl,
                        state: KeyState::UP | KeyState::E0, // E0 for extended Ctrl
                        information: 0,                     // Or original
                    };
                    intercept.send(dev, &[ctrl_up]);
                    expected_ctrl_down = false;
                    log::warn!("Resynced stuck Ctrl UP");
                }
                // 将处理后的事件发送出去（若有映射则发送修改后的值）
                intercept.send(dev, &[original_stroke]);
            }
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

/// 发送Ctrl 相关的模拟事件
fn ctrl_simulating(
    scan_code: ScanCode,
    intercept: &Interception,
    dev: Device,
    state: KeyState,
    information: u32,
    expected_ctrl_down: &mut bool,
) {
    if !state.contains(KeyState::UP) {
        // Batch: Ctrl DOWN + Key DOWN
        let ctrl_down = Stroke::Keyboard {
            code: ScanCode::LeftControl,
            state: KeyState::DOWN | KeyState::E0, // E0 required for Left Ctrl
            information,
        };
        let key_down = Stroke::Keyboard {
            code: scan_code,
            state: KeyState::DOWN,
            information,
        };
        // log::info!(
        //     "拦截到键盘按下事件: {:?}，映射成为了： {:?} {:?}",
        //     scan_code,
        //     ctrl_down,
        //     key_down
        // );
        intercept.send(dev, &[ctrl_down, key_down]);
        *expected_ctrl_down = true;
    } else {
        // Batch: Key UP + Ctrl UP (reverse order to match release)
        let key_up = Stroke::Keyboard {
            code: scan_code,
            state: KeyState::UP,
            information,
        };
        let ctrl_up = Stroke::Keyboard {
            code: ScanCode::LeftControl,
            state: KeyState::UP | KeyState::E0,
            information,
        };
        intercept.send(dev, &[key_up, ctrl_up]);
        // log::info!(
        //     "拦截到键盘释放事件: {:?}，映射成为了： {:?} {:?}",
        //     scan_code,
        //     key_up,
        //     ctrl_up
        // );
        *expected_ctrl_down = false;

        // Optional: Short sleep for high-load systems
        // std::thread::sleep(std::time::Duration::from_millis(1));
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

        unsafe {
            let home = is_key_down(VK_HOME);
            let shift = is_key_down(VK_LSHIFT);
            let a = is_key_down(VK_A);

            println!("HOME: {home}   LSHIFT: {shift}   A: {a}");

            // This will now PASS when Home is not pressed
            assert!(!home, "Home key is reported as down but it should be up");
        }
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
