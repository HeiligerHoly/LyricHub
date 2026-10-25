<#
.SYNOPSIS
  Spike B helper: a borderless fullscreen window that behaves like a game holding the
  topmost band. The LyricHub self-test detects it by its title and measures whether the
  overlay stays above it.

.PARAMETER ReassertMs
  How often the window re-asserts topmost. 0 = only once at start (most games).
  500 = aggressive, a worst case.

.PARAMETER Seconds
  Closes itself after this many seconds. Esc closes earlier.

.EXAMPLE
  powershell -NoProfile -ExecutionPolicy Bypass -File fake-game.ps1 -ReassertMs 0 -Seconds 40
#>
param(
  [int]$ReassertMs = 0,
  [int]$Seconds = 40
)

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

$screen = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$form = New-Object System.Windows.Forms.Form
$form.Text = 'LyricHub Fake Game'
$form.FormBorderStyle = 'None'
$form.StartPosition = 'Manual'
$form.Bounds = $screen
$form.TopMost = $true
$form.BackColor = [System.Drawing.Color]::FromArgb(24, 40, 32)
$form.KeyPreview = $true

$label = New-Object System.Windows.Forms.Label
$label.AutoSize = $false
$label.Dock = 'Fill'
$label.TextAlign = 'MiddleCenter'
$label.ForeColor = [System.Drawing.Color]::FromArgb(180, 220, 190)
$label.Font = New-Object System.Drawing.Font('Segoe UI', 20)
$label.Text = "LyricHub Fake Game`nBorderless Fullscreen, TopMost, Reassert: $ReassertMs ms`nEsc beendet, sonst nach $Seconds s"
$form.Controls.Add($label)

$form.Add_KeyDown({ if ($_.KeyCode -eq 'Escape') { $form.Close() } })

if ($ReassertMs -gt 0) {
  $reassert = New-Object System.Windows.Forms.Timer
  $reassert.Interval = $ReassertMs
  $reassert.Add_Tick({ $form.TopMost = $false; $form.TopMost = $true })
  $reassert.Start()
}

$close = New-Object System.Windows.Forms.Timer
$close.Interval = [Math]::Max(1, $Seconds) * 1000
$close.Add_Tick({ $form.Close() })
$close.Start()

$form.Add_Shown({ $form.Activate() })
[void]$form.ShowDialog()
