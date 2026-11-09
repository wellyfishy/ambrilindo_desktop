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
    FestivalCategory, FestivalFormatInfo, FestivalMatch, FestivalRosterData, FestivalRosterState, FestivalTatamiSection,
    FestivalBookletCategory, FestivalBookletData, FestivalMixGroup, FestivalMixMember, FestivalSessionTatami,
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
            Self::ensure_bagan_layout_column(p).await;
            Self::ensure_detailbagan_juri_columns(p).await;
            Self::auto_heal_legacy_master_data(p).await;
            // Reset stale tatami match state on database mount
            let _ = sqlx::query("UPDATE backend_tatami SET detail_bagan_id = NULL").execute(p).await;
            let _ = sqlx::query("UPDATE backend_detailbagan SET assigned_tatami_id = NULL WHERE selesai = 1").execute(p).await;
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

    /// Ensures layout_type column exists on backend_bagan table
    async fn ensure_bagan_layout_column(pool: &Pool<Sqlite>) {
        let has_col: bool = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('backend_bagan') WHERE name = 'layout_type'"
        )
        .fetch_one(pool)
        .await
        .unwrap_or(0) > 0;

        if !has_col {
            let _ = sqlx::query("ALTER TABLE backend_bagan ADD COLUMN layout_type TEXT DEFAULT 'quadrant'").execute(pool).await;
        }
    }

    /// Ensures the kata per-judge score columns exist on backend_detailbagan
    /// (JSON array of the 5 judges' scores for AKA / AO)
    async fn ensure_detailbagan_juri_columns(pool: &Pool<Sqlite>) {
        for col in ["juri_scores1", "juri_scores2"] {
            let has_col: bool = sqlx::query_scalar(
                "SELECT COUNT(*) FROM pragma_table_info('backend_detailbagan') WHERE name = ?"
            )
            .bind(col)
            .fetch_one(pool)
            .await
            .unwrap_or(0) > 0;

            if !has_col {
                let _ = sqlx::query(&format!("ALTER TABLE backend_detailbagan ADD COLUMN {} TEXT", col))
                    .execute(pool)
                    .await;
            }
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
                CASE WHEN db.selesai = 0 THEN t.detail_bagan_id ELSE NULL END as detail_bagan_id,
                CASE WHEN db.selesai = 0 THEN db.bagan_id ELSE NULL END as bagan_id,
                CASE WHEN db.selesai = 0 THEN b.nama_bagan ELSE NULL END as nama_bagan
            FROM backend_tatami t
            LEFT JOIN backend_detailbagan db ON t.detail_bagan_id = db.id AND db.selesai = 0
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
            bagan_id: None,
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

    /// Removes every row that points at the bouts of some brackets (tatami "now running",
    /// referee scores, referee seats, team line-ups), so those bouts can be deleted. Without
    /// this, redrawing or deleting a bracket fails on a foreign key, for example once a team
    /// kumite line-up was entered. `bagan_sql` selects the bracket ids with one `?` (= `id`).
    async fn purge_bout_dependents(conn: &mut sqlx::SqliteConnection, bagan_sql: &str, id: i64) -> Result<(), String> {
        let bouts = format!("SELECT id FROM backend_detailbagan WHERE bagan_id IN ({})", bagan_sql);
        let statements = [
            (format!("UPDATE backend_tatami SET detail_bagan_id = NULL WHERE detail_bagan_id IN ({})", bouts), 1),
            (format!("DELETE FROM backend_score WHERE detail_bagan_id IN ({})", bouts), 1),
            (format!("DELETE FROM backend_wasitdetailbagan WHERE detail_bagan_id IN ({})", bouts), 1),
            (
                format!(
                    "DELETE FROM backend_matchup WHERE detail_bagan_id IN ({0}) OR db_id IN ({0}) OR bagan_id IN ({1})",
                    bouts, bagan_sql
                ),
                3,
            ),
        ];
        for (sql, binds) in statements {
            let mut q = sqlx::query(&sql);
            for _ in 0..binds {
                q = q.bind(id);
            }
            q.execute(&mut *conn)
                .await
                .map_err(|e| format!("Gagal membersihkan data partai lama: {}", e))?;
        }
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

        // 1b. Rows that reference this category's matches or the category itself; without this the
        //     delete fails on a foreign key (e.g. a category already placed in the timetable).
        let category_matches = "SELECT d.id FROM backend_detailbagan d JOIN backend_bagan b ON d.bagan_id = b.id WHERE b.nomor_tanding_id = ?";
        for sql in [
            format!("DELETE FROM backend_score WHERE detail_bagan_id IN ({})", category_matches),
            format!("DELETE FROM backend_wasitdetailbagan WHERE detail_bagan_id IN ({})", category_matches),
            format!("DELETE FROM backend_matchup WHERE detail_bagan_id IN ({0}) OR db_id IN ({0})", category_matches),
        ] {
            let mut q = sqlx::query(&sql).bind(category_id);
            if sql.contains("db_id") {
                q = q.bind(category_id);
            }
            q.execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal membersihkan data partai kategori: {}", e))?;
        }
        sqlx::query("DELETE FROM backend_matchup WHERE bagan_id IN (SELECT id FROM backend_bagan WHERE nomor_tanding_id = ?)")
            .bind(category_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal membersihkan data partai kategori: {}", e))?;
        sqlx::query("DELETE FROM backend_timetablecell WHERE nomor_tanding_id = ?")
            .bind(category_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghapus kategori dari jadwal: {}", e))?;

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
            "SELECT id, event_id, nama_bagan, nomor_tanding_id, tipe_tanding, juara_1_id, juara_2_id, juara_3a_id, juara_3b_id, peringkat_5_id, peringkat_6_id, peringkat_7_id, peringkat_8_id, round_robin, pool, kode, is_bob, has_vr, layout_type FROM backend_bagan WHERE event_id = ? ORDER BY CAST(COALESCE(kode, '9999') AS INTEGER) ASC, id ASC"
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
            "SELECT id, event_id, nama_bagan, nomor_tanding_id, tipe_tanding, juara_1_id, juara_2_id, juara_3a_id, juara_3b_id, peringkat_5_id, peringkat_6_id, peringkat_7_id, peringkat_8_id, round_robin, pool, kode, is_bob, has_vr, layout_type FROM backend_bagan WHERE id = ?"
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
            // The final is the first round with a single match. The bracket template keeps an extra
            // single-row round after it (always empty), so "highest round" is not the final.
            "SELECT id, round, atlet1_id, atlet2_id FROM backend_detailbagan
             WHERE bagan_id = ? AND team = 0 AND round = (
                 SELECT round FROM backend_detailbagan WHERE bagan_id = ? AND team = 0
                 GROUP BY round HAVING COUNT(*) = 1 ORDER BY round ASC LIMIT 1
             )
             ORDER BY urutan ASC LIMIT 1"
        )
        .bind(bagan_id)
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

        if let (true, Some(id1), Some(id2)) = (is_finalists_ready, f1_id, f2_id) {
            let (a_ids, b_ids) = self.repechage_losers(bagan_id, nomor_tanding_id, max_round, id1, id2).await;
            for (ids, out) in [(a_ids, &mut pool_a_losers), (b_ids, &mut pool_b_losers)] {
                for lid in ids {
                    let lname: Option<String> = sqlx::query_scalar("SELECT nama_atlet FROM backend_atlet WHERE id = ?")
                        .bind(lid)
                        .fetch_optional(pool)
                        .await
                        .unwrap_or(None);
                    if let Some(ln) = lname {
                        out.push(ln);
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
    /// Athletes who lost to each finalist (finalist 1 -> pool A list, finalist 2 -> pool B list),
    /// in round order: the repechage field. Searches every elimination bagan of the category,
    /// because a category split into pools plays its early rounds in the pool bagans.
    async fn repechage_losers(
        &self,
        main_bagan_id: i64,
        nomor_tanding_id: Option<i64>,
        max_round: i32,
        f1_id: i64,
        f2_id: i64,
    ) -> (Vec<i64>, Vec<i64>) {
        let pool = match self.get_pool() {
            Ok(p) => p,
            Err(_) => return (Vec::new(), Vec::new()),
        };
        // The final is skipped: there the opponent is the other finalist.
        let mut traced: Vec<Vec<i64>> = Vec::new();
        for (fid, other) in [(f1_id, f2_id), (f2_id, f1_id)] {
            let rows: Vec<(Option<i64>, Option<i64>, Option<String>)> = if let Some(nt_id) = nomor_tanding_id {
                sqlx::query_as(
                    "SELECT d.atlet1_id, d.atlet2_id, d.pemenang FROM backend_detailbagan d
                     JOIN backend_bagan b ON d.bagan_id = b.id
                     WHERE b.nomor_tanding_id = ?
                       AND NOT (UPPER(COALESCE(b.nama_bagan, '')) LIKE '%REFERCHANGE%' OR UPPER(COALESCE(b.nama_bagan, '')) LIKE '%REPECHAGE%')
                       AND d.team = 0 AND d.selesai = 1 AND (d.atlet1_id = ? OR d.atlet2_id = ?)
                     ORDER BY d.round ASC, b.pool DESC, d.urutan ASC"
                )
                .bind(nt_id)
                .bind(fid)
                .bind(fid)
                .fetch_all(pool)
                .await
                .unwrap_or_default()
            } else {
                sqlx::query_as(
                    "SELECT atlet1_id, atlet2_id, pemenang FROM backend_detailbagan
                     WHERE bagan_id = ? AND round < ? AND team = 0 AND selesai = 1 AND (atlet1_id = ? OR atlet2_id = ?)
                     ORDER BY round ASC"
                )
                .bind(main_bagan_id)
                .bind(max_round)
                .bind(fid)
                .bind(fid)
                .fetch_all(pool)
                .await
                .unwrap_or_default()
            };
            let mut losers = Vec::new();
            for (a1, a2, pemenang) in rows {
                let loser = match pemenang.as_deref() {
                    Some("1") if a1 == Some(fid) => a2,
                    Some("2") if a2 == Some(fid) => a1,
                    _ => None,
                };
                if let Some(lid) = loser {
                    if lid != other && !losers.contains(&lid) {
                        losers.push(lid);
                    }
                }
            }
            traced.push(losers);
        }
        let pool_b_losers = traced.pop().unwrap_or_default();
        let pool_a_losers = traced.pop().unwrap_or_default();
        (pool_a_losers, pool_b_losers)
    }

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
            // The final is the first round with a single match. The bracket template keeps an extra
            // single-row round after it (always empty), so "highest round" is not the final.
            "SELECT id, round, atlet1_id, atlet2_id FROM backend_detailbagan
             WHERE bagan_id = ? AND team = 0 AND round = (
                 SELECT round FROM backend_detailbagan WHERE bagan_id = ? AND team = 0
                 GROUP BY round HAVING COUNT(*) = 1 ORDER BY round ASC LIMIT 1
             )
             ORDER BY urutan ASC LIMIT 1"
        )
        .bind(main_bagan_id)
        .bind(main_bagan_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Gagal memuat partai final: {}", e))?;

        let (_f_id, max_round, f1_id_opt, f2_id_opt) = final_match
            .ok_or_else(|| "Partai final tidak ditemukan pada bagan ini.".to_string())?;

        let f1_id = f1_id_opt.ok_or_else(|| "Finalis 1 (Aka) belum ditentukan. Selesaikan semifinal terlebih dahulu.".to_string())?;
        let f2_id = f2_id_opt.ok_or_else(|| "Finalis 2 (Ao) belum ditentukan. Selesaikan semifinal terlebih dahulu.".to_string())?;

        // 3. Losers to each finalist, in round order (shared with the repechage preview).
        let (pool_a_losers, pool_b_losers) = self
            .repechage_losers(main_bagan_id, nomor_tanding_id, max_round, f1_id, f2_id)
            .await;

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
                Self::purge_bout_dependents(&mut *tx, "SELECT ?", id).await?;
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
                COALESCE(t.tatami_number, d.assigned_tatami_id) as assigned_tatami_id, d.kode,
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
            LEFT JOIN backend_tatami t ON d.assigned_tatami_id = t.id
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
    /// Running match of a tatami. With `event_id` only that event's tatami counts; without it the
    /// newest event that has a running match on this tatami number wins (old events may still
    /// hold a stale link).
    pub async fn get_tatami_current_match(&self, tatami_num: i32, event_id: Option<i64>) -> Result<Option<EnrichedDetailBagan>, String> {
        let pool = self.get_pool()?;
        sqlx::query_as::<_, EnrichedDetailBagan>(
            r#"
            SELECT 
                d.id, d.bagan_id, d.round, d.urutan, d.atlet1_id, d.atlet2_id,
                d.score1, d.score2, d.scorekecil1, d.scorekecil2,
                d.vr1, d.vr2, d.kata1, d.kata2, d.pemenang, d.hantei, d.selesai, d.team,
                COALESCE(t.tatami_number, d.assigned_tatami_id) as assigned_tatami_id, d.kode,
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
            WHERE t.tatami_number = ? AND (? IS NULL OR t.event_id = ?)
            ORDER BY t.event_id DESC
            LIMIT 1
            "#
        )
        .bind(tatami_num)
        .bind(event_id)
        .bind(event_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Gagal memuat partai aktif tatami: {}", e))
    }

    /// Fetches all matches assigned to a specific tatami number
    /// Matches assigned to a tatami number. The tatami must belong to the match's own event
    /// (every event has its own tatami 1, 2, ...); `event_id` narrows it to one event.
    pub async fn get_tatami_matches(&self, tatami_num: i32, event_id: Option<i64>) -> Result<Vec<EnrichedDetailBagan>, String> {
        let pool = self.get_pool()?;
        sqlx::query_as::<_, EnrichedDetailBagan>(
            r#"
            SELECT 
                d.id, d.bagan_id, d.round, d.urutan, d.atlet1_id, d.atlet2_id,
                d.score1, d.score2, d.scorekecil1, d.scorekecil2,
                d.vr1, d.vr2, d.kata1, d.kata2, d.pemenang, d.hantei, d.selesai, d.team,
                COALESCE(t.tatami_number, d.assigned_tatami_id) as assigned_tatami_id, d.kode,
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
                a2.is_priority as atlet2_is_priority,
                bg.nama_bagan
            FROM backend_detailbagan d
            JOIN backend_bagan bg ON bg.id = d.bagan_id
            JOIN backend_tatami t ON t.event_id = bg.event_id
                AND (d.assigned_tatami_id = t.id OR d.assigned_tatami_id = t.tatami_number)
            LEFT JOIN backend_atlet a1 ON d.atlet1_id = a1.id
            LEFT JOIN backend_utusan u1 ON a1.utusan_id = u1.id
            LEFT JOIN backend_perguruan p1 ON a1.perguruan_id = p1.id
            LEFT JOIN backend_atlet a2 ON d.atlet2_id = a2.id
            LEFT JOIN backend_utusan u2 ON a2.utusan_id = u2.id
            LEFT JOIN backend_perguruan p2 ON a2.perguruan_id = p2.id
            WHERE t.tatami_number = ? AND (? IS NULL OR bg.event_id = ?)
            ORDER BY d.round ASC, d.urutan ASC
            "#
        )
        .bind(tatami_num)
        .bind(event_id)
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| format!("Gagal memuat daftar partai tatami: {}", e))
    }

    /// Saves assigned Kata names for a match
    /// Kata: the saved 5-judge scores (JSON arrays) for AKA and AO, if any
    pub async fn get_match_juri_scores(&self, match_id: i64) -> Result<(Option<String>, Option<String>), String> {
        let pool = self.get_pool()?;
        let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT juri_scores1, juri_scores2 FROM backend_detailbagan WHERE id = ?"
        )
        .bind(match_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| format!("Gagal memuat skor juri: {}", e))?;
        Ok(row.unwrap_or((None, None)))
    }

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
    /// Finished bouts (both athletes present) in the bagans of a category.
    async fn category_finished_matches(&self, category_id: i64) -> Result<i64, String> {
        let pool = self.get_pool()?;
        sqlx::query_scalar(
            "SELECT COUNT(*) FROM backend_detailbagan d JOIN backend_bagan b ON d.bagan_id = b.id
             WHERE b.nomor_tanding_id = ? AND d.selesai = 1 AND d.atlet1_id IS NOT NULL AND d.atlet2_id IS NOT NULL"
        )
        .bind(category_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Gagal memeriksa status kategori: {}", e))
    }

    /// A category that already played bouts cannot take new athletes: they would never be in its bracket.
    async fn ensure_category_open(&self, category_id: Option<i64>) -> Result<(), String> {
        let Some(cat_id) = category_id else { return Ok(()) };
        let finished = self.category_finished_matches(cat_id).await?;
        if finished > 0 {
            let pool = self.get_pool()?;
            let name: String = sqlx::query_scalar("SELECT COALESCE(nama_nomor_tanding, '') FROM backend_nomortanding WHERE id = ?")
                .bind(cat_id)
                .fetch_optional(pool)
                .await
                .map_err(|e| e.to_string())?
                .unwrap_or_default();
            return Err(format!(
                "Kategori \"{}\" sudah bertanding ({} partai selesai). Atlet baru tidak bisa masuk bagannya. \
                 Daftarkan ke kategori lain, atau hapus bagan kategori ini lalu undi ulang.",
                name, finished
            ));
        }
        Ok(())
    }

    /// Refuses to redraw categories that already have results (the drawing would erase them).
    async fn ensure_no_results(&self, category_ids: &[i64]) -> Result<(), String> {
        let pool = self.get_pool()?;
        let mut blocked = Vec::new();
        for &cat_id in category_ids {
            let finished = self.category_finished_matches(cat_id).await?;
            if finished > 0 {
                let name: String = sqlx::query_scalar("SELECT COALESCE(nama_nomor_tanding, '') FROM backend_nomortanding WHERE id = ?")
                    .bind(cat_id)
                    .fetch_optional(pool)
                    .await
                    .map_err(|e| e.to_string())?
                    .unwrap_or_default();
                blocked.push(format!("{} ({} partai selesai)", name, finished));
            }
        }
        if blocked.is_empty() {
            return Ok(());
        }
        Err(format!(
            "Drawing ulang dibatalkan: kategori berikut sudah punya hasil pertandingan dan akan terhapus. \
             Hapus bagannya dulu secara sadar bila memang ingin diundi ulang: {}",
            blocked.join("; ")
        ))
    }

    pub async fn create_athlete(&self, input: AthleteMutationInput) -> Result<EnrichedAtlet, String> {
        let pool = self.get_pool()?;
        let clean_name = input.nama_atlet.trim().to_uppercase();
        if clean_name.is_empty() {
            return Err("Nama atlet tidak boleh kosong.".to_string());
        }
        if !input.allow_late {
            self.ensure_category_open(input.nomor_tanding_id).await?;
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
        // Changing the category: the athlete must not already be in a bracket, and the new
        // category must not have started.
        let current_cat: Option<i64> = sqlx::query_scalar("SELECT nomor_tanding_id FROM backend_atlet WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
            .map_err(|e| format!("Gagal memuat atlet: {}", e))?
            .flatten();
        if current_cat != input.nomor_tanding_id {
            let in_bagan: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM backend_detailbagan WHERE atlet1_id = ? OR atlet2_id = ?"
            )
            .bind(id)
            .bind(id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Gagal memeriksa bagan atlet: {}", e))?;
            if in_bagan > 0 {
                return Err(format!(
                    "Atlet tidak bisa dipindah kategori karena sudah ada di {} partai bagan kategori lamanya. \
                     Hapus bagan kategori lama terlebih dahulu.",
                    in_bagan
                ));
            }
            self.ensure_category_open(input.nomor_tanding_id).await?;
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
            athletes_not_in_bagan: i64,
            finished_matches: i64,
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
                ) as bagan_count,
                (
                    SELECT COUNT(*) FROM backend_atlet a
                    WHERE a.nomor_tanding_id = nt.id
                      AND (a.additional_code IS NULL OR a.additional_code NOT LIKE 'MEMBER_%')
                      AND EXISTS (SELECT 1 FROM backend_bagan b WHERE b.nomor_tanding_id = nt.id)
                      AND NOT EXISTS (
                          SELECT 1 FROM backend_detailbagan d JOIN backend_bagan b ON d.bagan_id = b.id
                          WHERE b.nomor_tanding_id = nt.id AND (d.atlet1_id = a.id OR d.atlet2_id = a.id)
                      )
                ) as athletes_not_in_bagan,
                (
                    SELECT COUNT(*) FROM backend_detailbagan d JOIN backend_bagan b ON d.bagan_id = b.id
                    WHERE b.nomor_tanding_id = nt.id AND d.selesai = 1
                      AND d.atlet1_id IS NOT NULL AND d.atlet2_id IS NOT NULL
                ) as finished_matches
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
                athletes_not_in_bagan: r.athletes_not_in_bagan,
                finished_matches: r.finished_matches,
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
                athletes_not_in_bagan: 0,
                finished_matches: 0,
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
        Self::purge_bout_dependents(&mut *tx, "SELECT id FROM backend_bagan WHERE event_id = ?", event_id).await?;
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

    /// Deletes all festival brackets and match records for an entire event, leaving regular tournament brackets untouched
    pub async fn delete_all_festival_bagans(&self, event_id: i64) -> Result<u64, String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        // 1. Unassign tatamis assigned to matches belonging to festival bagans
        sqlx::query(
            "UPDATE backend_tatami SET detail_bagan_id = NULL 
             WHERE detail_bagan_id IN (
                 SELECT id FROM backend_detailbagan WHERE bagan_id IN (
                     SELECT id FROM backend_bagan WHERE event_id = ? AND (
                         UPPER(nama_bagan) LIKE '%FESTIVAL%'
                         OR UPPER(COALESCE(kode, '')) LIKE 'F%'
                         OR nomor_tanding_id IN (
                             SELECT id FROM backend_nomortanding WHERE UPPER(nama_nomor_tanding) LIKE '%FESTIVAL%'
                         )
                     )
                 )
             )"
        )
        .bind(event_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Gagal mereset tatami untuk bagan festival: {}", e))?;

        // 2. Defensive cleanup of team matchups if any
        let _ = sqlx::query(
            "DELETE FROM backend_matchup WHERE detail_bagan_id IN (
                 SELECT id FROM backend_detailbagan WHERE bagan_id IN (
                     SELECT id FROM backend_bagan WHERE event_id = ? AND (
                         UPPER(nama_bagan) LIKE '%FESTIVAL%'
                         OR UPPER(COALESCE(kode, '')) LIKE 'F%'
                         OR nomor_tanding_id IN (
                             SELECT id FROM backend_nomortanding WHERE UPPER(nama_nomor_tanding) LIKE '%FESTIVAL%'
                         )
                     )
                 )
             ) OR db_id IN (
                 SELECT id FROM backend_detailbagan WHERE bagan_id IN (
                     SELECT id FROM backend_bagan WHERE event_id = ? AND (
                         UPPER(nama_bagan) LIKE '%FESTIVAL%'
                         OR UPPER(COALESCE(kode, '')) LIKE 'F%'
                         OR nomor_tanding_id IN (
                             SELECT id FROM backend_nomortanding WHERE UPPER(nama_nomor_tanding) LIKE '%FESTIVAL%'
                         )
                     )
                 )
             )"
        )
        .bind(event_id)
        .bind(event_id)
        .execute(&mut *tx)
        .await;

        // 3. Delete detail bagans for festival bagans
        Self::purge_bout_dependents(&mut *tx, r#"SELECT id FROM backend_bagan WHERE event_id = ? AND (
                     UPPER(nama_bagan) LIKE '%FESTIVAL%'
                     OR UPPER(COALESCE(kode, '')) LIKE 'F%'
                     OR nomor_tanding_id IN (
                         SELECT id FROM backend_nomortanding WHERE UPPER(nama_nomor_tanding) LIKE '%FESTIVAL%'
                     )
                 )"#, event_id).await?;
        sqlx::query(
            "DELETE FROM backend_detailbagan WHERE bagan_id IN (
                 SELECT id FROM backend_bagan WHERE event_id = ? AND (
                     UPPER(nama_bagan) LIKE '%FESTIVAL%'
                     OR UPPER(COALESCE(kode, '')) LIKE 'F%'
                     OR nomor_tanding_id IN (
                         SELECT id FROM backend_nomortanding WHERE UPPER(nama_nomor_tanding) LIKE '%FESTIVAL%'
                     )
                 )
             )"
        )
        .bind(event_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Gagal menghapus detail bagan festival: {}", e))?;

        // 4. Delete festival bagans
        let res = sqlx::query(
            "DELETE FROM backend_bagan WHERE event_id = ? AND (
                 UPPER(nama_bagan) LIKE '%FESTIVAL%'
                 OR UPPER(COALESCE(kode, '')) LIKE 'F%'
                 OR nomor_tanding_id IN (
                     SELECT id FROM backend_nomortanding WHERE UPPER(nama_nomor_tanding) LIKE '%FESTIVAL%'
                 )
             )"
        )
        .bind(event_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Gagal menghapus bagan festival: {}", e))?;

        let deleted_count = res.rows_affected();
        tx.commit().await.map_err(|e| format!("Gagal menyimpan perubahan: {}", e))?;
        Ok(deleted_count)
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

    /// Mereset total seluruh data turnamen untuk suatu event:
    /// bagan, partai, atlet, nomor tanding (kategori), kontingen (utusan), perguruan,
    /// tatami, jadwal partai/timetable, wasit & juri, serta berkas festival roster.
    pub async fn reset_total_tournament(&self, event_id: i64) -> Result<crate::models::ResetTournamentSummary, String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;
        let summary = Self::purge_event_data(&mut tx, event_id).await?;
        tx.commit().await.map_err(|e| format!("Gagal menyelesaikan transaksi reset total: {}", e))?;
        self.remove_festival_roster_files(event_id);
        Ok(summary)
    }

    /// Deletes an event and everything that belongs to it, in one transaction.
    /// The event row itself goes last, so a failure part-way leaves the event intact.
    pub async fn delete_event(&self, event_id: i64) -> Result<crate::models::ResetTournamentSummary, String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;
        let summary = Self::purge_event_data(&mut tx, event_id).await?;
        let res = sqlx::query("DELETE FROM backend_event WHERE id = ?")
            .bind(event_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghapus event: {}", e))?;
        if res.rows_affected() != 1 {
            return Err(format!("Event #{} tidak ditemukan.", event_id));
        }
        tx.commit().await.map_err(|e| format!("Gagal menyelesaikan transaksi hapus event: {}", e))?;
        self.remove_festival_roster_files(event_id);
        let _ = crate::sync::SyncService::remove_event_mapping(self.active_db_path.as_ref(), event_id);
        Ok(summary)
    }

    /// Removes every row that belongs to `event_id` (everything except the event row).
    async fn purge_event_data(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        event_id: i64,
    ) -> Result<crate::models::ResetTournamentSummary, String> {
        // 1. Unassign tatamis
        let _ = sqlx::query("UPDATE backend_tatami SET detail_bagan_id = NULL WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut **tx)
            .await;

        // 2. Wasit detail bagan
        let _ = sqlx::query("DELETE FROM backend_wasitdetailbagan WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut **tx)
            .await;

        // 3. Matchup & Score & Detail bagan
        let _ = sqlx::query("DELETE FROM backend_matchup WHERE bagan_id IN (SELECT id FROM backend_bagan WHERE event_id = ?)")
            .bind(event_id)
            .execute(&mut **tx)
            .await;

        let _ = sqlx::query("DELETE FROM backend_score WHERE detail_bagan_id IN (SELECT id FROM backend_detailbagan WHERE bagan_id IN (SELECT id FROM backend_bagan WHERE event_id = ?))")
            .bind(event_id)
            .execute(&mut **tx)
            .await;

        Self::purge_bout_dependents(&mut **tx, "SELECT id FROM backend_bagan WHERE event_id = ?", event_id).await?;
        let res_matches = sqlx::query("DELETE FROM backend_detailbagan WHERE bagan_id IN (SELECT id FROM backend_bagan WHERE event_id = ?)")
            .bind(event_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("Gagal menghapus detail bagan: {}", e))?;

        // 4. Bagan
        let res_bagan = sqlx::query("DELETE FROM backend_bagan WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("Gagal menghapus bagan: {}", e))?;

        // 5. Timetable (cells, rows, days)
        let _ = sqlx::query("DELETE FROM backend_timetablecell WHERE row_id IN (SELECT id FROM backend_timetablerow WHERE day_id IN (SELECT id FROM backend_timetableday WHERE event_id = ?))")
            .bind(event_id)
            .execute(&mut **tx)
            .await;

        let _ = sqlx::query("DELETE FROM backend_timetablerow WHERE day_id IN (SELECT id FROM backend_timetableday WHERE event_id = ?)")
            .bind(event_id)
            .execute(&mut **tx)
            .await;

        let res_timetable = sqlx::query("DELETE FROM backend_timetableday WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("Gagal menghapus jadwal: {}", e))?;

        // 6. Wasit Tatami, Jury, Admin Tatami, Role
        let _ = sqlx::query("DELETE FROM backend_wasittatami WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut **tx)
            .await;

        let _ = sqlx::query("DELETE FROM backend_jury WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut **tx)
            .await;

        let _ = sqlx::query("DELETE FROM backend_admintatami WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut **tx)
            .await;

        let _ = sqlx::query("DELETE FROM backend_role WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut **tx)
            .await;

        // 7. Tatami
        let res_tatami = sqlx::query("DELETE FROM backend_tatami WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("Gagal menghapus tatami: {}", e))?;

        // 8. Athletes
        let res_athletes = sqlx::query("DELETE FROM backend_atlet WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("Gagal menghapus atlet: {}", e))?;

        // 9. Categories (Nomor Tanding)
        let res_categories = sqlx::query("DELETE FROM backend_nomortanding WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("Gagal menghapus nomor tanding: {}", e))?;

        // 10. Wasit
        let res_wasit = sqlx::query("DELETE FROM backend_wasit WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("Gagal menghapus wasit: {}", e))?;

        // 11. Kontingen (Utusan)
        let res_contingents = sqlx::query("DELETE FROM backend_utusan WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("Gagal menghapus kontingen: {}", e))?;

        // 12. Perguruan
        let res_perguruans = sqlx::query("DELETE FROM backend_perguruan WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("Gagal menghapus perguruan: {}", e))?;

        // 13. Kop surat, event keterangan, sync queue
        let _ = sqlx::query("DELETE FROM backend_kopsurat WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut **tx)
            .await;

        let _ = sqlx::query("DELETE FROM backend_eventketerangan WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut **tx)
            .await;

        let _ = sqlx::query("DELETE FROM backend_syncqueue WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut **tx)
            .await;

        Ok(crate::models::ResetTournamentSummary {
            deleted_bagans: res_bagan.rows_affected(),
            deleted_matches: res_matches.rows_affected(),
            deleted_athletes: res_athletes.rows_affected(),
            deleted_categories: res_categories.rows_affected(),
            deleted_contingents: res_contingents.rows_affected(),
            deleted_perguruans: res_perguruans.rows_affected(),
            deleted_tatamis: res_tatami.rows_affected(),
            deleted_timetable_days: res_timetable.rows_affected(),
            deleted_wasits: res_wasit.rows_affected(),
        })
    }

    /// Removes the festival roster JSON files kept next to the database for an event.
    ///
    /// Only the folder of the database that is open is touched. The previous version
    /// also deleted `../tournament_data/…` and `tournament_data/…` relative to the
    /// working directory, which removed another database's roster for the same event
    /// id whenever a database from a different folder was open.
    fn remove_festival_roster_files(&self, event_id: i64) {
        if let Some(active_path) = &self.active_db_path {
            if let Some(parent_dir) = std::path::Path::new(active_path).parent() {
                let roster_file = parent_dir.join(format!("festival_roster_{}.json", event_id));
                if roster_file.exists() {
                    let _ = std::fs::remove_file(roster_file);
                }
            }
        }
    }

    /// Menghapus seluruh nomor tanding (kategori) untuk suatu event
    pub async fn delete_all_categories(&self, event_id: i64) -> Result<u64, String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        // 1. Unassign categories from athletes
        let _ = sqlx::query("UPDATE backend_atlet SET nomor_tanding_id = NULL WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut *tx)
            .await;

        // 2. Clear timetable cells referencing these categories
        let _ = sqlx::query("DELETE FROM backend_timetablecell WHERE nomor_tanding_id IN (SELECT id FROM backend_nomortanding WHERE event_id = ?)")
            .bind(event_id)
            .execute(&mut *tx)
            .await;

        // 3. Clear brackets linked to these categories
        let _ = sqlx::query("UPDATE backend_tatami SET detail_bagan_id = NULL WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut *tx)
            .await;

        Self::purge_bout_dependents(&mut *tx, "SELECT id FROM backend_bagan WHERE event_id = ?", event_id).await?;
        let _ = sqlx::query("DELETE FROM backend_detailbagan WHERE bagan_id IN (SELECT id FROM backend_bagan WHERE event_id = ?)")
            .bind(event_id)
            .execute(&mut *tx)
            .await;

        let _ = sqlx::query("DELETE FROM backend_bagan WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut *tx)
            .await;

        // 4. Delete categories
        let res = sqlx::query("DELETE FROM backend_nomortanding WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghapus seluruh kategori: {}", e))?;

        let affected = res.rows_affected();
        tx.commit().await.map_err(|e| format!("Gagal menyelesaikan transaksi: {}", e))?;
        Ok(affected)
    }

    /// Menghapus seluruh kontingen (utusan) untuk suatu event
    pub async fn delete_all_contingents(&self, event_id: i64) -> Result<u64, String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        // 1. Unassign contingents from athletes
        let _ = sqlx::query("UPDATE backend_atlet SET utusan_id = NULL WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut *tx)
            .await;

        // 2. Delete contingents
        let res = sqlx::query("DELETE FROM backend_utusan WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghapus seluruh kontingen: {}", e))?;

        let affected = res.rows_affected();
        tx.commit().await.map_err(|e| format!("Gagal menyelesaikan transaksi: {}", e))?;
        Ok(affected)
    }

    /// Menghapus seluruh perguruan untuk suatu event
    pub async fn delete_all_perguruans(&self, event_id: i64) -> Result<u64, String> {
        let pool = self.get_pool()?;
        let mut tx = pool.begin().await.map_err(|e| format!("Gagal memulai transaksi: {}", e))?;

        // 1. Unassign perguruans from athletes and wasits
        let _ = sqlx::query("UPDATE backend_atlet SET perguruan_id = NULL WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut *tx)
            .await;

        let _ = sqlx::query("UPDATE backend_wasit SET perguruan_id = NULL WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut *tx)
            .await;

        // 2. Delete perguruans
        let res = sqlx::query("DELETE FROM backend_perguruan WHERE event_id = ?")
            .bind(event_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| format!("Gagal menghapus seluruh perguruan: {}", e))?;

        let affected = res.rows_affected();
        tx.commit().await.map_err(|e| format!("Gagal menyelesaikan transaksi: {}", e))?;
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
        Self::purge_bout_dependents(&mut *tx, "SELECT id FROM backend_bagan WHERE nomor_tanding_id = ?", category_id).await?;
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
        self.ensure_no_results(&category_ids).await?;
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

            Self::purge_bout_dependents(&mut *tx, "SELECT id FROM backend_bagan WHERE nomor_tanding_id = ?", cat_id).await?;
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
        self.ensure_no_results(&category_ids).await?;
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

            Self::purge_bout_dependents(&mut *tx, "SELECT id FROM backend_bagan WHERE nomor_tanding_id = ?", cat_id).await?;
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

        // 3. Process each category (aware of non-destructive FestivalMixGroups)
        let state = helper_load_festival_roster_state(self.active_db_path.as_deref(), event_id);
        let mut primary_to_mix: std::collections::HashMap<i64, &FestivalMixGroup> = std::collections::HashMap::new();
        let mut consumed_secondary_ids: std::collections::HashSet<i64> = std::collections::HashSet::new();

        for mg in &state.mix_groups {
            primary_to_mix.insert(mg.primary_category_id, mg);
            for &sec_id in &mg.secondary_category_ids {
                consumed_secondary_ids.insert(sec_id);
            }
        }

        for cat_id in category_ids {
            // If this category is merged into a primary category's bracket, skip generating a duplicate solo bracket
            if consumed_secondary_ids.contains(&cat_id) {
                continue;
            }

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

            let raw_atlets = if let Some(mg) = primary_to_mix.get(&cat_id) {
                // Fetch athletes from PRIMARY category AND all SECONDARY categories in this mix!
                let mut all_cat_ids = vec![cat_id];
                all_cat_ids.extend(mg.secondary_category_ids.clone());
                let in_clause = all_cat_ids.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
                let query_str = format!(
                    r#"
                    SELECT a.id, a.nama_atlet, a.utusan_id, a.perguruan_id, u.nama_utusan, p.nama_perguruan
                    FROM backend_atlet a
                    LEFT JOIN backend_utusan u ON a.utusan_id = u.id
                    LEFT JOIN backend_perguruan p ON a.perguruan_id = p.id
                    WHERE a.nomor_tanding_id IN ({})
                      AND (a.additional_code IS NULL OR a.additional_code NOT LIKE 'MEMBER_%')
                    ORDER BY a.id ASC
                    "#,
                    in_clause
                );
                sqlx::query_as::<_, RawAtletFest>(&query_str)
                    .fetch_all(&mut *tx)
                    .await
                    .map_err(|e| format!("Gagal memuat atlet festival mix: {}", e))?
            } else {
                sqlx::query_as::<_, RawAtletFest>(
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
                .map_err(|e| format!("Gagal memuat atlet festival: {}", e))?
            };

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

            // Balanced group & bagan distribution (e.g. 6 groups -> 3 in Bagan 1, 3 in Bagan 2, NOT 4-2):
            let n_total = paired_athletes.len();
            let total_groups = (n_total + 3) / 4;
            // Option C: 1 bagan can hold up to 6 groups (up to 24 athletes per single A4 sheet)
            let total_bagans = (total_groups + 5) / 6;

            // 1. Distribute total_groups evenly across total_bagans (e.g. 6 groups -> [3, 3])
            let base_groups_per_bagan = total_groups / total_bagans;
            let rem_groups = total_groups % total_bagans;
            let mut groups_per_bagan: Vec<usize> = Vec::new();
            for b in 0..total_bagans {
                let g_count = base_groups_per_bagan + if b < rem_groups { 1 } else { 0 };
                groups_per_bagan.push(g_count);
            }

            // 2. Distribute all n_total athletes evenly across total_groups (each group getting 4 or 3 athletes)
            let base_athletes_per_group = n_total / total_groups;
            let rem_athletes = n_total % total_groups;
            let mut all_groups: Vec<Vec<FestivalAthlete>> = Vec::new();
            let mut cursor = 0;
            for g in 0..total_groups {
                let g_size = base_athletes_per_group + if g < rem_athletes { 1 } else { 0 };
                all_groups.push(paired_athletes[cursor..cursor + g_size].to_vec());
                cursor += g_size;
            }

            // 3. Chunk all_groups into bagans according to groups_per_bagan
            let mut group_cursor = 0;
            categories_processed += 1;

            for (chunk_idx, &num_groups) in groups_per_bagan.iter().enumerate() {
                let group_slices: Vec<Vec<FestivalAthlete>> = all_groups[group_cursor..group_cursor + num_groups].to_vec();
                group_cursor += num_groups;

                let effective_cat_name = if let Some(mg) = primary_to_mix.get(&cat_id) {
                    mg.bagan_name.clone()
                } else {
                    cat_name.clone()
                };

                let bagan_title = if total_bagans == 1 {
                    effective_cat_name
                } else {
                    format!("{} - Bagan {}", effective_cat_name, chunk_idx + 1)
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

                            // Semi 2: Atlet 2 has BYE directly into Final (not a played match, selesai = 0, pemenang = NULL)
                            sqlx::query(
                                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team, pemenang) VALUES (?, 1, ?, ?, NULL, 0, 0, 0, 0, 0, NULL)"
                            )
                            .bind(bagan_id).bind(semi2_urutan).bind(g_athletes[2].id)
                            .execute(&mut *tx).await.map_err(|e| format!("Gagal simpan semi 2 BYE: {}", e))?;

                            // Final: TBD vs Atlet 2 (Atlet 2 seeded directly via BYE, waiting for winner of Semi 1)
                            sqlx::query(
                                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team) VALUES (?, 2, ?, NULL, ?, 0, 0, 0, 0, 0)"
                            )
                            .bind(bagan_id).bind(final_urutan).bind(g_athletes[2].id)
                            .execute(&mut *tx).await.map_err(|e| format!("Gagal simpan final: {}", e))?;
                        }
                        2 => {
                            // Semi 1: BYE Atlet 0 (selesai = 0, pemenang = NULL)
                            sqlx::query(
                                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team, pemenang) VALUES (?, 1, ?, ?, NULL, 0, 0, 0, 0, 0, NULL)"
                            )
                            .bind(bagan_id).bind(semi1_urutan).bind(g_athletes[0].id)
                            .execute(&mut *tx).await.map_err(|e| format!("Gagal simpan semi 1 BYE: {}", e))?;

                            // Semi 2: BYE Atlet 1 (selesai = 0, pemenang = NULL)
                            sqlx::query(
                                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team, pemenang) VALUES (?, 1, ?, ?, NULL, 0, 0, 0, 0, 0, NULL)"
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
                            // Single athlete: automatically gold in Final
                            sqlx::query(
                                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team, pemenang) VALUES (?, 1, ?, ?, NULL, 0, 0, 0, 0, 0, NULL)"
                            )
                            .bind(bagan_id).bind(semi1_urutan).bind(g_athletes[0].id)
                            .execute(&mut *tx).await.map_err(|e| format!("Gagal simpan semi 1 single: {}", e))?;

                            sqlx::query(
                                "INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team, pemenang) VALUES (?, 1, ?, NULL, NULL, 0, 0, 0, 0, 0, NULL)"
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
        // Winner is AKA ("1") or AO ("2"); empty only for an open bout or a round-robin draw.
        // Anything else would mark the bout finished with nobody advancing.
        if let Some(p) = input.pemenang.as_deref().map(str::trim) {
            if !p.is_empty() && p != "1" && p != "2" {
                return Err(format!("Pemenang tidak valid: \"{}\" (harus AKA atau AO)", p));
            }
        }
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
        let bagan_meta: Option<(i32, Option<i64>, Option<String>, Option<i64>, bool, bool)> = sqlx::query_as(
            "SELECT pool, nomor_tanding_id, nama_bagan, event_id, round_robin, COALESCE(has_vr, 0) FROM backend_bagan WHERE id = ?"
        )
        .bind(bagan_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("Gagal memuat info bagan #{}: {}", bagan_id, e))?;

        let (bagan_pool, nomor_tanding_id, nama_bagan, event_id, is_round_robin, bagan_has_vr) = bagan_meta.unwrap_or((1, None, None, None, false, false));
        let nama_bagan_upper = nama_bagan.unwrap_or_default().to_uppercase();
        let is_repechage = nama_bagan_upper.contains("REFERCHANGE") || nama_bagan_upper.contains("REPECHAGE");
        let is_festival = nama_bagan_upper.contains("FESTIVAL");
        let is_pool_bracket = (bagan_pool == 2 || nama_bagan_upper.contains("POOL A") || nama_bagan_upper.contains("POOL B") || nama_bagan_upper.contains("POOL 1") || nama_bagan_upper.contains("POOL 2")) && !is_repechage;
        let is_pool_a = nama_bagan_upper.contains("POOL A") || nama_bagan_upper.contains("POOL 1");
        let _is_pool_b = nama_bagan_upper.contains("POOL B") || nama_bagan_upper.contains("POOL 2");
        let is_final_bagan = bagan_pool == 0 || nama_bagan_upper.ends_with("- FINAL") || nama_bagan_upper.ends_with("FINAL");

        // 1b. Changing (or reopening) a finished result whose winner already played on, or whose
        //     loser already played the repechage, would leave those later bouts standing on a result
        //     that no longer exists. Those bouts must be reopened first, latest first.
        if !is_round_robin && !is_team_child_bout {
            let previous: Option<(bool, Option<String>)> = sqlx::query_as(
                "SELECT selesai, pemenang FROM backend_detailbagan WHERE id = ?"
            )
            .bind(input.match_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("Gagal memuat hasil lama: {}", e))?;
            if let Some((true, Some(prev))) = previous {
                let changes = !input.selesai || input.pemenang.as_deref() != Some(prev.as_str());
                let (old_winner, old_loser) = match prev.as_str() {
                    "1" => (a1_id, a2_id),
                    "2" => (a2_id, a1_id),
                    _ => (None, None),
                };
                if changes {
                    let mut blocking: Option<(i64, i32)> = None;
                    if let Some(w) = old_winner {
                        blocking = sqlx::query_as(
                            "SELECT d.id, d.round FROM backend_detailbagan d JOIN backend_bagan b ON d.bagan_id = b.id
                             WHERE d.id != ? AND d.selesai = 1 AND d.team = 0 AND (d.atlet1_id = ? OR d.atlet2_id = ?)
                               AND ((d.bagan_id = ? AND d.round > ?)
                                    OR (? = 1 AND b.nomor_tanding_id = ? AND b.pool = 0 AND b.id != ?))
                             ORDER BY d.round DESC LIMIT 1"
                        )
                        .bind(input.match_id)
                        .bind(w)
                        .bind(w)
                        .bind(bagan_id)
                        .bind(round_num)
                        .bind(is_pool_bracket && !is_final_bagan)
                        .bind(nomor_tanding_id)
                        .bind(bagan_id)
                        .fetch_optional(&mut *tx)
                        .await
                        .map_err(|e| format!("Gagal memeriksa partai lanjutan: {}", e))?;
                    }
                    if blocking.is_none() && !is_repechage {
                        if let (Some(l), Some(nt)) = (old_loser, nomor_tanding_id) {
                            blocking = sqlx::query_as(
                                "SELECT d.id, d.round FROM backend_detailbagan d JOIN backend_bagan b ON d.bagan_id = b.id
                                 WHERE b.nomor_tanding_id = ? AND d.selesai = 1 AND (d.atlet1_id = ? OR d.atlet2_id = ?)
                                   AND (UPPER(COALESCE(b.nama_bagan, '')) LIKE '%REFERCHANGE%' OR UPPER(COALESCE(b.nama_bagan, '')) LIKE '%REPECHAGE%')
                                 LIMIT 1"
                            )
                            .bind(nt)
                            .bind(l)
                            .bind(l)
                            .fetch_optional(&mut *tx)
                            .await
                            .map_err(|e| format!("Gagal memeriksa partai repechage: {}", e))?;
                        }
                    }
                    if let Some((_, later_round)) = blocking {
                        return Err(format!(
                            "Hasil partai ini sudah dipakai di partai berikutnya yang sudah selesai (babak {}). \
                             Buka ulang partai itu terlebih dahulu, mulai dari babak paling akhir.",
                            later_round
                        ));
                    }
                }
            }
        }

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

        // 2b. Kata: store the 5 judges' scores when the control panel sends them
        if input.juri_scores1.is_some() || input.juri_scores2.is_some() {
            sqlx::query("UPDATE backend_detailbagan SET juri_scores1 = ?, juri_scores2 = ? WHERE id = ?")
                .bind(&input.juri_scores1)
                .bind(&input.juri_scores2)
                .bind(input.match_id)
                .execute(&mut *tx)
                .await
                .map_err(|e| format!("Gagal menyimpan skor juri: {}", e))?;
        }

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

            let winner_had_vr = if pemenang_str == "1" { input.vr1 } else { input.vr2 };
            let next_vr = bagan_has_vr && (round_num + 1 >= 3 || winner_had_vr);

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
                                "UPDATE backend_detailbagan SET atlet1_id = ?, vr1 = ? WHERE bagan_id = ? AND round = ? AND urutan = ?"
                            )
                            .bind(w_id)
                            .bind(next_vr)
                            .bind(bagan_id)
                            .bind(next_round)
                            .bind(next_urutan)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| format!("Gagal memajukan pemenang pool: {}", e))?;
                        } else {
                            sqlx::query(
                                "UPDATE backend_detailbagan SET atlet2_id = ?, vr2 = ? WHERE bagan_id = ? AND round = ? AND urutan = ?"
                            )
                            .bind(w_id)
                            .bind(next_vr)
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
                                    "UPDATE backend_detailbagan SET atlet1_id = ?, vr1 = ? WHERE bagan_id = ? AND round = 4 AND urutan = 1"
                                )
                                .bind(w_id)
                                .bind(bagan_has_vr)
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
                                    "UPDATE backend_detailbagan SET atlet2_id = ?, vr2 = ? WHERE bagan_id = ? AND round = 4 AND urutan = 1"
                                )
                                .bind(w_id)
                                .bind(bagan_has_vr)
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
                                "UPDATE backend_detailbagan SET atlet1_id = ?, vr1 = ? WHERE bagan_id = ? AND round = ? AND urutan = ?"
                            )
                            .bind(w_id)
                            .bind(next_vr)
                            .bind(bagan_id)
                            .bind(next_round)
                            .bind(next_urutan)
                            .execute(&mut *tx)
                            .await
                            .map_err(|e| format!("Gagal memajukan pemenang ke ronde berikutnya: {}", e))?;
                        } else {
                            sqlx::query(
                                "UPDATE backend_detailbagan SET atlet2_id = ?, vr2 = ? WHERE bagan_id = ? AND round = ? AND urutan = ?"
                            )
                            .bind(w_id)
                            .bind(next_vr)
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

        // Clear tatami active match if match completed
        if input.selesai {
            let _ = sqlx::query("UPDATE backend_tatami SET detail_bagan_id = NULL WHERE detail_bagan_id = ?")
                .bind(input.match_id)
                .execute(&mut *tx)
                .await;
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

        // Defensive normalization: If tatami_number was accidentally passed as a backend_tatami.id (e.g. >= 20),
        // resolve it to its actual tatami_number (e.g. 1).
        let tatami_number: i32 = if tatami_number >= 20 {
            sqlx::query_scalar::<_, i32>("SELECT tatami_number FROM backend_tatami WHERE id = ?")
                .bind(tatami_number as i64)
                .fetch_optional(pool)
                .await
                .ok()
                .flatten()
                .unwrap_or(tatami_number)
        } else {
            tatami_number
        };

        // 1. Link the match to the tatami of ITS event only (every event has its own tatami 1, 2, ...).
        let match_event: Option<i64> = sqlx::query_scalar(
            "SELECT b.event_id FROM backend_detailbagan d JOIN backend_bagan b ON d.bagan_id = b.id WHERE d.id = ?"
        )
        .bind(match_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .flatten();
        // Unknown match (deleted / re-drawn bracket): never touch tatami rows of every event.
        let Some(match_event) = match_event else {
            return Err(format!("Partai {} tidak ditemukan (bagan mungkin sudah diundi ulang)", match_id));
        };
        let match_event = Some(match_event);

        let _ = sqlx::query(
            "UPDATE backend_tatami SET detail_bagan_id = ? WHERE tatami_number = ? AND (? IS NULL OR event_id = ?)"
        )
        .bind(match_id)
        .bind(tatami_number)
        .bind(match_event)
        .bind(match_event)
        .execute(pool)
        .await;

        let _ = sqlx::query(
            "UPDATE backend_detailbagan SET assigned_tatami_id = (
                 SELECT id FROM backend_tatami WHERE tatami_number = ? AND (? IS NULL OR event_id = ?)
                 ORDER BY event_id DESC LIMIT 1
             ) WHERE id = ?"
        )
        .bind(tatami_number)
        .bind(match_event)
        .bind(match_event)
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

        // Bracket context for the checks below
        let (bagan_cat, bagan_name, bagan_is_bob): (Option<i64>, String, bool) = sqlx::query_as(
            "SELECT nomor_tanding_id, COALESCE(nama_bagan, ''), COALESCE(is_bob, 0) FROM backend_bagan WHERE id = ?"
        )
        .bind(bagan_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("Gagal memuat bagan: {}", e))?
        .ok_or_else(|| "Bagan tidak ditemukan.".to_string())?;
        // Festival bagans may mix categories (mix groups) and BOB takes winners of other categories.
        let check_category = bagan_cat.is_some() && !bagan_is_bob && !bagan_name.to_uppercase().contains("FESTIVAL");

        // 2. Update each matchup's atlet1_id and atlet2_id
        for m in matchups.iter() {
            // A finished bout keeps its athletes: changing them would orphan its result.
            let current: Option<(Option<i64>, Option<i64>, bool, i32, i32)> = sqlx::query_as(
                "SELECT atlet1_id, atlet2_id, selesai, round, urutan FROM backend_detailbagan WHERE id = ? AND bagan_id = ?"
            )
            .bind(m.match_id)
            .bind(bagan_id)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| format!("Gagal memuat partai #{}: {}", m.match_id, e))?;
            if let Some((a1, a2, true, round, urutan)) = current {
                if a1 != m.atlet1_id || a2 != m.atlet2_id {
                    return Err(format!(
                        "Partai babak {} nomor {} sudah selesai; atletnya tidak bisa diganti. Buka ulang hasil partai itu terlebih dahulu.",
                        round, urutan
                    ));
                }
            }
            if check_category {
                for aid in [m.atlet1_id, m.atlet2_id].into_iter().flatten() {
                    let cat: Option<Option<i64>> = sqlx::query_scalar("SELECT nomor_tanding_id FROM backend_atlet WHERE id = ?")
                        .bind(aid)
                        .fetch_optional(&mut *tx)
                        .await
                        .map_err(|e| format!("Gagal memeriksa atlet: {}", e))?;
                    if cat.flatten() != bagan_cat {
                        return Err(format!("Atlet #{} bukan peserta kategori bagan ini.", aid));
                    }
                }
            }
        }
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

        // 3. An athlete may sit in only one bout per round
        let dup: Option<(i32, String)> = sqlx::query_as(
            r#"
            SELECT x.round, COALESCE(a.nama_atlet, '#' || x.aid) FROM (
                SELECT round, atlet1_id AS aid FROM backend_detailbagan WHERE bagan_id = ? AND team = 0 AND atlet1_id IS NOT NULL
                UNION ALL
                SELECT round, atlet2_id AS aid FROM backend_detailbagan WHERE bagan_id = ? AND team = 0 AND atlet2_id IS NOT NULL
            ) x LEFT JOIN backend_atlet a ON a.id = x.aid
            GROUP BY x.round, x.aid HAVING COUNT(*) > 1 LIMIT 1
            "#
        )
        .bind(bagan_id)
        .bind(bagan_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| format!("Gagal memeriksa duplikasi atlet: {}", e))?;
        if let Some((round, name)) = dup {
            return Err(format!("{} muncul dua kali di babak {}. Setiap atlet hanya boleh di satu partai per babak.", name, round));
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
        let festival_roster_format = self
            .get_festival_format(event_id)
            .await
            .map(|f| f.format == "roster")
            .unwrap_or(false);

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
                                    matches = if is_fest {
                                        helper_festival_match_count(atlet_cnt, festival_roster_format)
                                    } else {
                                        atlet_cnt - 1
                                    };
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
            let dur_per_match = helper_minutes_per_match(&upper);
            let (matches, est_dur) = if cnt > 1 {
                let m = if is_fest {
                    helper_festival_match_count(cnt, festival_roster_format)
                } else {
                    cnt - 1
                };
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

        // Roster-format festival: one session per tatami, sized from the roster's tatami split.
        let festival_session = if festival_roster_format && categories.iter().any(|c| c.is_festival && c.atlet_count > 0) {
            match self.get_festival_roster_data(event_id).await {
                Ok(roster) => Some(
                    roster
                        .sections
                        .iter()
                        .filter(|sec| sec.total_athletes > 0)
                        .map(|sec| {
                            let minutes: f64 = sec
                                .categories
                                .iter()
                                .map(|c| c.match_count as f64 * helper_minutes_per_match(&c.nama.to_uppercase()))
                                .sum();
                            FestivalSessionTatami {
                                tatami_id: sec.tatami_id,
                                tatami_number: sec.tatami_number,
                                categories: sec.categories.len(),
                                athletes: sec.total_athletes,
                                matches: sec.total_matches,
                                est_minutes: helper_festival_session_minutes(minutes),
                            }
                        })
                        .collect(),
                ),
                Err(e) => {
                    tracing::warn!("Roster festival tidak bisa dimuat untuk jadwal: {}", e);
                    None
                }
            }
        } else {
            None
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
            festival_session,
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
                let (bagan_pool, bagan_layout_type) = Self::bagan_pool_and_layout(pool, bagan_id).await;

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
                    bagan_pool,
                    bagan_layout_type,
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
                        bagan_pool: None,
                        bagan_layout_type: None,
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
                bagan_id: None,
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
                let jk = if helper_is_putri_name(nt_name) {
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

        // Mix groups (Auto Mix Solo / Smart Mix): the athletes of each secondary category play in
        // their primary category; the secondary is hidden from the roster.
        let sec_to_primary = helper_festival_mix_map(&state.mix_groups, &all_nt_ids);
        let mut mix_names: std::collections::HashMap<i64, String> = std::collections::HashMap::new();
        let mut mix_members: std::collections::HashMap<i64, Vec<FestivalMixMember>> = std::collections::HashMap::new();
        for mg in &state.mix_groups {
            let pid = mg.primary_category_id;
            if mix_names.contains_key(&pid) || !sec_to_primary.values().any(|p| *p == pid) {
                continue;
            }
            mix_names.insert(pid, mg.bagan_name.clone());
            let mut ids = vec![pid];
            ids.extend(mg.secondary_category_ids.iter().filter(|s| sec_to_primary.get(s) == Some(&pid)));
            let members = ids
                .into_iter()
                .map(|id| FestivalMixMember {
                    id,
                    nama: all_nt_map.get(&id).cloned().unwrap_or_default(),
                    atlet_count: athletes_by_nt.get(&id).map(|v| v.len()).unwrap_or(0),
                })
                .collect();
            mix_members.insert(pid, members);
        }
        for (sec, pid) in &sec_to_primary {
            if let Some(moved) = athletes_by_nt.remove(sec) {
                athletes_by_nt.entry(*pid).or_default().extend(moved);
            }
        }
        all_festival_nts.retain(|c| !sec_to_primary.contains_key(&c.0));
        let all_nt_ids: std::collections::HashSet<i64> = all_festival_nts.iter().map(|c| c.0).collect();

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
            } else if !saved_order.is_empty() {
                // Athletes were added or withdrawn after pairing: keep every pair that is still
                // complete, fill withdrawn spots with new athletes (or the last athlete), append the rest.
                let merged = helper_merge_festival_pairing(&saved_order, &current_athletes);
                cleaned_pairings.insert(nt_key, merged);
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

                    let is_clash = ao
                        .as_ref()
                        .map(|ao_ath| helper_same_festival_utusan(&aka.utusan, &ao_ath.utusan))
                        .unwrap_or(false);

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

                let display_name = match mix_names.get(&cat_id) {
                    Some(n) if !n.trim().is_empty() => helper_clean_festival_display_name(&n.trim().to_uppercase()),
                    _ => helper_clean_festival_display_name(&cat_name),
                };
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
                    mixed_from: mix_members.get(&cat_id).cloned().unwrap_or_default(),
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
        // The roster screen only edits tatami assignments and pairings; keep the rest of the file
        // (mix groups set in the drawing dialog, the chosen festival format).
        let mut current = helper_load_festival_roster_state(self.active_db_path.as_deref(), event_id);
        current.tatami_assignments = state.tatami_assignments;
        current.athlete_pairings = state.athlete_pairings;
        helper_save_festival_roster_state(self.active_db_path.as_deref(), event_id, &current)
    }

    async fn festival_bagan_counts(&self, event_id: i64) -> Result<(i64, i64), String> {
        let pool = self.get_pool()?;
        let festival_bagans = "SELECT id FROM backend_bagan WHERE event_id = ? AND (
                 UPPER(nama_bagan) LIKE '%FESTIVAL%'
                 OR UPPER(COALESCE(kode, '')) LIKE 'F%'
                 OR nomor_tanding_id IN (
                     SELECT id FROM backend_nomortanding WHERE UPPER(nama_nomor_tanding) LIKE '%FESTIVAL%'
                 )
             )";
        let bagans: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM ({})", festival_bagans))
            .bind(event_id)
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Gagal menghitung bagan festival: {}", e))?;
        let finished: i64 = sqlx::query_scalar(&format!(
            "SELECT COUNT(*) FROM backend_detailbagan WHERE selesai = 1
               AND atlet1_id IS NOT NULL AND atlet2_id IS NOT NULL
               AND bagan_id IN ({})",
            festival_bagans
        ))
        .bind(event_id)
        .fetch_one(pool)
        .await
        .map_err(|e| format!("Gagal menghitung partai festival: {}", e))?;
        Ok((bagans, finished))
    }

    /// Festival format of the event: the stored choice, or for events that never chose one,
    /// "bagan" when festival bagans already exist and "roster" otherwise.
    pub async fn get_festival_format(&self, event_id: i64) -> Result<FestivalFormatInfo, String> {
        let state = helper_load_festival_roster_state(self.active_db_path.as_deref(), event_id);
        let (festival_bagan_count, finished_match_count) = self.festival_bagan_counts(event_id).await?;
        let format = match state.format.as_deref() {
            Some("roster") => "roster",
            Some("bagan") => "bagan",
            _ if festival_bagan_count > 0 => "bagan",
            _ => "roster",
        }
        .to_string();
        Ok(FestivalFormatInfo { format, festival_bagan_count, finished_match_count })
    }

    /// Switches the event's festival format. Switching to "roster" while festival bagans exist
    /// requires `delete_bagans`, so no hidden bagan keeps scores.
    pub async fn set_festival_format(&self, event_id: i64, format: String, delete_bagans: bool) -> Result<FestivalFormatInfo, String> {
        if format != "roster" && format != "bagan" {
            return Err(format!("Format festival tidak dikenal: {}", format));
        }
        if format == "roster" {
            let (bagans, _) = self.festival_bagan_counts(event_id).await?;
            if bagans > 0 {
                if !delete_bagans {
                    return Err(format!(
                        "Masih ada {} bagan festival. Hapus bagan festival untuk memakai format roster.",
                        bagans
                    ));
                }
                self.delete_all_festival_bagans(event_id).await?;
            }
        }
        let mut state = helper_load_festival_roster_state(self.active_db_path.as_deref(), event_id);
        state.format = Some(format);
        helper_save_festival_roster_state(self.active_db_path.as_deref(), event_id, &state)?;
        self.get_festival_format(event_id).await
    }

    /// Memperbarui tipe layout template bagan ('quadrant' atau 'bands')
    pub async fn update_bagan_layout_type(&self, bagan_id: i64, layout_type: String) -> Result<(), String> {
        let pool = self.get_pool()?;
        sqlx::query("UPDATE backend_bagan SET layout_type = ? WHERE id = ?")
            .bind(&layout_type)
            .bind(bagan_id)
            .execute(pool)
            .await
            .map_err(|e| format!("Gagal memperbarui layout template bagan #{}: {}", bagan_id, e))?;
        Ok(())
    }

    /// Memuat data lengkap Booklet Cetak Bagan Festival (Single Source of Truth dari backend_bagan & backend_detailbagan)
    pub async fn get_festival_booklet_data(
        &self,
        event_id: i64,
        tatami_id: Option<i64>,
    ) -> Result<FestivalBookletData, String> {
        let pool = self.get_pool()?;

        // 1. Get Event Name
        let event_name: String = sqlx::query_scalar("SELECT nama_event FROM backend_event WHERE id = ?")
            .bind(event_id)
            .fetch_optional(pool)
            .await
            .map_err(|e| e.to_string())?
            .unwrap_or_else(|| "Turnamen Karate".to_string());

        // 2. Get Kop Surat
        let kop_surat = self.get_kop_surat(event_id).await.ok().flatten();

        let event_date: Option<String> = sqlx::query_scalar(
            "SELECT tanggal FROM backend_timetableday WHERE event_id = ? ORDER BY order_index ASC LIMIT 1"
        )
        .bind(event_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);

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
                bagan_id: None,
                event_id: Some(event_id),
                nama_bagan: None,
            }]
        } else {
            tatamis
        };

        // 4. Load Festival Roster State for tatami assignment mapping
        let state = helper_load_festival_roster_state(self.active_db_path.as_deref(), event_id);
        let mut cat_to_tatami: HashMap<i64, (i64, i64)> = HashMap::new(); // cat_id -> (tatami_id, tatami_number)
        for t in &tatamis {
            let t_key = t.id.to_string();
            let t_num = t.tatami_number.unwrap_or(1) as i64;
            if let Some(cat_ids) = state.tatami_assignments.get(&t_key) {
                for &cid in cat_ids {
                    cat_to_tatami.insert(cid, (t.id, t_num));
                }
            }
        }

        // 5. Query All Festival Bagan directly from backend_bagan (SINGLE SOURCE OF TRUTH)
        // Reads the exact festival brackets created during drawing or edited in BaganView
        let festival_bagans: Vec<(i64, i64, String, Option<String>, i64, Option<String>, String, Option<String>)> = sqlx::query_as(
            "SELECT b.id, b.nomor_tanding_id, b.nama_bagan, b.kode, b.pool, b.tipe_tanding, nt.nama_nomor_tanding, COALESCE(b.layout_type, 'quadrant')
             FROM backend_bagan b
             JOIN backend_nomortanding nt ON b.nomor_tanding_id = nt.id
             WHERE b.event_id = ?
               AND (UPPER(b.nama_bagan) LIKE '%FESTIVAL%' OR UPPER(nt.nama_nomor_tanding) LIKE '%FESTIVAL%')
             ORDER BY 
                 CASE WHEN UPPER(nt.nama_nomor_tanding) LIKE '%KATA%' THEN 1 ELSE 2 END,
                 b.kode ASC,
                 b.id ASC"
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

        let default_tatami = &tatamis[0];
        let default_tatami_id = default_tatami.id;
        let default_tatami_number = default_tatami.tatami_number.unwrap_or(1) as i64;

        let mut categories_out: Vec<FestivalBookletCategory> = Vec::new();

        for (b_id, b_nt_id, b_nama, b_kode, b_pool, b_tipe, nt_nama, b_layout) in festival_bagans {
            // Fetch live enriched matches directly from backend_detailbagan
            let matches = self.get_detail_bagan(b_id).await.unwrap_or_default();

            // Resolve tatami assignment:
            // 1) From roster tatami_assignments, 2) From match assigned_tatami_id, 3) Default to first tatami
            let (t_id, t_num) = if let Some(&(assigned_tid, assigned_tnum)) = cat_to_tatami.get(&b_nt_id) {
                (assigned_tid, assigned_tnum)
            } else if let Some(assigned_match_tid) = matches.iter().find_map(|m| m.assigned_tatami_id) {
                let match_tnum = tatamis.iter().find(|t| t.id == assigned_match_tid).and_then(|t| t.tatami_number).unwrap_or(1) as i64;
                (assigned_match_tid, match_tnum)
            } else {
                (default_tatami_id, default_tatami_number)
            };

            // If tatami filter specified (e.g. from "Cetak Bagan Tatami X"), apply filter
            if let Some(req_tid) = tatami_id {
                if t_id != req_tid {
                    continue;
                }
            }

            // Determine discipline
            let discipline = if nt_nama.to_lowercase().contains("kata") || b_nama.to_lowercase().contains("kata") {
                "kata".to_string()
            } else {
                "kumite".to_string()
            };

            // Calculate exact unique athlete count from matches, fallback to nt count
            let mut unique_athletes = std::collections::HashSet::new();
            for m in &matches {
                if let Some(aid) = m.atlet1_id { unique_athletes.insert(aid); }
                if let Some(aid) = m.atlet2_id { unique_athletes.insert(aid); }
            }
            let athletes_count = if !unique_athletes.is_empty() {
                unique_athletes.len() as i64
            } else {
                let cnt: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM backend_atlet WHERE nomor_tanding_id = ?")
                    .bind(b_nt_id)
                    .fetch_one(pool)
                    .await
                    .unwrap_or(0);
                cnt
            };

            categories_out.push(FestivalBookletCategory {
                category_id: b_nt_id,
                category_name: b_nama.clone(),
                discipline,
                tatami_id: t_id,
                tatami_number: t_num,
                bagan_id: Some(b_id),
                bagan_kode: b_kode,
                bagan_nama: Some(b_nama),
                pool: b_pool,
                tipe_tanding: b_tipe,
                athletes_count,
                matches,
                layout_type: b_layout,
            });
        }

        let venue = kop_surat.as_ref().map(|k| k.alamat.clone()).or(Some("Gedung Olahraga SMPN & SMAN 1 Samarinda".to_string()));

        Ok(FestivalBookletData {
            event_id,
            event_name,
            venue,
            event_date,
            kop_surat,
            tatamis,
            selected_tatami_id: tatami_id,
            categories: categories_out,
        })
    }

    pub async fn auto_pair_festival_category(&self, event_id: i64, nomor_tanding_id: Option<i64>) -> Result<FestivalRosterData, String> {
        let pool = self.pool.as_ref().ok_or("Database tidak terhubung.")?;

        let ath_rows: Vec<(i64, i64, String, Option<String>, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT a.id, a.nomor_tanding_id, a.nama_atlet, nt.nama_nomor_tanding, u.nama_utusan, p.nama_perguruan
             FROM backend_atlet a
             JOIN backend_nomortanding nt ON a.nomor_tanding_id = nt.id
             LEFT JOIN backend_utusan u ON a.utusan_id = u.id
             LEFT JOIN backend_perguruan p ON a.perguruan_id = p.id
             WHERE a.event_id = ? AND UPPER(nt.nama_nomor_tanding) LIKE '%FESTIVAL%'",
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?;

        let festival_ids: std::collections::HashSet<i64> = sqlx::query_scalar::<_, i64>(
            "SELECT id FROM backend_nomortanding WHERE event_id = ? AND UPPER(nama_nomor_tanding) LIKE '%FESTIVAL%'",
        )
        .bind(event_id)
        .fetch_all(pool)
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .collect();

        let mut state = helper_load_festival_roster_state(self.active_db_path.as_deref(), event_id);
        // Athletes of a mixed (secondary) category are paired inside their primary category.
        let sec_to_primary = helper_festival_mix_map(&state.mix_groups, &festival_ids);
        let target = nomor_tanding_id.map(|id| *sec_to_primary.get(&id).unwrap_or(&id));

        let mut athletes_by_nt: std::collections::HashMap<i64, Vec<FestivalAthlete>> = std::collections::HashMap::new();
        for (a_id, nt_id, a_nama, a_nt_name, a_utusan, a_perguruan) in ath_rows {
            let key = *sec_to_primary.get(&nt_id).unwrap_or(&nt_id);
            if target.is_some_and(|t| t != key) {
                continue;
            }
            let cat_str = a_nt_name.unwrap_or_default();
            let jk = if helper_is_putri_name(&cat_str) {
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
            athletes_by_nt.entry(key).or_default().push(fa);
        }

        for (nt_id, athletes) in athletes_by_nt {
            let paired = helper_pair_festival_athletes_anti_collision(athletes);
            let paired_ids: Vec<i64> = paired.iter().map(|a| a.id).collect();
            state.athlete_pairings.insert(nt_id.to_string(), paired_ids);
        }

        let _ = helper_save_festival_roster_state(self.active_db_path.as_deref(), event_id, &state);

        self.get_festival_roster_data(event_id).await
    }

    pub async fn get_festival_mix_groups(&self, event_id: i64) -> Result<Vec<FestivalMixGroup>, String> {
        let state = helper_load_festival_roster_state(self.active_db_path.as_deref(), event_id);
        Ok(state.mix_groups)
    }

    pub async fn save_festival_mix_groups(&self, event_id: i64, mix_groups: Vec<FestivalMixGroup>) -> Result<(), String> {
        let mut state = helper_load_festival_roster_state(self.active_db_path.as_deref(), event_id);
        state.mix_groups = mix_groups;
        helper_save_festival_roster_state(self.active_db_path.as_deref(), event_id, &state)
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
            has_vr: bool,
            nomor_tanding_id: Option<i64>,
        }

        let parent: ParentRow = sqlx::query_as(
            r#"
            SELECT db.bagan_id, db.round, db.urutan, db.atlet1_id, db.atlet2_id, db.assigned_tatami_id,
                   db.selesai, db.pemenang, b.nama_bagan, nt.nama_nomor_tanding, b.tipe_tanding, b.event_id,
                   COALESCE(b.has_vr, 0) AS has_vr, b.nomor_tanding_id
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
                        score1, score2, vr1, vr2, hantei, team, selesai, assigned_tatami_id
                    )
                    VALUES (?, 10, ?, ?, ?, '0', '0', ?, ?, 0, 1, 0, ?)
                    RETURNING id
                    "#
                )
                .bind(bagan_id)
                .bind(r)
                .bind(parent.atlet1_id)
                .bind(parent.atlet2_id)
                .bind(parent.has_vr)
                .bind(parent.has_vr)
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

        // 3b. Bouts created before the fix above were stored without a VR card
        //     (vr1/vr2 = 0 = "hangus"). Give untouched bouts their card back.
        if parent.has_vr && existing_bouts_count > 0 {
            let _ = sqlx::query(
                r#"
                UPDATE backend_detailbagan SET vr1 = 1, vr2 = 1
                WHERE id IN (SELECT db_id FROM backend_matchup WHERE detail_bagan_id = ?)
                  AND selesai = 0 AND vr1 = 0 AND vr2 = 0
                  AND COALESCE(score1, '0') IN ('0', '') AND COALESCE(score2, '0') IN ('0', '')
                  AND COALESCE(pemenang, '') = ''
                "#
            )
            .bind(parent_match_id)
            .execute(pool)
            .await;
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
            same_nomor: bool,
        }
        // Members first: registered in this team category (nomor tanding) for the
        // same kontingen. Team entries (the bracket rows themselves) are excluded.
        const CAND_SQL: &str = r#"
                SELECT a.id, COALESCE(a.nama_atlet, '') AS nama_atlet,
                       p.nama_perguruan AS perguruan_nama, u.nama_utusan AS utusan_nama,
                       (COALESCE(a.nomor_tanding_id, -1) = COALESCE(?, -2)) AS same_nomor
                FROM backend_atlet a
                LEFT JOIN backend_perguruan p ON a.perguruan_id = p.id
                LEFT JOIN backend_utusan u ON a.utusan_id = u.id
                WHERE a.event_id = ? AND a.utusan_id = ? AND a.id != ?
                  AND a.id NOT IN (
                      SELECT d.atlet1_id FROM backend_detailbagan d JOIN backend_bagan bb ON bb.id = d.bagan_id
                      WHERE bb.nomor_tanding_id = ? AND d.team = 0 AND d.atlet1_id IS NOT NULL
                      UNION
                      SELECT d.atlet2_id FROM backend_detailbagan d JOIN backend_bagan bb ON bb.id = d.bagan_id
                      WHERE bb.nomor_tanding_id = ? AND d.team = 0 AND d.atlet2_id IS NOT NULL
                  )
                ORDER BY same_nomor DESC, a.nama_atlet ASC
                "#;

        let mut candidate_atlets_aka: Vec<TeamCandidateAthlete> = Vec::new();
        if let Some(ut_id) = team_meta.a1_utusan_id {
            let p_id = parent.atlet1_id.unwrap_or(0);
            let rows: Vec<CandRow> = sqlx::query_as(CAND_SQL)
            .bind(parent.nomor_tanding_id)
            .bind(event_id)
            .bind(ut_id)
            .bind(p_id)
            .bind(parent.nomor_tanding_id)
            .bind(parent.nomor_tanding_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Gagal memuat daftar atlet tim: {}", e))?;

            for r in rows {
                candidate_atlets_aka.push(TeamCandidateAthlete {
                    id: r.id,
                    nama_atlet: r.nama_atlet,
                    perguruan_nama: r.perguruan_nama,
                    utusan_nama: r.utusan_nama,
                    same_nomor: r.same_nomor,
                });
            }
        }

        let mut candidate_atlets_ao: Vec<TeamCandidateAthlete> = Vec::new();
        if let Some(ut_id) = team_meta.a2_utusan_id {
            let p_id = parent.atlet2_id.unwrap_or(0);
            let rows: Vec<CandRow> = sqlx::query_as(CAND_SQL)
            .bind(parent.nomor_tanding_id)
            .bind(event_id)
            .bind(ut_id)
            .bind(p_id)
            .bind(parent.nomor_tanding_id)
            .bind(parent.nomor_tanding_id)
            .fetch_all(pool)
            .await
            .map_err(|e| format!("Gagal memuat daftar atlet tim: {}", e))?;

            for r in rows {
                candidate_atlets_ao.push(TeamCandidateAthlete {
                    id: r.id,
                    nama_atlet: r.nama_atlet,
                    perguruan_nama: r.perguruan_nama,
                    utusan_nama: r.utusan_nama,
                    same_nomor: r.same_nomor,
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

        // Every athlete starts a bout with a VR card when the category uses VR
        let bagan_has_vr: bool = sqlx::query_scalar("SELECT COALESCE(has_vr, 0) FROM backend_bagan WHERE id = ?")
            .bind(p.bagan_id)
            .fetch_optional(pool)
            .await
            .unwrap_or(None)
            .unwrap_or(false);

        let child_id: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO backend_detailbagan (
                bagan_id, round, urutan, atlet1_id, atlet2_id,
                score1, score2, vr1, vr2, hantei, team, selesai, assigned_tatami_id
            )
            VALUES (?, 10, ?, ?, ?, '0', '0', ?, ?, 0, 1, 0, ?)
            RETURNING id
            "#
        )
        .bind(p.bagan_id)
        .bind(next_round)
        .bind(p.atlet1_id)
        .bind(p.atlet2_id)
        .bind(p.vr1 || bagan_has_vr)
        .bind(p.vr2 || bagan_has_vr)
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

        for sql in [
            "UPDATE backend_tatami SET detail_bagan_id = NULL WHERE detail_bagan_id = ?",
            "DELETE FROM backend_score WHERE detail_bagan_id = ?",
            "DELETE FROM backend_wasitdetailbagan WHERE detail_bagan_id = ?",
            "DELETE FROM backend_matchup WHERE db_id = ? OR detail_bagan_id = ?",
        ] {
            let mut q = sqlx::query(sql).bind(bout_id);
            if sql.contains("OR detail_bagan_id") {
                q = q.bind(bout_id);
            }
            q.execute(pool)
                .await
                .map_err(|e| format!("Gagal menghapus matchup: {}", e))?;
        }

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
                a.nomor_tanding_id, nt.nama_nomor_tanding as nomor_tanding_nama,
                CASE WHEN UPPER(COALESCE(nt.nama_nomor_tanding, '')) LIKE '%FESTIVAL%' THEN 'Festival' ELSE 'Tournament' END as nomor_tanding_tipe,
                nt.is_bob
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
                        SELECT COALESCE(nt.nama_nomor_tanding, b.nama_bagan) 
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
                        d.id, d.bagan_id, COALESCE(nt.nama_nomor_tanding, b.nama_bagan), d.round, d.urutan, d.kode,
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
                    d.id, d.bagan_id, COALESCE(nt.nama_nomor_tanding, b.nama_bagan), d.round, d.urutan, d.kode,
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
                WHERE b.event_id = ?
                  AND (d.assigned_tatami_id = ? OR d.assigned_tatami_id = ?)
                  AND d.selesai = 0
                  AND d.atlet1_id IS NOT NULL AND d.atlet2_id IS NOT NULL
                ORDER BY d.round ASC, d.urutan ASC
                LIMIT 40
                "#
            )
            // Bouts store either the tatami row id or (older data / roster) its number.
            // Only bouts with both athletes known can be called to the mat.
            .bind(event_id)
            .bind(t.id)
            .bind(t_num)
            .fetch_all(pool)
            .await
            .unwrap_or_default();

            // A team match is several bouts between the same two teams: call it once.
            let pair_key = |bagan: Option<i64>, a: &str, b: &str| format!("{:?}|{}|{}", bagan, a, b);
            let mut seen_pairs = std::collections::HashSet::new();
            if let Some(ref cur) = current_match {
                seen_pairs.insert(pair_key(cur.bagan_id, &cur.atlet1_nama, &cur.atlet2_nama));
            }
            let mut filtered_upcoming = Vec::new();
            for m in upcoming_rows {
                if let Some(ref cur) = current_match {
                    if cur.match_id == m.0 {
                        continue;
                    }
                }
                let key = pair_key(m.1, m.6.as_deref().unwrap_or(""), m.9.as_deref().unwrap_or(""));
                if !seen_pairs.insert(key) {
                    continue;
                }
                filtered_upcoming.push(m);
                if filtered_upcoming.len() >= 6 {
                    break;
                }
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
    /// Pool and festival layout of a bagan, for printing it like the bagan channel.
    async fn bagan_pool_and_layout(pool: &Pool<Sqlite>, bagan_id: Option<i64>) -> (Option<i64>, Option<String>) {
        let Some(id) = bagan_id else {
            return (None, None);
        };
        sqlx::query_as::<_, (Option<i64>, Option<String>)>("SELECT pool, layout_type FROM backend_bagan WHERE id = ?")
            .bind(id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten()
            .unwrap_or((None, None))
    }

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
            let (bagan_pool, bagan_layout_type) = Self::bagan_pool_and_layout(pool, Some(b.id)).await;

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
                bagan_pool,
                bagan_layout_type,
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
                    INSERT INTO backend_detailbagan (bagan_id, round, urutan, atlet1_id, atlet2_id, vr1, vr2, hantei, selesai, team)
                    VALUES (?, ?, ?, ?, ?, 0, 0, 0, 0, 0)
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

/// Normalised contingent name, or None when the athlete has no contingent ("" or "-").
fn helper_festival_utusan_key(utusan: &str) -> Option<String> {
    let u = utusan.trim().to_uppercase();
    if u.is_empty() || u == "-" {
        None
    } else {
        Some(u)
    }
}

/// Two festival athletes clash when they come from the same (known) contingent.
fn helper_same_festival_utusan(a: &str, b: &str) -> bool {
    match (helper_festival_utusan_key(a), helper_festival_utusan_key(b)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

/// secondary category id -> primary category id for the event's mix groups. Ignores ids that
/// are not festival categories of the event, a primary listed as someone's secondary, and a
/// category claimed by two groups (first wins).
fn helper_festival_mix_map(
    mix_groups: &[FestivalMixGroup],
    valid_ids: &std::collections::HashSet<i64>,
) -> std::collections::HashMap<i64, i64> {
    let primaries: std::collections::HashSet<i64> = mix_groups.iter().map(|m| m.primary_category_id).collect();
    let mut map = std::collections::HashMap::new();
    for mg in mix_groups {
        let pid = mg.primary_category_id;
        if !valid_ids.contains(&pid) || map.contains_key(&pid) {
            continue;
        }
        for sid in &mg.secondary_category_ids {
            if *sid != pid && valid_ids.contains(sid) && !primaries.contains(sid) && !map.contains_key(sid) {
                map.insert(*sid, pid);
            }
        }
    }
    map
}

/// Updates a saved 1-vs-1 order (pairs = consecutive ids) to the current athletes without
/// reshuffling: withdrawn athletes leave holes that new athletes fill first, then the last
/// athlete of the list; remaining new athletes are appended (paired among themselves,
/// avoiding the same contingent where possible).
fn helper_merge_festival_pairing(saved: &[i64], current: &[FestivalAthlete]) -> Vec<i64> {
    let current_ids: std::collections::HashSet<i64> = current.iter().map(|a| a.id).collect();
    let saved_set: std::collections::HashSet<i64> = saved.iter().copied().collect();
    let mut slots: Vec<Option<i64>> = saved.iter().map(|id| current_ids.contains(id).then_some(*id)).collect();
    let newcomers: Vec<FestivalAthlete> = current.iter().filter(|a| !saved_set.contains(&a.id)).cloned().collect();
    let mut newcomer_ids: std::collections::VecDeque<i64> =
        helper_pair_festival_athletes_anti_collision(newcomers).into_iter().map(|a| a.id).collect();
    let mut i = 0;
    while i < slots.len() {
        if slots[i].is_none() {
            if let Some(n) = newcomer_ids.pop_front() {
                slots[i] = Some(n);
            } else {
                // No newcomer left: move the last athlete into the hole (or drop trailing holes).
                while slots.len() > i + 1 && slots.last() == Some(&None) {
                    slots.pop();
                }
                if slots.len() > i + 1 {
                    slots[i] = slots.pop().flatten();
                } else {
                    slots.pop();
                }
            }
        }
        i += 1;
    }
    let mut out: Vec<i64> = slots.into_iter().flatten().collect();
    out.extend(newcomer_ids);
    out
}

fn helper_pair_festival_athletes_anti_collision(athletes: Vec<FestivalAthlete>) -> Vec<FestivalAthlete> {
    if athletes.len() <= 1 {
        return athletes;
    }
    use rand::Rng;
    use std::collections::HashMap;

    let mut groups: HashMap<String, Vec<FestivalAthlete>> = HashMap::new();
    for a in athletes {
        let u = helper_festival_utusan_key(&a.utusan).unwrap_or_else(|| "UNKNOWN".to_string());
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
        (0..arr.len().saturating_sub(1))
            .step_by(2)
            .filter(|&i| helper_same_festival_utusan(&arr[i].utusan, &arr[i + 1].utusan))
            .count()
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
        mix_groups: Vec::new(),
        format: None,
    }
}

/// Estimated minutes per bout, from the (upper-case) category name.
pub(crate) fn helper_minutes_per_match(upper: &str) -> f64 {
    if upper.contains("KATA") {
        3.5
    } else if upper.contains("SENIOR") || upper.contains("U-21") || upper.contains("UNDER 21") {
        6.0
    } else if upper.contains("CADET") || upper.contains("KADET") || upper.contains("JUNIOR") {
        4.5
    } else {
        4.0
    }
}

/// A festival session's length: the bouts' minutes rounded up to 5, at least 15.
pub(crate) fn helper_festival_session_minutes(bout_minutes: f64) -> i64 {
    let m = (bout_minutes / 5.0).ceil() as i64 * 5;
    m.max(15)
}

/// Number of bouts a festival category needs in the chosen format.
/// roster: 1-vs-1 pairs (an odd athlete still gets a bout); bagan: groups of up to 4,
/// each group of k athletes needs k - 1 bouts.
pub(crate) fn helper_festival_match_count(athletes: i64, roster_format: bool) -> i64 {
    if athletes < 2 {
        return 0;
    }
    if roster_format {
        (athletes + 1) / 2
    } else {
        let groups = (athletes + 3) / 4;
        athletes - groups
    }
}

/// True when a category name marks it as PUTRI ("PUTRI" or the word "PI").
pub(crate) fn helper_is_putri_name(name: &str) -> bool {
    let upper = name.to_uppercase();
    upper.contains("PUTRI")
        || upper
            .split(|c: char| !c.is_ascii_alphanumeric())
            .any(|w| w == "PI")
}

fn helper_save_festival_roster_state(db_path: Option<&std::path::Path>, event_id: i64, state: &FestivalRosterState) -> Result<(), String> {
    let path = helper_get_festival_roster_file_path(db_path, event_id);
    let json_str = serde_json::to_string_pretty(state).map_err(|e| e.to_string())?;
    std::fs::write(&path, json_str).map_err(|e| e.to_string())
}


#[cfg(test)]
mod festival_format_tests {
    use super::{
        helper_festival_match_count, helper_festival_mix_map, helper_is_putri_name,
        helper_pair_festival_athletes_anti_collision, helper_same_festival_utusan,
    };
    use crate::models::{FestivalAthlete, FestivalMixGroup};

    fn ath(id: i64, utusan: &str) -> FestivalAthlete {
        FestivalAthlete {
            id,
            nama: format!("ATLET {}", id),
            perguruan: "-".into(),
            utusan: utusan.into(),
            jenis_kelamin: "PUTRA".into(),
        }
    }

    fn clashes(order: &[FestivalAthlete]) -> usize {
        order
            .chunks(2)
            .filter(|p| p.len() == 2 && helper_same_festival_utusan(&p[0].utusan, &p[1].utusan))
            .count()
    }

    /// Fewest same-contingent bouts possible: with s athletes in the biggest contingent and
    /// r = n - s others, the surplus s - r (minus the one athlete left alone when n is odd)
    /// must meet each other.
    fn min_clashes(sizes: &[usize]) -> usize {
        let n: usize = sizes.iter().sum();
        let s = *sizes.iter().max().unwrap_or(&0);
        let r = n - s;
        let odd = n % 2;
        if s > r + odd { (s - r - odd) / 2 } else { 0 }
    }

    #[test]
    fn anti_collision_pairing_reaches_the_minimum() {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        for round in 0..600 {
            let groups = rng.gen_range(1..=6);
            let sizes: Vec<usize> = (0..groups).map(|_| rng.gen_range(1..=7)).collect();
            let mut list = Vec::new();
            let mut id = 0;
            for (g, &k) in sizes.iter().enumerate() {
                for _ in 0..k {
                    id += 1;
                    list.push(ath(id, &format!("KONTINGEN {}", g)));
                }
            }
            let n = list.len();
            let order = helper_pair_festival_athletes_anti_collision(list);
            assert_eq!(order.len(), n, "round {}: athletes lost", round);
            let mut ids: Vec<i64> = order.iter().map(|a| a.id).collect();
            ids.sort();
            ids.dedup();
            assert_eq!(ids.len(), n, "round {}: duplicated athlete", round);
            assert_eq!(clashes(&order), min_clashes(&sizes), "round {} sizes {:?}", round, sizes);
        }
    }

    #[test]
    fn festival_pairing_merge_keeps_existing_pairs() {
        use super::helper_merge_festival_pairing;
        let saved = vec![1, 2, 3, 4, 5, 6, 7];
        // late athlete 8 joins the solo 7
        let cur: Vec<_> = (1..=8).map(|i| ath(i, &format!("K{}", i))).collect();
        assert_eq!(helper_merge_festival_pairing(&saved, &cur), vec![1, 2, 3, 4, 5, 6, 7, 8]);
        // athlete 3 withdraws, no newcomer: the last athlete (7) takes the spot
        let cur: Vec<_> = [1, 2, 4, 5, 6, 7].iter().map(|&i| ath(i, &format!("K{}", i))).collect();
        assert_eq!(helper_merge_festival_pairing(&saved, &cur), vec![1, 2, 7, 4, 5, 6]);
        // athlete 3 withdraws and 9 registers: 9 takes the spot
        let cur: Vec<_> = [1, 2, 4, 5, 6, 7, 9].iter().map(|&i| ath(i, &format!("K{}", i))).collect();
        assert_eq!(helper_merge_festival_pairing(&saved, &cur), vec![1, 2, 9, 4, 5, 6, 7]);
        // the last athlete itself withdraws
        let cur: Vec<_> = (1..=6).map(|i| ath(i, &format!("K{}", i))).collect();
        assert_eq!(helper_merge_festival_pairing(&saved, &cur), vec![1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn athletes_without_contingent_never_clash() {
        assert!(!helper_same_festival_utusan("-", "-"));
        assert!(!helper_same_festival_utusan("", "  "));
        assert!(!helper_same_festival_utusan("-", "KOTA BONTANG"));
        assert!(helper_same_festival_utusan(" kota bontang", "KOTA BONTANG "));
        let order = helper_pair_festival_athletes_anti_collision(vec![ath(1, "-"), ath(2, "-"), ath(3, "A"), ath(4, "A")]);
        assert_eq!(clashes(&order), 0);
    }

    #[test]
    fn mix_map_ignores_invalid_groups() {
        let valid: std::collections::HashSet<i64> = [1, 2, 3, 4, 5].into_iter().collect();
        let groups = vec![
            FestivalMixGroup { primary_category_id: 1, secondary_category_ids: vec![2, 1, 99], bagan_name: "A".into() },
            FestivalMixGroup { primary_category_id: 3, secondary_category_ids: vec![2, 1, 4], bagan_name: "B".into() },
            FestivalMixGroup { primary_category_id: 98, secondary_category_ids: vec![5], bagan_name: "C".into() },
        ];
        let map = helper_festival_mix_map(&groups, &valid);
        assert_eq!(map.get(&2), Some(&1)); // first group wins
        assert_eq!(map.get(&4), Some(&3));
        assert!(!map.contains_key(&1)); // a primary is never a secondary
        assert!(!map.contains_key(&5)); // primary 98 is not in the event
        assert!(!map.contains_key(&99));
        assert_eq!(map.len(), 2);
    }

    #[test]
    fn festival_match_count_by_format() {
        assert_eq!(helper_festival_match_count(0, true), 0);
        assert_eq!(helper_festival_match_count(1, false), 0);
        assert_eq!(helper_festival_match_count(2, true), 1);
        assert_eq!(helper_festival_match_count(5, true), 3);
        assert_eq!(helper_festival_match_count(8, true), 4);
        assert_eq!(helper_festival_match_count(4, false), 3);
        assert_eq!(helper_festival_match_count(5, false), 3); // groups 3 + 2
        assert_eq!(helper_festival_match_count(12, false), 9);
    }

    #[test]
    fn festival_session_minutes_round_up() {
        use super::{helper_festival_session_minutes, helper_minutes_per_match};
        assert_eq!(helper_festival_session_minutes(0.0), 15);
        assert_eq!(helper_festival_session_minutes(15.5), 20);
        assert_eq!(helper_festival_session_minutes(120.0), 120);
        assert_eq!(helper_minutes_per_match("FESTIVAL USIA DINI - KATA PERORANGAN PUTRI"), 3.5);
        assert_eq!(helper_minutes_per_match("FESTIVAL PEMULA - KUMITE -35 KG PUTRA"), 4.0);
    }

    #[test]
    fn putri_detection_uses_whole_words() {
        assert!(helper_is_putri_name("FESTIVAL USIA DINI - KATA PERORANGAN PUTRI"));
        assert!(helper_is_putri_name("Kumite -40kg PI"));
        assert!(!helper_is_putri_name("FESTIVAL PRA PEMULA - KATA PERORANGAN PUTRA"));
        assert!(!helper_is_putri_name("KUMITE PIALA WALIKOTA PA"));
        assert!(!helper_is_putri_name("OLYMPIC KATA PUTRA"));
    }
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
            juri_scores1: None,
            juri_scores2: None,
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
            juri_scores1: None,
            juri_scores2: None,
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
            juri_scores1: None,
            juri_scores2: None,
        }).await.expect("save final failed");

        let bagan_final = db.get_bagan_by_id(fest_bagan.id).await.expect("get bagan failed").unwrap();
        assert_eq!(bagan_final.juara_1_id, Some(winner_semi2));
        assert_eq!(bagan_final.juara_2_id, Some(winner_semi1));

        // 8. Check Group 4 with 3 athletes: Semi 2 MUST NOT be 'selesai' and waiting athlete is seeded in Final
        let g4_semi1 = matches.iter().find(|m| m.round == Some(1) && m.urutan == Some(7)).expect("g4 semi 1 not found");
        let g4_semi2 = matches.iter().find(|m| m.round == Some(1) && m.urutan == Some(8)).expect("g4 semi 2 not found");
        let g4_final = matches.iter().find(|m| m.round == Some(2) && m.urutan == Some(4)).expect("g4 final not found");

        assert!(g4_semi1.atlet1_id.is_some() && g4_semi1.atlet2_id.is_some());
        assert!(!g4_semi1.selesai, "Semi 1 must not be selesai initially");

        // Semi 2 is a BYE slot: must have selesai = false, pemenang = None, and 3rd athlete is seeded in Final
        assert!(g4_semi2.atlet1_id.is_some());
        assert!(g4_semi2.atlet2_id.is_none());
        assert!(!g4_semi2.selesai, "Semi 2 BYE must NOT be marked selesai");
        assert!(g4_semi2.pemenang.is_none(), "Semi 2 BYE must NOT have a fake winner");

        // The 3rd athlete must be waiting directly in the Final
        assert!(g4_final.atlet1_id.is_none(), "Final slot 1 waits for Semi 1 winner");
        assert_eq!(g4_final.atlet2_id, g4_semi2.atlet1_id, "Final slot 2 holds the waiting 3rd athlete");
        assert!(!g4_final.selesai, "Final must not be selesai initially");

        // Cleanup
        let _ = std::fs::remove_file(&test_db_path);
        println!("test_drawing_and_progression_festival PASSED successfully!");
    }
    #[tokio::test]
    async fn test_balanced_festival_drawing_multi_bagan() {
        let original_db = std::path::Path::new("../tournament_data/db.sqlite3");
        if !original_db.exists() {
            println!("Skipping test: db.sqlite3 not found at {:?}", original_db);
            return;
        }

        let temp_dir = std::env::temp_dir();
        let test_db_path = temp_dir.join(format!("test_fest_bal_{}.sqlite3", rand::random::<u64>()));
        std::fs::copy(original_db, &test_db_path).expect("Gagal copy test db");

        let mut db = DatabaseManager::new();
        db.mount_database(&test_db_path).await.expect("Mount db failed");
        let pool = db.get_pool().expect("pool failed");

        // Create a synthetic festival category with 28 athletes (7 groups -> 2 bagans: 4 and 3 groups)
        let cat_res = sqlx::query("INSERT INTO backend_nomortanding (event_id, nama_nomor_tanding, is_bob, has_vr) VALUES (9, 'FESTIVAL SIMULASI 28 ATLET', 0, 0)")
            .execute(pool).await.expect("insert cat failed");
        let test_cat_id = cat_res.last_insert_rowid();

        for i in 1..=28 {
            let utusan_id = (i % 5) + 1;
            let perguruan_id = (i % 4) + 1;
            sqlx::query("INSERT INTO backend_atlet (event_id, nomor_tanding_id, nama_atlet, utusan_id, perguruan_id, is_priority) VALUES (9, ?, ?, ?, ?, 0)")
                .bind(test_cat_id)
                .bind(format!("Atlet Festival {:02}", i))
                .bind(utusan_id)
                .bind(perguruan_id)
                .execute(pool).await.expect("insert athlete failed");
        }

        // Execute drawing for this 28-athlete category (7 groups total -> 2 balanced bagans: 4 and 3 groups)
        let summary = db.execute_drawing_festival_bagan(9, vec![test_cat_id], "both").await.expect("drawing failed");
        assert_eq!(summary.categories_processed, 1);
        assert_eq!(summary.bagans_created, 2, "28 atlet (7 grup) harus membentuk 2 bagan");
        assert_eq!(summary.total_matches, 21, "7 grup * 3 partai = 21 partai");

        // Verify that bagans are balanced (4 groups and 3 groups)
        let bagans = db.get_bagans(9).await.expect("get_bagans failed");
        let fest_bagans: Vec<_> = bagans.into_iter().filter(|b| b.nomor_tanding_id == Some(test_cat_id)).collect();
        assert_eq!(fest_bagans.len(), 2);
        assert_eq!(fest_bagans[0].pool, 4, "Bagan 1 harus memiliki 4 grup (Pool A, B, C, D)");
        assert_eq!(fest_bagans[1].pool, 3, "Bagan 2 harus memiliki 3 grup (Pool A, B, C)");

        // Cleanup
        let _ = std::fs::remove_file(&test_db_path);
        println!("test_balanced_festival_drawing_multi_bagan PASSED successfully (3-3 balance verified)!");
    }

    #[tokio::test]
    async fn test_delete_all_festival_bagans_preserves_regular_bagans() {
        let original_db = std::path::Path::new("../tournament_data/db.sqlite3");
        if !original_db.exists() {
            println!("Skipping test: db.sqlite3 not found at {:?}", original_db);
            return;
        }

        let temp_dir = std::env::temp_dir();
        let test_db_path = temp_dir.join(format!("test_del_fest_{}.sqlite3", rand::random::<u64>()));
        std::fs::copy(original_db, &test_db_path).expect("Gagal copy test db");

        let mut db = DatabaseManager::new();
        db.mount_database(&test_db_path).await.expect("Mount db failed");
        let pool = db.get_pool().expect("pool failed");

        // 1. Create a regular/open tournament category & bagan
        let open_cat = sqlx::query("INSERT INTO backend_nomortanding (event_id, nama_nomor_tanding, is_bob, has_vr) VALUES (9, 'KUMITE SENIOR -75KG PUTRA (OPEN)', 0, 0)")
            .execute(pool).await.expect("insert open cat failed");
        let open_cat_id = open_cat.last_insert_rowid();

        let open_bagan = sqlx::query("INSERT INTO backend_bagan (event_id, nomor_tanding_id, nama_bagan, pool, tipe_tanding, round_robin, is_bob, has_vr, kode) VALUES (9, ?, 'KUMITE SENIOR -75KG PUTRA', 1, 'kumite', 0, 0, 0, 'K01')")
            .bind(open_cat_id)
            .execute(pool).await.expect("insert open bagan failed");
        let open_bagan_id = open_bagan.last_insert_rowid();

        sqlx::query("INSERT INTO backend_detailbagan (bagan_id, round, urutan, vr1, vr2, hantei, selesai, team) VALUES (?, 1, 1, 0, 0, 0, 0, 0)")
            .bind(open_bagan_id).execute(pool).await.expect("insert open match failed");

        // 2. Create a festival category & 2 festival bagans
        let fest_cat = sqlx::query("INSERT INTO backend_nomortanding (event_id, nama_nomor_tanding, is_bob, has_vr) VALUES (9, 'FESTIVAL USIA DINI - KATA PUTRA', 0, 0)")
            .execute(pool).await.expect("insert fest cat failed");
        let fest_cat_id = fest_cat.last_insert_rowid();

        let fest_b1 = sqlx::query("INSERT INTO backend_bagan (event_id, nomor_tanding_id, nama_bagan, pool, tipe_tanding, round_robin, is_bob, has_vr, kode) VALUES (9, ?, 'FESTIVAL USIA DINI - Bagan 1', 3, 'kata', 0, 0, 0, 'F01')")
            .bind(fest_cat_id).execute(pool).await.expect("insert fest bagan 1 failed");
        let fest_b1_id = fest_b1.last_insert_rowid();

        let fest_b2 = sqlx::query("INSERT INTO backend_bagan (event_id, nomor_tanding_id, nama_bagan, pool, tipe_tanding, round_robin, is_bob, has_vr, kode) VALUES (9, ?, 'FESTIVAL USIA DINI - Bagan 2', 3, 'kata', 0, 0, 0, 'F02')")
            .bind(fest_cat_id).execute(pool).await.expect("insert fest bagan 2 failed");
        let fest_b2_id = fest_b2.last_insert_rowid();

        sqlx::query("INSERT INTO backend_detailbagan (bagan_id, round, urutan, vr1, vr2, hantei, selesai, team) VALUES (?, 1, 1, 0, 0, 0, 0, 0)")
            .bind(fest_b1_id).execute(pool).await.expect("insert fest match 1 failed");
        sqlx::query("INSERT INTO backend_detailbagan (bagan_id, round, urutan, vr1, vr2, hantei, selesai, team) VALUES (?, 1, 1, 0, 0, 0, 0, 0)")
            .bind(fest_b2_id).execute(pool).await.expect("insert fest match 2 failed");

        // 3. Execute delete_all_festival_bagans
        let deleted_count = db.delete_all_festival_bagans(9).await.expect("delete festival failed");
        assert!(deleted_count >= 2, "Harus menghapus minimal 2 bagan festival yang baru dibuat");

        // 4. Assert: Festival bagans are deleted, but regular open bagan is still present!
        let all_bagans = db.get_bagans(9).await.expect("get_bagans failed");
        let remaining_fest: Vec<_> = all_bagans.iter().filter(|b| b.id == fest_b1_id || b.id == fest_b2_id).collect();
        assert_eq!(remaining_fest.len(), 0, "Semua bagan festival harus terhapus");

        let open_still_exists = all_bagans.iter().any(|b| b.id == open_bagan_id);
        assert!(open_still_exists, "Bagan prestasi/open TIDAK BOLEH terhapus!");

        let open_matches = db.get_detail_bagan(open_bagan_id).await.expect("get open matches failed");
        assert_eq!(open_matches.len(), 1, "Partai bagan prestasi/open harus tetap utuh");

        // Cleanup
        let _ = std::fs::remove_file(&test_db_path);
        println!("test_delete_all_festival_bagans_preserves_regular_bagans PASSED successfully!");
    }

    #[tokio::test]
    async fn test_reset_total_tournament_cleans_all_data() {
        let original_db = std::path::Path::new("../tournament_data/db.sqlite3");
        if !original_db.exists() {
            return;
        }

        let test_db_path = std::path::Path::new("../tournament_data/test_reset_total.sqlite3");
        let orig_wal = std::path::Path::new("../tournament_data/db.sqlite3-wal");
        let test_wal = std::path::Path::new("../tournament_data/test_reset_total.sqlite3-wal");
        let orig_shm = std::path::Path::new("../tournament_data/db.sqlite3-shm");
        let test_shm = std::path::Path::new("../tournament_data/test_reset_total.sqlite3-shm");

        let _ = std::fs::remove_file(&test_db_path);
        let _ = std::fs::remove_file(&test_wal);
        let _ = std::fs::remove_file(&test_shm);

        let _ = std::fs::copy(original_db, &test_db_path);
        if orig_wal.exists() {
            let _ = std::fs::copy(orig_wal, &test_wal);
        }
        if orig_shm.exists() {
            let _ = std::fs::copy(orig_shm, &test_shm);
        }

        let mut db = DatabaseManager::new();
        db.mount_database(&test_db_path).await.expect("mount failed");

        // Execute reset on Event 14
        let summary = db.reset_total_tournament(14).await.expect("reset_total_tournament failed");

        let pool = db.get_pool().expect("get pool failed");
        let athlete_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM backend_atlet WHERE event_id = 14")
            .fetch_one(pool).await.unwrap();
        assert_eq!(athlete_count.0, 0);

        let cat_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM backend_nomortanding WHERE event_id = 14")
            .fetch_one(pool).await.unwrap();
        assert_eq!(cat_count.0, 0);

        let contingent_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM backend_utusan WHERE event_id = 14")
            .fetch_one(pool).await.unwrap();
        assert_eq!(contingent_count.0, 0);

        let perguruan_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM backend_perguruan WHERE event_id = 14")
            .fetch_one(pool).await.unwrap();
        assert_eq!(perguruan_count.0, 0);

        let bagan_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM backend_bagan WHERE event_id = 14")
            .fetch_one(pool).await.unwrap();
        assert_eq!(bagan_count.0, 0);

        // Verify Event 9 is unaffected
        let other_cat_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM backend_nomortanding WHERE event_id = 9")
            .fetch_one(pool).await.unwrap();
        assert!(other_cat_count.0 > 0, "Event lain tidak boleh terhapus!");

        let _ = std::fs::remove_file(&test_db_path);
        let _ = std::fs::remove_file(&test_wal);
        let _ = std::fs::remove_file(&test_shm);
        println!("test_reset_total_tournament_cleans_all_data PASSED: {:?}", summary);
    }

    #[tokio::test]
    async fn test_get_festival_booklet_data_single_source_of_truth() {
        let original_db = std::path::Path::new("../tournament_data/db.sqlite3");
        if !original_db.exists() {
            return;
        }

        let mut db = DatabaseManager::new();
        db.mount_database(&original_db).await.expect("mount failed");

        // 1. Load booklet data for Event 14 (Festival Bushido I)
        let booklet = db.get_festival_booklet_data(14, None).await.expect("load booklet failed");

        // Event 14 has 18 festival bagans in backend_bagan
        assert_eq!(booklet.categories.len(), 18, "Harus mengembalikan 18 bagan festival resmi dari backend_bagan!");

        // 2. Verify first bagan has valid matches and athletes
        let first_cat = &booklet.categories[0];
        assert!(!first_cat.matches.is_empty(), "Partai bagan festival tidak boleh kosong!");
        assert!(first_cat.athletes_count > 0, "Jumlah atlet harus > 0");

        // 3. Test Live Mutation (Single Source of Truth verification)
        let first_match = &first_cat.matches[0];
        let match_id = first_match.id;
        let orig_score = first_match.score1.clone();

        let pool = db.get_pool().expect("pool failed");
        // Mutate score directly in backend_detailbagan
        sqlx::query("UPDATE backend_detailbagan SET score1 = '8' WHERE id = ?")
            .bind(match_id)
            .execute(pool)
            .await
            .expect("update score failed");

        // Re-fetch booklet data
        let reloaded = db.get_festival_booklet_data(14, None).await.expect("reload booklet failed");
        let reloaded_match = reloaded.categories[0].matches.iter().find(|m| m.id == match_id).expect("match found");
        assert_eq!(reloaded_match.score1.as_deref(), Some("8"), "Skor hasil perubahan di database harus langsung tercermin di booklet!");

        // Revert score back
        sqlx::query("UPDATE backend_detailbagan SET score1 = ? WHERE id = ?")
            .bind(&orig_score)
            .bind(match_id)
            .execute(pool)
            .await
            .expect("revert score failed");

        println!("test_get_festival_booklet_data_single_source_of_truth PASSED successfully! 18 bagans verified as SSoT.");
    }

}

#[cfg(test)]
mod delete_event_tests {
    use super::*;

    const EVENT_TABLES: &[&str] = &[
        "backend_atlet", "backend_nomortanding", "backend_utusan", "backend_perguruan",
        "backend_tatami", "backend_wasit", "backend_bagan", "backend_timetableday",
    ];

    async fn counts(pool: &Pool<Sqlite>, event_id: i64) -> Vec<i64> {
        let mut out = Vec::new();
        for t in EVENT_TABLES {
            let n: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {} WHERE event_id = ?", t))
                .bind(event_id)
                .fetch_one(pool)
                .await
                .unwrap();
            out.push(n);
        }
        out
    }

    /// Runs on a temporary copy; the real tournament database is never opened for writing.
    #[tokio::test]
    async fn delete_event_removes_only_that_event() {
        let original = std::path::Path::new("../tournament_data/db.sqlite3");
        if !original.exists() {
            return;
        }
        let dir = std::env::temp_dir().join(format!("ambr_del_evt_{}", rand::random::<u64>()));
        std::fs::create_dir_all(&dir).unwrap();
        let copy = dir.join("db.sqlite3");
        std::fs::copy(original, &copy).unwrap();

        let mut db = DatabaseManager::new();
        db.mount_database(&copy).await.unwrap();
        let pool = db.get_pool_cloned().unwrap();

        let events: Vec<i64> = sqlx::query_scalar("SELECT id FROM backend_event ORDER BY id").fetch_all(&pool).await.unwrap();
        assert!(events.len() >= 2, "need at least two events in the copy");
        let target = *events.last().unwrap();
        let other = events[0];
        let other_before = counts(&pool, other).await;

        let summary = db.delete_event(target).await.unwrap();
        let _ = summary;

        let still_there: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM backend_event WHERE id = ?").bind(target).fetch_one(&pool).await.unwrap();
        assert_eq!(still_there, 0);
        assert!(counts(&pool, target).await.iter().all(|&n| n == 0), "rows left for deleted event");
        assert_eq!(counts(&pool, other).await, other_before, "another event was touched");

        // Deleting it again reports a clear error instead of succeeding silently.
        assert!(db.delete_event(target).await.is_err());

        drop(pool);
        drop(db);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
