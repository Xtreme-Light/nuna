#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! nuna —— `CapsLock` 增强小工具
//!
//! v0.1.1 起彻底移除 Interception 内核驱动依赖，改用 Windows 低级键盘钩子
//! （`WH_KEYBOARD_LL`，实现参考 kanata 的 LLHOOK 后端与 kbremap）拦截键盘事件：
//! - 无需安装任何驱动、无需管理员权限，解压即用；
//! - 不再向系统注册 keyboard/mouse 类过滤驱动，从根源上消除 v0.1.0 打包的
//!   Interception `mouse.sys` 过滤驱动在 USB 鼠标重新枚举后吞掉输入、导致
//!   "设备管理器正常但鼠标不动"的问题；
//! - 映射语义与 v0.1.0 保持一致：按下/抬起严格配对注入，避免本机或云桌面/
//!   远程桌面这类只透传按键流的会话出现按键卡住（影子按键）的问题。

// 导入模块
mod keys;
mod tray;

use crate::keys::{VK_CAPITAL, VK_LCONTROL, VK_SPACE, VK_BACK, VK_D, VK_DELETE, VK_A, VK_HOME, VK_E, VK_END, VK_H, VK_LEFT, VK_L, VK_RIGHT, VK_J, VK_UP, VK_K, VK_DOWN, VK_B, VK_F, VK_Z, VK_X, VK_C, VK_V, VK_W, VK_Q, VK_S, VK_PRIOR, VK_NEXT, VK_INSERT, VK_RCONTROL, VK_LSHIFT, VK_RSHIFT, VK_LMENU, VK_RMENU, VK_LWIN, VK_RWIN};
use crate::tray::init_tray;
use anyhow::Result;
use crossbeam_channel::{Sender, unbounded};
use log::LevelFilter;
use simplelog::{ColorChoice, CombinedLogger, ConfigBuilder, TermLogger, TerminalMode};
use single_instance::SingleInstance;
use std::cell::Cell;
use windows::core::PCWSTR;
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
    VIRTUAL_KEY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, GetMessageW, MessageBoxW, PostThreadMessageW, SetWindowsHookExW, HC_ACTION,
    HHOOK, KBDLLHOOKSTRUCT, LLKHF_INJECTED, LLKHF_UP, MB_ICONERROR, MB_OK, MSG, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

/// 程序入口函数
/// 初始化日志与低级键盘钩子，主线程运行托盘事件循环，退出时收尾
fn main() -> Result<()> {
    // 无控制台的 GUI 程序：任何 panic 都弹窗展示，而不是无声退出只剩退出码
    std::panic::set_hook(Box::new(|info: &std::panic::PanicHookInfo<'_>| {
        fatal_dialog("nuna 发生内部错误", &info.to_string());
    }));

    let instance = SingleInstance::new("nuna.exe")?;
    if !instance.is_single() {
        fatal_dialog("nuna", "已有实例正在运行，请勿重复启动。");
        return Err(anyhow::anyhow!("已有单实例正在执行，请勿重复启动"));
    }

    init_log();

    log::info!("程序启动中...");

    // 启动键盘钩子线程，并等待它回报「钩子已装好」+ 线程 id
    let (ready_tx, ready_rx) = unbounded::<Result<u32, String>>();
    let hook_handle = std::thread::spawn(move || {
        if let Err(e) = keyboard_hook_thread(&ready_tx) {
            log::error!("键盘钩子线程出错: {e}");
            // 无控制台的 GUI 程序，致命错误必须弹窗告知并退出
            fatal_dialog("nuna 运行出错", &format!("{e:#}"));
            std::process::exit(1);
        }
    });
    // 两个 ? 分别处理：线程启动失败 / 钩子安装失败（后者由线程回报 Err 消息）
    let hook_tid = match ready_rx.recv() {
        Ok(Ok(tid)) => tid,
        Ok(Err(msg)) => return Err(anyhow::anyhow!(msg)),
        Err(_) => return Err(anyhow::anyhow!("键盘钩子线程未能启动")),
    };

    // 初始化系统托盘（主线程阻塞在事件循环，直到用户点退出）
    init_tray()?;

    // 退出：通知钩子线程收尾（抬掉仍按着的注入键、卸载钩子）
    // WM_QUIT 特殊：即使钩子线程还没进入消息泵，也会让其 GetMessageW 立即返回 0
    // SAFETY: PostThreadMessageW 参数均为按值传递的 POD；tid 来自钩子线程回报，
    // 线程此时仍在消息泵中，目标有效
    unsafe {
        let _ = PostThreadMessageW(hook_tid, WM_QUIT, WPARAM(0), LPARAM(0));
    }
    if hook_handle.join().is_err() {
        log::error!("键盘钩子线程结束异常");
    }
    log::info!("nuna 已退出");
    Ok(())
}

/// 钩子回调可见的一个待注入按键事件（VK 级）
#[derive(Clone, Copy, Debug)]
struct VkEvent {
    vk: u16,
    /// 是否为抬起
    up: bool,
}

fn vk_down(vk: u16) -> VkEvent {
    VkEvent { vk, up: false }
}

fn vk_up(vk: u16) -> VkEvent {
    VkEvent { vk, up: true }
}

/// caps 层内一条已注入的映射：记录原始虚拟键，以及和按下序列严格配对的抬起序列
struct InjectedKey {
    vk: u16,
    downs: Vec<VkEvent>,
    ups: Vec<VkEvent>,
}

/// 钩子回调的映射状态
struct HookState {
    /// 启动后首次观测到「所有键已释放」才接管事件流，
    /// 避免接管到半截的按键流导致配对错乱（对应 v0.1.0 启动时的等待逻辑）
    armed: bool,
    /// `CapsLock` 是否处于按下状态（caps 层开关）
    caps_down: bool,
    /// caps 层内已注入且尚未抬起的键
    injected: Vec<InjectedKey>,
}

/// 钩子回调收到的按键事件
struct HookEvent {
    vk: u16,
    up: bool,
}

type HookFn = Box<dyn FnMut(HookEvent) -> bool>;

thread_local! {
    /// 当前线程的钩子回调。回调在本线程消息泵里被调度，
    /// 注入事件在 hook_proc 里已被过滤，不会重入到这里
    static HOOK: Cell<Option<HookFn>> = const { Cell::new(None) };
}

/// `WH_KEYBOARD_LL` 回调（WinAPI 签名）。
/// 返回非 0 表示吞掉该事件；调用 `CallNextHookEx` 表示放行。
///
/// 参数含义见 <https://learn.microsoft.com/windows/win32/winmsg/lowlevelkeyboardproc>
/// code   : <0 `必须原样转发；HC_ACTION(=0)` 时 wParam/lParam 携带按键消息
/// wParam : `WM_KEYDOWN` / `WM_KEYUP` / `WM_SYSKEYDOWN` / `WM_SYSKEYUP`
/// lParam : 指向 KBDLLHOOKSTRUCT
#[expect(
    clippy::as_conversions,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    reason = "Win32 回调 ABI 的宽度转换：LPARAM 承载指针、wParam 承载 WM_* 常量、vkCode 值域 0..=255，值域均由系统约定保证"
)]
unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // SAFETY: HC_ACTION 时系统保证 lParam 指向有效的 KBDLLHOOKSTRUCT；
    // 该结构体在回调返回前一直有效（不越出本次调用使用）
    let kb = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };

    // SendInput 注入的事件会再次进入本钩子：必须直接放行，否则无限递归。
    // code != HC_ACTION 的事件按约定必须原样转发。
    if code != HC_ACTION as i32 || kb.flags.contains(LLKHF_INJECTED) {
        // SAFETY: 参数原样按值转发给钩子链下一环，无所有权/别名问题
        return unsafe { CallNextHookEx(None, code, wparam, lparam) };
    }

    // 只处理键盘按下/抬起（含系统键 Alt 组合），其余消息转发
    match wparam.0 as u32 {
        WM_KEYDOWN | WM_KEYUP | WM_SYSKEYDOWN | WM_SYSKEYUP => {}
        _ => {
            // SAFETY: 参数原样按值转发给钩子链下一环
            return unsafe { CallNextHookEx(None, code, wparam, lparam) };
        }
    }

    let ev = HookEvent {
        vk: kb.vkCode as u16,
        up: kb.flags.contains(LLKHF_UP),
    };

    let mut handled = false;
    HOOK.with(|state| {
        // 把闭包取出、调用、再放回。注入事件已被上面过滤，
        // 因此回调不可能重入，「取出失败」只可能是编程错误
        let mut hook = state.take().expect("no recurse");
        handled = hook(ev);
        state.set(Some(hook));
    });

    if handled {
        LRESULT(1)
    } else {
        // SAFETY: 参数原样按值转发给钩子链下一环
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }
}

/// RAII 钩子句柄：drop 时卸载低级键盘钩子
struct KeyboardHook {
    handle: HHOOK,
}

impl KeyboardHook {
    /// 在当前线程安装 `WH_KEYBOARD_LL` 全局钩子。
    /// 注意：必须在调用前先把回调放入 HOOK（低级钩子回调依赖本线程泵消息）。
    fn install() -> windows::core::Result<Self> {
        // SAFETY: 本线程将立即进入消息泵（见 keyboard_hook_thread），
        // 满足低级钩子对安装线程泵消息的要求；句柄由 Self 持有并在 drop 时卸载
        let handle = unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), None, 0)? };
        Ok(Self { handle })
    }
}

impl Drop for KeyboardHook {
    fn drop(&mut self) {
        // SAFETY: handle 来自 SetWindowsHookExW 且未被卸载过（RAII 保证只 drop 一次）
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::UnhookWindowsHookEx(self.handle);
        }
        HOOK.with(std::cell::Cell::take);
    }
}

/// 键盘钩子线程：安装 `WH_KEYBOARD_LL` + 跑消息泵，直到主线程投递 `WM_QUIT`
fn keyboard_hook_thread(ready_tx: &Sender<Result<u32, String>>) -> Result<()> {
    // 启动阶段先调用一次 GetAsyncKeyState 清掉"曾被按下"的粘滞位，
    // 让 armed 检测不被启动前的历史按键干扰（沿袭 v0.1.0 的初始化逻辑）
    init_keyboard_state();

    // 钩子回调 → 注入线程的输出通道：每个元素是一批待注入的事件序列。
    // 不在回调里直接 SendInput：低级钩子回调必须尽快返回（系统对慢钩子
    // 有超时旁路惩罚），注入统一交给独立线程，结构上对齐 kanata。
    let (out_tx, out_rx) = unbounded::<Vec<VkEvent>>();

    let injector = std::thread::spawn(move || {
        for batch in &out_rx {
            send_events(&batch);
        }
        log::info!("注入线程退出");
    });

    // 映射状态包在 Rc<RefCell> 里：回调闭包和退出收尾都要访问
    let state = std::rc::Rc::new(std::cell::RefCell::new(HookState {
        armed: false,
        caps_down: false,
        injected: Vec::new(),
    }));
    let cb_state = std::rc::Rc::clone(&state);
    // 回调持有一份 Sender 克隆：退出收尾时线程本体还要用 out_tx 发送抬键事件
    let cb_out_tx = out_tx.clone();

    let callback = move |ev: HookEvent| -> bool {
        // 启动阶段：观测到所有键释放之前一律放行，避免接管半截按键流
        if !cb_state.borrow().armed {
            if are_all_keys_released() {
                cb_state.borrow_mut().armed = true;
                log::info!("所有按键均已释放，映射逻辑开始接管");
            } else {
                return false;
            }
        }

        // CapsLock 本身不下发，只用来维护 caps 层的开关状态
        if ev.vk == VK_CAPITAL {
            if ev.up {
                // 松开 caps 时，立即抬掉该层所有仍按着的注入键。
                // 不能依赖各按键自己的 UP 事件：用户可能先松 caps 再松按键，
                // 那时 caps_down 已变回 false，UP 会被原样放行，之前注入的
                // Ctrl/方向键就永远停在按下状态 —— 这就是影子按键/卡 Ctrl 的根源
                release_all_injected(&cb_out_tx, &mut cb_state.borrow_mut().injected);
                cb_state.borrow_mut().caps_down = false;
            } else {
                cb_state.borrow_mut().caps_down = true;
            }
            return true; // 吞掉 CapsLock，CapsLock 开关状态不再受影响
        }

        let mut st = cb_state.borrow_mut();

        // 按下时被映射注入过的键：无论此刻 caps 是否还按着，
        // UP/重复 DOWN 一律按当初注入的内容配对处理
        if let Some(pos) = st.injected.iter().position(|e| e.vk == ev.vk) {
            if ev.up {
                let entry = st.injected.remove(pos);
                let _ = cb_out_tx.send(entry.ups);
            } else {
                // 硬件自动重复产生的 DOWN：重发按下序列，保留连续触发的手感
                let downs = st.injected[pos].downs.clone();
                let _ = cb_out_tx.send(downs);
            }
            return true;
        }

        // caps 层内新按下的可映射键：成对生成 按下/抬起 序列并记录
        if st.caps_down && !ev.up
            && let Some((downs, ups)) = build_mapping(ev.vk) {
                let _ = cb_out_tx.send(downs.clone());
                st.injected.push(InjectedKey {
                    vk: ev.vk,
                    downs,
                    ups,
                });
                return true;
            }

        // 其余事件（未映射的按键、以及各种未被上面命中的情况）原样放行
        false
    };

    // 先注册回调再装钩子：钩子装好的瞬间回调必须已就位
    HOOK.with(|s| s.set(Some(Box::new(callback))));
    let hook = match KeyboardHook::install() {
        Ok(h) => h,
        Err(e) => {
            let msg = format!("安装低级键盘钩子失败: {e}");
            let _ = ready_tx.send(Err(msg.clone()));
            drop(out_tx);
            let _ = injector.join();
            HOOK.with(std::cell::Cell::take);
            return Err(anyhow::anyhow!(msg));
        }
    };

    // SAFETY: 无参数，仅返回当前线程 id
    let tid = unsafe { GetCurrentThreadId() };
    if ready_tx.send(Ok(tid)).is_err() {
        // 主线程已提前退出：无需收尾注入键（armed 尚未就绪或无按键），直接清理
        drop(hook);
        drop(out_tx);
        let _ = injector.join();
        return Ok(());
    }
    log::info!("LLHOOK 已安装（线程 {tid}），开始监听键盘事件...");

    // 消息泵：低级钩子的回调经由本线程的 GetMessageW 循环被调度。
    // 返回 0（WM_QUIT）或 -1（错误）都退出
    let mut msg = MSG::default();
    loop {
        // SAFETY: msg 为本线程栈上的合法 MSG，GetMessageW 仅写入该结构体
        let r = unsafe { GetMessageW(&raw mut msg, None, 0, 0) };
        if r.0 == 0 || r.0 == -1 {
            break;
        }
    }

    // 退出前抬掉所有仍按下的注入键，避免退出后残留卡键
    release_all_injected(&out_tx, &mut state.borrow_mut().injected);
    drop(out_tx);
    let _ = injector.join();
    drop(hook); // 卸载钩子
    log::info!("键盘钩子线程退出");
    Ok(())
}

/// 用 `SendInput` 以虚拟键码注入一批事件。
/// 注入事件自带 `LLKHF_INJECTED` `标志，hook_proc` 会直接放行，不会递归。
/// VK 注入无需关心 E0/E1 `扩展扫描码：VK_LEFT/VK_DELETE` 等本身无歧义，
/// 这也去掉了 v0.1.0 扫描码方案里最易错的扩展键状态拼装。
fn send_events(events: &[VkEvent]) {
    let inputs: Vec<INPUT> = events
        .iter()
        .map(|e| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(e.vk),
                    wScan: 0,
                    dwFlags: if e.up { KEYEVENTF_KEYUP } else { KEYBD_EVENT_FLAGS(0) },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        })
        .collect();
    // SAFETY: inputs 为合法 INPUT 切片；cbSize 传单个结构体大小是 SendInput 的
    // 约定调用方式，注入事件带 LLKHF_INJECTED 标志不会重入钩子
    let cb_size =
        i32::try_from(std::mem::size_of::<INPUT>()).expect("INPUT 结构体大小必在 i32 范围内");
    // SAFETY: inputs 为合法 INPUT 切片，cbSize 为单个结构体大小（SendInput 约定）；
    // 注入事件带 LLKHF_INJECTED 标志，hook_proc 会放行，不会重入
    let sent = unsafe { SendInput(&inputs, cb_size) };
    let sent_len = usize::try_from(sent).expect("u32 必可放入 usize（x64 目标）");
    if sent_len != inputs.len() {
        log::error!("SendInput 部分失败: 期望注入 {} 个事件，实际 {}", inputs.len(), sent);
    }
}

/// 按后按先抬的顺序，把所有仍处于按下状态的注入键全部抬起
fn release_all_injected(out_tx: &Sender<Vec<VkEvent>>, injected: &mut Vec<InjectedKey>) {
    if !injected.is_empty() {
        log::info!("抬起 {} 个仍处于按下状态的注入键", injected.len());
    }
    while let Some(entry) = injected.pop() {
        let _ = out_tx.send(entry.ups);
    }
}

/// 为 caps 层内按下的键生成 (按下序列, 抬起序列)。
/// 两条序列必须一一配对，否则接收端——无论是本机应用，还是云桌面/远程桌面
/// 这类只透传按键流的会话——都会认为按键一直被按住，表现为 Ctrl 卡键等影子按键
fn build_mapping(vk: u16) -> Option<(Vec<VkEvent>, Vec<VkEvent>)> {
    // 单键替换：按下和抬起使用同一个虚拟键
    let single =
        |mapped: u16| -> (Vec<VkEvent>, Vec<VkEvent>) { (vec![vk_down(mapped)], vec![vk_up(mapped)]) };
    // ctrl 组合键：按下 [ctrl, key]，抬起 [key, ctrl]，顺序严格相反。
    // 显式使用 VK_LCONTROL：避免注入通用 VK_CONTROL 时与右 Ctrl 的真实
    // 状态混淆导致修饰键状态对不上（v0.1.0 扫描码方案同样规避了这一点）
    let with_ctrl = |mapped: u16| -> (Vec<VkEvent>, Vec<VkEvent>) {
        (
            vec![vk_down(VK_LCONTROL), vk_down(mapped)],
            vec![vk_up(mapped), vk_up(VK_LCONTROL)],
        )
    };

    match vk {
        // Caps + Space = Backspace
        VK_SPACE => Some(single(VK_BACK)),
        // Caps + D = Del
        VK_D => Some(single(VK_DELETE)),
        // Caps + A = Home
        VK_A => Some(single(VK_HOME)),
        // Caps + E = End
        VK_E => Some(single(VK_END)),
        // 方向左键
        VK_H => Some(single(VK_LEFT)),
        // 方向右键
        VK_L => Some(single(VK_RIGHT)),
        // 方向上键
        VK_J => Some(single(VK_UP)),
        // 方向下键
        VK_K => Some(single(VK_DOWN)),
        // Caps + B = Ctrl + Left
        VK_B => Some(with_ctrl(VK_LEFT)),
        // Caps + F = Ctrl + Right
        VK_F => Some(with_ctrl(VK_RIGHT)),
        // Caps + Z/X/C/V/W = Ctrl + 对应键
        VK_Z => Some(with_ctrl(VK_Z)),
        VK_X => Some(with_ctrl(VK_X)),
        VK_C => Some(with_ctrl(VK_C)),
        VK_V => Some(with_ctrl(VK_V)),
        VK_W => Some(with_ctrl(VK_W)),
        // Caps + Q = Ctrl + A
        VK_Q => Some(with_ctrl(VK_A)),
        // Caps + S = Ctrl + S
        VK_S => Some(with_ctrl(VK_S)),
        _ => None,
    }
}

/// 无控制台的 GUI 程序，致命错误必须弹窗告知，
/// 否则 panic/abort 只会留下一个无意义的退出码
fn fatal_dialog(title: &str, text: &str) {
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }
    let title = wide(title);
    let text = wide(text);
    // SAFETY: UTF-16 缓冲区以 NUL 结尾（wide() 保证），且在调用期间保持存活
    unsafe {
        MessageBoxW(
            None,
            PCWSTR::from_raw(text.as_ptr()),
            PCWSTR::from_raw(title.as_ptr()),
            MB_OK | MB_ICONERROR,
        );
    }
}

fn init_log() {
    // 配置日志系统
    // 尝试将日志时间设置为本地时间，若失败则输出警告
    let mut log_config = ConfigBuilder::new();
    if let Err(e) = log_config.set_time_offset_to_local() {
        eprintln!("警告: 无法进行日期本地化: {e:?}");
    }

    // 初始化组合日志器，输出终端日志
    CombinedLogger::init(vec![TermLogger::new(
        LevelFilter::Info,   // 日志级别为 Info
        log_config.build(),  // 使用上述配置
        TerminalMode::Mixed, // 混合模式（ stdout 和 stderr）
        ColorChoice::Auto,   // 自动选择颜色（根据终端支持情况）
    )])
    .expect("日志初始化失败"); // 若初始化失败则终止程序并提示
    log::info!("日志初始化成功");
}

/// 启动时清一遍“曾被按下”粘滞位：对方向/编辑键与全部修饰键各调用一次
/// GetAsyncKeyState，让随后的 armed 检测只反映当前真实按下的键
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
            // 全部修饰键，防止残留的“幽灵按下”
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
            // SAFETY: 无副作用参数，仅读取异步键态并清粘滞位
            unsafe {
                windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(i32::from(vk));
            }
        }
    });
}

/// 检查当前是否所有按键都处于释放状态（armed 检测用）
pub fn are_all_keys_released() -> bool {
    const WEIRD: [u16; 18] = [
        0x21, 0x22, 0x23, 0x24, // Prior, Next, End, Home
        0x25, 0x26, 0x27, 0x28, // Left, Up, Right, Down
        0x2D, 0x2E, // Insert, Delete
        VK_LCONTROL,
        VK_RCONTROL,
        VK_LSHIFT,
        VK_RSHIFT,
        VK_LMENU,
        VK_RMENU,
        VK_LWIN,
        VK_RWIN,
    ];

    for code in 1u16..=255u16 {
        // SAFETY: 无副作用参数，仅读取异步键态
        let state = unsafe {
            windows::Win32::UI::Input::KeyboardAndMouse::GetAsyncKeyState(i32::from(code))
        };
        let pressed = if WEIRD.contains(&code) {
            (state & 1) != 0 // "was pressed" 粘滞位
        } else {
            state < 0 // 常规键：当前是否按下
        };
        if pressed {
            return false;
        }
    }
    true
}
