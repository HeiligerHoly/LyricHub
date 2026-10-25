<#
.SYNOPSIS
  Measures CPU and memory of a running LyricHub including its WebView2 child processes
  (PLAN.md 2, performance criterion).

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File tools\overlay-spike\measure-cpu.ps1 -Seconds 10
#>
param(
  [int]$Seconds = 10,
  [int]$WarmupSeconds = 0
)

$ErrorActionPreference = 'Stop'
for ($i = 0; $i -lt 40 -and -not (Get-Process lyrichub -ErrorAction SilentlyContinue); $i++) { Start-Sleep -Milliseconds 250 }
if ($WarmupSeconds -gt 0) { Start-Sleep -Seconds $WarmupSeconds }

$root = (Get-Process lyrichub).Id
$all = Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, Name, CommandLine
$ids = New-Object System.Collections.Generic.List[int]
$ids.Add($root)
do {
  $added = $false
  foreach ($p in $all) {
    if ($ids.Contains([int]$p.ParentProcessId) -and -not $ids.Contains([int]$p.ProcessId)) { $ids.Add([int]$p.ProcessId); $added = $true }
  }
} while ($added)

function Get-CpuMs { $h = @{}; foreach ($p in (Get-Process -Id $ids -ErrorAction SilentlyContinue)) { $h[$p.Id] = $p.TotalProcessorTime.TotalMilliseconds }; $h }

$a = Get-CpuMs
$sw = [Diagnostics.Stopwatch]::StartNew()
Start-Sleep -Seconds $Seconds
$b = Get-CpuMs
$elapsed = $sw.Elapsed.TotalMilliseconds
$cores = [Environment]::ProcessorCount

$total = 0.0
foreach ($id in $b.Keys) {
  $delta = $b[$id] - $a[$id]
  $total += $delta
  $cmd = ($all | Where-Object ProcessId -eq $id).CommandLine
  $type = if ($cmd -match '--type=([a-z-]+)') { $Matches[1] } else { 'main' }
  $share = $delta / $elapsed * 100
  if ($share -ge 0.1) { '{0,-16} {1,-14} {2,6:N1} % of one core' -f (Get-Process -Id $id).Name, $type, $share }
}
$mem = (Get-Process -Id $ids -ErrorAction SilentlyContinue | Measure-Object -Property WorkingSet64 -Sum).Sum / 1MB
'TOTAL {0:N1} % of one core = {1:N2} % of {2} cores, working set {3:N0} MB' -f ($total / $elapsed * 100), ($total / $elapsed * 100 / $cores), $cores, $mem
