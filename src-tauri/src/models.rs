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
pub struct EnrichedPerguruan {
    pub id: i64,
    pub event_id: Option<i64>,
    pub nama_perguruan: Option<String>,
    pub athlete_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Utusan {
    pub id: i64,
    pub event_id: Option<i64>,
    pub nama_utusan: Option<String>,
    pub logo: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct EnrichedUtusan {
    pub id: i64,
    pub event_id: Option<i64>,
    pub nama_utusan: Option<String>,
    pub logo: Option<String>,
    pub athlete_count: i64,
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
pub struct EnrichedWasit {
    pub id: i64,
    pub event_id: i64,
    pub nama_wasit: String,
    pub perguruan_id: Option<i64>,
    pub nama_perguruan: Option<String>,
    pub kab_kota: Option<String>,
    pub lisensi: Option<String>,
    pub no_lisensi: Option<String>,
    pub is_active: bool,
    pub assigned_tatami_id: Option<i64>,
    pub assigned_tatami_number: Option<i32>,
    pub assigned_posisi: Option<String>,
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
pub struct EnrichedWasitTatami {
    pub id: i64,
    pub event_id: i64,
    pub tatami_id: i64,
    pub tatami_number: Option<i32>,
    pub wasit_id: i64,
    pub nama_wasit: String,
    pub lisensi: Option<String>,
    pub no_lisensi: Option<String>,
    pub perguruan_id: Option<i64>,
    pub nama_perguruan: Option<String>,
    pub kab_kota: Option<String>,
    pub posisi: String,
    pub assigned_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasitConflict {
    pub wasit_id: i64,
    pub nama_wasit: String,
    pub posisi: String,
    pub conflict_type: String, // "perguruan" | "kontingen_daerah"
    pub reason: String,
    pub detail_bagan_id: Option<i64>,
    pub match_kode: Option<String>,
    pub athlete_side: String, // "AKA" | "AO"
    pub athlete_name: String,
    pub conflicting_value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TatamiPanelOverview {
    pub tatami_id: i64,
    pub tatami_number: i32,
    pub active_match: Option<EnrichedDetailBagan>,
    pub category_name: Option<String>,
    pub assignments: Vec<EnrichedWasitTatami>,
    pub conflicts: Vec<WasitConflict>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveWasitInput {
    pub id: Option<i64>,
    pub event_id: i64,
    pub nama_wasit: String,
    pub perguruan_id: Option<i64>,
    pub kab_kota: Option<String>,
    pub lisensi: Option<String>,
    pub no_lisensi: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignWasitTatamiInput {
    pub event_id: i64,
    pub tatami_id: i64,
    pub wasit_id: i64,
    pub posisi: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AutoAssignPanelInput {
    pub event_id: i64,
    pub tatami_id: i64,
    pub wasit_ids: Vec<i64>,
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
    pub order: i32,
    pub tanggal: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct TimetableRow {
    pub id: i64,
    pub day_id: Option<i64>,
    pub order: i32,
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

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EnrichedNomorTanding {
    pub id: i64,
    pub event_id: Option<i64>,
    pub nama_nomor_tanding: Option<String>,
    pub is_bob: bool,
    pub has_vr: bool,
    pub athlete_count: i64,
    pub bagan_count: i64,
    pub first_bagan_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CategoryMutationInput {
    pub id: Option<i64>,
    pub event_id: i64,
    pub nama_nomor_tanding: String,
    pub is_bob: bool,
    pub has_vr: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryBaganWinner {
    pub juara: String, // "1", "2", "3a", "3b"
    pub athlete_id: i64,
    pub nama_atlet: String,
    pub perguruan_nama: String,
    pub utusan_nama: String,
    pub logo: Option<String>,
    pub is_uncounted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryBaganItem {
    pub id: i64,
    pub kode: String,
    pub nama_bagan: String,
    pub tipe_tanding: String, // "1" = Kata, "2" = Kumite
    pub is_selesai: bool,
    pub winners: Vec<SummaryBaganWinner>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SummaryBaganResult {
    pub event_name: String,
    pub total_bagans: usize,
    pub completed_bagans: usize,
    pub total_gold: usize,
    pub total_silver: usize,
    pub total_bronze: usize,
    pub bagans: Vec<SummaryBaganItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedTimetableCell {
    pub id: i64,
    pub row_id: i64,
    pub tatami_id: i64,
    pub tatami_number: i32,
    pub nomor_tanding_id: Option<i64>,
    pub nomor_tanding_nama: Option<String>,
    pub custom_text: String,
    pub atlet_count: i64,
    pub matches: i64,
    pub discipline: String, // "kata" | "kumite"
    pub est_duration_minutes: i64,
    pub progress_status: String, // "belum_disentuh" | "running" | "selesai" | "festival" | "break"
    pub progress_pct: f64,
    pub finished_matches: i64,
    pub remaining_matches: i64,
    pub is_festival: bool,
    pub is_break: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AthleteConflictDetail {
    pub athlete_name: String,
    pub tatami1_number: i32,
    pub category1_id: i64,
    pub category1_name: String,
    pub tatami2_number: i32,
    pub category2_id: i64,
    pub category2_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedTimetableRow {
    pub id: i64,
    pub day_id: i64,
    pub order: i32,
    pub row_type: String,
    pub time_label: String,
    pub label_text: String,
    pub cells: Vec<EnrichedTimetableCell>,
    pub conflicts: Vec<AthleteConflictDetail>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichedTimetableDay {
    pub id: i64,
    pub event_id: i64,
    pub order: i32,
    pub tanggal: Option<String>,
    pub label: String,
    pub rows: Vec<EnrichedTimetableRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimetableCategoryOption {
    pub id: i64,
    pub nama: String,
    pub age_group: String,
    pub atlet_count: i64,
    pub matches: i64,
    pub discipline: String,
    pub est_duration_minutes: i64,
    pub is_festival: bool,
    pub progress_status: String,
    pub progress_pct: f64,
    pub is_assigned: bool,
    pub assigned_tatami: Option<i32>,
    pub assigned_day_order: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TatamiLiveStatus {
    pub tatami_id: i64,
    pub tatami_number: i32,
    pub is_running: bool,
    pub category_name: Option<String>,
    pub round: Option<i32>,
    pub urutan: Option<i32>,
    pub atlet1_name: Option<String>,
    pub atlet2_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimetableProgressSummary {
    pub total_matches: i64,
    pub finished_matches: i64,
    pub remaining_matches: i64,
    pub running_categories_count: usize,
    pub finished_categories_count: usize,
    pub unstarted_categories_count: usize,
    pub total_categories_count: usize,
    pub overall_pct: f64,
    pub total_conflicts: usize,
    pub tatamis_live: Vec<TatamiLiveStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimetableEditorData {
    pub event_id: i64,
    pub event_name: String,
    pub tatamis: Vec<Tatami>,
    pub days: Vec<EnrichedTimetableDay>,
    pub categories: Vec<TimetableCategoryOption>,
    pub summary: TimetableProgressSummary,
    pub kop_surat: Option<KopSurat>,
    pub nt_athletes: std::collections::HashMap<i64, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveTimetableCellInput {
    pub tatami_id: i64,
    pub nomor_tanding_id: Option<i64>,
    pub custom_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveTimetableRowInput {
    pub id: Option<i64>,
    pub order: i32,
    pub row_type: String,
    pub time_label: String,
    pub label_text: String,
    pub cells: Vec<SaveTimetableCellInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveTimetableDayInput {
    pub day_id: i64,
    pub tanggal: Option<String>,
    pub rows: Vec<SaveTimetableRowInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaveTimetablePayload {
    pub days: Vec<SaveTimetableDayInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TatamiBookletCategory {
    pub match_seq: usize,
    pub category_id: i64,
    pub category_name: String,
    pub discipline: String,
    pub matches_count: i64,
    pub athletes_count: i64,
    pub bagan_id: Option<i64>,
    pub bagan_kode: Option<String>,
    pub bagan_nama: Option<String>,
    pub tipe_tanding: Option<String>,
    pub winners: Vec<SummaryBaganWinner>,
    pub matches: Vec<EnrichedDetailBagan>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TatamiBookletData {
    pub event_id: i64,
    pub event_name: String,
    pub kop_surat: Option<KopSurat>,
    pub tatami_id: i64,
    pub tatami_number: i64,
    pub day_id: i64,
    pub day_order: i32,
    pub day_label: String,
    pub day_date: Option<String>,
    pub unscheduled_list: Vec<String>,
    pub ordered_categories: Vec<TatamiBookletCategory>,
}

// --- FESTIVAL ROSTER MODELS ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FestivalAthlete {
    pub id: i64,
    pub nama: String,
    pub perguruan: String,
    pub utusan: String,
    pub jenis_kelamin: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FestivalMatch {
    pub match_num: usize,
    pub aka: FestivalAthlete,
    pub ao: Option<FestivalAthlete>,
    pub is_clash: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FestivalCategory {
    pub id: i64,
    pub nama: String,
    pub display_name: String,
    pub discipline: String, // "kata" | "kumite"
    pub tatami_id: i64,
    pub matches: Vec<FestivalMatch>,
    pub atlet_count: usize,
    pub match_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FestivalTatamiSection {
    pub tatami_id: i64,
    pub tatami_number: i64,
    pub categories: Vec<FestivalCategory>,
    pub total_athletes: usize,
    pub total_matches: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FestivalRosterData {
    pub event_id: i64,
    pub event_name: String,
    pub tatamis: Vec<Tatami>,
    pub sections: Vec<FestivalTatamiSection>,
    pub total_categories: usize,
    pub total_athletes: usize,
    pub total_matches: usize,
    pub total_clashes: usize,
    pub kop_surat: Option<KopSurat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FestivalRosterState {
    pub tatami_assignments: std::collections::HashMap<String, Vec<i64>>, // tatami_id -> [nt_id]
    pub athlete_pairings: std::collections::HashMap<String, Vec<i64>>,  // nt_id -> [athlete_id]
}

// --- TEAM KUMITE (KUMITE BEREGU) MODELS ---

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamBoutItem {
    pub matchup_id: i64,
    pub round: i32, // Bout 1, 2, 3...
    pub bout_id: i64,
    pub atlet1_id: Option<i64>,
    pub atlet1_nama: Option<String>,
    pub atlet1_kontingen: Option<String>,
    pub atlet2_id: Option<i64>,
    pub atlet2_nama: Option<String>,
    pub atlet2_kontingen: Option<String>,
    pub score1: Option<String>,
    pub score2: Option<String>,
    pub scorekecil1: Option<String>,
    pub scorekecil2: Option<String>,
    pub pemenang: Option<String>, // '1', '2', '3' (Draw)
    pub selesai: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamCandidateAthlete {
    pub id: i64,
    pub nama_atlet: String,
    pub perguruan_nama: Option<String>,
    pub utusan_nama: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamMatchInfo {
    pub is_team_match: bool,
    pub parent_match_id: i64,
    pub active_bout_id: i64,
    pub active_bout_round: i32,
    pub team_aka_name: String,
    pub team_ao_name: String,
    pub team_aka_utusan: String,
    pub team_ao_utusan: String,
    pub team_aka_logo: Option<String>,
    pub team_ao_logo: Option<String>,
    pub team_aka_score: i32,      // Bouts won
    pub team_ao_score: i32,       // Bouts won
    pub team_draw_count: i32,     // Bouts tied
    pub team_aka_points: i32,     // Aggregate points
    pub team_ao_points: i32,      // Aggregate points
    pub bouts: Vec<TeamBoutItem>,
    pub candidate_atlets_aka: Vec<TeamCandidateAthlete>,
    pub candidate_atlets_ao: Vec<TeamCandidateAthlete>,
    pub is_finished: bool,
    pub winner_team: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssignBoutAthletesInput {
    pub bout_id: i64,
    pub aka_atlet_id: Option<i64>,
    pub ao_atlet_id: Option<i64>,
    pub aka_custom_name: Option<String>,
    pub ao_custom_name: Option<String>,
}




#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepechageInfo {
    pub main_bagan_id: i64,
    pub is_finalists_ready: bool,
    pub finalist1_id: Option<i64>,
    pub finalist1_name: Option<String>,
    pub finalist2_id: Option<i64>,
    pub finalist2_name: Option<String>,
    pub pool_a_losers: Vec<String>,
    pub pool_b_losers: Vec<String>,
    pub repechage_bagan_id: Option<i64>,
    pub repechage_bagan_name: Option<String>,
    pub can_generate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepechageResult {
    pub repechage_bagan_id: i64,
    pub nama_bagan: String,
    pub matches_created: usize,
    pub pool_a_count: usize,
    pub pool_b_count: usize,
    pub message: String,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundRobinAthleteStanding {
    pub athlete_id: i64,
    pub athlete_name: String,
    pub utusan_name: String,
    pub perguruan_name: String,
    pub logo: Option<String>,
    pub played: i32,
    pub won: i32,
    pub lost: i32,
    pub drawn: i32,
    pub points_for: i32,
    pub points_against: i32,
    pub points_diff: i32,
    pub standings_points: f32,
    pub rank: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundRobinMatrixCell {
    pub match_id: Option<i64>,
    pub round: i32,
    pub urutan: i32,
    pub row_athlete_id: i64,
    pub col_athlete_id: i64,
    pub is_self: bool,
    pub is_finished: bool,
    pub winner_id: Option<i64>,
    pub row_score: Option<String>,
    pub col_score: Option<String>,
    pub row_result: Option<String>,
    pub assigned_tatami_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundRobinData {
    pub bagan_id: i64,
    pub nama_bagan: String,
    pub nomor_tanding_id: Option<i64>,
    pub nomor_tanding_name: Option<String>,
    pub athletes: Vec<EnrichedAtlet>,
    pub standings: Vec<RoundRobinAthleteStanding>,
    pub matrix_rows: Vec<Vec<RoundRobinMatrixCell>>,
    pub total_matches: usize,
    pub finished_matches: usize,
    pub is_completed: bool,
    pub podium: BaganPodium,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarshallingMatchItem {
    pub match_id: i64,
    pub bagan_id: Option<i64>,
    pub category_name: String,
    pub round: i32,
    pub urutan: i32,
    pub kode: Option<String>,
    pub atlet1_nama: String,
    pub atlet1_kontingen: String,
    pub atlet1_perguruan: Option<String>,
    pub atlet2_nama: String,
    pub atlet2_kontingen: String,
    pub atlet2_perguruan: Option<String>,
    pub status: String, // "LIVE" | "CALL_1" | "CALL_2" | "PREPARE"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TatamiQueue {
    pub tatami_id: i64,
    pub tatami_number: i32,
    pub current_match: Option<MarshallingMatchItem>,
    pub next_match: Option<MarshallingMatchItem>,
    pub upcoming_matches: Vec<MarshallingMatchItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UrgentCall {
    pub id: String,
    pub tatami_number: i32,
    pub athlete_name: String,
    pub contingent: String,
    pub message: String,
    pub timestamp: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarshallingOverview {
    pub event_name: String,
    pub tatamis: Vec<TatamiQueue>,
    pub active_announcement: Option<UrgentCall>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubmitUrgentCallInput {
    pub tatami_number: i32,
    pub athlete_name: String,
    pub contingent: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultsBookletEventInfo {
    pub nama_event: String,
    pub tanggal: String,
    pub tempat: String,
    pub nama_organisasi: String,
    pub logo: Option<String>,
    pub alamat: Option<String>,
    pub kontak: Option<String>,
    pub kota: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultsBookletBeritaAcara {
    pub nomor_surat: String,
    pub hari_tanggal: String,
    pub total_kontingen: i64,
    pub total_perguruan: i64,
    pub total_atlet: i64,
    pub total_partai: i64,
    pub partai_selesai: i64,
    pub catatan_sanksi: String,
    pub status_pelaksanaan: String,
    pub tanggal_pengesahan: String,
    pub kota_pengesahan: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultsBookletSignatory {
    pub jabatan: String,
    pub peran: String,
    pub nama: String,
    pub gelar: Option<String>,
    pub nomor_lisensi: Option<String>,
    pub instansi: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResultsBookletData {
    pub event_info: ResultsBookletEventInfo,
    pub berita_acara: ResultsBookletBeritaAcara,
    pub signatories: Vec<ResultsBookletSignatory>,
    pub standings_kontingen: Vec<StandingItem>,
    pub standings_perguruan: Vec<StandingItem>,
    pub category_winners: Vec<SummaryBaganItem>,
    pub referees: Vec<EnrichedWasit>,
    pub total_gold: i64,
    pub total_silver: i64,
    pub total_bronze: i64,
    pub grand_total: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BobSourceCategory {
    pub bagan_id: i64,
    pub nomor_tanding_id: i64,
    pub nama_bagan: String,
    pub kode_bagan: String,
    pub juara_1_id: Option<i64>,
    pub juara_1_nama: Option<String>,
    pub juara_1_kontingen: Option<String>,
    pub juara_1_perguruan: Option<String>,
    pub is_completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBobBaganInput {
    pub event_id: i64,
    pub nama_bob: String,
    pub source_bagan_ids: Vec<i64>,
    pub shuffle_type: Option<String>, // "kontingen" | "perguruan"
    pub format_type: Option<String>,  // "elimination" | "round_robin"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BobBaganResult {
    pub nomor_tanding_id: i64,
    pub bagan_id: i64,
    pub athlete_count: usize,
    pub nama_bob: String,
}


