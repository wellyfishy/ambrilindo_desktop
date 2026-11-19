@echo off
title Ambrilindo Arena - Quick Test Build
echo ===================================================
echo   Quick test build (fast profile, ~2 min after the first)
echo   For the final exe you hand out, use build-desktop.bat
echo ===================================================
echo.

cd /d "%~dp0"
set CARGO_BUILD_JOBS=6
call npm.cmd run tauri build -- --target x86_64-pc-windows-msvc --no-bundle -- --profile fast

if %ERRORLEVEL% neq 0 (
    echo.
    echo [ERROR] Build gagal. Silakan periksa log error di atas.
    pause
    exit /b %ERRORLEVEL%
)

echo.
echo ===================================================
echo   BUILD SUKSES!
echo   Exe: src-tauri\target\x86_64-pc-windows-msvc\fast\ambrilindo_desktop.exe
echo ===================================================
pause
