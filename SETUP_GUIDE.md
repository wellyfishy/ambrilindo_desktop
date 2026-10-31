# Setup Guide: Ambrilindo Desktop di PC Baru

Panduan cepat untuk mengklon dan menjalankan **Ambrilindo Arena (Desktop App)** di PC atau laptop lain.

---

## 1. Prasyarat Sistem (Prerequisites)
Pastikan PC memiliki:
- **Windows 10 / 11 (64-bit)**
- **Git for Windows** ([Unduh Git](https://git-scm.com/download/win))
- **Node.js (LTS v18 atau v20+)** ([Unduh Node.js](https://nodejs.org/))
- **Rust & Cargo** ([Unduh Rustup](https://rustup.rs/))
  *Catatan: Saat install Rustup, pilih opsi default MSVC. Jika ditanya Visual Studio C++ Build Tools, izinkan installer untuk memasangnya.*

---

## 2. Cara Install & Setup 1-Click

1. Buka Terminal / PowerShell / Command Prompt di PC baru, lalu clone repository:
   ```cmd
   git clone https://github.com/wellyfishy/ambrilindo_desktop.git
   cd ambrilindo_desktop
   ```

2. **Jalankan Installer 1-Click**:
   - Cukup dobel-klik file: `install.bat`
   - Atau lewat CMD:
     ```cmd
     install.bat
     ```
   *Skrip akan secara otomatis memeriksa ketersediaan Node.js dan Rust, membuat folder data turnamen, menginstal dependensi npm (`npm install`), dan memverifikasi modul Rust (`cargo check`).*

---

## 3. Cara Menjalankan Aplikasi

- **Mode Development (Hot-Reload)**:
  - Dobel-klik file: `run-desktop.bat`
  - Atau lewat terminal:
    ```cmd
    npm run tauri dev
    ```

- **Build Installer Production (.exe / MSI)**:
  - Dobel-klik file: `build-desktop.bat`
  - Atau lewat terminal:
    ```cmd
    npm run tauri build
    ```
  *Installer release akan dibuat di folder `src-tauri/target/release/bundle/`.*

---

## 4. Sinkronisasi Data Turnamen Antar-PC

Database turnamen lokal SQLite tersimpan di dalam folder:
`tournament_data/db.sqlite3`
beserta file roster festival `tournament_data/festival_roster_*.json`.

- Karena file database turnamen sudah dilacak (tracked) di dalam git repo ini, setelah Anda melakukan `git pull`, data turnamen otomatis tersinkronisasi.
- Jika Anda ingin menyalin database turnamen terbaru secara manual dari PC pertama ke PC kedua tanpa git, cukup salin file `tournament_data/db.sqlite3` dan letakkan di folder yang sama di PC kedua.
