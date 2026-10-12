param(
    [switch]$Release
)

# Launches Juna elevated. This is the ONLY supported elevation path -- see
# ARCHITECTURE.md 3.8. The binary's own manifest deliberately does not request
# elevation, so casual launches stay unelevated.
#
# Children inherit the host token, so every shell Juna spawns will also be
# elevated. Verified on 2026-10-11: a child spawned from a High Mandatory Level
# parent reports High Mandatory Level.

$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot

$profile = if ($Release) { 'release' } else { 'debug' }
$exe = Join-Path $root "target\$profile\juna.exe"

if (-not (Test-Path -LiteralPath $exe)) {
    throw "Not built: $exe`nRun: cargo build $(if ($Release) { '--release' })"
}

$id = [Security.Principal.WindowsIdentity]::GetCurrent()
$alreadyElevated = [Security.Principal.WindowsPrincipal]::new($id).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)

if ($alreadyElevated) {
    Write-Host "Already elevated - launching directly." -ForegroundColor DarkGray
    & $exe
    exit $LASTEXITCODE
}

Write-Host "Requesting elevation via the 'runas' verb. Accept the UAC prompt." -ForegroundColor Yellow

# ShellExecute with the runas verb. Returns the child process handle.
$proc = Start-Process -FilePath $exe -Verb RunAs -PassThru

if (-not $proc.WaitForExit(300000)) {
    Write-Warning "Juna did not exit within 300s; abandoning wait."
    exit 1
}

# Elevated child exit codes are preserved. UAC cancellation surfaces as Win32Exception
# from Start-Process rather than an exit code, so a cancel is not masked here.
exit $proc.ExitCode