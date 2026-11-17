# nuna 会话交接文档（2026-11-17）

> 交接背景：按键配置窗口（SharpKeys 式）功能已合入 master，但存在两个待修问题（见第二节）。
> 接力环境要求：Windows + MSVC 构建工具链 + Interception 驱动。当前 master 停在 `548cc9f`。

## 一、项目现状

- 仓库：`github.com:Xtreme-Light/nuna`，分支 master，HEAD `548cc9f`
- 功能：CapsLock 组合键增强工具。Interception 驱动层拦截 → 映射表变换注入 → 托盘常驻；支持 SharpKeys 式按键采集配置
- 发布机制（`.github/workflows/release.yml`）：
  - push master（非 md/docs 改动）→ 构建，产物在 Actions Artifacts
  - push `v*` 标签 → 自动创建 GitHub Release 并附 zip（发版：改 Cargo.toml 版本 → `git tag v0.2.0` → push tag）
  - 支持 Actions 页面 `workflow_dispatch` 手动触发

### 模块结构

| 文件 | 职责 |
|---|---|
| `src/main.rs` | 入口；`AppShared` 共享状态（mappings / capture 通道 / ui_open）；拦截引擎：映射表驱动 + DOWN/UP 严格配对（`InjectedKey` 表）+ 修饰键 flush + 采集模式；panic 全局弹窗钩子 |
| `src/mapping.rs` | 映射模型 `KeyMapping`（原始组合→目标组合，各=修饰键位集+扫描码+E0）；修饰键位常量 `M_*`；默认 17 条映射；`config.json` 读写；按键中文名（`combo_display`） |
| `src/config_ui.rs` | 按键配置窗口（native-windows-gui 1.0.13，独立线程）：采集/列表/增删/恢复默认/保存并应用 |
| `src/tray/mod.rs` | tao 事件循环 + 托盘图标；菜单：按键配置 / 退出 |
| `build.rs` | embed-manifest 内嵌应用程序清单（comctl32 v6 依赖，必须，见下文坑 5） |
| `src/keys/`、`src/oscode/` | 早期遗留的 VK/OSCode 表，基本未使用，可不动 |

### 数据流

托盘点"按键配置" → `config_ui::open(shared)` 起线程开 nwg 窗口 → 点"采集"置 `capture_on=true` →
拦截线程吞掉所有按键、修饰键只记状态，首个非修饰键按下时经 `capture_tx` 上报 `CaptureMsg::Combo` 并自动退出采集 →
窗口 AnimationTimer 轮询 `capture_rx` 更新文本框 → "添加映射" → "保存并应用" 写 `config.json` 并替换 `shared.mappings`（即时生效）。

## 二、已知问题与待办（按优先级）

### 1.【bug·根因已定位】配置窗口"采集"无反应 —— 一行修复

- 现象：点"采集"后按组合键，界面无任何反应（采集文本一直停在"请按下组合键…"）
- 根因：`src/config_ui.rs` 中 `AnimationTimer` 的 builder 没设 `active`，**nwg 默认 `active: false`，定时器从未启动**，`OnTimerTick` 不触发，`capture_rx` 没人轮询
- 修复（已核实 nwg 1.0.13 源码，事件派发路径本身没问题：tick 以 `ControlHandle::Timer(hwnd, id)` 送达 `full_bind_event_handler`，`handle == timer` 比较有效）：

```rust
nwg::AnimationTimer::builder()
    .parent(&window)
    .interval(std::time::Duration::from_millis(100))
    .active(true)          // <-- 补这一行
    .build(&mut timer)?;
```

- 注意：本修复**尚未提交**（按"本地测试通过再推"的约定），接力时先改这里再构建

### 2.【bug·待验证】配置窗口字体模糊

- 现象：产物 `windows-build-548cc9f...` 的配置窗口字体模糊（用户反馈）
- 分析：`build.rs` 用 embed-manifest 默认清单（含 Common-Controls v6 + **PerMonitorV2 DPI 感知** + UTF-8 代码页），有它渲染应是清晰的；模糊疑似进程仍被系统 DPI 位图拉伸 → 清单可能没真正编进 exe
- 本地构建后先验证清单是否在 exe 里：

```powershell
# exe 字节里搜清单内容（嵌入式清单是明文 XML）
Select-String -Path .\nuna.exe -Pattern "Common-Controls" -Quiet   # 应返回 True
```

- 若清单缺失：查 build.rs 的 `embed_manifest()` 调用是否随 release/LTO 生效（embed-manifest 1.5.1，emit 的 link arg 形式）
- 若清单在但还模糊：进程实际 DPI 上下文问题，可在任务管理器"详细信息"加"DPI 感知"列确认；必要时窗口按 `WM_DPICHANGED`/`GetDpiForWindow` 缩放控件

### 3. MSVC 环境本地构建与打包

```powershell
cargo test                 # mapping.rs 有单测：默认17条/序列化往返/显示名/配对
cargo build --release
# 打包 = 3 样东西放同一目录：
#   target/release/nuna.exe
#   Interception/library/x64/interception.dll   （exe 动态链接，必须同目录）
#   Interception/ 整个目录（驱动安装器，给没装驱动的机器）
```

### 4. 功能回归清单

- [ ] 托盘出现熊猫图标；菜单"按键配置"能开窗、重复点击不开第二窗
- [ ] 采集：原始键位 Caps+C → 显示"Caps + C"；映射为 Ctrl+C；Esc 取消生效
- [ ] 添加 → 列表出现；删除选中；恢复默认（17 条）
- [ ] 保存并应用 → exe 同目录生成 `config.json`，映射立即生效；重启后读取自定义配置
- [ ] 采集期间按键完全被吞（Caps 不切换大小写锁定）
- [ ] 影子按键回归：caps+C 后**先松 caps 再松 C**；caps+方向移动中松 caps；云桌面/远程桌面里重复
- [ ] 真实修饰键映射（如 Ctrl+X→Home）：命中时不会变成 Ctrl+Home（抵消/恢复逻辑），任意松开顺序不卡键
- [ ] 界面清晰度（问题 2）

## 三、环境与历史坑（已排查过，勿重复劳动）

1. **Interception 驱动**：服务名是 `keyboard` 和 `mouse`（不是 `interception`），驱动文件 `system32\drivers\keyboard.sys`+`mouse.sys`（Oblita 签名）。设备 `\\.\interception00`~`09` **普通权限**可打开，程序无需管理员。已装驱动机器**不要再跑 install-interception**（文件被运行中的驱动占用，会报 `Could not write to \system32\drivers`）
2. `nuna.exe` 动态链接 `interception.dll`（来自 interception-sys），**必须同目录**；CI 打包已处理（zip 根目录 exe+dll，Interception 目录附驱动安装器）
3. 托盘图标已 `include_bytes!` 内嵌；早期版本用 `CARGO_MANIFEST_DIR` 编译期路径在 CI 产物上闪退（0xC0000409），勿回退
4. 有全局 panic 弹窗钩子（`fatal_dialog`），GUI 下任何 panic 可见；`panic="abort"` + `windows_subsystem="windows"`
5. `GetWindowSubclass` 启动报错 = exe 未内嵌 comctl32 v6 清单（nwg 子类化 API 只在 v6），`build.rs` 已修，勿删
6. 原测试机（本机）**无 MSVC**（安装被中止），装了 rustup + gnu 工具链：只能 `cargo check`（gnu 链接 windows 系列 crate 的 raw-dylib 需要 dlltool，自带 shim 起不来）
7. 网络：12334 本地代理时开时关。代理挂了 git push 失败时用 443 直连：
   `git -c core.sshCommand="ssh -o ProxyCommand=none -o HostName=ssh.github.com -o Port=443" push origin master`
   GitHub API 直连也时断时续，浏览器或 curl 加 `-x http://127.0.0.1:12334`（代理开着时）

## 四、关键设计决策（改动前先读）

- **映射匹配是精确匹配**：`src_mods == held_mods`（不是子集匹配）。旧写死逻辑是"只要按着 Caps 就触发"；现在 `Shift+Caps+H` 不会触发 `Caps+H`，需要变体就单独采集一条
- **影子按键修复的核心机制勿破坏**（详见 main.rs 注释）：注入 DOWN 时成对记录 UP 序列；源修饰键松开立即 flush 相关注入条目；真实修饰键组合命中时先抵消抬起、释放时按需恢复按下
- 扫描码表示法：右Ctrl=`0x1D+E0`、右Alt=`0x38+E0`、Win=`0x5B/0x5C+E0`、方向/编辑键=小键盘码+E0（如 ←=`0x4B+E0`）；采集记录的 `(code, ext)` 可直接原样注入
- 配置窗口 nwg 独立线程 + 主线程 tao 托盘循环并存，互不干扰；`ui_open` 原子标志防重复开窗；窗口关闭时强制 `capture_on=false`（防键盘被一直吞）
- `config.json`：`{"version":1,"mappings":[...]}`，exe 同目录（便携友好）；损坏时回退默认并弹窗告知

## 五、近期提交索引

| 提交 | 内容 |
|---|---|
| `4f0f6a3` | 影子按键修复（DOWN/UP 配对、松 caps flush、左Ctrl去E0） |
| `c2e97f5` | 打包带 interception.dll；驱动缺失/重复启动弹窗提示 |
| `395bd42` | 托盘图标内嵌修复 CI 产物闪退；全局 panic 弹窗 |
| `676faed` | workflow 支持 v* 标签自动发版 |
| `2b751bd` | SharpKeys 式按键配置窗口 + 拦截引擎映射表化 |
| `32f5c9b` | workflow_dispatch 手动触发 |
| `548cc9f` | build.rs 内嵌清单修 GetWindowSubclass 启动失败 |
