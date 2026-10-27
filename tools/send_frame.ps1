# Replays the reconstructed Turing 3.5" (USB35INCHIPSV2) protocol using the same
# System.IO.Ports.SerialPort class the vendor app uses.
param(
    [string]$Port = "COM20",
    [string]$Frame = "frame.bin",
    [int]$Width = 480,
    [int]$Height = 320
)

function New-Header([int]$cmd, [int]$x, [int]$y, [int]$ex, [int]$ey) {
    # Exact bit packing recovered from UsbMonitor.exe (device class, 5-int overload):
    #   b0 = x >> 2
    #   b1 = ((x & 3) << 6) | (y >> 4)
    #   b2 = ((y & 15) << 4) | (ex >> 6)
    #   b3 = ((ex & 63) << 2) | (ey >> 8)
    #   b4 = ey & 255
    #   b5 = cmd
    $b0 = (($x -shr 2) -band 0xFF)
    $b1 = (((($x -band 3) -shl 6) + ($y -shr 4)) -band 0xFF)
    $b2 = (((($y -band 15) -shl 4) + ($ex -shr 6)) -band 0xFF)
    $b3 = (((($ex -band 63) -shl 2) + ($ey -shr 8)) -band 0xFF)
    $b4 = ($ey -band 0xFF)
    $b5 = ($cmd -band 0xFF)
    return [byte[]]@($b0, $b1, $b2, $b3, $b4, $b5)
}

$sp = New-Object System.IO.Ports.SerialPort $Port, 115200, ([System.IO.Ports.Parity]::None), 8, ([System.IO.Ports.StopBits]::One)
$sp.DtrEnable = $true
$sp.RtsEnable = $true
$sp.ReadTimeout = 1000
$sp.WriteTimeout = 5000
$sp.Open()
try {
    Write-Output "opened $Port"
    # 109 = screen on
    $h = New-Header 109 0 0 0 0; $sp.Write($h, 0, 6); Start-Sleep -Milliseconds 10
    # 121 = set orientation: payload[6] = 100 + orientation (2 = landscape), [7..8] = width BE, [9..10] = height BE
    $p = New-Object byte[] 16
    $p[6] = 102; $p[7] = ($Width -shr 8); $p[8] = ($Width -band 0xFF); $p[9] = ($Height -shr 8); $p[10] = ($Height -band 0xFF)
    $h = New-Header 121 0 0 0 0
    [Array]::Copy($h, 0, $p, 0, 6)
    $sp.Write($p, 0, 16); Start-Sleep -Milliseconds 10
    # 197 = display bitmap into rect (x, y, x+w-1, y+h-1), then raw RGB565 LE pixels
    $data = [System.IO.File]::ReadAllBytes($Frame)
    if ($data.Length -ne $Width * $Height * 2) { throw "frame size mismatch: $($data.Length)" }
    $h = New-Header 197 0 0 ($Width - 1) ($Height - 1)
    Write-Output ("header 197 = " + (($h | ForEach-Object { $_.ToString("X2") }) -join " "))
    $t = [System.Diagnostics.Stopwatch]::StartNew()
    $sp.Write($h, 0, 6)
    $sp.Write($data, 0, $data.Length)
    $t.Stop()
    Write-Output ("wrote {0} bytes in {1} ms" -f $data.Length, $t.ElapsedMilliseconds)
    Start-Sleep -Milliseconds 50
    Write-Output ("bytes waiting from device: " + $sp.BytesToRead)
}
finally {
    $sp.Close(); $sp.Dispose()
    Write-Output "closed"
}
