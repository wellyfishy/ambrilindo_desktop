@echo off
title Ambrilindo Arena - Desktop Release Builder
echo ===================================================
echo   Building Ambrilindo Arena (Production Release)
echo ===================================================
echo.

cd /d "%~dp0"
echo Memulai proses build bundle installer Tauri...
call npm.cmd run tauri build

if %ERRORLEVEL% neq 0 (
    echo.
    echo [ERROR] Build gagal. Silakan periksa log error di atas.
    pause
    exit /b %ERRORLEVEL%
)

echo.
echo ===================================================
echo   BUILD SUKSES!
echo   Installer tersimpan di: src-tauri\target\release\bundle\
echo ===================================================
pause
