<#
.SYNOPSIS
  Spike A (PLAN.md 5.8): samples the Windows media session (SMTC) of Spotify and logs
  the reported timeline to CSV so its update cadence and jitter can be analysed.

.DESCRIPTION
  Uses the same WinRT API the Rust app will use (Windows.Media.Control). The timeline
  values come from Spotify itself, so the measured cadence and jitter do not depend on
  the reader's language. Runs on Windows PowerShell 5.1 (WinRT projection is not
  available in PowerShell 7).

  Passive mode only reads. -Active additionally drives playback through SMTC
  (seek and pause/resume every cycle) so seek and pause latency can be measured
  against a known request time.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File smtc-probe.ps1 -DurationSec 600
  powershell -NoProfile -ExecutionPolicy Bypass -File smtc-probe.ps1 -DurationSec 240 -Active
#>
param(
  [int]$DurationSec = 600,
  [int]$IntervalMs = 50,
  [string]$OutFile = (Join-Path $PSScriptRoot '..\..\docs\spike-smtc.csv'),
  [string]$AppFilter = 'Spotify',
  [switch]$Active,
  [int]$ActiveCycleSec = 40
)

$ErrorActionPreference = 'Stop'
# Decimal points, not the locale's decimal comma, so the CSV stays parseable.
[System.Threading.Thread]::CurrentThread.CurrentCulture = [System.Globalization.CultureInfo]::InvariantCulture

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class PreciseTime {
    [DllImport("kernel32.dll")] static extern void GetSystemTimePreciseAsFileTime(out long ft);
    [DllImport("winmm.dll")] public static extern uint timeBeginPeriod(uint ms);
    [DllImport("winmm.dll")] public static extern uint timeEndPeriod(uint ms);
    // UTC ticks (100 ns since 0001-01-01), same epoch as DateTimeOffset.UtcTicks
    public static long UtcTicks() { long ft; GetSystemTimePreciseAsFileTime(out ft); return DateTime.FromFileTimeUtc(ft).Ticks; }
}
'@

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
$tMgr   = [Windows.Media.Control.GlobalSystemMediaTransportControlsSessionManager]
$tProps = [Windows.Media.Control.GlobalSystemMediaTransportControlsSessionMediaProperties]
$mgr = Await ($tMgr::RequestAsync()) $tMgr

function Find-Session {
  foreach ($s in $mgr.GetSessions()) {
    if ($s.SourceAppUserModelId -like "*$AppFilter*") { return $s }
  }
  return $null
}

function Csv([string]$v) { '"' + ($v -replace '"', '""') + '"' }

$outPath = [System.IO.Path]::GetFullPath($OutFile)
$null = New-Item -ItemType Directory -Force -Path ([System.IO.Path]::GetDirectoryName($outPath))
$writer = New-Object System.IO.StreamWriter($outPath, $false, (New-Object System.Text.UTF8Encoding($false)))
$writer.AutoFlush = $true
$writer.WriteLine('mono_ms,now_utc_ticks,event,app,status,pos_ms,last_updated_utc_ticks,end_ms,title,artist')

[void][PreciseTime]::timeBeginPeriod(1)
$sw = [System.Diagnostics.Stopwatch]::StartNew()
$session = $null
$nextSessionCheck = 0.0
$nextPropsCheck = 0.0
$title = ''; $artist = ''
$nextAction = if ($Active) { 10.0 } else { [double]::MaxValue }
$actionStep = 0
$samples = 0

Write-Host "SMTC probe -> $outPath  (duration ${DurationSec}s, interval ${IntervalMs}ms, active=$Active)"

if ($Active) {
  # Active mode needs running playback; start it once so the pause/play steps are deterministic.
  $s0 = Find-Session
  if ($s0 -ne $null -and $s0.GetPlaybackInfo().PlaybackStatus -ne 'Playing') {
    $writer.WriteLine(('{0:F3},{1},action_play,,,,,,,' -f $sw.Elapsed.TotalMilliseconds, [PreciseTime]::UtcTicks()))
    [void](Await ($s0.TryPlayAsync()) ([bool]))
  }
}

try {
  while ($sw.Elapsed.TotalSeconds -lt $DurationSec) {
    $tickStart = $sw.Elapsed.TotalMilliseconds

    if ($session -eq $null -or $tickStart -ge $nextSessionCheck) {
      $session = Find-Session
      $nextSessionCheck = $tickStart + 2000
    }

    if ($session -eq $null) {
      $writer.WriteLine(('{0:F3},{1},no_session,,,,,,,' -f $tickStart, [PreciseTime]::UtcTicks()))
    } else {
      if ($tickStart -ge $nextPropsCheck) {
        try {
          $p = Await ($session.TryGetMediaPropertiesAsync()) $tProps
          $title = $p.Title; $artist = $p.Artist
        } catch { }
        $nextPropsCheck = $tickStart + 250
      }

      # Optional active control: seek, then pause and resume, once per cycle.
      $elapsedSec = $tickStart / 1000.0
      if ($elapsedSec -ge $nextAction) {
        $evt = $null
        try {
          switch ($actionStep % 3) {
            0 {
              $tl0 = $session.GetTimelineProperties()
              $end = $tl0.EndTime.TotalMilliseconds
              $target = [Math]::Min($tl0.Position.TotalMilliseconds + 30000, [Math]::Max(5000, $end - 15000))
              if ($target -le $tl0.Position.TotalMilliseconds) { $target = 5000 }
              $evt = "action_seek:$([int]$target)"
              $writer.WriteLine(('{0:F3},{1},{2},,,,,,,' -f $sw.Elapsed.TotalMilliseconds, [PreciseTime]::UtcTicks(), $evt))
              [void](Await ($session.TryChangePlaybackPositionAsync([long]($target * 10000))) ([bool]))
              $nextAction = $elapsedSec + 12
            }
            1 {
              $evt = 'action_pause'
              $writer.WriteLine(('{0:F3},{1},{2},,,,,,,' -f $sw.Elapsed.TotalMilliseconds, [PreciseTime]::UtcTicks(), $evt))
              [void](Await ($session.TryPauseAsync()) ([bool]))
              $nextAction = $elapsedSec + 3
            }
            2 {
              $evt = 'action_play'
              $writer.WriteLine(('{0:F3},{1},{2},,,,,,,' -f $sw.Elapsed.TotalMilliseconds, [PreciseTime]::UtcTicks(), $evt))
              [void](Await ($session.TryPlayAsync()) ([bool]))
              $nextAction = $elapsedSec + ($ActiveCycleSec - 15)
            }
          }
        } catch {
          $writer.WriteLine(('{0:F3},{1},action_error,,,,,,{2},' -f $sw.Elapsed.TotalMilliseconds, [PreciseTime]::UtcTicks(), (Csv $_.Exception.Message)))
        }
        $actionStep++
      }

      try {
        $tl = $session.GetTimelineProperties()
        $pi = $session.GetPlaybackInfo()
        $now = [PreciseTime]::UtcTicks()
        $line = '{0:F3},{1},sample,{2},{3},{4:F3},{5},{6:F3},{7},{8}' -f `
          $sw.Elapsed.TotalMilliseconds, $now, (Csv $session.SourceAppUserModelId), $pi.PlaybackStatus,
          $tl.Position.TotalMilliseconds, $tl.LastUpdatedTime.UtcTicks, $tl.EndTime.TotalMilliseconds,
          (Csv $title), (Csv $artist)
        $writer.WriteLine($line)
        $samples++
      } catch {
        $session = $null
      }
    }

    $sleep = $IntervalMs - ($sw.Elapsed.TotalMilliseconds - $tickStart)
    if ($sleep -gt 1) { [System.Threading.Thread]::Sleep([int]$sleep) }
  }
} finally {
  [void][PreciseTime]::timeEndPeriod(1)
  $writer.Dispose()
  Write-Host "Done: $samples samples written."
}
