use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Event {
    pub id: i64,
    pub nama_event: Option<String>,
    pub kode_event: Option<String>,
    pub is_live_sync_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct NomorTanding {
    pub id: i64,
    pub event_id: Option<i64>,
    pub nama_nomor_tanding: Option<String>,
    pub is_bob: bool,
    pub has_vr: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Perguruan {
    pub id: i64,
    pub event_id: Option<i64>,
    pub nama_perguruan: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Utusan {
    pub id: i64,
    pub event_id: Option<i64>,
    pub nama_utusan: Option<String>,
    pub logo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Atlet {
    pub id: i64,
    pub event_id: Option<i64>,
    pub nama_atlet: Option<String>,
    pub nik: Option<String>,
    pub kode_atlet: Option<String>,
    pub additional_code: Option<String>,
    pub is_priority: bool,
    pub perguruan_id: Option<i64>,
    pub utusan_id: Option<i64>,
    pub nomor_tanding_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EnrichedAtlet {
    pub id: i64,
    pub event_id: Option<i64>,
    pub nama_atlet: Option<String>,
    pub nik: Option<String>,
    pub kode_atlet: Option<String>,
    pub additional_code: Option<String>,
    pub is_priority: bool,
    pub perguruan_id: Option<i64>,
    pub perguruan_nama: Option<String>,
    pub utusan_id: Option<i64>,
    pub utusan_nama: Option<String>,
    pub utusan_logo: Option<String>,
    pub nomor_tanding_id: Option<i64>,
    pub nomor_tanding_nama: Option<String>,
    pub nomor_tanding_tipe: Option<String>,
    pub is_bob: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AthleteMutationInput {
    pub id: Option<i64>,
    pub event_id: i64,
    pub nama_atlet: String,
    pub nik: Option<String>,
    pub kode_atlet: Option<String>,
    pub additional_code: Option<String>,
    pub is_priority: bool,
    pub perguruan_id: Option<i64>,
    pub utusan_id: Option<i64>,
    pub nomor_tanding_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Bagan {
    pub id: i64,
    pub event_id: Option<i64>,
    pub nama_bagan: Option<String>,
    pub nomor_tanding_id: Option<i64>,
    pub tipe_tanding: Option<String>, // '1' = Kata, '2' = Kumite
    pub juara_1_id: Option<i64>,
    pub juara_2_id: Option<i64>,
    pub juara_3a_id: Option<i64>,
    pub juara_3b_id: Option<i64>,
    pub peringkat_5_id: Option<i64>,
    pub peringkat_6_id: Option<i64>,
    pub peringkat_7_id: Option<i64>,
    pub peringkat_8_id: Option<i64>,
    pub round_robin: bool,
    pub pool: i32,
    pub kode: Option<String>,
    pub is_bob: bool,
    pub has_vr: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DetailBagan {
    pub id: i64,
    pub bagan_id: Option<i64>,
    pub round: Option<i32>,
    pub urutan: Option<i32>,
    pub atlet1_id: Option<i64>,
    pub atlet2_id: Option<i64>,
    pub score1: Option<String>,
    pub score2: Option<String>,
    pub scorekecil1: Option<String>,
    pub scorekecil2: Option<String>,
    pub vr1: bool,
    pub vr2: bool,
    pub kata1: Option<String>,
    pub kata2: Option<String>,
    pub pemenang: Option<String>, // '1' = Aka, '2' = Ao, '3' = Draw
    pub hantei: bool,
    pub selesai: bool,
    pub team: bool,
    pub assigned_tatami_id: Option<i64>,
    pub kode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EnrichedDetailBagan {
    pub id: i64,
    pub bagan_id: Option<i64>,
    pub round: Option<i32>,
    pub urutan: Option<i32>,
    pub atlet1_id: Option<i64>,
    pub atlet2_id: Option<i64>,
    pub score1: Option<String>,
    pub score2: Option<String>,
    pub scorekecil1: Option<String>,
    pub scorekecil2: Option<String>,
    pub vr1: bool,
    pub vr2: bool,
    pub kata1: Option<String>,
    pub kata2: Option<String>,
    pub pemenang: Option<String>,
    pub hantei: bool,
    pub selesai: bool,
    pub team: bool,
    pub assigned_tatami_id: Option<i64>,
    pub kode: Option<String>,
    pub atlet1_nama: Option<String>,
    pub atlet1_kontingen: Option<String>,
    pub atlet1_logo: Option<String>,
    pub atlet1_is_priority: Option<bool>,
    pub atlet2_nama: Option<String>,
    pub atlet2_kontingen: Option<String>,
    pub atlet2_logo: Option<String>,
    pub atlet2_is_priority: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Tatami {
    pub id: i64,
    pub event_id: Option<i64>,
    pub tatami_number: Option<i32>,
    pub detail_bagan_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Score {
    pub id: i64,
    pub detail_bagan_id: Option<i64>,
    pub atlet: Option<i32>,
    pub score1: Option<String>,
    pub score2: Option<String>,
    pub score3: Option<String>,
    pub score4: Option<String>,
    pub score5: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Matchup {
    pub id: i64,
    pub bagan_id: Option<i64>,
    pub detail_bagan_id: Option<i64>,
    pub db_id: Option<i64>,
    pub round: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Role {
    pub id: i64,
    pub user_id: i64,
    pub role_type: String, // 'admin', 'admin_tatami', 'jury'
    pub event_id: Option<i64>,
    pub tatami_id: Option<i64>,
    pub jury_number: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Wasit {
    pub id: i64,
    pub event_id: i64,
    pub nama_wasit: String,
    pub perguruan_id: Option<i64>,
    pub kab_kota: Option<String>,
    pub lisensi: Option<String>,
    pub no_lisensi: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WasitTatami {
    pub id: i64,
    pub event_id: i64,
    pub tatami_id: i64,
    pub wasit_id: i64,
    pub posisi: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct WasitDetailBagan {
    pub id: i64,
    pub event_id: i64,
    pub detail_bagan_id: i64,
    pub wasit_id: i64,
    pub posisi: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TimetableDay {
    pub id: i64,
    pub event_id: i64,
    pub order: u32,
    pub tanggal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TimetableRow {
    pub id: i64,
    pub day_id: Option<i64>,
    pub order: u32,
    pub row_type: String, // 'slot', 'label'
    pub time_label: String,
    pub label_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TimetableCell {
    pub id: i64,
    pub row_id: i64,
    pub tatami_id: i64,
    pub nomor_tanding_id: Option<i64>,
    pub custom_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct KopSurat {
    pub id: i64,
    pub event_id: i64,
    pub logo: Option<String>,
    pub nama_organisasi: String,
    pub alamat: String,
    pub kontak: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EventKeterangan {
    pub id: i64,
    pub event_id: i64,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct SyncQueueItem {
    pub id: i64,
    pub event_id: Option<i64>,
    pub endpoint: String,
    pub payload: String, // Stored as JSON string
    pub status: String,   // 'pending', 'processing', 'synced', 'failed', 'abandoned'
    pub retry_count: i32,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawingCategoryInfo {
    pub id: i64,
    pub event_id: i64,
    pub nama_nomor_tanding: String,
    pub athlete_count: i64,
    pub bagan_count: i64,
    pub has_vr: bool,
    pub is_kumite: bool,
    pub is_beregu: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DrawingSummary {
    pub categories_processed: usize,
    pub bagans_created: usize,
    pub total_matches: usize,
    pub total_collisions: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveMatchInput {
    pub match_id: i64,
    pub bagan_id: i64,
    pub score1: Option<String>,
    pub score2: Option<String>,
    pub scorekecil1: Option<String>,
    pub scorekecil2: Option<String>,
    pub pemenang: Option<String>,
    pub kata1: Option<String>,
    pub kata2: Option<String>,
    pub vr1: bool,
    pub vr2: bool,
    pub hantei: bool,
    pub selesai: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodiumAthlete {
    pub id: i64,
    pub name: String,
    pub contingent: String,
    pub logo: Option<String>,
    pub is_priority: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaganPodium {
    pub j1: Option<PodiumAthlete>,
    pub j2: Option<PodiumAthlete>,
    pub j3a: Option<PodiumAthlete>,
    pub j3b: Option<PodiumAthlete>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateMatchAthletesInput {
    pub match_id: i64,
    pub atlet1_id: Option<i64>,
    pub atlet2_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryWinnerAthlete {
    pub juara: String, // "1", "2", "3a", "3b"
    pub nama_atlet: String,
    pub utusan_nama: String,
    pub perguruan_nama: String,
    pub bagan_nama: String,
    pub bagan_kode: Option<String>,
    pub is_counted: bool,
    pub keterangan: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StandingItem {
    pub id: i64,
    pub rank: usize,
    pub nama: String,
    pub logo: Option<String>,
    pub gold: i64,
    pub silver: i64,
    pub bronze: i64,
    pub total: i64,
    pub uncounted_gold: i64,
    pub winners: Vec<SummaryWinnerAthlete>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryJuaraResult {
    pub event_name: String,
    pub utusan_standings: Vec<StandingItem>,
    pub perguruan_standings: Vec<StandingItem>,
    pub total_gold: i64,
    pub total_silver: i64,
    pub total_bronze: i64,
    pub total_uncounted: i64,
}
