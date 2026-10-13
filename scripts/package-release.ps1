param(
    [string]$Executable = 'target/release/juna.exe',
    [string]$Tag = '',
    [string]$OutputDirectory = 'dist'
)
$ErrorActionPreference = 'Stop'
$metadata = (& cargo metadata --no-deps --format-version 1 | ConvertFrom-Json)
if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed' }
$version = ($metadata.packages | Where-Object name -eq 'juna').version
if ($version -notmatch '^\d+\.\d+\.\d+(-dev[1-9]\d*)?$') { throw 'Unsupported release version' }
$commit = (& git rev-parse HEAD).Trim()
if ($commit -notmatch '^[a-f0-9]{40}$') { throw 'Missing source commit' }
if ($Tag) {
    if ($Tag -ne "v$version") { throw 'Tag and Cargo version disagree' }
    $tagCommit = (& git rev-parse "${Tag}^{commit}").Trim()
    if ($LASTEXITCODE -ne 0 -or $tagCommit -ne $commit) { throw 'Tag does not identify this commit' }
    & git diff --exit-code HEAD
    if ($LASTEXITCODE -ne 0) { throw 'Release source has uncommitted changes' }
}
$exe = (Resolve-Path -LiteralPath $Executable).Path
$info = [Diagnostics.FileVersionInfo]::GetVersionInfo($exe)
if ($info.ProductVersion -ne $version -or $info.Comments -ne "Source commit: $commit") { throw 'Executable version/source metadata mismatch' }
$note = Join-Path 'releases' "$version.md"
if (-not (Test-Path -LiteralPath $note)) { throw 'Matching release notes are missing' }
$stage = Join-Path 'target/release-staging' ([Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $stage -Force | Out-Null
Copy-Item -LiteralPath $exe -Destination (Join-Path $stage 'juna.exe')
Copy-Item -LiteralPath $note -Destination (Join-Path $stage 'RELEASE_NOTES.md')
$noticeRoot = Join-Path $stage 'notices'
New-Item -ItemType Directory -Path $noticeRoot -Force | Out-Null
Copy-Item -LiteralPath 'licenses' -Destination (Join-Path $noticeRoot 'palettes') -Recurse
foreach ($font in Get-ChildItem -LiteralPath 'assets/fonts' -Directory) {
    if (-not @(Get-ChildItem -LiteralPath $font.FullName -File | Where-Object Extension -eq '.txt').Count) { throw "Font notice missing: $($font.Name)" }
    $folder = Join-Path $noticeRoot (Join-Path 'fonts' $font.Name)
    New-Item -ItemType Directory -Path $folder -Force | Out-Null
    foreach ($notice in Get-ChildItem -LiteralPath $font.FullName -File | Where-Object Extension -eq '.txt') {
        Copy-Item -LiteralPath $notice.FullName -Destination (Join-Path $folder $notice.Name)
    }
}
Copy-Item -LiteralPath 'assets/fonts/manifest.json' -Destination (Join-Path $noticeRoot 'font-manifest.json')
New-Item -ItemType Directory -Path $OutputDirectory -Force | Out-Null
$archiveName = "Juna_${version}_windows_amd64.zip"
$archive = Join-Path $OutputDirectory $archiveName
if (Test-Path -LiteralPath $archive) { throw 'Refusing to replace an existing version archive' }
Compress-Archive -Path (Join-Path $stage '*') -DestinationPath $archive -CompressionLevel Optimal
$digest = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
"$digest  $archiveName" | Set-Content -LiteralPath (Join-Path $OutputDirectory 'SHA256SUMS.txt') -Encoding utf8NoBOM
$buildInfo = [ordered]@{ version = $version; commit = $commit; target = 'x86_64-pc-windows-gnu'; asset = $archiveName; sha256 = $digest; prerelease = $version.Contains('-dev'); signature_verified = $false }
$buildInfo | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $OutputDirectory 'build-info.json') -Encoding utf8NoBOM
Write-Output "Packaged $archiveName ($digest)"
