<#
.SYNOPSIS
  Proves that a nudge belongs to its song (PLAN.md 5.7, user decision 14.1).

.DESCRIPTION
  1. Ctrl+Alt+. twice on the current song (song offset +200 ms)
  2. next song through SMTC: its offset must be its own (0 if never corrected)
  3. back to the first song: its +200 ms must apply again
  4. Ctrl+Alt+, twice: back to 0
  Reads LyricHub's log lines "song offset nudged" and "song offset applied". Needs LyricHub and
  Spotify running and playing; skips one song forward and back and sends the nudge hotkeys
  (LyricHub's global hotkeys take them, no other window sees them).

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools\sync\nudge-test.ps1
#>
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Runtime.WindowsRuntime
Add-Type @"
using System; using System.Runtime.InteropServices;
public static class K { [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra); }
"@
$asTaskOp = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
    $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and
    $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1' })[0]
function Await($op, [Type]$t) {
  $task = $asTaskOp.MakeGenericMethod($t).Invoke($null, @($op))
  $task.Wait(-1) | Out-Null
  $task.Result
}
$null = [Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager, Windows.Media.Control, ContentType = WindowsRuntime]
$tMgr = [Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager]
$mgr = Await ($tMgr::RequestAsync()) $tMgr
$session = $mgr.GetSessions() | Where-Object { $_.SourceAppUserModelId -like '*Spotify*' } | Select-Object -First 1
if (-not $session) { throw 'No Spotify session found.' }
$log = Get-ChildItem (Join-Path $env:APPDATA 'LyricHub\logs') -Filter 'lyrichub*.log' | Sort-Object LastWriteTime | Select-Object -Last 1

function Key([byte[]]$Vks) {
  foreach ($vk in $Vks) { [K]::keybd_event($vk, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 20 }
  [array]::Reverse($Vks)
  foreach ($vk in $Vks) { [K]::keybd_event($vk, 0, 2, [UIntPtr]::Zero); Start-Sleep -Milliseconds 20 }
}
function NudgeLater { Key ([byte[]](0x11, 0x12, 0xBE)) }   # Ctrl+Alt+.
function NudgeEarlier { Key ([byte[]](0x11, 0x12, 0xBC)) } # Ctrl+Alt+,
function LogSince([DateTime]$since, [string]$pattern) {
  Get-Content $log.FullName -Tail 600 | Where-Object { $_ -match $pattern } | ForEach-Object {
    $ts = [DateTime]::Parse(($_ -split '\s+')[0], $null, [Globalization.DateTimeStyles]::RoundtripKind).ToUniversalTime()
    if ($ts -gt $since) { $_ }
  }
}
function Last-Offset([DateTime]$since) {
  $line = LogSince $since 'song offset (applied|nudged)' | Select-Object -Last 1
  if ($line -match 'key=(.+?) (?:delta_ms=-?\d+ )?song_offset_ms=(-?\d+)') { [pscustomobject]@{ Key = $Matches[1]; Ms = [int]$Matches[2] } }
}
$ok = $true
function Check([string]$name, [bool]$pass, [string]$detail) {
  "[{0}] {1} - {2}" -f $(if ($pass) { 'PASS' } else { 'FAIL' }), $name, $detail
  if (-not $pass) { $script:ok = $false }
}

if ($session.GetPlaybackInfo().PlaybackStatus -ne 'Playing') { [void](Await ($session.TryPlayAsync()) ([bool])); Start-Sleep -Seconds 2 }
# "Repeat track" turns next and previous into a restart of the same song: off for the test.
$repeat = $session.GetPlaybackInfo().AutoRepeatMode
if ($repeat -eq [Windows.Media.MediaPlaybackAutoRepeatMode]::Track) {
  'Spotify repeats the current track; switching repeat off for the test'
  [void](Await ($session.TryChangeAutoRepeatModeAsync([Windows.Media.MediaPlaybackAutoRepeatMode]::None)) ([bool]))
  Start-Sleep -Milliseconds 500
}
# Song A may already have a stored offset: everything is checked relative to it.
$t0 = [DateTime]::UtcNow
NudgeLater; Start-Sleep -Milliseconds 600
$first = Last-Offset $t0
NudgeLater; Start-Sleep -Milliseconds 600
$a = Last-Offset $t0
$start = if ($first) { $first.Ms - 100 } else { 0 }
Check 'nudge on song A' ($a -and $a.Ms -eq $start + 200) "$($a.Key): $start -> $($a.Ms) ms"

$t1 = [DateTime]::UtcNow
[void](Await ($session.TrySkipNextAsync()) ([bool])); Start-Sleep -Milliseconds 1800
$b = Last-Offset $t1
Check 'next song B starts with its own offset' ($b -and $b.Key -ne $a.Key) "$($b.Key): $($b.Ms) ms"

$t2 = [DateTime]::UtcNow
# Spotify restarts the song on "previous" after the first seconds: press until A plays again.
for ($i = 0; $i -lt 3; $i++) {
  [void](Await ($session.TrySkipPreviousAsync()) ([bool])); Start-Sleep -Milliseconds 1800
  $c = Last-Offset $t2
  if ($c -and $c.Key -eq $a.Key) { break }
}
Check 'back on song A, its stored offset applies again' ($c -and $c.Key -eq $a.Key -and $c.Ms -eq $a.Ms) "$($c.Key): $($c.Ms) ms"

$t3 = [DateTime]::UtcNow
if ($c -and $c.Key -eq $a.Key) {
  NudgeEarlier; Start-Sleep -Milliseconds 600; NudgeEarlier; Start-Sleep -Milliseconds 600
  $d = Last-Offset $t3
  Check 'nudged back to the start value' ($d -and $d.Key -eq $a.Key -and $d.Ms -eq $start) "$($d.Key): $($d.Ms) ms"
} else {
  Check 'nudged back to the start value' $false "not on song A any more; reset $($a.Key) by hand (Synchronisation -> Auf 0)"
}
if ($repeat -eq [Windows.Media.MediaPlaybackAutoRepeatMode]::Track) {
  [void](Await ($session.TryChangeAutoRepeatModeAsync($repeat)) ([bool]))
  'repeat track restored'
}
if ($ok) { 'all passed'; exit 0 } else { exit 1 }
