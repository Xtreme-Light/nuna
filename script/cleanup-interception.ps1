# cleanup-interception.ps1 — nuna Interception 过滤驱动清理工具
#
# 背景：nuna v0.1.0 打包的 Interception 驱动安装器（install-interception /install）
# 会默认把 keyboard 和 mouse 两个内核过滤驱动都挂进系统。其中 mouse.sys 过滤
# 驱动存在缺陷：当 USB 鼠标被拔插（设备重新枚举）后，它会吞掉鼠标输入事件，
# 表现为「设备管理器里鼠标正常、但光标完全不动」，且换 USB 口无效。
#
# nuna v0.1.1 起已改用 WH_KEYBOARD_LL 低级键盘钩子，不再需要任何内核驱动。
# 本工具用于把 v0.1.0 时代写入注册表的 Interception 过滤器裁掉：
#   - 默认只裁掉鼠标侧的 mouse 过滤器（修复上述鼠标失灵问题）
#   - 加 -RemoveKeyboardToo 参数时同时裁掉键盘侧的 keyboard 过滤器
#     （v0.1.1+ 用户推荐：LLHOOK 方案完全不依赖 Interception，
#      留着旧过滤器只有风险没有收益；但如果你还想继续用 v0.1.0，别加这个参数）
#
# 用法（需管理员权限，未提权时会自动弹出 UAC）：
#   powershell -ExecutionPolicy Bypass -File cleanup-interception.ps1
#   powershell -ExecutionPolicy Bypass -File cleanup-interception.ps1 -RemoveKeyboardToo
#
# 生效方式：清理后重启电脑；或至少把鼠标接收器/鼠标重新插拔一次。

param(
    [switch]$RemoveKeyboardToo
)

$ErrorActionPreference = 'Stop'

# ---------- 自动提权 ----------
$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()
    ).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    Write-Host "需要管理员权限，正在请求提权（请在 UAC 窗口点“是”）..." -ForegroundColor Yellow
    $argList = @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $PSCommandPath)
    if ($RemoveKeyboardToo) { $argList += '-RemoveKeyboardToo' }
    Start-Process powershell -Verb RunAs -ArgumentList $argList | Out-Null
    exit
}

# ---------- 工具函数 ----------
$MouseClassKey    = 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e96f-e325-11ce-bfc1-08002be10318}'
$KeyboardClassKey = 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e96b-e325-11ce-bfc1-08002be10318}'

function Get-Filters([string]$KeyPath) {
    $v = (Get-ItemProperty -Path $KeyPath -Name UpperFilters -ErrorAction SilentlyContinue).UpperFilters
    if ($null -eq $v) { return @() }
    # UpperFilters 可能是 REG_SZ（单值）或 REG_MULTI_SZ（数组）
    if ($v -is [array]) { return $v } else { return @($v) }
}

function Remove-Filter([string]$KeyPath, [string]$FilterName, [string]$KeepDefault) {
    $filters = Get-Filters $KeyPath
    if ($filters -notcontains $FilterName) {
        Write-Host "  [$FilterName] 过滤器不存在，无需清理。" -ForegroundColor DarkGray
        return $false
    }

    # 备份：与脚本同目录（不可写则退回 %TEMP%）
    $backupDir = try { $PSScriptRoot } catch { $env:TEMP }
    $regPath = $KeyPath -replace '^HKLM:\\', 'HKLM\'
    $backupFile = Join-Path $backupDir ("backup-{0}-{1}.reg" -f $FilterName, (Get-Date -Format 'yyyyMMdd-HHmmss'))
    reg export $regPath $backupFile /y | Out-Null
    Write-Host "  已备份注册表键到: $backupFile" -ForegroundColor Cyan

    $newFilters = @($filters | Where-Object { $_ -ne $FilterName -and $_ -ne '' })
    if ($newFilters.Count -eq 0) { $newFilters = @($KeepDefault) } # 兜底：绝不能留空

    # 统一写回 REG_MULTI_SZ（设备过滤器的标准类型）
    New-ItemProperty -Path $KeyPath -Name UpperFilters -Value $newFilters `
        -PropertyType MultiString -Force | Out-Null

    Write-Host ("  [{0}] 已移除: {1}  =>  当前: {2}" -f $FilterName, ($filters -join ', '), ($newFilters -join ', ')) -ForegroundColor Green
    return $true
}

# ---------- 执行 ----------
Write-Host ""
Write-Host "=== nuna Interception 过滤驱动清理工具 ===" -ForegroundColor White
Write-Host ""

Write-Host "[1/2] 鼠标类过滤驱动（修复 USB 鼠标热插拔后失灵问题）:" -ForegroundColor White
$changedMouse = Remove-Filter $MouseClassKey 'mouse' 'mouclass'

Write-Host ""
if ($RemoveKeyboardToo) {
    Write-Host "[2/2] 键盘类过滤驱动（-RemoveKeyboardToo，v0.1.1+ 推荐一并清理）:" -ForegroundColor White
} else {
    Write-Host "[2/2] 键盘类过滤驱动（默认保留；加 -RemoveKeyboardToo 参数可一并清理）:" -ForegroundColor White
}
$changedKbd = $false
if ($RemoveKeyboardToo) {
    $changedKbd = Remove-Filter $KeyboardClassKey 'keyboard' 'kbdclass'
} else {
    $kbdFilters = Get-Filters $KeyboardClassKey
    Write-Host ("  当前键盘过滤器: {0}（未改动）" -f ($kbdFilters -join ', ')) -ForegroundColor DarkGray
}

Write-Host ""
if ($changedMouse -or $changedKbd) {
    Write-Host "清理完成。生效方式（二选一）：" -ForegroundColor Yellow
    Write-Host "  1. 重启电脑"
    Write-Host "  2. 或至少把鼠标接收器/鼠标重新插拔一次"
    Write-Host ""
    Write-Host "如需还原：双击备份的 .reg 文件导入并重启即可。" -ForegroundColor DarkGray
    Write-Host "如需彻底卸载 Interception 驱动本体（可选）：" -ForegroundColor DarkGray
    Write-Host "  以管理员运行 nuna v0.1.0 附带的 install-interception.exe 的 /uninstall 并重启。"
} else {
    Write-Host "没有需要清理的内容，系统当前是干净的。" -ForegroundColor Green
}
Write-Host ""
Read-Host "按回车键关闭"
