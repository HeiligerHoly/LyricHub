<#
.SYNOPSIS
  Runs Spike B end to end: debug build, then the overlay self-test in three scenarios.

.DESCRIPTION
  1. "plain": no game window
  2. "game-once": fake game sets TopMost once (typical game)
  3. "game-aggressive": fake game re-asserts TopMost every 500 ms (worst case)
  Each run starts lyrichub.exe with LYRICHUB_SELFTEST=exit and stores the report as
  docs/spike-overlay-<scenario>.json. Needs Rust, the MSVC Build Tools and npm packages.
  Close any running LyricHub first (single-instance would swallow the test run).

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools\overlay-spike\run-spike.ps1
#>
param(
  [switch]$SkipBuild,
  # Run only these scenarios, e.g. -Only game-aggressive
  [string[]]$Only = @('plain', 'game-once', 'game-aggressive'),
  # Overlay topmost reassert interval under test (watchdog default 2000)
  [int]$TopmostMs = 2000
)

$ErrorActionPreference = 'Stop'
$root = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$exe = Join-Path $root 'src-tauri\target\debug\lyrichub.exe'
$docs = Join-Path $root 'docs'

if (-not $SkipBuild) {
  Push-Location $root
  try {
    npx tauri build --debug --no-bundle
    if ($LASTEXITCODE -ne 0) { throw "tauri build failed ($LASTEXITCODE)" }
  } finally { Pop-Location }
}
if (-not (Test-Path $exe)) { throw "missing $exe" }

function Invoke-SelfTest([string]$Name, $FakeReassertMs) {
  Write-Host "== Scenario $Name"
  $game = $null
  if ($null -ne $FakeReassertMs) {
    # Start-Process does not quote array elements: paths with spaces need explicit quotes.
    $script = '"{0}"' -f (Join-Path $PSScriptRoot 'fake-game.ps1')
    $game = Start-Process powershell -PassThru -ArgumentList @(
      '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', $script,
      '-ReassertMs', $FakeReassertMs, '-Seconds', 60)
    Start-Sleep -Seconds 3
  }
  $stdout = Join-Path $env:TEMP "lyrichub-selftest-$Name.out"
  $env:LYRICHUB_SELFTEST = 'exit'
  $env:LYRICHUB_TOPMOST_MS = "$TopmostMs"
  try {
    $p = Start-Process $exe -PassThru -NoNewWindow -RedirectStandardOutput $stdout
    if (-not $p.WaitForExit(120000)) { $p.Kill(); throw "self-test '$Name' timed out" }
  } finally {
    Remove-Item Env:LYRICHUB_SELFTEST -ErrorAction SilentlyContinue
    Remove-Item Env:LYRICHUB_TOPMOST_MS -ErrorAction SilentlyContinue
    if ($game -and -not $game.HasExited) { $game.Kill() }
  }
  $line = Get-Content $stdout | Where-Object { $_ -like 'SELFTEST_RESULT *' } | Select-Object -Last 1
  if (-not $line) { throw "no SELFTEST_RESULT in output of '$Name' (see $stdout)" }
  $json = $line.Substring('SELFTEST_RESULT '.Length)
  $suffix = if ($TopmostMs -ne 2000) { "-topmost$TopmostMs" } else { '' }
  $target = Join-Path $docs "spike-overlay-$Name$suffix.json"
  [System.IO.File]::WriteAllText($target, $json, (New-Object System.Text.UTF8Encoding($false)))
  $r = $json | ConvertFrom-Json
  Write-Host ("   {0} passed, {1} failed -> {2}" -f $r.passed, $r.failed, $target)
}

$scenarios = [ordered]@{ 'plain' = $null; 'game-once' = 0; 'game-aggressive' = 500 }
foreach ($name in $scenarios.Keys) {
  if ($Only -contains $name) { Invoke-SelfTest $name $scenarios[$name] }
}
