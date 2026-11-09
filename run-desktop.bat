@echo off
title Ambrilindo Arena - Desktop Launcher
echo ===================================================
echo   Menjalankan Ambrilindo Arena (Tauri Desktop App)
echo ===================================================
echo.

cd /d "%~dp0"
echo [1/2] Menyiapkan bundle web juri & venue (npm run build)...
call npm.cmd run build

echo [2/2] Menjalankan mode dev desktop (Hot-Reload aktif)...
call npm.cmd run tauri dev

if %ERRORLEVEL% neq 0 (
    echo.
    echo [ERROR] Terjadi kesalahan saat menjalankan aplikasi.
    pause
)
