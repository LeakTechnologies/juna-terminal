$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)

$toolchain = 'stable-x86_64-pc-windows-msvc'
$probeDir = Join-Path $env:TEMP 'juna-toolchain-probe'
New-Item -ItemType Directory -Force -Path $probeDir | Out-Null
$probeSrc = Join-Path $probeDir 'probe.rs'
$probeExe = Join-Path $probeDir 'probe.exe'
Set-Content -LiteralPath $probeSrc -Value 'fn main() { println!("link ok"); }'

$failed = $false

function Step($name, $block) {
    Write-Host ""
    Write-Host "=== $name ===" -ForegroundColor Cyan
    $ok = $false
    try {
        $ok = & $block
    } catch {
        Write-Host "  FAILED: $($_.Exception.Message)" -ForegroundColor Red
    }
    if ($ok) { Write-Host "  PASS" -ForegroundColor Green }
    else { Write-Host "  FAIL" -ForegroundColor Red; $script:failed = $true }
    Write-Host ""
}

Step '1. VS C++ workload present' {
    $vswhere = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path $vswhere)) { throw "vswhere not found" }
    $path = & $vswhere -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if (-not $path) { throw "VC.Tools.x86.x64 component is still absent" }
    Write-Host "  installationPath: $path"
    $true
}

Step '2. Windows SDK present' {
    $sdk = "${env:ProgramFiles(x86)}\Windows Kits\10\Include"
    if (-not (Test-Path $sdk)) { throw "Windows Kits\10\Include not found" }
    Get-ChildItem $sdk -Directory | ForEach-Object { Write-Host "  SDK version: $($_.Name)" }
    $true
}

Step "3. rustc MSVC linkage probe" {
    rustc "+$toolchain" $probeSrc -o $probeExe
    if ($LASTEXITCODE -ne 0) { throw "rustc failed to link" }
    $out = & $probeExe
    if ($out -ne 'link ok') { throw "probe output was '$out'" }
    Write-Host "  probe output: $out"
    $true
}

Step '4. cargo test under MSVC' {
    $env:RUSTUP_TOOLCHAIN = $toolchain
    cargo test 2>&1 | ForEach-Object { Write-Host "  $_" }
    $code = $LASTEXITCODE
    Remove-Item Env:\RUSTUP_TOOLCHAIN
    if ($code -ne 0) { throw "cargo test exited $code" }
    $true
}

Write-Host ""
if ($failed) {
    Write-Host "TOOLCHAIN NOT READY - do not create rust-toolchain.toml" -ForegroundColor Red
    exit 1
}
Write-Host "TOOLCHAIN READY - rust-toolchain.toml is now justified" -ForegroundColor Green
exit 0