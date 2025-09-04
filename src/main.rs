//! 该程序使用 kanata_interception 库拦截键盘事件，实现将 CapsLock 键映射为 Left Ctrl 键的功能，
//! 并通过日志记录拦截到的键盘事件信息。

// 导入模块
mod keys;
mod oscode;

use std::collections::HashSet;
// 导入所需的外部库和模块
use crate::oscode::OsCode;
use anyhow::Result;
use kanata_interception as ic;
use kanata_interception::{Device, Interception, KeyState, ScanCode, Stroke};
use log::LevelFilter;
use simplelog::{ColorChoice, CombinedLogger, ConfigBuilder, TermLogger, TerminalMode};
use windows::Win32::UI::Input::KeyboardAndMouse::GetKeyboardState;

/// 程序入口函数
/// 初始化日志系统、拦截驱动，然后进入事件循环处理键盘事件
fn main() -> Result<()> {
    init_log();
    log::info!("等待所有的键释放");
    // 动态等待直到所有按键释放
    loop {
        if are_all_keys_released() {
            log::info!("All keys released. Starting...");
            break;
        }
        // 短间隔轮询，减少 CPU 占用
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // 初始化 Interception 驱动
    // 若驱动未安装则提示用户安装方法
    let intercept = Interception::new().expect(
        "interception driver 未安装，请下载安装。\n\
        下载地址： https://github.com/oblitum/Interception \n\
        安装步骤：\n\
        1. 下载完成后，使用管理员权限打开命令行工具\n\
        2. 在 interception 所在文件夹运行 install-interception /install 进行安装\n\
        3. 使用 install-interception /uninstall 进行卸载\n\
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
    let mut capslock_active = false;
    // 用于跟踪已处理的按键，避免重复发送
    let mut processed_keys = HashSet::new();
    // 是否使用CapsLock进行了组合按键使用
    let mut caps_combination = false;
    // 事件处理主循环：持续监听并处理键盘事件
    loop {
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
                    // 1. 先处理CapsLock本身：禁用原生大小写切换，仅标记激活状态

                    // 打印原始扫描码的十六进制值
                    log::info!("原始扫描码: {:#x}", code as u16);
                    
                    if code == ScanCode::CapsLock {
                        // 标记CapsLock状态，不发送原始事件(避免大小写切换)
                        capslock_active = !state.contains(KeyState::UP);
                        log::info!(
                            "CapsLock状态变更为: {}",
                            if capslock_active { "激活" } else { "关闭" }
                        );
                        // 如果是释放事件，清空已处理按键集
                        if !capslock_active {
                            processed_keys.clear();
                            if !caps_combination {
                                // 如果没有使用capslock进行组合按键使用,在按键释放的时候发送Esc模拟按键
                                intercept.send(
                                    dev,
                                    &[Stroke::Keyboard {
                                        code: ScanCode::Esc,
                                        state:KeyState::DOWN,
                                        information,
                                    }],
                                );
                                intercept.send(
                                    dev,
                                    &[Stroke::Keyboard {
                                        code: ScanCode::Esc,
                                        state:KeyState::UP,
                                        information,
                                    }],
                                );
                                log::info!("发送Esc键位");
                                
                            }
                            // capslock释放时重置caps_combination状态
                            caps_combination = false;
                        }

                        continue; // 不发送原始CapsLock事件
                    }

                    // 当CapsLock激活时处理组合键

                    if capslock_active {
                        // 确认开始使用组合按键
                        caps_combination = true;
                        // 检查是否已处理过该按键，避免重复处理
                        if processed_keys.contains(&code) && state.contains(KeyState::UP) {
                            processed_keys.remove(&code);
                        } else if !processed_keys.contains(&code) {
                            processed_keys.insert(code);
                            // 根据不同按键进行映射

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
                                    log::info!("发送模拟事件: left-control");
                                    intercept.send(dev, &[ctrl_simulating]);
                                    let left_simulating = Stroke::Keyboard {
                                        code: ScanCode::Numpad4,
                                        state: e0_extra_key_state(state),
                                        information,
                                    };
                                    log::info!("发送模拟事件: left");
                                    intercept.send(dev, &[left_simulating]);
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
                                    log::info!("发送模拟事件: left-control");
                                    intercept.send(dev, &[ctrl_simulating]);
                                    let right_simulating = Stroke::Keyboard {
                                        code: ScanCode::Numpad4,
                                        state: e0_extra_key_state(state),
                                        information,
                                    };
                                    log::info!("发送模拟事件: right");
                                    intercept.send(dev, &[right_simulating]);
                                    continue;
                                }
                                // 所有其他情况都使用 ctrl_simulating，第一个参数为当前的 code
                                _ => {
                                    // ctrl + c  ,ctrl + v , ctrl + z ,ctrl + x
                                    ctrl_simulating(code, &intercept, dev, state, information);
                                    continue;
                                }
                            };
                            log::info!("组合键映射: {:?} -> {:?}", original_stroke, mapped_stroke);
                            intercept.send(dev, &[mapped_stroke]);
                            continue;
                        }
                    }
                }

                // 记录事件信息到日志
                // 尝试将事件转换为操作系统编码并显示，若失败则显示"未知的映射信息"
                log::info!(
                    "拦截到键盘事件: {:?}，对应操作系统编码: {}",
                    original_stroke,
                    OsCode::try_from(original_stroke)
                        .map(|osc| osc.as_u16().to_string())
                        .unwrap_or_else(|_| "未知的映射信息".into()),
                );

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
) {
    // 开始模拟ctrl键位
    let ctrl_simulating = Stroke::Keyboard {
        code: ScanCode::LeftControl,
        state,
        information,
    };
    log::info!("发送模拟事件: left-control");
    intercept.send(dev, &[ctrl_simulating]);
    let c_simulating = Stroke::Keyboard {
        code: scan_code,
        state,
        information,
    };
    intercept.send(dev, &[c_simulating]);
    log::info!("发送模拟事件: {}", scan_code as u16);
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
fn are_all_keys_released() -> bool {
    let mut key_states = [0u8; 256]; // 存储所有虚拟键的状态（0-255）
    // 一次性获取所有键的状态
    let success = unsafe { GetKeyboardState(&mut key_states) };
    match success {
        Ok(_) => {
            // 检查0x01到0xFE范围内的所有键
            for vk_code in 0x01..=0xFE {
                let state = key_states[vk_code as usize];
                // 每个字节的最高位（0x80）表示按键是否按下
                if (state & 0x80) != 0 {
                    return false; // 检测到按下的键
                }
            }
        }
        Err(_) => {
            return false;
        }
    }
    log::info!("所有按键均已释放");

    true
}
