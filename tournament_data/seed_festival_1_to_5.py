import sqlite3
import json
import os

DB_PATH = os.path.join(os.path.dirname(__file__), 'db.sqlite3')
ROSTER_JSON_PATH = os.path.join(os.path.dirname(__file__), 'festival_roster_13.json')
EVENT_ID = 13

CATEGORIES_SPEC = [
    {
        "name": "FESTIVAL SIMULASI 1 GRUP - KATA PERORANGAN PUTRA",
        "tipe": "1", # Kata
        "groups": 1,
        "athletes_count": 4,
        "kode": "F33",
        "tatami_id": "53",
        "names": [
            "Arya Bima Pratama", "Rizky Dwi Mahendra", "Bagus Fajar Santoso", "Daffa Putra Ramadhan"
        ]
    },
    {
        "name": "FESTIVAL SIMULASI 2 GRUP - KUMITE PERORANGAN PUTRA",
        "tipe": "2", # Kumite
        "groups": 2,
        "athletes_count": 8,
        "kode": "F34",
        "tatami_id": "54",
        "names": [
            "Fahri Alamsyah", "Gilang Ramadhan", "Haikal Kurniawan", "Ihsan Nur Hidayat",
            "Jovan Aditya", "Kresna Bayu Wijaya", "Luthfi Zaidan", "Maulana Malik Ibrahim"
        ]
    },
    {
        "name": "FESTIVAL SIMULASI 3 GRUP - KATA PERORANGAN PUTRI",
        "tipe": "1", # Kata
        "groups": 3,
        "athletes_count": 12,
        "kode": "F35",
        "tatami_id": "55",
        "names": [
            "Adelia Putri Maharani", "Aisyah Nur Rohmah", "Bilqis Qanita", "Citra Kirana Dewi",
            "Dinda Ayu Larasati", "Elsa Safira", "Fatimah Azzahra", "Gita Permatasari",
            "Hana Maulida", "Intan Permata Sari", "Jihan Nur Aini", "Kayla Syakira"
        ]
    },
    {
        "name": "FESTIVAL SIMULASI 4 GRUP - KUMITE PERORANGAN PUTRI",
        "tipe": "2", # Kumite
        "groups": 4,
        "athletes_count": 16,
        "kode": "F36",
        "tatami_id": "56",
        "names": [
            "Laila Majnun", "Mutiara Cantika", "Nabila Syahrani", "Olivia Zahrani",
            "Putri Rahmawati", "Qonita Zahra", "Rania Azzahra", "Salwa Nadira",
            "Tazkia Aulia", "Ulya Rahmah", "Vina Panduwinata", "Winda Khairunnisa",
            "Xena Putri Warrior", "Yasmin Zahira", "Zahra Salsabila", "Anindya Nareswari"
        ]
    },
    {
        "name": "FESTIVAL SIMULASI 5 GRUP - KATA PERORANGAN PUTRA",
        "tipe": "1", # Kata
        "groups": 5,
        "athletes_count": 20,
        "kode": "F37",
        "tatami_id": "53",
        "names": [
            "Alifian Nur Hidayat", "Bima Sakti Pratama", "Candra Wijaya", "Danang Dwi Laksono",
            "Erlangga Putra", "Farhan Maulana", "Galang Rambu Anarki", "Haris Munandar",
            "Ilham Saputra", "Januar Pratama", "Kevin Sanjaya", "Lukman Hakim",
            "Muhammad Rafli", "Naufal Zhafif", "Oki Setiawan", "Pandu Wicaksana",
            "Qaisar Al Fatih", "Raden Mas Danu", "Satria Bagus Pambudi", "Taufik Hidayatullah"
        ]
    }
]

PERGURUAN_LIST = [3, 5, 6, 8, 9, 15, 36, 132] # INKAI, LEMKARI, WADOKAI, INKANAS, KKI, BKC, SHINDOKA, GOJU ASS
UTUSAN_LIST = [228, 229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239]

def main():
    conn = sqlite3.connect(DB_PATH)
    cur = conn.cursor()
    
    print(f"Connecting to database: {DB_PATH}")
    
    # 1. Clean up any existing simulation categories with these exact names first
    for spec in CATEGORIES_SPEC:
        cat_name = spec["name"]
        cur.execute("SELECT id FROM backend_nomortanding WHERE event_id = ? AND nama_nomor_tanding = ?", (EVENT_ID, cat_name))
        existing_cats = cur.fetchall()
        for (c_id,) in existing_cats:
            print(f"Cleaning up existing category {c_id}: {cat_name}")
            cur.execute("SELECT id FROM backend_bagan WHERE nomor_tanding_id = ?", (c_id,))
            bagans = cur.fetchall()
            for (b_id,) in bagans:
                cur.execute("DELETE FROM backend_detailbagan WHERE bagan_id = ?", (b_id,))
            cur.execute("DELETE FROM backend_bagan WHERE nomor_tanding_id = ?", (c_id,))
            cur.execute("DELETE FROM backend_atlet WHERE nomor_tanding_id = ?", (c_id,))
            cur.execute("DELETE FROM backend_nomortanding WHERE id = ?", (c_id,))
            
        # Also clean up any bagan with kode F33..F37
        cur.execute("SELECT id FROM backend_bagan WHERE event_id = ? AND kode = ?", (EVENT_ID, spec["kode"]))
        for (b_id,) in cur.fetchall():
            cur.execute("DELETE FROM backend_detailbagan WHERE bagan_id = ?", (b_id,))
            cur.execute("DELETE FROM backend_bagan WHERE id = ?", (b_id,))

    conn.commit()

    roster_data = {}
    if os.path.exists(ROSTER_JSON_PATH):
        try:
            with open(ROSTER_JSON_PATH, 'r') as f:
                roster_data = json.load(f)
        except Exception as e:
            print("Warning reading roster json:", e)
            roster_data = {"tatami_assignments": {}, "athlete_pairings": {}}
    else:
        roster_data = {"tatami_assignments": {}, "athlete_pairings": {}}

    tatami_assignments = roster_data.setdefault("tatami_assignments", {})
    athlete_pairings = roster_data.setdefault("athlete_pairings", {})

    created_summary = []
    
    perg_idx = 0
    utus_idx = 0

    for spec in CATEGORIES_SPEC:
        cat_name = spec["name"]
        tipe = spec["tipe"]
        groups_count = spec["groups"]
        kode = spec["kode"]
        names = spec["names"]
        
        # 1. Insert Nomor Tanding
        cur.execute(
            "INSERT INTO backend_nomortanding (event_id, nama_nomor_tanding, is_bob, has_vr) VALUES (?, ?, 0, 0)",
            (EVENT_ID, cat_name)
        )
        cat_id = cur.lastrowid
        
        # 2. Insert Athletes
        athlete_ids = []
        for i, a_name in enumerate(names):
            p_id = PERGURUAN_LIST[perg_idx % len(PERGURUAN_LIST)]
            u_id = UTUSAN_LIST[utus_idx % len(UTUSAN_LIST)]
            perg_idx += 1
            utus_idx += 1
            
            cur.execute(
                "INSERT INTO backend_atlet (nama_atlet, event_id, nomor_tanding_id, perguruan_id, utusan_id, is_priority) VALUES (?, ?, ?, ?, ?, 0)",
                (a_name, EVENT_ID, cat_id, p_id, u_id)
            )
            athlete_ids.append(cur.lastrowid)
            
        # 3. Insert Bagan
        cur.execute(
            "INSERT INTO backend_bagan (event_id, nomor_tanding_id, nama_bagan, pool, tipe_tanding, round_robin, is_bob, has_vr, kode) VALUES (?, ?, ?, ?, ?, 0, 0, 0, ?)",
            (EVENT_ID, cat_id, cat_name, groups_count, tipe, kode)
        )
        bagan_id = cur.lastrowid
        
        # 4. Insert Matches into backend_detailbagan
        # For each group g from 1 to groups_count:
        match_count = 0
        for g in range(1, groups_count + 1):
            base_idx = (g - 1) * 4
            a1 = athlete_ids[base_idx + 0]
            a2 = athlete_ids[base_idx + 1]
            a3 = athlete_ids[base_idx + 2]
            a4 = athlete_ids[base_idx + 3]
            
            semi1_urutan = 2 * g - 1
            semi2_urutan = 2 * g
            final_urutan = g
            
            # Semi 1: a1 vs a2
            cur.execute(
                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team) VALUES (?, 1, ?, ?, ?, 0, 0, 0, 0, 0)",
                (bagan_id, semi1_urutan, a1, a2)
            )
            match_count += 1
            
            # Semi 2: a3 vs a4
            cur.execute(
                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team) VALUES (?, 1, ?, ?, ?, 0, 0, 0, 0, 0)",
                (bagan_id, semi2_urutan, a3, a4)
            )
            match_count += 1
            
            # Final: NULL vs NULL
            cur.execute(
                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team) VALUES (?, 2, ?, NULL, NULL, 0, 0, 0, 0, 0)",
                (bagan_id, final_urutan)
            )
            match_count += 1
            
        # 5. Update Roster JSON
        tatami_key = spec["tatami_id"]
        tatami_list = tatami_assignments.setdefault(tatami_key, [])
        if cat_id not in tatami_list:
            tatami_list.append(cat_id)
        athlete_pairings[str(cat_id)] = athlete_ids
        
        created_summary.append({
            "cat_id": cat_id,
            "bagan_id": bagan_id,
            "kode": kode,
            "name": cat_name,
            "groups": groups_count,
            "athletes": len(athlete_ids),
            "matches": match_count,
            "tatami": tatami_key
        })

    conn.commit()
    conn.close()

    # Save updated roster JSON
    with open(ROSTER_JSON_PATH, 'w') as f:
        json.dump(roster_data, f, indent=2)

    print("\n=== HASIL PENAMBAHAN ATLET & BAGAN SIMULASI ===")
    for item in created_summary:
        print(f"[{item['kode']}] Bagan #{item['bagan_id']} (Cat #{item['cat_id']}): {item['name']}")
        print(f"      Grup: {item['groups']} | Atlet: {item['athletes']} | Partai: {item['matches']} | Tatami: {item['tatami']}")

if __name__ == '__main__':
    main()
