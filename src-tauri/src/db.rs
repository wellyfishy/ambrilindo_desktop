use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;
use tracing::info;

use crate::bracket::{build_bracket_in_memory, split_athletes_into_pools, AthleteInput};
use crate::models::{
    Atlet, AthleteMutationInput, Bagan, BaganPodium, DrawingCategoryInfo, DrawingSummary, EnrichedAtlet,
    EnrichedDetailBagan, Event, NomorTanding, Perguruan, PodiumAthlete, SaveMatchInput, Tatami,
    UpdateMatchAthletesInput, Utusan,
};

#[derive(Clone)]
pub struct DatabaseManager {
    pool: Option<Pool<Sqlite>>,
    active_db_path: Option<PathBuf>,
    active_media_path: Option<PathBuf>,
}

impl DatabaseManager {
    pub fn new() -> Self {
        Self {
            pool: None,
            active_db_path: None,
            active_media_path: None,
        }
    }

    /// Mounts an existing .sqlite3 tournament database file and sets adjacent media path
    pub async fn mount_database<P: AsRef<Path>>(&mut self, db_path: P) -> Result<(), String> {
        let path = db_path.as_ref().to_path_buf();
        if !path.exists() {
            return Err(format!("File database tidak ditemukan: {:?}", path));
        }

        let parent_dir = path.parent().unwrap_or_else(|| Path::new("."));
        let media_dir = parent_dir.join("media");

        // Ensure adjacent media directory exists
        if !media_dir.exists() {
            let _ = std::fs::create_dir_all(&media_dir);
            let _ = std::fs::create_dir_all(media_dir.join("logo_utusan"));
            let _ = std::fs::create_dir_all(media_dir.join("kop_surat_logos"));
            let _ = std::fs::create_dir_all(media_dir.join("festival_rosters"));
        }

        let db_url = format!("sqlite://{}", path.to_string_lossy());
        let options = SqliteConnectOptions::from_str(&db_url)
            .map_err(|e| format!("URL database tidak valid: {}", e))?
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal)
            .busy_timeout(Duration::from_millis(5000));

        let pool = SqlitePoolOptions::new()
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(5))
            .connect_with(options)
            .await
            .map_err(|e| format!("Gagal membuka koneksi database: {}", e))?;

        info!("Database mounted: {:?} (Media: {:?})", path, media_dir);

        self.pool = Some(pool);
        self.active_db_path = Some(path);
        self.active_media_path = Some(media_dir);

        Ok(())
    }

    pub fn get_pool(&self) -> Result<&Pool<Sqlite>, String> {
        self.pool
            .as_ref()
            .ok_or_else(|| "Tidak ada database aktif yang dimuat.".to_string())
    }

    pub fn get_media_dir(&self) -> Option<&PathBuf> {
        self.active_media_path.as_ref()
    }

    pub fn get_db_path(&self) -> Option<&PathBuf> {
        self.active_db_path.as_ref()
    }

    /// Fetches all championships/events in the active database
    pub async fn get_events(&self) -> Result<Vec<Event>, String> {
        let pool = self.get_pool()?;
        sqlx::query_as::<_, Event>(
            "SELECT id, nama_event, kode_event, is_live_sync_enabled FROM backend_event ORDER BY id DESC"
        )
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat daftar event: {}", e))
    }

    /// Creates a new championship/event in the active database
    pub async fn create_event(&self, nama_event: &str, kode_event: Option<String>) -> Result<Event, String> {
        let pool = self.get_pool()?;
        let code = match kode_event {
            Some(c) if !c.trim().is_empty() => c.trim().to_uppercase(),
            _ => {
                let millis = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                format!("EV{:08X}", (millis & 0xFFFFFFFF) as u32)
            }
        };

        let res = sqlx::query(
            "INSERT INTO backend_event (nama_event, kode_event, is_live_sync_enabled) VALUES (?, ?, 0)"
        )
        .bind(nama_event)
        .bind(&code)
        .execute(pool)
        .await
        .map_err(|e| format!("Gagal menambahkan event: {}", e))?;

        Ok(Event {
            id: res.last_insert_rowid(),
            nama_event: Some(nama_event.to_string()),
            kode_event: Some(code),
            is_live_sync_enabled: false,
        })
    }

    /// Fetches all tatamis for an event
    pub async fn get_tatamis(&self, event_id: i64) -> Result<Vec<Tatami>, String> {
        let pool = self.get_pool()?;
        sqlx::query_as::<_, Tatami>(
            "SELECT id, event_id, tatami_number, detail_bagan_id FROM backend_tatami WHERE event_id = ? ORDER BY tatami_number ASC"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat daftar tatami: {}", e))
    }

    /// Creates a new tatami for an event
    pub async fn create_tatami(&self, event_id: i64, tatami_number: i32) -> Result<Tatami, String> {
        let pool = self.get_pool()?;
        let res = sqlx::query(
            "INSERT INTO backend_tatami (event_id, tatami_number, detail_bagan_id) VALUES (?, ?, NULL)"
        )
        .bind(event_id)
        .bind(tatami_number)
        .execute(pool)
        .await
        .map_err(|e| format!("Gagal menambahkan tatami: {}", e))?;

        Ok(Tatami {
            id: res.last_insert_rowid(),
            event_id: Some(event_id),
            tatami_number: Some(tatami_number),
            detail_bagan_id: None,
        })
    }

    /// Deletes a tatami by ID
    pub async fn delete_tatami(&self, tatami_id: i64) -> Result<(), String> {
        let pool = self.get_pool()?;
        sqlx::query("DELETE FROM backend_tatami WHERE id = ?")
            .bind(tatami_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal menghapus tatami: {}", e))?;
        Ok(())
    }

    /// Updates tatami number
    pub async fn update_tatami_number(&self, tatami_id: i64, new_number: i32) -> Result<(), String> {
        let pool = self.get_pool()?;
        sqlx::query("UPDATE backend_tatami SET tatami_number = ? WHERE id = ?")
            .bind(new_number)
            .bind(tatami_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal memperbarui tatami: {}", e))?;
        Ok(())
    }

    /// Fetches all competition categories (Nomor Tanding) for an event
    pub async fn get_categories(&self, event_id: i64) -> Result<Vec<NomorTanding>, String> {
        let pool = self.get_pool()?;
        sqlx::query_as::<_, NomorTanding>(
            "SELECT id, event_id, nama_nomor_tanding, is_bob, has_vr FROM backend_nomortanding WHERE event_id = ? ORDER BY nama_nomor_tanding ASC"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat kategori pertandingan: {}", e))
    }

    /// Fetches all brackets (Bagan) for an event
    pub async fn get_bagans(&self, event_id: i64) -> Result<Vec<Bagan>, String> {
        let pool = self.get_pool()?;

        // Ensure all bagans for this event have a 3-digit zero-padded code (001, 002, ...)
        let has_unassigned_code: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM backend_bagan WHERE event_id = ? AND (kode IS NULL OR kode = ''))"
        )
        .bind(event_id)
        .fetch_one(pool)
        .await
        .unwrap_or(false);

        if has_unassigned_code {
            let bagan_ids: Vec<i64> = sqlx::query_scalar(
                "SELECT id FROM backend_bagan WHERE event_id = ? ORDER BY id ASC"
            )
            .bind(event_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            for (idx, b_id) in bagan_ids.into_iter().enumerate() {
                let code = format!("{:03}", idx + 1);
                let _ = sqlx::query("UPDATE backend_bagan SET kode = ? WHERE id = ?")
                    .bind(code)
                    .bind(b_id)
                    .execute(pool)
                    .await;
            }
        }

        sqlx::query_as::<_, Bagan>(
            "SELECT id, event_id, nama_bagan, nomor_tanding_id, tipe_tanding, juara_1_id, juara_2_id, juara_3a_id, juara_3b_id, peringkat_5_id, peringkat_6_id, peringkat_7_id, peringkat_8_id, round_robin, pool, kode, is_bob, has_vr FROM backend_bagan WHERE event_id = ? ORDER BY CAST(COALESCE(kode, '9999') AS INTEGER) ASC, id ASC"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat daftar bagan: {}", e))
    }

    /// Fetches a single bracket (Bagan) by its ID
    pub async fn get_bagan_by_id(&self, bagan_id: i64) -> Result<Option<Bagan>, String> {
        let pool = self.get_pool()?;
        sqlx::query_as::<_, Bagan>(
            "SELECT id, event_id, nama_bagan, nomor_tanding_id, tipe_tanding, juara_1_id, juara_2_id, juara_3a_id, juara_3b_id, peringkat_5_id, peringkat_6_id, peringkat_7_id, peringkat_8_id, round_robin, pool, kode, is_bob, has_vr FROM backend_bagan WHERE id = ?"
        )
        .bind(bagan_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Gagal memuat bagan #{}: {}", bagan_id, e))
    }

    /// Fetches podium standings (Juara 1, 2, 3A, 3B) for a bagan or category
    pub async fn get_bagan_podium(&self, bagan_id: i64) -> Result<BaganPodium, String> {
        let pool = self.get_pool()?;

        // Fetch juara IDs from the bagan
        let bagan_row: Option<(Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<i64>, Option<String>, i32)> = sqlx::query_as(
            "SELECT juara_1_id, juara_2_id, juara_3a_id, juara_3b_id, nomor_tanding_id, nama_bagan, pool FROM backend_bagan WHERE id = ?"
        )
        .bind(bagan_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Gagal memuat juara bagan: {}", e))?;

        let (mut j1_id, mut j2_id, mut j3a_id, mut j3b_id, nt_id, _nama_bagan, _pool_num) = match bagan_row {
            Some(row) => row,
            None => return Ok(BaganPodium { j1: None, j2: None, j3a: None, j3b: None }),
        };

        // If this bagan has related pool bagans or final bagan in the same category, check them for any missing juara IDs
        if let Some(category_id) = nt_id {
            if j1_id.is_none() || j2_id.is_none() || j3a_id.is_none() || j3b_id.is_none() {
                let related_ids: Vec<(Option<i64>, Option<i64>, Option<i64>, Option<i64>)> = sqlx::query_as(
                    "SELECT juara_1_id, juara_2_id, juara_3a_id, juara_3b_id FROM backend_bagan WHERE nomor_tanding_id = ? AND id != ?"
                )
                .bind(category_id)
                .bind(bagan_id)
                .fetch_all(pool)
                .await
                .unwrap_or_default();

                for (r1, r2, r3a, r3b) in related_ids {
                    if j1_id.is_none() && r1.is_some() { j1_id = r1; }
                    if j2_id.is_none() && r2.is_some() { j2_id = r2; }
                    if j3a_id.is_none() && r3a.is_some() { j3a_id = r3a; }
                    if j3b_id.is_none() && r3b.is_some() { j3b_id = r3b; }
                }
            }
        }

        async fn fetch_podium_athlete(pool: &sqlx::SqlitePool, athlete_id: i64) -> Option<PodiumAthlete> {
            sqlx::query_as::<_, (i64, Option<String>, Option<String>, Option<String>, Option<String>, bool)>(
                r#"
                SELECT 
                    a.id, a.nama_atlet,
                    p.nama_perguruan,
                    u.nama_utusan,
                    u.logo,
                    COALESCE(a.is_priority, 0)
                FROM backend_atlet a
                LEFT JOIN backend_perguruan p ON a.perguruan_id = p.id
                LEFT JOIN backend_utusan u ON a.utusan_id = u.id
                WHERE a.id = ?
                "#
            )
            .bind(athlete_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .map(|(id, name, perguruan, utusan, logo, is_priority)| {
                let contingent = match (perguruan, utusan) {
                    (Some(p), Some(u)) if p != u => format!("{} • {}", p, u),
                    (Some(p), _) => p,
                    (_, Some(u)) => u,
                    _ => "—".to_string(),
                };
                PodiumAthlete {
                    id,
                    name: name.unwrap_or_else(|| format!("Atlet #{}", id)),
                    contingent,
                    logo,
                    is_priority,
                }
            })
        }

        let j1 = if let Some(id) = j1_id { fetch_podium_athlete(pool, id).await } else { None };
        let j2 = if let Some(id) = j2_id { fetch_podium_athlete(pool, id).await } else { None };
        let j3a = if let Some(id) = j3a_id { fetch_podium_athlete(pool, id).await } else { None };
        let j3b = if let Some(id) = j3b_id { fetch_podium_athlete(pool, id).await } else { None };

        Ok(BaganPodium { j1, j2, j3a, j3b })
    }

    /// Fetches matches (DetailBagan) for a bracket enriched with athlete names and clubs
    pub async fn get_detail_bagan(&self, bagan_id: i64) -> Result<Vec<EnrichedDetailBagan>, String> {
        let pool = self.get_pool()?;
        sqlx::query_as::<_, EnrichedDetailBagan>(
            r#"
            SELECT 
                d.id, d.bagan_id, d.round, d.urutan, d.atlet1_id, d.atlet2_id,
                d.score1, d.score2, d.scorekecil1, d.scorekecil2,
                d.vr1, d.vr2, d.kata1, d.kata2, d.pemenang, d.hantei, d.selesai, d.team,
                d.assigned_tatami_id, d.kode,
                a1.nama_atlet as atlet1_nama,
                CASE 
                    WHEN p1.nama_perguruan IS NOT NULL AND u1.nama_utusan IS NOT NULL AND p1.nama_perguruan != u1.nama_utusan 
                        THEN p1.nama_perguruan || ' • ' || u1.nama_utusan
                    ELSE COALESCE(u1.nama_utusan, p1.nama_perguruan)
                END as atlet1_kontingen,
                u1.logo as atlet1_logo,
                a1.is_priority as atlet1_is_priority,
                a2.nama_atlet as atlet2_nama,
                CASE 
                    WHEN p2.nama_perguruan IS NOT NULL AND u2.nama_utusan IS NOT NULL AND p2.nama_perguruan != u2.nama_utusan 
                        THEN p2.nama_perguruan || ' • ' || u2.nama_utusan
                    ELSE COALESCE(u2.nama_utusan, p2.nama_perguruan)
                END as atlet2_kontingen,
                u2.logo as atlet2_logo,
                a2.is_priority as atlet2_is_priority
            FROM backend_detailbagan d
            LEFT JOIN backend_atlet a1 ON d.atlet1_id = a1.id
            LEFT JOIN backend_utusan u1 ON a1.utusan_id = u1.id
            LEFT JOIN backend_perguruan p1 ON a1.perguruan_id = p1.id
            LEFT JOIN backend_atlet a2 ON d.atlet2_id = a2.id
            LEFT JOIN backend_utusan u2 ON a2.utusan_id = u2.id
            LEFT JOIN backend_perguruan p2 ON a2.perguruan_id = p2.id
            WHERE d.bagan_id = ?
            ORDER BY d.round ASC, d.urutan ASC
            "#
        )
        .bind(bagan_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat detail bagan: {}", e))
    }

    /// Fetches all athletes for an event with contingent, club, and category details
    pub async fn get_enriched_athletes(&self, event_id: i64) -> Result<Vec<EnrichedAtlet>, String> {
        let pool = self.get_pool()?;
        sqlx::query_as::<_, EnrichedAtlet>(
            r#"
            SELECT 
                a.id, a.event_id, a.nama_atlet, a.nik, a.kode_atlet, a.additional_code, a.is_priority,
                a.perguruan_id, p.nama_perguruan as perguruan_nama,
                a.utusan_id, u.nama_utusan as utusan_nama, u.logo as utusan_logo,
                a.nomor_tanding_id, nt.nama_nomor_tanding as nomor_tanding_nama,
                CASE 
                    WHEN nt.is_bob = 1 THEN 'BOB'
                    WHEN UPPER(nt.nama_nomor_tanding) LIKE '%FESTIVAL%' THEN 'Festival'
                    ELSE 'Tournament'
                END as nomor_tanding_tipe,
                nt.is_bob
            FROM backend_atlet a
            LEFT JOIN backend_perguruan p ON a.perguruan_id = p.id
            LEFT JOIN backend_utusan u ON a.utusan_id = u.id
            LEFT JOIN backend_nomortanding nt ON a.nomor_tanding_id = nt.id
            WHERE a.event_id = ?
            ORDER BY a.nama_atlet ASC
            "#
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat daftar atlet lengkap: {}", e))
    }

    /// Creates a new athlete
    pub async fn create_athlete(&self, input: AthleteMutationInput) -> Result<EnrichedAtlet, String> {
        let pool = self.get_pool()?;
        let clean_name = input.nama_atlet.trim().to_uppercase();
        if clean_name.is_empty() {
            return Err("Nama atlet tidak boleh kosong.".to_string());
        }

        let res = sqlx::query(
            r#"
            INSERT INTO backend_atlet (
                event_id, nama_atlet, nik, kode_atlet, additional_code, is_priority,
                perguruan_id, utusan_id, nomor_tanding_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(input.event_id)
        .bind(&clean_name)
        .bind(input.nik.filter(|s| !s.trim().is_empty()))
        .bind(input.kode_atlet.filter(|s| !s.trim().is_empty()))
        .bind(input.additional_code.filter(|s| !s.trim().is_empty()))
        .bind(input.is_priority)
        .bind(input.perguruan_id)
        .bind(input.utusan_id)
        .bind(input.nomor_tanding_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Gagal menyimpan atlet: {}", e))?;

        let new_id = res.last_insert_rowid();

        sqlx::query_as::<_, EnrichedAtlet>(
            r#"
            SELECT 
                a.id, a.event_id, a.nama_atlet, a.nik, a.kode_atlet, a.additional_code, a.is_priority,
                a.perguruan_id, p.nama_perguruan as perguruan_nama,
                a.utusan_id, u.nama_utusan as utusan_nama, u.logo as utusan_logo,
                a.nomor_tanding_id, nt.nama_nomor_tanding as nomor_tanding_nama,
                CASE 
                    WHEN nt.is_bob = 1 THEN 'BOB'
                    WHEN UPPER(nt.nama_nomor_tanding) LIKE '%FESTIVAL%' THEN 'Festival'
                    ELSE 'Tournament'
                END as nomor_tanding_tipe,
                nt.is_bob
            FROM backend_atlet a
            LEFT JOIN backend_perguruan p ON a.perguruan_id = p.id
            LEFT JOIN backend_utusan u ON a.utusan_id = u.id
            LEFT JOIN backend_nomortanding nt ON a.nomor_tanding_id = nt.id
            WHERE a.id = ?
            "#
        )
        .bind(new_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Gagal memuat atlet yang baru dibuat: {}", e))
    }

    /// Updates an existing athlete
    pub async fn update_athlete(&self, id: i64, input: AthleteMutationInput) -> Result<EnrichedAtlet, String> {
        let pool = self.get_pool()?;
        let clean_name = input.nama_atlet.trim().to_uppercase();
        if clean_name.is_empty() {
            return Err("Nama atlet tidak boleh kosong.".to_string());
        }

        sqlx::query(
            r#"
            UPDATE backend_atlet SET
                nama_atlet = ?,
                nik = ?,
                kode_atlet = ?,
                additional_code = ?,
                is_priority = ?,
                perguruan_id = ?,
                utusan_id = ?,
                nomor_tanding_id = ?
            WHERE id = ?
            "#
        )
        .bind(&clean_name)
        .bind(input.nik.filter(|s| !s.trim().is_empty()))
        .bind(input.kode_atlet.filter(|s| !s.trim().is_empty()))
        .bind(input.additional_code.filter(|s| !s.trim().is_empty()))
        .bind(input.is_priority)
        .bind(input.perguruan_id)
        .bind(input.utusan_id)
        .bind(input.nomor_tanding_id)
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| format!("Gagal mengupdate data atlet: {}", e))?;

        sqlx::query_as::<_, EnrichedAtlet>(
            r#"
            SELECT 
                a.id, a.event_id, a.nama_atlet, a.nik, a.kode_atlet, a.additional_code, a.is_priority,
                a.perguruan_id, p.nama_perguruan as perguruan_nama,
                a.utusan_id, u.nama_utusan as utusan_nama, u.logo as utusan_logo,
                a.nomor_tanding_id, nt.nama_nomor_tanding as nomor_tanding_nama,
                CASE 
                    WHEN nt.is_bob = 1 THEN 'BOB'
                    WHEN UPPER(nt.nama_nomor_tanding) LIKE '%FESTIVAL%' THEN 'Festival'
                    ELSE 'Tournament'
                END as nomor_tanding_tipe,
                nt.is_bob
            FROM backend_atlet a
            LEFT JOIN backend_perguruan p ON a.perguruan_id = p.id
            LEFT JOIN backend_utusan u ON a.utusan_id = u.id
            LEFT JOIN backend_nomortanding nt ON a.nomor_tanding_id = nt.id
            WHERE a.id = ?
            "#
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Gagal memuat atlet yang diupdate: {}", e))
    }

    /// Deletes a single athlete with validation check against active bracket matches
    pub async fn delete_athlete(&self, id: i64) -> Result<(), String> {
        let pool = self.get_pool()?;
        let linked_count: (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM backend_detailbagan WHERE atlet1_id = ? OR atlet2_id = ?"
        )
        .bind(id)
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Gagal memeriksa keterikatan atlet: {}", e))?;

        if linked_count.0 > 0 {
            return Err(format!(
                "Atlet tidak dapat dihapus karena sudah terdaftar di {} partai bagan tanding. Hapus bagan terlebih dahulu.",
                linked_count.0
            ));
        }

        sqlx::query("DELETE FROM backend_atlet WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal menghapus atlet: {}", e))?;

        Ok(())
    }

    /// Batch deletes athletes (skips any currently locked in active bracket matches)
    pub async fn delete_athletes_batch(&self, ids: Vec<i64>) -> Result<u64, String> {
        if ids.is_empty() {
            return Ok(0);
        }
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        let mut deleted_count = 0u64;
        for id in ids {
            let linked: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM backend_detailbagan WHERE atlet1_id = ? OR atlet2_id = ?"
            )
            .bind(id)
            .bind(id)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or((0,));

            if linked.0 == 0 {
                let res = sqlx::query("DELETE FROM backend_atlet WHERE id = ?")
                    .bind(id)
                    .execute(&mut *tx)
                    .await;
                if let Ok(r) = res {
                    deleted_count += r.rows_affected();
                }
            }
        }

        tx.commit().await.map_err(|e| format!("Gagal menyimpan transaksi: {}", e))?;
        Ok(deleted_count)
    }

    /// Toggles an athlete's seed/priority flag
    pub async fn toggle_athlete_priority(&self, id: i64, is_priority: bool) -> Result<(), String> {
        let pool = self.get_pool()?;
        sqlx::query("UPDATE backend_atlet SET is_priority = ? WHERE id = ?")
            .bind(is_priority)
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal mengubah status prioritas atlet: {}", e))?;
        Ok(())
    }

    /// Fetches all athletes for an event
    pub async fn get_athletes(&self, event_id: i64) -> Result<Vec<Atlet>, String> {
        let pool = self.get_pool()?;
        sqlx::query_as::<_, Atlet>(
            "SELECT id, event_id, nama_atlet, nik, kode_atlet, additional_code, is_priority, perguruan_id, utusan_id, nomor_tanding_id FROM backend_atlet WHERE event_id = ? ORDER BY nama_atlet ASC"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat daftar atlet: {}", e))
    }

    /// Fetches all perguruans for an event
    pub async fn get_perguruans(&self, event_id: i64) -> Result<Vec<Perguruan>, String> {
        let pool = self.get_pool()?;
        sqlx::query_as::<_, Perguruan>(
            "SELECT id, event_id, nama_perguruan FROM backend_perguruan WHERE event_id = ? ORDER BY nama_perguruan ASC"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat daftar perguruan: {}", e))
    }

    /// Fetches all contingents/clubs for an event
    pub async fn get_contingents(&self, event_id: i64) -> Result<Vec<Utusan>, String> {
        let pool = self.get_pool()?;
        sqlx::query_as::<_, Utusan>(
            "SELECT id, event_id, nama_utusan, logo FROM backend_utusan WHERE event_id = ? ORDER BY nama_utusan ASC"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat daftar kontingen: {}", e))
    }

    /// Fetches all tournament categories eligible for bracket drawing
    pub async fn get_drawing_eligible_categories(&self, event_id: i64) -> Result<Vec<DrawingCategoryInfo>, String> {
        let pool = self.get_pool()?;

        #[derive(sqlx::FromRow)]
        struct RawCat {
            id: i64,
            event_id: Option<i64>,
            nama_nomor_tanding: Option<String>,
            has_vr: bool,
            athlete_count: i64,
            bagan_count: i64,
        }

        let rows = sqlx::query_as::<_, RawCat>(
            r#"
            SELECT 
                nt.id,
                nt.event_id,
                nt.nama_nomor_tanding,
                nt.has_vr,
                (
                    SELECT COUNT(*) FROM backend_atlet a 
                    WHERE a.nomor_tanding_id = nt.id 
                      AND (a.additional_code IS NULL OR a.additional_code NOT LIKE 'MEMBER_%')
                ) as athlete_count,
                (
                    SELECT COUNT(*) FROM backend_bagan b 
                    WHERE b.nomor_tanding_id = nt.id
                ) as bagan_count
            FROM backend_nomortanding nt
            WHERE nt.event_id = ?
              AND LOWER(COALESCE(nt.nama_nomor_tanding, '')) NOT LIKE '%festival%'
              AND (nt.is_bob = 0 OR nt.is_bob IS NULL)
            ORDER BY nt.nama_nomor_tanding ASC
            "#
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat kategori drawing: {}", e))?;

        let res = rows.into_iter().map(|r| {
            let nama = r.nama_nomor_tanding.unwrap_or_default();
            let upper = nama.to_uppercase();
            let is_kumite = upper.contains("KUMITE");
            let is_beregu = upper.contains("BEREGU");
            DrawingCategoryInfo {
                id: r.id,
                event_id: r.event_id.unwrap_or(event_id),
                nama_nomor_tanding: nama,
                athlete_count: r.athlete_count,
                bagan_count: r.bagan_count,
                has_vr: r.has_vr,
                is_kumite,
                is_beregu,
            }
        }).collect();

        Ok(res)
    }

    /// Deletes all brackets and match records for an entire event
    pub async fn delete_all_bagans(&self, event_id: i64) -> Result<(), String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        // 1. Unassign tatamis
        sqlx::query("UPDATE backend_tatami SET detail_bagan_id = NULL WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal mereset tatami: {}", e))?;

        // 2. Delete detail bagans
        sqlx::query(
            "DELETE FROM backend_detailbagan WHERE bagan_id IN (SELECT id FROM backend_bagan WHERE event_id = ?)"
        )
        .bind(event_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Gagal menghapus detail bagan: {}", e))?;

        // 3. Delete bagans
        sqlx::query("DELETE FROM backend_bagan WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghapus bagan: {}", e))?;

        tx.commit().await.map_err(|e| format!("Gagal menyimpan perubahan: {}", e))?;
        Ok(())
    }

    /// Deletes all athletes for an entire event, clearing bracket/match references
    pub async fn delete_all_athletes(&self, event_id: i64) -> Result<u64, String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        // 1. Clear athlete references in detailbagan for this event
        sqlx::query(
            "UPDATE backend_detailbagan SET atlet1_id = NULL, atlet2_id = NULL WHERE bagan_id IN (SELECT id FROM backend_bagan WHERE event_id = ?)"
        )
        .bind(event_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Gagal mereset atlet pada partai: {}", e))?;

        // 2. Clear winner references in bagan for this event
        sqlx::query(
            "UPDATE backend_bagan SET juara_1_id = NULL, juara_2_id = NULL, juara_3a_id = NULL, juara_3b_id = NULL, peringkat_5_id = NULL, peringkat_6_id = NULL, peringkat_7_id = NULL, peringkat_8_id = NULL WHERE event_id = ?"
        )
        .bind(event_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Gagal mereset juara pada bagan: {}", e))?;

        // 3. Delete athletes
        let res = sqlx::query("DELETE FROM backend_atlet WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghapus atlet: {}", e))?;

        let affected = res.rows_affected();
        tx.commit().await.map_err(|e| format!("Gagal menyimpan perubahan: {}", e))?;
        Ok(affected)
    }

    /// Deletes brackets and match records for a single category
    pub async fn delete_category_bagans(&self, _event_id: i64, category_id: i64) -> Result<(), String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        // 1. Unassign tatami matches for this category
        sqlx::query(
            "UPDATE backend_tatami SET detail_bagan_id = NULL WHERE detail_bagan_id IN (
                SELECT d.id FROM backend_detailbagan d 
                JOIN backend_bagan b ON d.bagan_id = b.id 
                WHERE b.nomor_tanding_id = ?
            )"
        )
        .bind(category_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Gagal mereset tatami: {}", e))?;

        // 2. Delete detail bagans
        sqlx::query(
            "DELETE FROM backend_detailbagan WHERE bagan_id IN (SELECT id FROM backend_bagan WHERE nomor_tanding_id = ?)"
        )
        .bind(category_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Gagal menghapus detail bagan kategori: {}", e))?;

        // 3. Delete bagans
        sqlx::query("DELETE FROM backend_bagan WHERE nomor_tanding_id = ?")
            .bind(category_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghapus bagan kategori: {}", e))?;

        tx.commit().await.map_err(|e| format!("Gagal menyimpan perubahan: {}", e))?;
        Ok(())
    }

    /// Executes tournament bracket drawing for selected categories
    pub async fn execute_drawing_bagan(
        &self,
        event_id: i64,
        category_ids: Vec<i64>,
        shuffle_type: &str,
        vr_category_ids: Vec<i64>,
    ) -> Result<DrawingSummary, String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        let vr_set: HashSet<i64> = vr_category_ids.into_iter().collect();
        let mut categories_processed = 0;
        let mut bagans_created = 0;
        let mut total_matches = 0;
        let mut total_collisions = 0;

        // Clean up previous brackets for selected categories first
        for &cat_id in &category_ids {
            sqlx::query(
                "UPDATE backend_tatami SET detail_bagan_id = NULL WHERE detail_bagan_id IN (
                    SELECT d.id FROM backend_detailbagan d 
                    JOIN backend_bagan b ON d.bagan_id = b.id 
                    WHERE b.nomor_tanding_id = ?
                )"
            )
            .bind(cat_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal mereset tatami untuk kategori {}: {}", cat_id, e))?;

            sqlx::query("DELETE FROM backend_detailbagan WHERE bagan_id IN (SELECT id FROM backend_bagan WHERE nomor_tanding_id = ?)")
                .bind(cat_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal menghapus detail bagan lama: {}", e))?;

            sqlx::query("DELETE FROM backend_bagan WHERE nomor_tanding_id = ?")
                .bind(cat_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal menghapus bagan lama: {}", e))?;
        }

        // Determine starting code number (001, 002...) for this event
        let existing_codes: Vec<String> = sqlx::query_scalar(
            "SELECT kode FROM backend_bagan WHERE event_id = ? AND kode IS NOT NULL AND kode != ''"
        )
        .bind(event_id)
        .fetch_all(&mut *tx)
        .await
        .unwrap_or_default();

        let mut next_code_num: u32 = existing_codes
            .iter()
            .filter_map(|c| c.parse::<u32>().ok())
            .max()
            .unwrap_or(0) + 1;

        for cat_id in category_ids {

            // Fetch category info
            #[derive(sqlx::FromRow)]
            #[allow(dead_code)]
            struct CatRow {
                id: i64,
                nama_nomor_tanding: Option<String>,
                is_bob: bool,
                has_vr: bool,
            }

            let cat = sqlx::query_as::<_, CatRow>(
                "SELECT id, nama_nomor_tanding, is_bob, has_vr FROM backend_nomortanding WHERE id = ?"
            )
            .bind(cat_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("Gagal memuat kategori {}: {}", cat_id, e))?;

            let Some(cat) = cat else { continue; };
            let cat_name = cat.nama_nomor_tanding.unwrap_or_default();
            if cat.is_bob || cat_name.to_lowercase().contains("festival") {
                continue;
            }

            let is_kumite = cat_name.to_uppercase().contains("KUMITE");
            let is_kata = cat_name.to_uppercase().contains("KATA");
            let tipe_tanding = if is_kumite {
                Some("2")
            } else if is_kata {
                Some("1")
            } else {
                None
            };

            // Update VR status for category
            let should_have_vr = is_kumite && vr_set.contains(&cat_id);
            if cat.has_vr != should_have_vr {
                sqlx::query("UPDATE backend_nomortanding SET has_vr = ? WHERE id = ?")
                    .bind(should_have_vr)
                    .bind(cat_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("Gagal update status VR kategori: {}", e))?;
            }

            // Fetch athletes
            #[derive(sqlx::FromRow)]
            struct RawAtlet {
                id: i64,
                nama_atlet: Option<String>,
                is_priority: bool,
                perguruan_id: Option<i64>,
                utusan_id: Option<i64>,
            }

            let raw_atlets = sqlx::query_as::<_, RawAtlet>(
                r#"
                SELECT id, nama_atlet, is_priority, perguruan_id, utusan_id
                FROM backend_atlet
                WHERE nomor_tanding_id = ?
                  AND (additional_code IS NULL OR additional_code NOT LIKE 'MEMBER_%')
                ORDER BY id ASC
                "#
            )
            .bind(cat_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| format!("Gagal memuat atlet: {}", e))?;

            if raw_atlets.is_empty() {
                continue;
            }

            let athletes: Vec<AthleteInput> = raw_atlets.into_iter().map(|a| {
                let group_key = if shuffle_type == "perguruan" {
                    a.perguruan_id.map(|id| format!("P_{}", id)).unwrap_or_else(|| format!("A_{}", a.id))
                } else {
                    a.utusan_id.map(|id| format!("U_{}", id)).unwrap_or_else(|| format!("A_{}", a.id))
                };
                AthleteInput {
                    id: a.id,
                    name: a.nama_atlet.unwrap_or_default(),
                    is_priority: a.is_priority,
                    group_key,
                }
            }).collect();

            let total_atlets = athletes.len();
            let mut perulangan = if total_atlets < 17 {
                1
            } else if total_atlets < 33 {
                2
            } else if total_atlets < 49 {
                3
            } else if total_atlets < 65 {
                4
            } else {
                1
            };

            if perulangan > 1 && perulangan % 2 != 0 {
                perulangan += 1;
            }

            categories_processed += 1;

            if perulangan == 1 {
                // Single bracket
                let bagan_code = format!("{:03}", next_code_num);
                next_code_num += 1;

                let bagan_res = sqlx::query(
                    "INSERT INTO backend_bagan (event_id, nomor_tanding_id, nama_bagan, pool, tipe_tanding, round_robin, is_bob, has_vr, kode) VALUES (?, ?, ?, ?, ?, 0, 0, ?, ?)"
                )
                .bind(event_id)
                .bind(cat_id)
                .bind(&cat_name)
                .bind(1)
                .bind(tipe_tanding)
                .bind(should_have_vr)
                .bind(&bagan_code)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal membuat bagan: {}", e))?;

                let bagan_id = bagan_res.last_insert_rowid();
                bagans_created += 1;

                let bracket_data = build_bracket_in_memory(&athletes, 1, should_have_vr);
                total_collisions += bracket_data.collision_count;

                for r in 1..=4 {
                    let max_u = match r { 1 => 8, 2 => 4, 3 => 2, _ => 1 };
                    for u in 1..=max_u {
                        let m = bracket_data.matches.get(&r).and_then(|round| round.get(&u)).cloned().unwrap_or_default();
                        sqlx::query(
                            "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team) VALUES (?, ?, ?, ?, ?, ?, ?, 0, 0, 0)"
                        )
                        .bind(bagan_id)
                        .bind(r as i32)
                        .bind(u as i32)
                        .bind(m.atlet1_id)
                        .bind(m.atlet2_id)
                        .bind(m.vr1)
                        .bind(m.vr2)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| format!("Gagal menyimpan detail bagan: {}", e))?;
                        total_matches += 1;
                    }
                }

                // Round 5 (Perebutan Juara 3)
                let m5 = bracket_data.matches.get(&5).and_then(|round| round.get(&1)).cloned().unwrap_or_default();
                sqlx::query(
                    "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team) VALUES (?, 5, 1, ?, ?, ?, ?, 0, 0, 0)"
                )
                .bind(bagan_id)
                .bind(m5.atlet1_id)
                .bind(m5.atlet2_id)
                .bind(m5.vr1)
                .bind(m5.vr2)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal menyimpan partai perebutan juara 3: {}", e))?;
                total_matches += 1;
            } else {
                // Multi-pool
                let pools_athletes = split_athletes_into_pools(&athletes, perulangan);

                // 1. Final bracket (pool 0)
                let final_name = format!("{} - Final", cat_name);
                let final_code = format!("{:03}", next_code_num);
                next_code_num += 1;

                let final_bagan_res = sqlx::query(
                    "INSERT INTO backend_bagan (event_id, nomor_tanding_id, nama_bagan, pool, tipe_tanding, round_robin, is_bob, has_vr, kode) VALUES (?, ?, ?, 0, ?, 0, 0, ?, ?)"
                )
                .bind(event_id)
                .bind(cat_id)
                .bind(&final_name)
                .bind(tipe_tanding)
                .bind(should_have_vr)
                .bind(&final_code)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal membuat bagan final: {}", e))?;

                let final_bagan_id = final_bagan_res.last_insert_rowid();
                bagans_created += 1;

                let empty_bracket = build_bracket_in_memory(&[], 0, should_have_vr);
                for r in 1..=4 {
                    let max_u = match r { 1 => 8, 2 => 4, 3 => 2, _ => 1 };
                    for u in 1..=max_u {
                        let m = empty_bracket.matches.get(&r).and_then(|round| round.get(&u)).cloned().unwrap_or_default();
                        sqlx::query(
                            "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team) VALUES (?, ?, ?, ?, ?, ?, ?, 0, 0, 0)"
                        )
                        .bind(final_bagan_id)
                        .bind(r as i32)
                        .bind(u as i32)
                        .bind(m.atlet1_id)
                        .bind(m.atlet2_id)
                        .bind(m.vr1)
                        .bind(m.vr2)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| format!("Gagal menyimpan detail bagan final: {}", e))?;
                        total_matches += 1;
                    }
                }
                let m5 = empty_bracket.matches.get(&5).and_then(|round| round.get(&1)).cloned().unwrap_or_default();
                sqlx::query(
                    "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team) VALUES (?, 5, 1, ?, ?, ?, ?, 0, 0, 0)"
                )
                .bind(final_bagan_id)
                .bind(m5.atlet1_id)
                .bind(m5.atlet2_id)
                .bind(m5.vr1)
                .bind(m5.vr2)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal menyimpan partai perebutan juara 3 final: {}", e))?;
                total_matches += 1;

                // 2. Pool brackets (Pool A, B, C, D)
                let pool_letters = ['A', 'B', 'C', 'D'];
                for i in 1..=perulangan {
                    let letter = pool_letters[i - 1];
                    let pool_name = format!("{} - Pool {}", cat_name, letter);
                    let pool_code = format!("{:03}", next_code_num);
                    next_code_num += 1;
                    let pool_athletes = &pools_athletes[i - 1];

                    let pool_bagan_res = sqlx::query(
                        "INSERT INTO backend_bagan (event_id, nomor_tanding_id, nama_bagan, pool, tipe_tanding, round_robin, is_bob, has_vr, kode) VALUES (?, ?, ?, ?, ?, 0, 0, ?, ?)"
                    )
                    .bind(event_id)
                    .bind(cat_id)
                    .bind(&pool_name)
                    .bind(perulangan as i32)
                    .bind(tipe_tanding)
                    .bind(should_have_vr)
                    .bind(&pool_code)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("Gagal membuat bagan pool: {}", e))?;

                    let pool_bagan_id = pool_bagan_res.last_insert_rowid();
                    bagans_created += 1;

                    let bracket_data = build_bracket_in_memory(pool_athletes, perulangan as i32, should_have_vr);
                    total_collisions += bracket_data.collision_count;

                    for r in 1..=4 {
                        let max_u = match r { 1 => 8, 2 => 4, 3 => 2, _ => 1 };
                        for u in 1..=max_u {
                            let m = bracket_data.matches.get(&r).and_then(|round| round.get(&u)).cloned().unwrap_or_default();
                            sqlx::query(
                                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team) VALUES (?, ?, ?, ?, ?, ?, ?, 0, 0, 0)"
                            )
                            .bind(pool_bagan_id)
                            .bind(r as i32)
                            .bind(u as i32)
                            .bind(m.atlet1_id)
                            .bind(m.atlet2_id)
                            .bind(m.vr1)
                            .bind(m.vr2)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| format!("Gagal menyimpan detail bagan pool: {}", e))?;
                            total_matches += 1;
                        }
                    }

                    let m5 = bracket_data.matches.get(&5).and_then(|round| round.get(&1)).cloned().unwrap_or_default();
                    sqlx::query(
                        "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team) VALUES (?, 5, 1, ?, ?, ?, ?, 0, 0, 0)"
                    )
                    .bind(pool_bagan_id)
                    .bind(m5.atlet1_id)
                    .bind(m5.atlet2_id)
                    .bind(m5.vr1)
                    .bind(m5.vr2)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("Gagal menyimpan perebutan juara 3 pool: {}", e))?;
                    total_matches += 1;
                }
            }
        }

        tx.commit().await.map_err(|e| format!("Gagal melakukan commit transaksi drawing: {}", e))?;

        Ok(DrawingSummary {
            categories_processed,
            bagans_created,
            total_matches,
            total_collisions,
        })
    }

    /// Saves a match result in detail_bagan and advances the winner in tournament brackets
    pub async fn save_match_result(&self, input: SaveMatchInput) -> Result<(), String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        // 1. Fetch the match being updated
        let current_match: (i64, i64, i64, i64, Option<i64>, Option<i64>) = sqlx::query_as(
            "SELECT id, bagan_id, round, urutan, atlet1_id, atlet2_id FROM backend_detailbagan WHERE id = ?"
        )
        .bind(input.match_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| format!("Partai dengan ID {} tidak ditemukan: {}", input.match_id, e))?;

        let (_match_id, bagan_id, round_num, urutan_num, a1_id, a2_id) = current_match;

        // Fetch bagan metadata to identify pool / multi-pool context
        let bagan_meta: Option<(i32, Option<i64>, Option<String>)> = sqlx::query_as(
            "SELECT pool, nomor_tanding_id, nama_bagan FROM backend_bagan WHERE id = ?"
        )
        .bind(bagan_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("Gagal memuat info bagan #{}: {}", bagan_id, e))?;

        let (bagan_pool, nomor_tanding_id, nama_bagan) = bagan_meta.unwrap_or((1, None, None));
        let nama_bagan_upper = nama_bagan.unwrap_or_default().to_uppercase();
        let is_pool_bracket = bagan_pool == 2 || nama_bagan_upper.contains("POOL A") || nama_bagan_upper.contains("POOL B") || nama_bagan_upper.contains("POOL 1") || nama_bagan_upper.contains("POOL 2");
        let is_pool_a = nama_bagan_upper.contains("POOL A") || nama_bagan_upper.contains("POOL 1");
        let _is_pool_b = nama_bagan_upper.contains("POOL B") || nama_bagan_upper.contains("POOL 2");
        let is_final_bagan = bagan_pool == 0 || nama_bagan_upper.ends_with("- FINAL") || nama_bagan_upper.ends_with("FINAL");

        // 2. Update current match in backend_detailbagan
        sqlx::query(
            r#"
            UPDATE backend_detailbagan
            SET score1 = ?, score2 = ?, scorekecil1 = ?, scorekecil2 = ?,
                pemenang = ?, kata1 = ?, kata2 = ?,
                vr1 = ?, vr2 = ?, hantei = ?, selesai = ?
            WHERE id = ?
            "#
        )
        .bind(&input.score1)
        .bind(&input.score2)
        .bind(&input.scorekecil1)
        .bind(&input.scorekecil2)
        .bind(&input.pemenang)
        .bind(&input.kata1)
        .bind(&input.kata2)
        .bind(input.vr1)
        .bind(input.vr2)
        .bind(input.hantei)
        .bind(input.selesai)
        .bind(input.match_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Gagal menyimpan partai: {}", e))?;

        // 3. Handle bracket advancement or reopening
        if input.selesai && input.pemenang.is_some() {
            let pemenang_str = input.pemenang.as_deref().unwrap_or("");
            let winner_id = match pemenang_str {
                "1" => a1_id,
                "2" => a2_id,
                _ => None,
            };
            let loser_id = match pemenang_str {
                "1" => a2_id,
                "2" => a1_id,
                _ => None,
            };

            if let Some(w_id) = winner_id {
                if is_pool_bracket && !is_final_bagan {
                    // Pool Preliminary Bracket (Pool A / Pool B):
                    if round_num < 4 {
                        // Advance winner within this pool bracket
                        let next_round = round_num + 1;
                        let next_urutan = (urutan_num + 1) / 2;
                        if urutan_num % 2 == 1 {
                            sqlx::query(
                                "UPDATE backend_detailbagan SET atlet1_id = ? WHERE bagan_id = ? AND round = ? AND urutan = ?"
                            )
                            .bind(w_id)
                            .bind(bagan_id)
                            .bind(next_round)
                            .bind(next_urutan)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| format!("Gagal memajukan pemenang pool: {}", e))?;
                        } else {
                            sqlx::query(
                                "UPDATE backend_detailbagan SET atlet2_id = ? WHERE bagan_id = ? AND round = ? AND urutan = ?"
                            )
                            .bind(w_id)
                            .bind(bagan_id)
                            .bind(next_round)
                            .bind(next_urutan)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| format!("Gagal memajukan pemenang pool: {}", e))?;
                        }
                    } else if round_num == 4 {
                        // Round 4 is the SEMIFINAL of this pool!
                        // Find the Final bagan of this category
                        let final_bagan_id: Option<i64> = if let Some(nt_id) = nomor_tanding_id {
                            sqlx::query_scalar(
                                "SELECT id FROM backend_bagan WHERE nomor_tanding_id = ? AND (pool = 0 OR UPPER(nama_bagan) LIKE '%- FINAL' OR UPPER(nama_bagan) LIKE '%FINAL') LIMIT 1"
                            )
                            .bind(nt_id)
                            .fetch_optional(&mut *tx)
                            .await
                            .unwrap_or(None)
                        } else {
                            None
                        };

                        if is_pool_a {
                            // Pool A winner -> Final match (Round 4 slot 1, atlet1)
                            if let Some(fb_id) = final_bagan_id {
                                sqlx::query(
                                    "UPDATE backend_detailbagan SET atlet1_id = ? WHERE bagan_id = ? AND round = 4 AND urutan = 1"
                                )
                                .bind(w_id)
                                .bind(fb_id)
                                .execute(&mut *tx)
                                .await
                                .map_err(|e| format!("Gagal memajukan pemenang Pool A ke final: {}", e))?;

                                // Pool A loser -> Juara 3A!
                                sqlx::query("UPDATE backend_bagan SET juara_3a_id = ? WHERE id = ?")
                                    .bind(loser_id)
                                    .bind(fb_id)
                                    .execute(&mut *tx)
                                    .await
                                    .map_err(|e| format!("Gagal memperbarui juara 3A pada final: {}", e))?;
                            }
                            let _ = sqlx::query("UPDATE backend_bagan SET juara_3a_id = ? WHERE id = ?")
                                .bind(loser_id)
                                .bind(bagan_id)
                                .execute(&mut *tx)
                                .await;
                        } else {
                            // Pool B (or subsequent pool) winner -> Final match (Round 4 slot 2, atlet2)
                            if let Some(fb_id) = final_bagan_id {
                                sqlx::query(
                                    "UPDATE backend_detailbagan SET atlet2_id = ? WHERE bagan_id = ? AND round = 4 AND urutan = 1"
                                )
                                .bind(w_id)
                                .bind(fb_id)
                                .execute(&mut *tx)
                                .await
                                .map_err(|e| format!("Gagal memajukan pemenang Pool B ke final: {}", e))?;

                                // Pool B loser -> Juara 3B!
                                sqlx::query("UPDATE backend_bagan SET juara_3b_id = ? WHERE id = ?")
                                    .bind(loser_id)
                                    .bind(fb_id)
                                    .execute(&mut *tx)
                                    .await
                                    .map_err(|e| format!("Gagal memperbarui juara 3B pada final: {}", e))?;
                            }
                            let _ = sqlx::query("UPDATE backend_bagan SET juara_3b_id = ? WHERE id = ?")
                                .bind(loser_id)
                                .bind(bagan_id)
                                .execute(&mut *tx)
                                .await;
                        }
                    }
                } else {
                    // Standard Single Pool or Final Bagan:
                    if round_num < 4 {
                        let next_round = round_num + 1;
                        let next_urutan = (urutan_num + 1) / 2;

                        if urutan_num % 2 == 1 {
                            sqlx::query(
                                "UPDATE backend_detailbagan SET atlet1_id = ? WHERE bagan_id = ? AND round = ? AND urutan = ?"
                            )
                            .bind(w_id)
                            .bind(bagan_id)
                            .bind(next_round)
                            .bind(next_urutan)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| format!("Gagal memajukan pemenang ke ronde berikutnya: {}", e))?;
                        } else {
                            sqlx::query(
                                "UPDATE backend_detailbagan SET atlet2_id = ? WHERE bagan_id = ? AND round = ? AND urutan = ?"
                            )
                            .bind(w_id)
                            .bind(bagan_id)
                            .bind(next_round)
                            .bind(next_urutan)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| format!("Gagal memajukan pemenang ke ronde berikutnya: {}", e))?;
                        }
                    } else if round_num == 4 {
                        // Final: Winner is Juara 1, Loser is Juara 2
                        sqlx::query(
                            "UPDATE backend_bagan SET juara_1_id = ?, juara_2_id = ? WHERE id = ?"
                        )
                        .bind(w_id)
                        .bind(loser_id)
                        .bind(bagan_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| format!("Gagal memperbarui juara 1 & 2: {}", e))?;

                        // If multi-pool category, also sync juara 1 & 2 to related pool bagans
                        if let Some(nt_id) = nomor_tanding_id {
                            let _ = sqlx::query(
                                "UPDATE backend_bagan SET juara_1_id = ?, juara_2_id = ? WHERE nomor_tanding_id = ?"
                            )
                            .bind(w_id)
                            .bind(loser_id)
                            .bind(nt_id)
                            .execute(&mut *tx)
                            .await;
                        }
                    }

                    // If round == 3 (Semifinal), loser becomes Juara 3A (semi 1) or Juara 3B (semi 2)
                    if round_num == 3 {
                        if urutan_num == 1 {
                            sqlx::query("UPDATE backend_bagan SET juara_3a_id = ? WHERE id = ?")
                                .bind(loser_id)
                                .bind(bagan_id)
                                .execute(&mut *tx)
                                .await
                                .map_err(|e| format!("Gagal memperbarui juara 3A: {}", e))?;
                        } else if urutan_num == 2 {
                            sqlx::query("UPDATE backend_bagan SET juara_3b_id = ? WHERE id = ?")
                                .bind(loser_id)
                                .bind(bagan_id)
                                .execute(&mut *tx)
                                .await
                                .map_err(|e| format!("Gagal memperbarui juara 3B: {}", e))?;
                        }
                    }
                }
            }
        } else if !input.selesai {
            // Match is re-opened or reset: if next match hasn't started, clear winner from next round
            if is_pool_bracket && !is_final_bagan {
                if round_num < 4 {
                    let next_round = round_num + 1;
                    let next_urutan = (urutan_num + 1) / 2;

                    if urutan_num % 2 == 1 {
                        let _ = sqlx::query(
                            "UPDATE backend_detailbagan SET atlet1_id = NULL WHERE bagan_id = ? AND round = ? AND urutan = ? AND selesai = 0"
                        )
                        .bind(bagan_id)
                        .bind(next_round)
                        .bind(next_urutan)
                        .execute(&mut *tx)
                        .await;
                    } else {
                        let _ = sqlx::query(
                            "UPDATE backend_detailbagan SET atlet2_id = NULL WHERE bagan_id = ? AND round = ? AND urutan = ? AND selesai = 0"
                        )
                        .bind(bagan_id)
                        .bind(next_round)
                        .bind(next_urutan)
                        .execute(&mut *tx)
                        .await;
                    }
                } else if round_num == 4 {
                    // Semifinal reset
                    let final_bagan_id: Option<i64> = if let Some(nt_id) = nomor_tanding_id {
                        sqlx::query_scalar(
                            "SELECT id FROM backend_bagan WHERE nomor_tanding_id = ? AND (pool = 0 OR UPPER(nama_bagan) LIKE '%- FINAL' OR UPPER(nama_bagan) LIKE '%FINAL') LIMIT 1"
                        )
                        .bind(nt_id)
                        .fetch_optional(&mut *tx)
                        .await
                        .unwrap_or(None)
                    } else {
                        None
                    };

                    if is_pool_a {
                        if let Some(fb_id) = final_bagan_id {
                            let _ = sqlx::query(
                                "UPDATE backend_detailbagan SET atlet1_id = NULL WHERE bagan_id = ? AND round = 4 AND urutan = 1 AND selesai = 0"
                            )
                            .bind(fb_id)
                            .execute(&mut *tx)
                            .await;

                            let _ = sqlx::query("UPDATE backend_bagan SET juara_3a_id = NULL WHERE id = ?")
                                .bind(fb_id)
                                .execute(&mut *tx)
                                .await;
                        }
                        let _ = sqlx::query("UPDATE backend_bagan SET juara_3a_id = NULL WHERE id = ?")
                            .bind(bagan_id)
                            .execute(&mut *tx)
                            .await;
                    } else {
                        if let Some(fb_id) = final_bagan_id {
                            let _ = sqlx::query(
                                "UPDATE backend_detailbagan SET atlet2_id = NULL WHERE bagan_id = ? AND round = 4 AND urutan = 1 AND selesai = 0"
                            )
                            .bind(fb_id)
                            .execute(&mut *tx)
                            .await;

                            let _ = sqlx::query("UPDATE backend_bagan SET juara_3b_id = NULL WHERE id = ?")
                                .bind(fb_id)
                                .execute(&mut *tx)
                                .await;
                        }
                        let _ = sqlx::query("UPDATE backend_bagan SET juara_3b_id = NULL WHERE id = ?")
                            .bind(bagan_id)
                            .execute(&mut *tx)
                            .await;
                    }
                }
            } else {
                // Standard Single Pool or Final Bagan
                if round_num < 4 {
                    let next_round = round_num + 1;
                    let next_urutan = (urutan_num + 1) / 2;

                    if urutan_num % 2 == 1 {
                        let _ = sqlx::query(
                            "UPDATE backend_detailbagan SET atlet1_id = NULL WHERE bagan_id = ? AND round = ? AND urutan = ? AND selesai = 0"
                        )
                        .bind(bagan_id)
                        .bind(next_round)
                        .bind(next_urutan)
                        .execute(&mut *tx)
                        .await;
                    } else {
                        let _ = sqlx::query(
                            "UPDATE backend_detailbagan SET atlet2_id = NULL WHERE bagan_id = ? AND round = ? AND urutan = ? AND selesai = 0"
                        )
                        .bind(bagan_id)
                        .bind(next_round)
                        .bind(next_urutan)
                        .execute(&mut *tx)
                        .await;
                    }
                }
                if round_num == 3 {
                    if urutan_num == 1 {
                        let _ = sqlx::query("UPDATE backend_bagan SET juara_3a_id = NULL WHERE id = ?")
                            .bind(bagan_id)
                            .execute(&mut *tx)
                            .await;
                    } else if urutan_num == 2 {
                        let _ = sqlx::query("UPDATE backend_bagan SET juara_3b_id = NULL WHERE id = ?")
                            .bind(bagan_id)
                            .execute(&mut *tx)
                            .await;
                    }
                } else if round_num == 4 {
                    let _ = sqlx::query(
                        "UPDATE backend_bagan SET juara_1_id = NULL, juara_2_id = NULL WHERE id = ?"
                    )
                    .bind(bagan_id)
                    .execute(&mut *tx)
                    .await;

                    if let Some(nt_id) = nomor_tanding_id {
                        let _ = sqlx::query(
                            "UPDATE backend_bagan SET juara_1_id = NULL, juara_2_id = NULL WHERE nomor_tanding_id = ?"
                        )
                        .bind(nt_id)
                        .execute(&mut *tx)
                        .await;
                    }
                }
            }
        }

        tx.commit().await.map_err(|e| format!("Gagal melakukan commit transaksi: {}", e))?;
        Ok(())
    }

    /// Fetches all enriched athletes belonging to the bagan's category (nomor_tanding).
    pub async fn get_athletes_for_bagan(&self, bagan_id: i64) -> Result<Vec<EnrichedAtlet>, String> {
        let pool = self.get_pool()?;

        let nomor_tanding_id: Option<Option<i64>> = sqlx::query_scalar(
            "SELECT nomor_tanding_id FROM backend_bagan WHERE id = ?"
        )
        .bind(bagan_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Gagal membaca bagan #{}: {}", bagan_id, e))?;

        let nt_id = match nomor_tanding_id {
            Some(Some(id)) => id,
            _ => return Ok(vec![]),
        };

        sqlx::query_as::<_, EnrichedAtlet>(
            r#"
            SELECT 
                a.id, a.event_id, a.nama_atlet, a.nik, a.kode_atlet, a.additional_code, a.is_priority,
                a.perguruan_id, p.nama_perguruan as perguruan_nama,
                a.utusan_id, u.nama_utusan as utusan_nama, u.logo as utusan_logo,
                a.nomor_tanding_id, nt.nama_nomor_tanding as nomor_tanding_nama,
                CASE 
                    WHEN nt.is_bob = 1 THEN 'BOB'
                    WHEN UPPER(nt.nama_nomor_tanding) LIKE '%FESTIVAL%' THEN 'Festival'
                    ELSE 'Tournament'
                END as nomor_tanding_tipe,
                nt.is_bob
            FROM backend_atlet a
            LEFT JOIN backend_perguruan p ON a.perguruan_id = p.id
            LEFT JOIN backend_utusan u ON a.utusan_id = u.id
            LEFT JOIN backend_nomortanding nt ON a.nomor_tanding_id = nt.id
            WHERE a.nomor_tanding_id = ?
              AND (a.additional_code IS NULL OR a.additional_code NOT LIKE 'MEMBER_%')
            ORDER BY a.nama_atlet ASC
            "#
        )
        .bind(nt_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat atlet nomor tanding #{}: {}", nt_id, e))
    }

    /// Updates bagan name and match participant athletes (atlet1_id & atlet2_id).
    pub async fn update_bagan_details(
        &self,
        bagan_id: i64,
        nama_bagan: Option<String>,
        matchups: Vec<UpdateMatchAthletesInput>,
    ) -> Result<(), String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        // 1. Update nama_bagan if provided
        if let Some(name) = nama_bagan {
            let trimmed = name.trim();
            if !trimmed.is_empty() {
                sqlx::query("UPDATE backend_bagan SET nama_bagan = ? WHERE id = ?")
                    .bind(trimmed)
                    .bind(bagan_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("Gagal memperbarui nama bagan: {}", e))?;
            }
        }

        // 2. Update each matchup's atlet1_id and atlet2_id
        for m in matchups {
            sqlx::query(
                "UPDATE backend_detailbagan SET atlet1_id = ?, atlet2_id = ? WHERE id = ? AND bagan_id = ?"
            )
            .bind(m.atlet1_id)
            .bind(m.atlet2_id)
            .bind(m.match_id)
            .bind(bagan_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal memperbarui atlet partai #{}: {}", m.match_id, e))?;
        }

        tx.commit().await.map_err(|e| format!("Gagal menyimpan perubahan bagan: {}", e))?;
        Ok(())
    }
}
