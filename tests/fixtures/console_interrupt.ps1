param([string]$Executable, [string]$Report, [switch]$Isolated)
$ErrorActionPreference='Stop'
if (-not $Isolated) {
  $helper=Start-Process powershell.exe -WindowStyle Hidden -PassThru -ArgumentList @('-NoProfile','-File',('"'+$PSCommandPath+'"'),'-Executable',('"'+$Executable+'"'),'-Report',('"'+$Report+'"'),'-Isolated')
  if (-not $helper.WaitForExit(35000)) { $helper.Kill(); throw 'Isolated helper timed out' }
  if ($helper.ExitCode -ne 0) { throw 'Isolated helper failed' }
  exit 0
}
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class ConsoleProbe {
  delegate bool Handler(uint code);
  static Handler guard = code => code == 0 || code == 1;
  [DllImport("kernel32.dll", SetLastError=true)] static extern bool SetConsoleCtrlHandler(Handler handler, bool add);
  [DllImport("kernel32.dll", SetLastError=true)] public static extern uint GetConsoleProcessList([Out] uint[] ids, uint size);
  [DllImport("kernel32.dll", SetLastError=true)] public static extern bool GenerateConsoleCtrlEvent(uint code, uint group);
  public static void Protect() { if(!SetConsoleCtrlHandler(guard,true)) throw new Exception("Cannot protect isolated helper"); }
}
'@
$ids=[uint32[]]::new(32)
$count=[ConsoleProbe]::GetConsoleProcessList($ids,32)
if ($count -ne 1 -or $ids[0] -ne $PID) { throw 'Helper console is not isolated; no event sent' }
[ConsoleProbe]::Protect()
$start=[Diagnostics.ProcessStartInfo]::new()
$start.FileName=$Executable
$start.UseShellExecute=$false
$start.Arguments='--shell cmd.exe -- /Q /C "ping -n 16 127.0.0.1 >nul"'
$start.EnvironmentVariables['LOCALAPPDATA']=(Split-Path $Report)
$start.EnvironmentVariables['RUST_LOG']='info'
$child=[Diagnostics.Process]::Start($start)
try {
  Start-Sleep -Seconds 4
  $ids=[uint32[]]::new(32)
  $count=[ConsoleProbe]::GetConsoleProcessList($ids,32)
  if ($count -eq 0 -or $count -gt 32) { throw 'Console membership unavailable' }
  $members=@($ids[0..($count-1)])
  if (@($members | Where-Object {$_ -ne $PID -and $_ -ne $child.Id}).Count -ne 0) { throw 'Unexpected console client; no event sent' }
  $attached=$members -contains $child.Id
  $before=$child.HasExited
  if (-not [ConsoleProbe]::GenerateConsoleCtrlEvent(1,0)) { throw 'Isolated control event failed' }
  Start-Sleep -Seconds 1
  $exited=$child.HasExited
  $result=[ordered]@{attached=$attached;exited_before_event=$before;exited_after_event=$exited;exit_code=$(if($exited){$child.ExitCode}else{$null})}
  if (-not $exited -and -not $child.WaitForExit(20000)) { throw 'Controlled window did not finish' }
  $result['final_exit_code']=$child.ExitCode
  $result | ConvertTo-Json | Set-Content -LiteralPath $Report -Encoding utf8
} finally {
  if (-not $child.HasExited) { $child.Kill(); $child.WaitForExit() }
}
