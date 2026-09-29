# Rebuild Xime and test with the real install effect (MSIX development registration).
#
# Flow mirrors msix-bundle.ps1 -Register, so testing happens against the exact
# layout that gets installed:
#   1. build release (windows_subsystem -> no console window)
#   2. stage install layout to target\msix-pkg (binaries + rime.dll + data + user-data + resources)
#   3. Add-AppxPackage -Register (loose-file development registration = install effect)
#   4. start winxime-server.exe from the staged layout (same as MSI's StartServer action)
#
# Notes:
#   - Auto-elevates via UAC (the server self-registers the TSF DLL, which writes HKLM).
#   - target\msix-pkg must stay on disk: the registered package points at that folder
#     (it plays the role of C:\Program Files\WindowsApps for a real install).
#   - User data lives in %APPDATA%\Xime and persists across rebuilds, like a real install.
#   - Logs: %TEMP%\winxime\*.log

$ErrorActionPreference = "Stop"

$isAdmin = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $isAdmin) {
    # 自动以管理员重启本脚本：UAC 确认后在新窗口继续执行（-NoExit 保持窗口显示输出）。
    try {
        Start-Process powershell.exe -Verb RunAs -ArgumentList @(
            "-NoExit", "-ExecutionPolicy", "Bypass", "-File", "$PSCommandPath"
        ) | Out-Null
    } catch {
        Write-Host "UAC cancelled; run again to retry." -ForegroundColor Red
        exit 1
    }
    exit
}

# cargo 与 msix-bundle.ps1 都按仓库根目录解析相对路径；提权重启后 cwd 是 System32，统一锚定。
Set-Location -LiteralPath $PSScriptRoot

$packageDir = "$PSScriptRoot\target\msix-pkg"

Write-Host "Step 0: Clearing old logs..." -ForegroundColor Yellow
Remove-Item "$env:TEMP\winxime\*.log" -Force -ErrorAction SilentlyContinue

Write-Host "Step 1: Stopping server and setup..." -ForegroundColor Yellow
# 优雅退出：用上一次构建的 server /q（IPC shutdown）；都没有时跳过，稍后强制结束兜底。
foreach ($exe in @(
    "$packageDir\winxime-server.exe",
    "$PSScriptRoot\target\release\winxime-server.exe",
    "$PSScriptRoot\target\debug\winxime-server.exe"
)) {
    if (Test-Path $exe) {
        Start-Process -FilePath $exe -ArgumentList "/q" -Wait -ErrorAction SilentlyContinue
        break
    }
}
# setup 也要停：暂存目录被占用会导致复制失败。
# 注意：不能直接 Get-Process | Stop-Process——无匹配进程时管道为空，
# Stop-Process 的必选参数 Id 会进入交互式提示（-ErrorAction 压不住）。
$staleProcesses = Get-Process -Name "winxime-server", "winxime-setup" -ErrorAction SilentlyContinue
if ($staleProcesses) {
    $staleProcesses | Stop-Process -Force -ErrorAction SilentlyContinue
}
Start-Sleep -Seconds 2

Write-Host "Step 2: Building release..." -ForegroundColor Yellow
cargo build --release --quiet
if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}

Write-Host "Step 3: Staging install layout + MSIX registration..." -ForegroundColor Yellow
& "$PSScriptRoot\msix-bundle.ps1" -Register
if ($LASTEXITCODE -ne 0) {
    Write-Host "Staging/registration failed!" -ForegroundColor Red
    exit 1
}

Write-Host "Step 4: Starting server from the staged layout..." -ForegroundColor Yellow
Start-Process -FilePath "$packageDir\winxime-server.exe"
Start-Sleep -Seconds 3

if (Get-Process -Name "winxime-server" -ErrorAction SilentlyContinue) {
    Write-Host ""
    Write-Host "Done! Server is running from the installed layout (no console window)." -ForegroundColor Green
    Write-Host "  Package layout: $packageDir" -ForegroundColor White
    Write-Host "  Logs:           $env:TEMP\winxime\*.log" -ForegroundColor White
    Write-Host "Test input in Notepad or any application." -ForegroundColor White
    Write-Host "NOTE: TSF DLL 是进程内加载的，请关掉重开要测试的应用（旧进程仍用旧 DLL）" -ForegroundColor Yellow
} else {
    Write-Host "Server did not start; check logs at $env:TEMP\winxime" -ForegroundColor Red
    exit 1
}
