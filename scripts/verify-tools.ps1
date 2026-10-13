$ErrorActionPreference = 'Stop'
$manifest = Get-Content -LiteralPath 'assets/tools/manifest.json' -Raw | ConvertFrom-Json
if ($manifest.schema_version -ne 1 -or $manifest.detector_commit -notmatch '^[a-f0-9]{40}$') { throw 'Invalid tool pin manifest' }
foreach ($pair in @(@('tools/junafetch/assets/detector.bin',$manifest.detector_sha256),@('assets/tools/junafetch-front.bin',$manifest.frontend_sha256))) {
    if ((Get-FileHash -LiteralPath $pair[0] -Algorithm SHA256).Hash.ToLowerInvariant() -ne $pair[1]) { throw 'Pinned tool payload hash mismatch' }
}
foreach ($entry in $manifest.source_hashes.PSObject.Properties) {
    if ($entry.Name.EndsWith('logo-full.txt')) { $bytes=[IO.File]::ReadAllBytes((Resolve-Path -LiteralPath $entry.Name)) }
    else { $text=[IO.File]::ReadAllText((Resolve-Path -LiteralPath $entry.Name)).Replace("`r`n","`n"); $bytes=[Text.Encoding]::UTF8.GetBytes($text) }
    $sha=[Security.Cryptography.SHA256]::Create()
    try { $digest=([BitConverter]::ToString($sha.ComputeHash($bytes))).Replace('-','').ToLowerInvariant() } finally { $sha.Dispose() }
    if ($digest -ne $entry.Value) { throw "Tool source fingerprint mismatch: $($entry.Name)" }
}
Write-Output 'Pinned native/frontend payloads and normalized source fingerprints verified'
