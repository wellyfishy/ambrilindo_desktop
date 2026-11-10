@echo off
title Ambrilindo Arena - Desktop Launcher
echo ===================================================
echo   Menjalankan Ambrilindo Arena (Tauri Desktop App)
echo ===================================================
echo.

cd /d "%~dp0"
echo [1/3] Menyiapkan bundle web juri & venue (npm run build)...
call npm.cmd run build

echo [2/3] Membebaskan port dev server (1420)...
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0free-dev-port.ps1"
if %ERRORLEVEL% neq 0 (
    echo.
    echo [ERROR] Port 1420 masih dipakai program lain.
    pause
    exit /b 1
)

echo [3/3] Menjalankan mode dev desktop (Hot-Reload aktif)...
call npm.cmd run tauri dev

if %ERRORLEVEL% neq 0 (
    echo.
    echo [ERROR] Terjadi kesalahan saat menjalankan aplikasi.
    pause
)
