# HANDOFF — v0.1.1 分支交接文档

> 分支：`fix/v0.1.1-replace-interception-with-llhook`（基于 tag `v0.1.0`）
> 本文档为会话交接用，**打 v0.1.1 正式标签前请删除本文件**。

## 背景：v0.1.1 要解决什么问题

v0.1.0 打包的 Interception 驱动安装器（`install-interception /install`）会默认把
`keyboard` 和 `mouse` 两个内核过滤驱动写入注册表。其中 oblitum 的 `mouse.sys`
过滤驱动存在缺陷：**USB 鼠标被拔插（设备重新枚举）后会吞掉鼠标输入事件**，
表现为设备管理器正常、配对正常、但光标完全不动，触摸板正常，换口无效
（2026-11-23 在作者本机实测复现并定位，根因分析见 Release 说明草稿）。

v0.1.1 的目标：**彻底脱离 Interception**，改用 `WH_KEYBOARD_LL` 低级键盘钩子
（参考 kanata 的 LLHOOK 后端），免驱动、免管理员；同时提供清理工具给 v0.1.0
老用户裁掉残留的鼠标过滤器。作者明确：**未来不做鼠标侧改键**。

## 本分支已完成的内容

| 文件 | 变更 |
|---|---|
| `src/main.rs` | **全部重写**：移除 interception，改为 WH_KEYBOARD_LL 钩子线程 + 消息泵 + 独立注入线程（crossbeam channel），回调决定吞/放行，注入用 SendInput（VK 级）。CapsLock 层语义与 v0.1.0 完全一致（严格 DOWN/UP 配对、caps 抬起时释放全部注入键、repeat 重发 downs） |
| `src/keys/mod.rs` | 删除 `all_scan_codes()` 与 ScanCode 依赖，补齐 `VK_A`..`VK_Z` 字母常量（0x41-0x5A） |
| `src/oscode/` | 整个删除（扫描码枚举，不再需要） |
| `src/tray/mod.rs` | `init_tray()` 去掉 exit channel 参数（退出改为主线程托盘返回后 `PostThreadMessageW(WM_QUIT)` 通知钩子线程） |
| `Cargo.toml` | version 0.1.1；移除 `kanata-interception`；windows crate 增加 `Win32_Foundation`、`Win32_System_Threading` feature |
| `Interception/` | **整个目录删除**（驱动安装包、dll、lib） |
| `script/cleanup-interception.ps1` | 新增：清理工具。默认只裁鼠标侧 `mouse` 过滤器；`-RemoveKeyboardToo` 连键盘侧一并裁。自动提权、自动备份 .reg、幂等。**注意：必须保持 UTF-8 with BOM 编码**（Windows PowerShell 5.1 无 BOM 会把中文注释读成乱码导致语法错误——已踩过坑并修复） |
| `script/cleanup-interception.bat` | 新增：双击入口（自动提权跑 ps1） |
| `README.md` | 新增「v0.1.1 重要变更」章节：问题说明 + 升级清理指引 |
| `.github/workflows/release.yml` | 1) 打包不再需要 interception.dll，改为附带 `script/` 清理工具；2) Release body 已写好 v0.1.1 版本说明（问题、修复、老用户清理步骤）——推 v* 标签即自动发版 |

## 已验证 ✅

- `cargo check` 通过（0 error 0 warning，本机 Rust 1.99.0）
- `cleanup-interception.ps1` PowerShell 5.1 语法解析通过
- 清理逻辑本身已在作者本机真机验证过（2026-11-23 手工执行的等效注册表操作
  立即修复了鼠标失灵，脚本逻辑与之一致）
- release.yml YAML 解析通过

## 未完成 ❌（接手会话的 TODO，按顺序）

1. **release 构建验证**。本机 Rust 是 `x86_64-pc-windows-gnu` host（缺 dlltool，
   无法本地链接），安装 msvc target 的操作被中断。两个选择：
   - 本机：`rustup target add x86_64-pc-windows-msvc`（需 VS BuildTools），
     然后 `cargo build --release --target x86_64-pc-windows-msvc`
   - 或直接信 CI（CI 的 windows-latest + msvc target 是齐全的）
2. **CI 不在本分支触发**：workflow 的 push 触发只配了 `master` 和 `v*` 标签。
   建议先给 release.yml 加 `workflow_dispatch:` 触发器并手动跑一次构建验证，
   通过后再打标签（避免 tag 推上去构建失败还要删 release）。
3. **冒烟测试**（作者本机）：退出旧版 nuna.exe → 运行新版 → 验证
   caps+hjkl/a/e/d/space/ctrl 组合映射正常、鼠标正常、**拔插一次鼠标接收器
   鼠标依然正常**（v0.1.0 的复现路径）、托盘退出无残留卡键。
4. **删除本文件 HANDOFF.md**。
5. **打标签发版**：commit 全部就绪后 `git tag v0.1.1 && git push origin v0.1.1`，
   CI 自动构建并以 release.yml 里写好的 body 创建 GitHub Release
   （zip 名：`nuna-windows-x64-v0.1.1.zip`）。
6. **合并回 master**（作者自己决定时机；master 领先 v0.1.0 不少，可能有冲突，
   本分支改动集中在 main.rs/keys/tray/Cargo.toml/workflow/README）。

## 技术决策与踩坑记录（接手会话速查）

- **windows crate 0.61 API**：`SendInput(&[INPUT], cbsize)`（切片，不是指针+长度）；
  `KBDLLHOOKSTRUCT.flags.contains(LLKHF_INJECTED)`（新类型 + contains，不是位与）；
  `HC_ACTION as i32`；吞事件返回 `LRESULT(1)`。
- **VK 常量撞名**：windows crate 的 `VK_*` 是 `VIRTUAL_KEY` 新类型，与
  `src/keys/mod.rs` 的 `u16` 常量同名——main.rs **不要**从 windows crate import
  VK_*，统一用 `crate::keys::*` 的 u16 常量；`VIRTUAL_KEY` 类型本身仍需 import
  （SendInput 用）。
- **注入线程独立**：不在钩子回调里直接 SendInput（慢钩子会被系统超时旁路），
  回调只做决策 + channel 发送，对齐 kanata 的结构。
- **armed 机制**：不再像 v0.1.0 那样阻塞等待所有键释放，而是钩子装好后先放行
  所有事件，直到 `are_all_keys_released()` 为真才接管（避免启动期接管半截按键流，
  也避免启动阻塞导致退出挂起）。
- **GitHub 访问**：作者本机直连 GitHub 被重置，git 操作需带本地代理：
  `git -c http.proxy=http://127.0.0.1:12334 -c https.proxy=http://127.0.0.1:12334 ...`

## v0.1.1 行为变化说明（面向用户）

- 无需安装驱动、无需管理员、解压即用；不再写注册表过滤器
- CapsLock 映射行为与 v0.1.0 一致（注入从扫描码级改为 VK 级，对用户无感；
  云桌面/远程桌面兼容性理论等价或更好）
- CapsLock 本身仍被完全吞掉（不开大写锁定），与 v0.1.0 一致
