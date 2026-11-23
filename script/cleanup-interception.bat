@echo off
chcp 65001 >nul
REM nuna Interception 过滤器清理工具（鼠标侧）
REM 双击运行：自动请求管理员权限，移除导致 USB 鼠标热插拔后失灵的 mouse 过滤器
REM 如需连键盘侧过滤器一并清理，请用管理员 PowerShell 运行：
REM   powershell -ExecutionPolicy Bypass -File cleanup-interception.ps1 -RemoveKeyboardToo

set "SCRIPT=%~dp0cleanup-interception.ps1"
if not exist "%SCRIPT%" (
    echo 错误: 未找到 %SCRIPT%
    pause
    exit /b 1
)

powershell -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT%"
