# Full uninstall script for 曦码·曜 (Xime Yao) (including user data)
# This script removes everything: TSF DLL, registry, install dir, and user data

Write-Host "=== 曦码·曜 (Xime Yao) Full Uninstall ===" -ForegroundColor Cyan

$registerExe = "$PSScriptRoot\target\release\winxime-tsf-register.exe"
if (-not (Test-Path $registerExe)) {
    $registerExe = "$PSScriptRoot\target\debug\winxime-tsf-register.exe"
}

if (-not (Test-Path $registerExe)) {
    Write-Host "Error: winxime-tsf-register.exe not found" -ForegroundColor Red
    Write-Host "Please build the project first: cargo build --release" -ForegroundColor Yellow
    exit 1
}

Write-Host "This will remove:" -ForegroundColor Yellow
Write-Host "  - TSF DLL from System32" -ForegroundColor White
Write-Host "  - Registry keys (HKLM\SOFTWARE\Xime, HKCU\SOFTWARE\Xime)" -ForegroundColor White
Write-Host "  - Run startup entry" -ForegroundColor White
Write-Host "  - Install directory (C:\Program Files\Xime)" -ForegroundColor White
Write-Host "  - User data directory (%APPDATA%\Xime)" -ForegroundColor White
Write-Host ""

$confirm = Read-Host "Continue? (Y/N)"
if ($confirm -ne "Y" -and $confirm -ne "y") {
    Write-Host "Cancelled" -ForegroundColor Yellow
    exit 0
}

Write-Host ""
Write-Host "Step 1: Disabling input method..." -ForegroundColor Yellow
Start-Process -Verb RunAs -Wait -FilePath $registerExe -ArgumentList "-d"

Write-Host ""
Write-Host "Step 2: Running TSF unregister..." -ForegroundColor Yellow
Start-Process -Verb RunAs -Wait -FilePath $registerExe -ArgumentList "-unregister-and-remove"

Write-Host ""
Write-Host "Step 3: Removing install directory..." -ForegroundColor Yellow
$installDir = "C:\Program Files\Xime"
if (Test-Path $installDir) {
    Start-Process -Verb RunAs -Wait -FilePath "cmd.exe" -ArgumentList "/c", "rmdir", "/s", "/q", $installDir
}

Write-Host ""
Write-Host "Step 4: Removing user data directory..." -ForegroundColor Yellow
$userDataDir = "$env:APPDATA\Xime"
if (Test-Path $userDataDir) {
    Remove-Item -Path $userDataDir -Recurse -Force
}

Write-Host ""
Write-Host "Step 5: Cleaning registry..." -ForegroundColor Yellow
Start-Process -Verb RunAs -Wait -FilePath "reg.exe" -ArgumentList "delete", "HKCR\CLSID\{5C1E4D8A-F3B2-4A7E-9CD1-2A3B4C5D6E7F}", "/f" -ErrorAction SilentlyContinue
Start-Process -Verb RunAs -Wait -FilePath "reg.exe" -ArgumentList "delete", "HKLM\SOFTWARE\Microsoft\CTF\TIP\{5C1E4D8A-F3B2-4A7E-9CD1-2A3B4C5D6E7F}", "/f" -ErrorAction SilentlyContinue
Start-Process -Verb RunAs -Wait -FilePath "reg.exe" -ArgumentList "delete", "HKLM\SOFTWARE\Classes\CLSID\{5C1E4D8A-F3B2-4A7E-9CD1-2A3B4C5D6E7F}", "/f" -ErrorAction SilentlyContinue
Start-Process -Verb RunAs -Wait -FilePath "reg.exe" -ArgumentList "delete", "HKLM\SOFTWARE\Xime", "/f" -ErrorAction SilentlyContinue
Start-Process -Verb RunAs -Wait -FilePath "reg.exe" -ArgumentList "delete", "HKCU\SOFTWARE\Xime", "/f" -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "Step 6: Attempting to remove System32 DLL..." -ForegroundColor Yellow
$systemDll = "C:\Windows\System32\winxime_tsf.dll"
if (Test-Path $systemDll) {
    Start-Process -Verb RunAs -Wait -FilePath "cmd.exe" -ArgumentList "/c", "del", "/q", $systemDll -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "Step 7: Verifying uninstall..." -ForegroundColor Yellow
$errors = @()

if (Test-Path $systemDll) {
    $errors += "System32 DLL still exists (will be removed after restart)"
}
if (Test-Path $installDir) {
    $errors += "Install directory still exists"
}
if (Test-Path $userDataDir) {
    $errors += "User data directory still exists"
}
$clsid = reg query "HKCR\CLSID\{5C1E4D8A-F3B2-4A7E-9CD1-2A3B4C5D6E7F}" 2>$null
if ($clsid) {
    $errors += "CLSID registry key still exists"
}
$tip = reg query "HKLM\SOFTWARE\Microsoft\CTF\TIP\{5C1E4D8A-F3B2-4A7E-9CD1-2A3B4C5D6E7F}" 2>$null
if ($tip) {
    $errors += "TIP registry key still exists"
}

Write-Host ""
if ($errors.Count -gt 0) {
    Write-Host "WARNING: Some items could not be removed:" -ForegroundColor Yellow
    foreach ($err in $errors) {
        Write-Host "  - $err" -ForegroundColor Yellow
    }
    Write-Host ""
    Write-Host "Please RESTART Windows to complete cleanup." -ForegroundColor Cyan
    Write-Host "After restart, the System32 DLL will be removed automatically." -ForegroundColor White
} else {
    Write-Host "=== Uninstall Complete ===" -ForegroundColor Green
    Write-Host "All components removed successfully." -ForegroundColor Green
}