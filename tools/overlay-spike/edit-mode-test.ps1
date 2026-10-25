<#
.SYNOPSIS
  Phase 1 check of the overlay edit mode against a running LyricHub, over the fake game.

.DESCRIPTION
  Starts fake-game.ps1 (borderless fullscreen, takes the foreground), then drives LyricHub
  with synthetic input:
    1. Ctrl+Alt+E      -> overlay covers the monitor, catches clicks, game keeps the focus
    2. drag by +DragX/+DragY, Enter -> overlay saved at the new place, settings.json updated
    3. Ctrl+Alt+E, drag, Esc        -> overlay back at the saved place (cancel works)
    4. Ctrl+Alt+E, drag back, Enter -> snaps to the original place
  After every step the foreground must still be the fake game. An Esc that leaked to the game
  would close it, which the test also reports.

  Moves the mouse for a few seconds. Needs LyricHub running with the overlay on the primary
  monitor and default hotkeys; the overlay must be visible.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools\overlay-spike\edit-mode-test.ps1
#>
param(
  [int]$DragX = 400,
  [int]$DragY = 300
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Windows.Forms
Add-Type @"
using System; using System.Runtime.InteropServices; using System.Text;
public static class T {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowW(string c, string t);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtrW(IntPtr h, int i);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(POINT p);
  [DllImport("user32.dll")] public static extern IntPtr GetAncestor(IntPtr h, uint flags);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  [DllImport("user32.dll")] public static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
  public static string Title(IntPtr h) { var sb = new StringBuilder(256); GetWindowTextW(h, sb, 256); return sb.ToString(); }
}
"@
[T]::SetProcessDPIAware() | Out-Null

$WS_EX_TRANSPARENT = 0x20
$WS_EX_NOACTIVATE = 0x08000000
$settingsPath = Join-Path $env:APPDATA 'LyricHub\settings.json'
$results = New-Object System.Collections.Generic.List[object]

function Check([string]$Name, [bool]$Ok, [string]$Detail) {
  $results.Add([pscustomobject]@{ status = $(if ($Ok) { 'pass' } else { 'fail' }); name = $Name; detail = $Detail })
  Write-Host ("[{0}] {1} - {2}" -f $(if ($Ok) { 'PASS' } else { 'FAIL' }), $Name, $Detail)
}

function Overlay { [T]::FindWindowW([NullString]::Value, 'LyricHub Overlay') }
function RectOf([IntPtr]$h) { $r = New-Object T+RECT; [T]::GetWindowRect($h, [ref]$r) | Out-Null; $r }
function Fmt($r) { '{0},{1} {2}x{3}' -f $r.L, $r.T, ($r.R - $r.L), ($r.B - $r.T) }
function ExStyle([IntPtr]$h) { [int64][T]::GetWindowLongPtrW($h, -20) }
function Foreground { [T]::Title([T]::GetForegroundWindow()) }
function HitAt([int]$X, [int]$Y) { $p = New-Object T+POINT; $p.X = $X; $p.Y = $Y; [T]::Title([T]::GetAncestor([T]::WindowFromPoint($p), 2)) }

function Key([byte[]]$Vks) {
  foreach ($vk in $Vks) { [T]::keybd_event($vk, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 20 }
  [array]::Reverse($Vks)
  foreach ($vk in $Vks) { [T]::keybd_event($vk, 0, 2, [UIntPtr]::Zero); Start-Sleep -Milliseconds 20 }
}
function CtrlAltE { Key ([byte[]](0x11, 0x12, 0x45)) }
function Enter { Key ([byte[]](0x0D)) }
function Esc { Key ([byte[]](0x1B)) }

function Drag([int]$X, [int]$Y, [int]$Dx, [int]$Dy) {
  [T]::SetCursorPos($X, $Y) | Out-Null; Start-Sleep -Milliseconds 120
  [T]::mouse_event(0x0002, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 120   # left down
  for ($i = 1; $i -le 20; $i++) {
    [T]::SetCursorPos([int]($X + $Dx * $i / 20), [int]($Y + $Dy * $i / 20)) | Out-Null
    Start-Sleep -Milliseconds 25
  }
  Start-Sleep -Milliseconds 120
  [T]::mouse_event(0x0004, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 300    # left up
}

function Near($a, $b, [int]$Tol) {
  [Math]::Abs($a.L - $b.L) -le $Tol -and [Math]::Abs($a.T - $b.T) -le $Tol -and
  [Math]::Abs(($a.R - $a.L) - ($b.R - $b.L)) -le $Tol -and [Math]::Abs(($a.B - $a.T) - ($b.B - $b.T)) -le $Tol
}

$overlay = Overlay
if ($overlay -eq [IntPtr]::Zero) { throw 'LyricHub overlay window not found. Is LyricHub running?' }
$cursor = New-Object T+POINT; [T]::GetCursorPos([ref]$cursor) | Out-Null

$script = '"{0}"' -f (Join-Path $PSScriptRoot 'fake-game.ps1')
$game = Start-Process powershell -PassThru -ArgumentList @('-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $script, '-ReassertMs', 0, '-Seconds', 60)
Start-Sleep -Seconds 3   # game shows, overlay watchdog re-asserts topmost within 2 s
$fg0 = Foreground
$conclusive = $fg0 -eq 'LyricHub Fake Game'
Write-Host "Foreground at start: '$fg0' (focus checks conclusive: $conclusive)"
function FocusKept([string]$Step) {
  $fg = Foreground
  if ($fg -eq 'LyricHub Overlay') { Check "$Step - focus" $false "overlay took the foreground" }
  elseif ($conclusive) { Check "$Step - focus" ($fg -eq $fg0 -and -not $game.HasExited) "foreground '$fg', game running: $(-not $game.HasExited)" }
  else { Write-Host "[INFO] $Step - foreground '$fg' (not conclusive)" }
}

try {
  $r0 = RectOf $overlay
  $primary = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
  Write-Host "Overlay at start: $(Fmt $r0)"

  # 1. Enter edit mode
  CtrlAltE; Start-Sleep -Milliseconds 900
  $r = RectOf $overlay; $ex = ExStyle $overlay
  Check 'edit: overlay covers the monitor' ($r.L -eq $primary.X -and $r.T -eq $primary.Y -and ($r.R - $r.L) -eq $primary.Width -and ($r.B - $r.T) -eq $primary.Height) (Fmt $r)
  Check 'edit: catches clicks, stays no-activate' ((($ex -band $WS_EX_TRANSPARENT) -eq 0) -and (($ex -band $WS_EX_NOACTIVATE) -ne 0)) ('0x{0:X8}' -f $ex)
  FocusKept 'edit start'
  $cx = [int](($r0.L + $r0.R) / 2); $cy = [int](($r0.T + $r0.B) / 2)
  $hit = HitAt $cx $cy
  Check 'edit: mouse hits the overlay' ($hit -eq 'LyricHub Overlay') "window at ($cx,$cy): '$hit'"

  # 2. Drag and save with Enter
  Drag $cx $cy $DragX $DragY
  FocusKept 'drag'
  Enter; Start-Sleep -Milliseconds 900
  $r1 = RectOf $overlay; $ex = ExStyle $overlay
  $expected = [pscustomobject]@{ L = $r0.L + $DragX; T = $r0.T + $DragY; R = $r0.R + $DragX; B = $r0.B + $DragY }
  Check 'save: overlay at the dragged place' (Near $r1 $expected 3) "$(Fmt $r1), expected $(Fmt $expected)"
  Check 'save: click-through again' (($ex -band $WS_EX_TRANSPARENT) -ne 0) ('0x{0:X8}' -f $ex)
  FocusKept 'save'
  $saved = Get-Content $settingsPath -Raw | ConvertFrom-Json
  Check 'save: settings.json updated' ($saved.overlay.offset_y_pct -gt 10) ("anchor {0}, x {1} %, y {2} %, width {3} %, scale {4}" -f $saved.overlay.anchor, $saved.overlay.offset_x_pct, $saved.overlay.offset_y_pct, $saved.overlay.width_pct, $saved.overlay.scale)

  # 3. Drag and cancel with Esc
  CtrlAltE; Start-Sleep -Milliseconds 900
  Drag ([int](($r1.L + $r1.R) / 2)) ([int](($r1.T + $r1.B) / 2)) 250 120
  Esc; Start-Sleep -Milliseconds 900
  $r2 = RectOf $overlay
  Check 'cancel: overlay back at the saved place' (Near $r2 $r1 1) "$(Fmt $r2), expected $(Fmt $r1)"
  FocusKept 'cancel'

  # 4. Drag back; the snap points pull it onto the original place
  CtrlAltE; Start-Sleep -Milliseconds 900
  Drag ([int](($r1.L + $r1.R) / 2)) ([int](($r1.T + $r1.B) / 2)) (-$DragX) (-$DragY)
  Enter; Start-Sleep -Milliseconds 900
  $r3 = RectOf $overlay
  Check 'restore: overlay back at the start' (Near $r3 $r0 2) "$(Fmt $r3), expected $(Fmt $r0)"
  FocusKept 'restore'
} finally {
  [T]::SetCursorPos($cursor.X, $cursor.Y) | Out-Null
  if (-not $game.HasExited) { $game.Kill() }
}

$failed = @($results | Where-Object status -eq 'fail').Count
Write-Host ("{0} passed, {1} failed" -f ($results.Count - $failed), $failed)
exit $(if ($failed -eq 0) { 0 } else { 1 })
