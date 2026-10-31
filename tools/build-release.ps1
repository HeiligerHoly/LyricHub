# Release build: the LyricHub installer and the plugin ZIPs. Rust embeds the source paths of
# the crates in the Cargo home folder for panic messages (C:\Users\<name>\.cargo\registry\...);
# here that folder is replaced by ~/.cargo, so no local user name ends up in the files. At the
# end every executable is checked for paths with a user folder.
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools\build-release.ps1
param([switch]$SkipPlugin)
$ErrorActionPreference = 'Stop'

$cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
# Encoded flags are separated by 0x1F, so a user folder with spaces stays one argument.
$env:CARGO_ENCODED_RUSTFLAGS = "--remap-path-prefix=$cargoHome=~/.cargo"
Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue

$root = Resolve-Path (Join-Path $PSScriptRoot '..')
Push-Location $root
try {
    npx tauri build
    if ($LASTEXITCODE -ne 0) { throw 'tauri build failed' }
    $exes = @(Join-Path $root 'src-tauri\target\release\lyrichub.exe')
    if (-not $SkipPlugin) {
        foreach ($plugin in 'align', 'recognize') {
            & (Join-Path $root "plugins\$plugin\package.ps1")
            $exes += Join-Path $root "plugins\$plugin\target\release\lyrichub-$plugin.exe"
        }
    }
    foreach ($exe in $exes) {
        $text = [Text.Encoding]::GetEncoding(28591).GetString([IO.File]::ReadAllBytes($exe))
        $found = [regex]::Matches($text, '[A-Za-z]:\\Users\\[^\\\x00]+\\').Count
        if ($found -gt 0) { throw "$exe still contains $found paths with a user folder" }
        "{0}: no paths with a user folder" -f (Split-Path $exe -Leaf)
    }
} finally {
    Pop-Location
}
