<#
.SYNOPSIS
  Acceptance criterion "no focus steal at start" (PLAN.md 2): starts LyricHub the way the
  autostart does (--minimized) and samples the foreground window every 100 ms for a few seconds.
  It passes when the foreground never moves to a LyricHub window.

.DESCRIPTION
  LyricHub must not be running (single instance: a second start would hand over and exit).
  Uses the release build by default; -Exe picks another one.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools\overlay-spike\start-focus-test.ps1
#>
param(
  [string]$Exe = (Join-Path $PSScriptRoot '..\..\src-tauri\target\release\lyrichub.exe'),
  [int]$Seconds = 6
)
$ErrorActionPreference = 'Stop'
Add-Type @"
using System; using System.Runtime.InteropServices; using System.Text;
public static class F {
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  public static string Title(IntPtr h) { var sb = new StringBuilder(256); GetWindowTextW(h, sb, 256); return sb.ToString(); }
}
"@
if (Get-Process lyrichub -ErrorAction SilentlyContinue) { throw 'LyricHub is running; quit it first (tray → Beenden).' }
$exePath = (Resolve-Path $Exe).Path
$before = [F]::GetForegroundWindow()
"foreground before: '{0}'" -f [F]::Title($before)
$p = Start-Process $exePath -ArgumentList '--minimized' -PassThru
$stolen = @()
$end = (Get-Date).AddSeconds($Seconds)
while ((Get-Date) -lt $end) {
  $fg = [F]::GetForegroundWindow()
  $fgPid = 0
  [void][F]::GetWindowThreadProcessId($fg, [ref]$fgPid)
  if ($fgPid -eq $p.Id) { $stolen += [F]::Title($fg) }
  Start-Sleep -Milliseconds 100
}
$after = [F]::GetForegroundWindow()
"foreground after:  '{0}'" -f [F]::Title($after)
if ($stolen.Count) {
  "[FAIL] LyricHub took the foreground: {0}" -f (($stolen | Select-Object -Unique) -join ', ')
  exit 1
}
"[PASS] no focus steal in $Seconds s (LyricHub keeps running, pid $($p.Id))"
