use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::time::Duration;
use tracing::info;

use crate::bracket::{build_bracket_in_memory, split_athletes_into_pools, AthleteInput, generate_round_robin_schedule};
use crate::models::{
    AthleteConflictDetail, Atlet, AthleteMutationInput, Bagan, BaganPodium, CategoryMutationInput,
    DrawingCategoryInfo, DrawingSummary, EnrichedAtlet, EnrichedDetailBagan, EnrichedNomorTanding,
    EnrichedPerguruan, EnrichedUtusan,
    EnrichedTimetableCell, EnrichedTimetableDay, EnrichedTimetableRow, Event, FestivalAthlete,
    FestivalCategory, FestivalMatch, FestivalRosterData, FestivalRosterState, FestivalTatamiSection,
    KopSurat, NomorTanding, Perguruan, PodiumAthlete, SaveMatchInput, SaveTimetablePayload,
    StandingItem, SummaryBaganItem, SummaryBaganResult, SummaryBaganWinner, SummaryJuaraResult,
    SummaryWinnerAthlete, Tatami, TatamiBookletCategory, TatamiBookletData, TatamiLiveStatus,
    TimetableCategoryOption, TimetableCell, TimetableDay, TimetableEditorData, TimetableProgressSummary,
    TimetableRow, UpdateMatchAthletesInput, Utusan,
    AssignBoutAthletesInput, TeamBoutItem, TeamCandidateAthlete, TeamMatchInfo,
    RepechageInfo, RepechageResult,
    RoundRobinAthleteStanding, RoundRobinData, RoundRobinMatrixCell,
    AssignWasitTatamiInput, AutoAssignPanelInput, EnrichedWasit, EnrichedWasitTatami, SaveWasitInput,
    TatamiPanelOverview,
    MarshallingMatchItem, MarshallingOverview, TatamiQueue,
    ResultsBookletBeritaAcara, ResultsBookletData, ResultsBookletEventInfo, ResultsBookletSignatory,
    BobBaganResult, BobSourceCategory, CreateBobBaganInput,
};
use crate::referee::evaluate_panel_conflicts;

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

        if let Some(p) = &self.pool {
            let _ = crate::sync::SyncService::ensure_sync_queue_table(p).await;
            Self::ensure_performance_indexes(p).await;
            Self::auto_heal_legacy_master_data(p).await;
        }

        Ok(())
    }

    /// Creates high-performance SQLite indexes for high-frequency queries
    async fn ensure_performance_indexes(pool: &Pool<Sqlite>) {
        let index_queries = [
            "CREATE INDEX IF NOT EXISTS idx_detailbagan_bagan_round_urutan ON backend_detailbagan (bagan_id, round, urutan);",
            "CREATE INDEX IF NOT EXISTS idx_detailbagan_tatami_selesai ON backend_detailbagan (assigned_tatami_id, selesai);",
            "CREATE INDEX IF NOT EXISTS idx_bagan_event_nomortanding ON backend_bagan (event_id, nomor_tanding_id);",
            "CREATE INDEX IF NOT EXISTS idx_atlet_event_nomortanding ON backend_atlet (event_id, nomor_tanding_id);",
            "CREATE INDEX IF NOT EXISTS idx_atlet_utusan ON backend_atlet (utusan_id);",
            "CREATE INDEX IF NOT EXISTS idx_atlet_perguruan ON backend_atlet (perguruan_id);",
            "CREATE INDEX IF NOT EXISTS idx_wasittatami_tatami_wasit ON backend_wasittatami (tatami_id, wasit_id);",
            "CREATE INDEX IF NOT EXISTS idx_syncqueue_synced ON backend_syncqueue (is_synced, created_at);",
            "CREATE INDEX IF NOT EXISTS idx_timetablecell_day_tatami ON backend_timetablecell (day_id, tatami_id);",
        ];

        for q in index_queries {
            let _ = sqlx::query(q).execute(pool).await;
        }
    }

    /// Auto-heals orphaned/unassigned master data records from older databases or imports
    async fn auto_heal_legacy_master_data(pool: &Pool<Sqlite>) {
        let _ = sqlx::query(
            "UPDATE backend_utusan 
             SET event_id = (SELECT a.event_id FROM backend_atlet a WHERE a.utusan_id = backend_utusan.id AND a.event_id IS NOT NULL LIMIT 1)
             WHERE event_id IS NULL AND id IN (SELECT DISTINCT utusan_id FROM backend_atlet WHERE utusan_id IS NOT NULL);"
        ).execute(pool).await;

        let _ = sqlx::query(
            "UPDATE backend_perguruan 
             SET event_id = (SELECT a.event_id FROM backend_atlet a WHERE a.perguruan_id = backend_perguruan.id AND a.event_id IS NOT NULL LIMIT 1)
             WHERE event_id IS NULL AND id IN (SELECT DISTINCT perguruan_id FROM backend_atlet WHERE perguruan_id IS NOT NULL);"
        ).execute(pool).await;
    }

    pub fn get_pool(&self) -> Result<&Pool<Sqlite>, String> {
        self.pool
            .as_ref()
            .ok_or_else(|| "Tidak ada database aktif yang dimuat.".to_string())
    }

    pub fn get_media_dir(&self) -> Option<&PathBuf> {
        self.active_media_path.as_ref()
    }

    pub fn get_pool_cloned(&self) -> Option<Pool<Sqlite>> {
        self.pool.clone()
    }

    pub fn get_active_db_path_cloned(&self) -> Option<PathBuf> {
        self.active_db_path.clone()
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
            r#"SELECT 
                t.id, 
                t.event_id, 
                t.tatami_number, 
                t.detail_bagan_id,
                b.nama_bagan
            FROM backend_tatami t
            LEFT JOIN backend_detailbagan db ON t.detail_bagan_id = db.id
            LEFT JOIN backend_bagan b ON db.bagan_id = b.id
            WHERE t.event_id = ? 
            ORDER BY t.tatami_number ASC"#
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
            nama_bagan: None,
        })
    }

    /// Deletes a tatami by ID, safely cascading references to avoid foreign key errors
    pub async fn delete_tatami(&self, tatami_id: i64) -> Result<(), String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        // 1. Bersihkan sel jadwal yang terikat ke tatami ini
        sqlx::query("DELETE FROM backend_timetablecell WHERE tatami_id = ?")
            .bind(tatami_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal membersihkan sel jadwal tatami: {}", e))?;

        // 2. Putuskan assigned_tatami_id pada detail bagan
        sqlx::query("UPDATE backend_detailbagan SET assigned_tatami_id = NULL WHERE assigned_tatami_id = ?")
            .bind(tatami_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal memutuskan penugasan tatami pada bagan: {}", e))?;

        // 3. Putuskan tatami_id pada wasit tatami
        sqlx::query("DELETE FROM backend_wasittatami WHERE tatami_id = ?")
            .bind(tatami_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghapus penugasan wasit tatami: {}", e))?;

        // 4. Putuskan tatami_id pada penugasan admin, juri, dan role
        sqlx::query("UPDATE backend_admintatami SET tatami_id = NULL WHERE tatami_id = ?")
            .bind(tatami_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal memperbarui admin tatami: {}", e))?;

        sqlx::query("UPDATE backend_jury SET tatami_id = NULL WHERE tatami_id = ?")
            .bind(tatami_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal memperbarui juri tatami: {}", e))?;

        sqlx::query("UPDATE backend_role SET tatami_id = NULL WHERE tatami_id = ?")
            .bind(tatami_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal memperbarui role tatami: {}", e))?;

        // 5. Putuskan hubungan detail_bagan_id pada tatami sebelum delete
        sqlx::query("UPDATE backend_tatami SET detail_bagan_id = NULL WHERE id = ?")
            .bind(tatami_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal mereset detail_bagan pada tatami: {}", e))?;

        // 6. Hapus tatami
        sqlx::query("DELETE FROM backend_tatami WHERE id = ?")
            .bind(tatami_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghapus tatami: {}", e))?;

        tx.commit().await.map_err(|e| format!("Gagal menyimpan transaksi penghapusan tatami: {}", e))?;
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

    /// Fetches all categories for an event with participant count, bracket count, and first bracket ID
    pub async fn get_enriched_categories(&self, event_id: i64) -> Result<Vec<EnrichedNomorTanding>, String> {
        let pool = self.get_pool()?;
        sqlx::query_as::<_, EnrichedNomorTanding>(
            r#"
            SELECT 
                nt.id,
                nt.event_id,
                nt.nama_nomor_tanding,
                COALESCE(nt.is_bob, 0) as is_bob,
                COALESCE(nt.has_vr, 0) as has_vr,
                (
                    SELECT COUNT(*) FROM backend_atlet a 
                    WHERE a.nomor_tanding_id = nt.id 
                      AND (a.additional_code IS NULL OR a.additional_code NOT LIKE 'MEMBER_%')
                ) as athlete_count,
                (
                    SELECT COUNT(*) FROM backend_bagan b 
                    WHERE b.nomor_tanding_id = nt.id
                ) as bagan_count,
                (
                    SELECT b.id FROM backend_bagan b
                    WHERE b.nomor_tanding_id = nt.id
                    ORDER BY b.id ASC
                    LIMIT 1
                ) as first_bagan_id
            FROM backend_nomortanding nt
            WHERE nt.event_id = ?
            ORDER BY nt.nama_nomor_tanding ASC
            "#
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat kategori nomor tanding: {}", e))
    }

    /// Creates a new competition category (Nomor Tanding)
    pub async fn create_category(&self, input: CategoryMutationInput) -> Result<EnrichedNomorTanding, String> {
        let pool = self.get_pool()?;
        let res = sqlx::query(
            "INSERT INTO backend_nomortanding (event_id, nama_nomor_tanding, is_bob, has_vr) VALUES (?, ?, ?, ?)"
        )
        .bind(input.event_id)
        .bind(&input.nama_nomor_tanding)
        .bind(input.is_bob)
        .bind(input.has_vr)
        .execute(pool)
        .await
        .map_err(|e| format!("Gagal menambahkan kategori: {}", e))?;

        let id = res.last_insert_rowid();
        Ok(EnrichedNomorTanding {
            id,
            event_id: Some(input.event_id),
            nama_nomor_tanding: Some(input.nama_nomor_tanding),
            is_bob: input.is_bob,
            has_vr: input.has_vr,
            athlete_count: 0,
            bagan_count: 0,
            first_bagan_id: None,
        })
    }

    /// Updates an existing competition category (Nomor Tanding)
    pub async fn update_category(&self, input: CategoryMutationInput) -> Result<EnrichedNomorTanding, String> {
        let cat_id = input.id.ok_or_else(|| "ID kategori tidak boleh kosong untuk pembaruan".to_string())?;
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        sqlx::query(
            "UPDATE backend_nomortanding SET nama_nomor_tanding = ?, is_bob = ?, has_vr = ? WHERE id = ?"
        )
        .bind(&input.nama_nomor_tanding)
        .bind(input.is_bob)
        .bind(input.has_vr)
        .bind(cat_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Gagal memperbarui kategori: {}", e))?;

        // Synchronize VR & is_bob to related bagans if existing
        sqlx::query(
            "UPDATE backend_bagan SET is_bob = ?, has_vr = ? WHERE nomor_tanding_id = ?"
        )
        .bind(input.is_bob)
        .bind(input.has_vr)
        .bind(cat_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Gagal menyinkronkan status ke bagan: {}", e))?;

        tx.commit().await.map_err(|e| format!("Gagal menyimpan transaksi: {}", e))?;

        // Query updated counts
        let enriched = sqlx::query_as::<_, EnrichedNomorTanding>(
            r#"
            SELECT 
                nt.id,
                nt.event_id,
                nt.nama_nomor_tanding,
                COALESCE(nt.is_bob, 0) as is_bob,
                COALESCE(nt.has_vr, 0) as has_vr,
                (
                    SELECT COUNT(*) FROM backend_atlet a 
                    WHERE a.nomor_tanding_id = nt.id 
                      AND (a.additional_code IS NULL OR a.additional_code NOT LIKE 'MEMBER_%')
                ) as athlete_count,
                (
                    SELECT COUNT(*) FROM backend_bagan b 
                    WHERE b.nomor_tanding_id = nt.id
                ) as bagan_count,
                (
                    SELECT b.id FROM backend_bagan b
                    WHERE b.nomor_tanding_id = nt.id
                    ORDER BY b.id ASC
                    LIMIT 1
                ) as first_bagan_id
            FROM backend_nomortanding nt
            WHERE nt.id = ?
            "#
        )
        .bind(cat_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Gagal memuat kategori yang diperbarui: {}", e))?;

        Ok(enriched)
    }

    /// Toggles Video Review (VR) status for a category and its associated brackets
    pub async fn toggle_category_vr(&self, category_id: i64, has_vr: bool) -> Result<(), String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        sqlx::query("UPDATE backend_nomortanding SET has_vr = ? WHERE id = ?")
            .bind(has_vr)
            .bind(category_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal memperbarui status VR kategori: {}", e))?;

        sqlx::query("UPDATE backend_bagan SET has_vr = ? WHERE nomor_tanding_id = ?")
            .bind(has_vr)
            .bind(category_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menyinkronkan status VR ke bagan: {}", e))?;

        tx.commit().await.map_err(|e| format!("Gagal menyimpan transaksi: {}", e))?;
        Ok(())
    }

    /// Deletes a competition category, safely unlinking athletes and cleaning up any related brackets
    pub async fn delete_category(&self, category_id: i64) -> Result<(), String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        // 1. Unassign tatamis
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
        sqlx::query("DELETE FROM backend_detailbagan WHERE bagan_id IN (SELECT id FROM backend_bagan WHERE nomor_tanding_id = ?)")
            .bind(category_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghapus detail bagan: {}", e))?;

        // 3. Delete bagans
        sqlx::query("DELETE FROM backend_bagan WHERE nomor_tanding_id = ?")
            .bind(category_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghapus bagan: {}", e))?;

        // 4. Detach athletes (set nomor_tanding_id = NULL so athlete records are safely preserved)
        sqlx::query("UPDATE backend_atlet SET nomor_tanding_id = NULL WHERE nomor_tanding_id = ?")
            .bind(category_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal melepaskan atlet dari kategori: {}", e))?;

        // 5. Delete category itself
        sqlx::query("DELETE FROM backend_nomortanding WHERE id = ?")
            .bind(category_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghapus kategori: {}", e))?;

        tx.commit().await.map_err(|e| format!("Gagal menyimpan perubahan: {}", e))?;
        Ok(())
    }

    /// Fetches all athletes belonging to a specific category
    pub async fn get_category_athletes(&self, category_id: i64) -> Result<Vec<EnrichedAtlet>, String> {
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
            WHERE a.nomor_tanding_id = ?
            ORDER BY a.nama_atlet ASC
            "#
        )
        .bind(category_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat atlet untuk kategori: {}", e))
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

    /// Fetches WKF repechage eligibility info, current status, and qualified athletes
    pub async fn get_repechage_info(&self, bagan_id: i64) -> Result<RepechageInfo, String> {
        let pool = self.get_pool()?;

        // 1. Fetch bagan info
        let bagan: (i64, Option<i64>, Option<String>, Option<i64>) = sqlx::query_as(
            "SELECT id, event_id, nama_bagan, nomor_tanding_id FROM backend_bagan WHERE id = ?"
        )
        .bind(bagan_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Bagan tidak ditemukan: {}", e))?;

        let (_b_id, _event_id, _nama_bagan, nomor_tanding_id) = bagan;

        // Check if an existing repechage bagan exists for this category
        let existing_repechage: Option<(i64, Option<String>)> = if let Some(nt_id) = nomor_tanding_id {
            sqlx::query_as(
                "SELECT id, nama_bagan FROM backend_bagan WHERE nomor_tanding_id = ? AND (UPPER(nama_bagan) LIKE '%REFERCHANGE%' OR UPPER(nama_bagan) LIKE '%REPECHAGE%') LIMIT 1"
            )
            .bind(nt_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None)
        } else {
            None
        };

        let (repechage_bagan_id, repechage_bagan_name) = match existing_repechage {
            Some((id, name)) => (Some(id), name),
            None => (None, None),
        };

        // 2. Fetch the Final match of this main bagan (highest round, urutan 1)
        let final_match: Option<(i64, i32, Option<i64>, Option<i64>)> = sqlx::query_as(
            "SELECT id, round, atlet1_id, atlet2_id FROM backend_detailbagan WHERE bagan_id = ? ORDER BY round DESC, urutan ASC LIMIT 1"
        )
        .bind(bagan_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Gagal memuat partai final: {}", e))?;

        let (_f_id, max_round, f1_id, f2_id) = match final_match {
            Some(m) => m,
            None => {
                return Ok(RepechageInfo {
                    main_bagan_id: bagan_id,
                    is_finalists_ready: false,
                    finalist1_id: None,
                    finalist1_name: None,
                    finalist2_id: None,
                    finalist2_name: None,
                    pool_a_losers: Vec::new(),
                    pool_b_losers: Vec::new(),
                    repechage_bagan_id,
                    repechage_bagan_name,
                    can_generate: false,
                })
            }
        };

        let is_finalists_ready = f1_id.is_some() && f2_id.is_some();
        let mut finalist1_name = None;
        let mut finalist2_name = None;
        let mut pool_a_losers = Vec::new();
        let mut pool_b_losers = Vec::new();

        if let Some(id1) = f1_id {
            let name: Option<String> = sqlx::query_scalar("SELECT nama_atlet FROM backend_atlet WHERE id = ?")
                .bind(id1)
                .fetch_optional(pool)
                .await
                .unwrap_or(None);
            finalist1_name = name;
        }

        if let Some(id2) = f2_id {
            let name: Option<String> = sqlx::query_scalar("SELECT nama_atlet FROM backend_atlet WHERE id = ?")
                .bind(id2)
                .fetch_optional(pool)
                .await
                .unwrap_or(None);
            finalist2_name = name;
        }

        if is_finalists_ready {
            let id1 = f1_id.unwrap();
            for r in 1..max_round {
                let row: Option<(Option<i64>, Option<i64>, Option<String>)> = sqlx::query_as(
                    "SELECT atlet1_id, atlet2_id, pemenang FROM backend_detailbagan WHERE bagan_id = ? AND round = ? AND (atlet1_id = ? OR atlet2_id = ?) AND selesai = 1 LIMIT 1"
                )
                .bind(bagan_id)
                .bind(r)
                .bind(id1)
                .bind(id1)
                .fetch_optional(pool)
                .await
                .unwrap_or(None);

                if let Some((a1, a2, pemenang)) = row {
                    let pemenang_str = pemenang.as_deref().unwrap_or("");
                    let loser_id = if pemenang_str == "1" && a1 == Some(id1) {
                        a2
                    } else if pemenang_str == "2" && a2 == Some(id1) {
                        a1
                    } else {
                        None
                    };

                    if let Some(lid) = loser_id {
                        let lname: Option<String> = sqlx::query_scalar("SELECT nama_atlet FROM backend_atlet WHERE id = ?")
                            .bind(lid)
                            .fetch_optional(pool)
                            .await
                            .unwrap_or(None);
                        if let Some(ln) = lname {
                            pool_a_losers.push(ln);
                        }
                    }
                }
            }

            let id2 = f2_id.unwrap();
            for r in 1..max_round {
                let row: Option<(Option<i64>, Option<i64>, Option<String>)> = sqlx::query_as(
                    "SELECT atlet1_id, atlet2_id, pemenang FROM backend_detailbagan WHERE bagan_id = ? AND round = ? AND (atlet1_id = ? OR atlet2_id = ?) AND selesai = 1 LIMIT 1"
                )
                .bind(bagan_id)
                .bind(r)
                .bind(id2)
                .bind(id2)
                .fetch_optional(pool)
                .await
                .unwrap_or(None);

                if let Some((a1, a2, pemenang)) = row {
                    let pemenang_str = pemenang.as_deref().unwrap_or("");
                    let loser_id = if pemenang_str == "1" && a1 == Some(id2) {
                        a2
                    } else if pemenang_str == "2" && a2 == Some(id2) {
                        a1
                    } else {
                        None
                    };

                    if let Some(lid) = loser_id {
                        let lname: Option<String> = sqlx::query_scalar("SELECT nama_atlet FROM backend_atlet WHERE id = ?")
                            .bind(lid)
                            .fetch_optional(pool)
                            .await
                            .unwrap_or(None);
                        if let Some(ln) = lname {
                            pool_b_losers.push(ln);
                        }
                    }
                }
            }
        }

        let can_generate = is_finalists_ready && (!pool_a_losers.is_empty() || !pool_b_losers.is_empty());

        Ok(RepechageInfo {
            main_bagan_id: bagan_id,
            is_finalists_ready,
            finalist1_id: f1_id,
            finalist1_name,
            finalist2_id: f2_id,
            finalist2_name,
            pool_a_losers,
            pool_b_losers,
            repechage_bagan_id,
            repechage_bagan_name,
            can_generate,
        })
    }

    /// Automatically generates WKF repechage bracket by tracing opponent ancestry of both finalists
    pub async fn generate_repechage_bracket(&self, main_bagan_id: i64) -> Result<RepechageResult, String> {
        let pool = self.get_pool()?;

        // 1. Fetch main bagan metadata
        let main_bagan: (i64, Option<i64>, Option<String>, Option<i64>, Option<String>, bool) = sqlx::query_as(
            "SELECT id, event_id, nama_bagan, nomor_tanding_id, tipe_tanding, has_vr FROM backend_bagan WHERE id = ?"
        )
        .bind(main_bagan_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Bagan utama #{} tidak ditemukan: {}", main_bagan_id, e))?;

        let (_b_id, event_id, nama_bagan_opt, nomor_tanding_id, tipe_tanding, has_vr) = main_bagan;
        let base_name = nama_bagan_opt.unwrap_or_else(|| "BAGAN".to_string());

        // 2. Fetch the Final match to get Finalist 1 and Finalist 2
        let final_match: Option<(i64, i32, Option<i64>, Option<i64>)> = sqlx::query_as(
            "SELECT id, round, atlet1_id, atlet2_id FROM backend_detailbagan WHERE bagan_id = ? ORDER BY round DESC, urutan ASC LIMIT 1"
        )
        .bind(main_bagan_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Gagal memuat partai final: {}", e))?;

        let (_f_id, max_round, f1_id_opt, f2_id_opt) = final_match
            .ok_or_else(|| "Partai final tidak ditemukan pada bagan ini.".to_string())?;

        let f1_id = f1_id_opt.ok_or_else(|| "Finalis 1 (Aka) belum ditentukan. Selesaikan semifinal terlebih dahulu.".to_string())?;
        let f2_id = f2_id_opt.ok_or_else(|| "Finalis 2 (Ao) belum ditentukan. Selesaikan semifinal terlebih dahulu.".to_string())?;

        // 3. Trace ancestor losers in round order
        let mut pool_a_losers: Vec<i64> = Vec::new();
        let mut pool_b_losers: Vec<i64> = Vec::new();

        for r in 1..max_round {
            let row: Option<(Option<i64>, Option<i64>, Option<String>)> = sqlx::query_as(
                "SELECT atlet1_id, atlet2_id, pemenang FROM backend_detailbagan WHERE bagan_id = ? AND round = ? AND (atlet1_id = ? OR atlet2_id = ?) AND selesai = 1 LIMIT 1"
            )
            .bind(main_bagan_id)
            .bind(r)
            .bind(f1_id)
            .bind(f1_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

            if let Some((a1, a2, pemenang)) = row {
                let pemenang_str = pemenang.as_deref().unwrap_or("");
                let loser_id = if pemenang_str == "1" && a1 == Some(f1_id) {
                    a2
                } else if pemenang_str == "2" && a2 == Some(f1_id) {
                    a1
                } else {
                    None
                };
                if let Some(lid) = loser_id {
                    pool_a_losers.push(lid);
                }
            }
        }

        for r in 1..max_round {
            let row: Option<(Option<i64>, Option<i64>, Option<String>)> = sqlx::query_as(
                "SELECT atlet1_id, atlet2_id, pemenang FROM backend_detailbagan WHERE bagan_id = ? AND round = ? AND (atlet1_id = ? OR atlet2_id = ?) AND selesai = 1 LIMIT 1"
            )
            .bind(main_bagan_id)
            .bind(r)
            .bind(f2_id)
            .bind(f2_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

            if let Some((a1, a2, pemenang)) = row {
                let pemenang_str = pemenang.as_deref().unwrap_or("");
                let loser_id = if pemenang_str == "1" && a1 == Some(f2_id) {
                    a2
                } else if pemenang_str == "2" && a2 == Some(f2_id) {
                    a1
                } else {
                    None
                };
                if let Some(lid) = loser_id {
                    pool_b_losers.push(lid);
                }
            }
        }

        if pool_a_losers.is_empty() && pool_b_losers.is_empty() {
            return Err("Tidak ditemukan atlet yang terkualifikasi untuk repechage. Pastikan partai penyisihan dan semifinal telah diisi skor pemenangnya.".to_string());
        }

        // 4. Build WKF repechage plan
        let plan = crate::bracket::build_wkf_repechage_plan(&pool_a_losers, &pool_b_losers);

        // 5. Database transaction: create or update repechage bagan
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        let rep_name = format!("{} - REFERCHANGE", base_name);

        let existing_rep_id: Option<i64> = if let Some(nt_id) = nomor_tanding_id {
            sqlx::query_scalar(
                "SELECT id FROM backend_bagan WHERE nomor_tanding_id = ? AND UPPER(nama_bagan) = UPPER(?) LIMIT 1"
            )
            .bind(nt_id)
            .bind(&rep_name)
            .fetch_optional(&mut *tx)
            .await
            .unwrap_or(None)
        } else {
            None
        };

        let repechage_bagan_id = match existing_rep_id {
            Some(id) => {
                sqlx::query("DELETE FROM backend_detailbagan WHERE bagan_id = ?")
                    .bind(id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("Gagal membersihkan partai repechage lama: {}", e))?;
                id
            }
            None => {
                let row: (i64,) = sqlx::query_as(
                    r#"
                    INSERT INTO backend_bagan (
                        event_id, nama_bagan, nomor_tanding_id, tipe_tanding,
                        round_robin, pool, is_bob, has_vr
                    )
                    VALUES (?, ?, ?, ?, 0, 3, 0, ?)
                    RETURNING id
                    "#
                )
                .bind(event_id)
                .bind(&rep_name)
                .bind(nomor_tanding_id)
                .bind(&tipe_tanding)
                .bind(has_vr)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| format!("Gagal membuat bagan repechage baru: {}", e))?;

                row.0
            }
        };

        // 6. Insert matches into backend_detailbagan
        let mut matches_created = 0;
        for m in &plan.matches {
            sqlx::query(
                r#"
                INSERT INTO backend_detailbagan (
                    bagan_id, round, urutan, atlet1_id, atlet2_id,
                    vr1, vr2, hantei, selesai, team
                )
                VALUES (?, ?, ?, ?, ?, 0, 0, 0, 0, 0)
                "#
            )
            .bind(repechage_bagan_id)
            .bind(m.round)
            .bind(m.urutan)
            .bind(m.atlet1_id)
            .bind(m.atlet2_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menyimpan partai repechage: {}", e))?;

            matches_created += 1;
        }

        // 7. Handle direct bronze if any pool only had 1 loser
        if let Some(direct_a) = plan.direct_bronze_a {
            sqlx::query("UPDATE backend_bagan SET juara_3a_id = ? WHERE id = ?")
                .bind(direct_a)
                .bind(repechage_bagan_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal menetapkan juara 3A langsung: {}", e))?;

            sqlx::query("UPDATE backend_bagan SET juara_3a_id = ? WHERE id = ?")
                .bind(direct_a)
                .bind(main_bagan_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal menyinkronkan juara 3A ke bagan utama: {}", e))?;
        }

        if let Some(direct_b) = plan.direct_bronze_b {
            sqlx::query("UPDATE backend_bagan SET juara_3b_id = ? WHERE id = ?")
                .bind(direct_b)
                .bind(repechage_bagan_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal menetapkan juara 3B langsung: {}", e))?;

            sqlx::query("UPDATE backend_bagan SET juara_3b_id = ? WHERE id = ?")
                .bind(direct_b)
                .bind(main_bagan_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal menyinkronkan juara 3B ke bagan utama: {}", e))?;
        }

        tx.commit().await.map_err(|e| format!("Gagal komit transaksi: {}", e))?;

        Ok(RepechageResult {
            repechage_bagan_id,
            nama_bagan: rep_name,
            matches_created,
            pool_a_count: pool_a_losers.len(),
            pool_b_count: pool_b_losers.len(),
            message: format!(
                "Bagan Referchange berhasil dibuat dengan {} partai (Pool A: {} atlet, Pool B: {} atlet).",
                matches_created, pool_a_losers.len(), pool_b_losers.len()
            ),
        })
    }

    /// Creates a round-robin tournament bracket for a small category (3-5 athletes)
    pub async fn create_round_robin_bagan(&self, event_id: i64, nomor_tanding_id: i64) -> Result<i64, String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        // 1. Fetch category info
        #[derive(sqlx::FromRow)]
        struct CatInfo {
            nama_nomor_tanding: Option<String>,
            has_vr: bool,
        }
        let cat: Option<CatInfo> = sqlx::query_as(
            "SELECT nama_nomor_tanding, has_vr FROM backend_nomortanding WHERE id = ?"
        )
        .bind(nomor_tanding_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("Gagal memeriksa nomor tanding: {}", e))?;

        let cat = cat.ok_or_else(|| "Nomor tanding tidak ditemukan".to_string())?;
        let cat_name = cat.nama_nomor_tanding.unwrap_or_else(|| "Kategori".to_string());
        let is_kumite = cat_name.to_uppercase().contains("KUMITE");
        let tipe_tanding = if is_kumite { "2" } else { "1" };
        let should_have_vr = is_kumite && cat.has_vr;

        // 2. Fetch athletes in this category
        #[derive(sqlx::FromRow)]
        struct RawId {
            id: i64,
        }
        let athletes: Vec<RawId> = sqlx::query_as(
            r#"
            SELECT id FROM backend_atlet
            WHERE nomor_tanding_id = ?
              AND (additional_code IS NULL OR additional_code NOT LIKE 'MEMBER_%')
            ORDER BY is_priority DESC, id ASC
            "#
        )
        .bind(nomor_tanding_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| format!("Gagal memuat atlet: {}", e))?;

        if athletes.len() < 2 {
            return Err("Kategori membutuhkan minimal 2 atau 3 atlet untuk sistem Round-Robin.".to_string());
        }

        let athlete_ids: Vec<i64> = athletes.into_iter().map(|a| a.id).collect();

        // 3. Check for existing codes to assign unique sequential bagan code
        let existing_codes: Vec<String> = sqlx::query_scalar(
            "SELECT kode FROM backend_bagan WHERE event_id = ? AND kode IS NOT NULL AND kode != ''"
        )
        .bind(event_id)
        .fetch_all(&mut *tx)
        .await
        .unwrap_or_default();

        let next_code_num: u32 = existing_codes
            .iter()
            .filter_map(|c| c.parse::<u32>().ok())
            .max()
            .unwrap_or(0) + 1;
        let bagan_code = format!("{:03}", next_code_num);
        let round_robin_name = format!("{} (ROUND-ROBIN)", cat_name);

        // 4. Create round-robin bagan
        let bagan_res = sqlx::query(
            r#"
            INSERT INTO backend_bagan (
                event_id, nomor_tanding_id, nama_bagan, pool, tipe_tanding,
                round_robin, is_bob, has_vr, kode
            )
            VALUES (?, ?, ?, 0, ?, 1, 0, ?, ?)
            "#
        )
        .bind(event_id)
        .bind(nomor_tanding_id)
        .bind(&round_robin_name)
        .bind(tipe_tanding)
        .bind(should_have_vr)
        .bind(&bagan_code)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Gagal membuat bagan round-robin: {}", e))?;

        let bagan_id = bagan_res.last_insert_rowid();

        // 5. Generate schedule using Berger algorithm
        let schedule = generate_round_robin_schedule(&athlete_ids);

        // 6. Insert matches into backend_detailbagan
        for m in schedule {
            sqlx::query(
                r#"
                INSERT INTO backend_detailbagan (
                    bagan_id, round, urutan, atlet1_id, atlet2_id,
                    vr1, vr2, hantei, selesai, team
                )
                VALUES (?, ?, ?, ?, ?, ?, ?, 0, 0, 0)
                "#
            )
            .bind(bagan_id)
            .bind(m.round)
            .bind(m.urutan)
            .bind(m.atlet1_id)
            .bind(m.atlet2_id)
            .bind(if should_have_vr { 1 } else { 0 })
            .bind(if should_have_vr { 1 } else { 0 })
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menyimpan partai round-robin: {}", e))?;
        }

        tx.commit().await.map_err(|e| format!("Gagal komit transaksi: {}", e))?;
        Ok(bagan_id)
    }

    /// Calculates round-robin standings and cross-table matrix
    pub fn calculate_round_robin_matrix_and_standings(
        &self,
        athletes: &[EnrichedAtlet],
        matches: &[EnrichedDetailBagan],
    ) -> (Vec<RoundRobinAthleteStanding>, Vec<Vec<RoundRobinMatrixCell>>) {
        let mut standings_map: std::collections::HashMap<i64, RoundRobinAthleteStanding> = athletes
            .iter()
            .map(|a| {
                (
                    a.id,
                    RoundRobinAthleteStanding {
                        athlete_id: a.id,
                        athlete_name: a.nama_atlet.clone().unwrap_or_default(),
                        utusan_name: a.utusan_nama.clone().unwrap_or_default(),
                        perguruan_name: a.perguruan_nama.clone().unwrap_or_default(),
                        logo: a.utusan_logo.clone(),
                        played: 0,
                        won: 0,
                        lost: 0,
                        drawn: 0,
                        points_for: 0,
                        points_against: 0,
                        points_diff: 0,
                        standings_points: 0.0,
                        rank: 0,
                    },
                )
            })
            .collect();

        // 1. Process match results
        for m in matches {
            if !m.selesai {
                continue;
            }
            let (Some(a1_id), Some(a2_id)) = (m.atlet1_id, m.atlet2_id) else {
                continue;
            };

            let s1: i32 = m.score1.as_deref().and_then(|s| s.parse().ok()).unwrap_or(0);
            let s2: i32 = m.score2.as_deref().and_then(|s| s.parse().ok()).unwrap_or(0);

            if let Some(entry1) = standings_map.get_mut(&a1_id) {
                entry1.played += 1;
                entry1.points_for += s1;
                entry1.points_against += s2;
            }
            if let Some(entry2) = standings_map.get_mut(&a2_id) {
                entry2.played += 1;
                entry2.points_for += s2;
                entry2.points_against += s1;
            }

            let is_a1_win = match m.pemenang.as_deref() {
                Some("1") => true,
                Some("2") => false,
                _ => s1 > s2,
            };
            let is_a2_win = match m.pemenang.as_deref() {
                Some("2") => true,
                Some("1") => false,
                _ => s2 > s1,
            };

            if is_a1_win {
                if let Some(entry1) = standings_map.get_mut(&a1_id) {
                    entry1.won += 1;
                    entry1.standings_points += 3.0;
                }
                if let Some(entry2) = standings_map.get_mut(&a2_id) {
                    entry2.lost += 1;
                }
            } else if is_a2_win {
                if let Some(entry2) = standings_map.get_mut(&a2_id) {
                    entry2.won += 1;
                    entry2.standings_points += 3.0;
                }
                if let Some(entry1) = standings_map.get_mut(&a1_id) {
                    entry1.lost += 1;
                }
            } else {
                // Draw
                if let Some(entry1) = standings_map.get_mut(&a1_id) {
                    entry1.drawn += 1;
                    entry1.standings_points += 1.0;
                }
                if let Some(entry2) = standings_map.get_mut(&a2_id) {
                    entry2.drawn += 1;
                    entry2.standings_points += 1.0;
                }
            }
        }

        // Calculate points_diff
        for entry in standings_map.values_mut() {
            entry.points_diff = entry.points_for - entry.points_against;
        }

        let mut standings: Vec<RoundRobinAthleteStanding> = standings_map.into_values().collect();

        // 2. Sort by tiebreakers
        standings.sort_by(|a, b| {
            if (b.standings_points - a.standings_points).abs() > f32::EPSILON {
                return b.standings_points.partial_cmp(&a.standings_points).unwrap_or(std::cmp::Ordering::Equal);
            }

            // Head-to-head check between a and b
            let h2h = matches.iter().find(|m| {
                m.selesai
                    && ((m.atlet1_id == Some(a.athlete_id) && m.atlet2_id == Some(b.athlete_id))
                        || (m.atlet1_id == Some(b.athlete_id) && m.atlet2_id == Some(a.athlete_id)))
            });

            if let Some(m) = h2h {
                let a_is_atlet1 = m.atlet1_id == Some(a.athlete_id);
                let winner_was_a = match m.pemenang.as_deref() {
                    Some("1") => a_is_atlet1,
                    Some("2") => !a_is_atlet1,
                    _ => {
                        let s1: i32 = m.score1.as_deref().and_then(|s| s.parse().ok()).unwrap_or(0);
                        let s2: i32 = m.score2.as_deref().and_then(|s| s.parse().ok()).unwrap_or(0);
                        if a_is_atlet1 { s1 > s2 } else { s2 > s1 }
                    }
                };
                let winner_was_b = match m.pemenang.as_deref() {
                    Some("1") => !a_is_atlet1,
                    Some("2") => a_is_atlet1,
                    _ => {
                        let s1: i32 = m.score1.as_deref().and_then(|s| s.parse().ok()).unwrap_or(0);
                        let s2: i32 = m.score2.as_deref().and_then(|s| s.parse().ok()).unwrap_or(0);
                        if a_is_atlet1 { s2 > s1 } else { s1 > s2 }
                    }
                };

                if winner_was_a {
                    return std::cmp::Ordering::Less;
                } else if winner_was_b {
                    return std::cmp::Ordering::Greater;
                }
            }

            if b.points_diff != a.points_diff {
                return b.points_diff.cmp(&a.points_diff);
            }

            if b.points_for != a.points_for {
                return b.points_for.cmp(&a.points_for);
            }

            a.athlete_id.cmp(&b.athlete_id)
        });

        // Assign ranks
        for (idx, item) in standings.iter_mut().enumerate() {
            item.rank = (idx + 1) as i32;
        }

        // 3. Construct cross-table matrix rows
        let mut matrix_rows: Vec<Vec<RoundRobinMatrixCell>> = Vec::new();
        for row_a in &standings {
            let mut row_cells: Vec<RoundRobinMatrixCell> = Vec::new();
            for col_a in &standings {
                if row_a.athlete_id == col_a.athlete_id {
                    row_cells.push(RoundRobinMatrixCell {
                        match_id: None,
                        round: 0,
                        urutan: 0,
                        row_athlete_id: row_a.athlete_id,
                        col_athlete_id: col_a.athlete_id,
                        is_self: true,
                        is_finished: false,
                        winner_id: None,
                        row_score: None,
                        col_score: None,
                        row_result: None,
                        assigned_tatami_id: None,
                    });
                } else {
                    let match_opt = matches.iter().find(|m| {
                        (m.atlet1_id == Some(row_a.athlete_id) && m.atlet2_id == Some(col_a.athlete_id))
                            || (m.atlet1_id == Some(col_a.athlete_id) && m.atlet2_id == Some(row_a.athlete_id))
                    });

                    if let Some(m) = match_opt {
                        let is_row_a1 = m.atlet1_id == Some(row_a.athlete_id);
                        let row_score = if is_row_a1 { m.score1.clone() } else { m.score2.clone() };
                        let col_score = if is_row_a1 { m.score2.clone() } else { m.score1.clone() };
                        let row_winner_id = match m.pemenang.as_deref() {
                            Some("1") => m.atlet1_id,
                            Some("2") => m.atlet2_id,
                            _ => None,
                        };

                        let row_result = if m.selesai {
                            if row_winner_id == Some(row_a.athlete_id) {
                                Some("W".to_string())
                            } else if row_winner_id == Some(col_a.athlete_id) {
                                Some("L".to_string())
                            } else {
                                Some("D".to_string())
                            }
                        } else {
                            None
                        };

                        row_cells.push(RoundRobinMatrixCell {
                            match_id: Some(m.id),
                            round: m.round.unwrap_or(1),
                            urutan: m.urutan.unwrap_or(1),
                            row_athlete_id: row_a.athlete_id,
                            col_athlete_id: col_a.athlete_id,
                            is_self: false,
                            is_finished: m.selesai,
                            winner_id: row_winner_id,
                            row_score,
                            col_score,
                            row_result,
                            assigned_tatami_id: m.assigned_tatami_id,
                        });
                    } else {
                        row_cells.push(RoundRobinMatrixCell {
                            match_id: None,
                            round: 0,
                            urutan: 0,
                            row_athlete_id: row_a.athlete_id,
                            col_athlete_id: col_a.athlete_id,
                            is_self: false,
                            is_finished: false,
                            winner_id: None,
                            row_score: None,
                            col_score: None,
                            row_result: None,
                            assigned_tatami_id: None,
                        });
                    }
                }
            }
            matrix_rows.push(row_cells);
        }

        (standings, matrix_rows)
    }

    /// Fetches all data for a round-robin bagan including standings, matrix, and podium
    pub async fn get_round_robin_data(&self, bagan_id: i64) -> Result<RoundRobinData, String> {
        let pool = self.get_pool()?;

        #[derive(sqlx::FromRow)]
        #[allow(dead_code)]
        struct BaganRow {
            id: i64,
            nama_bagan: String,
            nomor_tanding_id: Option<i64>,
            round_robin: bool,
        }

        let bagan: Option<BaganRow> = sqlx::query_as(
            "SELECT id, nama_bagan, nomor_tanding_id, round_robin FROM backend_bagan WHERE id = ?"
        )
        .bind(bagan_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Gagal memuat bagan: {}", e))?;

        let bagan = bagan.ok_or_else(|| "Bagan tidak ditemukan".to_string())?;

        let nomor_tanding_name = if let Some(nt_id) = bagan.nomor_tanding_id {
            sqlx::query_scalar("SELECT nama_nomor_tanding FROM backend_nomortanding WHERE id = ?")
                .bind(nt_id)
                .fetch_optional(pool)
                .await
                .unwrap_or(None)
        } else {
            None
        };

        let matches = self.get_detail_bagan(bagan_id).await?;
        let total_matches = matches.len();
        let finished_matches = matches.iter().filter(|m| m.selesai).count();
        let is_completed = total_matches > 0 && finished_matches == total_matches;

        // Fetch athletes for this bagan
        let athletes = self.get_athletes_for_bagan(bagan_id).await.unwrap_or_default();

        let (standings, matrix_rows) = self.calculate_round_robin_matrix_and_standings(&athletes, &matches);
        let podium = self.get_bagan_podium(bagan_id).await?;

        Ok(RoundRobinData {
            bagan_id,
            nama_bagan: bagan.nama_bagan,
            nomor_tanding_id: bagan.nomor_tanding_id,
            nomor_tanding_name,
            athletes,
            standings,
            matrix_rows,
            total_matches,
            finished_matches,
            is_completed,
            podium,
        })
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

    /// Fetches the currently active match for a specific tatami number
    pub async fn get_tatami_current_match(&self, tatami_num: i32) -> Result<Option<EnrichedDetailBagan>, String> {
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
            JOIN backend_tatami t ON t.detail_bagan_id = d.id
            LEFT JOIN backend_atlet a1 ON d.atlet1_id = a1.id
            LEFT JOIN backend_utusan u1 ON a1.utusan_id = u1.id
            LEFT JOIN backend_perguruan p1 ON a1.perguruan_id = p1.id
            LEFT JOIN backend_atlet a2 ON d.atlet2_id = a2.id
            LEFT JOIN backend_utusan u2 ON a2.utusan_id = u2.id
            LEFT JOIN backend_perguruan p2 ON a2.perguruan_id = p2.id
            WHERE t.tatami_number = ?
            LIMIT 1
            "#
        )
        .bind(tatami_num)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Gagal memuat partai aktif tatami: {}", e))
    }

    /// Fetches all matches assigned to a specific tatami number
    pub async fn get_tatami_matches(&self, tatami_num: i32) -> Result<Vec<EnrichedDetailBagan>, String> {
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
            JOIN backend_tatami t ON (d.assigned_tatami_id = t.id OR d.assigned_tatami_id = t.tatami_number)
            LEFT JOIN backend_atlet a1 ON d.atlet1_id = a1.id
            LEFT JOIN backend_utusan u1 ON a1.utusan_id = u1.id
            LEFT JOIN backend_perguruan p1 ON a1.perguruan_id = p1.id
            LEFT JOIN backend_atlet a2 ON d.atlet2_id = a2.id
            LEFT JOIN backend_utusan u2 ON a2.utusan_id = u2.id
            LEFT JOIN backend_perguruan p2 ON a2.perguruan_id = p2.id
            WHERE t.tatami_number = ?
            ORDER BY d.round ASC, d.urutan ASC
            "#
        )
        .bind(tatami_num)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat daftar partai tatami: {}", e))
    }

    /// Saves assigned Kata names for a match
    pub async fn save_match_kata(&self, match_id: i64, kata1: Option<String>, kata2: Option<String>) -> Result<(), String> {
        let pool = self.get_pool()?;
        sqlx::query(
            "UPDATE backend_detailbagan SET kata1 = ?, kata2 = ? WHERE id = ?"
        )
        .bind(kata1)
        .bind(kata2)
        .bind(match_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Gagal menyimpan kata pertandingan: {}", e))?;

        Ok(())
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
            "SELECT id, event_id, nama_perguruan FROM backend_perguruan 
             WHERE event_id = ? 
                OR id IN (SELECT DISTINCT perguruan_id FROM backend_atlet WHERE event_id = ? AND perguruan_id IS NOT NULL)
             ORDER BY nama_perguruan ASC"
        )
        .bind(event_id)
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat daftar perguruan: {}", e))
    }

    /// Fetches all contingents/clubs for an event
    pub async fn get_contingents(&self, event_id: i64) -> Result<Vec<Utusan>, String> {
        let pool = self.get_pool()?;
        sqlx::query_as::<_, Utusan>(
            "SELECT id, event_id, nama_utusan, logo FROM backend_utusan 
             WHERE event_id = ? 
                OR id IN (SELECT DISTINCT utusan_id FROM backend_atlet WHERE event_id = ? AND utusan_id IS NOT NULL)
             ORDER BY nama_utusan ASC"
        )
        .bind(event_id)
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat daftar kontingen: {}", e))
    }

    /// Fetches all contingents for an event with athlete counts
    pub async fn get_enriched_contingents(&self, event_id: i64) -> Result<Vec<EnrichedUtusan>, String> {
        let pool = self.get_pool()?;
        sqlx::query_as::<_, EnrichedUtusan>(
            "SELECT u.id, u.event_id, u.nama_utusan, u.logo, COUNT(a.id) as athlete_count
             FROM backend_utusan u
             LEFT JOIN backend_atlet a ON a.utusan_id = u.id AND a.event_id = ?
             WHERE u.event_id = ?
                OR u.id IN (SELECT DISTINCT utusan_id FROM backend_atlet WHERE event_id = ? AND utusan_id IS NOT NULL)
             GROUP BY u.id, u.event_id, u.nama_utusan, u.logo
             ORDER BY u.nama_utusan ASC"
        )
        .bind(event_id)
        .bind(event_id)
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat kontingen: {}", e))
    }

    /// Creates a new contingent
    pub async fn create_contingent(&self, event_id: i64, nama_utusan: String, logo: Option<String>) -> Result<EnrichedUtusan, String> {
        let pool = self.get_pool()?;
        let clean_name = nama_utusan.trim().to_uppercase();
        if clean_name.is_empty() {
            return Err("Nama kontingen tidak boleh kosong".into());
        }

        let res = sqlx::query("INSERT INTO backend_utusan (event_id, nama_utusan, logo) VALUES (?, ?, ?)")
            .bind(event_id)
            .bind(&clean_name)
            .bind(&logo)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal menambahkan kontingen: {}", e))?;

        let id = res.last_insert_rowid();
        Ok(EnrichedUtusan {
            id,
            event_id: Some(event_id),
            nama_utusan: Some(clean_name),
            logo,
            athlete_count: 0,
        })
    }

    /// Updates an existing contingent
    pub async fn update_contingent(&self, id: i64, nama_utusan: String, logo: Option<String>) -> Result<EnrichedUtusan, String> {
        let pool = self.get_pool()?;
        let clean_name = nama_utusan.trim().to_uppercase();
        if clean_name.is_empty() {
            return Err("Nama kontingen tidak boleh kosong".into());
        }

        if let Some(ref l) = logo {
            sqlx::query("UPDATE backend_utusan SET nama_utusan = ?, logo = ? WHERE id = ?")
                .bind(&clean_name)
                .bind(l)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| format!("Gagal memperbarui kontingen: {}", e))?;
        } else {
            sqlx::query("UPDATE backend_utusan SET nama_utusan = ? WHERE id = ?")
                .bind(&clean_name)
                .bind(id)
                .execute(pool)
                .await
                .map_err(|e| format!("Gagal memperbarui kontingen: {}", e))?;
        }

        sqlx::query_as::<_, EnrichedUtusan>(
            "SELECT u.id, u.event_id, u.nama_utusan, u.logo, COUNT(a.id) as athlete_count
             FROM backend_utusan u
             LEFT JOIN backend_atlet a ON a.utusan_id = u.id
             WHERE u.id = ?
             GROUP BY u.id, u.event_id, u.nama_utusan, u.logo"
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Gagal memuat kontingen setelah update: {}", e))
    }

    /// Deletes a contingent and dissociates any attached athletes
    pub async fn delete_contingent(&self, id: i64) -> Result<(), String> {
        let pool = self.get_pool()?;
        sqlx::query("UPDATE backend_atlet SET utusan_id = NULL WHERE utusan_id = ?")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal melepas asosiasi atlet: {}", e))?;

        sqlx::query("DELETE FROM backend_utusan WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal menghapus kontingen: {}", e))?;

        Ok(())
    }

    /// Fetches all perguruans for an event with athlete counts
    pub async fn get_enriched_perguruans(&self, event_id: i64) -> Result<Vec<EnrichedPerguruan>, String> {
        let pool = self.get_pool()?;
        sqlx::query_as::<_, EnrichedPerguruan>(
            "SELECT p.id, p.event_id, p.nama_perguruan, COUNT(a.id) as athlete_count
             FROM backend_perguruan p
             LEFT JOIN backend_atlet a ON a.perguruan_id = p.id AND a.event_id = ?
             WHERE p.event_id = ?
                OR p.id IN (SELECT DISTINCT perguruan_id FROM backend_atlet WHERE event_id = ? AND perguruan_id IS NOT NULL)
             GROUP BY p.id, p.event_id, p.nama_perguruan
             ORDER BY p.nama_perguruan ASC"
        )
        .bind(event_id)
        .bind(event_id)
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat perguruan: {}", e))
    }

    /// Creates a new perguruan
    pub async fn create_perguruan(&self, event_id: i64, nama_perguruan: String) -> Result<EnrichedPerguruan, String> {
        let pool = self.get_pool()?;
        let clean_name = nama_perguruan.trim().to_uppercase();
        if clean_name.is_empty() {
            return Err("Nama perguruan tidak boleh kosong".into());
        }

        let res = sqlx::query("INSERT INTO backend_perguruan (event_id, nama_perguruan) VALUES (?, ?)")
            .bind(event_id)
            .bind(&clean_name)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal menambahkan perguruan: {}", e))?;

        let id = res.last_insert_rowid();
        Ok(EnrichedPerguruan {
            id,
            event_id: Some(event_id),
            nama_perguruan: Some(clean_name),
            athlete_count: 0,
        })
    }

    /// Updates an existing perguruan
    pub async fn update_perguruan(&self, id: i64, nama_perguruan: String) -> Result<EnrichedPerguruan, String> {
        let pool = self.get_pool()?;
        let clean_name = nama_perguruan.trim().to_uppercase();
        if clean_name.is_empty() {
            return Err("Nama perguruan tidak boleh kosong".into());
        }

        sqlx::query("UPDATE backend_perguruan SET nama_perguruan = ? WHERE id = ?")
            .bind(&clean_name)
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal memperbarui perguruan: {}", e))?;

        sqlx::query_as::<_, EnrichedPerguruan>(
            "SELECT p.id, p.event_id, p.nama_perguruan, COUNT(a.id) as athlete_count
             FROM backend_perguruan p
             LEFT JOIN backend_atlet a ON a.perguruan_id = p.id
             WHERE p.id = ?
             GROUP BY p.id, p.event_id, p.nama_perguruan"
        )
        .bind(id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Gagal memuat perguruan setelah update: {}", e))
    }

    /// Deletes a perguruan and dissociates any attached athletes
    pub async fn delete_perguruan(&self, id: i64) -> Result<(), String> {
        let pool = self.get_pool()?;
        sqlx::query("UPDATE backend_atlet SET perguruan_id = NULL WHERE perguruan_id = ?")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal melepas asosiasi atlet: {}", e))?;

        sqlx::query("DELETE FROM backend_perguruan WHERE id = ?")
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal menghapus perguruan: {}", e))?;

        Ok(())
    }

    /// Saves a contingent logo from base64 data to media/logo_utusan/
    pub async fn save_contingent_logo(&self, file_name: &str, base64_data: &str) -> Result<String, String> {
        let media_dir = self.get_media_dir()
            .ok_or_else(|| "Media directory tidak aktif (database belum dimount)".to_string())?;

        let logo_dir = media_dir.join("logo_utusan");
        if !logo_dir.exists() {
            let _ = std::fs::create_dir_all(&logo_dir);
        }

        // Clean base64 string if it contains data URI header
        let raw_b64 = if let Some(idx) = base64_data.find(";base64,") {
            &base64_data[idx + 8..]
        } else {
            base64_data
        };

        use base64::Engine;
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(raw_b64.trim())
            .map_err(|e| format!("Gagal decode base64 gambar: {}", e))?;

        let safe_name = file_name.replace(|c: char| !c.is_alphanumeric() && c != '.' && c != '_' && c != '-', "_");
        let dest_path = logo_dir.join(&safe_name);

        tokio::fs::write(&dest_path, bytes)
            .await
            .map_err(|e| format!("Gagal menyimpan file logo: {}", e))?;

        let relative = format!("logo_utusan/{}", safe_name);
        Ok(relative)
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
              AND (nt.is_bob = 0 OR nt.is_bob IS NULL)
              AND UPPER(nt.nama_nomor_tanding) NOT LIKE '%FESTIVAL%'
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

    /// Fetches all festival categories eligible for festival bracket drawing
    pub async fn get_drawing_eligible_festival_categories(&self, event_id: i64) -> Result<Vec<DrawingCategoryInfo>, String> {
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
              AND UPPER(nt.nama_nomor_tanding) LIKE '%FESTIVAL%'
            ORDER BY nt.nama_nomor_tanding ASC
            "#
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat kategori festival drawing: {}", e))?;

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
                let (group_key, secondary_key) = match shuffle_type {
                    "perguruan" => (
                        a.perguruan_id.map(|id| format!("P_{}", id)).unwrap_or_else(|| format!("A_{}", a.id)),
                        None
                    ),
                    "kontingen" => (
                        a.utusan_id.map(|id| format!("U_{}", id)).unwrap_or_else(|| format!("A_{}", a.id)),
                        None
                    ),
                    _ => (
                        // WKF & FORKI Default ("both"): Primary contingent, secondary perguruan
                        a.utusan_id.map(|id| format!("U_{}", id)).unwrap_or_else(|| format!("A_{}", a.id)),
                        a.perguruan_id.map(|id| format!("P_{}", id)),
                    ),
                };
                AthleteInput {
                    id: a.id,
                    name: a.nama_atlet.unwrap_or_default(),
                    is_priority: a.is_priority,
                    group_key,
                    secondary_key,
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

    /// Executes drawing for festival categories (4 athletes per group, up to 4 groups per bagan)
    pub async fn execute_drawing_festival_bagan(
        &self,
        event_id: i64,
        category_ids: Vec<i64>,
        shuffle_type: &str,
    ) -> Result<DrawingSummary, String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        let mut categories_processed = 0;
        let mut bagans_created = 0;
        let mut total_matches = 0;
        let total_collisions = 0;

        // 1. Clean up previous brackets for selected categories first
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
            .map_err(|e| format!("Gagal mereset tatami untuk kategori festival {}: {}", cat_id, e))?;

            sqlx::query("DELETE FROM backend_detailbagan WHERE bagan_id IN (SELECT id FROM backend_bagan WHERE nomor_tanding_id = ?)")
                .bind(cat_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal menghapus detail bagan festival lama: {}", e))?;

            sqlx::query("DELETE FROM backend_bagan WHERE nomor_tanding_id = ?")
                .bind(cat_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal menghapus bagan festival lama: {}", e))?;
        }

        // 2. Determine starting festival code number (F01, F02...) for this event
        let existing_codes: Vec<String> = sqlx::query_scalar(
            "SELECT kode FROM backend_bagan WHERE event_id = ? AND kode LIKE 'F%'"
        )
        .bind(event_id)
        .fetch_all(&mut *tx)
        .await
        .unwrap_or_default();

        let mut next_code_num: u32 = existing_codes
            .iter()
            .filter_map(|c| c.strip_prefix('F').and_then(|s| s.parse::<u32>().ok()))
            .max()
            .unwrap_or(0) + 1;

        // 3. Process each category
        for cat_id in category_ids {
            #[derive(sqlx::FromRow)]
            #[allow(dead_code)]
            struct CatRow {
                id: i64,
                nama_nomor_tanding: Option<String>,
            }

            let cat = sqlx::query_as::<_, CatRow>(
                "SELECT id, nama_nomor_tanding FROM backend_nomortanding WHERE id = ?"
            )
            .bind(cat_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("Gagal memuat kategori festival {}: {}", cat_id, e))?;

            let Some(cat) = cat else { continue; };
            let cat_name = cat.nama_nomor_tanding.unwrap_or_default();
            if !cat_name.to_uppercase().contains("FESTIVAL") {
                continue;
            }

            let is_kumite = cat_name.to_uppercase().contains("KUMITE");
            let tipe_tanding = if is_kumite { Some("2") } else { Some("1") };

            #[derive(sqlx::FromRow)]
            #[allow(dead_code)]
            struct RawAtletFest {
                id: i64,
                nama_atlet: Option<String>,
                utusan_id: Option<i64>,
                perguruan_id: Option<i64>,
                nama_utusan: Option<String>,
                nama_perguruan: Option<String>,
            }

            let raw_atlets = sqlx::query_as::<_, RawAtletFest>(
                r#"
                SELECT a.id, a.nama_atlet, a.utusan_id, a.perguruan_id, u.nama_utusan, p.nama_perguruan
                FROM backend_atlet a
                LEFT JOIN backend_utusan u ON a.utusan_id = u.id
                LEFT JOIN backend_perguruan p ON a.perguruan_id = p.id
                WHERE a.nomor_tanding_id = ?
                  AND (a.additional_code IS NULL OR a.additional_code NOT LIKE 'MEMBER_%')
                ORDER BY a.id ASC
                "#
            )
            .bind(cat_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| format!("Gagal memuat atlet festival: {}", e))?;

            if raw_atlets.is_empty() {
                continue;
            }

            // Convert to FestivalAthlete and run anti-collision pairing
            let fest_athletes: Vec<FestivalAthlete> = raw_atlets.into_iter().map(|a| FestivalAthlete {
                id: a.id,
                nama: a.nama_atlet.unwrap_or_default(),
                perguruan: a.nama_perguruan.clone().unwrap_or_default(),
                utusan: match shuffle_type {
                    "perguruan" => a.nama_perguruan.unwrap_or_default(),
                    _ => a.nama_utusan.unwrap_or_default(),
                },
                jenis_kelamin: String::new(),
            }).collect();

            let paired_athletes = helper_pair_festival_athletes_anti_collision(fest_athletes);

            // Chunk into bagans of max 16 athletes (up to 4 groups of 4 per bagan)
            let chunks: Vec<Vec<FestivalAthlete>> = paired_athletes.chunks(16).map(|c| c.to_vec()).collect();
            let total_chunks = chunks.len();

            categories_processed += 1;

            for (chunk_idx, chunk_athletes) in chunks.into_iter().enumerate() {
                let n_chunk = chunk_athletes.len();
                let num_groups = (n_chunk + 3) / 4; // 1..=4 groups

                let bagan_title = if total_chunks == 1 {
                    cat_name.clone()
                } else {
                    format!("{} - Bagan {}", cat_name, chunk_idx + 1)
                };

                let bagan_code = format!("F{:02}", next_code_num);
                next_code_num += 1;

                let bagan_res = sqlx::query(
                    "INSERT INTO backend_bagan (event_id, nomor_tanding_id, nama_bagan, pool, tipe_tanding, round_robin, is_bob, has_vr, kode) VALUES (?, ?, ?, ?, ?, 0, 0, 0, ?)"
                )
                .bind(event_id)
                .bind(cat_id)
                .bind(&bagan_title)
                .bind(num_groups as i32)
                .bind(tipe_tanding)
                .bind(&bagan_code)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal membuat bagan festival: {}", e))?;

                let bagan_id = bagan_res.last_insert_rowid();
                bagans_created += 1;

                // Distribute n_chunk athletes across num_groups as evenly as possible (max 4 per group)
                let base_size = n_chunk / num_groups;
                let remainder = n_chunk % num_groups;
                let mut start_idx = 0;
                let mut group_slices: Vec<Vec<FestivalAthlete>> = Vec::new();
                for g_idx in 0..num_groups {
                    let size = base_size + if g_idx < remainder { 1 } else { 0 };
                    let end_idx = start_idx + size;
                    group_slices.push(chunk_athletes[start_idx..end_idx].to_vec());
                    start_idx = end_idx;
                }

                // For each group g (1..=num_groups): create 3 matches (Semi 1, Semi 2, Final)
                for (g_idx, g_athletes) in group_slices.into_iter().enumerate() {
                    let g = (g_idx + 1) as i32;
                    let semi1_urutan = 2 * g - 1;
                    let semi2_urutan = 2 * g;
                    let final_urutan = g;

                    match g_athletes.len() {
                        4 => {
                            // Semi 1: Atlet 0 vs Atlet 1
                            sqlx::query(
                                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team) VALUES (?, 1, ?, ?, ?, 0, 0, 0, 0, 0)"
                            )
                            .bind(bagan_id).bind(semi1_urutan).bind(g_athletes[0].id).bind(g_athletes[1].id)
                            .execute(&mut *tx).await.map_err(|e| format!("Gagal simpan semi 1: {}", e))?;

                            // Semi 2: Atlet 2 vs Atlet 3
                            sqlx::query(
                                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team) VALUES (?, 1, ?, ?, ?, 0, 0, 0, 0, 0)"
                            )
                            .bind(bagan_id).bind(semi2_urutan).bind(g_athletes[2].id).bind(g_athletes[3].id)
                            .execute(&mut *tx).await.map_err(|e| format!("Gagal simpan semi 2: {}", e))?;

                            // Final: TBD vs TBD
                            sqlx::query(
                                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team) VALUES (?, 2, ?, NULL, NULL, 0, 0, 0, 0, 0)"
                            )
                            .bind(bagan_id).bind(final_urutan)
                            .execute(&mut *tx).await.map_err(|e| format!("Gagal simpan final: {}", e))?;
                        }
                        3 => {
                            // Semi 1: Atlet 0 vs Atlet 1
                            sqlx::query(
                                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team) VALUES (?, 1, ?, ?, ?, 0, 0, 0, 0, 0)"
                            )
                            .bind(bagan_id).bind(semi1_urutan).bind(g_athletes[0].id).bind(g_athletes[1].id)
                            .execute(&mut *tx).await.map_err(|e| format!("Gagal simpan semi 1: {}", e))?;

                            // Semi 2: Atlet 2 vs BYE (selesai = 1, pemenang = 1)
                            sqlx::query(
                                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team, pemenang) VALUES (?, 1, ?, ?, NULL, 0, 0, 0, 1, 0, '1')"
                            )
                            .bind(bagan_id).bind(semi2_urutan).bind(g_athletes[2].id)
                            .execute(&mut *tx).await.map_err(|e| format!("Gagal simpan semi 2 BYE: {}", e))?;

                            // Final: TBD vs Atlet 2 (Atlet 2 seeded directly via BYE)
                            sqlx::query(
                                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team) VALUES (?, 2, ?, NULL, ?, 0, 0, 0, 0, 0)"
                            )
                            .bind(bagan_id).bind(final_urutan).bind(g_athletes[2].id)
                            .execute(&mut *tx).await.map_err(|e| format!("Gagal simpan final: {}", e))?;
                        }
                        2 => {
                            // Semi 1: BYE Atlet 0
                            sqlx::query(
                                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team, pemenang) VALUES (?, 1, ?, ?, NULL, 0, 0, 0, 1, 0, '1')"
                            )
                            .bind(bagan_id).bind(semi1_urutan).bind(g_athletes[0].id)
                            .execute(&mut *tx).await.map_err(|e| format!("Gagal simpan semi 1 BYE: {}", e))?;

                            // Semi 2: BYE Atlet 1
                            sqlx::query(
                                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team, pemenang) VALUES (?, 1, ?, ?, NULL, 0, 0, 0, 1, 0, '1')"
                            )
                            .bind(bagan_id).bind(semi2_urutan).bind(g_athletes[1].id)
                            .execute(&mut *tx).await.map_err(|e| format!("Gagal simpan semi 2 BYE: {}", e))?;

                            // Final: Atlet 0 vs Atlet 1 directly
                            sqlx::query(
                                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team) VALUES (?, 2, ?, ?, ?, 0, 0, 0, 0, 0)"
                            )
                            .bind(bagan_id).bind(final_urutan).bind(g_athletes[0].id).bind(g_athletes[1].id)
                            .execute(&mut *tx).await.map_err(|e| format!("Gagal simpan final 2 atlet: {}", e))?;
                        }
                        1 => {
                            // Single athlete: automatically gold
                            sqlx::query(
                                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team, pemenang) VALUES (?, 1, ?, ?, NULL, 0, 0, 0, 1, 0, '1')"
                            )
                            .bind(bagan_id).bind(semi1_urutan).bind(g_athletes[0].id)
                            .execute(&mut *tx).await.map_err(|e| format!("Gagal simpan semi 1 single: {}", e))?;

                            sqlx::query(
                                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team, pemenang) VALUES (?, 1, ?, NULL, NULL, 0, 0, 0, 1, 0, NULL)"
                            )
                            .bind(bagan_id).bind(semi2_urutan)
                            .execute(&mut *tx).await.map_err(|e| format!("Gagal simpan semi 2 single: {}", e))?;

                            sqlx::query(
                                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team, pemenang) VALUES (?, 2, ?, ?, NULL, 0, 0, 0, 1, 0, '1')"
                            )
                            .bind(bagan_id).bind(final_urutan).bind(g_athletes[0].id)
                            .execute(&mut *tx).await.map_err(|e| format!("Gagal simpan final single: {}", e))?;
                        }
                        _ => {}
                    }
                    total_matches += 3;
                }
            }
        }

        tx.commit().await.map_err(|e| format!("Gagal commit transaksi drawing festival: {}", e))?;

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
        let current_match: (i64, i64, i64, i64, Option<i64>, Option<i64>, bool) = sqlx::query_as(
            "SELECT id, bagan_id, round, urutan, atlet1_id, atlet2_id, team FROM backend_detailbagan WHERE id = ?"
        )
        .bind(input.match_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| format!("Partai dengan ID {} tidak ditemukan: {}", input.match_id, e))?;

        let (_match_id, bagan_id, round_num, urutan_num, a1_id, a2_id, is_team_child_bout) = current_match;

        // Fetch bagan metadata to identify pool / multi-pool context
        let bagan_meta: Option<(i32, Option<i64>, Option<String>, Option<i64>, bool)> = sqlx::query_as(
            "SELECT pool, nomor_tanding_id, nama_bagan, event_id, round_robin FROM backend_bagan WHERE id = ?"
        )
        .bind(bagan_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("Gagal memuat info bagan #{}: {}", bagan_id, e))?;

        let (bagan_pool, nomor_tanding_id, nama_bagan, event_id, is_round_robin) = bagan_meta.unwrap_or((1, None, None, None, false));
        let nama_bagan_upper = nama_bagan.unwrap_or_default().to_uppercase();
        let is_repechage = nama_bagan_upper.contains("REFERCHANGE") || nama_bagan_upper.contains("REPECHAGE");
        let is_festival = nama_bagan_upper.contains("FESTIVAL");
        let is_pool_bracket = (bagan_pool == 2 || nama_bagan_upper.contains("POOL A") || nama_bagan_upper.contains("POOL B") || nama_bagan_upper.contains("POOL 1") || nama_bagan_upper.contains("POOL 2")) && !is_repechage;
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
        if is_round_robin {
            // Round-robin tournament logic:
            // Check if all matches in this bagan are finished
            let total_m: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM backend_detailbagan WHERE bagan_id = ?"
            )
            .bind(bagan_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghitung partai round-robin: {}", e))?;

            let fin_m: (i64,) = sqlx::query_as(
                "SELECT COUNT(*) FROM backend_detailbagan WHERE bagan_id = ? AND selesai = 1"
            )
            .bind(bagan_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghitung partai selesai: {}", e))?;

            if fin_m.0 == total_m.0 && total_m.0 > 0 {
                // Fetch matches and athletes to compute final standings
                let matches_db = sqlx::query_as::<_, EnrichedDetailBagan>(
                    r#"
                    SELECT 
                        d.id, d.bagan_id, d.round, d.urutan, d.atlet1_id, d.atlet2_id,
                        d.score1, d.score2, d.scorekecil1, d.scorekecil2,
                        d.vr1, d.vr2, d.kata1, d.kata2, d.pemenang, d.hantei, d.selesai, d.team,
                        d.assigned_tatami_id, d.kode,
                        a1.nama_atlet as atlet1_nama,
                        NULL as atlet1_kontingen,
                        NULL as atlet1_logo,
                        a1.is_priority as atlet1_is_priority,
                        a2.nama_atlet as atlet2_nama,
                        NULL as atlet2_kontingen,
                        NULL as atlet2_logo,
                        a2.is_priority as atlet2_is_priority
                    FROM backend_detailbagan d
                    LEFT JOIN backend_atlet a1 ON d.atlet1_id = a1.id
                    LEFT JOIN backend_atlet a2 ON d.atlet2_id = a2.id
                    WHERE d.bagan_id = ?
                    ORDER BY d.round ASC, d.urutan ASC
                    "#
                )
                .bind(bagan_id)
                .fetch_all(&mut *tx)
                .await
                .map_err(|e| format!("Gagal membaca partai round-robin: {}", e))?;

                let athletes_db = sqlx::query_as::<_, EnrichedAtlet>(
                    r#"
                    SELECT 
                        a.id, a.event_id, a.nama_atlet, a.nik, a.kode_atlet, a.additional_code, a.is_priority,
                        a.perguruan_id, p.nama_perguruan as perguruan_nama,
                        a.utusan_id, u.nama_utusan as utusan_nama, u.logo as utusan_logo,
                        a.nomor_tanding_id, nt.nama_nomor_tanding as nomor_tanding_nama,
                        'Tournament' as nomor_tanding_tipe,
                        nt.is_bob
                    FROM backend_atlet a
                    LEFT JOIN backend_perguruan p ON a.perguruan_id = p.id
                    LEFT JOIN backend_utusan u ON a.utusan_id = u.id
                    LEFT JOIN backend_nomortanding nt ON a.nomor_tanding_id = nt.id
                    WHERE a.id IN (
                        SELECT atlet1_id FROM backend_detailbagan WHERE bagan_id = ? AND atlet1_id IS NOT NULL
                        UNION
                        SELECT atlet2_id FROM backend_detailbagan WHERE bagan_id = ? AND atlet2_id IS NOT NULL
                    )
                    "#
                )
                .bind(bagan_id)
                .bind(bagan_id)
                .fetch_all(&mut *tx)
                .await
                .map_err(|e| format!("Gagal membaca atlet round-robin: {}", e))?;

                let (standings, _) = self.calculate_round_robin_matrix_and_standings(&athletes_db, &matches_db);

                let j1_id = standings.first().map(|s| s.athlete_id);
                let j2_id = standings.get(1).map(|s| s.athlete_id);
                let j3a_id = standings.get(2).map(|s| s.athlete_id);
                let j3b_id = standings.get(3).map(|s| s.athlete_id);

                sqlx::query(
                    "UPDATE backend_bagan SET juara_1_id = ?, juara_2_id = ?, juara_3a_id = ?, juara_3b_id = ? WHERE id = ?"
                )
                .bind(j1_id)
                .bind(j2_id)
                .bind(j3a_id)
                .bind(j3b_id)
                .bind(bagan_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal mengupdate podium round-robin: {}", e))?;
            } else {
                // If any match was reopened / reset, clear podium
                sqlx::query(
                    "UPDATE backend_bagan SET juara_1_id = NULL, juara_2_id = NULL, juara_3a_id = NULL, juara_3b_id = NULL WHERE id = ?"
                )
                .bind(bagan_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal mereset podium round-robin: {}", e))?;
            }
        } else if is_festival && !is_team_child_bout {
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
                    if round_num == 1 {
                        // Semifinal of group g: advance winner to Final of group g
                        let group_idx = (urutan_num + 1) / 2;
                        if urutan_num % 2 == 1 {
                            // Semi 1 winner goes to Final atlet1
                            sqlx::query("UPDATE backend_detailbagan SET atlet1_id = ? WHERE bagan_id = ? AND round = 2 AND urutan = ?")
                                .bind(w_id)
                                .bind(bagan_id)
                                .bind(group_idx)
                                .execute(&mut *tx)
                                .await
                                .map_err(|e| format!("Gagal memajukan pemenang semifinal grup {} ke final: {}", group_idx, e))?;
                        } else {
                            // Semi 2 winner goes to Final atlet2
                            sqlx::query("UPDATE backend_detailbagan SET atlet2_id = ? WHERE bagan_id = ? AND round = 2 AND urutan = ?")
                                .bind(w_id)
                                .bind(bagan_id)
                                .bind(group_idx)
                                .execute(&mut *tx)
                                .await
                                .map_err(|e| format!("Gagal memajukan pemenang semifinal grup {} ke final: {}", group_idx, e))?;
                        }
                    } else if round_num == 2 {
                        // Final of group g:
                        if urutan_num == 1 {
                            let _ = sqlx::query("UPDATE backend_bagan SET juara_1_id = ?, juara_2_id = ? WHERE id = ?")
                                .bind(w_id)
                                .bind(loser_id)
                                .bind(bagan_id)
                                .execute(&mut *tx)
                                .await;
                        }
                    }
                }
            } else if !input.selesai {
                // Match is re-opened or reset:
                if round_num == 1 {
                    let group_idx = (urutan_num + 1) / 2;
                    if urutan_num % 2 == 1 {
                        let _ = sqlx::query("UPDATE backend_detailbagan SET atlet1_id = NULL WHERE bagan_id = ? AND round = 2 AND urutan = ? AND selesai = 0")
                            .bind(bagan_id)
                            .bind(group_idx)
                            .execute(&mut *tx)
                            .await;
                    } else {
                        let _ = sqlx::query("UPDATE backend_detailbagan SET atlet2_id = NULL WHERE bagan_id = ? AND round = 2 AND urutan = ? AND selesai = 0")
                            .bind(bagan_id)
                            .bind(group_idx)
                            .execute(&mut *tx)
                            .await;
                    }
                } else if round_num == 2 {
                    if urutan_num == 1 {
                        let _ = sqlx::query("UPDATE backend_bagan SET juara_1_id = NULL, juara_2_id = NULL WHERE id = ?")
                            .bind(bagan_id)
                            .execute(&mut *tx)
                            .await;
                    }
                }
            }
        } else if !is_team_child_bout {
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
                if is_repechage {
                    // Repechage bracket: cascade within pool (urutan 1 = Pool A, urutan 2 = Pool B)
                    let next_match: Option<(i64,)> = sqlx::query_as(
                        "SELECT id FROM backend_detailbagan WHERE bagan_id = ? AND round = ? AND urutan = ?"
                    )
                    .bind(bagan_id)
                    .bind(round_num + 1)
                    .bind(urutan_num)
                    .fetch_optional(&mut *tx)
                    .await
                    .unwrap_or(None);

                    if let Some((next_id,)) = next_match {
                        // Advance winner to next round's slot (atlet1_id)
                        sqlx::query("UPDATE backend_detailbagan SET atlet1_id = ? WHERE id = ?")
                            .bind(w_id)
                            .bind(next_id)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| format!("Gagal memajukan pemenang repechage: {}", e))?;
                    } else {
                        // Final round of repechage for this pool! Winner is Bronze Medalist!
                        if urutan_num == 1 {
                            // Pool A -> Juara 3A
                            sqlx::query("UPDATE backend_bagan SET juara_3a_id = ? WHERE id = ?")
                                .bind(w_id)
                                .bind(bagan_id)
                                .execute(&mut *tx)
                                .await
                                .map_err(|e| format!("Gagal memperbarui juara 3A repechage: {}", e))?;

                            if let Some(nt_id) = nomor_tanding_id {
                                let _ = sqlx::query(
                                    "UPDATE backend_bagan SET juara_3a_id = ? WHERE nomor_tanding_id = ? AND NOT (UPPER(nama_bagan) LIKE '%REFERCHANGE%' OR UPPER(nama_bagan) LIKE '%REPECHAGE%')"
                                )
                                .bind(w_id)
                                .bind(nt_id)
                                .execute(&mut *tx)
                                .await;
                            }
                        } else if urutan_num == 2 {
                            // Pool B -> Juara 3B
                            sqlx::query("UPDATE backend_bagan SET juara_3b_id = ? WHERE id = ?")
                                .bind(w_id)
                                .bind(bagan_id)
                                .execute(&mut *tx)
                                .await
                                .map_err(|e| format!("Gagal memperbarui juara 3B repechage: {}", e))?;

                            if let Some(nt_id) = nomor_tanding_id {
                                let _ = sqlx::query(
                                    "UPDATE backend_bagan SET juara_3b_id = ? WHERE nomor_tanding_id = ? AND NOT (UPPER(nama_bagan) LIKE '%REFERCHANGE%' OR UPPER(nama_bagan) LIKE '%REPECHAGE%')"
                                )
                                .bind(w_id)
                                .bind(nt_id)
                                .execute(&mut *tx)
                                .await;
                            }
                        }
                    }
                } else if is_pool_bracket && !is_final_bagan {
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
            if is_repechage {
                let next_match: Option<(i64,)> = sqlx::query_as(
                    "SELECT id FROM backend_detailbagan WHERE bagan_id = ? AND round = ? AND urutan = ? AND selesai = 0"
                )
                .bind(bagan_id)
                .bind(round_num + 1)
                .bind(urutan_num)
                .fetch_optional(&mut *tx)
                .await
                .unwrap_or(None);

                if let Some((next_id,)) = next_match {
                    let _ = sqlx::query("UPDATE backend_detailbagan SET atlet1_id = NULL WHERE id = ?")
                        .bind(next_id)
                        .execute(&mut *tx)
                        .await;
                } else {
                    if urutan_num == 1 {
                        let _ = sqlx::query("UPDATE backend_bagan SET juara_3a_id = NULL WHERE id = ?").bind(bagan_id).execute(&mut *tx).await;
                        if let Some(nt_id) = nomor_tanding_id {
                            let _ = sqlx::query("UPDATE backend_bagan SET juara_3a_id = NULL WHERE nomor_tanding_id = ? AND NOT (UPPER(nama_bagan) LIKE '%REFERCHANGE%' OR UPPER(nama_bagan) LIKE '%REPECHAGE%')").bind(nt_id).execute(&mut *tx).await;
                        }
                    } else if urutan_num == 2 {
                        let _ = sqlx::query("UPDATE backend_bagan SET juara_3b_id = NULL WHERE id = ?").bind(bagan_id).execute(&mut *tx).await;
                        if let Some(nt_id) = nomor_tanding_id {
                            let _ = sqlx::query("UPDATE backend_bagan SET juara_3b_id = NULL WHERE nomor_tanding_id = ? AND NOT (UPPER(nama_bagan) LIKE '%REFERCHANGE%' OR UPPER(nama_bagan) LIKE '%REPECHAGE%')").bind(nt_id).execute(&mut *tx).await;
                        }
                    }
                }
            } else if is_pool_bracket && !is_final_bagan {
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
        } else if is_team_child_bout && input.selesai {
            // Auto-advance tatami to next unfinished team bout if available
            let next_bout_id: Option<i64> = sqlx::query_scalar(
                r#"
                SELECT m.db_id FROM backend_matchup m
                JOIN backend_detailbagan db ON m.db_id = db.id
                WHERE m.detail_bagan_id = (SELECT detail_bagan_id FROM backend_matchup WHERE db_id = ?)
                  AND db.selesai = 0
                  AND m.db_id != ?
                ORDER BY m.round ASC LIMIT 1
                "#
            )
            .bind(input.match_id)
            .bind(input.match_id)
            .fetch_optional(&mut *tx)
            .await
            .unwrap_or(None);

            if let Some(nid) = next_bout_id {
                let _ = sqlx::query("UPDATE backend_tatami SET detail_bagan_id = ? WHERE detail_bagan_id = ?")
                    .bind(nid)
                    .bind(input.match_id)
                    .execute(&mut *tx)
                    .await;
            }
        }

        // 4. Enqueue into backend_syncqueue if live sync is enabled for this event
        if let Some(ev_id) = event_id {
            let ev_info: Option<(Option<String>, bool)> = sqlx::query_as(
                "SELECT kode_event, is_live_sync_enabled FROM backend_event WHERE id = ?"
            )
            .bind(ev_id)
            .fetch_optional(&mut *tx)
            .await
            .unwrap_or(None);

            if let Some((kode_event, is_live_sync_enabled)) = ev_info {
                if is_live_sync_enabled {
                    let ev_code = kode_event.unwrap_or_else(|| format!("EV{}", ev_id));
                    let kode_realtime = format!("{}-{}-{}", ev_code, bagan_id, input.match_id);

                    // Find tatami number associated with this match or assigned tatami
                    let ring_number: Option<i64> = sqlx::query_scalar(
                        r#"
                        SELECT tatami_number FROM backend_tatami 
                        WHERE detail_bagan_id = ? 
                           OR id = (SELECT assigned_tatami_id FROM backend_detailbagan WHERE id = ?)
                        LIMIT 1
                        "#
                    )
                    .bind(input.match_id)
                    .bind(input.match_id)
                    .fetch_optional(&mut *tx)
                    .await
                    .unwrap_or(None);

                    let ring_str = ring_number.map(|r| r.to_string()).unwrap_or_default();

                    let mut result_payload = serde_json::json!({
                        "status": if input.selesai { "finished" } else { "running" },
                        "detail_bagan_id": input.match_id,
                        "kode_realtime": &kode_realtime,
                        "ring_number": ring_str,
                        "pemenang": input.pemenang.as_deref().unwrap_or(""),
                        "score1": input.score1.as_deref().unwrap_or("0"),
                        "score2": input.score2.as_deref().unwrap_or("0"),
                        "score_aka": input.score1.as_deref().unwrap_or("0"),
                        "score_ao": input.score2.as_deref().unwrap_or("0"),
                        "vr1": input.vr1,
                        "vr2": input.vr2,
                        "selesai": input.selesai,
                        "hantei": input.hantei,
                    });

                    if let Some(s1) = &input.scorekecil1 {
                        result_payload["lil_score_aka"] = serde_json::Value::String(s1.clone());
                    }
                    if let Some(s2) = &input.scorekecil2 {
                        result_payload["lil_score_ao"] = serde_json::Value::String(s2.clone());
                    }
                    if let Some(k1) = &input.kata1 {
                        result_payload["kata_aka"] = serde_json::Value::String(k1.clone());
                    }
                    if let Some(k2) = &input.kata2 {
                        result_payload["kata_ao"] = serde_json::Value::String(k2.clone());
                    }

                    let payload_str = serde_json::to_string(&result_payload).unwrap_or_default();

                    let _ = sqlx::query(
                        "INSERT INTO backend_syncqueue (event_id, endpoint, payload, status) VALUES (?, 'api/result/', ?, 'pending')"
                    )
                    .bind(ev_id)
                    .bind(&payload_str)
                    .execute(&mut *tx)
                    .await;

                    // If final match completed, also enqueue api/final-result/
                    if input.selesai && round_num == 4 {
                        let podium_row: Option<(
                            Option<String>, Option<String>,
                            Option<String>, Option<String>,
                            Option<String>, Option<String>,
                            Option<String>, Option<String>,
                        )> = sqlx::query_as(
                            r#"
                            SELECT 
                                j1.nama_atlet, j1.kode_atlet,
                                j2.nama_atlet, j2.kode_atlet,
                                j3a.nama_atlet, j3a.kode_atlet,
                                j3b.nama_atlet, j3b.kode_atlet
                            FROM backend_bagan b
                            LEFT JOIN backend_atlet j1 ON b.juara_1_id = j1.id
                            LEFT JOIN backend_atlet j2 ON b.juara_2_id = j2.id
                            LEFT JOIN backend_atlet j3a ON b.juara_3a_id = j3a.id
                            LEFT JOIN backend_atlet j3b ON b.juara_3b_id = j3b.id
                            WHERE b.id = ?
                            "#
                        )
                        .bind(bagan_id)
                        .fetch_optional(&mut *tx)
                        .await
                        .unwrap_or(None);

                        if let Some((j1_name, j1_code, j2_name, j2_code, j3a_name, j3a_code, j3b_name, j3b_code)) = podium_row {
                            let final_payload = serde_json::json!({
                                "status": "finished",
                                "kode_realtime": &kode_realtime,
                                "juara_1": j1_name,
                                "juara_1_kode": j1_code,
                                "juara_2": j2_name,
                                "juara_2_kode": j2_code,
                                "juara_3a": j3a_name,
                                "juara_3a_kode": j3a_code,
                                "juara_3b": j3b_name,
                                "juara_3b_kode": j3b_code,
                            });
                            let final_str = serde_json::to_string(&final_payload).unwrap_or_default();
                            let _ = sqlx::query(
                                "INSERT INTO backend_syncqueue (event_id, endpoint, payload, status) VALUES (?, 'api/final-result/', ?, 'pending')"
                            )
                            .bind(ev_id)
                            .bind(&final_str)
                            .execute(&mut *tx)
                            .await;
                        }
                    }
                }
            }
        }

        tx.commit().await.map_err(|e| format!("Gagal melakukan commit transaksi: {}", e))?;
        Ok(())
    }

    /// Enqueues api/status/ (running) to backend_syncqueue and links the match to the tatami.
    pub async fn notify_match_running(&self, match_id: i64, tatami_number: i32) -> Result<(), String> {
        let pool = self.get_pool()?;

        // 1. Update tatami's active match
        let _ = sqlx::query("UPDATE backend_tatami SET detail_bagan_id = ? WHERE tatami_number = ?")
            .bind(match_id)
            .bind(tatami_number)
            .execute(pool)
            .await;

        let _ = sqlx::query(
            "UPDATE backend_detailbagan SET assigned_tatami_id = (SELECT id FROM backend_tatami WHERE tatami_number = ? LIMIT 1) WHERE id = ?"
        )
        .bind(tatami_number)
        .bind(match_id)
        .execute(pool)
        .await;

        // 2. Fetch match and event info to check live sync
        let match_info: Option<(i64, i64, i64, bool, bool, Option<String>, Option<i64>, bool)> = sqlx::query_as(
            r#"
            SELECT d.bagan_id, d.round, d.urutan, d.vr1, d.vr2, e.kode_event, b.event_id, e.is_live_sync_enabled
            FROM backend_detailbagan d
            JOIN backend_bagan b ON d.bagan_id = b.id
            JOIN backend_event e ON b.event_id = e.id
            WHERE d.id = ?
            "#
        )
        .bind(match_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| e.to_string())?;

        let Some((bagan_id, round_num, urutan_num, vr1, vr2, kode_event, event_id, is_live_sync)) = match_info else {
            return Ok(());
        };

        if !is_live_sync {
            return Ok(());
        }

        let ev_code = kode_event.unwrap_or_else(|| format!("EV{}", event_id.unwrap_or(0)));
        let kode_realtime = format!("{}-{}-{}", ev_code, bagan_id, match_id);

        let status_payload = serde_json::json!({
            "status": "running",
            "detail_bagan_id": match_id,
            "bagan_id": bagan_id,
            "round": round_num,
            "urutan": urutan_num,
            "vr1": vr1,
            "vr2": vr2,
            "kode_realtime": kode_realtime,
            "ring_number": tatami_number.to_string()
        });

        let payload_str = serde_json::to_string(&status_payload).unwrap_or_default();
        let _ = sqlx::query(
            "INSERT INTO backend_syncqueue (event_id, endpoint, payload, status) VALUES (?, 'api/status/', ?, 'pending')"
        )
        .bind(event_id)
        .bind(&payload_str)
        .execute(pool)
        .await;

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

    /// Fetches medal standings and detailed winner athlete lists for Kontingen & Perguruan.
    /// In accordance with Ambrilindo tournament rules, single-participant categories
    /// (tanpa lawan) receive a gold medal but are marked as uncounted for Juara Umum standings.
    pub async fn get_summary_juara(&self, event_id: i64) -> Result<SummaryJuaraResult, String> {
        let pool = self.get_pool()?;

        // 1. Event Name
        let event_name: String = sqlx::query_scalar(
            "SELECT COALESCE(nama_event, 'Turnamen Karate') FROM backend_event WHERE id = ?"
        )
        .bind(event_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| "Turnamen Karate".to_string());

        // 2. Fetch all Utusans (Kontingen) for this event
        #[derive(sqlx::FromRow)]
        struct UtusanRow {
            id: i64,
            nama_utusan: Option<String>,
            logo: Option<String>,
        }
        let utusan_rows = sqlx::query_as::<_, UtusanRow>(
            r#"
            SELECT u.id, u.nama_utusan, u.logo
            FROM backend_utusan u
            WHERE u.event_id = ? OR u.id IN (SELECT DISTINCT utusan_id FROM backend_atlet WHERE event_id = ? AND utusan_id IS NOT NULL)
            ORDER BY u.nama_utusan ASC
            "#
        )
        .bind(event_id)
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat kontingen: {}", e))?;

        // 3. Fetch all Perguruans for this event
        #[derive(sqlx::FromRow)]
        struct PerguruanRow {
            id: i64,
            nama_perguruan: Option<String>,
        }
        let perguruan_rows = sqlx::query_as::<_, PerguruanRow>(
            r#"
            SELECT p.id, p.nama_perguruan
            FROM backend_perguruan p
            WHERE p.event_id = ? OR p.id IN (SELECT DISTINCT perguruan_id FROM backend_atlet WHERE event_id = ? AND perguruan_id IS NOT NULL)
            ORDER BY p.nama_perguruan ASC
            "#
        )
        .bind(event_id)
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat perguruan: {}", e))?;

        // Initialize standings maps
        let mut utusan_map: HashMap<i64, StandingItem> = HashMap::new();
        for u in utusan_rows {
            utusan_map.insert(
                u.id,
                StandingItem {
                    id: u.id,
                    rank: 0,
                    nama: u.nama_utusan.unwrap_or_else(|| format!("Kontingen #{}", u.id)),
                    logo: u.logo,
                    gold: 0,
                    silver: 0,
                    bronze: 0,
                    total: 0,
                    uncounted_gold: 0,
                    winners: Vec::new(),
                },
            );
        }

        let mut perguruan_map: HashMap<i64, StandingItem> = HashMap::new();
        for p in perguruan_rows {
            perguruan_map.insert(
                p.id,
                StandingItem {
                    id: p.id,
                    rank: 0,
                    nama: p.nama_perguruan.unwrap_or_else(|| format!("Perguruan #{}", p.id)),
                    logo: None,
                    gold: 0,
                    silver: 0,
                    bronze: 0,
                    total: 0,
                    uncounted_gold: 0,
                    winners: Vec::new(),
                },
            );
        }

        // 4. Map unique athletes in each bagan to detect single participant / no opponent
        #[derive(sqlx::FromRow)]
        struct MatchParticipantRow {
            bagan_id: i64,
            atlet1_id: Option<i64>,
            atlet2_id: Option<i64>,
        }
        let match_athletes = sqlx::query_as::<_, MatchParticipantRow>(
            r#"
            SELECT d.bagan_id, d.atlet1_id, d.atlet2_id
            FROM backend_detailbagan d
            JOIN backend_bagan b ON d.bagan_id = b.id
            WHERE b.event_id = ?
            "#
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let mut bagan_athlete_counts: HashMap<i64, HashSet<i64>> = HashMap::new();
        for m in match_athletes {
            let set = bagan_athlete_counts.entry(m.bagan_id).or_default();
            if let Some(a1) = m.atlet1_id {
                set.insert(a1);
            }
            if let Some(a2) = m.atlet2_id {
                set.insert(a2);
            }
        }

        // 5. Fetch all candidate bagans for this event
        #[derive(sqlx::FromRow)]
        struct BaganMedalRow {
            id: i64,
            nama_bagan: Option<String>,
            kode: Option<String>,
            nomor_tanding_id: Option<i64>,
            pool: Option<i32>,
            is_bob: Option<bool>,
            nt_nama: Option<String>,
            nt_is_bob: Option<bool>,
            juara_1_id: Option<i64>,
            juara_2_id: Option<i64>,
            juara_3a_id: Option<i64>,
            juara_3b_id: Option<i64>,
        }
        let all_raw_bagans = sqlx::query_as::<_, BaganMedalRow>(
            r#"
            SELECT b.id, b.nama_bagan, b.kode, b.nomor_tanding_id, b.pool, b.is_bob,
                   nt.nama_nomor_tanding AS nt_nama, nt.is_bob AS nt_is_bob,
                   b.juara_1_id, b.juara_2_id, b.juara_3a_id, b.juara_3b_id
            FROM backend_bagan b
            LEFT JOIN backend_nomortanding nt ON b.nomor_tanding_id = nt.id
            WHERE b.event_id = ?
            ORDER BY CAST(COALESCE(b.kode, '9999') AS INTEGER) ASC, b.id ASC
            "#
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat bagan untuk rekap juara: {}", e))?;

        // Helper: Check if a name or category is BOB (Best of the Best)
        let is_bob = |b_name: &str, nt_name: &str, b_bob: bool, nt_bob: bool| -> bool {
            if b_bob || nt_bob {
                return true;
            }
            for s in [b_name, nt_name] {
                let u = s.to_uppercase();
                if u.contains("BEST OF THE BEST")
                    || u.contains("(BOB)")
                    || u.contains(" BOB")
                    || u.contains("BOB ")
                    || u == "BOB"
                {
                    return true;
                }
            }
            false
        };

        // Helper: Check if festival
        let is_festival = |b_name: &str, nt_name: &str| -> bool {
            for s in [b_name, nt_name] {
                if s.to_uppercase().contains("FESTIVAL") {
                    return true;
                }
            }
            false
        };

        // Step A: Exclude BOB & Festival, and group bagans by category to identify multi-pool categories
        let mut non_bob_bagans: Vec<BaganMedalRow> = Vec::new();
        let mut cat_count_map: HashMap<i64, usize> = HashMap::new();

        for b in all_raw_bagans {
            let b_name = b.nama_bagan.as_deref().unwrap_or_default();
            let nt_name = b.nt_nama.as_deref().unwrap_or_default();
            let b_bob = b.is_bob.unwrap_or(false);
            let nt_bob = b.nt_is_bob.unwrap_or(false);

            if is_bob(b_name, nt_name, b_bob, nt_bob) || is_festival(b_name, nt_name) {
                continue;
            }

            if let Some(cat_id) = b.nomor_tanding_id {
                *cat_count_map.entry(cat_id).or_insert(0) += 1;
            }
            non_bob_bagans.push(b);
        }

        // Step B: Only keep Bagan Tunggal or Bagan Final (exclude preliminary non-final pools e.g. Pool A, Pool B)
        let mut bagans: Vec<BaganMedalRow> = Vec::new();
        for b in non_bob_bagans {
            let b_name = b.nama_bagan.as_deref().unwrap_or_default();
            let pool_val = b.pool.unwrap_or(1);

            let is_multi = b.nomor_tanding_id.map(|cat_id| {
                cat_count_map.get(&cat_id).copied().unwrap_or(1) > 1
            }).unwrap_or(false);

            if is_multi {
                // Multi-pool category: only keep Final!
                let is_final = pool_val == 0 || b_name.to_uppercase().contains("FINAL");
                if is_final {
                    bagans.push(b);
                }
            } else {
                // Bagan tunggal: make sure it is not an orphaned pool
                let is_pool = b_name.to_uppercase().contains("POOL ") && !b_name.to_uppercase().contains("FINAL");
                if !is_pool {
                    bagans.push(b);
                }
            }
        }

        // 6. Collect all winner athlete IDs to fetch their names & affiliations in one query
        let mut needed_athlete_ids: HashSet<i64> = HashSet::new();
        for b in &bagans {
            let athletes_in_b = bagan_athlete_counts.get(&b.id);
            let effective_j1 = b.juara_1_id.or_else(|| {
                if let Some(set) = athletes_in_b {
                    if set.len() == 1 && b.juara_2_id.is_none() && b.juara_3a_id.is_none() && b.juara_3b_id.is_none() {
                        return set.iter().next().copied();
                    }
                }
                None
            });

            if let Some(id) = effective_j1 { needed_athlete_ids.insert(id); }
            if let Some(id) = b.juara_2_id { needed_athlete_ids.insert(id); }
            if let Some(id) = b.juara_3a_id { needed_athlete_ids.insert(id); }
            if let Some(id) = b.juara_3b_id { needed_athlete_ids.insert(id); }
        }

        #[derive(Clone, sqlx::FromRow)]
        struct AthleteAffiliation {
            id: i64,
            nama_atlet: Option<String>,
            utusan_id: Option<i64>,
            nama_utusan: Option<String>,
            perguruan_id: Option<i64>,
            nama_perguruan: Option<String>,
        }

        let mut athlete_map: HashMap<i64, AthleteAffiliation> = HashMap::new();
        if !needed_athlete_ids.is_empty() {
            let id_list: Vec<String> = needed_athlete_ids.iter().map(|id| id.to_string()).collect();
            let sql = format!(
                r#"
                SELECT a.id, a.nama_atlet, a.utusan_id, u.nama_utusan, a.perguruan_id, p.nama_perguruan
                FROM backend_atlet a
                LEFT JOIN backend_utusan u ON a.utusan_id = u.id
                LEFT JOIN backend_perguruan p ON a.perguruan_id = p.id
                WHERE a.id IN ({})
                "#,
                id_list.join(",")
            );
            let athletes = sqlx::query_as::<_, AthleteAffiliation>(&sql)
                .fetch_all(pool)
                .await
                .unwrap_or_default();
            for a in athletes {
                athlete_map.insert(a.id, a);
            }
        }

        let mut total_gold = 0i64;
        let mut total_silver = 0i64;
        let mut total_bronze = 0i64;
        let mut total_uncounted = 0i64;

        // 7. Process medals for each bagan
        for b in bagans {
            let bagan_name = b.nama_bagan.unwrap_or_else(|| format!("Bagan #{}", b.id));
            let athletes_in_b = bagan_athlete_counts.get(&b.id);
            let has_opponents = athletes_in_b.map(|s| s.len() > 1).unwrap_or(false)
                || b.juara_2_id.is_some()
                || b.juara_3a_id.is_some()
                || b.juara_3b_id.is_some();

            let effective_j1 = b.juara_1_id.or_else(|| {
                if let Some(set) = athletes_in_b {
                    if set.len() == 1 && b.juara_2_id.is_none() && b.juara_3a_id.is_none() && b.juara_3b_id.is_none() {
                        return set.iter().next().copied();
                    }
                }
                None
            });

            // JUARA 1 (Emas)
            if let Some(a_id) = effective_j1 {
                if let Some(atlet) = athlete_map.get(&a_id) {
                    let a_nama = atlet.nama_atlet.clone().unwrap_or_else(|| format!("Atlet #{}", a_id));
                    let u_nama = atlet.nama_utusan.clone().unwrap_or_else(|| "-".to_string());
                    let p_nama = atlet.nama_perguruan.clone().unwrap_or_else(|| "-".to_string());

                    let winner_item = SummaryWinnerAthlete {
                        juara: "1".to_string(),
                        nama_atlet: a_nama,
                        utusan_nama: u_nama,
                        perguruan_nama: p_nama,
                        bagan_nama: bagan_name.clone(),
                        bagan_kode: b.kode.clone(),
                        is_counted: has_opponents,
                        keterangan: if has_opponents {
                            None
                        } else {
                            Some("Tidak dihitung (Tanpa Lawan)".to_string())
                        },
                    };

                    if has_opponents {
                        total_gold += 1;
                        if let Some(u_id) = atlet.utusan_id {
                            if let Some(entry) = utusan_map.get_mut(&u_id) {
                                entry.gold += 1;
                                entry.total += 1;
                                entry.winners.push(winner_item.clone());
                            }
                        }
                        if let Some(p_id) = atlet.perguruan_id {
                            if let Some(entry) = perguruan_map.get_mut(&p_id) {
                                entry.gold += 1;
                                entry.total += 1;
                                entry.winners.push(winner_item);
                            }
                        }
                    } else {
                        total_uncounted += 1;
                        if let Some(u_id) = atlet.utusan_id {
                            if let Some(entry) = utusan_map.get_mut(&u_id) {
                                entry.uncounted_gold += 1;
                                entry.winners.push(winner_item.clone());
                            }
                        }
                        if let Some(p_id) = atlet.perguruan_id {
                            if let Some(entry) = perguruan_map.get_mut(&p_id) {
                                entry.uncounted_gold += 1;
                                entry.winners.push(winner_item);
                            }
                        }
                    }
                }
            }

            // JUARA 2 (Perak)
            if let Some(a_id) = b.juara_2_id {
                if let Some(atlet) = athlete_map.get(&a_id) {
                    total_silver += 1;
                    let a_nama = atlet.nama_atlet.clone().unwrap_or_else(|| format!("Atlet #{}", a_id));
                    let u_nama = atlet.nama_utusan.clone().unwrap_or_else(|| "-".to_string());
                    let p_nama = atlet.nama_perguruan.clone().unwrap_or_else(|| "-".to_string());

                    let winner_item = SummaryWinnerAthlete {
                        juara: "2".to_string(),
                        nama_atlet: a_nama,
                        utusan_nama: u_nama,
                        perguruan_nama: p_nama,
                        bagan_nama: bagan_name.clone(),
                        bagan_kode: b.kode.clone(),
                        is_counted: true,
                        keterangan: None,
                    };

                    if let Some(u_id) = atlet.utusan_id {
                        if let Some(entry) = utusan_map.get_mut(&u_id) {
                            entry.silver += 1;
                            entry.total += 1;
                            entry.winners.push(winner_item.clone());
                        }
                    }
                    if let Some(p_id) = atlet.perguruan_id {
                        if let Some(entry) = perguruan_map.get_mut(&p_id) {
                            entry.silver += 1;
                            entry.total += 1;
                            entry.winners.push(winner_item);
                        }
                    }
                }
            }

            // JUARA 3A (Perunggu)
            if let Some(a_id) = b.juara_3a_id {
                if let Some(atlet) = athlete_map.get(&a_id) {
                    total_bronze += 1;
                    let a_nama = atlet.nama_atlet.clone().unwrap_or_else(|| format!("Atlet #{}", a_id));
                    let u_nama = atlet.nama_utusan.clone().unwrap_or_else(|| "-".to_string());
                    let p_nama = atlet.nama_perguruan.clone().unwrap_or_else(|| "-".to_string());

                    let winner_item = SummaryWinnerAthlete {
                        juara: "3a".to_string(),
                        nama_atlet: a_nama,
                        utusan_nama: u_nama,
                        perguruan_nama: p_nama,
                        bagan_nama: bagan_name.clone(),
                        bagan_kode: b.kode.clone(),
                        is_counted: true,
                        keterangan: None,
                    };

                    if let Some(u_id) = atlet.utusan_id {
                        if let Some(entry) = utusan_map.get_mut(&u_id) {
                            entry.bronze += 1;
                            entry.total += 1;
                            entry.winners.push(winner_item.clone());
                        }
                    }
                    if let Some(p_id) = atlet.perguruan_id {
                        if let Some(entry) = perguruan_map.get_mut(&p_id) {
                            entry.bronze += 1;
                            entry.total += 1;
                            entry.winners.push(winner_item);
                        }
                    }
                }
            }

            // JUARA 3B (Perunggu)
            if let Some(a_id) = b.juara_3b_id {
                if let Some(atlet) = athlete_map.get(&a_id) {
                    total_bronze += 1;
                    let a_nama = atlet.nama_atlet.clone().unwrap_or_else(|| format!("Atlet #{}", a_id));
                    let u_nama = atlet.nama_utusan.clone().unwrap_or_else(|| "-".to_string());
                    let p_nama = atlet.nama_perguruan.clone().unwrap_or_else(|| "-".to_string());

                    let winner_item = SummaryWinnerAthlete {
                        juara: "3b".to_string(),
                        nama_atlet: a_nama,
                        utusan_nama: u_nama,
                        perguruan_nama: p_nama,
                        bagan_nama: bagan_name.clone(),
                        bagan_kode: b.kode.clone(),
                        is_counted: true,
                        keterangan: None,
                    };

                    if let Some(u_id) = atlet.utusan_id {
                        if let Some(entry) = utusan_map.get_mut(&u_id) {
                            entry.bronze += 1;
                            entry.total += 1;
                            entry.winners.push(winner_item.clone());
                        }
                    }
                    if let Some(p_id) = atlet.perguruan_id {
                        if let Some(entry) = perguruan_map.get_mut(&p_id) {
                            entry.bronze += 1;
                            entry.total += 1;
                            entry.winners.push(winner_item);
                        }
                    }
                }
            }
        }

        // Helper to sort winner athletes: Juara 1 first, 2, 3a, 3b
        let sort_winners = |winners: &mut Vec<SummaryWinnerAthlete>| {
            winners.sort_by(|a, b| {
                let r_a = match a.juara.as_str() { "1" => 1, "2" => 2, "3a" => 3, "3b" => 4, _ => 5 };
                let r_b = match b.juara.as_str() { "1" => 1, "2" => 2, "3a" => 3, "3b" => 4, _ => 5 };
                r_a.cmp(&r_b).then_with(|| a.bagan_nama.cmp(&b.bagan_nama))
            });
        };

        // 8. Sort and assign ranks to Utusan standings
        let mut utusan_standings: Vec<StandingItem> = utusan_map.into_values().collect();
        for item in &mut utusan_standings {
            sort_winners(&mut item.winners);
        }
        utusan_standings.sort_by(|a, b| {
            b.gold.cmp(&a.gold)
                .then_with(|| b.silver.cmp(&a.silver))
                .then_with(|| b.bronze.cmp(&a.bronze))
                .then_with(|| a.nama.to_lowercase().cmp(&b.nama.to_lowercase()))
        });
        for (idx, item) in utusan_standings.iter_mut().enumerate() {
            item.rank = idx + 1;
        }

        // 9. Sort and assign ranks to Perguruan standings
        let mut perguruan_standings: Vec<StandingItem> = perguruan_map.into_values().collect();
        for item in &mut perguruan_standings {
            sort_winners(&mut item.winners);
        }
        perguruan_standings.sort_by(|a, b| {
            b.gold.cmp(&a.gold)
                .then_with(|| b.silver.cmp(&a.silver))
                .then_with(|| b.bronze.cmp(&a.bronze))
                .then_with(|| a.nama.to_lowercase().cmp(&b.nama.to_lowercase()))
        });
        for (idx, item) in perguruan_standings.iter_mut().enumerate() {
            item.rank = idx + 1;
        }

        Ok(SummaryJuaraResult {
            event_name,
            utusan_standings,
            perguruan_standings,
            total_gold,
            total_silver,
            total_bronze,
            total_uncounted,
        })
    }

    /// Fetches all bagans with their respective winners (Juara 1, 2, 3a, 3b)
    /// formatted for the "summary-bagan" view, including single-participant resolution.
    pub async fn get_summary_bagan(&self, event_id: i64) -> Result<SummaryBaganResult, String> {
        let pool = self.get_pool()?;

        // 1. Event Name
        let event_name: String = sqlx::query_scalar(
            "SELECT COALESCE(nama_event, 'Turnamen Karate') FROM backend_event WHERE id = ?"
        )
        .bind(event_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None)
        .unwrap_or_else(|| "Turnamen Karate".to_string());

        // 2. Map unique athletes in each bagan to detect single participant
        #[derive(sqlx::FromRow)]
        struct MatchParticipantRow {
            bagan_id: i64,
            atlet1_id: Option<i64>,
            atlet2_id: Option<i64>,
        }
        let match_athletes = sqlx::query_as::<_, MatchParticipantRow>(
            r#"
            SELECT d.bagan_id, d.atlet1_id, d.atlet2_id
            FROM backend_detailbagan d
            JOIN backend_bagan b ON d.bagan_id = b.id
            WHERE b.event_id = ?
            "#
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let mut bagan_athlete_counts: HashMap<i64, HashSet<i64>> = HashMap::new();
        for m in match_athletes {
            let set = bagan_athlete_counts.entry(m.bagan_id).or_default();
            if let Some(a1) = m.atlet1_id { set.insert(a1); }
            if let Some(a2) = m.atlet2_id { set.insert(a2); }
        }

        // 3. Fetch all candidate bagans for this event
        #[derive(sqlx::FromRow)]
        struct BaganSummaryRow {
            id: i64,
            nama_bagan: Option<String>,
            kode: Option<String>,
            tipe_tanding: Option<String>,
            nomor_tanding_id: Option<i64>,
            pool: Option<i32>,
            juara_1_id: Option<i64>,
            juara_2_id: Option<i64>,
            juara_3a_id: Option<i64>,
            juara_3b_id: Option<i64>,
        }
        let all_raw_bagans = sqlx::query_as::<_, BaganSummaryRow>(
            r#"
            SELECT b.id, b.nama_bagan, b.kode, b.tipe_tanding, b.nomor_tanding_id, b.pool,
                   b.juara_1_id, b.juara_2_id, b.juara_3a_id, b.juara_3b_id
            FROM backend_bagan b
            WHERE b.event_id = ?
            ORDER BY CAST(COALESCE(b.kode, '9999') AS INTEGER) ASC, b.id ASC
            "#
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat bagan untuk summary: {}", e))?;

        let mut cat_count_map: HashMap<i64, usize> = HashMap::new();
        let mut cat_has_final: HashMap<i64, bool> = HashMap::new();

        for b in &all_raw_bagans {
            if let Some(cat_id) = b.nomor_tanding_id {
                *cat_count_map.entry(cat_id).or_insert(0) += 1;
                let b_name = b.nama_bagan.as_deref().unwrap_or_default();
                let pool_val = b.pool.unwrap_or(1);
                if pool_val == 0 || b_name.to_uppercase().contains("FINAL") {
                    cat_has_final.insert(cat_id, true);
                }
            }
        }

        let mut bagan_rows: Vec<BaganSummaryRow> = Vec::new();
        for b in all_raw_bagans {
            let b_name = b.nama_bagan.as_deref().unwrap_or_default();
            let pool_val = b.pool.unwrap_or(1);

            let is_multi = b.nomor_tanding_id.map(|cat_id| {
                cat_count_map.get(&cat_id).copied().unwrap_or(1) > 1
            }).unwrap_or(false);

            if is_multi {
                let cat_id = b.nomor_tanding_id.unwrap();
                let has_final = cat_has_final.get(&cat_id).copied().unwrap_or(false);
                if has_final {
                    let is_final = pool_val == 0 || b_name.to_uppercase().contains("FINAL");
                    if is_final {
                        bagan_rows.push(b);
                    }
                } else {
                    // Multi-pool but no single final bracket (e.g. separate Festival pools): keep all pools!
                    bagan_rows.push(b);
                }
            } else {
                bagan_rows.push(b);
            }
        }

        // 4. Collect athlete IDs needed for affiliations
        let mut needed_athlete_ids: HashSet<i64> = HashSet::new();
        for b in &bagan_rows {
            let athletes_in_b = bagan_athlete_counts.get(&b.id);
            let effective_j1 = b.juara_1_id.or_else(|| {
                if let Some(set) = athletes_in_b {
                    if set.len() == 1 && b.juara_2_id.is_none() && b.juara_3a_id.is_none() && b.juara_3b_id.is_none() {
                        return set.iter().next().copied();
                    }
                }
                None
            });
            if let Some(id) = effective_j1 { needed_athlete_ids.insert(id); }
            if let Some(id) = b.juara_2_id { needed_athlete_ids.insert(id); }
            if let Some(id) = b.juara_3a_id { needed_athlete_ids.insert(id); }
            if let Some(id) = b.juara_3b_id { needed_athlete_ids.insert(id); }
        }

        #[derive(Clone, sqlx::FromRow)]
        struct AthleteAffiliation {
            id: i64,
            nama_atlet: Option<String>,
            _utusan_id: Option<i64>,
            nama_utusan: Option<String>,
            logo: Option<String>,
            _perguruan_id: Option<i64>,
            nama_perguruan: Option<String>,
        }

        let mut athlete_map: HashMap<i64, AthleteAffiliation> = HashMap::new();
        if !needed_athlete_ids.is_empty() {
            let id_list: Vec<String> = needed_athlete_ids.iter().map(|id| id.to_string()).collect();
            let sql = format!(
                r#"
                SELECT a.id, a.nama_atlet, a.utusan_id AS _utusan_id, u.nama_utusan, u.logo, a.perguruan_id AS _perguruan_id, p.nama_perguruan
                FROM backend_atlet a
                LEFT JOIN backend_utusan u ON a.utusan_id = u.id
                LEFT JOIN backend_perguruan p ON a.perguruan_id = p.id
                WHERE a.id IN ({})
                "#,
                id_list.join(",")
            );
            let athletes = sqlx::query_as::<_, AthleteAffiliation>(&sql)
                .fetch_all(pool)
                .await
                .unwrap_or_default();
            for a in athletes {
                athlete_map.insert(a.id, a);
            }
        }

        let mut total_gold = 0usize;
        let mut total_silver = 0usize;
        let mut total_bronze = 0usize;
        let mut completed_bagans = 0usize;
        let mut result_bagans: Vec<SummaryBaganItem> = Vec::new();

        for b in bagan_rows {
            let athletes_in_b = bagan_athlete_counts.get(&b.id);
            let is_single = athletes_in_b.map(|s| s.len() == 1).unwrap_or(false)
                && b.juara_2_id.is_none()
                && b.juara_3a_id.is_none()
                && b.juara_3b_id.is_none();

            let effective_j1 = b.juara_1_id.or_else(|| {
                if is_single {
                    athletes_in_b.and_then(|s| s.iter().next().copied())
                } else {
                    None
                }
            });

            let mut winners: Vec<SummaryBaganWinner> = Vec::new();

            // Juara 1 (Emas)
            if let Some(aid) = effective_j1 {
                if let Some(atlet) = athlete_map.get(&aid) {
                    total_gold += 1;
                    winners.push(SummaryBaganWinner {
                        juara: "1".to_string(),
                        athlete_id: aid,
                        nama_atlet: atlet.nama_atlet.clone().unwrap_or_else(|| format!("Atlet #{}", aid)),
                        perguruan_nama: atlet.nama_perguruan.clone().unwrap_or_else(|| "-".to_string()),
                        utusan_nama: atlet.nama_utusan.clone().unwrap_or_else(|| "-".to_string()),
                        logo: atlet.logo.clone(),
                        is_uncounted: is_single,
                    });
                }
            }

            // Juara 2 (Perak)
            if let Some(aid) = b.juara_2_id {
                if let Some(atlet) = athlete_map.get(&aid) {
                    total_silver += 1;
                    winners.push(SummaryBaganWinner {
                        juara: "2".to_string(),
                        athlete_id: aid,
                        nama_atlet: atlet.nama_atlet.clone().unwrap_or_else(|| format!("Atlet #{}", aid)),
                        perguruan_nama: atlet.nama_perguruan.clone().unwrap_or_else(|| "-".to_string()),
                        utusan_nama: atlet.nama_utusan.clone().unwrap_or_else(|| "-".to_string()),
                        logo: atlet.logo.clone(),
                        is_uncounted: false,
                    });
                }
            }

            // Juara 3A (Perunggu)
            if let Some(aid) = b.juara_3a_id {
                if let Some(atlet) = athlete_map.get(&aid) {
                    total_bronze += 1;
                    winners.push(SummaryBaganWinner {
                        juara: "3a".to_string(),
                        athlete_id: aid,
                        nama_atlet: atlet.nama_atlet.clone().unwrap_or_else(|| format!("Atlet #{}", aid)),
                        perguruan_nama: atlet.nama_perguruan.clone().unwrap_or_else(|| "-".to_string()),
                        utusan_nama: atlet.nama_utusan.clone().unwrap_or_else(|| "-".to_string()),
                        logo: atlet.logo.clone(),
                        is_uncounted: false,
                    });
                }
            }

            // Juara 3B (Perunggu)
            if let Some(aid) = b.juara_3b_id {
                if let Some(atlet) = athlete_map.get(&aid) {
                    total_bronze += 1;
                    winners.push(SummaryBaganWinner {
                        juara: "3b".to_string(),
                        athlete_id: aid,
                        nama_atlet: atlet.nama_atlet.clone().unwrap_or_else(|| format!("Atlet #{}", aid)),
                        perguruan_nama: atlet.nama_perguruan.clone().unwrap_or_else(|| "-".to_string()),
                        utusan_nama: atlet.nama_utusan.clone().unwrap_or_else(|| "-".to_string()),
                        logo: atlet.logo.clone(),
                        is_uncounted: false,
                    });
                }
            }

            let is_selesai = !winners.is_empty();
            if is_selesai {
                completed_bagans += 1;
            }

            let kode_str = b.kode.unwrap_or_else(|| format!("{:03}", b.id));
            let nama_str = b.nama_bagan.unwrap_or_else(|| format!("Bagan {}", kode_str));
            let tipe_str = b.tipe_tanding.unwrap_or_else(|| {
                if nama_str.to_uppercase().contains("KATA") {
                    "1".to_string()
                } else {
                    "2".to_string()
                }
            });

            result_bagans.push(SummaryBaganItem {
                id: b.id,
                kode: kode_str,
                nama_bagan: nama_str,
                tipe_tanding: tipe_str,
                is_selesai,
                winners,
            });
        }

        let total_bagans = result_bagans.len();

        Ok(SummaryBaganResult {
            event_name,
            total_bagans,
            completed_bagans,
            total_gold,
            total_silver,
            total_bronze,
            bagans: result_bagans,
        })
    }

    /// Memuat data jadwal pertandingan (Timetable Run-Sheet) lengkap dengan deteksi konflik atlet
    pub async fn get_timetable_editor_data(&self, event_id: i64) -> Result<TimetableEditorData, String> {
        let pool = self.get_pool()?;

        // 1. Validasi Event
        let event_row: Option<(String,)> = sqlx::query_as("SELECT COALESCE(nama_event, 'Turnamen') FROM backend_event WHERE id = ?")
            .bind(event_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Gagal memuat event: {}", e))?;
        let event_name = event_row.map(|r| r.0).unwrap_or_else(|| "Turnamen".to_string());

        // 2. Muat Tatamis
        let tatamis: Vec<Tatami> = sqlx::query_as("SELECT id, event_id, tatami_number, detail_bagan_id FROM backend_tatami WHERE event_id = ? ORDER BY tatami_number ASC")
            .bind(event_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Gagal memuat tatami: {}", e))?;

        // 3. Muat TimetableDays (buat Hari 1 jika belum ada)
        let mut days: Vec<TimetableDay> = sqlx::query_as("SELECT id, event_id, [order], tanggal FROM backend_timetableday WHERE event_id = ? ORDER BY [order] ASC")
            .bind(event_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Gagal memuat hari jadwal: {}", e))?;

        if days.is_empty() {
            let _ = sqlx::query("INSERT INTO backend_timetableday (event_id, [order], tanggal) VALUES (?, 0, NULL)")
                .bind(event_id)
                .execute(pool)
                .await
                .map_err(|e| format!("Gagal membuat hari default: {}", e))?;

            days = sqlx::query_as("SELECT id, event_id, [order], tanggal FROM backend_timetableday WHERE event_id = ? ORDER BY [order] ASC")
                .bind(event_id)
                .fetch_all(pool)
                .await
                .map_err(|e| format!("Gagal memuat hari jadwal setelah pembuatan default: {}", e))?;
        }

        // 4. Muat NomorTanding dan Hitungan Atlet
        let categories_raw: Vec<(i64, Option<String>, bool, bool)> = sqlx::query_as(
            "SELECT id, nama_nomor_tanding, is_bob, has_vr FROM backend_nomortanding WHERE event_id = ? ORDER BY nama_nomor_tanding ASC"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat nomor tanding: {}", e))?;

        let athlete_counts_raw: Vec<(i64, i64)> = sqlx::query_as(
            "SELECT nomor_tanding_id, COUNT(id) FROM backend_atlet WHERE event_id = ? AND nomor_tanding_id IS NOT NULL GROUP BY nomor_tanding_id"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal menghitung atlet: {}", e))?;
        let athlete_counts: HashMap<i64, i64> = athlete_counts_raw.into_iter().collect();

        // 5. Muat Bagan & DetailBagan untuk kalkulasi progress per kategori
        let bagans_raw: Vec<(i64, Option<i64>, Option<String>)> = sqlx::query_as(
            "SELECT id, nomor_tanding_id, nama_bagan FROM backend_bagan WHERE event_id = ?"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let mut nt_bagans_map: HashMap<i64, Vec<i64>> = HashMap::new();
        for (b_id, nt_id_opt, _) in &bagans_raw {
            if let Some(nt_id) = nt_id_opt {
                nt_bagans_map.entry(*nt_id).or_default().push(*b_id);
            }
        }

        let active_tatamis_raw: Vec<(i64, Option<i64>)> = sqlx::query_as(
            "SELECT tatami_number, detail_bagan_id FROM backend_tatami WHERE event_id = ? AND detail_bagan_id IS NOT NULL"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let mut active_detail_tatami_map: HashMap<i64, i64> = HashMap::new();
        for (t_num, d_id_opt) in active_tatamis_raw {
            if let Some(d_id) = d_id_opt {
                active_detail_tatami_map.insert(d_id, t_num);
            }
        }

        let detail_bagans_raw: Vec<(i64, i64, bool)> = sqlx::query_as(
            "SELECT db.id, db.bagan_id, db.selesai FROM backend_detailbagan db JOIN backend_bagan b ON db.bagan_id = b.id WHERE b.event_id = ?"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let mut bagan_matches: HashMap<i64, (i64, i64, bool)> = HashMap::new();
        for (d_id, b_id, selesai) in detail_bagans_raw {
            let entry = bagan_matches.entry(b_id).or_insert((0, 0, false));
            entry.0 += 1;
            if selesai {
                entry.1 += 1;
            }
            if active_detail_tatami_map.contains_key(&d_id) {
                entry.2 = true;
            }
        }

        struct InternalCatProgress {
            status: String,
            total_matches: i64,
            finished_matches: i64,
            remaining_matches: i64,
            progress_pct: f64,
        }

        let mut cat_progress_map: HashMap<i64, InternalCatProgress> = HashMap::new();
        for (nt_id, _, _, _) in &categories_raw {
            let b_list = nt_bagans_map.get(nt_id);
            let ath_cnt = athlete_counts.get(nt_id).copied().unwrap_or(0);
            if let Some(bagans) = b_list {
                let mut nt_total = 0;
                let mut nt_fin = 0;
                let mut nt_active = false;
                for b_id in bagans {
                    if let Some(&(tot, fin, act)) = bagan_matches.get(b_id) {
                        nt_total += tot;
                        nt_fin += fin;
                        if act { nt_active = true; }
                    }
                }
                let nt_all_fin = nt_total > 0 && nt_fin >= nt_total;
                let status = if nt_all_fin {
                    "selesai".to_string()
                } else if nt_active || nt_fin > 0 {
                    "running".to_string()
                } else {
                    "belum_disentuh".to_string()
                };
                let remaining = std::cmp::max(0, nt_total - nt_fin);
                let pct = if nt_total > 0 {
                    ((nt_fin as f64 / nt_total as f64) * 100.0 * 10.0).round() / 10.0
                } else if nt_all_fin {
                    100.0
                } else {
                    0.0
                };
                cat_progress_map.insert(*nt_id, InternalCatProgress {
                    status,
                    total_matches: nt_total,
                    finished_matches: nt_fin,
                    remaining_matches: remaining,
                    progress_pct: pct,
                });
            } else {
                let nt_total = if ath_cnt > 1 { ath_cnt - 1 } else if ath_cnt == 1 { 1 } else { 0 };
                cat_progress_map.insert(*nt_id, InternalCatProgress {
                    status: "belum_disentuh".to_string(),
                    total_matches: nt_total,
                    finished_matches: 0,
                    remaining_matches: nt_total,
                    progress_pct: 0.0,
                });
            }
        }

        // 6. Muat Atlet untuk Algoritma Deteksi Konflik Lintas Tatami
        let athletes_raw: Vec<(i64, Option<String>, Option<i64>, Option<String>, Option<i64>)> = sqlx::query_as(
            "SELECT a.id, a.nama_atlet, a.nomor_tanding_id, nt.nama_nomor_tanding, a.utusan_id
             FROM backend_atlet a
             LEFT JOIN backend_nomortanding nt ON a.nomor_tanding_id = nt.id
             WHERE a.event_id = ? AND a.nomor_tanding_id IS NOT NULL"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let mut individual_athletes: HashSet<String> = HashSet::new();
        let mut individual_by_utusan: HashMap<i64, Vec<String>> = HashMap::new();
        let mut athlete_names: HashMap<String, String> = HashMap::new();

        for (_id, name_opt, _nt_id_opt, nt_nama_opt, u_id_opt) in &athletes_raw {
            let raw_name = name_opt.as_deref().unwrap_or("").trim();
            let nt_nama = nt_nama_opt.as_deref().unwrap_or("").to_lowercase();
            if !nt_nama.contains("beregu") {
                let norm = helper_normalize_name(raw_name);
                if norm.len() >= 3 {
                    individual_athletes.insert(norm.clone());
                    athlete_names.insert(norm.clone(), raw_name.to_string());
                    if let Some(u_id) = u_id_opt {
                        individual_by_utusan.entry(*u_id).or_default().push(norm);
                    }
                }
            }
        }

        let stop_words = [
            "TEAM", "DOJO", "INKANAS", "FORKI", "LEMKARI", "WADOKAI", "KUTAI",
            "SAMARINDA", "BALIKPAPAN", "PUTRA", "PUTRI", "GABUNGAN", "CAMPURAN",
            "CS", "DAN", "AND", "BIN", "BINTI"
        ];

        let mut nt_athletes_map: HashMap<i64, Vec<String>> = HashMap::new();
        for (_id, name_opt, nt_id_opt, _nt_nama_opt, u_id_opt) in &athletes_raw {
            if let Some(nt_id) = nt_id_opt {
                let raw_name = name_opt.as_deref().unwrap_or("").trim();
                let norm = helper_normalize_name(raw_name);
                if norm.is_empty() {
                    continue;
                }

                let list = nt_athletes_map.entry(*nt_id).or_default();
                if !list.contains(&norm) {
                    list.push(norm.clone());
                }
                if !athlete_names.contains_key(&norm) {
                    athlete_names.insert(norm.clone(), raw_name.to_string());
                }

                // Cross check kata beregu / nama ganda dengan atlet perorangan
                for ind in &individual_athletes {
                    let words_count = ind.split_whitespace().count();
                    if words_count >= 2 && (norm.contains(ind) || ind.contains(&norm)) {
                        let l = nt_athletes_map.entry(*nt_id).or_default();
                        if !l.contains(ind) {
                            l.push(ind.clone());
                        }
                    }
                }

                // Panggilan di dalam kurung e.g. '(FAJAR CS)' - lingkup satu utusan/dojo
                if let Some(u_id) = u_id_opt {
                    if let Some(dojo_inds) = individual_by_utusan.get(u_id) {
                        let parens = helper_extract_parentheses(raw_name);
                        for p in parens {
                            let cleaned = p.replace("CS", " ");
                            let tokens: Vec<String> = cleaned.split_whitespace()
                                .map(|s| s.to_ascii_uppercase())
                                .filter(|s| s.len() >= 3 && !stop_words.contains(&s.as_str()))
                                .collect();

                            for t in &tokens {
                                for ind in dojo_inds {
                                    if ind.split_whitespace().any(|part| part == t) {
                                        let l = nt_athletes_map.entry(*nt_id).or_default();
                                        if !l.contains(ind) {
                                            l.push(ind.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 7. Muat Rows dan Cells
        let day_ids: Vec<i64> = days.iter().map(|d| d.id).collect();
        let mut rows_by_day: HashMap<i64, Vec<TimetableRow>> = HashMap::new();

        if !day_ids.is_empty() {
            let placeholders = day_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let query_str = format!(
                "SELECT id, day_id, [order], row_type, time_label, label_text FROM backend_timetablerow WHERE day_id IN ({}) ORDER BY [order] ASC",
                placeholders
            );
            let mut query = sqlx::query_as::<_, TimetableRow>(&query_str);
            for d_id in &day_ids {
                query = query.bind(d_id);
            }
            let all_rows = query.fetch_all(pool).await.unwrap_or_default();
            for r in all_rows {
                if let Some(d_id) = r.day_id {
                    rows_by_day.entry(d_id).or_default().push(r);
                }
            }
        }

        let mut row_ids: Vec<i64> = Vec::new();
        for rows in rows_by_day.values() {
            for r in rows {
                row_ids.push(r.id);
            }
        }

        let mut cells_by_row: HashMap<i64, HashMap<i64, TimetableCell>> = HashMap::new();
        if !row_ids.is_empty() {
            let placeholders = row_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
            let query_str = format!(
                "SELECT id, row_id, tatami_id, nomor_tanding_id, custom_text FROM backend_timetablecell WHERE row_id IN ({})",
                placeholders
            );
            let mut query = sqlx::query_as::<_, TimetableCell>(&query_str);
            for r_id in &row_ids {
                query = query.bind(r_id);
            }
            let all_cells = query.fetch_all(pool).await.unwrap_or_default();
            for c in all_cells {
                cells_by_row.entry(c.row_id).or_default().insert(c.tatami_id, c);
            }
        }

        // 8. Bentuk Enriched Days, Rows, Cells dan Jalankan Deteksi Konflik
        let mut enriched_days = Vec::new();
        let mut total_conflict_count = 0;
        let mut assigned_categories: HashMap<i64, (i32, i32)> = HashMap::new();

        for day in &days {
            let rows = rows_by_day.remove(&day.id).unwrap_or_default();
            let mut enriched_rows = Vec::new();

            for row in rows {
                let mut cells_map = cells_by_row.remove(&row.id).unwrap_or_default();
                let mut enriched_cells = Vec::new();

                for tatami in &tatamis {
                    let t_num = tatami.tatami_number.unwrap_or(1);
                    if let Some(c) = cells_map.remove(&tatami.id) {
                        let nt_id = c.nomor_tanding_id;
                        let mut nt_nama = None;
                        let mut atlet_cnt = 0;
                        let mut matches = 0;
                        let mut discipline = "kumite".to_string();
                        let mut est_dur = 15;
                        let mut prog_status = "belum_disentuh".to_string();
                        let mut prog_pct = 0.0;
                        let mut finished_matches = 0;
                        let mut remaining_matches = 0;
                        let mut is_fest = false;
                        let mut is_break = false;

                        if let Some(nid) = nt_id {
                            assigned_categories.insert(nid, (t_num, day.order));
                            if let Some(cat) = categories_raw.iter().find(|c| c.0 == nid) {
                                nt_nama = cat.1.clone();
                                let upper = nt_nama.as_deref().unwrap_or("").to_uppercase();
                                is_fest = upper.contains("FESTIVAL");
                                let is_kata = upper.contains("KATA");
                                discipline = if is_kata { "kata".to_string() } else { "kumite".to_string() };
                                atlet_cnt = athlete_counts.get(&nid).copied().unwrap_or(0);
                                let dur_per_match = if is_kata {
                                    3.5
                                } else if upper.contains("SENIOR") || upper.contains("U-21") || upper.contains("UNDER 21") {
                                    6.0
                                } else if upper.contains("CADET") || upper.contains("KADET") || upper.contains("JUNIOR") {
                                    4.5
                                } else {
                                    4.0
                                };
                                if atlet_cnt > 1 {
                                    matches = atlet_cnt - 1;
                                    est_dur = std::cmp::max(15, (matches as f64 * dur_per_match).round() as i64);
                                } else if atlet_cnt == 1 {
                                    matches = 0;
                                    est_dur = 15;
                                } else {
                                    matches = 0;
                                    est_dur = 0;
                                }

                                if let Some(prog) = cat_progress_map.get(&nid) {
                                    prog_status = prog.status.clone();
                                    prog_pct = prog.progress_pct;
                                    finished_matches = prog.finished_matches;
                                    remaining_matches = prog.remaining_matches;
                                }
                            }
                        } else if !c.custom_text.trim().is_empty() {
                            let ct_upper = c.custom_text.to_uppercase();
                            if ct_upper.contains("FESTIVAL") {
                                is_fest = true;
                                prog_status = "festival".to_string();
                            } else if ct_upper.contains("ISHOMA") || ct_upper.contains("BREAK") || ct_upper.contains("ISTIRAHAT") || ct_upper.contains("JEDA") {
                                is_break = true;
                                prog_status = "break".to_string();
                            }
                        }

                        enriched_cells.push(EnrichedTimetableCell {
                            id: c.id,
                            row_id: row.id,
                            tatami_id: tatami.id,
                            tatami_number: t_num,
                            nomor_tanding_id: nt_id,
                            nomor_tanding_nama: nt_nama,
                            custom_text: c.custom_text,
                            atlet_count: atlet_cnt,
                            matches,
                            discipline,
                            est_duration_minutes: est_dur,
                            progress_status: prog_status,
                            progress_pct: prog_pct,
                            finished_matches,
                            remaining_matches,
                            is_festival: is_fest,
                            is_break,
                        });
                    } else {
                        enriched_cells.push(EnrichedTimetableCell {
                            id: 0,
                            row_id: row.id,
                            tatami_id: tatami.id,
                            tatami_number: t_num,
                            nomor_tanding_id: None,
                            nomor_tanding_nama: None,
                            custom_text: String::new(),
                            atlet_count: 0,
                            matches: 0,
                            discipline: "kumite".to_string(),
                            est_duration_minutes: 0,
                            progress_status: "belum_disentuh".to_string(),
                            progress_pct: 0.0,
                            finished_matches: 0,
                            remaining_matches: 0,
                            is_festival: false,
                            is_break: false,
                        });
                    }
                }

                // Deteksi bentrok atlet pada row yang sama antar tatami berbeda
                let mut row_conflicts = Vec::new();
                if row.row_type == "slot" {
                    for i in 0..enriched_cells.len() {
                        for j in (i + 1)..enriched_cells.len() {
                            let cell_a = &enriched_cells[i];
                            let cell_b = &enriched_cells[j];
                            if cell_a.tatami_id == cell_b.tatami_id {
                                continue;
                            }
                            if let (Some(nt_a), Some(nt_b)) = (cell_a.nomor_tanding_id, cell_b.nomor_tanding_id) {
                                if let (Some(ath_a), Some(ath_b)) = (nt_athletes_map.get(&nt_a), nt_athletes_map.get(&nt_b)) {
                                    for ath in ath_a {
                                        if ath_b.contains(ath) {
                                            let display_name = athlete_names.get(ath).cloned().unwrap_or_else(|| ath.clone());
                                            row_conflicts.push(AthleteConflictDetail {
                                                athlete_name: display_name,
                                                tatami1_number: cell_a.tatami_number,
                                                category1_id: nt_a,
                                                category1_name: cell_a.nomor_tanding_nama.clone().unwrap_or_default(),
                                                tatami2_number: cell_b.tatami_number,
                                                category2_id: nt_b,
                                                category2_name: cell_b.nomor_tanding_nama.clone().unwrap_or_default(),
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                total_conflict_count += row_conflicts.len();

                enriched_rows.push(EnrichedTimetableRow {
                    id: row.id,
                    day_id: day.id,
                    order: row.order,
                    row_type: row.row_type,
                    time_label: row.time_label,
                    label_text: row.label_text,
                    cells: enriched_cells,
                    conflicts: row_conflicts,
                });
            }

            let day_label = helper_format_day_label(day.order, &day.tanggal);
            enriched_days.push(EnrichedTimetableDay {
                id: day.id,
                event_id: day.event_id,
                order: day.order,
                tanggal: day.tanggal.clone(),
                label: day_label,
                rows: enriched_rows,
            });
        }

        // 9. Bentuk Daftar Kategori Lengkap untuk Drawer / Picker
        let mut categories: Vec<TimetableCategoryOption> = Vec::new();
        for (nt_id, name_opt, _, _) in &categories_raw {
            let name_str = name_opt.clone().unwrap_or_else(|| format!("Kategori #{}", nt_id));
            let age_grp = helper_get_age_group(&name_str);
            let upper = name_str.to_uppercase();
            let is_fest = upper.contains("FESTIVAL");
            let is_kata = upper.contains("KATA");
            let discipline = if is_kata { "kata".to_string() } else { "kumite".to_string() };
            let cnt = athlete_counts.get(nt_id).copied().unwrap_or(0);
            let dur_per_match = if is_kata {
                3.5
            } else if upper.contains("SENIOR") || upper.contains("U-21") || upper.contains("UNDER 21") {
                6.0
            } else if upper.contains("CADET") || upper.contains("KADET") || upper.contains("JUNIOR") {
                4.5
            } else {
                4.0
            };
            let (matches, est_dur) = if cnt > 1 {
                let m = cnt - 1;
                (m, std::cmp::max(15, (m as f64 * dur_per_match).round() as i64))
            } else if cnt == 1 {
                (0, 15)
            } else {
                (0, 0)
            };

            let prog = cat_progress_map.get(nt_id);
            let status = prog.map(|p| p.status.clone()).unwrap_or_else(|| "belum_disentuh".to_string());
            let pct = prog.map(|p| p.progress_pct).unwrap_or(0.0);

            let assigned_info = assigned_categories.get(nt_id);

            categories.push(TimetableCategoryOption {
                id: *nt_id,
                nama: name_str,
                age_group: age_grp,
                atlet_count: cnt,
                matches,
                discipline,
                est_duration_minutes: est_dur,
                is_festival: is_fest,
                progress_status: status,
                progress_pct: pct,
                is_assigned: assigned_info.is_some(),
                assigned_tatami: assigned_info.map(|(t, _)| *t),
                assigned_day_order: assigned_info.map(|(_, d)| *d),
            });
        }

        // 10. Summary Progress Keseluruhan
        let total_m = cat_progress_map.values().map(|p| p.total_matches).sum();
        let fin_m = cat_progress_map.values().map(|p| p.finished_matches).sum();
        let rem_m = cat_progress_map.values().map(|p| p.remaining_matches).sum();
        let run_cnt = cat_progress_map.values().filter(|p| p.status == "running").count();
        let fin_cnt = cat_progress_map.values().filter(|p| p.status == "selesai").count();
        let unstart_cnt = cat_progress_map.values().filter(|p| p.status == "belum_disentuh").count();
        let tot_cnt = categories.len();
        let overall_pct = if total_m > 0 {
            ((fin_m as f64 / total_m as f64) * 100.0 * 10.0).round() / 10.0
        } else {
            0.0
        };

        // Live status match tiap tatami
        let live_tatami_rows: Vec<(i64, i32, Option<String>, Option<i32>, Option<i32>, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT t.id, t.tatami_number, nt.nama_nomor_tanding, db.round, db.urutan, a1.nama_atlet, a2.nama_atlet
             FROM backend_tatami t
             LEFT JOIN backend_detailbagan db ON t.detail_bagan_id = db.id AND db.selesai = 0
             LEFT JOIN backend_bagan b ON db.bagan_id = b.id
             LEFT JOIN backend_nomortanding nt ON b.nomor_tanding_id = nt.id
             LEFT JOIN backend_atlet a1 ON db.atlet1_id = a1.id
             LEFT JOIN backend_atlet a2 ON db.atlet2_id = a2.id
             WHERE t.event_id = ?
             ORDER BY t.tatami_number ASC"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let mut tatamis_live = Vec::new();
        for (t_id, t_num, cat_opt, round_opt, urutan_opt, a1_opt, a2_opt) in live_tatami_rows {
            let is_running = cat_opt.is_some();
            tatamis_live.push(TatamiLiveStatus {
                tatami_id: t_id,
                tatami_number: t_num,
                is_running,
                category_name: cat_opt,
                round: round_opt,
                urutan: urutan_opt,
                atlet1_name: a1_opt,
                atlet2_name: a2_opt,
            });
        }

        // Kop Surat Event
        let kop_surat: Option<KopSurat> = sqlx::query_as(
            "SELECT id, event_id, logo, nama_organisasi, alamat, kontak FROM backend_kopsurat WHERE event_id = ?"
        )
        .bind(event_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        let summary = TimetableProgressSummary {
            total_matches: total_m,
            finished_matches: fin_m,
            remaining_matches: rem_m,
            running_categories_count: run_cnt,
            finished_categories_count: fin_cnt,
            unstarted_categories_count: unstart_cnt,
            total_categories_count: tot_cnt,
            overall_pct,
            total_conflicts: total_conflict_count,
            tatamis_live,
        };

        Ok(TimetableEditorData {
            event_id,
            event_name,
            tatamis,
            days: enriched_days,
            categories,
            summary,
            kop_surat,
            nt_athletes: nt_athletes_map,
        })
    }

    /// Menyimpan perubahan baris dan cell pada timetable secara transaksional
    pub async fn save_timetable_data(&self, event_id: i64, payload: SaveTimetablePayload) -> Result<(), String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        for (day_idx, day_input) in payload.days.into_iter().enumerate() {
            // Validasi hari milik event
            let day_exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM backend_timetableday WHERE id = ? AND event_id = ?")
                .bind(day_input.day_id)
                .bind(event_id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(|e| format!("Gagal memverifikasi hari: {}", e))?;

            if day_exists.is_none() {
                continue;
            }

            // Update urutan [order] dan tanggal hari agar hari selalu terindeks 0, 1, 2...
            let tanggal_val = match day_input.tanggal.as_deref() {
                Some(s) if !s.trim().is_empty() => Some(s.trim().to_string()),
                _ => None,
            };
            sqlx::query("UPDATE backend_timetableday SET [order] = ?, tanggal = ? WHERE id = ?")
                .bind(day_idx as i32)
                .bind(tanggal_val)
                .bind(day_input.day_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal memperbarui tanggal dan urutan hari: {}", e))?;

            // Hapus cell lama dan row lama untuk hari ini
            sqlx::query("DELETE FROM backend_timetablecell WHERE row_id IN (SELECT id FROM backend_timetablerow WHERE day_id = ?)")
                .bind(day_input.day_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal menghapus cell lama: {}", e))?;

            sqlx::query("DELETE FROM backend_timetablerow WHERE day_id = ?")
                .bind(day_input.day_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal menghapus baris lama: {}", e))?;

            // Simpan baris baru dan cell baru
            for (idx, row_input) in day_input.rows.into_iter().enumerate() {
                let row_order = idx as i32;
                let insert_res = sqlx::query(
                    "INSERT INTO backend_timetablerow (day_id, [order], row_type, time_label, label_text) VALUES (?, ?, ?, ?, ?)"
                )
                .bind(day_input.day_id)
                .bind(row_order)
                .bind(&row_input.row_type)
                .bind(&row_input.time_label)
                .bind(&row_input.label_text)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal menyimpan baris jadwal: {}", e))?;

                let new_row_id = insert_res.last_insert_rowid();

                if row_input.row_type == "slot" {
                    for cell_input in row_input.cells {
                        if cell_input.tatami_id > 0 {
                            sqlx::query(
                                "INSERT INTO backend_timetablecell (row_id, tatami_id, nomor_tanding_id, custom_text) VALUES (?, ?, ?, ?)"
                            )
                            .bind(new_row_id)
                            .bind(cell_input.tatami_id)
                            .bind(cell_input.nomor_tanding_id)
                            .bind(&cell_input.custom_text)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| format!("Gagal menyimpan cell jadwal: {}", e))?;
                        }
                    }
                }
            }
        }

        tx.commit().await.map_err(|e| format!("Gagal commit transaksi jadwal: {}", e))?;
        Ok(())
    }

    /// Menambahkan hari baru pada timetable
    pub async fn add_timetable_day(&self, event_id: i64) -> Result<TimetableDay, String> {
        let pool = self.get_pool()?;
        let next_order: (i32,) = sqlx::query_as("SELECT COALESCE(MAX([order]), -1) + 1 FROM backend_timetableday WHERE event_id = ?")
            .bind(event_id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Gagal menghitung urutan hari: {}", e))?;

        let insert_res = sqlx::query("INSERT INTO backend_timetableday (event_id, [order], tanggal) VALUES (?, ?, NULL)")
            .bind(event_id)
            .bind(next_order.0)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal menambahkan hari baru: {}", e))?;

        let new_id = insert_res.last_insert_rowid();

        Ok(TimetableDay {
            id: new_id,
            event_id,
            order: next_order.0,
            tanggal: None,
        })
    }

    /// Menghapus hari beserta baris dan cell pada timetable, lalu me-reindex sisa hari agar urutannya selalu 0, 1, 2...
    pub async fn delete_timetable_day(&self, day_id: i64) -> Result<(), String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        // 1. Dapatkan event_id sebelum baris hari dihapus
        let event_row: Option<(i64,)> = sqlx::query_as("SELECT event_id FROM backend_timetableday WHERE id = ?")
            .bind(day_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("Gagal membaca event hari: {}", e))?;

        let event_id = event_row.map(|r| r.0);

        // 2. Hapus cell dan baris
        sqlx::query("DELETE FROM backend_timetablecell WHERE row_id IN (SELECT id FROM backend_timetablerow WHERE day_id = ?)")
            .bind(day_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghapus cell: {}", e))?;

        sqlx::query("DELETE FROM backend_timetablerow WHERE day_id = ?")
            .bind(day_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghapus baris: {}", e))?;

        sqlx::query("DELETE FROM backend_timetableday WHERE id = ?")
            .bind(day_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghapus hari: {}", e))?;

        // 3. Re-index sisa hari agar urutan [order] selalu berurutan (0, 1, 2, ...) sehingga jika Hari 1 dihapus, Hari 2 otomatis jadi Hari 1!
        if let Some(ev_id) = event_id {
            let remaining_days: Vec<(i64,)> = sqlx::query_as(
                "SELECT id FROM backend_timetableday WHERE event_id = ? ORDER BY [order] ASC, id ASC"
            )
            .bind(ev_id)
            .fetch_all(&mut *tx)
            .await
            .unwrap_or_default();

            for (idx, (d_id,)) in remaining_days.iter().enumerate() {
                sqlx::query("UPDATE backend_timetableday SET [order] = ? WHERE id = ?")
                    .bind(idx as i32)
                    .bind(d_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| format!("Gagal memperbarui urutan hari: {}", e))?;
            }
        }

        tx.commit().await.map_err(|e| format!("Gagal commit hapus hari: {}", e))?;
        Ok(())
    }

    /// Memuat data lengkap Buku Tatami / LO Booklet (Sampul, Roster, dan Bagan Berurutan)
    pub async fn get_tatami_booklet_data(
        &self,
        event_id: i64,
        day_id: i64,
        tatami_id: i64,
    ) -> Result<TatamiBookletData, String> {
        let pool = self.get_pool()?;

        // 1. Info Event
        let event_row: Option<(String,)> = sqlx::query_as("SELECT COALESCE(nama_event, 'Turnamen') FROM backend_event WHERE id = ?")
            .bind(event_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Gagal memuat event: {}", e))?;
        let event_name = event_row.map(|r| r.0).unwrap_or_else(|| "Turnamen".to_string());

        // 2. Kop Surat
        let kop_surat = self.get_kop_surat(event_id).await.unwrap_or(None);

        // 3. Info Tatami
        let tatami_row: Option<(i64,)> = sqlx::query_as("SELECT tatami_number FROM backend_tatami WHERE id = ?")
            .bind(tatami_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Gagal memuat tatami: {}", e))?;
        let tatami_number = tatami_row.map(|r| r.0).unwrap_or(1);

        // 4. Info Hari
        let day_row: Option<(i32, Option<String>)> = sqlx::query_as("SELECT [order], tanggal FROM backend_timetableday WHERE id = ?")
            .bind(day_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Gagal memuat hari: {}", e))?;
        let (day_order, day_date) = day_row.unwrap_or((0, None));
        let day_label = helper_format_day_label(day_order, &day_date);

        // 5. Sel terjadwal pada tatami ini di hari ini
        let cells_raw: Vec<(i64, Option<i64>, Option<String>, Option<String>)> = sqlx::query_as(
            r#"
            SELECT r.[order], c.nomor_tanding_id, n.nama_nomor_tanding, c.custom_text
            FROM backend_timetablerow r
            JOIN backend_timetablecell c ON c.row_id = r.id
            LEFT JOIN backend_nomortanding n ON c.nomor_tanding_id = n.id
            WHERE r.day_id = ? AND c.tatami_id = ? AND r.row_type = 'slot'
            ORDER BY r.[order] ASC
            "#
        )
        .bind(day_id)
        .bind(tatami_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat sel tatami: {}", e))?;

        let mut ordered_categories: Vec<TatamiBookletCategory> = Vec::new();
        let mut seq_counter = 0;

        for (_order, nt_id_opt, nt_name_opt, custom_opt) in cells_raw {
            if let Some(nt_id) = nt_id_opt {
                seq_counter += 1;
                let cat_name = nt_name_opt.unwrap_or_else(|| format!("Kategori #{}", nt_id));
                let is_kata = cat_name.to_lowercase().contains("kata");
                let discipline = if is_kata { "kata".to_string() } else { "kumite".to_string() };

                // Hitung atlet
                let ath_count: i64 = sqlx::query_scalar("SELECT COUNT(id) FROM backend_atlet WHERE nomor_tanding_id = ?")
                    .bind(nt_id)
                    .fetch_one(pool)
                    .await
                    .unwrap_or(0);

                // Cari bagan untuk kategori ini
                let bagan_row: Option<(i64, Option<String>, Option<String>, Option<String>, Option<i64>, Option<i64>, Option<i64>, Option<i64>)> = sqlx::query_as(
                    "SELECT id, nama_bagan, kode, tipe_tanding, juara_1_id, juara_2_id, juara_3a_id, juara_3b_id FROM backend_bagan WHERE event_id = ? AND nomor_tanding_id = ? LIMIT 1"
                )
                .bind(event_id)
                .bind(nt_id)
                .fetch_optional(pool)
                .await
                .unwrap_or(None);

                let mut winners: Vec<SummaryBaganWinner> = Vec::new();

                let (bagan_id, bagan_nama, bagan_kode, tipe_tanding, matches) = match bagan_row {
                    Some((b_id, b_nama, b_kode, b_tipe, j1, j2, j3a, j3b)) => {
                        let matches = self.get_detail_bagan(b_id).await.unwrap_or_default();

                        // Query juara untuk Hasil Juara podium box
                        let rank_tuples = vec![
                            ("1", j1),
                            ("2", j2),
                            ("3a", j3a),
                            ("3b", j3b),
                        ];

                        for (rank, opt_aid) in rank_tuples {
                            if let Some(aid) = opt_aid {
                                let ath_info: Option<(String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
                                    r#"
                                    SELECT a.nama_atlet, p.nama_perguruan, u.nama_utusan, u.logo
                                    FROM backend_atlet a
                                    LEFT JOIN backend_perguruan p ON a.perguruan_id = p.id
                                    LEFT JOIN backend_utusan u ON a.utusan_id = u.id
                                    WHERE a.id = ?
                                    "#
                                )
                                .bind(aid)
                                .fetch_optional(pool)
                                .await
                                .unwrap_or(None);

                                if let Some((nama_atlet, nama_p, nama_u, logo)) = ath_info {
                                    winners.push(SummaryBaganWinner {
                                        juara: rank.to_string(),
                                        athlete_id: aid,
                                        nama_atlet,
                                        perguruan_nama: nama_p.unwrap_or_else(|| "-".to_string()),
                                        utusan_nama: nama_u.unwrap_or_else(|| "-".to_string()),
                                        logo,
                                        is_uncounted: false,
                                    });
                                }
                            }
                        }

                        (Some(b_id), b_nama, b_kode, b_tipe, matches)
                    }
                    None => (None, None, None, None, Vec::new()),
                };

                let matches_count = matches.len() as i64;

                ordered_categories.push(TatamiBookletCategory {
                    match_seq: seq_counter,
                    category_id: nt_id,
                    category_name: cat_name,
                    discipline,
                    matches_count,
                    athletes_count: ath_count,
                    bagan_id,
                    bagan_kode,
                    bagan_nama,
                    tipe_tanding,
                    winners,
                    matches,
                });
            } else if let Some(cust) = custom_opt {
                if !cust.trim().is_empty() {
                    seq_counter += 1;
                    ordered_categories.push(TatamiBookletCategory {
                        match_seq: seq_counter,
                        category_id: 0,
                        category_name: cust,
                        discipline: "festival".to_string(),
                        matches_count: 0,
                        athletes_count: 0,
                        bagan_id: None,
                        bagan_kode: None,
                        bagan_nama: None,
                        tipe_tanding: None,
                        winners: Vec::new(),
                        matches: Vec::new(),
                    });
                }
            }
        }

        // 6. Hitung kelas yang tidak ditandingkan (unscheduled list)
        let scheduled_nt_ids: Vec<i64> = sqlx::query_scalar(
            "SELECT DISTINCT c.nomor_tanding_id FROM backend_timetablecell c JOIN backend_timetablerow r ON c.row_id = r.id JOIN backend_timetableday d ON r.day_id = d.id WHERE d.event_id = ? AND c.nomor_tanding_id IS NOT NULL"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let scheduled_set: std::collections::HashSet<i64> = scheduled_nt_ids.into_iter().collect();

        let all_nts: Vec<(i64, String)> = sqlx::query_as(
            "SELECT id, nama_nomor_tanding FROM backend_nomortanding WHERE event_id = ? ORDER BY nama_nomor_tanding ASC"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let mut unscheduled_list = Vec::new();
        for (nt_id, nt_nama) in all_nts {
            if !scheduled_set.contains(&nt_id) && !nt_nama.to_lowercase().contains("festival") {
                let cnt: i64 = sqlx::query_scalar("SELECT COUNT(id) FROM backend_atlet WHERE nomor_tanding_id = ?")
                    .bind(nt_id)
                    .fetch_one(pool)
                    .await
                    .unwrap_or(0);
                unscheduled_list.push(format!("{} ({})", nt_nama, cnt));
            }
        }

        Ok(TatamiBookletData {
            event_id,
            event_name,
            kop_surat,
            tatami_id,
            tatami_number,
            day_id,
            day_order,
            day_label,
            day_date,
            unscheduled_list,
            ordered_categories,
        })
    }

    /// Mereset seluruh jadwal pertandingan untuk suatu event (menghapus semua sel dan baris)
    pub async fn reset_timetable_data(&self, event_id: i64) -> Result<(), String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        // 1. Hapus semua cell jadwal event ini
        sqlx::query(
            "DELETE FROM backend_timetablecell WHERE row_id IN (
                SELECT r.id FROM backend_timetablerow r
                JOIN backend_timetableday d ON r.day_id = d.id
                WHERE d.event_id = ?
            )"
        )
        .bind(event_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Gagal membersihkan sel jadwal: {}", e))?;

        // 2. Hapus semua baris jadwal event ini
        sqlx::query(
            "DELETE FROM backend_timetablerow WHERE day_id IN (
                SELECT id FROM backend_timetableday WHERE event_id = ?
            )"
        )
        .bind(event_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Gagal membersihkan baris jadwal: {}", e))?;

        // 3. Hapus hari tambahan selain Hari 1
        sqlx::query("DELETE FROM backend_timetableday WHERE event_id = ? AND [order] > 0")
            .bind(event_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal mereset hari jadwal tambahan: {}", e))?;

        // 4. Pastikan minimal Hari 1 tetap ada
        let count_days: (i64,) = sqlx::query_as("SELECT COUNT(id) FROM backend_timetableday WHERE event_id = ?")
            .bind(event_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghitung hari jadwal: {}", e))?;

        if count_days.0 == 0 {
            sqlx::query("INSERT INTO backend_timetableday (event_id, [order], tanggal) VALUES (?, 0, NULL)")
                .bind(event_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal membuat hari default: {}", e))?;
        }

        tx.commit().await.map_err(|e| format!("Gagal menyelesaikan reset jadwal: {}", e))?;
        Ok(())
    }

    /// Memuat pengaturan kop surat event
    pub async fn get_kop_surat(&self, event_id: i64) -> Result<Option<KopSurat>, String> {
        let pool = self.get_pool()?;
        let kop: Option<KopSurat> = sqlx::query_as(
            "SELECT id, event_id, logo, nama_organisasi, alamat, kontak FROM backend_kopsurat WHERE event_id = ?"
        )
        .bind(event_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Gagal memuat kop surat: {}", e))?;
        Ok(kop)
    }

    /// Menyimpan pengaturan kop surat event
    pub async fn save_kop_surat(
        &self,
        event_id: i64,
        nama_organisasi: String,
        alamat: String,
        kontak: String,
        logo: Option<String>,
    ) -> Result<KopSurat, String> {
        let pool = self.get_pool()?;
        let existing: Option<(i64, Option<String>)> = sqlx::query_as("SELECT id, logo FROM backend_kopsurat WHERE event_id = ?")
            .bind(event_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Gagal memeriksa kop surat: {}", e))?;

        if let Some((id, old_logo)) = existing {
            let final_logo = if logo.is_some() { logo } else { old_logo };
            sqlx::query(
                "UPDATE backend_kopsurat SET nama_organisasi = ?, alamat = ?, kontak = ?, logo = ? WHERE id = ?"
            )
            .bind(&nama_organisasi)
            .bind(&alamat)
            .bind(&kontak)
            .bind(&final_logo)
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal memperbarui kop surat: {}", e))?;

            Ok(KopSurat {
                id,
                event_id,
                nama_organisasi,
                alamat,
                kontak,
                logo: final_logo,
            })
        } else {
            let res = sqlx::query(
                "INSERT INTO backend_kopsurat (event_id, nama_organisasi, alamat, kontak, logo) VALUES (?, ?, ?, ?, ?)"
            )
            .bind(event_id)
            .bind(&nama_organisasi)
            .bind(&alamat)
            .bind(&kontak)
            .bind(&logo)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal membuat kop surat baru: {}", e))?;

            Ok(KopSurat {
                id: res.last_insert_rowid(),
                event_id,
                nama_organisasi,
                alamat,
                kontak,
                logo,
            })
        }
    }

    // =========================================================================
    // FESTIVAL ROSTER MAKER METHODS
    // =========================================================================

    pub async fn get_festival_roster_data(&self, event_id: i64) -> Result<FestivalRosterData, String> {
        let pool = self.pool.as_ref().ok_or("Database tidak terhubung.")?;

        // 1. Get Event Name
        let event_name: String = sqlx::query_scalar("SELECT nama_event FROM backend_event WHERE id = ?")
            .bind(event_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| "Turnamen Karate".to_string());

        // 2. Get Kop Surat
        let kop_surat = self.get_kop_surat(event_id).await.ok().flatten();

        // 3. Get Tatamis
        let tatamis: Vec<Tatami> = sqlx::query_as(
            "SELECT id, tatami_number, detail_bagan_id, event_id FROM backend_tatami WHERE event_id = ? ORDER BY tatami_number ASC"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

        let tatamis = if tatamis.is_empty() {
            vec![Tatami {
                id: 1,
                tatami_number: Some(1),
                detail_bagan_id: None,
                event_id: Some(event_id),
                nama_bagan: None,
            }]
        } else {
            tatamis
        };

        // 4. Query All Festival Categories
        let cat_rows: Vec<(i64, String)> = sqlx::query_as(
            "SELECT id, nama_nomor_tanding FROM backend_nomortanding WHERE event_id = ? AND UPPER(nama_nomor_tanding) LIKE '%FESTIVAL%'"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

        let mut all_festival_nts: Vec<(i64, String)> = cat_rows;
        all_festival_nts.sort_by(|a, b| {
            helper_festival_category_sort_key(&a.1).cmp(&helper_festival_category_sort_key(&b.1))
        });

        let all_nt_ids: std::collections::HashSet<i64> = all_festival_nts.iter().map(|c| c.0).collect();
        let all_nt_map: std::collections::HashMap<i64, String> = all_festival_nts.iter().cloned().collect();

        // 5. Query Athletes for all festival categories
        let ath_rows: Vec<(i64, i64, String, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT a.id, a.nomor_tanding_id, a.nama_atlet, u.nama_utusan, p.nama_perguruan
             FROM backend_atlet a
             LEFT JOIN backend_utusan u ON a.utusan_id = u.id
             LEFT JOIN backend_perguruan p ON a.perguruan_id = p.id
             WHERE a.event_id = ? AND a.nomor_tanding_id IS NOT NULL"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

        let mut athletes_by_id: std::collections::HashMap<i64, FestivalAthlete> = std::collections::HashMap::new();
        let mut athletes_by_nt: std::collections::HashMap<i64, Vec<FestivalAthlete>> = std::collections::HashMap::new();

        for (a_id, nt_id, a_nama, a_utusan, a_perguruan) in ath_rows {
            if all_nt_ids.contains(&nt_id) {
                let nt_name = all_nt_map.get(&nt_id).map(|s| s.as_str()).unwrap_or("");
                let jk = if nt_name.to_lowercase().contains("putri") || nt_name.to_lowercase().contains("pi") {
                    "PUTRI".to_string()
                } else {
                    "PUTRA".to_string()
                };
                let fa = FestivalAthlete {
                    id: a_id,
                    nama: a_nama,
                    perguruan: a_perguruan.unwrap_or_else(|| "-".to_string()),
                    utusan: a_utusan.unwrap_or_else(|| "-".to_string()),
                    jenis_kelamin: jk,
                };
                athletes_by_id.insert(a_id, fa.clone());
                athletes_by_nt.entry(nt_id).or_default().push(fa);
            }
        }

        // 6. Load State
        let mut state = helper_load_festival_roster_state(self.active_db_path.as_deref(), event_id);

        // Sanitize tatami_assignments
        let mut cleaned_assignments: std::collections::HashMap<String, Vec<i64>> = std::collections::HashMap::new();
        let mut assigned_nt_ids = std::collections::HashSet::new();

        for t in &tatamis {
            let t_key = t.id.to_string();
            let mut valid_pks = Vec::new();
            if let Some(pks) = state.tatami_assignments.get(&t_key) {
                for pk in pks {
                    if all_nt_ids.contains(pk) && !assigned_nt_ids.contains(pk) {
                        valid_pks.push(*pk);
                        assigned_nt_ids.insert(*pk);
                    }
                }
            }
            cleaned_assignments.insert(t_key, valid_pks);
        }

        // Distribute unassigned categories evenly across tatamis
        let unassigned_nts: Vec<i64> = all_festival_nts
            .iter()
            .map(|c| c.0)
            .filter(|pk| !assigned_nt_ids.contains(pk))
            .collect();

        if !unassigned_nts.is_empty() {
            let t_count = tatamis.len();
            for (idx, nt_id) in unassigned_nts.into_iter().enumerate() {
                let target_tatami_id = tatamis[idx % t_count].id;
                cleaned_assignments
                    .entry(target_tatami_id.to_string())
                    .or_default()
                    .push(nt_id);
            }
        }

        // Re-sort categories inside each tatami according to WKF sort key (Kata first, Kumite second)
        for (_, pks) in cleaned_assignments.iter_mut() {
            pks.sort_by(|a, b| {
                let name_a = all_nt_map.get(a).map(|s| s.as_str()).unwrap_or("");
                let name_b = all_nt_map.get(b).map(|s| s.as_str()).unwrap_or("");
                helper_festival_category_sort_key(name_a).cmp(&helper_festival_category_sort_key(name_b))
            });
        }

        // Sanitize athlete pairings for each category
        let mut cleaned_pairings: std::collections::HashMap<String, Vec<i64>> = std::collections::HashMap::new();

        for (nt_id, _) in &all_festival_nts {
            let nt_key = nt_id.to_string();
            let current_athletes = athletes_by_nt.get(nt_id).cloned().unwrap_or_default();
            let curr_id_set: std::collections::HashSet<i64> = current_athletes.iter().map(|a| a.id).collect();

            let saved_order = state.athlete_pairings.get(&nt_key).cloned().unwrap_or_default();
            let saved_id_set: std::collections::HashSet<i64> = saved_order.iter().cloned().collect();

            if curr_id_set == saved_id_set && !saved_order.is_empty() {
                cleaned_pairings.insert(nt_key, saved_order);
            } else {
                // Run anti-collision auto-pairing
                let paired = helper_pair_festival_athletes_anti_collision(current_athletes);
                let paired_ids: Vec<i64> = paired.iter().map(|a| a.id).collect();
                cleaned_pairings.insert(nt_key, paired_ids);
            }
        }

        state.tatami_assignments = cleaned_assignments.clone();
        state.athlete_pairings = cleaned_pairings.clone();
        let _ = helper_save_festival_roster_state(self.active_db_path.as_deref(), event_id, &state);

        // 7. Assemble Sections, Categories, Matches, and Clashes
        let mut sections = Vec::new();
        let mut total_categories = 0;
        let mut total_athletes = 0;
        let mut total_matches = 0;
        let mut total_clashes = 0;

        for t in &tatamis {
            let t_key = t.id.to_string();
            let cat_pks = cleaned_assignments.get(&t_key).cloned().unwrap_or_default();
            let mut categories_data = Vec::new();
            let mut tatami_athlete_count = 0;
            let mut tatami_match_count = 0;

            for cat_id in cat_pks {
                let cat_name = match all_nt_map.get(&cat_id) {
                    Some(n) => n.clone(),
                    None => continue,
                };

                let paired_ids = cleaned_pairings.get(&cat_id.to_string()).cloned().unwrap_or_default();
                let mut cat_athletes = Vec::new();
                for aid in paired_ids {
                    if let Some(ath) = athletes_by_id.get(&aid) {
                        cat_athletes.push(ath.clone());
                    }
                }

                // Buat match pairs
                let mut matches = Vec::new();
                for i in (0..cat_athletes.len()).step_by(2) {
                    let aka = cat_athletes[i].clone();
                    let ao = if i + 1 < cat_athletes.len() {
                        Some(cat_athletes[i + 1].clone())
                    } else {
                        None
                    };

                    let is_clash = if let Some(ref ao_ath) = ao {
                        let u1 = aka.utusan.trim().to_uppercase();
                        let u2 = ao_ath.utusan.trim().to_uppercase();
                        !u1.is_empty() && !u2.is_empty() && u1 == u2
                    } else {
                        false
                    };

                    if is_clash {
                        total_clashes += 1;
                    }

                    matches.push(FestivalMatch {
                        match_num: (i / 2) + 1,
                        aka,
                        ao,
                        is_clash,
                    });
                }

                let discipline = if cat_name.to_lowercase().contains("kata") {
                    "kata".to_string()
                } else {
                    "kumite".to_string()
                };

                let display_name = helper_clean_festival_display_name(&cat_name);
                let ath_count = cat_athletes.len();
                let m_count = matches.len();

                tatami_athlete_count += ath_count;
                tatami_match_count += m_count;

                categories_data.push(FestivalCategory {
                    id: cat_id,
                    nama: cat_name,
                    display_name,
                    discipline,
                    tatami_id: t.id,
                    matches,
                    atlet_count: ath_count,
                    match_count: m_count,
                });
            }

            total_categories += categories_data.len();
            total_athletes += tatami_athlete_count;
            total_matches += tatami_match_count;

            sections.push(FestivalTatamiSection {
                tatami_id: t.id,
                tatami_number: t.tatami_number.unwrap_or(1) as i64,
                categories: categories_data,
                total_athletes: tatami_athlete_count,
                total_matches: tatami_match_count,
            });
        }

        Ok(FestivalRosterData {
            event_id,
            event_name,
            tatamis,
            sections,
            total_categories,
            total_athletes,
            total_matches,
            total_clashes,
            kop_surat,
        })
    }

    pub async fn save_festival_roster_state(&self, event_id: i64, state: FestivalRosterState) -> Result<(), String> {
        helper_save_festival_roster_state(self.active_db_path.as_deref(), event_id, &state)
    }

    pub async fn auto_pair_festival_category(&self, event_id: i64, nomor_tanding_id: Option<i64>) -> Result<FestivalRosterData, String> {
        let pool = self.pool.as_ref().ok_or("Database tidak terhubung.")?;

        let target_clause = if let Some(nt_id) = nomor_tanding_id {
            format!("AND a.nomor_tanding_id = {}", nt_id)
        } else {
            "AND UPPER(nt.nama_nomor_tanding) LIKE '%FESTIVAL%'".to_string()
        };

        let sql = format!(
            "SELECT a.id, a.nomor_tanding_id, a.nama_atlet, nt.nama_nomor_tanding, u.nama_utusan, p.nama_perguruan
             FROM backend_atlet a
             JOIN backend_nomortanding nt ON a.nomor_tanding_id = nt.id
             LEFT JOIN backend_utusan u ON a.utusan_id = u.id
             LEFT JOIN backend_perguruan p ON a.perguruan_id = p.id
             WHERE a.event_id = ? {}",
            target_clause
        );

        let ath_rows: Vec<(i64, i64, String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(&sql)
            .bind(event_id)
            .fetch_all(pool)
            .await
            .map_err(|e| e.to_string())?;

        let mut athletes_by_nt: std::collections::HashMap<i64, Vec<FestivalAthlete>> = std::collections::HashMap::new();
        for (a_id, nt_id, a_nama, a_nt_name, a_utusan, a_perguruan) in ath_rows {
            let cat_str = a_nt_name.unwrap_or_default();
            let jk = if cat_str.to_lowercase().contains("putri") || cat_str.to_lowercase().contains("pi") {
                "PUTRI".to_string()
            } else {
                "PUTRA".to_string()
            };
            let fa = FestivalAthlete {
                id: a_id,
                nama: a_nama,
                perguruan: a_perguruan.unwrap_or_else(|| "-".to_string()),
                utusan: a_utusan.unwrap_or_else(|| "-".to_string()),
                jenis_kelamin: jk,
            };
            athletes_by_nt.entry(nt_id).or_default().push(fa);
        }

        let mut state = helper_load_festival_roster_state(self.active_db_path.as_deref(), event_id);

        for (nt_id, athletes) in athletes_by_nt {
            let paired = helper_pair_festival_athletes_anti_collision(athletes);
            let paired_ids: Vec<i64> = paired.iter().map(|a| a.id).collect();
            state.athlete_pairings.insert(nt_id.to_string(), paired_ids);
        }

        let _ = helper_save_festival_roster_state(self.active_db_path.as_deref(), event_id, &state);

        self.get_festival_roster_data(event_id).await
    }

    pub async fn mix_festival_categories(
        &self,
        event_id: i64,
        source_ids: Vec<i64>,
        target_id: Option<i64>,
        new_name: String,
        delete_sources: bool,
    ) -> Result<FestivalRosterData, String> {
        let pool = self.pool.as_ref().ok_or("Database tidak terhubung.")?;

        if source_ids.is_empty() {
            return Err("Pilih minimal 1 kategori festival untuk digabungkan.".to_string());
        }

        let mut clean_new_name = new_name.trim().to_uppercase();
        if !clean_new_name.is_empty() && !clean_new_name.starts_with("FESTIVAL") {
            clean_new_name = format!("FESTIVAL {}", clean_new_name);
        }

        let mut tx = pool.begin().await.map_err(|e| e.to_string())?;

        let final_target_id: i64 = if let Some(tid) = target_id {
            if tid > 0 {
                if !clean_new_name.is_empty() {
                    sqlx::query("UPDATE backend_nomortanding SET nama_nomor_tanding = ? WHERE id = ? AND event_id = ?")
                        .bind(&clean_new_name)
                        .bind(tid)
                        .bind(event_id)
                        .execute(&mut *tx)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                tid
            } else {
                let name = if clean_new_name.is_empty() {
                    "FESTIVAL GABUNGAN".to_string()
                } else {
                    clean_new_name.clone()
                };
                let row: (i64,) = sqlx::query_as("INSERT INTO backend_nomortanding (event_id, nama_nomor_tanding, is_bob, has_vr) VALUES (?, ?, 0, 0) RETURNING id")
                    .bind(event_id)
                    .bind(&name)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
                row.0
            }
        } else {
            let name = if clean_new_name.is_empty() {
                "FESTIVAL GABUNGAN".to_string()
            } else {
                clean_new_name.clone()
            };
            let row: (i64,) = sqlx::query_as("INSERT INTO backend_nomortanding (event_id, nama_nomor_tanding, is_bob, has_vr) VALUES (?, ?, 0, 0) RETURNING id")
                .bind(event_id)
                .bind(&name)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;
            row.0
        };

        // Move all athletes from source_ids (excluding final_target_id) to final_target_id
        let source_str_list: Vec<String> = source_ids.iter().filter(|id| **id != final_target_id).map(|id| id.to_string()).collect();
        if !source_str_list.is_empty() {
            let in_clause = source_str_list.join(",");
            let update_sql = format!(
                "UPDATE backend_atlet SET nomor_tanding_id = ? WHERE event_id = ? AND nomor_tanding_id IN ({})",
                in_clause
            );
            sqlx::query(&update_sql)
                .bind(final_target_id)
                .bind(event_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| e.to_string())?;

            if delete_sources {
                let delete_sql = format!(
                    "DELETE FROM backend_nomortanding WHERE event_id = ? AND id IN ({})",
                    in_clause
                );
                sqlx::query(&delete_sql)
                    .bind(event_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| e.to_string())?;
            }
        }

        tx.commit().await.map_err(|e| e.to_string())?;

        // Update state file: remove deleted sources and auto-pair the target category
        let mut state = helper_load_festival_roster_state(self.active_db_path.as_deref(), event_id);
        if delete_sources {
            for s_id in &source_ids {
                if *s_id != final_target_id {
                    let s_key = s_id.to_string();
                    state.athlete_pairings.remove(&s_key);
                    for (_, pks) in state.tatami_assignments.iter_mut() {
                        pks.retain(|pk| *pk != *s_id);
                    }
                }
            }
        }

        // Ensure final_target_id is in tatami_assignments
        let mut already_in_tatami = false;
        for (_, pks) in &state.tatami_assignments {
            if pks.contains(&final_target_id) {
                already_in_tatami = true;
                break;
            }
        }
        if !already_in_tatami {
            let first_key = state.tatami_assignments.keys().next().cloned().unwrap_or_else(|| "1".to_string());
            state.tatami_assignments.entry(first_key).or_default().push(final_target_id);
        }

        let _ = helper_save_festival_roster_state(self.active_db_path.as_deref(), event_id, &state);

        // Auto-pair target category
        self.auto_pair_festival_category(event_id, Some(final_target_id)).await
    }

    // --- TEAM KUMITE (KUMITE BEREGU) METHODS ---

    /// Retrieves detailed Team Kumite match context, all sub-bouts, aggregate standings, and eligible athlete pools
    pub async fn get_team_match_info(&self, match_id: i64) -> Result<TeamMatchInfo, String> {
        let pool = self.get_pool()?;

        // 1. Check if match_id is a child bout (db_id in backend_matchup)
        let matchup_parent: Option<(i64, i32)> = sqlx::query_as(
            "SELECT detail_bagan_id, round FROM backend_matchup WHERE db_id = ?"
        )
        .bind(match_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Gagal cek matchup parent: {}", e))?;

        let (parent_match_id, active_bout_id, active_bout_round) = if let Some((pid, r)) = matchup_parent {
            (pid, match_id, r)
        } else {
            (match_id, 0, 1)
        };

        // 2. Fetch parent match metadata
        #[derive(sqlx::FromRow)]
        #[allow(dead_code)]
        struct ParentRow {
            bagan_id: Option<i64>,
            round: Option<i32>,
            urutan: Option<i32>,
            atlet1_id: Option<i64>,
            atlet2_id: Option<i64>,
            assigned_tatami_id: Option<i64>,
            selesai: bool,
            pemenang: Option<String>,
            nama_bagan: Option<String>,
            nama_nomor_tanding: Option<String>,
            tipe_tanding: Option<String>,
            event_id: Option<i64>,
        }

        let parent: ParentRow = sqlx::query_as(
            r#"
            SELECT db.bagan_id, db.round, db.urutan, db.atlet1_id, db.atlet2_id, db.assigned_tatami_id,
                   db.selesai, db.pemenang, b.nama_bagan, nt.nama_nomor_tanding, b.tipe_tanding, b.event_id
            FROM backend_detailbagan db
            JOIN backend_bagan b ON db.bagan_id = b.id
            LEFT JOIN backend_nomortanding nt ON b.nomor_tanding_id = nt.id
            WHERE db.id = ?
            "#
        )
        .bind(parent_match_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Partai utama #{} tidak ditemukan: {}", parent_match_id, e))?;

        let bagan_id = parent.bagan_id.unwrap_or(1);
        let event_id = parent.event_id.unwrap_or(1);
        let cat_name = parent.nama_nomor_tanding.as_deref().unwrap_or("").to_lowercase();
        let bagan_name = parent.nama_bagan.as_deref().unwrap_or("").to_lowercase();
        let tipe_tanding = parent.tipe_tanding.as_deref().unwrap_or("2");

        let existing_bouts_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM backend_matchup WHERE detail_bagan_id = ?"
        )
        .bind(parent_match_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        let is_team_cat = (cat_name.contains("beregu") || bagan_name.contains("beregu")) && tipe_tanding != "1";

        if !is_team_cat && existing_bouts_count == 0 {
            // Not a team match
            return Ok(TeamMatchInfo {
                is_team_match: false,
                parent_match_id,
                active_bout_id: match_id,
                active_bout_round: 1,
                team_aka_name: "TEAM AKA".to_string(),
                team_ao_name: "TEAM AO".to_string(),
                team_aka_utusan: String::new(),
                team_ao_utusan: String::new(),
                team_aka_logo: None,
                team_ao_logo: None,
                team_aka_score: 0,
                team_ao_score: 0,
                team_draw_count: 0,
                team_aka_points: 0,
                team_ao_points: 0,
                bouts: Vec::new(),
                candidate_atlets_aka: Vec::new(),
                candidate_atlets_ao: Vec::new(),
                is_finished: parent.selesai,
                winner_team: parent.pemenang,
            });
        }

        // 3. Initialize child bouts if not already generated
        if existing_bouts_count == 0 {
            let default_count = if cat_name.contains("putri")
                || cat_name.contains("cadet")
                || cat_name.contains("kadet")
                || cat_name.contains("junior")
                || cat_name.contains("pemula")
                || cat_name.contains("usia dini")
                || cat_name.contains("3")
                || bagan_name.contains("putri")
                || bagan_name.contains("3")
            {
                3
            } else {
                5
            };

            for r in 1..=default_count {
                let child_id: i64 = sqlx::query_scalar(
                    r#"
                    INSERT INTO backend_detailbagan (
                        bagan_id, round, urutan, atlet1_id, atlet2_id,
                        score1, score2, vr1, vr2, team, selesai, assigned_tatami_id
                    )
                    VALUES (?, 10, ?, ?, ?, '0', '0', 0, 0, 1, 0, ?)
                    RETURNING id
                    "#
                )
                .bind(bagan_id)
                .bind(r)
                .bind(parent.atlet1_id)
                .bind(parent.atlet2_id)
                .bind(parent.assigned_tatami_id)
                .fetch_one(pool)
                .await
                .map_err(|e| format!("Gagal membuat partai anak #{}: {}", r, e))?;

                sqlx::query(
                    "INSERT INTO backend_matchup (bagan_id, detail_bagan_id, db_id, round) VALUES (?, ?, ?, ?)"
                )
                .bind(bagan_id)
                .bind(parent_match_id)
                .bind(child_id)
                .bind(r)
                .execute(pool)
                .await
                .map_err(|e| format!("Gagal membuat matchup relasi #{}: {}", r, e))?;
            }
        }

        // 4. Query all child bouts
        #[derive(sqlx::FromRow)]
        struct BoutRow {
            matchup_id: i64,
            round: i32,
            db_id: i64,
            atlet1_id: Option<i64>,
            atlet1_nama: Option<String>,
            atlet1_utusan: Option<String>,
            atlet2_id: Option<i64>,
            atlet2_nama: Option<String>,
            atlet2_utusan: Option<String>,
            score1: Option<String>,
            score2: Option<String>,
            scorekecil1: Option<String>,
            scorekecil2: Option<String>,
            pemenang: Option<String>,
            selesai: bool,
        }

        let bout_rows: Vec<BoutRow> = sqlx::query_as(
            r#"
            SELECT m.id AS matchup_id, m.round, m.db_id,
                   db.atlet1_id, a1.nama_atlet AS atlet1_nama, ut1.nama_utusan AS atlet1_utusan,
                   db.atlet2_id, a2.nama_atlet AS atlet2_nama, ut2.nama_utusan AS atlet2_utusan,
                   db.score1, db.score2, db.scorekecil1, db.scorekecil2,
                   db.pemenang, db.selesai
            FROM backend_matchup m
            JOIN backend_detailbagan db ON m.db_id = db.id
            LEFT JOIN backend_atlet a1 ON db.atlet1_id = a1.id
            LEFT JOIN backend_utusan ut1 ON a1.utusan_id = ut1.id
            LEFT JOIN backend_atlet a2 ON db.atlet2_id = a2.id
            LEFT JOIN backend_utusan ut2 ON a2.utusan_id = ut2.id
            WHERE m.detail_bagan_id = ?
            ORDER BY m.round ASC
            "#
        )
        .bind(parent_match_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat list partai anak: {}", e))?;

        let mut team_aka_score = 0;
        let mut team_ao_score = 0;
        let mut team_draw_count = 0;
        let mut team_aka_points = 0;
        let mut team_ao_points = 0;

        let mut bouts: Vec<TeamBoutItem> = Vec::new();
        let mut first_unfinished_id = None;
        let mut first_unfinished_round = 1;

        for b in bout_rows {
            if let Some(ref pem) = b.pemenang {
                if pem == "1" {
                    team_aka_score += 1;
                } else if pem == "2" {
                    team_ao_score += 1;
                } else if pem == "3" {
                    team_draw_count += 1;
                }
            }
            if let Some(ref s1) = b.score1 {
                if let Ok(v) = s1.trim().parse::<i32>() {
                    team_aka_points += v;
                }
            }
            if let Some(ref s2) = b.score2 {
                if let Ok(v) = s2.trim().parse::<i32>() {
                    team_ao_points += v;
                }
            }

            if !b.selesai && first_unfinished_id.is_none() {
                first_unfinished_id = Some(b.db_id);
                first_unfinished_round = b.round;
            }

            bouts.push(TeamBoutItem {
                matchup_id: b.matchup_id,
                round: b.round,
                bout_id: b.db_id,
                atlet1_id: b.atlet1_id,
                atlet1_nama: b.atlet1_nama,
                atlet1_kontingen: b.atlet1_utusan,
                atlet2_id: b.atlet2_id,
                atlet2_nama: b.atlet2_nama,
                atlet2_kontingen: b.atlet2_utusan,
                score1: b.score1,
                score2: b.score2,
                scorekecil1: b.scorekecil1,
                scorekecil2: b.scorekecil2,
                pemenang: b.pemenang,
                selesai: b.selesai,
            });
        }

        let resolved_active_id = if active_bout_id > 0 {
            active_bout_id
        } else {
            first_unfinished_id.unwrap_or_else(|| bouts.first().map(|x| x.bout_id).unwrap_or(parent_match_id))
        };

        let resolved_active_round = if active_bout_id > 0 {
            active_bout_round
        } else {
            first_unfinished_round
        };

        // 5. Query parent team names, contingents, and logos
        #[derive(sqlx::FromRow)]
        struct ParentTeamMeta {
            a1_nama: Option<String>,
            a1_utusan: Option<String>,
            a1_logo: Option<String>,
            a1_utusan_id: Option<i64>,
            a2_nama: Option<String>,
            a2_utusan: Option<String>,
            a2_logo: Option<String>,
            a2_utusan_id: Option<i64>,
        }

        let team_meta: ParentTeamMeta = sqlx::query_as(
            r#"
            SELECT a1.nama_atlet AS a1_nama, ut1.nama_utusan AS a1_utusan, ut1.logo AS a1_logo, a1.utusan_id AS a1_utusan_id,
                   a2.nama_atlet AS a2_nama, ut2.nama_utusan AS a2_utusan, ut2.logo AS a2_logo, a2.utusan_id AS a2_utusan_id
            FROM backend_detailbagan db
            LEFT JOIN backend_atlet a1 ON db.atlet1_id = a1.id
            LEFT JOIN backend_utusan ut1 ON a1.utusan_id = ut1.id
            LEFT JOIN backend_atlet a2 ON db.atlet2_id = a2.id
            LEFT JOIN backend_utusan ut2 ON a2.utusan_id = ut2.id
            WHERE db.id = ?
            "#
        )
        .bind(parent_match_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Gagal memuat metadata tim: {}", e))?;

        let team_aka_name = team_meta.a1_nama.unwrap_or_else(|| "TEAM AKA".to_string());
        let team_aka_utusan = team_meta.a1_utusan.unwrap_or_default();
        let team_aka_logo = team_meta.a1_logo;

        let team_ao_name = team_meta.a2_nama.unwrap_or_else(|| "TEAM AO".to_string());
        let team_ao_utusan = team_meta.a2_utusan.unwrap_or_default();
        let team_ao_logo = team_meta.a2_logo;

        // 6. Query candidate athletes for AKA and AO by utusan_id
        #[derive(sqlx::FromRow)]
        struct CandRow {
            id: i64,
            nama_atlet: String,
            perguruan_nama: Option<String>,
            utusan_nama: Option<String>,
        }

        let mut candidate_atlets_aka: Vec<TeamCandidateAthlete> = Vec::new();
        if let Some(ut_id) = team_meta.a1_utusan_id {
            let p_id = parent.atlet1_id.unwrap_or(0);
            let rows: Vec<CandRow> = sqlx::query_as(
                r#"
                SELECT a.id, a.nama_atlet, p.nama_perguruan, u.nama_utusan
                FROM backend_atlet a
                LEFT JOIN backend_perguruan p ON a.perguruan_id = p.id
                LEFT JOIN backend_utusan u ON a.utusan_id = u.id
                WHERE a.event_id = ? AND a.utusan_id = ? AND a.id != ?
                ORDER BY a.nama_atlet ASC
                "#
            )
            .bind(event_id)
            .bind(ut_id)
            .bind(p_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            for r in rows {
                candidate_atlets_aka.push(TeamCandidateAthlete {
                    id: r.id,
                    nama_atlet: r.nama_atlet,
                    perguruan_nama: r.perguruan_nama,
                    utusan_nama: r.utusan_nama,
                });
            }
        }

        let mut candidate_atlets_ao: Vec<TeamCandidateAthlete> = Vec::new();
        if let Some(ut_id) = team_meta.a2_utusan_id {
            let p_id = parent.atlet2_id.unwrap_or(0);
            let rows: Vec<CandRow> = sqlx::query_as(
                r#"
                SELECT a.id, a.nama_atlet, p.nama_perguruan, u.nama_utusan
                FROM backend_atlet a
                LEFT JOIN backend_perguruan p ON a.perguruan_id = p.id
                LEFT JOIN backend_utusan u ON a.utusan_id = u.id
                WHERE a.event_id = ? AND a.utusan_id = ? AND a.id != ?
                ORDER BY a.nama_atlet ASC
                "#
            )
            .bind(event_id)
            .bind(ut_id)
            .bind(p_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            for r in rows {
                candidate_atlets_ao.push(TeamCandidateAthlete {
                    id: r.id,
                    nama_atlet: r.nama_atlet,
                    perguruan_nama: r.perguruan_nama,
                    utusan_nama: r.utusan_nama,
                });
            }
        }

        Ok(TeamMatchInfo {
            is_team_match: true,
            parent_match_id,
            active_bout_id: resolved_active_id,
            active_bout_round: resolved_active_round,
            team_aka_name,
            team_ao_name,
            team_aka_utusan,
            team_ao_utusan,
            team_aka_logo,
            team_ao_logo,
            team_aka_score,
            team_ao_score,
            team_draw_count,
            team_aka_points,
            team_ao_points,
            bouts,
            candidate_atlets_aka,
            candidate_atlets_ao,
            is_finished: parent.selesai,
            winner_team: parent.pemenang,
        })
    }

    /// Assigns specific fighters to a child bout (creating new athlete records if custom names are entered)
    pub async fn assign_bout_athletes(&self, input: AssignBoutAthletesInput) -> Result<(), String> {
        let pool = self.get_pool()?;

        // Fetch parent match context to inherit utusan & perguruan for custom athletes
        #[derive(sqlx::FromRow)]
        #[allow(dead_code)]
        struct BoutContext {
            bagan_id: Option<i64>,
            parent_a1_id: Option<i64>,
            parent_a2_id: Option<i64>,
            p1_utusan_id: Option<i64>,
            p1_perguruan_id: Option<i64>,
            p1_event_id: Option<i64>,
            p1_nt_id: Option<i64>,
            p2_utusan_id: Option<i64>,
            p2_perguruan_id: Option<i64>,
        }

        let ctx: BoutContext = sqlx::query_as(
            r#"
            SELECT db.bagan_id,
                   p.atlet1_id AS parent_a1_id, p.atlet2_id AS parent_a2_id,
                   a1.utusan_id AS p1_utusan_id, a1.perguruan_id AS p1_perguruan_id, a1.event_id AS p1_event_id, a1.nomor_tanding_id AS p1_nt_id,
                   a2.utusan_id AS p2_utusan_id, a2.perguruan_id AS p2_perguruan_id
            FROM backend_detailbagan db
            JOIN backend_matchup m ON m.db_id = db.id
            JOIN backend_detailbagan p ON m.detail_bagan_id = p.id
            LEFT JOIN backend_atlet a1 ON p.atlet1_id = a1.id
            LEFT JOIN backend_atlet a2 ON p.atlet2_id = a2.id
            WHERE db.id = ?
            "#
        )
        .bind(input.bout_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Konteks partai anak #{} tidak ditemukan: {}", input.bout_id, e))?;

        let mut target_aka_id = input.aka_atlet_id;
        if target_aka_id.is_none() {
            if let Some(ref custom_aka) = input.aka_custom_name {
                let clean_name = custom_aka.trim().to_uppercase();
                if !clean_name.is_empty() {
                    let ev_id = ctx.p1_event_id.unwrap_or(1);
                    // Check if athlete with same name and utusan already exists
                    let existing: Option<(i64,)> = sqlx::query_as(
                        "SELECT id FROM backend_atlet WHERE event_id = ? AND utusan_id = ? AND UPPER(nama_atlet) = ?"
                    )
                    .bind(ev_id)
                    .bind(ctx.p1_utusan_id)
                    .bind(&clean_name)
                    .fetch_optional(pool)
                    .await
                    .unwrap_or(None);

                    if let Some((aid,)) = existing {
                        target_aka_id = Some(aid);
                    } else {
                        let new_id: i64 = sqlx::query_scalar(
                            "INSERT INTO backend_atlet (event_id, nama_atlet, utusan_id, perguruan_id, nomor_tanding_id, is_priority) VALUES (?, ?, ?, ?, ?, 0) RETURNING id"
                        )
                        .bind(ev_id)
                        .bind(&clean_name)
                        .bind(ctx.p1_utusan_id)
                        .bind(ctx.p1_perguruan_id)
                        .bind(ctx.p1_nt_id)
                        .fetch_one(pool)
                        .await
                        .map_err(|e| format!("Gagal membuat atlet AKA kustom: {}", e))?;
                        target_aka_id = Some(new_id);
                    }
                }
            }
        }

        let mut target_ao_id = input.ao_atlet_id;
        if target_ao_id.is_none() {
            if let Some(ref custom_ao) = input.ao_custom_name {
                let clean_name = custom_ao.trim().to_uppercase();
                if !clean_name.is_empty() {
                    let ev_id = ctx.p1_event_id.unwrap_or(1);
                    let existing: Option<(i64,)> = sqlx::query_as(
                        "SELECT id FROM backend_atlet WHERE event_id = ? AND utusan_id = ? AND UPPER(nama_atlet) = ?"
                    )
                    .bind(ev_id)
                    .bind(ctx.p2_utusan_id)
                    .bind(&clean_name)
                    .fetch_optional(pool)
                    .await
                    .unwrap_or(None);

                    if let Some((aid,)) = existing {
                        target_ao_id = Some(aid);
                    } else {
                        let new_id: i64 = sqlx::query_scalar(
                            "INSERT INTO backend_atlet (event_id, nama_atlet, utusan_id, perguruan_id, nomor_tanding_id, is_priority) VALUES (?, ?, ?, ?, ?, 0) RETURNING id"
                        )
                        .bind(ev_id)
                        .bind(&clean_name)
                        .bind(ctx.p2_utusan_id)
                        .bind(ctx.p2_perguruan_id)
                        .bind(ctx.p1_nt_id)
                        .fetch_one(pool)
                        .await
                        .map_err(|e| format!("Gagal membuat atlet AO kustom: {}", e))?;
                        target_ao_id = Some(new_id);
                    }
                }
            }
        }

        sqlx::query("UPDATE backend_detailbagan SET atlet1_id = ?, atlet2_id = ? WHERE id = ?")
            .bind(target_aka_id)
            .bind(target_ao_id)
            .bind(input.bout_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal menetapkan atlet partai: {}", e))?;

        Ok(())
    }

    /// Adds an extra child bout to a team match (e.g. sudden-death tiebreaker bout)
    pub async fn add_team_bout(&self, parent_match_id: i64) -> Result<TeamBoutItem, String> {
        let pool = self.get_pool()?;

        #[derive(sqlx::FromRow)]
        struct ParentBoutMeta {
            bagan_id: Option<i64>,
            atlet1_id: Option<i64>,
            atlet2_id: Option<i64>,
            assigned_tatami_id: Option<i64>,
            vr1: bool,
            vr2: bool,
        }

        let p: ParentBoutMeta = sqlx::query_as(
            "SELECT bagan_id, atlet1_id, atlet2_id, assigned_tatami_id, vr1, vr2 FROM backend_detailbagan WHERE id = ?"
        )
        .bind(parent_match_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Partai utama #{} tidak ditemukan: {}", parent_match_id, e))?;

        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM backend_matchup WHERE detail_bagan_id = ?"
        )
        .bind(parent_match_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        let next_round = (count + 1) as i32;

        let child_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO backend_detailbagan (
                bagan_id, round, urutan, atlet1_id, atlet2_id,
                score1, score2, vr1, vr2, team, selesai, assigned_tatami_id
            )
            VALUES (?, 10, ?, ?, ?, '0', '0', ?, ?, 1, 0, ?)
            RETURNING id
            "#
        )
        .bind(p.bagan_id)
        .bind(next_round)
        .bind(p.atlet1_id)
        .bind(p.atlet2_id)
        .bind(p.vr1)
        .bind(p.vr2)
        .bind(p.assigned_tatami_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Gagal menambah partai anak: {}", e))?;

        let matchup_id: i64 = sqlx::query_scalar(
            "INSERT INTO backend_matchup (bagan_id, detail_bagan_id, db_id, round) VALUES (?, ?, ?, ?) RETURNING id"
        )
        .bind(p.bagan_id)
        .bind(parent_match_id)
        .bind(child_id)
        .bind(next_round)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Gagal membuat matchup relasi: {}", e))?;

        Ok(TeamBoutItem {
            matchup_id,
            round: next_round,
            bout_id: child_id,
            atlet1_id: p.atlet1_id,
            atlet1_nama: None,
            atlet1_kontingen: None,
            atlet2_id: p.atlet2_id,
            atlet2_nama: None,
            atlet2_kontingen: None,
            score1: Some("0".to_string()),
            score2: Some("0".to_string()),
            scorekecil1: None,
            scorekecil2: None,
            pemenang: None,
            selesai: false,
        })
    }

    /// Deletes a team bout
    pub async fn delete_team_bout(&self, bout_id: i64) -> Result<(), String> {
        let pool = self.get_pool()?;

        sqlx::query("DELETE FROM backend_matchup WHERE db_id = ?")
            .bind(bout_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal menghapus matchup: {}", e))?;

        sqlx::query("DELETE FROM backend_detailbagan WHERE id = ? AND team = 1")
            .bind(bout_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal menghapus partai anak: {}", e))?;

        Ok(())
    }

    /// Finalizes a team match: computes final aggregate scores, advances winning team in bracket tree, enqueues sync
    pub async fn finish_team_match(&self, parent_match_id: i64, winner_override: Option<String>) -> Result<(), String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        // 1. Fetch all child bouts to calculate aggregate tallies
        #[derive(sqlx::FromRow)]
        struct ChildBoutTally {
            score1: Option<String>,
            score2: Option<String>,
            pemenang: Option<String>,
        }

        let bouts: Vec<ChildBoutTally> = sqlx::query_as(
            "SELECT db.score1, db.score2, db.pemenang FROM backend_matchup m JOIN backend_detailbagan db ON m.db_id = db.id WHERE m.detail_bagan_id = ? ORDER BY m.round ASC"
        )
        .bind(parent_match_id)
        .fetch_all(&mut *tx)
        .await
        .map_err(|e| format!("Gagal memuat partai beregu: {}", e))?;

        let mut t_aka_score = 0;
        let mut t_ao_score = 0;
        let mut t_aka_points = 0;
        let mut t_ao_points = 0;

        for b in &bouts {
            if let Some(ref pem) = b.pemenang {
                if pem == "1" {
                    t_aka_score += 1;
                } else if pem == "2" {
                    t_ao_score += 1;
                }
            }
            if let Some(ref s1) = b.score1 {
                if let Ok(v) = s1.trim().parse::<i32>() {
                    t_aka_points += v;
                }
            }
            if let Some(ref s2) = b.score2 {
                if let Ok(v) = s2.trim().parse::<i32>() {
                    t_ao_points += v;
                }
            }
        }

        // 2. Determine final winner
        let final_pemenang = if let Some(w) = winner_override {
            w
        } else if t_aka_score > t_ao_score {
            "1".to_string()
        } else if t_ao_score > t_aka_score {
            "2".to_string()
        } else if t_aka_points > t_ao_points {
            "1".to_string()
        } else if t_ao_points > t_aka_points {
            "2".to_string()
        } else {
            "1".to_string()
        };

        // 3. Update parent match
        sqlx::query(
            "UPDATE backend_detailbagan SET score1 = ?, score2 = ?, scorekecil1 = ?, scorekecil2 = ?, pemenang = ?, selesai = 1 WHERE id = ?"
        )
        .bind(t_aka_score.to_string())
        .bind(t_ao_score.to_string())
        .bind(t_aka_points.to_string())
        .bind(t_ao_points.to_string())
        .bind(&final_pemenang)
        .bind(parent_match_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Gagal update partai utama: {}", e))?;

        // 4. Advance winning team in bracket tree
        #[derive(sqlx::FromRow)]
        struct ParentTreeMeta {
            bagan_id: i64,
            round: i64,
            urutan: i64,
            atlet1_id: Option<i64>,
            atlet2_id: Option<i64>,
            assigned_tatami_id: Option<i64>,
            event_id: Option<i64>,
        }

        let p_meta: ParentTreeMeta = sqlx::query_as(
            "SELECT db.bagan_id, db.round, db.urutan, db.atlet1_id, db.atlet2_id, db.assigned_tatami_id, b.event_id FROM backend_detailbagan db JOIN backend_bagan b ON db.bagan_id = b.id WHERE db.id = ?"
        )
        .bind(parent_match_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| format!("Gagal memuat pohon bagan: {}", e))?;

        let winner_athlete_id = if final_pemenang == "1" {
            p_meta.atlet1_id
        } else {
            p_meta.atlet2_id
        };

        if let Some(w_id) = winner_athlete_id {
            let next_round = p_meta.round + 1;
            let next_urutan = (p_meta.urutan + 1) / 2;

            if p_meta.urutan % 2 == 1 {
                let _ = sqlx::query(
                    "UPDATE backend_detailbagan SET atlet1_id = ? WHERE bagan_id = ? AND round = ? AND urutan = ?"
                )
                .bind(w_id)
                .bind(p_meta.bagan_id)
                .bind(next_round)
                .bind(next_urutan)
                .execute(&mut *tx)
                .await;
            } else {
                let _ = sqlx::query(
                    "UPDATE backend_detailbagan SET atlet2_id = ? WHERE bagan_id = ? AND round = ? AND urutan = ?"
                )
                .bind(w_id)
                .bind(p_meta.bagan_id)
                .bind(next_round)
                .bind(next_urutan)
                .execute(&mut *tx)
                .await;
            }
        }

        // 5. Clear tatami assignment or advance
        if let Some(t_id) = p_meta.assigned_tatami_id {
            let _ = sqlx::query("UPDATE backend_tatami SET detail_bagan_id = NULL WHERE id = ? AND detail_bagan_id = ?")
                .bind(t_id)
                .bind(parent_match_id)
                .execute(&mut *tx)
                .await;
        }

        // 6. Enqueue cloud sync if enabled
        if let Some(ev_id) = p_meta.event_id {
            let ev_info: Option<(Option<String>, bool)> = sqlx::query_as(
                "SELECT kode_event, is_live_sync_enabled FROM backend_event WHERE id = ?"
            )
            .bind(ev_id)
            .fetch_optional(&mut *tx)
            .await
            .unwrap_or(None);

            if let Some((kode_event, is_live_sync_enabled)) = ev_info {
                if is_live_sync_enabled {
                    let ev_code = kode_event.unwrap_or_else(|| format!("EV{}", ev_id));
                    let kode_realtime = format!("{}-{}-{}", ev_code, p_meta.bagan_id, parent_match_id);

                    let payload = serde_json::json!({
                        "status": "finished",
                        "pemenang": if final_pemenang == "1" { "aka" } else { "ao" },
                        "round": p_meta.round,
                        "urutan": p_meta.urutan,
                        "kode_realtime": kode_realtime,
                        "score_aka": t_aka_score.to_string(),
                        "score_ao": t_ao_score.to_string(),
                        "lil_score_aka": t_aka_points.to_string(),
                        "lil_score_ao": t_ao_points.to_string(),
                    });

                    let _ = sqlx::query(
                        "INSERT INTO backend_syncqueue (event_id, endpoint, payload, attempts, created_at) VALUES (?, ?, ?, 0, CURRENT_TIMESTAMP)"
                    )
                    .bind(ev_id)
                    .bind("api/result/")
                    .bind(payload.to_string())
                    .execute(&mut *tx)
                    .await;
                }
            }
        }

        tx.commit().await.map_err(|e| format!("Gagal menyelesaikan partai beregu: {}", e))?;
        Ok(())
    }

    pub async fn get_wasits(&self, event_id: i64) -> Result<Vec<EnrichedWasit>, String> {
        let pool = self.get_pool()?;
        let rows = sqlx::query_as::<_, EnrichedWasit>(
            r#"
            SELECT 
                w.id,
                w.event_id,
                w.nama_wasit,
                w.perguruan_id,
                p.nama_perguruan,
                w.kab_kota,
                w.lisensi,
                w.no_lisensi,
                w.is_active,
                wt.tatami_id as assigned_tatami_id,
                t.tatami_number as assigned_tatami_number,
                wt.posisi as assigned_posisi
            FROM backend_wasit w
            LEFT JOIN backend_perguruan p ON w.perguruan_id = p.id
            LEFT JOIN backend_wasittatami wt ON wt.wasit_id = w.id
            LEFT JOIN backend_tatami t ON wt.tatami_id = t.id
            WHERE w.event_id = ?
            ORDER BY w.nama_wasit ASC
            "#
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat data wasit: {}", e))?;

        Ok(rows)
    }

    pub async fn save_wasit(&self, input: SaveWasitInput) -> Result<i64, String> {
        let pool = self.get_pool()?;
        if let Some(id) = input.id {
            if id > 0 {
                sqlx::query(
                    r#"
                    UPDATE backend_wasit
                    SET nama_wasit = ?, perguruan_id = ?, kab_kota = ?, lisensi = ?, no_lisensi = ?, is_active = ?
                    WHERE id = ? AND event_id = ?
                    "#
                )
                .bind(&input.nama_wasit)
                .bind(input.perguruan_id)
                .bind(&input.kab_kota)
                .bind(&input.lisensi)
                .bind(&input.no_lisensi)
                .bind(input.is_active)
                .bind(id)
                .bind(input.event_id)
                .execute(pool)
                .await
                .map_err(|e| format!("Gagal mengupdate wasit: {}", e))?;

                return Ok(id);
            }
        }

        let res = sqlx::query(
            r#"
            INSERT INTO backend_wasit (nama_wasit, perguruan_id, kab_kota, lisensi, no_lisensi, is_active, event_id, created_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, datetime('now'))
            "#
        )
        .bind(&input.nama_wasit)
        .bind(input.perguruan_id)
        .bind(&input.kab_kota)
        .bind(&input.lisensi)
        .bind(&input.no_lisensi)
        .bind(input.is_active)
        .bind(input.event_id)
        .execute(pool)
        .await
        .map_err(|e| format!("Gagal menambahkan wasit baru: {}", e))?;

        Ok(res.last_insert_rowid())
    }

    pub async fn delete_wasit(&self, wasit_id: i64) -> Result<(), String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal membuka transaksi: {}", e))?;
        
        sqlx::query("DELETE FROM backend_wasittatami WHERE wasit_id = ?")
            .bind(wasit_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghapus relasi wasit tatami: {}", e))?;

        sqlx::query("DELETE FROM backend_wasitdetailbagan WHERE wasit_id = ?")
            .bind(wasit_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghapus relasi wasit bagan: {}", e))?;

        sqlx::query("DELETE FROM backend_wasit WHERE id = ?")
            .bind(wasit_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghapus wasit: {}", e))?;

        tx.commit().await.map_err(|e| format!("Gagal commit transaksi hapus wasit: {}", e))?;
        Ok(())
    }

    pub async fn get_enriched_atlet_by_id(&self, atlet_id: i64) -> Result<Option<EnrichedAtlet>, String> {
        let pool = self.get_pool()?;
        sqlx::query_as::<_, EnrichedAtlet>(
            r#"
            SELECT 
                a.id, a.event_id, a.nama_atlet, a.nik, a.kode_atlet, a.additional_code, a.is_priority,
                a.perguruan_id, p.nama_perguruan,
                a.utusan_id, u.nama_utusan, u.logo as utusan_logo,
                a.nomor_tanding_id, nt.nama_kategori as nomor_tanding_nama, nt.tipe as nomor_tanding_tipe,
                NULL as is_bob
            FROM backend_atlet a
            LEFT JOIN backend_perguruan p ON a.perguruan_id = p.id
            LEFT JOIN backend_utusan u ON a.utusan_id = u.id
            LEFT JOIN backend_nomortanding nt ON a.nomor_tanding_id = nt.id
            WHERE a.id = ?
            LIMIT 1
            "#
        )
        .bind(atlet_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Gagal mengambil data atlet: {}", e))
    }

    pub async fn get_tatami_panel_overview(&self, _event_id: i64, tatami_id: i64) -> Result<TatamiPanelOverview, String> {
        let pool = self.get_pool()?;
        
        let tatami_opt = sqlx::query_as::<_, Tatami>("SELECT id, event_id, tatami_number, detail_bagan_id FROM backend_tatami WHERE id = ?")
            .bind(tatami_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Gagal mengambil data tatami: {}", e))?;

        let tatami = tatami_opt.ok_or_else(|| format!("Tatami dengan ID {} tidak ditemukan", tatami_id))?;
        let t_num = tatami.tatami_number.unwrap_or(1);

        let assignments = sqlx::query_as::<_, EnrichedWasitTatami>(
            r#"
            SELECT 
                wt.id,
                wt.event_id,
                wt.tatami_id,
                t.tatami_number,
                wt.wasit_id,
                w.nama_wasit,
                w.lisensi,
                w.no_lisensi,
                w.perguruan_id,
                p.nama_perguruan,
                w.kab_kota,
                wt.posisi,
                wt.assigned_at
            FROM backend_wasittatami wt
            JOIN backend_wasit w ON wt.wasit_id = w.id
            LEFT JOIN backend_perguruan p ON w.perguruan_id = p.id
            LEFT JOIN backend_tatami t ON wt.tatami_id = t.id
            WHERE wt.tatami_id = ?
            ORDER BY 
                CASE wt.posisi
                    WHEN 'tatami_manager' THEN 1
                    WHEN 'kansa' THEN 2
                    WHEN 'referee' THEN 3
                    WHEN 'judge_1' THEN 4
                    WHEN 'judge_2' THEN 5
                    WHEN 'judge_3' THEN 6
                    WHEN 'judge_4' THEN 7
                    WHEN 'pool' THEN 8
                    ELSE 9
                END ASC
            "#
        )
        .bind(tatami_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal mengambil penugasan tatami: {}", e))?;

        let mut active_match: Option<EnrichedDetailBagan> = None;
        let mut category_name: Option<String> = None;
        let mut atlet1: Option<EnrichedAtlet> = None;
        let mut atlet2: Option<EnrichedAtlet> = None;

        if let Some(detail_id) = tatami.detail_bagan_id {
            let m_opt = sqlx::query_as::<_, EnrichedDetailBagan>(
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
                WHERE d.id = ?
                LIMIT 1
                "#
            )
            .bind(detail_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);

            if let Some(m) = m_opt {
                if let Some(bagan_id) = m.bagan_id {
                    let cat_row: Option<(Option<String>,)> = sqlx::query_as(
                        r#"
                        SELECT nt.nama_kategori 
                        FROM backend_bagan b
                        JOIN backend_nomortanding nt ON b.nomor_tanding_id = nt.id
                        WHERE b.id = ?
                        "#
                    )
                    .bind(bagan_id)
                    .fetch_optional(pool)
                    .await
                    .unwrap_or(None);

                    category_name = cat_row.and_then(|r| r.0);
                }

                if let Some(a1_id) = m.atlet1_id {
                    atlet1 = self.get_enriched_atlet_by_id(a1_id).await.ok().flatten();
                }
                if let Some(a2_id) = m.atlet2_id {
                    atlet2 = self.get_enriched_atlet_by_id(a2_id).await.ok().flatten();
                }
                active_match = Some(m);
            }
        }

        let conflicts = evaluate_panel_conflicts(
            &assignments,
            atlet1.as_ref(),
            atlet2.as_ref(),
            active_match.as_ref().map(|m| m.id),
            active_match.as_ref().and_then(|m| m.kode.as_deref()),
        );

        Ok(TatamiPanelOverview {
            tatami_id,
            tatami_number: t_num,
            active_match,
            category_name,
            assignments,
            conflicts,
        })
    }

    pub async fn assign_wasit_tatami(&self, input: AssignWasitTatamiInput) -> Result<i64, String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        if input.posisi != "pool" {
            sqlx::query("DELETE FROM backend_wasittatami WHERE tatami_id = ? AND posisi = ?")
                .bind(input.tatami_id)
                .bind(&input.posisi)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal mengosongkan posisi sebelumnya: {}", e))?;
        }

        sqlx::query("DELETE FROM backend_wasittatami WHERE tatami_id = ? AND wasit_id = ?")
            .bind(input.tatami_id)
            .bind(input.wasit_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal mereposisi wasit: {}", e))?;

        let res = sqlx::query(
            r#"
            INSERT INTO backend_wasittatami (event_id, tatami_id, wasit_id, posisi, assigned_at)
            VALUES (?, ?, ?, ?, datetime('now'))
            "#
        )
        .bind(input.event_id)
        .bind(input.tatami_id)
        .bind(input.wasit_id)
        .bind(&input.posisi)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Gagal menugaskan wasit tatami: {}", e))?;

        tx.commit().await.map_err(|e| format!("Gagal menyimpan penugasan wasit: {}", e))?;
        Ok(res.last_insert_rowid())
    }

    pub async fn remove_wasit_tatami(&self, assignment_id: i64) -> Result<(), String> {
        let pool = self.get_pool()?;
        sqlx::query("DELETE FROM backend_wasittatami WHERE id = ?")
            .bind(assignment_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal menghapus penugasan wasit: {}", e))?;
        Ok(())
    }

    pub async fn clear_tatami_panel(&self, tatami_id: i64) -> Result<(), String> {
        let pool = self.get_pool()?;
        sqlx::query("DELETE FROM backend_wasittatami WHERE tatami_id = ?")
            .bind(tatami_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal mengosongkan panel tatami: {}", e))?;
        Ok(())
    }

    pub async fn auto_assign_tatami_panel(&self, input: AutoAssignPanelInput) -> Result<(), String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        sqlx::query("DELETE FROM backend_wasittatami WHERE tatami_id = ?")
            .bind(input.tatami_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal reset panel: {}", e))?;

        let standard_roles = [
            "tatami_manager",
            "kansa",
            "referee",
            "judge_1",
            "judge_2",
            "judge_3",
            "judge_4",
        ];

        for (i, wasit_id) in input.wasit_ids.iter().enumerate() {
            let role = if i < standard_roles.len() {
                standard_roles[i]
            } else {
                "pool"
            };

            sqlx::query(
                r#"
                INSERT INTO backend_wasittatami (event_id, tatami_id, wasit_id, posisi, assigned_at)
                VALUES (?, ?, ?, ?, datetime('now'))
                "#
            )
            .bind(input.event_id)
            .bind(input.tatami_id)
            .bind(*wasit_id)
            .bind(role)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal insert wasit otomatis: {}", e))?;
        }

        tx.commit().await.map_err(|e| format!("Gagal menyimpan auto-assign panel: {}", e))?;
        Ok(())
    }

    pub async fn get_tournament_excel_data(&self, event_id: i64) -> Result<crate::excel_exporter::TournamentExcelData, String> {
        let pool = self.get_pool()?;

        let event_name_row: Option<(String,)> = sqlx::query_as("SELECT nama_event FROM backend_event WHERE id = ?")
            .bind(event_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
        let event_name = event_name_row.map(|r| r.0).unwrap_or_else(|| "Tournament".to_string());

        let summary_juara = self.get_summary_juara(event_id).await?;
        let summary_bagan = self.get_summary_bagan(event_id).await?;
        let athletes = self.get_enriched_athletes(event_id).await.unwrap_or_default();

        let matches = sqlx::query_as::<_, EnrichedDetailBagan>(
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
            JOIN backend_bagan b ON d.bagan_id = b.id
            LEFT JOIN backend_atlet a1 ON d.atlet1_id = a1.id
            LEFT JOIN backend_utusan u1 ON a1.utusan_id = u1.id
            LEFT JOIN backend_perguruan p1 ON a1.perguruan_id = p1.id
            LEFT JOIN backend_atlet a2 ON d.atlet2_id = a2.id
            LEFT JOIN backend_utusan u2 ON a2.utusan_id = u2.id
            LEFT JOIN backend_perguruan p2 ON a2.perguruan_id = p2.id
            WHERE b.event_id = ?
            ORDER BY d.assigned_tatami_id ASC, d.round ASC, d.urutan ASC
            "#
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let wasits = self.get_wasits(event_id).await.unwrap_or_default();

        Ok(crate::excel_exporter::TournamentExcelData {
            event_name,
            summary_juara,
            summary_bagan,
            athletes,
            matches,
            wasits,
        })
    }

    pub async fn get_marshalling_overview(&self, event_id: i64) -> Result<MarshallingOverview, String> {
        let pool = self.get_pool()?;

        let event_name_row: Option<(String,)> = sqlx::query_as("SELECT nama_event FROM backend_event WHERE id = ?")
            .bind(event_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None);
        let event_name = event_name_row.map(|r| r.0).unwrap_or_else(|| "Tournament".to_string());

        let tatamis = sqlx::query_as::<_, Tatami>("SELECT id, event_id, tatami_number, detail_bagan_id FROM backend_tatami WHERE event_id = ? ORDER BY tatami_number ASC")
            .bind(event_id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

        let mut tatami_queues = Vec::new();

        for t in tatamis {
            let t_num = t.tatami_number.unwrap_or(1);

            let mut current_match: Option<MarshallingMatchItem> = None;
            if let Some(detail_id) = t.detail_bagan_id {
                let m_opt = sqlx::query_as::<_, (i64, Option<i64>, Option<String>, Option<i32>, Option<i32>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)>(
                    r#"
                    SELECT 
                        d.id, d.bagan_id, nt.nama_kategori, d.round, d.urutan, d.kode,
                        a1.nama_atlet, u1.nama_utusan, p1.nama_perguruan,
                        a2.nama_atlet, u2.nama_utusan, p2.nama_perguruan
                    FROM backend_detailbagan d
                    JOIN backend_bagan b ON d.bagan_id = b.id
                    LEFT JOIN backend_nomortanding nt ON b.nomor_tanding_id = nt.id
                    LEFT JOIN backend_atlet a1 ON d.atlet1_id = a1.id
                    LEFT JOIN backend_utusan u1 ON a1.utusan_id = u1.id
                    LEFT JOIN backend_perguruan p1 ON a1.perguruan_id = p1.id
                    LEFT JOIN backend_atlet a2 ON d.atlet2_id = a2.id
                    LEFT JOIN backend_utusan u2 ON a2.utusan_id = u2.id
                    LEFT JOIN backend_perguruan p2 ON a2.perguruan_id = p2.id
                    WHERE d.id = ?
                    LIMIT 1
                    "#
                )
                .bind(detail_id)
                .fetch_optional(pool)
                .await
                .unwrap_or(None);

                if let Some(m) = m_opt {
                    current_match = Some(MarshallingMatchItem {
                        match_id: m.0,
                        bagan_id: m.1,
                        category_name: m.2.unwrap_or_else(|| "Kategori".to_string()),
                        round: m.3.unwrap_or(1),
                        urutan: m.4.unwrap_or(1),
                        kode: m.5,
                        atlet1_nama: m.6.unwrap_or_else(|| "BYE".to_string()),
                        atlet1_kontingen: m.7.unwrap_or_else(|| "-".to_string()),
                        atlet1_perguruan: m.8,
                        atlet2_nama: m.9.unwrap_or_else(|| "BYE".to_string()),
                        atlet2_kontingen: m.10.unwrap_or_else(|| "-".to_string()),
                        atlet2_perguruan: m.11,
                        status: "LIVE".to_string(),
                    });
                }
            }

            let upcoming_rows = sqlx::query_as::<_, (i64, Option<i64>, Option<String>, Option<i32>, Option<i32>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>)>(
                r#"
                SELECT 
                    d.id, d.bagan_id, nt.nama_kategori, d.round, d.urutan, d.kode,
                    a1.nama_atlet, u1.nama_utusan, p1.nama_perguruan,
                    a2.nama_atlet, u2.nama_utusan, p2.nama_perguruan
                FROM backend_detailbagan d
                JOIN backend_bagan b ON d.bagan_id = b.id
                LEFT JOIN backend_nomortanding nt ON b.nomor_tanding_id = nt.id
                LEFT JOIN backend_atlet a1 ON d.atlet1_id = a1.id
                LEFT JOIN backend_utusan u1 ON a1.utusan_id = u1.id
                LEFT JOIN backend_perguruan p1 ON a1.perguruan_id = p1.id
                LEFT JOIN backend_atlet a2 ON d.atlet2_id = a2.id
                LEFT JOIN backend_utusan u2 ON a2.utusan_id = u2.id
                LEFT JOIN backend_perguruan p2 ON a2.perguruan_id = p2.id
                WHERE d.assigned_tatami_id = ? AND d.selesai = 0
                ORDER BY d.round ASC, d.urutan ASC
                LIMIT 6
                "#
            )
            .bind(t.id)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            let mut filtered_upcoming = Vec::new();
            for m in upcoming_rows {
                if let Some(ref cur) = current_match {
                    if cur.match_id == m.0 {
                        continue;
                    }
                }
                filtered_upcoming.push(m);
            }

            let mut next_match: Option<MarshallingMatchItem> = None;
            let mut other_upcoming: Vec<MarshallingMatchItem> = Vec::new();

            for (idx, m) in filtered_upcoming.into_iter().enumerate() {
                let status = match idx {
                    0 => "CALL_1",
                    1 => "CALL_2",
                    _ => "PREPARE",
                };

                let item = MarshallingMatchItem {
                    match_id: m.0,
                    bagan_id: m.1,
                    category_name: m.2.unwrap_or_else(|| "Kategori".to_string()),
                    round: m.3.unwrap_or(1),
                    urutan: m.4.unwrap_or(1),
                    kode: m.5,
                    atlet1_nama: m.6.unwrap_or_else(|| "BYE".to_string()),
                    atlet1_kontingen: m.7.unwrap_or_else(|| "-".to_string()),
                    atlet1_perguruan: m.8,
                    atlet2_nama: m.9.unwrap_or_else(|| "BYE".to_string()),
                    atlet2_kontingen: m.10.unwrap_or_else(|| "-".to_string()),
                    atlet2_perguruan: m.11,
                    status: status.to_string(),
                };

                if idx == 0 {
                    next_match = Some(item);
                } else {
                    other_upcoming.push(item);
                }
            }

            tatami_queues.push(TatamiQueue {
                tatami_id: t.id,
                tatami_number: t_num,
                current_match,
                next_match,
                upcoming_matches: other_upcoming,
            });
        }

        Ok(MarshallingOverview {
            event_name,
            tatamis: tatami_queues,
            active_announcement: None,
        })
    }

    /// Generates complete Official Results Booklet and Minutes of Meeting (Berita Acara)
    pub async fn get_results_booklet_data(&self, event_id: i64) -> Result<ResultsBookletData, String> {
        let pool = self.get_pool()?;

        // 1. Event info
        let ev_row: Option<(String, Option<String>)> = sqlx::query_as(
            "SELECT nama_event, kode_event FROM backend_event WHERE id = ?"
        )
        .bind(event_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        let nama_event = ev_row.as_ref().map(|r| r.0.clone()).unwrap_or_else(|| "KEJUARAAN KARATE AMBRILINDO".to_string());

        // 2. Kop Surat & Venue
        let kop: Option<KopSurat> = sqlx::query_as(
            "SELECT id, event_id, logo, nama_organisasi, alamat, kontak FROM backend_kopsurat WHERE event_id = ?"
        )
        .bind(event_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

        let nama_organisasi = kop.as_ref().map(|k| k.nama_organisasi.clone()).unwrap_or_else(|| nama_event.clone());
        let logo = kop.as_ref().and_then(|k| k.logo.clone());
        let alamat = kop.as_ref().map(|k| k.alamat.clone());
        let kontak = kop.as_ref().map(|k| k.kontak.clone());

        // Parse venue and city from alamat or event
        let mut tempat = "GOR Gelora Kadrie Oening".to_string();
        let mut kota = "Samarinda".to_string();
        if let Some(ref al) = alamat {
            let lines: Vec<&str> = al.lines().map(|l| l.trim()).filter(|l| !l.is_empty()).collect();
            if !lines.is_empty() {
                tempat = lines[0].to_string();
                if lines.len() > 1 {
                    kota = lines[1].to_string();
                } else if lines[0].contains(',') {
                    let parts: Vec<&str> = lines[0].splitn(2, ',').collect();
                    tempat = parts[0].trim().to_string();
                    kota = parts[1].trim().to_string();
                }
            }
        }

        // 3. Timetable Dates
        let dates_rows: Vec<(Option<String>,)> = sqlx::query_as(
            "SELECT tanggal FROM backend_timetableday WHERE event_id = ? AND tanggal IS NOT NULL ORDER BY `order` ASC"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

        let valid_dates: Vec<String> = dates_rows.into_iter().filter_map(|r| r.0).collect();
        let tanggal_str = if valid_dates.is_empty() {
            "3 - 4 November 2026".to_string()
        } else if valid_dates.len() == 1 {
            valid_dates[0].clone()
        } else {
            format!("{} s.d {}", valid_dates.first().unwrap(), valid_dates.last().unwrap())
        };

        // 4. Statistics (Totals)
        let total_kontingen: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM backend_utusan WHERE event_id = ?")
            .bind(event_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0);

        let total_perguruan: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM backend_perguruan WHERE event_id = ?")
            .bind(event_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0);

        let total_atlet: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM backend_atlet WHERE event_id = ?")
            .bind(event_id)
            .fetch_one(pool)
            .await
            .unwrap_or(0);

        let total_partai: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(d.id)
            FROM backend_detailbagan d
            JOIN backend_bagan b ON d.bagan_id = b.id
            JOIN backend_nomortanding n ON b.nomor_tanding_id = n.id
            WHERE n.event_id = ? AND (d.atlet1_id IS NOT NULL OR d.atlet2_id IS NOT NULL)
            "#
        )
        .bind(event_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        let partai_selesai: i64 = sqlx::query_scalar(
            r#"
            SELECT COUNT(d.id)
            FROM backend_detailbagan d
            JOIN backend_bagan b ON d.bagan_id = b.id
            JOIN backend_nomortanding n ON b.nomor_tanding_id = n.id
            WHERE n.event_id = ? AND d.selesai = 1
            "#
        )
        .bind(event_id)
        .fetch_one(pool)
        .await
        .unwrap_or(0);

        // 5. Standings & Winners
        let summary_juara = self.get_summary_juara(event_id).await.unwrap_or_else(|_| SummaryJuaraResult {
            event_name: nama_event.clone(),
            utusan_standings: Vec::new(),
            perguruan_standings: Vec::new(),
            total_gold: 0,
            total_silver: 0,
            total_bronze: 0,
            total_uncounted: 0,
        });

        let summary_bagan = self.get_summary_bagan(event_id).await.unwrap_or_else(|_| SummaryBaganResult {
            event_name: nama_event.clone(),
            total_bagans: 0,
            completed_bagans: 0,
            total_gold: 0,
            total_silver: 0,
            total_bronze: 0,
            bagans: Vec::new(),
        });

        // 6. Referees
        let referees = self.get_wasits(event_id).await.unwrap_or_default();

        // 7. Signatories
        let signatories = vec![
            ResultsBookletSignatory {
                jabatan: "Technical Delegate (TD)".to_string(),
                peran: "Pengawas Pertandingan PB / Pengprov FORKI".to_string(),
                nama: "Sensei Dr. H. Ahmad Sudrajat, M.Pd".to_string(),
                gelar: Some("Dan VI Karate-Do".to_string()),
                nomor_lisensi: Some("TD-WKF/FORKI-2026".to_string()),
                instansi: Some("Dewan Wasit & Pertandingan FORKI".to_string()),
            },
            ResultsBookletSignatory {
                jabatan: "Ketua Panitia Pelaksana".to_string(),
                peran: "Penanggung Jawab Kejuaraan".to_string(),
                nama: "Bambang Kurniawan, S.E., M.M.".to_string(),
                gelar: Some("Ketua Pelaksana".to_string()),
                nomor_lisensi: None,
                instansi: Some("Panitia Pelaksana Kejuaraan".to_string()),
            },
            ResultsBookletSignatory {
                jabatan: "Ketua Dewan Wasit".to_string(),
                peran: "Pimpinan Perwasitan & Juri".to_string(),
                nama: "Shihan Ir. Hendra Wijaya".to_string(),
                gelar: Some("Dan VII / Wasit WKF A".to_string()),
                nomor_lisensi: Some("REF-WKF-7789".to_string()),
                instansi: Some("Komisi Perwasitan FORKI".to_string()),
            },
            ResultsBookletSignatory {
                jabatan: "Sekretaris Pertandingan".to_string(),
                peran: "Pencatatan & Administrasi Hasil".to_string(),
                nama: "Rizky Pratama, S.Kom".to_string(),
                gelar: Some("Sekretaris Pertandingan".to_string()),
                nomor_lisensi: None,
                instansi: Some("Sekretariat Pertandingan Ambrilindo".to_string()),
            },
        ];

        let event_info = ResultsBookletEventInfo {
            nama_event: nama_event.clone(),
            tanggal: tanggal_str.clone(),
            tempat: tempat.clone(),
            nama_organisasi,
            logo,
            alamat,
            kontak,
            kota: kota.clone(),
        };

        let berita_acara = ResultsBookletBeritaAcara {
            nomor_surat: format!("BA-FORKI/{}/2026", event_id),
            hari_tanggal: tanggal_str,
            total_kontingen,
            total_perguruan,
            total_atlet,
            total_partai,
            partai_selesai,
            catatan_sanksi: "Nihil / Seluruh pertandingan diselesaikan sesuai WKF Rules tanpa sanksi banding tertulis.".to_string(),
            status_pelaksanaan: if partai_selesai >= total_partai && total_partai > 0 {
                "SELESAI 100% (PARIPURNA)".to_string()
            } else {
                format!("BERLANGSUNG ({} / {} Partai Selesai)", partai_selesai, total_partai)
            },
            tanggal_pengesahan: "4 November 2026".to_string(),
            kota_pengesahan: kota,
        };

        let grand_total = summary_juara.total_gold + summary_juara.total_silver + summary_juara.total_bronze;

        let mut category_brackets: Vec<TatamiBookletCategory> = Vec::new();
        for (idx, b) in summary_bagan.bagans.iter().enumerate() {
            let matches = self.get_detail_bagan(b.id).await.unwrap_or_default();
            let matches_count = matches.len() as i64;
            let mut athlete_ids = std::collections::HashSet::new();
            for m in &matches {
                if let Some(a1) = m.atlet1_id { athlete_ids.insert(a1); }
                if let Some(a2) = m.atlet2_id { athlete_ids.insert(a2); }
            }
            let athletes_count = athlete_ids.len() as i64;

            category_brackets.push(TatamiBookletCategory {
                match_seq: idx + 1,
                category_id: b.id,
                category_name: b.nama_bagan.clone(),
                discipline: if b.tipe_tanding == "1" { "kata".to_string() } else { "kumite".to_string() },
                matches_count,
                athletes_count,
                bagan_id: Some(b.id),
                bagan_kode: Some(b.kode.clone()),
                bagan_nama: Some(b.nama_bagan.clone()),
                tipe_tanding: Some(b.tipe_tanding.clone()),
                winners: b.winners.clone(),
                matches,
            });
        }

        Ok(ResultsBookletData {
            event_info,
            berita_acara,
            signatories,
            standings_kontingen: summary_juara.utusan_standings,
            standings_perguruan: summary_juara.perguruan_standings,
            category_winners: summary_bagan.bagans,
            category_brackets,
            referees,
            total_gold: summary_juara.total_gold,
            total_silver: summary_juara.total_silver,
            total_bronze: summary_juara.total_bronze,
            grand_total,
        })
    }

    /// Fetches all candidate categories available for creating a Best of the Best (BOB) bracket
    pub async fn get_bob_candidates(&self, event_id: i64) -> Result<Vec<BobSourceCategory>, String> {
        let pool = self.get_pool()?;

        let rows = sqlx::query_as::<_, (i64, i64, String, String, Option<i64>, Option<String>, Option<String>, Option<String>)>(
            r#"
            SELECT 
                b.id as bagan_id,
                b.nomor_tanding_id,
                b.nama_bagan,
                b.kode as kode_bagan,
                b.juara_1_id,
                a.nama_atlet as juara_1_nama,
                u.nama_utusan as juara_1_kontingen,
                p.nama_perguruan as juara_1_perguruan
            FROM backend_bagan b
            LEFT JOIN backend_atlet a ON b.juara_1_id = a.id
            LEFT JOIN backend_utusan u ON a.utusan_id = u.id
            LEFT JOIN backend_perguruan p ON a.perguruan_id = p.id
            WHERE b.event_id = ? AND b.is_bob = 0
            ORDER BY b.kode ASC
            "#
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat kandidat bagan BOB: {}", e))?;

        let candidates = rows
            .into_iter()
            .map(|r| BobSourceCategory {
                bagan_id: r.0,
                nomor_tanding_id: r.1,
                nama_bagan: r.2,
                kode_bagan: r.3,
                juara_1_id: r.4,
                juara_1_nama: r.5,
                juara_1_kontingen: r.6,
                juara_1_perguruan: r.7,
                is_completed: r.4.is_some(),
            })
            .collect();

        Ok(candidates)
    }

    /// Creates a Best of the Best (BOB) tournament bracket from the gold medalists of selected categories
    pub async fn create_bob_bagan(&self, input: CreateBobBaganInput) -> Result<BobBaganResult, String> {
        let pool = self.get_pool()?;

        let nama_bob = input.nama_bob.trim();
        if nama_bob.is_empty() {
            return Err("Nama kategori / bagan Best of the Best (BOB) tidak boleh kosong.".to_string());
        }

        if input.source_bagan_ids.is_empty() {
            return Err("Pilih minimal 2 bagan sumber untuk membentuk turnamen BOB.".to_string());
        }

        // Fetch champions (juara_1_id) from chosen bagans
        let placeholders = input.source_bagan_ids.iter().map(|_| "?").collect::<Vec<_>>().join(",");
        let query_str = format!(
            "SELECT juara_1_id FROM backend_bagan WHERE id IN ({}) AND juara_1_id IS NOT NULL",
            placeholders
        );

        let mut q = sqlx::query_scalar::<_, i64>(&query_str);
        for id in &input.source_bagan_ids {
            q = q.bind(id);
        }

        let champion_ids = q.fetch_all(pool).await.map_err(|e| format!("Gagal membaca juara bagan: {}", e))?;

        if champion_ids.is_empty() {
            return Err("Tidak ada Juara 1 pada bagan-bagan yang dipilih. Pastikan pertandingan babak final telah selesai dipertandingkan.".to_string());
        }

        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        // 1. Create or get backend_nomortanding with is_bob = 1
        let existing_cat: Option<(i64,)> = sqlx::query_as(
            "SELECT id FROM backend_nomortanding WHERE event_id = ? AND nama_nomor_tanding = ? LIMIT 1"
        )
        .bind(input.event_id)
        .bind(nama_bob)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("Gagal cek nomor tanding: {}", e))?;

        let nomor_tanding_id = if let Some(cat) = existing_cat {
            sqlx::query("UPDATE backend_nomortanding SET is_bob = 1 WHERE id = ?")
                .bind(cat.0)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal update status BOB nomor tanding: {}", e))?;
            cat.0
        } else {
            let res = sqlx::query(
                "INSERT INTO backend_nomortanding (event_id, nama_nomor_tanding, is_bob, has_vr) VALUES (?, ?, 1, 0)"
            )
            .bind(input.event_id)
            .bind(nama_bob)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal membuat nomor tanding BOB: {}", e))?;
            res.last_insert_rowid()
        };

        // 2. Fetch original athlete records for champions
        let mut new_athletes: Vec<AthleteInput> = Vec::new();
        let mut athlete_count = 0;

        for champ_id in champion_ids {
            let a_opt: Option<(String, Option<i64>, Option<i64>, Option<String>, Option<String>, bool)> = sqlx::query_as(
                "SELECT nama_atlet, perguruan_id, utusan_id, kode_atlet, nik, is_priority FROM backend_atlet WHERE id = ?"
            )
            .bind(champ_id)
            .fetch_optional(&mut *tx)
            .await
            .unwrap_or(None);

            if let Some((nama, perguruan, utusan, kode, nik, priority)) = a_opt {
                // Insert clone into backend_atlet for this BOB division
                let ins_res = sqlx::query(
                    r#"
                    INSERT INTO backend_atlet (event_id, nomor_tanding_id, nama_atlet, perguruan_id, utusan_id, kode_atlet, nik, is_priority)
                    VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                    "#
                )
                .bind(input.event_id)
                .bind(nomor_tanding_id)
                .bind(&nama)
                .bind(perguruan)
                .bind(utusan)
                .bind(kode)
                .bind(nik)
                .bind(priority)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal insert atlet BOB: {}", e))?;

                let new_id = ins_res.last_insert_rowid();
                let group_key = if input.shuffle_type.as_deref() == Some("kontingen") {
                    format!("U_{}", utusan.unwrap_or(0))
                } else {
                    format!("P_{}", perguruan.unwrap_or(0))
                };

                new_athletes.push(AthleteInput {
                    id: new_id,
                    name: nama,
                    is_priority: priority,
                    group_key,
                    secondary_key: None,
                });
                athlete_count += 1;
            }
        }

        // 3. Generate Next Bagan Code
        let max_code_row: Option<(Option<String>,)> = sqlx::query_as(
            "SELECT kode FROM backend_bagan WHERE event_id = ? ORDER BY id DESC LIMIT 1"
        )
        .bind(input.event_id)
        .fetch_optional(&mut *tx)
        .await
        .unwrap_or(None);

        let mut next_code_num = 1;
        if let Some((Some(code_str),)) = max_code_row {
            if let Ok(num) = code_str.trim().parse::<i32>() {
                next_code_num = num + 1;
            }
        }
        let bagan_code = format!("{:03}", next_code_num);

        // 4. Create backend_bagan
        let is_round_robin = input.format_type.as_deref() == Some("round_robin");
        let bagan_res = sqlx::query(
            r#"
            INSERT INTO backend_bagan (event_id, nomor_tanding_id, nama_bagan, pool, tipe_tanding, round_robin, is_bob, has_vr, kode)
            VALUES (?, ?, ?, 1, '2', ?, 1, 0, ?)
            "#
        )
        .bind(input.event_id)
        .bind(nomor_tanding_id)
        .bind(nama_bob)
        .bind(if is_round_robin { 1 } else { 0 })
        .bind(&bagan_code)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Gagal membuat bagan BOB: {}", e))?;

        let bagan_id = bagan_res.last_insert_rowid();

        // 5. Populate detail bagan
        if is_round_robin {
            let athlete_ids: Vec<i64> = new_athletes.iter().map(|a| a.id).collect();
            let schedule = generate_round_robin_schedule(&athlete_ids);
            for match_sched in schedule {
                sqlx::query(
                    r#"
                    INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, selesai, team)
                    VALUES (?, ?, ?, ?, ?, 0, 0)
                    "#
                )
                .bind(bagan_id)
                .bind(match_sched.round)
                .bind(match_sched.urutan)
                .bind(match_sched.atlet1_id)
                .bind(match_sched.atlet2_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal menyimpan partai round robin BOB: {}", e))?;
            }
        } else {
            // Elimination tournament
            let bracket_data = build_bracket_in_memory(&new_athletes, 1, false);
            for r in 1..=4 {
                let max_u = match r { 1 => 8, 2 => 4, 3 => 2, _ => 1 };
                for u in 1..=max_u {
                    let m = bracket_data.matches.get(&r).and_then(|round| round.get(&u)).cloned().unwrap_or_default();
                    sqlx::query(
                        r#"
                        INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team)
                        VALUES (?, ?, ?, ?, ?, ?, ?, 0, 0, 0)
                        "#
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
                    .map_err(|e| format!("Gagal menyimpan detail bagan BOB: {}", e))?;
                }
            }
            // Round 5 (Juara 3 Playoff)
            let m5 = bracket_data.matches.get(&5).and_then(|round| round.get(&1)).cloned().unwrap_or_default();
            let _ = sqlx::query(
                r#"
                INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team)
                VALUES (?, 5, 1, ?, ?, ?, ?, 0, 0, 0)
                "#
            )
            .bind(bagan_id)
            .bind(m5.atlet1_id)
            .bind(m5.atlet2_id)
            .bind(m5.vr1)
            .bind(m5.vr2)
            .execute(&mut *tx)
            .await;
        }

        tx.commit().await.map_err(|e| format!("Gagal commit transaksi: {}", e))?;

        Ok(BobBaganResult {
            nomor_tanding_id,
            bagan_id,
            athlete_count,
            nama_bob: nama_bob.to_string(),
        })
    }
    /// Performs an atomic, self-contained backup of the active SQLite database using VACUUM INTO.
    pub async fn backup_database(&self, custom_path: Option<String>) -> Result<String, String> {
        let pool = self.get_pool()?;
        let active_path = self.active_db_path.as_ref().ok_or("Path database aktif tidak diketahui.")?;

        let backup_path = if let Some(p) = custom_path {
            PathBuf::from(p)
        } else {
            let parent = active_path.parent().unwrap_or_else(|| Path::new("."));
            let backup_dir = parent.join("backups");
            if !backup_dir.exists() {
                let _ = std::fs::create_dir_all(&backup_dir);
            }
            let epoch = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            backup_dir.join(format!("ambrilindo_backup_{}.sqlite3", epoch))
        };

        let backup_str = backup_path.to_string_lossy().to_string();

        // Checkpoint WAL to flush outstanding write transactions
        let _ = sqlx::query("PRAGMA wal_checkpoint(PASSIVE)").execute(pool).await;

        let escaped = backup_str.replace('\'', "''");
        let query_str = format!("VACUUM INTO '{}'", escaped);
        sqlx::query(&query_str)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal mencadangkan database: {}", e))?;

        info!("Cadangan database berhasil dibuat di: {}", backup_str);
        Ok(backup_str)
    }

}

// Helper functions for Timetable
fn helper_get_age_group(name: &str) -> String {
    let lower = name.to_lowercase();
    if lower.contains("pra usia dini") {
        "Pra Usia Dini".to_string()
    } else if lower.contains("usia dini") {
        "Usia Dini".to_string()
    } else if lower.contains("pra pemula") {
        "Pra Pemula".to_string()
    } else if lower.contains("pemula") {
        "Pemula".to_string()
    } else if lower.contains("kadet") || lower.contains("cadet") {
        "Kadet".to_string()
    } else if lower.contains("junior") {
        "Junior".to_string()
    } else if lower.contains("under-21") || lower.contains("under 21") || lower.contains("u-21") || lower.contains("u21") {
        "Under-21".to_string()
    } else if lower.contains("senior") {
        "Senior".to_string()
    } else if lower.contains("veteran") {
        "Veteran".to_string()
    } else {
        "Lainnya".to_string()
    }
}

fn helper_format_day_label(order: i32, tanggal: &Option<String>) -> String {
    let day_num = order + 1;
    if let Some(t_str) = tanggal {
        if !t_str.trim().is_empty() {
            let parts: Vec<&str> = t_str.trim().split('-').collect();
            if parts.len() == 3 {
                if let (Ok(year), Ok(month), Ok(day)) = (parts[0].parse::<i32>(), parts[1].parse::<usize>(), parts[2].parse::<i32>()) {
                    let indo_months = [
                        "Januari", "Februari", "Maret", "April", "Mei", "Juni",
                        "Juli", "Agustus", "September", "Oktober", "November", "Desember"
                    ];
                    if month >= 1 && month <= 12 {
                        return format!("Day {}, {} {} {}", day_num, day, indo_months[month - 1], year);
                    }
                }
            }
            return format!("Day {}, {}", day_num, t_str);
        }
    }
    format!("Day {}", day_num)
}

fn helper_normalize_name(name: &str) -> String {
    let mut clean = String::with_capacity(name.len());
    for c in name.chars() {
        if c.is_ascii_alphanumeric() || c == ' ' {
            clean.push(c.to_ascii_uppercase());
        }
    }
    clean.split_whitespace().collect::<Vec<&str>>().join(" ")
}

fn helper_extract_parentheses(text: &str) -> Vec<String> {
    let mut results = Vec::new();
    let mut in_paren = false;
    let mut current = String::new();
    for c in text.chars() {
        if c == '(' {
            in_paren = true;
            current.clear();
        } else if c == ')' {
            if in_paren && !current.trim().is_empty() {
                results.push(current.trim().to_string());
            }
            in_paren = false;
        } else if in_paren {
            current.push(c);
        }
    }
    results
}

// --- Helper Functions for Festival Roster ---

fn helper_clean_festival_display_name(raw: &str) -> String {
    let trimmed = raw.trim();
    if let Some(stripped) = trimmed.strip_prefix("FESTIVAL ") {
        stripped.trim().to_string()
    } else if let Some(stripped) = trimmed.strip_prefix("Festival ") {
        stripped.trim().to_string()
    } else {
        trimmed.to_string()
    }
}

fn helper_festival_category_sort_key(name: &str) -> (i32, i32, i32, String) {
    let lower = name.to_lowercase();
    let disc_rank = if lower.contains("kata") { 0 } else { 1 };
    let age_rank = if lower.contains("pra usia dini") {
        0
    } else if lower.contains("usia dini") {
        1
    } else if lower.contains("pra pemula") {
        2
    } else if lower.contains("pemula") {
        3
    } else if lower.contains("kadet") || lower.contains("cadet") {
        4
    } else if lower.contains("junior") {
        5
    } else if lower.contains("under") || lower.contains("u-21") || lower.contains("u21") {
        6
    } else if lower.contains("senior") {
        7
    } else if lower.contains("veteran") {
        8
    } else {
        9
    };
    let gender_rank = if lower.contains("putra") {
        0
    } else if lower.contains("putri") {
        1
    } else {
        2
    };
    (disc_rank, age_rank, gender_rank, name.to_string())
}

fn helper_pair_festival_athletes_anti_collision(athletes: Vec<FestivalAthlete>) -> Vec<FestivalAthlete> {
    if athletes.len() <= 1 {
        return athletes;
    }
    use rand::Rng;
    use std::collections::HashMap;

    let mut groups: HashMap<String, Vec<FestivalAthlete>> = HashMap::new();
    for a in athletes {
        let u = if a.utusan.trim().is_empty() {
            "UNKNOWN".to_string()
        } else {
            a.utusan.trim().to_uppercase()
        };
        groups.entry(u).or_default().push(a);
    }

    let mut sorted_groups: Vec<Vec<FestivalAthlete>> = groups.into_values().collect();
    sorted_groups.sort_by(|a, b| b.len().cmp(&a.len()));

    let mut all_sorted = Vec::new();
    for g in sorted_groups {
        all_sorted.extend(g);
    }

    let n = all_sorted.len();
    let half = (n + 1) / 2;
    let m_list = all_sorted[..half].to_vec();
    let b_list = all_sorted[half..].to_vec();

    let mut res = Vec::new();
    for i in 0..b_list.len() {
        res.push(m_list[i].clone());
        res.push(b_list[i].clone());
    }
    if m_list.len() > b_list.len() {
        res.push(m_list.last().unwrap().clone());
    }

    let count_clashes = |arr: &[FestivalAthlete]| -> usize {
        let mut clashes = 0;
        for i in (0..arr.len().saturating_sub(1)).step_by(2) {
            let u1 = arr[i].utusan.trim().to_uppercase();
            let u2 = arr[i + 1].utusan.trim().to_uppercase();
            if !u1.is_empty() && !u2.is_empty() && u1 == u2 {
                clashes += 1;
            }
        }
        clashes
    };

    let mut best = res;
    let mut best_c = count_clashes(&best);
    let mut rng = rand::thread_rng();

    for _ in 0..500 {
        if best_c == 0 {
            break;
        }
        let i1 = rng.gen_range(0..n);
        let i2 = rng.gen_range(0..n);
        if i1 == i2 {
            continue;
        }
        let mut test = best.clone();
        test.swap(i1, i2);
        let c = count_clashes(&test);
        if c < best_c {
            best = test;
            best_c = c;
        }
    }

    best
}

fn helper_get_festival_roster_file_path(db_path: Option<&std::path::Path>, event_id: i64) -> std::path::PathBuf {
    let base = if let Some(p) = db_path {
        if let Some(parent) = p.parent() {
            parent.to_path_buf()
        } else {
            std::path::PathBuf::from("tournament_data")
        }
    } else {
        std::path::PathBuf::from("tournament_data")
    };
    let _ = std::fs::create_dir_all(&base);
    base.join(format!("festival_roster_{}.json", event_id))
}

fn helper_load_festival_roster_state(db_path: Option<&std::path::Path>, event_id: i64) -> FestivalRosterState {
    let path = helper_get_festival_roster_file_path(db_path, event_id);
    if path.exists() {
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(state) = serde_json::from_str::<FestivalRosterState>(&content) {
                return state;
            }
        }
    }
    FestivalRosterState {
        tatami_assignments: std::collections::HashMap::new(),
        athlete_pairings: std::collections::HashMap::new(),
    }
}

fn helper_save_festival_roster_state(db_path: Option<&std::path::Path>, event_id: i64, state: &FestivalRosterState) -> Result<(), String> {
    let path = helper_get_festival_roster_file_path(db_path, event_id);
    let json_str = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(&path, json_str).map_err(|e| e.to_string())
}


#[cfg(test)]
mod festival_tests {
    #[tokio::test]
    async fn test_draw_event9_real() {
        let db_path = std::path::Path::new("../tournament_data/db.sqlite3");
        if !db_path.exists() { return; }

        let mut db = DatabaseManager::new();
        db.mount_database(&db_path).await.expect("Mount real db failed");

        // Draw categories 881 (15 athletes), 888 (8 athletes), 904 (4 athletes)
        let summary = db.execute_drawing_festival_bagan(9, vec![881, 888, 904], "both").await.expect("Real draw failed");
        println!("Real draw summary: processed={}, bagans={}, matches={}", summary.categories_processed, summary.bagans_created, summary.total_matches);
    }

    use super::*;

    #[tokio::test]
    async fn test_drawing_and_progression_festival() {
        let original_db = std::path::Path::new("../tournament_data/db.sqlite3");
        if !original_db.exists() {
            println!("Skipping test: db.sqlite3 not found at {:?}", original_db);
            return;
        }

        let temp_dir = std::env::temp_dir();
        let test_db_path = temp_dir.join(format!("test_fest_{}.sqlite3", rand::random::<u64>()));
        std::fs::copy(original_db, &test_db_path).expect("Gagal copy test db");

        let mut db = DatabaseManager::new();
        db.mount_database(&test_db_path).await.expect("Mount db failed");

        // 1. Test drawing eligible festival categories for Event 9
        let eligible = db.get_drawing_eligible_festival_categories(9).await.expect("get eligible failed");
        assert!(!eligible.is_empty(), "Event 9 harus memiliki kategori festival");
        let cat881 = eligible.iter().find(|c| c.id == 881).expect("Cat 881 tidak ditemukan");
        assert_eq!(cat881.athlete_count, 15);

        // 2. Execute drawing for Category 881
        let summary = db.execute_drawing_festival_bagan(9, vec![881], "both").await.expect("drawing failed");
        assert_eq!(summary.categories_processed, 1);
        assert_eq!(summary.bagans_created, 1);
        assert_eq!(summary.total_matches, 12); // 4 groups * 3 matches = 12

        // 3. Fetch the created festival bagan
        let bagans = db.get_bagans(9).await.expect("get_bagans failed");
        let fest_bagan = bagans.iter().find(|b| b.nomor_tanding_id == Some(881)).expect("Bagan festival 881 tidak ditemukan");
        assert_eq!(fest_bagan.pool, 4, "15 atlet harus membentuk 4 grup");
        assert!(fest_bagan.kode.as_ref().map(|k| k.starts_with('F')).unwrap_or(false));

        // 4. Fetch detail matches
        let matches = db.get_detail_bagan(fest_bagan.id).await.expect("get_detail_bagan failed");
        assert_eq!(matches.len(), 12);

        // Check Group 1 (semi 1: round 1 urutan 1, semi 2: round 1 urutan 2, final: round 2 urutan 1)
        let g1_semi1 = matches.iter().find(|m| m.round == Some(1) && m.urutan == Some(1)).expect("semi 1 not found");
        let g1_semi2 = matches.iter().find(|m| m.round == Some(1) && m.urutan == Some(2)).expect("semi 2 not found");
        let g1_final = matches.iter().find(|m| m.round == Some(2) && m.urutan == Some(1)).expect("final not found");

        assert!(g1_semi1.atlet1_id.is_some() && g1_semi1.atlet2_id.is_some());
        assert!(g1_semi2.atlet1_id.is_some() && g1_semi2.atlet2_id.is_some());
        assert!(g1_final.atlet1_id.is_none() && g1_final.atlet2_id.is_none());

        // 5. Score Semi 1 (Aka wins) -> check advancement to final slot 1
        let winner_semi1 = g1_semi1.atlet1_id.unwrap();
        db.save_match_result(SaveMatchInput {
            match_id: g1_semi1.id,
            bagan_id: fest_bagan.id,
            score1: Some("3".into()),
            score2: Some("0".into()),
            scorekecil1: None,
            scorekecil2: None,
            pemenang: Some("1".into()),
            kata1: None,
            kata2: None,
            vr1: false,
            vr2: false,
            hantei: false,
            selesai: true,
        }).await.expect("save semi 1 failed");

        // Verify final match slot 1 has winner_semi1
        let matches_after_s1 = db.get_detail_bagan(fest_bagan.id).await.expect("get detail failed");
        let g1_final_after_s1 = matches_after_s1.iter().find(|m| m.round == Some(2) && m.urutan == Some(1)).unwrap();
        assert_eq!(g1_final_after_s1.atlet1_id, Some(winner_semi1));

        // 6. Score Semi 2 (Ao wins) -> check advancement to final slot 2
        let winner_semi2 = g1_semi2.atlet2_id.unwrap();
        db.save_match_result(SaveMatchInput {
            match_id: g1_semi2.id,
            bagan_id: fest_bagan.id,
            score1: Some("1".into()),
            score2: Some("4".into()),
            scorekecil1: None,
            scorekecil2: None,
            pemenang: Some("2".into()),
            kata1: None,
            kata2: None,
            vr1: false,
            vr2: false,
            hantei: false,
            selesai: true,
        }).await.expect("save semi 2 failed");

        // Verify final match slot 2 has winner_semi2
        let matches_after_s2 = db.get_detail_bagan(fest_bagan.id).await.expect("get detail failed");
        let g1_final_after_s2 = matches_after_s2.iter().find(|m| m.round == Some(2) && m.urutan == Some(1)).unwrap();
        assert_eq!(g1_final_after_s2.atlet2_id, Some(winner_semi2));

        // 7. Score Final (Ao wins -> winner_semi2 gets Gold, winner_semi1 gets Silver)
        db.save_match_result(SaveMatchInput {
            match_id: g1_final_after_s2.id,
            bagan_id: fest_bagan.id,
            score1: Some("2".into()),
            score2: Some("5".into()),
            scorekecil1: None,
            scorekecil2: None,
            pemenang: Some("2".into()),
            kata1: None,
            kata2: None,
            vr1: false,
            vr2: false,
            hantei: false,
            selesai: true,
        }).await.expect("save final failed");

        let bagan_final = db.get_bagan_by_id(fest_bagan.id).await.expect("get bagan failed").unwrap();
        assert_eq!(bagan_final.juara_1_id, Some(winner_semi2));
        assert_eq!(bagan_final.juara_2_id, Some(winner_semi1));

        // Cleanup
        let _ = std::fs::remove_file(&test_db_path);
        println!("test_drawing_and_progression_festival PASSED successfully!");
    }
}
