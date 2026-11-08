@echo off
setlocal
title Cryptex VM Kontrol
"%SystemRoot%\System32\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "%~dp0collect-vm-check.ps1"
if errorlevel 1 (
  echo Kontrol tamamlanamadi. Yukaridaki hata turunu paylasin.
) else (
  echo Acilan klasordeki SONUC.zip dosyasini sohbete ekleyin.
)
pause
