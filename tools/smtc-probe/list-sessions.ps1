<#
.SYNOPSIS
  Lists every media session Windows knows (SMTC): app ID, status, title, artist, album,
  timeline and the controls the app offers. Read only.

.DESCRIPTION
  Used to find the session ID of a player before measuring it with smtc-probe.ps1 -AppFilter,
  for example a Firefox-based browser, which reports a 16-digit hex ID instead of a name
  (docs/spike-browser.md). Runs on Windows PowerShell 5.1 (WinRT projection is not available
  in PowerShell 7).

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File list-sessions.ps1
#>
$ErrorActionPreference = 'Stop'
[System.Threading.Thread]::CurrentThread.CurrentCulture = [System.Globalization.CultureInfo]::InvariantCulture
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
$current = $mgr.GetCurrentSession()
foreach ($s in $mgr.GetSessions()) {
  $p = Await ($s.TryGetMediaPropertiesAsync()) $tProps
  $tl = $s.GetTimelineProperties()
  $pi = $s.GetPlaybackInfo()
  $c = $pi.Controls
  [pscustomobject]@{
    app           = $s.SourceAppUserModelId
    current       = ($current -ne $null -and $current.SourceAppUserModelId -eq $s.SourceAppUserModelId)
    status        = $pi.PlaybackStatus
    type          = $p.PlaybackType
    title         = $p.Title
    artist        = $p.Artist
    album         = $p.AlbumTitle
    position_s    = [Math]::Round($tl.Position.TotalSeconds, 2)
    end_s         = [Math]::Round($tl.EndTime.TotalSeconds, 2)
    updated_s_ago = [Math]::Round(([DateTimeOffset]::UtcNow - $tl.LastUpdatedTime).TotalSeconds, 1)
    can_seek      = $c.IsPlaybackPositionEnabled
    can_pause     = $c.IsPauseEnabled
  } | Format-List
}
