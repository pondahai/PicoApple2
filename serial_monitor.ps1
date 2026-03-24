# Pico Serial Monitor - High-Speed Auto-Connect
param([string]$portName)

$baudRate = 115200

function Get-PicoPort {
    # 超快速檢查目前的 COM Port，過濾出 rp2040 核心的名稱或直接取最後一個 COM
    # 在 Pico 應用中，通常最後一個出現的 COM 就是它
    return [System.IO.Ports.SerialPort]::GetPortNames() | Select-Object -Last 1
}

Write-Host ">>> Professional Pico Terminal Active <<<" -ForegroundColor Cyan
Write-Host "Waiting for Pico device..." -ForegroundColor Gray

while ($true) {
    $currentPort = Get-PicoPort
    
    if ($currentPort) {
        Write-Host "`n[CONNECTING] Found $currentPort..." -ForegroundColor Yellow
        $port = New-Object System.IO.Ports.SerialPort $currentPort, $baudRate, None, 8, one
        $port.ReadTimeout = 500
        $port.DtrEnable = $true # 重要：Pico 有時需要 DTR 才能啟動 Serial 傳輸
        
        try {
            $port.Open()
            Write-Host "[CONNECTED] $currentPort is online.`n" -ForegroundColor Green
            
            # 高速讀取迴圈
            while ($port.IsOpen) {
                if ($port.BytesToRead -gt 0) {
                    $data = $port.ReadExisting()
                    Write-Host $data -NoNewline
                }
                Start-Sleep -Milliseconds 1 # 降到 1ms 達成「瞬間」感
            }
        } catch {
            Write-Host "`n[LOST] Connection to $currentPort interrupted." -ForegroundColor Red
        } finally {
            if ($port.IsOpen) { $port.Close() }
            $port.Dispose()
        }
    }
    
    # 如果沒找到 Port，每 100ms 快速掃描一次
    Start-Sleep -Milliseconds 100
}
