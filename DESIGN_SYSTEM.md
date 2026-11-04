# Ambrilindo Desktop - Design System & UI Guidelines

Dokumen ini merupakan panduan patokan desain resmi (*Single Source of Truth*) untuk antarmuka pengguna (UI) dan pengalaman pengguna (UX) pada aplikasi **Ambrilindo Desktop & Embedded Arena LAN Server**.

---

## 1. Filosofi Desain

1. **Focused Arena Operations (Operasional Terfokus)**:
   Aplikasi digunakan di tengah hiruk-pikuk arena kejuaraan karate. Antarmuka harus minim distraksi, cepat dimengerti dalam 1-2 detik, dan meminimalkan klik yang tidak perlu.
2. **High-Contrast Dark Mode**:
   Palet warna berbasis gelap pekat (*deep slate & charcoal*) untuk mengurangi kelelahan mata operator dan memastikan teks serta status partai terlihat jelas di berbagai kondisi pencahayaan GOR.
3. **Information Hierarchy with Dual Typography**:
   - Teks bacaan, label, dan instruksi menggunakan **Sans-Serif** yang bersih.
   - Angka partai, skor, waktu, nomor tatami, dan kode unik menggunakan **Monospace Tabular** agar sejajar sempurna dan mudah dipindai secara visual.
4. **Real-time First, Zero Manual Friction**:
   Data arena, skor, dan status tatami diperbarui secara otomatis di latar belakang tanpa menuntut operator mengklik tombol *refresh* berulang kali.

---

## 2. Palet Warna & Token Permukaan (Dark Theme Elevation)

### Tingkatan Permukaan (Surface Levels)
| Token | Kode Warna | Penggunaan |
|---|---|---|
| **Base App Background** | `#111216` / `#14161b` | Latar belakang root aplikasi dan workspace utama |
| **Card / Surface Container** | `#181a20` | Kotak kartu data, kontainer tabel, panel filter |
| **Header / Subsurface** | `#111215` | Bar header tabel (sticky), toolbar atas, panel judul |
| **Elevated / Modal Dialog** | `#13151b` / `#161820` | Modal pop-up, dropdown menu, kartu melayang |

### Batas & Pemisah (Borders & Dividers)
- **Border Default**: `border border-white/[0.08]`
- **Border Interaktif (Hover/Focus)**: `hover:border-white/20` atau `focus:border-blue-500/40`
- **Pemisah Baris Tabel**: `divide-y divide-white/[0.06]`
- **Backdrop Modal**: `bg-black/80 backdrop-blur-xs`

### Warna Semantik (Semantic Status)
| Status | Background Token | Teks Token | Border Token | Penggunaan |
|---|---|---|---|---|
| **Live / Aktif** | `bg-amber-500/10` | `text-amber-400` | `border-amber-500/20` | Partai sedang bertanding, timer aktif, sesi berjalan |
| **Ready / Online** | `bg-emerald-500/10` | `text-emerald-400` | `border-emerald-500/20` | Tatami standby, server aktif, sukses tersimpan |
| **Primer / Info** | `bg-blue-500/10` | `text-blue-400` | `border-blue-500/20` | Filter terpilih, rekomendasi WKF, link portal |
| **Bahaya / Destruktif** | `bg-rose-500/10` | `text-rose-400` | `border-rose-500/20` | Hapus tatami, diskualifikasi, reset database |
| **Netral / Inaktif** | `bg-white/[0.04]` | `text-zinc-400` | `border-white/[0.08]` | Badge sekunder, ID kode, status belum mulai |

---

## 3. Aturan Tipografi

| Hierarki | Kelas Tailwind | Karakteristik | Contoh Penggunaan |
|---|---|---|---|
| **Judul Halaman / Channel** | `text-base sm:text-lg font-black tracking-tight text-white font-sans uppercase` | Kuat, tegas, huruf kapital | "MANAJEMEN TATAMI", "DATA ATLET" |
| **Judul Seksi / Subheader** | `text-xs font-mono font-bold tracking-wider text-white uppercase` | Monospace terstruktur | "1. DAFTAR ARENA PERTANDINGAN" |
| **Teks Utama (Body)** | `text-xs font-medium text-zinc-300 font-sans` | Mudah dibaca, kontras pas | Deskripsi, pesan notifikasi |
| **Teks Pelengkap (Subtext)** | `text-[11px] text-zinc-500 font-sans` | Ringan, sekunder | Keterangan tambahan, catatan kaki |
| **Angka & Identitas** | `font-mono tabular-nums font-bold` | Lebar angka sama (*fixed width*) | Nomor tatami (`1..N`), Partai `#104`, Skor `3 - 1` |
| **Pill / Status Tag** | `text-[10px] font-mono font-bold uppercase tracking-wider` | Huruf kecil/padat dalam kotak badge | `LIVE`, `READY`, `ONLINE` |

---

## 4. Pola Komponen Standar

### A. Header Bar & Toolbar
- **Struktur Baris Atas**:
  - Sisi Kiri: Judul channel dengan `#` penanda channel dan deskripsi singkat.
  - Sisi Kanan: Tombol aksi utama (misal `+ Tambah Tatami`) dengan warna kontras putih `bg-white text-black font-bold hover:bg-zinc-200`.

### B. KPI Metrics (Kartu Statistik)
- Disajikan dalam grid responsif (umumnya 4 kolom seimbang).
- Tiap kartu memuat:
  1. Label atas: `text-[10px] font-bold text-zinc-400 uppercase tracking-wider`.
  2. Angka besar: `text-2xl font-black font-mono text-white tabular-nums`.
  3. Keterangan kecil di kanan bawah: `text-[10px] font-mono text-zinc-500`.

### C. Filter Pills (Filter Cepat)
- Tombol kapsul berbentuk pill yang mudah disentuh atau diklik:
  - **Aktif**: `bg-white text-black border-white shadow-sm font-semibold`
  - **Inaktif**: `bg-[#181a20] text-zinc-400 border-white/[0.08] hover:border-white/20 hover:text-white`
- Menampilkan hitungan dalam kurung, misal `Semua (5)`, `Live (2)`.

### D. Tabel Data Modern
- **Kontainer Luar**: `rounded-2xl border border-white/[0.08] bg-[#181a20] overflow-hidden shadow-lg`.
- **Header Kolom (`<thead>`)**:
  - `sticky top-0 z-10 bg-[#111215] text-zinc-400 font-mono text-[11px] uppercase tracking-wider border-b border-white/[0.08]`.
- **Baris Data (`<tbody>`)**:
  - Tinggi konsisten, `hover:bg-white/[0.02] transition-colors`.
  - Pemisah `divide-y divide-white/[0.06]`.
  - Kolom nomor urut rata tengah `font-mono text-zinc-500 text-[11px] tabular-nums`.
- **Kolom Aksi**:
  - Terletak paling kanan (`text-right pr-5`).
  - Menggunakan ikon aksi minimalis dengan efek hover yang lembut (`p-1.5 rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08]`).

### E. Dialog & Modal Pop-up
- Wajib menggunakan directive `use:portal` agar berada langsung di bawah `<body>`.
- Kelas layer: `fixed inset-0 z-[99999] bg-black/80 backdrop-blur-xs flex items-center justify-center p-4`.
- Kotak dialog: `bg-[#13151b] border border-white/[0.12] rounded-2xl shadow-2xl overflow-hidden`.
- Header modal memiliki tombol tutup `✕` di kanan atas dan dapat ditutup dengan mengklik backdrop luar atau menekan tombol `Batal`.

---

## 5. Pedoman Pemeliharaan Kode

1. **Hindari Fitur Berlapis yang Menumpuk**:
   Jika suatu fitur dapat diakses melalui link portal standar (seperti scoreboard via browser LAN), jangan memaksakan duplicate interface rumit di dalam tabel utama.
2. **Jangan Biarkan Dead Code**:
   Saat merombak komponen atau memisahkan modul, hapus seluruh state, fungsi pembantu, dan modal yang sudah tidak dipakai untuk menjaga kebersihan bundle aplikasi.
3. **Jaga Kompatibilitas Svelte 5 Runes**:
   Gunakan `$state(...)`, `$derived(...)` atau `$derived.by(...)`, serta hindari sintaks usang Svelte 4 (`$: reactive declarations`).
