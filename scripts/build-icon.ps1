param(
    [string]$Source = (Join-Path (Split-Path -Parent $PSScriptRoot) 'assets\juna-logo.png'),
    [string]$Output = (Join-Path (Split-Path -Parent $PSScriptRoot) 'assets\juna.ico'),
    [int[]]$Sizes = @(16, 24, 32, 48, 64, 128, 256)
)

$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Drawing

if (-not (Test-Path $Source)) { throw "source not found: $Source" }

$src = [System.Drawing.Image]::FromFile($Source)
Write-Host "source: $Source ($($src.Width)x$($src.Height)) $($src.PixelFormat)"

$pngs = @()
foreach ($s in $Sizes) {
    $bmp = New-Object System.Drawing.Bitmap($s, $s, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode  = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.PixelOffsetMode    = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
    $g.SmoothingMode      = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
    $g.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
    $g.CompositingMode    = [System.Drawing.Drawing2D.CompositingMode]::SourceCopy
    $g.Clear([System.Drawing.Color]::Transparent)
    $g.DrawImage($src, 0, 0, $s, $s)
    $g.Dispose()

    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    $pngs += , ([byte[]]$ms.ToArray())
    $ms.Dispose()
    $bmp.Dispose()
}

$count = $Sizes.Count
$dir = New-Object byte[] (6 + 16 * $count)
$dir[0] = 0; $dir[1] = 0
$dir[2] = 1; $dir[3] = 0
$dir[4] = [byte]$count; $dir[5] = 0

$offset = 6 + 16 * $count
for ($i = 0; $i -lt $count; $i++) {
    $b = 6 + $i * 16
    $dim = $Sizes[$i]
    $dir[$b + 0] = [byte]($(if ($dim -ge 256) { 0 } else { $dim }))
    $dir[$b + 1] = [byte]($(if ($dim -ge 256) { 0 } else { $dim }))
    $dir[$b + 2] = 0
    $dir[$b + 3] = 0
    $dir[$b + 4] = 1; $dir[$b + 5] = 0
    $dir[$b + 6] = 32; $dir[$b + 7] = 0
    $len = $pngs[$i].Length
    $dir[$b + 8]  = [byte]($len -band 0xFF)
    $dir[$b + 9]  = [byte](($len -shr 8) -band 0xFF)
    $dir[$b + 10] = [byte](($len -shr 16) -band 0xFF)
    $dir[$b + 11] = [byte](($len -shr 24) -band 0xFF)
    $dir[$b + 12] = [byte]($offset -band 0xFF)
    $dir[$b + 13] = [byte](($offset -shr 8) -band 0xFF)
    $dir[$b + 14] = [byte](($offset -shr 16) -band 0xFF)
    $dir[$b + 15] = [byte](($offset -shr 24) -band 0xFF)
    $offset += $len
}

$out = New-Object byte[] $offset
[Array]::Copy($dir, 0, $out, 0, $dir.Length)
$pos = 6 + 16 * $count
for ($i = 0; $i -lt $count; $i++) {
    [Array]::Copy($pngs[$i], 0, $out, $pos, $pngs[$i].Length)
    $pos += $pngs[$i].Length
}

[System.IO.File]::WriteAllBytes($Output, $out)
$src.Dispose()

Write-Host "wrote $Output ($([math]::Round($out.Length/1KB,1)) KB) with sizes: $($Sizes -join ', ')"