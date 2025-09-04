# win + r  => shell:startup
# 启动程序并隐藏窗口（后台运行）
$process = Start-Process -FilePath "path\to\your\nuna.exe" -NoNewWindow -PassThru

# 输出进程信息
Write-Host "程序已在后台启动，进程 ID: $($process.Id)"
Write-Host "结束程序可执行：Stop-Process -Id $($process.Id)"