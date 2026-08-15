param(
    [string]$Version = "",
    [switch]$Sign,
    [switch]$Register,
    [switch]$InstallUnsigned
)

$ErrorActionPreference = "Continue"

# Auto-detect version from Cargo.toml
if ($Version -eq "") {
    $cargoTomlContent = Get-Content "Cargo.toml" -Raw
    if ($cargoTomlContent -match 'version\s*=\s*"([^"]+)"') {
        $Version = $matches[1]
    } else {
        $Version = "0.1.0"
    }
}

$parts = $Version.Split('.')
$msixVersion = "{0}.{1}.{2}.0" -f $parts[0], $parts[1], $parts[2]

Write-Host "Building Xime MSIX v$msixVersion..." -ForegroundColor Cyan

$packageDir = "target\msix-pkg"
if (Test-Path $packageDir) { Remove-Item $packageDir -Recurse -Force }

New-Item "$packageDir\assets" -ItemType Directory -Force | Out-Null
New-Item "$packageDir\data" -ItemType Directory -Force | Out-Null
New-Item "$packageDir\resources" -ItemType Directory -Force | Out-Null

# 1. Copy binaries
Write-Host "Step 1: Copying binaries..." -ForegroundColor Yellow
Copy-Item "target\release\winxime-server.exe" $packageDir
Copy-Item "target\release\winxime_tsf.dll" $packageDir
Copy-Item "target\release\winxime-setup.exe" $packageDir
Copy-Item "target\release\winxime-tsf-register.exe" $packageDir
$rimeDll = "target\release\rime.dll"
if (Test-Path $rimeDll) {
    Copy-Item $rimeDll $packageDir
} else {
    Write-Warning "rime.dll not found at target\release, copying from libximecore..."
    $json = (& cargo metadata --format-version 1 2>$null) | ConvertFrom-Json
    $pkg = $json.packages | Where-Object { $_.name -eq 'librime-sys2' }
    if (-not $pkg) { Write-Error "librime-sys2 not found"; exit 1 }
    $manifestPath = $pkg.manifest_path
    $libximecoreRoot = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $manifestPath))
    $srcDll = Join-Path $libximecoreRoot "librime\dist\lib\rime.dll"
    if (-not (Test-Path $srcDll)) { Write-Error "rime.dll not found at $srcDll"; exit 1 }
    Copy-Item $srcDll $rimeDll -Force
    Copy-Item $rimeDll $packageDir
}

# 2. Copy rime base data (to data/)
Write-Host "Step 2: Copying rime base data..." -ForegroundColor Yellow
$json = (& cargo metadata --format-version 1 2>$null) | ConvertFrom-Json
$pkg = $json.packages | Where-Object { $_.name -eq 'librime-sys2' }
$manifestPath = $pkg.manifest_path
$libximecoreRoot = Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $manifestPath))
$rimeData = Join-Path $libximecoreRoot "librime\data\minimal"
if (Test-Path $rimeData) {
    Copy-Item "$rimeData\*" "$packageDir\data" -Recurse -Force
} else {
    Write-Warning "rime data not found at $rimeData, skipping"
}

# 3. Copy rime-wubi data (to user-data/, deployed to %APPDATA% on first run)
Write-Host "Step 3: Copying rime-wubi data..." -ForegroundColor Yellow
New-Item "$packageDir\user-data" -ItemType Directory -Force | Out-Null
$rimeWubiFiles = Get-ChildItem -Path "rime-wubi" -Recurse -File | Where-Object {
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
foreach ($file in $rimeWubiFiles) {
    $relativePath = $file.FullName.Substring($PWD.Path.Length + "rime-wubi".Length + 2)
    $destPath = "$packageDir\user-data\$relativePath"
    $destDir = Split-Path -Parent $destPath
    if (-not (Test-Path $destDir)) { New-Item $destDir -ItemType Directory -Force | Out-Null }
    Copy-Item $file.FullName $destPath -Force
}

# 4. Copy resources
Write-Host "Step 4: Copying resources..." -ForegroundColor Yellow
Copy-Item "resources\*" "$packageDir\resources" -Recurse

# 5. Copy MSIX assets (logos, manifest)
Write-Host "Step 5: Copying MSIX assets..." -ForegroundColor Yellow
Copy-Item "crates\winxime-server\msix\assets\*" "$packageDir\assets"
$manifest = Get-Content "crates\winxime-server\msix\AppxManifest.xml" -Raw -Encoding UTF8
$manifest = $manifest.Replace('{{VERSION}}', $msixVersion)
$utf8Bom = New-Object System.Text.UTF8Encoding($true)
[System.IO.File]::WriteAllText("$packageDir\AppxManifest.xml", $manifest, $utf8Bom)

# Find MakeAppx.exe and SignTool.exe
$kitRoot = "${env:ProgramFiles(x86)}\Windows Kits\10\bin"
$makeAppx = Get-ChildItem "$kitRoot\*\x64\MakeAppx.exe" | Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName
if (-not $makeAppx) { Write-Error "MakeAppx.exe not found"; exit 1 }
$signTool = Get-ChildItem "$kitRoot\*\x64\SignTool.exe" | Sort-Object FullName -Descending | Select-Object -First 1 -ExpandProperty FullName

if ($Register) {
    Write-Host "Registering for development..." -ForegroundColor Yellow
    Add-AppxPackage -Register "$packageDir\AppxManifest.xml" -Verbose
    Remove-Item $packageDir -Recurse -Force
    Write-Host "Development registration complete!" -ForegroundColor Green
    return
}

# Create MSIX
Write-Host "Step 6: Creating MSIX package..." -ForegroundColor Yellow
if (-not (Test-Path "target\wix")) { New-Item "target\wix" -ItemType Directory -Force | Out-Null }
$msixPath = "target\wix\xime-$Version-x86_64.msix"
& $makeAppx pack /d $packageDir /p $msixPath /l
if ($LASTEXITCODE -ne 0) { Write-Error "MakeAppx failed"; exit 1 }

# Install unsigned (for testing without signing)
if ($InstallUnsigned) {
    Write-Host "Installing unsigned MSIX (AllowUnsigned)..." -ForegroundColor Yellow
    Add-AppxPackage -AllowUnsigned -Path $msixPath -Verbose
    Remove-Item $packageDir -Recurse -Force
    Write-Host "Installation complete!" -ForegroundColor Green
    return
}

# Sign (optional)
if ($Sign) {
    Write-Host "Step 7: Signing MSIX..." -ForegroundColor Yellow
    $cert = Get-ChildItem "Cert:\CurrentUser\My" | Where-Object { $_.Subject -eq "CN=XimeOrg" } | Select-Object -First 1
    if (-not $cert) {
        $cert = New-SelfSignedCertificate -Type Custom -Subject "CN=XimeOrg" -KeyUsage DigitalSignature -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3") -CertStoreLocation "Cert:\CurrentUser\My" -NotAfter (Get-Date).AddYears(3)
        Write-Host "  Created new self-signed certificate" -ForegroundColor Yellow
    }

    $inRoot = Get-ChildItem "Cert:\CurrentUser\Root" | Where-Object { $_.Subject -eq "CN=XimeOrg" } | Select-Object -First 1
    if (-not $inRoot) {
        $certPath = Join-Path $env:TEMP "XimeOrg.cer"
        Export-Certificate -Cert $cert -FilePath $certPath -Type CERT | Out-Null
        Import-Certificate -FilePath $certPath -CertStoreLocation "Cert:\CurrentUser\Root" | Out-Null
        Remove-Item $certPath -Force
        Write-Host "  Certificate installed to Trusted Root" -ForegroundColor Yellow
    }

    if ($signTool) {
        & $signTool sign /fd SHA256 /a /s My /n "XimeOrg" $msixPath
        Write-Host "Signed with self-signed certificate" -ForegroundColor Green
    } else {
        Write-Warning "SignTool not found, skipping signing"
    }
}

# Result
Write-Host ""
Write-Host "Success: $((Get-Item $msixPath).FullName)" -ForegroundColor Green
Write-Host "Size: $([math]::Round((Get-Item $msixPath).Length / 1MB, 2)) MB" -ForegroundColor White

Remove-Item $packageDir -Recurse -Force
