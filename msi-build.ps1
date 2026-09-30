# MSI build script

param(
    [string]$Version = ""
)

$ErrorActionPreference = "Stop"

# Add WiX v3.14 to PATH
$env:PATH += ";C:\Program Files (x86)\WiX Toolset v3.14\bin"

# Auto-detect version from Cargo.toml
if ($Version -eq "") {
    $cargoTomlContent = Get-Content "Cargo.toml" -Raw
    if ($cargoTomlContent -match 'version\s*=\s*"([^"]+)"') {
        $Version = $matches[1]
    } else {
        $Version = "0.1.0"
    }
}

Write-Host "Building XimeYao (曦码·曜) MSI v$Version..." -ForegroundColor Cyan

# 1. Build release
Write-Host "Step 1: Building release..." -ForegroundColor Yellow
cargo build --release --quiet
if ($LASTEXITCODE -ne 0) {
    Write-Host "Build failed!" -ForegroundColor Red
    exit 1
}

# 1.5. Copy rime.dll from libximecore git dep to target\release
Write-Host "Step 1.5: Copying rime.dll..." -ForegroundColor Yellow
$rimeDll = Join-Path $librimeRoot "dist\lib\rime.dll"
if (Test-Path $rimeDll) {
    Copy-Item $rimeDll "target\release\rime.dll" -Force
    Write-Host "  rime.dll copied"
} else {
    Write-Warning "rime.dll not found at $rimeDll"
}

# 2. data/ 不再分发 librime 自带 minimal 示例（cangjie5 / luna_pinyin /
#    essay.txt 等会混入用户 rime 目录）；rime-wubi（user-data/）自包含。
if (Test-Path "target\release\data") {
    Remove-Item "target\release\data" -Recurse -Force
}
New-Item "target\release\data" -ItemType Directory -Force | Out-Null

# 2.5. Copy user schema files (to user-data/, deployed to %APPDATA% on first run)
Write-Host "Step 2.5: Copying user schema files..." -ForegroundColor Yellow
if (Test-Path "target\release\user-data") {
    Remove-Item "target\release\user-data" -Recurse -Force
}

$files = Get-ChildItem -Path "rime-wubi" -Recurse -File | Where-Object {
    $dir = $_.DirectoryName
    $name = $_.Name
    -not ($dir -like "*\.git*") -and
    -not ($dir -like "*\.github*") -and
    -not ($dir -like "*imgs*") -and
    -not ($name -like "*.md") -and
    -not ($name -like ".gitignore") -and
    -not ($name -like "macOS-*") -and
    -not ($name -like "*.command") -and
    -not ($name -like "LICENSE") -and
    -not ($name -like "squirrel.custom.yaml") -and
    -not ($name -like "trime.custom.yaml")
}

foreach ($file in $files) {
    $relativePath = $file.FullName.Substring($PWD.Path.Length + "rime-wubi".Length + 2)
    $destPath = "target\release\user-data\$relativePath"
    $destDir = Split-Path -Parent $destPath
    if (-not (Test-Path $destDir)) {
        New-Item $destDir -ItemType Directory -Force | Out-Null
    }
    Copy-Item $file.FullName $destPath -Force
}

# 3. Copy resources
Write-Host "Step 3: Copying resources..." -ForegroundColor Yellow
if (Test-Path "target\release\resources") {
    Remove-Item "target\release\resources" -Recurse -Force
}
Copy-Item "resources" "target\release\resources" -Recurse

# 4. Harvest data, user-data, and resources
Write-Host "Step 4: Harvesting data and resources..." -ForegroundColor Yellow
heat dir "target\release\data" -o "crates\winxime-server\wix\data.wxs" -dr DataFolder -cg DataFiles -var var.DataDir -sreg -srd -ag
heat dir "target\release\user-data" -o "crates\winxime-server\wix\userdata.wxs" -dr UserDataFolder -cg UserDataFiles -var var.UserDataDir -sreg -srd -ag
heat dir "target\release\resources" -o "crates\winxime-server\wix\resources.wxs" -dr ResourcesFolder -cg ResourcesFiles -var var.ResourcesDir -sreg -srd -ag

# 5. Compile with candle
Write-Host "Step 5: Compiling WiX sources..." -ForegroundColor Yellow
if (-not (Test-Path "target\wix")) {
    New-Item "target\wix" -ItemType Directory -Force | Out-Null
}

candle -arch x64 "crates\winxime-server\wix\main.wxs" "crates\winxime-server\wix\data.wxs" "crates\winxime-server\wix\userdata.wxs" "crates\winxime-server\wix\resources.wxs" `
    -ext WixUIExtension -ext WixUtilExtension `
    "-dCargoTargetBinDir=target\release" `
    "-dDataDir=target\release\data" `
    "-dUserDataDir=target\release\user-data" `
    "-dResourcesDir=target\release\resources" `
    "-dVersion=$Version" `
    -out "target\wix\"

if ($LASTEXITCODE -ne 0) {
    Write-Host "Candle failed!" -ForegroundColor Red
    exit 1
}

# 6. Link with light
Write-Host "Step 6: Linking MSI..." -ForegroundColor Yellow
light "target\wix\main.wixobj" "target\wix\data.wixobj" "target\wix\userdata.wixobj" "target\wix\resources.wixobj" `
    -ext WixUIExtension -ext WixUtilExtension `
    -cultures:zh-CN `
    -loc "crates\winxime-server\wix\zh-cn.wxl" `
    -out "target\wix\ximeyao-$Version-x86_64.msi"

if ($LASTEXITCODE -ne 0) {
    Write-Host "Light failed!" -ForegroundColor Red
    exit 1
}

# 7. Check result
$msiPath = "target\wix\ximeyao-$Version-x86_64.msi"
if (Test-Path $msiPath) {
    $msi = Get-Item $msiPath
    Write-Host ""
    Write-Host "Success: $($msi.FullName)" -ForegroundColor Green
    Write-Host "Size: $([math]::Round($msi.Length / 1MB, 2)) MB" -ForegroundColor White
    Write-Host ""
    Write-Host "Install: msiexec /i $($msi.FullName)" -ForegroundColor Yellow
} else {
    Write-Host "MSI build failed!" -ForegroundColor Red
    exit 1
}