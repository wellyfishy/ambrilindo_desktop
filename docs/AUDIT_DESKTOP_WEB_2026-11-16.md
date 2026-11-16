# Audit Ambrilindo Desktop + API Web (2026-11-16)

Audit sebelum rebuild. Kode desktop di commit `d8883db`, kode web di commit `71c516c3`.

**Cara verifikasi:**
- Baca kode: `sync.rs`, `db.rs` (save_match_result, drawing, live sync), `server.rs` (router LAN + WebSocket), `license.rs`, web `views/sync.py`, `views/api.py`, `sync_auth.py`.
- Tes otomatis: `cargo test --lib` (69 lulus, 0 gagal), `svelte-check` (0 error), web `manage.py check` (0 masalah).
- Skenario nyata pada **salinan** DB lewat server QA (`qa_serve_for_manual_testing`, port 8010). DB asli tidak disentuh. Hasilnya ditandai **TERBUKTI**.

Tingkat: KRITIS = hasil/medali salah atau data hilang · TINGGI = kemungkinan besar kena di event nyata · SEDANG · RENDAH.

---

## A. Logika turnamen (desktop)

> **Update 2026-11-16 (belum di-commit):** A1 dan A2 sudah diperbaiki.
> - **A1:** 4 pool → pemenang pool masuk semifinal bagan Final (A vs B, C vs D). Kalah babak 4 di pool tidak lagi dapat perunggu. Sinkron live mengikuti aturan yang sama. Helper yang dipakai: `pool_slot_of`, `pool_final_target`, `final_bagan_of`.
> - **A2:** drawing kategori >64 atlet ditolak sebelum ada bagan yang dihapus.
> - **Tes:** `qa_four_pool_*`, `qa_two_pool_*`, `qa_drawing_refuses_*`, `pool_slot_tests`.
> - **Belum diperbaiki:** B4 (force-sync) masih memakai aturan naif untuk pool. Dua kategori event 3 yang sudah dimainkan dengan logika lama (PEMULA KUMITE -40 KG PUTRA, PRA PEMULA KUMITE -35 KG PUTRA) tidak diperbaiki otomatis.

### A1. KRITIS, TERBUKTI: kategori 4 pool (33–64 atlet) salah total
`execute_drawing_bagan` (`db.rs:3590`) membuat Pool A–D + Final. Tapi `save_match_result` (`db.rs:4368`) mengenali bagan pool hanya dari `pool == 2` atau nama "POOL A/B/1/2":
- Pool A → final atlet1, Pool B → final atlet2. **Pemenang Pool C dan D tidak pernah masuk final.**
- Pool C/D dianggap bagan biasa: babak 4 mereka dianggap FINAL, jadi `juara_1/juara_2` di **semua bagan kategori** ditimpa (`UPDATE ... WHERE nomor_tanding_id = ?`). Kalau Pool C/D selesai setelah final, juara kategori jadi salah.
- Bagan Final tidak punya semifinal (A vs B, C vs D); hanya babak 4 urutan 1 yang dipakai.
- Frontend (`bracket_layout.ts:89`) menganggap babak 4 pool = "8 besar". Backend tidak.

Uji 40 atlet: final = juara Pool A vs juara Pool B. Juara Pool C (8483) dan Pool D (8498) tersingkir tanpa kalah. Pool C/D masing-masing menulis J3a/J3b sendiri.
DB nyata punya kategori seperti ini: event 4 "PRA PEMULA KUMITE -35 KG PUTRA" (49 atlet), event 3 (48), event 2 (36), dan 37 bagan dengan `pool=4`.
`live_next_target` (`db.rs:860`) punya aturan yang sama, jadi web juga salah.

### A2. KRITIS, TERBUKTI: kategori ≥65 atlet, 54+ atlet hilang tanpa peringatan
`perulangan` jatuh ke `else { 1 }` (`db.rs:3598`), lalu `build_bracket_in_memory` memotong ke 16 (`bracket.rs:432`). Uji 70 atlet: **16/70 masuk bagan**, tanpa pesan error.

### A3. TINGGI: pool dikenali dari nama bagan
`is_pool_bracket`, `is_final_bagan`, `is_repechage` dan `is_festival` diambil dari substring nama (`"FINAL"`, `"POOL A"`, `"FESTIVAL"`, `"REFERCHANGE"`). Contoh yang salah dibaca:
- nomor tanding yang berakhiran "FINAL";
- nama yang memuat "FESTIVAL";
- bagan yang di-rename.

Query bagan final `LIKE '%FINAL' LIMIT 1` tanpa `ORDER BY`. **Rebuild: simpan jenis bagan sebagai kolom (`kind`: single/pool/final/repechage/round_robin/festival + `pool_index`).**

### A4. SEDANG: babak 5 "Perebutan Juara 3" ditulis tapi tidak pernah dipakai
`insert_bracket_rows` selalu menulis baris babak 5. `save_match_result` tidak menangani babak 5, dan komentar di `live_next_target` bilang "tidak ada partai perunggu". Baris ini tetap dikirim ke web (push-bagan). Web `force-results` punya cabang `round == 5`. Bagan 16 slot tetap bikin 16 baris walau atletnya 3.

### A5. SEDANG: ID tatami vs nomor tatami tercampur
`lib.rs:724` menganggap `tatami >= 20` sebagai PK lalu mengonversinya. Path WebSocket bernama `tatami_pk` tapi dipakai sebagai nomor. Rebuild: satu identitas saja.

### A6. RENDAH: dispatcher LAN memakai default `event_id = 1`
Banyak handler memakai `get_i64_val(...).unwrap_or(1)` (`server.rs:2007, 2060, 2293`). Payload yang salah akan beroperasi di event #1, bukan ditolak.

## B. Kontrak sinkron desktop ↔ web

> **Hasil uji B1–B5 (2026-11-16).** Diuji dengan probe sementara (sudah dihapus) memakai salinan DB desktop dan web lokal ber-DB sementara. Production tidak disentuh.
> - **B1 TERBUKTI, lebih parah dari dugaan.** Web punya 2 regu × 3 anggota. Di desktop, kategori beregu jadi berisi 3 entri drawing: REGU X, REGU Y, dan satu baris "ANGGOTA Y2". Keenam anggota saling menimpa menjadi satu baris (lihat B2).
> - **B2 TERBUKTI, jauh lebih parah: semua atlet tanpa kode di satu kategori melebur jadi satu.** Web mengirim `kode_atlet: ""` untuk atlet tanpa kode global. Desktop menyimpan `""`, lalu mencari atlet berikutnya dengan `kode_atlet = ''` (`sync.rs:1003`), sehingga atlet berikutnya ketemu atlet pertama dan menimpanya. Uji: 5 atlet (ANDI ×2, CITRA, DEWI, EKA) → **1 baris** ("EKA"). Pesan sukses tetap "4 baru, 9 diperbarui". DB asli belum terkena (0 atlet dengan kode `''`, karena data lama dari app Django), tapi akan terkena pada "Tarik atlet" desktop pertama untuk atlet tanpa kode global.
> - **B3 TERBUKTI.** Lima kali dijawab 502 → status `abandoned`. Statistik antrean menunjukkan failed 0 dan pending 0 (tidak terlihat). Setelah web hidup lagi, "Coba lagi semua" mengirim 0 baris dan partai di web tetap belum selesai.
> - **B4 TERBUKTI, plus temuan baru B12.** Hasil "Kirim ulang semua hasil" untuk kategori 2 pool:
>   - Partai final di web jadi `selesai` tapi **tanpa atlet**, karena pemenang pool tidak pernah dimasukkan ke final.
>   - Juara 1/2 ada di bagan Final, Pool A dan Pool B sekaligus.
>   - Setelah "Kirim bagan", 3 dari 3 bagan punya juara 1.
> - **B12 (baru, TINGGI).** Peringkat publik web (`landing.py:120`) memberi medali untuk setiap bagan yang punya juara, termasuk bagan pool dan referchange. Satu kategori 2 pool tercatat 3 emas. Penyebabnya: desktop menyalin juara 1/2 ke semua bagan kategori, lalu "Kirim bagan" dan "Kirim ulang semua hasil" mengirimnya ke web.
> - **B1 + B2 DIPERBAIKI (belum di-commit).** Perubahan di `pull_athletes_from_hosted` (`sync.rs`):
>   - Kode atlet/NIK kosong dianggap "tidak ada" dan tidak pernah dipakai untuk mencocokkan.
>   - Anggota tim disimpan sebagai `MEMBER_B_<regu>_<id>`, format yang sama dengan app lama (contoh di DB asli: `MEMBER_B_4_1`).
>   - Baris anggota yang tersimpan dengan id polos dari pull desktop sebelumnya ikut diperbaiki. Baris anggota app lama dengan akhiran lain dipakai ulang (lewat nama + kontingen + tim yang sama), jadi tidak digandakan.
>   - Pertandingan beregu tetap menemukan 3 anggota per tim (diuji).
>   - Pencocokan nama hanya untuk atlet di kontingen yang sama dan yang belum tertaut ke web.
>   - Satu baris lokal tidak pernah diklaim dua kali dalam satu pull.
>   - Langkah 1–4 dijalankan dalam satu transaksi (ikut menutup sebagian B10).
>   - Tes: `web_e2e_e_pull_keeps_every_entry` (gagal di kode lama, lulus setelah perbaikan).
> - **B3 DIPERBAIKI (belum di-commit).**
>   - Jawaban 5xx/408/429 dari web diperlakukan seperti koneksi putus: baris tetap `failed`, antrean berhenti (FIFO), dan dicoba lagi tiap tick tanpa batas.
>   - Hanya 400/404 (dan 4xx lain setelah 5 kali) yang dilewati sebagai `abandoned`.
>   - Baris `abandoned` dihitung di "Gagal", tampil sebagai "Ditolak web" dengan tombol Kirim/Lewati, dan ikut dikirim oleh "Kirim ulang yang gagal".
>   - Tes: `web_e2e_f_web_errors_never_lose_results` (gagal di kode lama, lulus setelah perbaikan).
>   - Catatan data asli: event 8 punya 13 baris dan event 9 punya 3 baris `abandoned` (404 lama). Sekarang baris-baris itu terlihat di "Gagal" dan bisa dikirim ulang atau dilewati.
> - **B12 DIPERBAIKI di web (belum di-commit).**
>   - Fungsi baru `medal_bagans()` di `views/landing.py` hanya menghitung satu podium per kategori. Bagan referchange tidak dihitung, dan bagan pool tidak dihitung kalau kategorinya punya bagan Final (`pool=0` atau nama berakhiran "Final"). Festival dan bracket tunggal tetap dihitung.
>   - Dipakai di peringkat global, detail atlet, peringkat Kaltim dan detail kontingen.
>   - Tes: `backend/tests_medal_counting.py` (gagal tanpa penyaring). Suite web: 65 tes lulus.
>   - Di salinan DB web lokal tidak ada bagan yang berubah status (91 → 91). Dampak di production belum dicek.
> - **B5, B4, B13 DIPERBAIKI (kedua repo, belum di-commit). Web harus di-deploy sebelum exe desktop baru dibagikan.**
>   - **B5:**
>     - Endpoint web baru `api/reopen/` (`views/api.py` `receive_reopen`, `urls.py`) membuka ulang partai dan mengosongkan slot partai berikutnya, hanya kalau partai berikutnya belum selesai.
>     - Desktop (`save_match_result`) mengirimnya saat partai yang tadinya selesai dibuka ulang, ditambah podium terbaru kategori lewat `api/final-result/`.
>   - **B4:**
>     - Desktop `force_sync_results_to_hosted` memakai `live_next_target`, melewati partai anak beregu, dan mengirim hasil sesuai urutan main (pool → Final → referchange, per babak).
>     - Web `force_sync_results_api` tidak lagi menebak juara dari partai tanpa partai berikutnya, kecuali untuk kiriman yang tidak membawa `champions`.
>   - **B13 (baru):** partai anak beregu (babak 10, urutan 1–5 di setiap pertandingan beregu) ikut "Kirim bagan" dan saling menimpa di web. Sekarang `push_bagan_to_hosted` tidak mengirimnya.
>   - **Tes:** `web_e2e_g_reopen_reaches_the_web`, `web_e2e_g_force_results_for_pool_categories`, `web_e2e_g_team_child_bouts_not_pushed`. Ketiganya gagal tanpa perbaikan (web masih menampilkan partai selesai; final 4-pool di web tanpa atlet; 10 partai anak menyusut jadi 5 baris di web).
> - **B5 TERBUKTI untuk buka ulang; koreksi pemenang aman.** Mengganti pemenang (Aka → Ao) terkirim, dan web memindahkan atlet di partai berikutnya. Membuka ulang partai tidak mengirim apa pun: web tetap `selesai` dengan pemenang lama dan slot berikutnya tetap terisi.

### B1. TINGGI, regresi: tarik atlet beregu memasukkan anggota tim sebagai peserta
Web `export-atlet` mengirim tiap anggota tim dengan `is_beregu_member: true`. App Django lama memberi `additional_code = MEMBER_<beregu>_<detail>`. Desktop (`sync.rs:891`) **mengabaikan flag itu**, jadi anggota tim masuk sebagai atlet biasa di kategori beregu. Drawing, roster dan hitungan menyaring `MEMBER_%`, jadi anggota tim ikut di-drawing sebagai peserta.

### B2. TINGGI: tarik atlet menggabungkan dua atlet bernama sama
Pencarian terakhir di `sync.rs:1031` mencocokkan **nama saja** (tanpa utusan). Contoh: dua "BUDI" di kategori sama dari kontingen berbeda. Atlet kedua tidak ditemukan lewat nama+utusan, lalu ketemu atlet pertama lewat nama saja, dan atlet pertama **ditimpa** (utusan dan `additional_code` diganti). Satu atlet hilang.

### B3. TINGGI: 5xx sementara membuat hasil "abandoned" selamanya
`process_sync_queue_once` (`sync.rs:2323`) menandai `abandoned` setelah 5 retry untuk status non-401/403. Interval 12 detik, jadi web down/deploy sekitar 1 menit (502/503 dari proxy) sudah cukup untuk mengubur hasil. "Coba lagi semua" hanya mengambil `failed`, dan `get_queue_stats` tidak menghitung `abandoned`. Hasilnya tidak terlihat dan tidak dikirim ulang. Hasil yang tiba sebelum "Kirim bagan" (404) juga langsung abandoned.

### B4. TINGGI: `force-results` memakai aturan maju yang beda dari live
`force_sync_results_to_hosted` (`sync.rs:1651`) memakai `(urutan+1)/2` naif, bukan `live_next_target`. Untuk pool → final, repechage dan round robin, slotnya salah. Lalu web (`sync.py:785`) **mengangkat juara 1/2 dari partai apa saja yang tidak punya `next_kode_realtime`**. Contohnya partai repechage terakhir, round robin, semifinal pool, dan babak 4 Pool C/D. Akibatnya juara di web bisa salah sampai snapshot champions menimpanya, dan snapshot hanya menimpa field yang terisi. `next_vr1/2` dan `public_detail_id` juara juga tidak dikirim.

### B5. TINGGI: buka ulang / koreksi bagan tidak pernah dikirim live
Desktop hanya mengirim `status`, `result` (partai selesai), `final-result` dan `tatami-status`. Hal-hal berikut tidak pernah mengirim apa pun ke web:
- membuka ulang partai (`selesai=false`);
- edit atlet di bagan (`update_bagan_details`);
- generate ulang referchange;
- drawing ulang;
- tim beregu.

Endpoint web `api/edit-bagan/` tidak dipakai sama sekali. Web tetap menampilkan pemenang lama sampai operator menekan "Kirim bagan" manual.

### B6. SEDANG: push-bagan web mengubah data pendaftaran
`resolve_or_create_detail_atlet` (`sync.py:399, 420, 447`):
- memindahkan `DetailAtlet.nomor_tanding` atlet ke kategori bagan kalau berbeda (pendaftaran asli berubah);
- membuat `Atlet`/`DetailAtlet` baru dari nama saja (`nama_atlet__iexact`) tanpa utusan, sehingga dua atlet bernama sama tertukar;
- juara di-resolve tanpa utusan/perguruan.

### B7. SEDANG: full push menghapus semua bagan web yang tidak ada di payload
`is_full_sync` menghapus semua bagan event web lain. Kalau mapping event salah, bagan event web itu terhapus. Contoh mapping salah: fallback `../ambrilindo/ambrilindo/database/event_mapping.json` dengan ID dari DB Django lama, atau kunci lama `local_<id>` tanpa mode. Push sebagian (pilih bagan) setelah drawing ulang meninggalkan bagan lama (kode baru `max+1`), jadi kategori dobel di web. DetailBagan web yang sudah tidak ada di desktop tidak pernah dihapus.

### B8. SEDANG: `kode_realtime` bisa bentrok antar event
Formatnya `"{kode_event}-{bagan.id}-{detail.id}"`. Lookup di web hanya dibatasi oleh event yang diizinkan lisensi, **bukan oleh event yang dipetakan**. Dua DB desktop (dua laptop/venue) dengan `kode_event` sama, atau kosong / `EV<id>` lama, yang memakai lisensi yang sama bisa saling menimpa partai. Token legacy bisa menyentuh semua event. Rebuild: kirim `event_id` web di setiap payload live dan scope lookup ke sana.

### B9. RENDAH: field daftar event tidak cocok
Web `list_events_api` mengirim `tempat_event` dan `tanggal_event`. Desktop membaca `tanggal_mulai`, `tanggal_selesai`, `tempat` dan `status`, jadi pemilih event selalu tanpa tanggal/tempat.

### B10. RENDAH: tarik atlet
- Nomor tanding dicocokkan case-sensitive, sedangkan perguruan/utusan di-UPPER.
- Tidak transaksional: kalau gagal di tengah, data setengah masuk.
- Ekstensi logo dibaca dari akhir URL. URL S3 dengan `?X-Amz…` jadi `.webp`.
- Logo tidak pernah diperbarui kalau file sudah ada.
- `tipe` (kata/kumite) dari web tidak dipakai. Desktop menebak dari nama ("KATA"/"KUMITE").

### B11. RENDAH: `tatami-status` dengan `kode_realtime` yang tidak ditemukan membiarkan `detail_bagan` lama tetap terpasang di web.

## C. Keamanan & operasional LAN

> **Hasil uji C2/C3/C5 (2026-11-16), di-commit: desktop `acf1312`, web `b6a57d4d`.**
> - **C5 TERBUKTI, DIPERBAIKI.**
>   - Skor berjalan (poin, penalti, senshu, VR, timer, kata, juri) sekarang disimpan sebagai draft per partai di localStorage pada setiap perubahan (`src/lib/liveDraft.ts`).
>   - Draft dipulihkan saat panel partai itu dibuka lagi, dengan timer dijeda dan pemberitahuan "Skor dipulihkan…". Draft dihapus setelah `save_match_result` berhasil, atau kalau partai sudah selesai di DB.
>   - Diuji di browser: panel dimuat ulang, skor 3–1 dan timer 01:04 kembali. Ada juga vitest.
> - **C2 TERBUKTI.**
>   - Request LAN yang memegang kunci DB menahan request lain sepanjang durasinya: export Excel event 500 atlet sekitar 0,24 detik, probe IP sekitar 0,36 detik.
>   - Perintah tanpa DB (probe IP, lisensi, klaim tatami) sekarang dijawab tanpa mengambil kunci. Setelah perbaikan, request tablet tetap 16–25 ms selama probe IP berjalan.
>   - Kunci DB global itu sendiri masuk rencana rebuild.
> - **C3 SEBAGIAN.**
>   - Bug yang diperbaiki: lisensi `clock_tampered` aktif lagi dengan sendirinya setelah jam komputer dibenarkan (sebelumnya baru setelah restart), dan pesannya tidak lagi meminta internet.
>   - **Keputusan untuk user, tidak diubah:** saat lisensi kedaluwarsa di tengah event, panel yang sudah terbuka tetap jalan, tapi partai berikutnya tidak bisa dibuka (`expiry_locks_immediately` disengaja). Pilihannya: masa tenggang untuk Kata/Kumite, atau peringatan beberapa hari sebelum kedaluwarsa.

### C1. DITERIMA (keputusan user, 2026-11-16): server LAN tanpa autentikasi
> Wi-Fi venue tertutup: yang tersambung hanya laptop operator dan tablet wasit. Tidak diperbaiki sekarang. Pertimbangkan lagi kalau suatu saat jaringan venue dipakai bersama penonton atau panitia lain.

Server di `0.0.0.0:8000` dengan `CORS Any` (`server.rs:3489`). Semua `/api/*` tanpa auth, termasuk:
- `/api/bagan/delete-all`
- `/api/athletes/delete-all`
- `/api/match/save-result`
- `/api/command/:cmd`

WebSocket juga menerima `tatami_live_state` dari klien mana pun dan menyimpannya ke cache, jadi siapa pun di Wi-Fi venue bisa memalsukan skor di scoreboard. Karena CORS terbuka, halaman web jahat yang dibuka HP penonton di jaringan yang sama juga bisa memanggilnya. Rebuild: PIN/token pairing per perangkat + peran (host, operator, juri, tampilan).

### C2. SEDANG–TINGGI: satu mutex DB global untuk semua request LAN
Setiap handler memegang `state.db.lock()` sepanjang request (`server.rs:1764` dan seterusnya). `dispatch_command` bahkan memegangnya saat `check_ip_availability` melakukan probe jaringan. Export Excel, booklet atau drawing memblokir semua tablet skor. Rebuild: pool koneksi (sqlx sudah pool) tanpa mutex manager.

### C3. SEDANG: lisensi bisa mengunci scoring di tengah event
Kontrol panel butuh lisensi aktif (`lib.rs:717`). Tidak ada masa tenggang untuk kedaluwarsa di tengah event. `clock_tampered` (jam mundur, misalnya baterai CMOS atau ganti zona waktu) mengunci dan butuh internet untuk diperbaiki, padahal venue sering tanpa internet.

### C4. SEDANG: token legacy masih aktif secara default
Web `SYNC_ALLOW_LEGACY_TOKEN` default `true`, dan Bearer token bisa mengakses semua event (termasuk penghapusan via full push). Desktop release akan memakai token itu kalau ada `.env` di CWD, `../.env` atau `tournament_data/.env`. Matikan setelah audit log bersih.

### C5. SEDANG: skor yang sedang berjalan hanya ada di memori
Desktop menyimpan partai ke DB hanya saat selesai. Live state ada di jendela kontrol panel + cache server. Kalau app crash di tengah partai, skor berjalan hilang (perlu dicek apakah `snapshot_bout_panel` menutupi kasus ini: BELUM DIVERIFIKASI). Klien WebSocket yang lambat juga melewatkan pesan (`Lagged`) tanpa diminta resync.

### C6. Di luar desktop, tapi penting
Web `cari_atlet_global` (`api.py:30`) bisa dipakai pengguna login mana pun untuk mencari NIK atlet global dengan substring (`nik_atlet__icontains`), sehingga NIK semua atlet bisa dienumerasi. Repo web juga meng-commit `db.sqlite3` dan `__pycache__` (keduanya muncul sebagai modified di `git status`).

---

## Rekomendasi untuk rebuild (urut prioritas)
1. **Model bracket eksplisit:** tabel bagan dengan `kind`, `pool_index` dan `parent_final_id`, plus tabel "edge" (`match → next_match, slot`, `loser → repechage/bronze`). Advancement, live sync, force-sync dan web semuanya membaca edge yang sama. Ini menutup A1, A3, A4 dan B4.
2. **Ukuran bracket dinamis** (2^n, 32/64/128) atau pool + final berjenjang (semifinal antar pool). Tolak drawing kalau ada atlet yang tidak tertempatkan (A2).
3. **ID stabil lintas sistem:** UUID untuk event/bagan/partai/atlet, bukan `kode_realtime` gabungan. Payload membawa `web_event_id`.
4. **Sinkron berbasis event log:** kirim `match_updated` (termasuk dibuka ulang/dikoreksi) dan `bracket_replaced` dengan versi. Web menerapkan secara idempoten. Gagal sementara tidak pernah di-abandon, dan `abandoned` terlihat di UI dan bisa dikirim ulang.
5. **Tarik atlet:** pakai `public_detail_id` sebagai kunci tunggal, tangani `is_beregu_member` dan jangan pernah mencocokkan dari nama saja. Web push-bagan juga tidak boleh mengubah `DetailAtlet` pendaftaran.
6. **Keamanan LAN:** pairing perangkat + peran, CORS ke origin aplikasi saja, WebSocket ber-token.
7. **Persistensi skor berjalan** (snapshot tiap aksi) dan masa tenggang lisensi offline.

## Tidak dicakup / belum diverifikasi
- UI Svelte per komponen hanya dicek lewat `svelte-check`, tidak diuji klik.
- Festival, round robin, BOB, tim beregu dan alur approval wasit hanya dibaca sekilas. Tes QA yang ada lulus.
- Referchange untuk kategori pool (laporan user sebelumnya) tidak diuji ulang. Dengan bug A1, referchange 4 pool hampir pasti ikut salah.
- `web_e2e` (11 tes, butuh web lokal) tidak dijalankan di sesi ini.
