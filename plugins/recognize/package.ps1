# Builds lyrichub-recognize and packs the release ZIP (PLAN-plugins.md 5.6): plugin.json, the exe,
# LICENSE.txt and the third-party notices. Prints the ZIP's path and SHA-256 for the release notes.
#   powershell -NoProfile -ExecutionPolicy Bypass -File plugins\recognize\package.ps1
param([string]$Out = "$PSScriptRoot\target\dist")
$ErrorActionPreference = 'Stop'
# No local user name in the exe: the Cargo home folder in embedded source paths becomes ~/.cargo
# (tools\build-release.ps1 explains it).
if (-not $env:CARGO_ENCODED_RUSTFLAGS) {
    $cargoHome = if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }
    $env:CARGO_ENCODED_RUSTFLAGS = "--remap-path-prefix=$cargoHome=~/.cargo"
}
Push-Location $PSScriptRoot
try {
    cargo build --release --locked --bin lyrichub-recognize
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
    $version = (Get-Content plugin.json -Raw | ConvertFrom-Json).version
    $stage = Join-Path $Out "lyrichub-recognize-$version"
    if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
    New-Item -ItemType Directory -Force $stage | Out-Null
    Copy-Item plugin.json, target\release\lyrichub-recognize.exe, THIRD_PARTY_NOTICES.md $stage
    Copy-Item ..\..\tools\licenses\onnxruntime-ThirdPartyNotices.txt $stage
    Copy-Item ..\..\LICENSE (Join-Path $stage 'LICENSE.txt')
    $zip = Join-Path $Out "lyrichub-recognize-$version.zip"
    if (Test-Path $zip) { Remove-Item $zip -Force }
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
    $zip
    (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
} finally {
    Pop-Location
}
