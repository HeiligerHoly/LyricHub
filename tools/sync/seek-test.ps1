<#
.SYNOPSIS
  Seek test of the sync protocol (PLAN.md 11.3, criterion in 2: correct again within 300 ms).

.DESCRIPTION
  Seeks the running Spotify session through the Windows media session (SMTC), alternately
  forward and back, and reads LyricHub's log for "seek shown in overlay": LyricHub logs it
  once the overlay has drawn the anchor of that seek. The time from the seek command to that
  line is the end-to-end seek latency. Needs LyricHub and Spotify running and playing; moves
  the playback position of the current song.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools\sync\seek-test.ps1 -Count 5
#>
param(
  [int]$Count = 5,
  [int]$StepSec = 25,
  [int]$WaitSec = 3
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Runtime.WindowsRuntime
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

$logDir = Join-Path $env:APPDATA 'LyricHub\logs'
$log = Get-ChildItem $logDir -Filter 'lyrichub*.log' | Sort-Object LastWriteTime | Select-Object -Last 1
if (-not $log) { throw "No LyricHub log in $logDir" }

# Seeks while paused are adopted directly and draw nothing to confirm: play during the test.
$wasPaused = $session.GetPlaybackInfo().PlaybackStatus -ne 'Playing'
if ($wasPaused) {
  'Spotify was paused, starting playback for the test'
  [void](Await ($session.TryPlayAsync()) ([bool]))
  Start-Sleep -Seconds 2
}

function Position-Ms {
  $tl = $session.GetTimelineProperties()
  $pos = $tl.Position.TotalMilliseconds
  if ($session.GetPlaybackInfo().PlaybackStatus -eq 'Playing') {
    $pos += ([DateTimeOffset]::UtcNow - $tl.LastUpdatedTime).TotalMilliseconds
  }
  [pscustomobject]@{ Pos = $pos; End = $tl.EndTime.TotalMilliseconds }
}

$results = New-Object System.Collections.Generic.List[double]
for ($i = 1; $i -le $Count; $i++) {
  $p = Position-Ms
  $dir = if ($i % 2 -eq 1) { 1 } else { -1 }
  $target = $p.Pos + $dir * $StepSec * 1000
  if ($target -gt $p.End - 15000 -or $target -lt 5000) { $target = [Math]::Min(30000, $p.End / 3) }
  $sent = [DateTime]::UtcNow
  [void](Await ($session.TryChangePlaybackPositionAsync([long]($target * 10000))) ([bool]))
  Start-Sleep -Seconds $WaitSec
  $line = Get-Content $log.FullName -Tail 400 | Where-Object { $_ -match 'seek shown in overlay' } | ForEach-Object {
    $ts = [DateTime]::Parse(($_ -split '\s+')[0], $null, [Globalization.DateTimeStyles]::RoundtripKind).ToUniversalTime()
    [pscustomobject]@{ Time = $ts; Text = $_ }
  } | Where-Object { $_.Time -gt $sent } | Select-Object -First 1
  if ($line) {
    $ms = ($line.Time - $sent).TotalMilliseconds
    $results.Add($ms)
    $report = if ($line.Text -match 'since_report_ms=([\d.]+)') { $Matches[1] } else { '?' }
    "{0}. seek {1:+0;-0} s to {2:N1} s: overlay correct after {3:N0} ms (from Spotify's report {4} ms)" -f $i, ($dir * $StepSec), ($target / 1000), $ms, $report
  } else {
    "{0}. seek to {1:N1} s: no 'seek shown in overlay' within {2} s" -f $i, ($target / 1000), $WaitSec
  }
}

if ($wasPaused) { [void](Await ($session.TryPauseAsync()) ([bool])) }

if ($results.Count) {
  $sorted = $results | Sort-Object
  $median = $sorted[[int][Math]::Floor(($sorted.Count - 1) / 2)]
  "median {0:N0} ms, max {1:N0} ms over {2} seeks (target <= 300 ms)" -f $median, ($sorted | Select-Object -Last 1), $sorted.Count
  if (($sorted | Select-Object -Last 1) -gt 300) { exit 1 }
} else { exit 1 }
