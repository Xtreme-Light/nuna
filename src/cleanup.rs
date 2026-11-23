//! 托盘菜单「清理驱动残留」：移除 v0.1.0 时代 Interception 驱动写进注册表的
//! 键盘/鼠标类过滤器（HKLM，需要管理员）。v0.1.1 起 LLHOOK 方案完全不依赖
//! 该驱动，残留只有风险没有收益。
//!
//! 提权方式：以 runas 重新启动自身并附带 `--cleanup-driver` 参数，由提升后的
//! 子进程完成 HKLM 修改（改动前 reg export 备份），主进程等待退出码并弹窗汇报：
//! 0=已清理、2=本来就干净、1=失败（详情写入 %TEMP%\nuna-cleanup-error.txt）。
//! 等价于 script/cleanup-interception.ps1 -RemoveKeyboardToo，集成进 exe 后
//! 单文件即可完成清理，不依赖 zip 里的脚本。

use std::path::PathBuf;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, WAIT_OBJECT_0};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use windows::Win32::UI::WindowsAndMessaging::{
    MessageBoxW, IDYES, MB_ICONERROR, MB_ICONINFORMATION, MB_ICONQUESTION, MB_OK, MB_YESNO,
    MESSAGEBOX_STYLE, SW_HIDE,
};
use winreg::enums::{HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE, REG_MULTI_SZ};
use winreg::{RegKey, RegValue};

const KEYBOARD_CLASS_KEY: &str =
    r"SYSTEM\CurrentControlSet\Control\Class\{4d36e96b-e325-11ce-bfc1-08002be10318}";
const MOUSE_CLASS_KEY: &str =
    r"SYSTEM\CurrentControlSet\Control\Class\{4d36e96f-e325-11ce-bfc1-08002be10318}";

/// `--cleanup-driver` 子进程的错误详情落盘位置（主进程读取后用于失败弹窗）
fn error_detail_file() -> PathBuf {
    std::env::temp_dir().join("nuna-cleanup-error.txt")
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn msg_box(text: &str, title: &str, flags: MESSAGEBOX_STYLE) -> i32 {
    let (text, title) = (wide(text), wide(title));
    // SAFETY: UTF-16 缓冲区以 NUL 结尾（wide() 保证），且在调用期间保持存活
    unsafe {
        MessageBoxW(
            None,
            PCWSTR::from_raw(text.as_ptr()),
            PCWSTR::from_raw(title.as_ptr()),
            flags,
        )
        .0
    }
}

// ---------------------------------------------------------------- 注册表编解码

/// 把 `REG_MULTI_SZ` 的原始字节解码成字符串列表（容忍 `REG_SZ` 单值与结尾缺空串）
fn decode_multi_sz(value: &RegValue) -> Vec<String> {
    let units: Vec<u16> = value
        .bytes
        .as_chunks::<2>().0.iter()
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let mut list = Vec::new();
    let mut current = Vec::new();
    for &u in &units {
        if u == 0 {
            if current.is_empty() {
                break; // 结尾的双 NUL
            }
            list.push(String::from_utf16_lossy(&current));
            current.clear();
        } else {
            current.push(u);
        }
    }
    if !current.is_empty() {
        list.push(String::from_utf16_lossy(&current));
    }
    if value.vtype != REG_MULTI_SZ && !list.is_empty() {
        // REG_SZ 单值场景：整串即一个条目
        return vec![list.concat()];
    }
    list
}

/// 把字符串列表编码为 `REG_MULTI_SZ` 原始字节（每串 NUL 结尾 + 列表结尾再补一个 NUL）
fn encode_multi_sz(list: &[String]) -> RegValue {
    let mut units: Vec<u16> = Vec::new();
    for s in list {
        units.extend(s.encode_utf16());
        units.push(0);
    }
    units.push(0);
    RegValue {
        bytes: units.iter().flat_map(|&u| u.to_le_bytes()).collect(),
        vtype: REG_MULTI_SZ,
    }
}

// ---------------------------------------------------------------- 清理实现

enum Outcome {
    /// 移除了过滤器；携带备份文件路径（可能为空=备份失败但已继续）
    Changed { backups: Vec<PathBuf> },
    /// 注册表里本来就没有 Interception 过滤器
    AlreadyClean,
}

/// 从指定设备类键的 `UpperFilters` 里移除 Interception 过滤器。
/// 列表绝不允许写空（兜底写入系统默认类驱动）。
fn clean_filter(hklm: &RegKey, subkey: &str, filter: &str, keep_default: &str) -> Result<Outcome, String> {
    let key = hklm
        .open_subkey_with_flags(subkey, KEY_READ | KEY_WRITE)
        .map_err(|e| format!("打开注册表键 {subkey} 失败: {e}"))?;

    let filters = key
        .get_raw_value("UpperFilters")
        .map_or_default(|v| decode_multi_sz(&v));
    if !filters.iter().any(|f| f == filter) {
        return Ok(Outcome::AlreadyClean);
    }

    // 备份到 exe 同目录（不可写则退回 %TEMP%），与随附脚本行为一致
    let backup = backup_subkey(subkey, filter);

    let mut new_filters: Vec<String> = filters.iter().filter(|f| *f != filter && !f.is_empty()).cloned().collect();
    if new_filters.is_empty() {
        new_filters.push(keep_default.to_string());
    }
    key.set_raw_value("UpperFilters", &encode_multi_sz(&new_filters))
        .map_err(|e| format!("写回 {subkey}\\UpperFilters 失败: {e}"))?;
    log::info!("[清理] {subkey}: {filters:?} => {new_filters:?}");
    Ok(Outcome::Changed {
        backups: backup.into_iter().collect(),
    })
}

/// reg export 备份一个注册表键；失败返回 None（清理继续，仅少一份备份）
fn backup_subkey(subkey: &str, filter: &str) -> Option<PathBuf> {
    let stamp = local_time_stamp();
    let file_name = format!("backup-{filter}-{stamp}.reg");
    let target_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(PathBuf::from))
        .filter(|d| d.try_exists().unwrap_or(false))
        .unwrap_or_else(std::env::temp_dir);
    let file = target_dir.join(file_name);
    let reg_path = format!(r"HKLM\{subkey}");
    let status = std::process::Command::new("reg")
        .args(["export", &reg_path])
        .arg(&file)
        .arg("/y")
        .status();
    match status {
        Ok(s) if s.success() => Some(file),
        _ => {
            log::warn!("[清理] 备份 {reg_path} 到 {} 失败，继续清理", file.display());
            None
        }
    }
}

/// yyyymmdd-hhmmss 格式本地时间戳（备份文件名用）
fn local_time_stamp() -> String {
    // SAFETY: GetLocalTime 无副作用参数，仅返回当前本地时间
    let st = unsafe { GetLocalTime() };
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        st.wYear, st.wMonth, st.wDay, st.wHour, st.wMinute, st.wSecond
    )
}

/// 子进程入口：完成两侧清理并以约定退出码结束（0=已清理 2=本来干净 1=失败）
pub fn run_standalone_child() -> ! {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let mut backups = Vec::new();
    let mut changed = false;
    let result = (|| -> Result<(), String> {
        for (subkey, filter, keep) in [
            (KEYBOARD_CLASS_KEY, "keyboard", "kbdclass"),
            (MOUSE_CLASS_KEY, "mouse", "mouclass"),
        ] {
            match clean_filter(&hklm, subkey, filter, keep)? {
                Outcome::Changed { backups: b } => {
                    changed = true;
                    backups.extend(b);
                }
                Outcome::AlreadyClean => {}
            }
        }
        if !backups.is_empty() {
            log::info!("[清理] 备份: {backups:?}");
        }
        Ok(())
    })();
    match result {
        Ok(()) if changed => std::process::exit(0),
        Ok(()) => std::process::exit(2),
        Err(e) => {
            let _ = std::fs::write(error_detail_file(), &e);
            std::process::exit(1);
        }
    }
}

// ---------------------------------------------------------------- 主进程触发

/// 主进程侧完整流程（在线程里跑，避免阻塞托盘事件循环）：
/// 确认 → 提权运行子进程 → 按退出码弹结果。
pub fn cleanup_flow() {
    let ok = msg_box(
        "将移除 v0.1.0 时代残留的 Interception 键盘/鼠标驱动过滤器。\n\
         需要管理员权限（UAC 确认）；清理后重启电脑（或重插鼠标接收器）生效。\n\n\
         现在清理？",
        "nuna 清理驱动残留",
        MB_YESNO | MB_ICONQUESTION,
    ) == IDYES.0;
    if !ok {
        return;
    }

    match run_elevated_child() {
        ElevatedResult::ExitCode(0) => {
            let _ = msg_box(
                "清理完成！已移除 Interception 键盘/鼠标过滤器。\n\
                 备份的 .reg 文件在 nuna 同目录（重装可还原）。\n\n\
                 重启电脑后彻底生效。",
                "nuna 清理驱动残留",
                MB_OK | MB_ICONINFORMATION,
            );
        }
        ElevatedResult::ExitCode(2) => {
            let _ = msg_box(
                "系统当前没有 Interception 过滤器残留，无需清理。",
                "nuna 清理驱动残留",
                MB_OK | MB_ICONINFORMATION,
            );
        }
        ElevatedResult::ExitCode(_) => {
            let detail = std::fs::read_to_string(error_detail_file())
                .unwrap_or_else(|_| "（无详情，详见日志）".to_string());
            let _ = msg_box(
                &format!("清理失败：{detail}"),
                "nuna 清理驱动残留",
                MB_OK | MB_ICONERROR,
            );
        }
        ElevatedResult::Cancelled => {
            // 用户在 UAC 点了"否"，视为主动放弃，不打扰
            log::info!("[清理] 用户取消了提权");
        }
        ElevatedResult::Failed(e) => {
            let _ = msg_box(
                &format!("无法启动清理进程: {e}"),
                "nuna 清理驱动残留",
                MB_OK | MB_ICONERROR,
            );
        }
    }
}

enum ElevatedResult {
    ExitCode(u32),
    /// 用户在 UAC 确认框点了"否"
    Cancelled,
    Failed(String),
}

/// 以 runas 提权重启自身执行 `--cleanup-driver`，阻塞等待其退出码
fn run_elevated_child() -> ElevatedResult {
    let exe = match std::env::current_exe() {
        Ok(p) => p,
        Err(e) => return ElevatedResult::Failed(format!("获取自身路径失败: {e}")),
    };
    let verb = wide("runas");
    let file = wide(exe.to_string_lossy().as_ref());
    let params = wide("--cleanup-driver");
    let mut info = SHELLEXECUTEINFOW {
        cbSize: u32::try_from(std::mem::size_of::<SHELLEXECUTEINFOW>())
            .expect("SHELLEXECUTEINFOW 大小必在 u32 范围内"),
        fMask: SEE_MASK_NOCLOSEPROCESS,
        lpVerb: PCWSTR::from_raw(verb.as_ptr()),
        lpFile: PCWSTR::from_raw(file.as_ptr()),
        lpParameters: PCWSTR::from_raw(params.as_ptr()),
        nShow: SW_HIDE.0,
        ..Default::default()
    };
    // SAFETY: info 为本线程栈上的合法结构体，三个字符串缓冲区在调用期间保持存活；
    // 拒绝 UAC 时返回 ERROR_CANCELLED(1223)
    if let Err(e) = unsafe { ShellExecuteExW(&raw mut info) } {
        return if e.code().0 == 1223 {
            ElevatedResult::Cancelled
        } else {
            ElevatedResult::Failed(format!("{e}"))
        };
    }

    let process = info.hProcess;
    if process.is_invalid() {
        return ElevatedResult::ExitCode(2);
    }
    // SAFETY: hProcess 由 ShellExecuteExW 返回且未被关闭（SEE_MASK_NOCLOSEPROCESS）；
    // 子进程只做注册表修改后即退出，等待不会长期阻塞。u32::MAX 即 INFINITE
    let wait = unsafe { WaitForSingleObject(process, u32::MAX) };
    let mut exit_code: u32 = 1;
    // SAFETY: process 句柄有效且尚未关闭
    let _ = unsafe { GetExitCodeProcess(process, &raw mut exit_code) };
    // SAFETY: 最后一次使用该句柄，关闭后不再访问
    let _ = unsafe { CloseHandle(process) };
    if wait != WAIT_OBJECT_0 {
        return ElevatedResult::Failed("等待清理进程结束时出错".into());
    }
    ElevatedResult::ExitCode(exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_sz_roundtrip() {
        let list = vec!["kbdclass".to_string(), "keyboard".to_string()];
        let encoded = encode_multi_sz(&list);
        assert_eq!(encoded.vtype, REG_MULTI_SZ);
        assert_eq!(decode_multi_sz(&encoded), list);
    }

    #[test]
    fn multi_sz_decode_single_string_and_trailing() {
        // 结尾双 NUL
        let v = RegValue {
            bytes: [u16::from(b'k'), 0, 0].iter().flat_map(|&u| u.to_le_bytes()).collect(),
            vtype: REG_MULTI_SZ,
        };
        assert_eq!(decode_multi_sz(&v), vec!["k".to_string()]);
        // 缺少结尾空串的容错
        let v2 = RegValue {
            bytes: [u16::from(b'a'), 0, u16::from(b'b'), 0].iter().flat_map(|&u| u.to_le_bytes()).collect(),
            vtype: REG_MULTI_SZ,
        };
        assert_eq!(decode_multi_sz(&v2), vec!["a".to_string(), "b".to_string()]);
    }
}
