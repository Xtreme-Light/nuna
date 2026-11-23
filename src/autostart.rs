//! 开机自启：写当前用户 `HKCU\...\CurrentVersion\Run` 注册表键。
//! 用户级配置、无需管理员权限，与 v0.1.1「免驱动免提权、解压即用」的定位一致。

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
use winreg::RegKey;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "nuna";

/// 是否已启用开机自启（Run 键里存在 nuna 项即视为启用）
pub fn is_enabled() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(RUN_KEY, KEY_READ)
        .ok()
        .and_then(|key| key.get_value::<String, _>(VALUE_NAME).ok())
        .is_some()
}

/// 启用：把当前 exe 的绝对路径写进 Run 键（带引号，兼容含空格的路径）
pub fn enable() -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    let command = format!("\"{}\"", exe.display());
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu.create_subkey(RUN_KEY)?;
    key.set_value(VALUE_NAME, &command)
}

/// 禁用：删除 Run 键里的 nuna 项；项不存在视为成功（幂等）
pub fn disable() -> std::io::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu.open_subkey_with_flags(RUN_KEY, KEY_WRITE) {
        Ok(key) => match key.delete_value(VALUE_NAME) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 直接读写当前用户真实的 HKCU Run 键做往返验证（无权限要求），
    /// 结束时恢复进入测试前的状态
    #[test]
    fn enable_disable_roundtrip() {
        let originally_enabled = is_enabled();

        disable().expect("清理失败");
        assert!(!is_enabled(), "禁用后应检测为未启用");

        enable().expect("启用失败");
        assert!(is_enabled(), "启用后应检测为已启用");
        // 写入的命令行必须是带引号的当前 exe 绝对路径
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let key = hkcu.open_subkey_with_flags(RUN_KEY, KEY_READ).unwrap();
        let value: String = key.get_value(VALUE_NAME).unwrap();
        let exe = std::env::current_exe().unwrap();
        assert_eq!(value, format!("\"{}\"", exe.display()));

        if originally_enabled {
            enable().expect("恢复启用状态失败");
        } else {
            disable().expect("恢复禁用状态失败");
        }
    }
}
