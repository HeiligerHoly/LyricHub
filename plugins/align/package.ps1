# Builds lyrichub-align and packs the release ZIP (PLAN-plugins.md 5.6): plugin.json, the exe,
# LICENSE.txt and the third-party notices. Prints the ZIP's path and SHA-256 for the release notes.
#   powershell -NoProfile -ExecutionPolicy Bypass -File plugins\align\package.ps1
param([string]$Out = "$PSScriptRoot\target\dist")
$ErrorActionPreference = 'Stop'
Push-Location $PSScriptRoot
try {
    cargo build --release --locked --bin lyrichub-align
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
    $version = (Get-Content plugin.json -Raw | ConvertFrom-Json).version
    $stage = Join-Path $Out "lyrichub-align-$version"
    if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
    New-Item -ItemType Directory -Force $stage | Out-Null
    Copy-Item plugin.json, target\release\lyrichub-align.exe, THIRD_PARTY_NOTICES.md $stage
    Copy-Item ..\..\tools\licenses\onnxruntime-ThirdPartyNotices.txt $stage
    Copy-Item ..\..\LICENSE (Join-Path $stage 'LICENSE.txt')
    $zip = Join-Path $Out "lyrichub-align-$version.zip"
    if (Test-Path $zip) { Remove-Item $zip -Force }
    Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $zip
    $zip
    (Get-FileHash $zip -Algorithm SHA256).Hash.ToLower()
} finally {
    Pop-Location
}
