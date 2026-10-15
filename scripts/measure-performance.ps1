param(
    [Parameter(Mandatory = $true)][string]$Executable,
    [ValidateRange(1, 10)][int]$Runs = 3,
    [ValidateRange(3, 60)][int]$IdleSeconds = 10,
    [ValidateSet('Idle', 'Flood')][string]$Workload = 'Idle',
    [ValidateSet('Juna', 'Alacritty')][string]$Terminal = 'Juna',
    [ValidateSet('IBM Plex Mono', 'Consolas')][string]$FontFamily = 'Consolas',
    [string]$Output = 'target/acceptance/performance/baseline.json'
)
$ErrorActionPreference = 'Stop'
$exe = (Resolve-Path -LiteralPath $Executable).Path
$project = Split-Path -Parent $PSScriptRoot
$root = Join-Path $project ('target/acceptance/performance/' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root -Force | Out-Null
$shell = Join-Path $env:WINDIR 'System32/WindowsPowerShell/v1.0/powershell.exe'
$rows = @()
if ($Workload -eq 'Flood') {
    $flood = Join-Path $root 'output.ps1'
    @'
$line = [Text.Encoding]::ASCII.GetBytes(('x' * 78) + "`r`n")
$chunk = New-Object byte[] (80 * 800)
for ($i = 0; $i -lt 800; $i++) { [Array]::Copy($line, 0, $chunk, $i * 80, 80) }
$stream = [Console]::OpenStandardOutput()
for ($i = 0; $i -lt 64; $i++) { $stream.Write($chunk, 0, $chunk.Length) }
$stream.Flush()
exit 0
'@ | Set-Content -LiteralPath $flood -Encoding ASCII
}
for ($run = 0; $run -lt $Runs; $run++) {
    $cache = Join-Path $root "run-$run"
    $settingsFolder = Join-Path $cache 'Juna'
    New-Item -ItemType Directory -Path $settingsFolder -Force | Out-Null
    $settings = @{
        schema_version = 1; active_theme_id = 'catppuccin-mocha'
        font_family = $FontFamily; font_size = 16; minimum_contrast = 4.5
        show_line_numbers = $false; show_git_status = $false
        report_shell_directory = $false; favourite_theme_ids = @()
    } | ConvertTo-Json
    [System.IO.File]::WriteAllText((Join-Path $settingsFolder 'settings.json'), $settings,
        (New-Object System.Text.UTF8Encoding($false)))
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $exe
    $info.Arguments = '--shell "' + $shell + '" -- -NoLogo -NoProfile'
    if ($Workload -eq 'Flood') {
        $info.Arguments += ' -ExecutionPolicy Bypass -File "' + $flood + '"'
    }
    if ($Terminal -eq 'Alacritty') {
        $config = Join-Path $cache 'alacritty.toml'
        $toml = @"
[general]
live_config_reload = false
[window]
dimensions = { columns = 100, lines = 30 }
[font]
normal = { family = "$FontFamily" }
size = 12.0
[cursor]
style = { shape = "Block", blinking = "Never" }
"@
        [System.IO.File]::WriteAllText($config, $toml, (New-Object System.Text.UTF8Encoding($false)))
        $info.Arguments = '--config-file "' + $config + '" --command "' + $shell + '" -NoLogo -NoProfile'
        if ($Workload -eq 'Flood') {
            $info.Arguments += ' -ExecutionPolicy Bypass -File "' + $flood + '"'
        }
    }
    $info.UseShellExecute = $false
    $info.WorkingDirectory = $root
    $info.EnvironmentVariables['LOCALAPPDATA'] = $cache
    $info.EnvironmentVariables['USERPROFILE'] = $root
    $process = New-Object System.Diagnostics.Process
    $process.StartInfo = $info
    $timer = [System.Diagnostics.Stopwatch]::StartNew()
    try {
        if (-not $process.Start()) { throw 'Cannot start measurement fixture' }
        do {
            if ($process.HasExited) { throw 'Measurement fixture exited before creating a window' }
            $process.Refresh()
            if ($process.MainWindowHandle -ne [IntPtr]::Zero) { break }
            if ($timer.Elapsed.TotalSeconds -gt 20) { throw 'Window creation measurement timed out' }
            Start-Sleep -Milliseconds 10
        } while ($true)
        $windowMs = $timer.Elapsed.TotalMilliseconds
        if ($Workload -eq 'Flood') {
            $peak = 0L
            while (-not $process.WaitForExit(20)) {
                $process.Refresh()
                if ($process.HasExited) { break }
                try { $peak = [math]::Max($peak, $process.PeakWorkingSet64) } catch {
                    if (-not $process.HasExited) { throw }
                }
                if ($timer.Elapsed.TotalSeconds -gt 60) { throw 'Output fixture timed out' }
            }
            if ($process.ExitCode -ne 0) { throw 'Output fixture failed' }
            $rows += [ordered]@{
                run = $run + 1
                window_handle_ms = [math]::Round($windowMs, 2)
                output_bytes = 4096000
                launch_to_exit_ms = [math]::Round($timer.Elapsed.TotalMilliseconds, 2)
                cpu_ms = [math]::Round($process.TotalProcessorTime.TotalMilliseconds, 2)
                observed_peak_working_set_bytes = $peak
            }
            continue
        }
        Start-Sleep -Seconds 3
        $process.Refresh()
        $cpu = $process.TotalProcessorTime.TotalMilliseconds
        $idle = [System.Diagnostics.Stopwatch]::StartNew()
        Start-Sleep -Seconds $IdleSeconds
        $process.Refresh()
        $elapsed = $idle.Elapsed.TotalMilliseconds
        $rows += [ordered]@{
            run = $run + 1
            window_handle_ms = [math]::Round($windowMs, 2)
            idle_cpu_ms = [math]::Round($process.TotalProcessorTime.TotalMilliseconds - $cpu, 2)
            idle_elapsed_ms = [math]::Round($elapsed, 2)
            private_bytes = $process.PrivateMemorySize64
            working_set_bytes = $process.WorkingSet64
            peak_working_set_bytes = $process.PeakWorkingSet64
        }
    } finally {
        if (-not $process.HasExited) {
            $null = $process.CloseMainWindow()
            if (-not $process.WaitForExit(15000)) {
                # Only the process created by this fixture may be terminated.
                $process.Kill()
                $process.WaitForExit()
                throw 'Fixture failed normal window shutdown'
            }
        }
        $process.Dispose()
    }
}
$destination = [System.IO.Path]::GetFullPath((Join-Path $project $Output))
New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
[ordered]@{
    schema_version = 1
    executable_sha256 = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
    os_version = [Environment]::OSVersion.Version.ToString()
    logical_processors = [Environment]::ProcessorCount
    workload = $Workload
    terminal = $Terminal
    font_family = $FontFamily
    conditions = 'Release fixture; fresh isolated config/cache per run; font 16px (12pt in Alacritty); Git footer off; PowerShell without profile. Viewport geometry requires separate verification. Window handle is a startup proxy, not first presentation or input latency. Parent process memory/CPU only; no GPU or child-shell accounting. First run may warm OS caches. Flood duration includes startup and shutdown, not pure parser throughput.'
    samples = $rows
} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath $destination -Encoding UTF8
$rows | Format-Table
Write-Output "Measurements written to $destination"
