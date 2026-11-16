# Langkah berikutnya: Ambrilindo desktop + web (per 16 Nov 2026)

Lanjutan dari audit (`docs/AUDIT_DESKTOP_WEB_2026-11-16.md`, satu folder dengan file ini). Urutannya: **Wajib** dulu, lalu **Keputusan**, lalu **Backlog**.

**Posisi terakhir di `main`:**

| Repo | Commit | Isi |
|---|---|---|
| `ambrilindo_web_pub` | `b6a57d4d` | Peringkat 1 podium per kategori, endpoint `api/reopen/`, `force-results` tidak menebak juara |
| `ambrilindo_desktop` | `acf1312` | Pool 4, batas 64 atlet, tarik atlet, antrean sinkron, buka ulang ke web, kirim ulang hasil, draft skor, dispatcher LAN, jam lisensi |
| `ambrilindo_desktop` | (commit ini) | Dokumen ini + salinan laporan audit |

**Sudah di `main` (17 Nov):** semua perbaikan di bagian "Selesai" di bawah ada di `main` kedua repo (fast-forward dari branch `claude/stoic-keller-xehhxw`). Cukup `git pull` di `main`. Urutan tetap: **deploy web dulu** (1.1), baru build dan bagikan exe (1.2).

---

## Selesai (sesi lanjutan 16 Nov, bagian 2): #10, A5/A6, B10, B11, `__pycache__`

| | Perbaikan | Tes |
|---|---|---|
| **#10 podium** | Desktop: `save_match_result` membandingkan podium kategori sebelum dan sesudah simpan; setiap partai selesai yang mengubahnya (perunggu dari semifinal, babak 4 pool, referchange, round robin, atau koreksi hasil) mengirim `api/final-result/`. Web: notifikasi "Bagan Selesai - Juara" hanya muncul kalau sudah ada Juara 1; tabel podium tetap diperbarui. | `qa_podium_sent_when_it_changes`, e2e `web_e2e_h_referchange_bronze_reaches_the_web` |
| **A5/A6** | Server LAN: 45 perintah yang dulu memakai event 1 kalau `eventId` kosong sekarang menjawab 400 (`required_event_id`), juga `dayId`/`tatamiId` di dua perintah. Semua 73 pemanggil di frontend sudah mengirim `eventId`. `open_control_panel_window` tidak lagi menebak "tatami ≥ 20 = ID database"; `BaganView` mencari tatami hanya lewat ID. | `qa_lan_commands_need_an_event_id` |
| **B10** | Tarik atlet: kategori dicocokkan tanpa peduli huruf besar/kecil dan spasi (dulu membuat kategori kembar). Logo kontingen diunduh ulang setiap tarik (file ditulis hanya kalau berubah), ekstensi diambil dari path URL tanpa `?query`. | e2e `web_e2e_e_pull_matches_category_case_insensitively`, unit `logo_extension_ignores_query` |
| **B11** | Web: status tatami untuk partai yang tidak dikenal menghapus tautan partai lama. | web `B11TatamiStatusUnknownBout` |
| **1.4** | `__pycache__` tidak lagi dilacak git di repo web. Saat `git pull`, file `.pyc` lama terhapus dari folder; Python membuatnya lagi sendiri. | |

**Celah yang diketahui (belum diubah):** "Generate Referchange" mengosongkan 3A/3B di desktop, tapi web tetap menampilkan yang kalah di semifinal sampai final referchange selesai (sama seperti sebelumnya).

**Deploy:** tetap web dulu. Exe baru mengirim podium lebih sering; di web lama notifikasi "Bagan Selesai" akan muncul setelah semifinal (hanya kosmetik).

---

## Selesai (sesi lanjutan 16 Nov): B6, B7, B8, B9

Semua dengan tes yang **gagal di kode lama dan lulus setelah perbaikan** (sudah dibuktikan dua arah: desktop baru vs web lama, dan desktop lama vs web baru).

| | Perbaikan | Tes |
|---|---|---|
| **B6** | Web: satu fungsi `resolve_detail_atlet()` (`views/sync.py`) dipakai push-bagan, force-results, dan final-result. Tidak pernah lagi mengubah `nomor_tanding` DetailAtlet yang sudah ada: atlet dari kategori lain mendapat DetailAtlet tambahan. Nama kembar dibedakan dengan kontingen (utusan, lalu perguruan). Desktop: juara dikirim lengkap dengan `_utusan`, `_perguruan`, `_public_detail_id` (`DatabaseManager::podium_fields`). | web `tests_sync_b6_b8.py` (B6…) |
| **B7** | Web: full push hanya menghapus bagan yang punya `kode` (kiriman desktop), bagan buatan web aman. Push sebagian menghapus bagan kategori yang dikirim yang `kode`-nya tidak ada lagi di `known_kodes` (daftar kode bagan desktop saat ini). Desktop: kirim `known_kodes`; fallback `../ambrilindo/.../event_mapping.json` dan kunci lama `local_<id>` dihapus. | web `B7…`, e2e `web_e2e_h_push_prunes_only_desktop_brackets`, unit `mapping_ignores_legacy_keys` |
| **B8** | Desktop: semua pesan live (`status`, `result`, `reopen`, `final-result`, `tatami-status`) diberi `event_id` web saat dikirim (baris lama di antrean juga). Web: `_scoped()` di `views/api.py` memfilter partai per event bila `event_id` ada; `event_id` bukan angka → 400; event tanpa izin → 403. | web `B8…`, e2e `web_e2e_h_live_result_stays_in_its_event`, unit `live_messages_get_the_web_event_id` |
| **B9** | Desktop membaca `tanggal_event`/`tempat_event` (nama lama tetap dibaca), dan pilihan event web di Sinkronisasi sekarang menampilkan tempat · tanggal. | unit `hosted_events_read_web_field_names` |

**Kompatibilitas:** web baru tetap menerima exe lama (tanpa `event_id` / `known_kodes` = perilaku lama, kecuali bagan buatan web kini tidak ikut terhapus). Urutan deploy tetap: **web dulu**, baru bagikan exe.

**Catatan:**
- Bagan dengan nama + kategori sama memang ada di data asli (event 8: dua bagan "KUMITE PEMULA PUTRI -42 KG" yang sama-sama sudah dimainkan). Karena itu pembersihan push sebagian memakai `known_kodes`, bukan nama.
- Drawing ulang manual memakai kode yang sama bila kategori itu memegang kode tertinggi; web lalu cukup memperbarui bagan yang sama.
- `test_support.rs`: tes e2e sekarang juga jalan di Linux/macOS (python venv tidak lagi di-`canonicalize`, yang membuang venv).

---

## 0. Siapkan laptop

```bash
git clone https://github.com/wellyfishy/ambrilindo_desktop.git
git clone https://github.com/wellyfishy/ambrilindo_web_pub.git
```
Kalau repo sudah ada, cukup `git pull` di masing-masing.

Susun folder supaya tes e2e menemukan web secara otomatis:
```
<workspace>/
  ambrilindo_desktop/
  ambrilindo_web/
    ambrilindo_web_pub/   <- repo web
    venv/                 <- python -m venv venv; pip install -r ambrilindo_web_pub/requirements.txt
```
Kalau susunannya berbeda, set `AMBRILINDO_WEB_DIR` ke folder `ambrilindo_web_pub`.

Desktop:
```bash
cd ambrilindo_desktop
npm install
```

Data turnamen (`tournament_data/db.sqlite3`) ikut di repo desktop. File `.env` tidak ada di git, jadi salin manual dari PC lama kalau dibutuhkan (`HOSTED_API_TOKEN` hanya untuk mode token lama). File lisensi tidak ikut: aktivasi ulang lisensi di laptop lewat menu **Lisensi** (butuh internet; satu lisensi punya batas jumlah perangkat).

**Cek bahwa semuanya hijau sebelum mulai mengubah apa pun:**
```bash
cd ambrilindo_desktop/src-tauri
cargo test --lib                                          # 81 lulus
cargo test --lib web_e2e -- --ignored --test-threads=1    # 20 lulus (butuh repo web + venv)
cd ..
npx vitest run                                            # 22 lulus
npx svelte-check                                          # 0 error
```
```bash
cd ambrilindo_web/ambrilindo_web_pub
# JANGAN pakai db.sqlite3 asli untuk tes:
SQLITE_PATH=/tmp/web_test.sqlite3 DEBUG=true ../venv/Scripts/python manage.py test backend   # 84 lulus
../venv/Scripts/python manage.py check
```

---

## 1. WAJIB sebelum event berikutnya

### 1.1 Deploy web DULU
> Deploy `main` web terbaru (berisi `8c2f612b` … `792c23a6`), bukan `b6a57d4d`.
Push ke `main` **tidak** otomatis deploy (repo web tidak punya CI). Desktop baru memanggil `api/reopen/`, yang baru ada di web `b6a57d4d`.
- **Fly.io:** dari `ambrilindo_web_pub`, jalankan `fly deploy`. Container menjalankan `migrate` sendiri, dan tidak ada migrasi baru.
- **VPS Nevacloud:** ikuti `deploy/MIGRASI_NEVACLOUD.md` di repo web (kirim kode → `docker compose up -d --build`).
- **Setelah deploy**, buka halaman peringkat ambrilindo.com. Kategori pool sekarang hanya menyumbang 1 emas, jadi jumlah medali beberapa atlet/kontingen bisa turun. Itu memang koreksi yang diharapkan.

### 1.2 Build ulang exe desktop
Tutup aplikasi Ambrilindo dulu; kalau masih jalan, build gagal dengan "Access is denied".
```bash
cd ambrilindo_desktop
npm run tauri build
```
**Setelah exe baru terpasang:** buka Sinkronisasi untuk setiap event yang aktif dan pastikan event web-nya masih tampil. Exe baru tidak lagi membaca mapping lama (file app lama dan kunci `local_<id>` tanpa mode). Event yang hanya terhubung lewat mapping lama akan tampil belum terhubung: hubungkan ulang. (File `tournament_data/event_mapping.json` di repo sudah memakai format baru, tapi file di PC event bisa berbeda.)

Event yang atletnya dibuat lokal (bukan hasil Tarik atlet, misalnya event 1–4, 12, 13, 16, 17) sekarang dicocokkan di web dengan nama **dan** kontingen. Kalau nama kontingen di desktop dan di web ditulis berbeda, push membuat atlet baru di web, bukan memakai yang lama. Periksa sekali setelah push pertama event seperti ini.

Bagikan exe hanya setelah 1.1 selesai. Kalau desktop baru bertemu web lama, pesan buka ulang muncul sebagai "Ditolak web" dan bisa dikirim ulang setelah web diperbarui.

### 1.3 Uji coba sebelum event (pakai event uji, bukan event asli)
Centang satu per satu:

- [ ] **Pool 4:** buat kategori uji 40 atlet → Drawing → mainkan Pool A–D sampai babak 4.
  - Bagan Final berisi Semi 1 = juara A vs juara B, Semi 2 = juara C vs juara D.
  - Mainkan semi dan final: Juara 1/2 dari final, 3A/3B dari yang kalah di semi.
  - Generate Referchange dari bagan Final, mainkan, lalu pastikan 3A/3B berasal dari referchange.
- [ ] **>64 atlet:** drawing kategori 70 atlet → ditolak dengan pesan "maksimal 64 atlet…", dan bagan lama tetap utuh.
- [ ] **Draft skor:**
  - buka kontrol panel, beri skor dan penalti, jalankan timer;
  - tutup paksa jendelanya, lalu buka lagi partai yang sama → muncul "Skor dipulihkan…", skor kembali, timer dijeda;
  - simpan → buka lagi → tidak ada pemberitahuan.
- [ ] **Buka ulang → web** (live sync aktif, bagan sudah dikirim):
  - selesaikan partai → di web tercatat selesai;
  - buka ulang di desktop → di web kembali belum selesai, dan pemenang hilang dari partai berikutnya;
  - buka ulang final → juara di web kosong; simpan lagi → juara kembali.
- [ ] **Kirim ulang semua hasil:** mainkan kategori pool dengan live sync mati → "Kirim ulang semua hasil" → di web, final/semi berisi atlet yang benar dan tidak ada partai "selesai" tanpa atlet.
- [ ] **Tarik atlet:** event web dengan atlet tanpa kode atlet dan nama kembar dari kontingen berbeda → semua masuk sebagai baris terpisah. Kategori beregu → 1 entri drawing per regu; anggota tampil sebagai kandidat saat pertandingan beregu.
- [ ] **Antrean sinkron:** matikan web (atau putuskan internet) selama 1–2 menit di tengah pertandingan → hasil tetap "Gagal"/"Tertunda", tidak hilang → web hidup lagi → terkirim sendiri.
- [ ] **Hal yang belum pernah dites di app asli** (dari catatan sebelumnya): "Hentikan" tatami dengan laptop operator, host LAN mati (operator otomatis keluar), Simpan PDF dari Cetak, Kirim roster ke web production.

### 1.4 Bersihkan data lama
- **Event 2 "SATRIA WADOKAI CUP I (Testing 2)":** 18 bagan Final memakai format lama (hanya 3 baris, tanpa babak 3/4), jadi pemenang pool tidak bisa masuk. Drawing ulang kategori pool-nya sebelum dipakai, atau abaikan kalau event ini memang hanya untuk uji.
- **Event 8 (SELEKPROV INKANAS) dan 9:** total 16 baris lama "Ditolak web" (error 404 lama) sekarang tampil di "Gagal". Kalau event sudah lewat, klik **Lewati**.
- ✅ SELESAI: **Repo web:** `__pycache__/*.pyc` ikut tercatat di git. Sebaiknya keluarkan dari git (`git rm -r --cached '**/__pycache__'`, lalu tambahkan ke `.gitignore`) supaya tidak terus muncul sebagai perubahan.

---

## 2. Keputusan yang menunggu kamu

| # | Keputusan | Pilihan | Catatan |
|---|---|---|---|
| K1 | Lisensi kedaluwarsa di tengah event | a) tetap kunci (sekarang) b) masa tenggang 48 jam khusus Kata/Kumite c) peringatan H-7 / H-3 di aplikasi | Sekarang: panel yang sudah terbuka tetap jalan, partai berikutnya tidak bisa dibuka. Ini disengaja (`license.rs` tes `expiry_locks_immediately`). Opsi b/c cukup diubah di `license::check()`; Drawing dan sinkron tetap ketat. |
| K2 | Target hosting web | Fly.io atau VPS Nevacloud | Menentukan cara deploy di 1.1. Pilih satu supaya tidak ada dua versi live. |
| K3 | Token sinkron lama | Matikan `SYNC_ALLOW_LEGACY_TOKEN` di web | Cek dulu di admin web (Lisensi Desktop / audit log) bahwa tidak ada lagi panggilan "legacy". Token lama bisa mengakses semua event, termasuk menghapus bagan lewat full push. |
| K4 | Ditolak web (404) | Biarkan manual (sekarang) atau kirim ulang otomatis setelah "Kirim bagan" | Hasil yang tiba sebelum bagan dikirim akan 404. Sekarang perlu klik "Kirim ulang yang gagal". |

---

## 3. Backlog perbaikan (belum dikerjakan)

Detail tiap temuan ada di laporan audit. Semua butuh tes yang **gagal di kode lama dan lulus setelah perbaikan**, mengikuti pola tes `web_e2e` E/F/G dan `qa_tests.rs`.

### Prioritas TINGGI
1. **✅ SELESAI: B6: "Kirim bagan" mengubah data pendaftaran di web.**
   - **Di mana:** web `backend/views/sync.py` → `resolve_or_create_detail_atlet()` di dalam `receive_bagan_sync`.
   - **Masalah:**
     - Kalau atlet di bagan desktop berada di kategori lain, `DetailAtlet.nomor_tanding` dipindah sehingga pendaftaran aslinya berubah.
     - Atlet/DetailAtlet baru dibuat dari nama saja tanpa kontingen, sehingga dua atlet bernama sama bisa tertukar.
   - **Arah perbaikan:**
     - Jangan pernah mengubah `nomor_tanding` DetailAtlet yang sudah ada.
     - Cocokkan dengan urutan `public_detail_id` → kode atlet → nama + kontingen.
     - Juara di `force_sync_results_api` dan `receive_final_result` juga di-resolve dengan kontingen (desktop perlu mengirim `*_public_detail_id` dan kontingen juara).
   - **Tes:** e2e dengan atlet terdaftar Kata yang muncul di bagan Kumite → pendaftaran Kata tetap utuh.
2. **✅ SELESAI: B7: full push menghapus semua bagan web yang tidak ada di kiriman.**
   - **Di mana:** web `sync.py` (`is_full_sync` → `stale_bagans.delete()`), dan desktop `sync.rs` `get_event_mapping()`, yang masih membaca fallback `../ambrilindo/ambrilindo/database/event_mapping.json` dan kunci lama `local_<id>`.
   - **Arah perbaikan:**
     - Hapus fallback mapping lama di desktop.
     - Di web, hapus hanya bagan yang pernah dikirim desktop (`kode` tidak kosong / punya `kode_realtime`), bukan bagan buatan web.
     - Push sebagian setelah drawing ulang: hapus bagan web dengan kategori yang sama tapi `kode` lama.
3. **✅ SELESAI: B8: `kode_realtime` bisa bentrok antar event.**
   - **Di mana:** web `views/api.py` (`receive_status`, `receive_result`, `receive_reopen`, `receive_final_result`) mencari partai hanya dengan `kode_realtime`, dibatasi event lisensi.
   - **Arah perbaikan:**
     - Desktop mengirim `event_id` web di setiap payload live (isi dari mapping, seperti yang sudah dilakukan untuk `tatami-status` di worker).
     - Web memfilter `bagan__event_id=event_id`.
   - **Tes:** dua event web dengan kode partai yang sama.

### Prioritas SEDANG
4. **A3: jenis bagan masih ditebak dari nama** ("POOL A", "FINAL", "FESTIVAL", "REFERCHANGE"), baik di desktop (`db.rs` `pool_slot_of`, `save_match_result`, `bracket_layout.ts`) maupun di web. Bagan yang di-rename bisa salah dibaca. Saran untuk rebuild: kolom `kind` + `pool_index` pada bagan.
5. **A4: baris babak 5 ("Perebutan Juara 3")** selalu ditulis tapi tidak pernah dipakai, dan bagan 16 slot selalu dibuat 16 baris walau atletnya 3. Bisa dibersihkan saat rebuild.
6. **✅ SELESAI: A5/A6: input LAN yang ambigu.**
   - `lib.rs` `open_control_panel_window` menganggap `tatami >= 20` sebagai ID database.
   - Dispatcher di `server.rs` memakai `unwrap_or(1)` untuk `eventId` yang hilang.
   - **Arah perbaikan:** tolak payload yang tidak lengkap (400), jangan diam-diam memakai event 1.
7. **✅ SELESAI: B9: daftar event web tanpa tanggal/tempat di desktop.** Web `list_events_api` mengirim `tempat_event`/`tanggal_event`, sedangkan desktop `fetch_hosted_events()` membaca `tanggal_mulai`/`tempat`. Samakan nama field-nya (cukup ubah sisi desktop).
8. **✅ SELESAI: B10 (sisa): tarik atlet.**
   - Nomor tanding dicocokkan case-sensitive.
   - Logo kontingen tidak diperbarui kalau file sudah ada, dan ekstensinya salah untuk URL S3 yang memakai `?query`.
9. **✅ SELESAI: B11: status tatami web.** `update_tatami_status_api` membiarkan `detail_bagan` lama kalau `kode_realtime` tidak ditemukan.
10. **✅ SELESAI: Koreksi semifinal (tanpa buka ulang).** Mengganti pemenang semifinal mengubah 3A/3B di desktop, tapi podium tidak dikirim ulang ke web (hanya final yang mengirim `final-result`). Kirim podium juga untuk koreksi di babak 3 dan babak 4 pool.

### Prioritas RENDAH / rebuild
11. **C2: kunci DB global di server LAN** (`server.rs`, setiap handler memakai `state.db.lock()`). Dampak terukur 0,1–0,25 detik per request berat. Untuk rebuild: pakai pool koneksi sqlx langsung, tanpa mutex `DatabaseManager`.
12. **C1: server LAN tanpa auth.** Risiko diterima (Wi-Fi venue tertutup). Pertimbangkan lagi kalau jaringan venue dipakai bersama.
13. **C6 (web): pencarian NIK atlet.** `cari_atlet_global` (`views/api.py`) bisa dipakai pengguna login mana pun untuk mencari NIK dengan substring. Batasi ke admin, atau ke pencocokan NIK yang persis sama.

---

## 4. Rekomendasi rebuild (ringkas dari audit)
1. Model bracket eksplisit: jenis bagan sebagai kolom, dan tabel "edge" (partai → partai berikutnya + slot; yang kalah → repechage). Satu sumber aturan untuk simpan hasil, sinkron live, kirim ulang hasil, dan web.
2. Ukuran bracket dinamis, atau pool + final berjenjang, sehingga lebih dari 64 atlet tidak perlu ditolak.
3. ID stabil lintas sistem (UUID), bukan `kode_realtime` gabungan. Setiap payload membawa `event_id` web.
4. Sinkron berbasis event log yang memakai versi; koreksi dan buka ulang juga lewat jalur yang sama.
5. Skor berjalan disimpan di DB host, bukan hanya di draft lokal per laptop.
6. Server LAN tanpa mutex global; kalau suatu saat dibutuhkan, pairing perangkat + peran.

---

## 5. Catatan teknis yang berguna
- **UI tanpa Tauri:**
  - jalankan `cargo test --lib qa_serve_for_manual_testing -- --ignored --nocapture` (salinan DB di port 8010), lalu `npm run dev` (1420);
  - buat route sementara yang memanggil `setConnectionConfig("client", "http://127.0.0.1:8010")`. Channel yang hanya ada di host (misalnya Sinkronisasi web) butuh stub `window.__TAURI_INTERNALS__`;
  - hapus route sementara setelah selesai.
- **Hentikan server QA sebelum `cargo test`.** Windows mengunci exe tes, sehingga build gagal.
- **Hati-hati saat menukar file sementara** untuk membuktikan tes gagal di kode lama: mengembalikan file dengan `cp` bisa membawa mtime lama, sehingga cargo tidak build ulang. Jalankan `touch` pada file itu.
- **Aturan repo:** `AGENTS.md` di workspace dan di desktop (pola UI, kontrak sinkron). Perubahan endpoint sinkron atau model web perlu rencana dulu.
