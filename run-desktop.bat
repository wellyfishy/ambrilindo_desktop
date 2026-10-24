@echo off
title Ambrilindo Arena — Desktop Launcher
echo ===================================================
echo   Menjalankan Ambrilindo Arena (Tauri Desktop App)
echo ===================================================
echo.

cd /d "%~dp0"
echo Menjalankan mode dev (Hot-Reload aktif)...
call npm.cmd run tauri dev

if %ERRORLEVEL% neq 0 (
    echo.
    echo [ERROR] Terjadi kesalahan saat menjalankan aplikasi.
    pause
)
