# nuna

## capslock 增强工具

为capslock做增强，增加capslock的使用率

键位映射列表

1. CAPS + A = HOME (按下Caps+ A组合键，发送HOME键，调到行首)
2. CAPS + E = END (调到行尾)
3. CAPS + H = LEFT (左移)
4. CAPS + J = UP （上移）
5. CAPS + K = DOWN （下移）
6. CAPS + L = RIGHT （右移）
7. CAPS + F  = CTRL + RIGHT （光标按单词向右调）
8. CAPS + B  = CTRL + LEFT（光标按单词向左调）
9. CAPS + SPACE  = BACKSPACE（退格键）
10. CAPS + D = DEL（删除键）
11. CAPS + Q = CTRL + A
12. CAPS + S =  CTRL + S 
13. CAPS + W =  CTRL + W 
14. CAPS + Z = CTRL + Z 
15. CAPS + X = CTRL + X
16. CAPS + C = CTRL + C
17. CAPS + V = CTRL + V

## v0.1.1 重要变更：移除 Interception 驱动依赖

**v0.1.1 起无需安装任何驱动、无需管理员权限，解压即用。** 输入拦截改用
Windows 低级键盘钩子（`WH_KEYBOARD_LL`，实现参考 [kanata](https://github.com/jtroo/kanata)
的 LLHOOK 后端），键位映射行为与 v0.1.0 保持一致。

### 为什么移除 Interception

v0.1.0 打包的 Interception 驱动安装器（`install-interception /install`）会默认把
`keyboard` 和 `mouse` 两个内核过滤驱动都写入系统注册表，而其中的 `mouse.sys`
过滤驱动存在缺陷：**当 USB 鼠标被拔插（设备重新枚举）后，它会吞掉鼠标输入事件**，
表现为：

- 设备管理器里鼠标一切正常；
- 鼠标和接收器配对正常（罗技用户用 logiwebconnect.com 也能识别到）；
- 但光标完全不动，触摸板正常，换 USB 口也无效。

nuna 本身只订阅键盘事件，但安装器把鼠标过滤器也一并装了，属于"躺着中枪"。
v0.1.1 从根源上解决：不再依赖 Interception，也就不再写入任何过滤器。

### 从 v0.1.0 升级的用户请清理鼠标过滤器

如果你装过 v0.1.0（跑过 `install-interception /install`），请运行发行包里的
清理工具把残留的过滤器裁掉：

- **双击 `script\cleanup-interception.bat`**：自动请求管理员权限，只移除鼠标侧的
  `mouse` 过滤器（修复上述鼠标失灵问题，不影响 v0.1.0 的键盘映射——如果你还要
  继续用 v0.1.0 的话）；
- 已经升级到 v0.1.1 的用户推荐彻底清理（键盘侧过滤器留着也只有风险没有收益）：

  ```powershell
  powershell -ExecutionPolicy Bypass -File script\cleanup-interception.ps1 -RemoveKeyboardToo
  ```

- 清理后**重启电脑**，或至少把鼠标接收器重新插拔一次即可生效；
- 工具会自动把注册表键备份为 `.reg` 文件，双击导入即可还原；
- 也可以用 v0.1.0 附带的 `install-interception.exe /uninstall`（管理员运行并重启）
  彻底卸载 Interception 驱动本体。

## roadmap

- [x] 支持以windows托盘程序的形式启动
- [x] 支持CAPS的基本增强，包括光标的上下左右移动、退格、删除等
- [x] v0.1.1：移除 Interception 内核驱动依赖，改用 WH_KEYBOARD_LL，免驱动免管理员
- [ ] 硬编码键位映射改为配置文件配置
