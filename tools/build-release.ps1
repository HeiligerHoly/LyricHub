# Release build: the LyricHub installer and the plugin ZIPs. Before building it refuses a working
# tree with changes, runs the tests of the app and the plugins, the frontend check and the
# security audits, and builds with the locked dependency versions. At the end it writes
# SHA256SUMS.txt for the release files.
#
# Rust embeds the source paths of the crates in the Cargo home folder for panic messages
# (C:\Users\<name>\.cargo\registry\...); here that folder is replaced by ~/.cargo, so no local
# user name ends up in the files. Every executable is checked for paths with a user folder.
#   powershell -NoProfile -ExecutionPolicy Bypass -File tools\build-release.ps1
# -AllowDirty is for test builds only: a release comes from a committed state.
param([switch]$SkipPlugin, [switch]$AllowDirty)
$ErrorActionPreference = 'Stop'

function Run([string]$what, [scriptblock]$command) {
    "== $what"
    & $command
    if ($LASTEXITCODE -ne 0) { throw "$what failed" }
}

$root = Resolve-Path (Join-Path $PSScriptRoot '..')
Push-Location $root
try {
    $changes = git status --porcelain
    if ($changes -and -not $AllowDirty) { throw "the working tree has changes; commit them first:`n$($changes -join "`n")" }
    if (-not (Get-Command cargo-audit -ErrorAction SilentlyContinue)) { throw 'cargo-audit is missing: cargo install cargo-audit --locked' }

    $plugins = if ($SkipPlugin) { @() } else { @('align', 'recognize') }
    # Tests before the path remapping below, so they reuse the normal development builds.
    Run 'frontend check' { npm run check }
    Run 'npm audit' { npm audit --omit=dev }
    foreach ($crate in @('src-tauri') + ($plugins | ForEach-Object { "plugins\$_" })) {
        Run "tests $crate" { cargo test --locked --quiet --manifest-path "$crate\Cargo.toml" }
        Push-Location $crate
        try { Run "cargo audit $crate" { cargo audit --quiet } } finally { Pop-Location }
    }

    $cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
    # Encoded flags are separated by 0x1F, so a user folder with spaces stays one argument.
    $env:CARGO_ENCODED_RUSTFLAGS = "--remap-path-prefix=$cargoHome=~/.cargo"
    Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue

    # The CLI directly: PowerShell's npx.ps1 would eat the `--` before cargo's arguments.
    Run 'tauri build' { node node_modules\@tauri-apps\cli\tauri.js build -- --locked }
    $version = (Get-Content src-tauri\tauri.conf.json -Raw | ConvertFrom-Json).version
    $exes = @(Join-Path $root 'src-tauri\target\release\lyrichub.exe')
    $files = @(Join-Path $root "src-tauri\target\release\bundle\nsis\LyricHub_${version}_x64-setup.exe")
    foreach ($plugin in $plugins) {
        # package.ps1 builds with --locked and prints the ZIP's path and its hash.
        $zip = & (Join-Path $root "plugins\$plugin\package.ps1") | Where-Object { "$_" -like '*.zip' } | Select-Object -First 1
        $exes += Join-Path $root "plugins\$plugin\target\release\lyrichub-$plugin.exe"
        $files += $zip
    }
    foreach ($exe in $exes) {
        $text = [Text.Encoding]::GetEncoding(28591).GetString([IO.File]::ReadAllBytes($exe))
        $found = [regex]::Matches($text, '[A-Za-z]:\\Users\\[^\\\x00]+\\').Count
        if ($found -gt 0) { throw "$exe still contains $found paths with a user folder" }
        "{0}: no paths with a user folder" -f (Split-Path $exe -Leaf)
    }

    # sha256sum format: hash, two spaces, file name.
    $sums = Join-Path $root 'src-tauri\target\release\bundle\SHA256SUMS.txt'
    $lines = foreach ($f in $files) { '{0}  {1}' -f (Get-FileHash $f -Algorithm SHA256).Hash.ToLower(), (Split-Path $f -Leaf) }
    [IO.File]::WriteAllText($sums, ($lines -join "`n") + "`n")
    "== release files"
    $files
    $sums
    $lines
} finally {
    Pop-Location
}
