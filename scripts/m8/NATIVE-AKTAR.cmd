@echo off
cd /d "%~dp0"
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0NATIVE-AKTAR.ps1"
echo.
pause
