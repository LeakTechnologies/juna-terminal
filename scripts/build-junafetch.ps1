$ErrorActionPreference = 'Stop'
$env:CARGO_BUILD_JOBS = '2'
cargo build --locked --release --manifest-path tools/junafetch/Cargo.toml --target x86_64-pc-windows-gnu
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
Copy-Item -LiteralPath 'tools/junafetch/target/x86_64-pc-windows-gnu/release/junafetch.exe' -Destination 'assets/tools/junafetch-front.bin'
$manifest = Get-Content assets/tools/manifest.json -Raw | ConvertFrom-Json
$metadata = cargo metadata --locked --no-deps --format-version 1 --manifest-path tools/junafetch/Cargo.toml | ConvertFrom-Json
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
$manifest.frontend_version = ($metadata.packages | Where-Object name -eq 'junafetch').version
$manifest.frontend_sha256 = (Get-FileHash assets/tools/junafetch-front.bin -Algorithm SHA256).Hash.ToLowerInvariant()
foreach ($source in @('tools/junafetch/src/privacy.rs', 'src/privacy/wire.rs')) {
    $manifest.source_hashes | Add-Member -NotePropertyName $source -NotePropertyValue '' -Force
}
foreach ($entry in $manifest.source_hashes.PSObject.Properties) {
    if ($entry.Name.EndsWith('logo-full.txt')) { $bytes = [IO.File]::ReadAllBytes((Resolve-Path $entry.Name)) }
    else { $bytes = [Text.Encoding]::UTF8.GetBytes([IO.File]::ReadAllText((Resolve-Path $entry.Name)).Replace("`r`n", "`n")) }
    $sha = [Security.Cryptography.SHA256]::Create()
    try { $entry.Value = ([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-', '').ToLowerInvariant() }
    finally { $sha.Dispose() }
}
$manifest | ConvertTo-Json -Depth 6 | Set-Content assets/tools/manifest.json -Encoding utf8NoBOM
./scripts/verify-tools.ps1
