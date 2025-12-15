# Create a basic 32x32 PNG icon
Add-Type -AssemblyName System.Drawing

# Create a 32x32 bitmap
$bitmap = New-Object System.Drawing.Bitmap(32, 32)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)

# Fill with a simple color (blue)
$brush = New-Object System.Drawing.SolidBrush([System.Drawing.Color]::Blue)
$graphics.FillRectangle($brush, 0, 0, 32, 32)

# Add a simple white border
$pen = New-Object System.Drawing.Pen([System.Drawing.Color]::White, 2)
$graphics.DrawRectangle($pen, 1, 1, 30, 30)

# Save as PNG
$bitmap.Save("icons/icon.png", [System.Drawing.Imaging.ImageFormat]::Png)

# Clean up
$graphics.Dispose()
$bitmap.Dispose()
$brush.Dispose()
$pen.Dispose()

Write-Host "Basic PNG icon created successfully!"