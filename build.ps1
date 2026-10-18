[CmdletBinding()]
param(
    [switch]$SkipTests,
    [switch]$SkipUiAudit,
    [switch]$DebugBuild,
    [switch]$CleanDebug
)

$ErrorActionPreference = 'Continue'
Set-StrictMode -Version Latest
$root = $PSScriptRoot

$NodeHome = if ($env:NODE_HOME) { $env:NODE_HOME } else { 'E:\node' }
$NodeExe = Join-Path $NodeHome 'node.exe'
Add-Type -AssemblyName System.IO.Compression.FileSystem
function New-PortableZip {
    param([string]$SourceDir, [string]$Destination)
    if (Test-Path -LiteralPath $Destination) { Remove-Item -LiteralPath $Destination -Force }
    # Compress-Archive rejects the 1973 timestamp shipped in WebView2Loader.dll
    [System.IO.Compression.ZipFile]::CreateFromDirectory(
        $SourceDir, $Destination, [System.IO.Compression.CompressionLevel]::Optimal, $false)
    if (-not (Test-Path -LiteralPath $Destination)) { throw "Portable ZIP was not produced: $Destination" }
    Write-Host "portable: $Destination" -ForegroundColor Green
}

$profile = if ($DebugBuild) { 'debug' } else { 'release' }

function Write-Step([string]$message) {
    Write-Host ""
    Write-Host "==> $message" -ForegroundColor Cyan
}

function Get-CargoPath {
    $candidate = Join-Path $env:USERPROFILE '.cargo\bin\cargo.exe'
    if (Test-Path $candidate) { return $candidate }
    $command = Get-Command cargo -ErrorAction SilentlyContinue
    if ($command) { return $command.Source }
    throw 'Rust toolchain not found. Install Rust from https://rustup.rs and retry.'
}

$cargo = Get-CargoPath
$env:Path = (Split-Path $cargo -Parent) + ';' + $env:Path

Write-Step 'cargo fmt check'
& $cargo fmt --all -- --check 2>&1 | Out-Host
if ($LASTEXITCODE -ne 0) { throw 'cargo fmt reported formatting differences.' }

Write-Step 'cargo clippy'
& $cargo clippy --all-targets -- -D warnings 2>&1 | Out-Host
if ($LASTEXITCODE -ne 0) { throw 'cargo clippy reported findings.' }

if (-not $SkipTests) {
    Write-Step 'cargo test'
    & $cargo test --all-targets 2>&1 | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'cargo test failed.' }
}

Write-Step "cargo build ($profile)"
if ($DebugBuild) { & $cargo build 2>&1 | Out-Host } else { & $cargo build --release 2>&1 | Out-Host }
if ($LASTEXITCODE -ne 0) { throw 'cargo build failed.' }

$target = Join-Path $root "target\$profile"
$loader = Get-ChildItem -Path (Join-Path $root 'target') -Recurse -Filter 'WebView2Loader.dll' -ErrorAction SilentlyContinue |
    Where-Object { $_.FullName -match "[\\/]x64[\\/]" } |
    Select-Object -First 1
if (-not $loader) { throw 'WebView2Loader.dll was not produced by the webview2-com-sys build script.' }

$dist = Join-Path $root 'dist'
if (Test-Path $dist) { Remove-Item -LiteralPath $dist -Recurse -Force }
$package = Join-Path $dist 'MinecraftServerDownloader-win-x64'
New-Item -ItemType Directory -Path $package -Force | Out-Null

Copy-Item -LiteralPath (Join-Path $target 'MinecraftServerDownloader.exe') -Destination $package -ErrorAction Stop
Copy-Item -LiteralPath (Join-Path $target 'uninstall.exe') -Destination $package -ErrorAction Stop
Copy-Item -LiteralPath $loader.FullName -Destination $package -ErrorAction Stop
if (Test-Path (Join-Path $root 'app.ico')) { Copy-Item -LiteralPath (Join-Path $root 'app.ico') -Destination $package -ErrorAction Stop }
Copy-Item -LiteralPath (Join-Path $root 'README.md') -Destination $package -ErrorAction Stop
Copy-Item -LiteralPath (Join-Path $root 'LICENSE') -Destination $package -ErrorAction Stop
Copy-Item -LiteralPath (Join-Path $root 'Ui\Web') -Destination (Join-Path $package 'Ui\Web') -Recurse -ErrorAction Stop

Write-Step 'Inno Setup'
$iscc = 'C:\Program Files (x86)\Inno Setup 7\ISCC.exe'
if (-not (Test-Path -LiteralPath $iscc)) { $iscc = 'C:\Program Files (x86)\Inno Setup 6\ISCC.exe' }
if (-not (Test-Path -LiteralPath $iscc)) {
    Write-Warning 'Inno Setup was not found; skipping the installer step.'
} else {
    & $iscc --quiet (Join-Path $root 'installer\MinecraftServerDownloader.iss')
    if ($LASTEXITCODE -ne 0) { throw 'Inno Setup reported a compilation error.' }
    Write-Host ("installer: " + (Join-Path $root 'dist\MinecraftServerDownloader-Setup.exe')) -ForegroundColor Green
}

if (-not $SkipUiAudit) {
    $audit = Join-Path $root 'tools\ui-audit\run.js'
    if (Test-Path -LiteralPath $audit) {
        $playwright = Join-Path $NodeHome 'node_modules\playwright-core\package.json'
        if (-not (Test-Path -LiteralPath $playwright)) {
            Write-Warning 'playwright-core is not installed; skipping the UI audit.'
        } else {
            Write-Step 'UI audit'
            & $NodeExe $audit
            if ($LASTEXITCODE -ne 0) { throw 'UI audit reported violations.' }
        }
    }
}

Write-Step 'WiX MSI'
$wix = Join-Path $env:USERPROFILE '.dotnet\tools\wix.exe'
if (-not (Test-Path -LiteralPath $wix)) {
    Write-Warning 'The WiX tool was not found; skipping the MSI step.'
} else {
    $msi = Join-Path $root 'dist\MinecraftServerDownloader.msi'
    & $wix build (Join-Path $root 'installer\Package.wxs') -ext WixToolset.UI.wixext -arch x64 -pdbtype none -d ProductVersion=0.0.1 -d ("AppSource=" + (Join-Path $root 'dist\MinecraftServerDownloader-win-x64')) -o $msi
    if ($LASTEXITCODE -ne 0) { throw 'WiX reported a compilation error.' }
    if (-not (Test-Path -LiteralPath $msi)) { throw "MSI was not produced: $msi" }
    Write-Host "installer: $msi" -ForegroundColor Green
}

Write-Step 'Portable ZIP'
New-PortableZip -SourceDir $package -Destination (Join-Path $dist 'MinecraftServerDownloader-portable-win-x64.zip')

Write-Step 'package summary'
Get-ChildItem -LiteralPath $package -Recurse -File | Measure-Object -Property Length -Sum |
    ForEach-Object { Write-Host ("files={0} bytes={1}" -f $_.Count, $_.Sum) }
Write-Host "package: $package" -ForegroundColor Green
if ($CleanDebug) {
    $debugDir = Join-Path $PSScriptRoot 'target\debug'
    if (Test-Path -LiteralPath $debugDir) {
        Remove-Item -LiteralPath $debugDir -Recurse -Force
        Write-Host "cleaned: $debugDir" -ForegroundColor Green
    }
}