@echo off
chcp 65001 >nul
setlocal enabledelayedexpansion

REM 设置程序路径和进程名
set "EXE_PATH=C:\Users\AI\Desktop\nuna-windows-x64\nuna.exe"
set "PROCESS_NAME=nuna.exe"

REM 检查程序文件是否存在
if not exist "%EXE_PATH%" (
    echo 错误: 找不到程序文件
    echo 路径: %EXE_PATH%
    pause
    exit /b 1
)

REM 检查程序是否已在运行
tasklist /FI "IMAGENAME eq %PROCESS_NAME%" 2>NUL | find /I "%PROCESS_NAME%" >NUL
if !ERRORLEVEL! equ 0 (
    echo 程序 "%PROCESS_NAME%" 已在运行中
    REM 获取并显示进程ID
    for /f "tokens=2" %%i in ('tasklist /FI "IMAGENAME eq %PROCESS_NAME%" /FO TABLE /NH') do (
        echo 运行中的进程 ID: %%i
    )
    timeout /t 5 /nobreak >nul
    exit /b 0
)

REM 启动程序
echo 正在启动程序...
start "" /B "%EXE_PATH%"

REM 等待程序启动
echo 等待程序启动...
timeout /t 3 /nobreak >nul

REM 检查是否成功启动
tasklist /FI "IMAGENAME eq %PROCESS_NAME%" 2>NUL | find /I "%PROCESS_NAME%" >NUL
if !ERRORLEVEL! equ 0 (
    echo 程序启动成功!
    for /f "tokens=2" %%i in ('tasklist /FI "IMAGENAME eq %PROCESS_NAME%" /FO TABLE /NH') do (
        echo 进程 ID: %%i
    )
) else (
    echo 程序启动失败!
    echo 请检查程序路径和权限
)

REM 保持窗口打开一段时间以便查看结果
timeout /t 5 /nobreak >nul