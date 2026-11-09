"""Tambah volume besar ke event dummy (kode_event "DUMMY-UJI") supaya mirip turnamen nyata.

Pemakaian (dari folder ambrilindo_desktop), setelah event dummy dibuat:
    python tournament_data/seed_dummy_lengkap.py --replace   # (opsional) buat ulang event dummy dasar
    python tournament_data/seed_dummy_besar.py                # tambah ~320 atlet
    python tournament_data/seed_dummy_besar.py --db path.sqlite3

Yang ditambahkan (event lain tidak disentuh):
- 10 kontingen dan 6 perguruan baru (total 14 kontingen berisi atlet), tatami 3 dan 4.
- 12 kategori turnamen perorangan, 2 kategori beregu, 9 kategori festival.
  Semuanya kategori BARU: kategori dan kasus uji dari seed_dummy_lengkap.py tidak diubah.
- Sebaran kontingen tidak rata (ada kontingen besar), jadi anti-bentrok ikut teruji.

Skrip menolak jalan dua kali: bila kontingen penanda sudah ada, tidak ada yang ditambahkan.
"""

import os
import random
import sqlite3
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
from seed_dummy_lengkap import EVENT_CODE, NameGen, gender_of  # noqa: E402

DB_PATH = os.path.join(HERE, "db.sqlite3")
MARKER_UTUSAN = "KAB. PASER"

# (nama, bobot): bobot besar = kontingen besar
UTUSAN_BARU = [
    ("KAB. PASER", 3),
    ("KAB. KUTAI TIMUR", 5),
    ("KAB. KUTAI BARAT", 2),
    ("KAB. PENAJAM PASER UTARA", 3),
    ("KAB. MAHAKAM ULU", 1),
    ("KOTA TARAKAN", 4),
    ("KAB. BULUNGAN", 2),
    ("KAB. NUNUKAN", 2),
    ("KAB. MALINAU", 1),
    ("KAB. TANA TIDUNG", 1),
]
# Bobot kontingen lama (dari seed dasar); BERAU tetap tanpa atlet.
BOBOT_LAMA = {
    "KOTA SAMARINDA": 8,
    "KOTA BALIKPAPAN": 7,
    "KAB. KUTAI KARTANEGARA": 5,
    "KOTA BONTANG": 4,
}
PERGURUAN_BARU = ["KKI", "GOJUKAI", "SHOKAIDO", "INKADO", "BKC", "TAKO"]

# (nama kategori, jumlah atlet, has_vr)
TOURNAMENT = [
    ("TOURNAMENT SENIOR - KUMITE -60 KG PUTRA", 16, 1),
    ("TOURNAMENT SENIOR - KUMITE -67 KG PUTRA", 18, 1),
    ("TOURNAMENT SENIOR - KUMITE -75 KG PUTRA", 10, 1),
    ("TOURNAMENT SENIOR - KUMITE -55 KG PUTRI", 12, 1),
    ("TOURNAMENT SENIOR - KUMITE -61 KG PUTRI", 9, 1),
    ("TOURNAMENT JUNIOR - KATA PERORANGAN PUTRI", 20, 0),
    ("TOURNAMENT JUNIOR - KUMITE -55 KG PUTRA", 22, 1),
    ("TOURNAMENT JUNIOR - KUMITE -48 KG PUTRI", 13, 1),
    ("TOURNAMENT KADET - KATA PERORANGAN PUTRA", 24, 0),
    ("TOURNAMENT KADET - KUMITE -52 KG PUTRA", 17, 1),
    ("TOURNAMENT KADET - KUMITE -47 KG PUTRI", 11, 1),
    ("TOURNAMENT UNDER 21 - KUMITE -67 KG PUTRA", 8, 1),
]
# (nama kategori, jumlah tim, anggota per tim, has_vr)
TEAMS = [
    ("TOURNAMENT JUNIOR - KATA BEREGU PUTRA", 6, 3, 0),
    ("TOURNAMENT SENIOR - KUMITE BEREGU PUTRI", 4, 3, 1),
]
FESTIVAL = [
    ("FESTIVAL PRA USIA DINI - KUMITE -25 KG PUTRA", 14),
    ("FESTIVAL PRA USIA DINI - KUMITE -25 KG PUTRI", 9),
    ("FESTIVAL USIA DINI - KUMITE -30 KG PUTRA", 18),
    ("FESTIVAL USIA DINI - KUMITE -30 KG PUTRI", 11),
    ("FESTIVAL PRA PEMULA - KUMITE -35 KG PUTRA", 16),
    ("FESTIVAL PRA PEMULA - KUMITE -35 KG PUTRI", 1),  # solo: cocok di-mix ke -30 KG PUTRI
    ("FESTIVAL PEMULA - KATA PERORANGAN PUTRI", 15),
    ("FESTIVAL PEMULA - KUMITE -40 KG PUTRI", 7),
    ("FESTIVAL PEMULA - KUMITE -45 KG PUTRA", 13),
]


def main():
    db_path = sys.argv[sys.argv.index("--db") + 1] if "--db" in sys.argv else DB_PATH
    rng = random.Random(2026)
    conn = sqlite3.connect(db_path, timeout=20)
    conn.isolation_level = None
    cur = conn.cursor()
    cur.execute("PRAGMA busy_timeout = 20000")

    row = cur.execute("SELECT id FROM backend_event WHERE kode_event = ?", (EVENT_CODE,)).fetchone()
    if not row:
        print("Event dummy belum ada. Jalankan dulu: python tournament_data/seed_dummy_lengkap.py")
        return 1
    event_id = row[0]
    if cur.execute(
        "SELECT 1 FROM backend_utusan WHERE event_id = ? AND nama_utusan = ?", (event_id, MARKER_UTUSAN)
    ).fetchone():
        print(f"Volume besar sudah pernah ditambahkan ke event {event_id}. Tidak ada yang diubah.")
        return 1

    cur.execute("BEGIN IMMEDIATE")
    try:
        # Kontingen: lama (berbobot) + baru
        utusan_pool = []  # (id, bobot)
        for uid, nama in cur.execute(
            "SELECT id, nama_utusan FROM backend_utusan WHERE event_id = ?", (event_id,)
        ).fetchall():
            if nama in BOBOT_LAMA:
                utusan_pool.append((uid, BOBOT_LAMA[nama]))
        for nama, bobot in UTUSAN_BARU:
            cur.execute("INSERT INTO backend_utusan (nama_utusan, event_id, logo) VALUES (?, ?, NULL)", (nama, event_id))
            utusan_pool.append((cur.lastrowid, bobot))

        perguruan_ids = [
            pid for (pid, nama) in cur.execute(
                "SELECT id, nama_perguruan FROM backend_perguruan WHERE event_id = ?", (event_id,)
            ).fetchall()
            if "TANPA ATLET" not in nama
        ]
        for nama in PERGURUAN_BARU:
            cur.execute("INSERT INTO backend_perguruan (nama_perguruan, event_id) VALUES (?, ?)", (nama, event_id))
            perguruan_ids.append(cur.lastrowid)

        existing_tatami = {
            n for (n,) in cur.execute("SELECT tatami_number FROM backend_tatami WHERE event_id = ?", (event_id,))
        }
        for t in (3, 4):
            if t not in existing_tatami:
                cur.execute(
                    "INSERT INTO backend_tatami (tatami_number, event_id, detail_bagan_id) VALUES (?, ?, NULL)",
                    (t, event_id),
                )

        # Nama unik terhadap atlet yang sudah ada di event
        names = NameGen()
        names.used.update(
            n.upper() for (n,) in cur.execute("SELECT nama_atlet FROM backend_atlet WHERE event_id = ?", (event_id,))
        )
        # Tiap kontingen cenderung memakai 1-2 perguruan tetap (lebih realistis)
        club_of = {uid: rng.sample(perguruan_ids, 2) for uid, _ in utusan_pool}
        u_ids = [u for u, _ in utusan_pool]
        u_w = [w for _, w in utusan_pool]

        def pick_utusan():
            return rng.choices(u_ids, weights=u_w)[0]

        def add_category(name, has_vr):
            cur.execute(
                "INSERT INTO backend_nomortanding (nama_nomor_tanding, event_id, is_bob, has_vr) VALUES (?, ?, 0, ?)",
                (name, event_id, has_vr),
            )
            return cur.lastrowid

        def add_atlet(cat_id, nama, utusan_id, perguruan_id, additional_code=None):
            cur.execute(
                "INSERT INTO backend_atlet (nama_atlet, event_id, nomor_tanding_id, perguruan_id, utusan_id, nik, kode_atlet, additional_code, is_priority) VALUES (?, ?, ?, ?, ?, NULL, NULL, ?, 0)",
                (nama, event_id, cat_id, perguruan_id, utusan_id, additional_code),
            )

        totals = {"turnamen": 0, "beregu": 0, "festival": 0}
        for name, count, has_vr in TOURNAMENT:
            cat_id = add_category(name, has_vr)
            g = gender_of(name)
            for _ in range(count):
                u = pick_utusan()
                add_atlet(cat_id, names.next(g), u, rng.choice(club_of[u]))
            totals["turnamen"] += count

        team_no = 0
        for name, teams, members, has_vr in TEAMS:
            cat_id = add_category(name, has_vr)
            g = gender_of(name)
            used_u = rng.sample(u_ids, teams)  # satu tim per kontingen
            for u in used_u:
                team_no += 1
                anggota = [names.next(g) for _ in range(members)]
                code = f"B_DUMMYBESAR{team_no}"
                club = club_of[u][0]
                add_atlet(cat_id, f"TEAM {anggota[0].split()[0]} CS", u, club, additional_code=code)
                for m, nama in enumerate(anggota, start=1):
                    add_atlet(cat_id, nama, u, club, additional_code=f"MEMBER_{code}_{m}")
                totals["beregu"] += members + 1

        for name, count in FESTIVAL:
            cat_id = add_category(name, 0)
            g = gender_of(name)
            for _ in range(count):
                u = pick_utusan()
                add_atlet(cat_id, names.next(g), u, rng.choice(club_of[u]))
            totals["festival"] += count

        cur.execute("COMMIT")
    except Exception:
        cur.execute("ROLLBACK")
        raise

    total = cur.execute("SELECT count(*) FROM backend_atlet WHERE event_id = ?", (event_id,)).fetchone()[0]
    print(f"Ditambahkan ke event {event_id}:")
    print(f"  {len(TOURNAMENT)} kategori turnamen perorangan, {totals['turnamen']} atlet")
    print(f"  {len(TEAMS)} kategori beregu, {team_no} tim ({totals['beregu']} baris)")
    print(f"  {len(FESTIVAL)} kategori festival, {totals['festival']} atlet")
    print(f"  {len(UTUSAN_BARU)} kontingen baru, {len(PERGURUAN_BARU)} perguruan baru, tatami 3-4")
    print(f"Total baris atlet di event sekarang: {total}")
    conn.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
