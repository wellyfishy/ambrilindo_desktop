@echo off
setlocal enabledelayedexpansion
title Ambrilindo Desktop - 1-Click Installer and Setup

echo =====================================================================
echo           AMBRILINDO ARENA DESKTOP - 1-CLICK SETUP INSTALLER
echo =====================================================================
echo.
cd /d "%~dp0"

:: 1. Check Winget Availability
set HAS_WINGET=0
where winget >nul 2>&1
if %ERRORLEVEL% equ 0 (
    set HAS_WINGET=1
)

:: 2. Check Node.js
echo [1/5] Memeriksa instalasi Node.js...
set HAS_NODE=0
where node >nul 2>&1
if %ERRORLEVEL% equ 0 (
    for /f "tokens=*" %%v in ('node -v') do set NODE_VER=%%v
    echo [OK] Node.js terdeteksi: !NODE_VER!
    set HAS_NODE=1
) else (
    echo [PERINGATAN] Node.js TIDAK ditemukan di sistem ini.
    if !HAS_WINGET! equ 1 (
        echo Ingin menginstal Node.js LTS secara otomatis via winget? [Y/N]
        set /p INSTALL_NODE="Pilihan [Y]: "
        if "!INSTALL_NODE!"=="" set INSTALL_NODE=Y
        if /i "!INSTALL_NODE!"=="Y" (
            echo Mengunduh dan menginstal Node.js LTS...
            winget install OpenJS.NodeJS.LTS -e --accept-source-agreements --accept-package-agreements
            echo Silakan restart terminal atau buka ulang install.bat setelah instalasi selesai jika PATH belum terbaca.
        )
    ) else (
        echo Silakan download dan install Node.js LTS dari: https://nodejs.org/
    )
)

:: 3. Check Rust and Cargo
echo.
echo [2/5] Memeriksa instalasi Rust dan Cargo...
set HAS_CARGO=0
where cargo >nul 2>&1
if %ERRORLEVEL% equ 0 (
    for /f "tokens=*" %%v in ('cargo --version') do set CARGO_VER=%%v
    echo [OK] Rust/Cargo terdeteksi: !CARGO_VER!
    set HAS_CARGO=1
) else (
    echo [PERINGATAN] Rust/Cargo TIDAK ditemukan di sistem ini.
    echo Ambrilindo Desktop memerlukan Rust untuk backend Tauri.
    if !HAS_WINGET! equ 1 (
        echo Ingin menginstal Rustup secara otomatis via winget? [Y/N]
        set /p INSTALL_RUST="Pilihan [Y]: "
        if "!INSTALL_RUST!"=="" set INSTALL_RUST=Y
        if /i "!INSTALL_RUST!"=="Y" (
            echo Mengunduh dan menginstal Rustup...
            winget install Rustlang.Rustup -e --accept-source-agreements --accept-package-agreements
            echo [PENTING] Setelah instalasi Rustup selesai, ikuti petunjuk visual rustup atau restart installer.
        )
    ) else (
        echo Silakan download dan install Rustup dari: https://rustup.rs/
    )
)

:: 4. Check and Create Tournament Data folders
echo.
echo [3/5] Memeriksa folder database turnamen...
if not exist "tournament_data" (
    mkdir "tournament_data"
    echo [OK] Folder tournament_data dibuat.
) else (
    echo [OK] Folder tournament_data tersedia.
)
if not exist "tournament_data\media" (
    mkdir "tournament_data\media"
)

:: 5. Install NPM Dependencies
echo.
echo [4/5] Menginstal dependensi Node.js [npm install]...
where npm.cmd >nul 2>&1
if %ERRORLEVEL% equ 0 (
    call npm.cmd install
    if !ERRORLEVEL! neq 0 (
        echo [ERROR] Gagal menjalankan npm install.
        pause
        exit /b 1
    )
    echo [OK] Dependensi npm berhasil diinstal.
) else (
    echo [ERROR] npm tidak ditemukan. Silakan pastikan Node.js terpasang dengan benar.
)

:: 6. Pre-compiling Rust backend
echo.
echo [5/5] Memeriksa dan memverifikasi Rust backend [src-tauri]...
where cargo >nul 2>&1
if %ERRORLEVEL% equ 0 (
    cd src-tauri
    call cargo check
    cd ..
    if !ERRORLEVEL! equ 0 (
        echo [OK] Rust backend siap.
    ) else (
        echo [WARNING] Cargo check mengalami masalah. Pastikan C++ Build Tools MSVC terpasang.
    )
) else (
    echo [INFO] Cargo belum terdeteksi di sesi saat ini. Setelah restart terminal, jalankan cargo check jika diperlukan.
)

echo.
echo =====================================================================
echo           SETUP AMBRILINDO DESKTOP SELESAI DENGAN SUKSES!
echo =====================================================================
echo.
echo Untuk menjalankan aplikasi:
echo   - Dobel-klik file: run-desktop.bat [Development mode]
echo   - Atau jalankan:   npm run tauri dev
echo.
echo Untuk build installer production [.exe]:
echo   - Dobel-klik file: build-desktop.bat
echo.
set /p LAUNCH="Ingin langsung menjalankan Ambrilindo Desktop sekarang? [Y/N] [Y]: "
if "!LAUNCH!"=="" set LAUNCH=Y
if /i "!LAUNCH!"=="Y" (
    call run-desktop.bat
)
pause
