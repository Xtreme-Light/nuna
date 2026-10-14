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





## 按键配置（类似 SharpKeys）

托盘图标右键 -> “按键配置” 打开配置窗口：

1. 点击“原始键位”旁的**采集**，直接按下组合键（如先按住 `CapsLock` 再按 `C`），按 `Esc` 可取消
2. 点击“映射为”旁的**采集**，按下目标组合键（如 `Ctrl + C`）
3. 点击**添加映射**，新映射进入列表
4. **保存并应用** 后立即生效，无需重启（配置持久化在 exe 同目录的 `config.json`）

列表中选中一行可**删除选中**；**恢复默认** 回到上表内置键位。采集期间按键被完全拦截，
不会真的触发（例如采集 CapsLock 时不会切换大小写锁定）。

## roadmap

- [x] 支持以windows托盘程序的形式启动
- [x] 支持CAPS的基本增强，包括光标的上下左右移动、退格、删除等
- [x] 硬编码键位映射改为配置文件配置（托盘菜单“按键配置”，支持按键采集）





