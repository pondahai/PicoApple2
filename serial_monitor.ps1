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
            
            # 雙向讀寫迴圈
            while ($port.IsOpen) {
                # 1. 接收 Pico 傳回的資料 (輸出至螢幕)
                if ($port.BytesToRead -gt 0) {
                    $data = $port.ReadExisting()
                    Write-Host $data -NoNewline
                }

                # 2. 捕捉鍵盤輸入 (傳送至 Pico)
                if ([Console]::KeyAvailable) {
                    $keyInfo = [Console]::ReadKey($true)
                    $esc = [char]27
                    
                    # 處理特殊按鍵的轉義序列 (ANSI)
                    if ($keyInfo.Key -eq 'UpArrow') { $port.Write("${esc}[A") }
                    elseif ($keyInfo.Key -eq 'DownArrow') { $port.Write("${esc}[B") }
                    elseif ($keyInfo.Key -eq 'RightArrow') { $port.Write("${esc}[C") }
                    elseif ($keyInfo.Key -eq 'LeftArrow') { $port.Write("${esc}[D") }
                    elseif ($keyInfo.Key -eq 'F1') { $port.Write("${esc}OP") }
                    elseif ($keyInfo.Key -eq 'F2') { $port.Write("${esc}OQ") }
                    elseif ($keyInfo.Key -eq 'F3') { $port.Write("${esc}OR") }
                    elseif ($keyInfo.Key -eq 'F4') { $port.Write("${esc}[15~") }
                    elseif ($keyInfo.Key -eq 'PageUp') { $port.Write("${esc}[5~") }
                    elseif ($keyInfo.Key -eq 'PageDown') { $port.Write("${esc}[6~") }
                    elseif ($keyInfo.Key -eq 'Backspace') { $port.Write([char]8) }
                    elseif ($keyInfo.Key -eq 'Enter') { $port.Write([char]13) }
                    elseif ($keyInfo.Key -eq 'Escape') { $port.Write("${esc}") }
                    else {
                        # 傳送一般字元
                        $port.Write($keyInfo.KeyChar)
                    }
                }
                Start-Sleep -Milliseconds 1 
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
