"""Seed satu event dummy kecil yang mencakup semua kasus turnamen dan festival.

Pemakaian (dari folder ambrilindo_desktop):
    python tournament_data/seed_dummy_lengkap.py            # buat event dummy
    python tournament_data/seed_dummy_lengkap.py --replace  # hapus event dummy lama lalu buat ulang

Event dikenali dari kode_event = "DUMMY-UJI". --replace hanya menghapus event itu
beserta seluruh datanya; event lain tidak disentuh. Bagan TIDAK dibuat: lakukan
drawing sendiri dari aplikasi supaya alur drawing ikut teruji.
"""

import os
import sqlite3
import sys
from datetime import datetime

DB_PATH = os.path.join(os.path.dirname(os.path.abspath(__file__)), "db.sqlite3")
EVENT_NAME = "SIMULASI LENGKAP (DUMMY UJI)"
EVENT_CODE = "DUMMY-UJI"

UTUSAN = [
    "KOTA SAMARINDA",
    "KOTA BALIKPAPAN",
    "KAB. KUTAI KARTANEGARA",
    "KOTA BONTANG",
    "KAB. BERAU (TANPA ATLET)",  # kasus: kontingen kosong (hapus aman / rekap 0)
]
PERGURUAN = ["INKAI", "LEMKARI", "WADOKAI", "INKANAS", "SHINDOKA (TANPA ATLET)"]

# (nama kategori, jumlah atlet/tim, has_vr, catatan kasus)
# Atlet dibagi bergiliran ke 4 kontingen pertama, kecuali kasus khusus di bawah.
TOURNAMENT = [
    ("TOURNAMENT SENIOR - KATA PERORANGAN PUTRA", 1, 0, "1 atlet: juara tanpa bertanding"),
    ("TOURNAMENT SENIOR - KATA PERORANGAN PUTRI", 2, 0, "2 atlet: langsung final"),
    ("TOURNAMENT SENIOR - KUMITE -55 KG PUTRA", 3, 1, "3 atlet: round robin / BYE"),
    ("TOURNAMENT SENIOR - KUMITE -50 KG PUTRI", 4, 1, "4 atlet: bagan pas, VR aktif"),
    ("TOURNAMENT JUNIOR - KUMITE -60 KG PUTRA", 5, 1, "5 atlet: banyak BYE, 2 sekontingen"),
    ("TOURNAMENT JUNIOR - KATA PERORANGAN PUTRA", 8, 0, "8 atlet: bagan penuh, repechage, 3 sekontingen"),
    ("TOURNAMENT JUNIOR - KUMITE -67 KG PUTRA", 9, 1, "9 atlet: lewat 8, jadi bagan 16"),
    ("TOURNAMENT KADET - KATA PERORANGAN PUTRI", 0, 0, "kategori kosong"),
]
TEAMS = [
    ("TOURNAMENT SENIOR - KATA BEREGU PUTRI", 3, 3, 0, "3 tim x 3 anggota"),
    ("TOURNAMENT SENIOR - KUMITE BEREGU PUTRA", 2, 4, 1, "2 tim x 4 anggota (langsung final beregu)"),
]
FESTIVAL = [
    ("FESTIVAL PEMULA - KUMITE -30 KG PUTRA", 1, "1 atlet"),
    ("FESTIVAL PRA PEMULA - KATA PERORANGAN PUTRA", 2, "2 atlet"),
    ("FESTIVAL USIA DINI - KATA PERORANGAN PUTRA", 4, "4 atlet: 1 grup penuh"),
    ("FESTIVAL USIA DINI - KUMITE -25 KG PUTRI", 5, "5 atlet: grup ganjil, ada BYE"),
    ("FESTIVAL PRA PEMULA - KATA PERORANGAN PUTRI", 8, "8 atlet: 2 grup"),
    ("FESTIVAL PEMULA - KATA PERORANGAN PUTRA", 12, "12 atlet: 3 grup"),
]

WASIT = [
    # nama, lisensi, perguruan (index), kab/kota
    ("SENSEI ANDI PRASETYO", "Wasit WKF / AKF", 0, "Kota Samarinda"),
    ("SENSEI BAMBANG SUTRISNO", "Wasit Nasional A", 1, "Kota Balikpapan"),
    ("SENSEI CAHYO NUGROHO", "Wasit Nasional B", 2, "Kab. Kutai Kartanegara"),
    ("SENSEI DEWI KARTIKA", "Juri Nasional A", 3, "Kota Bontang"),
    ("SENSEI EKO WAHYUDI", "Juri Nasional B", 0, "Kota Samarinda"),
    ("SENSEI FITRI HANDAYANI", "Juri Daerah", None, "Kota Balikpapan"),
    ("SENSEI GUNAWAN SALIM", "Wasit Daerah", 1, "Kota Bontang"),
]

PUTRA = [
    "Arya", "Bima", "Candra", "Dimas", "Fajar", "Gilang", "Hafiz", "Ilham", "Joko", "Kevin",
    "Lutfi", "Malik", "Naufal", "Oki", "Pandu", "Rafli", "Satria", "Taufik", "Umar", "Vino",
    "Wahyu", "Yoga", "Zidan", "Aldi", "Bayu", "Dafa", "Erlangga", "Farhan", "Galih", "Hendra",
]
PUTRI = [
    "Aisyah", "Bunga", "Citra", "Dinda", "Elsa", "Fatimah", "Gita", "Hana", "Intan", "Jihan",
    "Kayla", "Laila", "Mutiara", "Nabila", "Olivia", "Putri", "Qonita", "Rania", "Salwa", "Tiara",
    "Ulya", "Vina", "Winda", "Yasmin", "Zahra",
]
FAMILY = [
    "Pratama", "Saputra", "Wijaya", "Hidayat", "Kurniawan", "Santoso", "Ramadhan", "Nugroho",
    "Lestari", "Maharani", "Permata", "Anggraini", "Rahmawati", "Setiawan", "Utami", "Kusuma",
]


class NameGen:
    def __init__(self):
        self.used = set()
        self.i = {"PUTRA": 0, "PUTRI": 0}

    def next(self, gender):
        pool = PUTRA if gender == "PUTRA" else PUTRI
        while True:
            k = self.i[gender]
            self.i[gender] += 1
            name = f"{pool[k % len(pool)]} {FAMILY[(k // len(pool) + k) % len(FAMILY)]}".upper()
            if name not in self.used:
                self.used.add(name)
                return name


def gender_of(cat_name):
    return "PUTRI" if cat_name.endswith("PUTRI") else "PUTRA"


def delete_event(cur, event_id):
    """Deletes one event and everything that belongs to it (children first)."""
    bagan_ids = f"SELECT id FROM backend_bagan WHERE event_id = {event_id}"
    detail_ids = f"SELECT id FROM backend_detailbagan WHERE bagan_id IN ({bagan_ids})"
    statements = [
        f"DELETE FROM backend_score WHERE detail_bagan_id IN ({detail_ids})",
        f"DELETE FROM backend_matchup WHERE bagan_id IN ({bagan_ids})",
        f"DELETE FROM backend_wasitdetailbagan WHERE event_id = {event_id}",
        f"UPDATE backend_tatami SET detail_bagan_id = NULL WHERE event_id = {event_id}",
        f"DELETE FROM backend_detailbagan WHERE bagan_id IN ({bagan_ids})",
        f"DELETE FROM backend_timetablecell WHERE row_id IN (SELECT r.id FROM backend_timetablerow r JOIN backend_timetableday d ON r.day_id = d.id WHERE d.event_id = {event_id})",
        f"DELETE FROM backend_timetablerow WHERE day_id IN (SELECT id FROM backend_timetableday WHERE event_id = {event_id})",
        f"DELETE FROM backend_timetableday WHERE event_id = {event_id}",
        f"DELETE FROM backend_bagan WHERE event_id = {event_id}",
        f"DELETE FROM backend_wasittatami WHERE event_id = {event_id}",
        f"DELETE FROM backend_wasit WHERE event_id = {event_id}",
        f"DELETE FROM backend_atlet WHERE event_id = {event_id}",
        f"DELETE FROM backend_nomortanding WHERE event_id = {event_id}",
        f"DELETE FROM backend_tatami WHERE event_id = {event_id}",
        f"DELETE FROM backend_perguruan WHERE event_id = {event_id}",
        f"DELETE FROM backend_utusan WHERE event_id = {event_id}",
        f"DELETE FROM backend_kopsurat WHERE event_id = {event_id}",
        f"DELETE FROM backend_eventketerangan WHERE event_id = {event_id}",
        f"DELETE FROM backend_syncqueue WHERE event_id = {event_id}",
        f"DELETE FROM backend_event WHERE id = {event_id}",
    ]
    for sql in statements:
        cur.execute(sql)


def main():
    replace = "--replace" in sys.argv
    db_path = sys.argv[sys.argv.index("--db") + 1] if "--db" in sys.argv else DB_PATH
    conn = sqlite3.connect(db_path, timeout=20)
    conn.isolation_level = None  # manual transaction
    cur = conn.cursor()
    cur.execute("PRAGMA busy_timeout = 20000")

    existing = cur.execute("SELECT id FROM backend_event WHERE kode_event = ?", (EVENT_CODE,)).fetchall()
    if existing and not replace:
        print(f"Event dummy sudah ada (id {existing[0][0]}). Pakai --replace untuk membuat ulang.")
        return 1

    names = NameGen()
    summary = []
    cur.execute("BEGIN IMMEDIATE")
    try:
        for (old_id,) in existing:
            delete_event(cur, old_id)
            print(f"Event dummy lama (id {old_id}) dihapus.")

        cur.execute(
            "INSERT INTO backend_event (nama_event, kode_event, is_live_sync_enabled) VALUES (?, ?, 0)",
            (EVENT_NAME, EVENT_CODE),
        )
        event_id = cur.lastrowid

        utusan_ids = []
        for u in UTUSAN:
            cur.execute("INSERT INTO backend_utusan (nama_utusan, event_id, logo) VALUES (?, ?, NULL)", (u, event_id))
            utusan_ids.append(cur.lastrowid)
        perguruan_ids = []
        for p in PERGURUAN:
            cur.execute("INSERT INTO backend_perguruan (nama_perguruan, event_id) VALUES (?, ?)", (p, event_id))
            perguruan_ids.append(cur.lastrowid)
        for t in (1, 2):
            cur.execute("INSERT INTO backend_tatami (tatami_number, event_id, detail_bagan_id) VALUES (?, ?, NULL)", (t, event_id))

        now = datetime.now().strftime("%Y-%m-%d %H:%M:%S.%f")
        for i, (nama, lis, p_idx, kota) in enumerate(WASIT):
            cur.execute(
                "INSERT INTO backend_wasit (nama_wasit, lisensi, no_lisensi, is_active, created_at, event_id, perguruan_id, kab_kota) VALUES (?, ?, ?, 1, ?, ?, ?, ?)",
                (nama, lis, f"LIS-DUMMY-{i + 1:03d}", now, event_id, perguruan_ids[p_idx] if p_idx is not None else None, kota),
            )

        def add_category(name, has_vr):
            cur.execute(
                "INSERT INTO backend_nomortanding (nama_nomor_tanding, event_id, is_bob, has_vr) VALUES (?, ?, 0, ?)",
                (name, event_id, has_vr),
            )
            return cur.lastrowid

        def add_atlet(cat_id, nama, u_idx, p_idx, additional_code=None, priority=0):
            cur.execute(
                "INSERT INTO backend_atlet (nama_atlet, event_id, nomor_tanding_id, perguruan_id, utusan_id, nik, kode_atlet, additional_code, is_priority) VALUES (?, ?, ?, ?, ?, NULL, NULL, ?, ?)",
                (
                    nama, event_id, cat_id,
                    perguruan_ids[p_idx] if p_idx is not None else None,
                    utusan_ids[u_idx], additional_code, priority,
                ),
            )
            return cur.lastrowid

        rot = 0  # rotates contingents/clubs across the whole event
        for name, count, has_vr, note in TOURNAMENT:
            cat_id = add_category(name, has_vr)
            g = gender_of(name)
            for i in range(count):
                u_idx = rot % 4
                p_idx = (rot + 1) % 4
                if "2 sekontingen" in note and i < 2:
                    u_idx = 0  # dua atlet KOTA SAMARINDA -> uji anti-bentrok babak awal
                if "3 sekontingen" in note and i < 3:
                    u_idx = 1  # tiga atlet KOTA BALIKPAPAN
                if "9 atlet" in note and i == 8:
                    p_idx = None  # kasus: atlet tanpa perguruan
                priority = 1 if ("8 atlet" in note and i < 2) else 0  # kasus: unggulan
                add_atlet(cat_id, names.next(g), u_idx, p_idx, priority=priority)
                rot += 1
            summary.append((name, count, note))

        team_no = 0
        for name, teams, members, has_vr, note in TEAMS:
            cat_id = add_category(name, has_vr)
            g = gender_of(name)
            for t in range(teams):
                team_no += 1
                u_idx = t % 4
                anggota = [names.next(g) for _ in range(members)]
                code = f"B_DUMMY{team_no}"
                add_atlet(cat_id, f"TEAM {anggota[0].split()[0]} CS", u_idx, t % 4, additional_code=code)
                for m, nama in enumerate(anggota, start=1):
                    add_atlet(cat_id, nama, u_idx, t % 4, additional_code=f"MEMBER_{code}_{m}")
            summary.append((name, f"{teams} tim", note))

        for name, count, note in FESTIVAL:
            cat_id = add_category(name, 0)
            g = gender_of(name)
            for i in range(count):
                add_atlet(cat_id, names.next(g), rot % 4, (rot + 2) % 4)
                rot += 1
            summary.append((name, count, note))

        cur.execute("COMMIT")
    except Exception:
        cur.execute("ROLLBACK")
        raise

    total = cur.execute("SELECT count(*) FROM backend_atlet WHERE event_id = ?", (event_id,)).fetchone()[0]
    print(f"Event dibuat: id {event_id} - {EVENT_NAME} ({EVENT_CODE})")
    print(f"  {len(UTUSAN)} kontingen, {len(PERGURUAN)} perguruan, 2 tatami, {len(WASIT)} wasit, {total} baris atlet")
    for name, count, note in summary:
        print(f"  - {name}: {count} ({note})")
    conn.close()
    return 0


if __name__ == "__main__":
    sys.exit(main())
