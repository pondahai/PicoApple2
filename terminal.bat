@echo off
:: 直接啟動 PowerShell 高速監聽器
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0serial_monitor.ps1"
pause
