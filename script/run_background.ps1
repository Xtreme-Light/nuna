# 定义程序路径
$exePath = "C:\Users\AI\Desktop\nuna-windows-x64\nuna.exe"
$processName = "nuna"

# 检查程序是否已在运行
$existingProcesses = Get-Process -Name $processName -ErrorAction SilentlyContinue

if ($existingProcesses) {
    # 程序已在运行，退出并返回代码1
    exit 1
}

# 启动程序并隐藏窗口
try {
    $process = Start-Process -FilePath $exePath -WindowStyle Hidden -PassThru
    # 记录启动日志（可选）
    "$(Get-Date): 程序已启动，PID: $($process.Id)" | Out-File "$env:LOCALAPPDATA\Nuna\nuna_startup.log" -Append
}
catch {
    # 记录错误信息
    "$(Get-Date): 启动错误 - $($_.Exception.Message)" | Out-File "$env:LOCALAPPDATA\Nuna\nuna_error.log" -Append
    exit 2
}