# Convert PNG to ICO and ICNS for Tauri
Add-Type -AssemblyName System.Drawing

$sourcePng = "icons\icon.png"
$icoPath = "icons\icon.ico"
$icnsPath = "icons\icon.icns"

# Load the source image
$img = [System.Drawing.Image]::FromFile((Resolve-Path $sourcePng))

# Create ICO file with multiple sizes
$sizes = @(16, 32, 48, 64, 128, 256)
$ms = New-Object System.IO.MemoryStream

# ICO header
$writer = New-Object System.IO.BinaryWriter($ms)
$writer.Write([uint16]0)  # Reserved
$writer.Write([uint16]1)  # Type (1 = ICO)
$writer.Write([uint16]$sizes.Length)  # Number of images

$imageDataList = @()
$offset = 6 + ($sizes.Length * 16)  # Header + directory entries

foreach ($size in $sizes) {
    # Resize image
    $resized = New-Object System.Drawing.Bitmap($size, $size)
    $graphics = [System.Drawing.Graphics]::FromImage($resized)
    $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $graphics.DrawImage($img, 0, 0, $size, $size)
    $graphics.Dispose()
    
    # Save to PNG in memory
    $pngStream = New-Object System.IO.MemoryStream
    $resized.Save($pngStream, [System.Drawing.Imaging.ImageFormat]::Png)
    $pngData = $pngStream.ToArray()
    $pngStream.Dispose()
    $resized.Dispose()
    
    # Write directory entry
    $writer.Write([byte]$size)  # Width
    $writer.Write([byte]$size)  # Height
    $writer.Write([byte]0)      # Color palette
    $writer.Write([byte]0)      # Reserved
    $writer.Write([uint16]1)    # Color planes
    $writer.Write([uint16]32)   # Bits per pixel
    $writer.Write([uint32]$pngData.Length)  # Size of image data
    $writer.Write([uint32]$offset)  # Offset to image data
    
    $imageDataList += $pngData
    $offset += $pngData.Length
}

# Write image data
foreach ($data in $imageDataList) {
    $writer.Write($data)
}

$writer.Flush()
$icoData = $ms.ToArray()
[System.IO.File]::WriteAllBytes((Join-Path (Get-Location) $icoPath), $icoData)

$writer.Dispose()
$ms.Dispose()
$img.Dispose()

Write-Host "ICO file created: $icoPath"

# For ICNS, we'll just copy the PNG (Tauri can handle PNG for macOS)
Copy-Item $sourcePng $icnsPath -Force
Write-Host "ICNS placeholder created: $icnsPath (using PNG)"

Write-Host "Icon conversion complete!"
