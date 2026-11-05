export type AppMode = 'landing' | 'solo' | 'host' | 'client';

export interface ActiveChannel {
  type: 'tatami' | 'bagan' | 'roster' | 'contingent' | 'perguruan' | 'nomor_tanding' | 'sync' | 'overview' | 'master' | 'summary_juara' | 'summary_bagan' | 'timetable' | 'festival' | 'wasit' | 'buku_hasil';
  id: number | string;
  name: string;
  subtitle?: string;
}

export interface NetworkInterfaceInfo {
  name: string;
  ip: string;
  is_default: boolean;
}

export interface IpAvailabilityInfo {
  ip: string;
  name: string;
  status: "available" | "occupied" | "active_event" | "current_host" | string;
  event_name: string | null;
  is_default: boolean;
  is_current_host?: boolean;
}

export interface NetworkInfo {
  port: number;
  local_ip: string;
  server_url: string;
  qr_svg: string;
  available_ips?: NetworkInterfaceInfo[];
}

export interface EventItem {
  id: number;
  nama_event: string | null;
  kode_event: string | null;
  is_live_sync_enabled: boolean;
}

export interface NomorTandingItem {
  id: number;
  event_id: number | null;
  nama_nomor_tanding: string | null;
  tipe: string | null;
  is_bob: boolean;
  has_vr: boolean;
}

export interface EnrichedNomorTanding {
  id: number;
  event_id: number | null;
  nama_nomor_tanding: string | null;
  is_bob: boolean;
  has_vr: boolean;
  athlete_count: number;
  bagan_count: number;
  first_bagan_id: number | null;
}

export interface DatabaseSummary {
  db_path: string;
  media_path: string;
  events: EventItem[];
}

export interface TatamiItem {
  id: number;
  event_id: number | null;
  tatami_number: number | null;
  detail_bagan_id: number | null;
  nama_bagan?: string | null;
}

export interface BaganItem {
  id: number;
  event_id: number | null;
  nama_bagan: string | null;
  nomor_tanding_id: number | null;
  tipe_tanding: string | null;
  juara_1_id: number | null;
  juara_2_id: number | null;
  juara_3a_id: number | null;
  juara_3b_id: number | null;
  peringkat_5_id: number | null;
  peringkat_6_id: number | null;
  peringkat_7_id: number | null;
  peringkat_8_id: number | null;
  round_robin: boolean;
  pool: number;
  kode: string | null;
  is_bob: boolean;
  has_vr: boolean;
}

export interface DetailBaganItem {
  id: number;
  bagan_id: number | null;
  round: number | null;
  urutan: number | null;
  atlet1_id: number | null;
  atlet2_id: number | null;
  score1: string | null;
  score2: string | null;
  scorekecil1: string | null;
  scorekecil2: string | null;
  vr1: boolean;
  vr2: boolean;
  kata1: string | null;
  kata2: string | null;
  pemenang: string | null;
  hantei: boolean;
  selesai: boolean;
  team: boolean;
  assigned_tatami_id: number | null;
  kode: string | null;
  atlet1_nama?: string | null;
  atlet1_kontingen?: string | null;
  atlet1_logo?: string | null;
  atlet1_is_priority?: boolean;
  atlet2_nama?: string | null;
  atlet2_kontingen?: string | null;
  atlet2_logo?: string | null;
  atlet2_is_priority?: boolean;
}

export interface AtletItem {
  id: number;
  event_id: number | null;
  nama_atlet: string | null;
  nik: string | null;
  kode_atlet: string | null;
  additional_code: string | null;
  is_priority: boolean;
  perguruan_id: number | null;
  utusan_id: number | null;
  nomor_tanding_id: number | null;
}

export interface EnrichedAtletItem {
  id: number;
  event_id: number | null;
  nama_atlet: string | null;
  nik: string | null;
  kode_atlet: string | null;
  additional_code: string | null;
  is_priority: boolean;
  perguruan_id: number | null;
  perguruan_nama: string | null;
  utusan_id: number | null;
  utusan_nama: string | null;
  utusan_logo: string | null;
  nomor_tanding_id: number | null;
  nomor_tanding_nama: string | null;
  nomor_tanding_tipe: string | null;
  is_bob: boolean | null;
}

export interface AthleteMutationInput {
  id?: number | null;
  event_id: number;
  nama_atlet: string;
  nik?: string | null;
  kode_atlet?: string | null;
  additional_code?: string | null;
  is_priority: boolean;
  perguruan_id?: number | null;
  utusan_id?: number | null;
  nomor_tanding_id?: number | null;
}

export interface PerguruanItem {
  id: number;
  event_id: number | null;
  nama_perguruan: string | null;
}

export interface UtusanItem {
  id: number;
  event_id: number | null;
  nama_utusan: string | null;
  logo: string | null;
}

export interface EnrichedPerguruanItem {
  id: number;
  event_id: number | null;
  nama_perguruan: string | null;
  athlete_count: number;
}

export interface EnrichedUtusanItem {
  id: number;
  event_id: number | null;
  nama_utusan: string | null;
  logo: string | null;
  athlete_count: number;
}

export interface MonitorOption {
  id: number;
  name: string;
  width: number;
  height: number;
  is_primary: boolean;
  label: string;
}

export interface SyncConfig {
  mode: 'local' | 'production' | 'custom';
  url: string;
  label: string;
}

export interface EventMapping {
  hosted_event_id: number | null;
  hosted_event_name: string | null;
  target_mode: string;
  is_mismatch: boolean;
}

export interface HostedEventOption {
  id: number;
  nama_event: string;
  tanggal_mulai: string | null;
  tanggal_selesai: string | null;
  tempat: string | null;
  status: string | null;
}

export interface PingResult {
  success: boolean;
  status_code: number | null;
  latency_ms: number;
  message: string;
}

export interface SyncQueueStats {
  total_count: number;
  failed_count: number;
  pending_count: number;
  synced_count: number;
}

export interface SyncQueueEntry {
  id: number;
  event_id: number | null;
  endpoint: string;
  payload: string;
  status: string;
  retry_count: number;
  last_error: string | null;
  created_at: string | null;
  updated_at: string | null;
  kode_realtime: string | null;
  ring: number | null;
  round: number | null;
  urutan: number | null;
  score: string | null;
  winner: string | null;
}

export interface PullResult {
  success: boolean;
  message: string;
  created_count: number;
  updated_count: number;
  logo_count: number;
}

export interface DrawingCategoryInfo {
  id: number;
  event_id: number;
  nama_nomor_tanding: string;
  athlete_count: number;
  bagan_count: number;
  has_vr: boolean;
  is_kumite: boolean;
  is_beregu: boolean;
}

export interface DrawingSummary {
  categories_processed: number;
  bagans_created: number;
  total_matches: number;
  total_collisions: number;
}

export interface SaveMatchInput {
  match_id: number;
  bagan_id: number;
  score1: string | null;
  score2: string | null;
  scorekecil1?: string | null;
  scorekecil2?: string | null;
  pemenang: string | null;
  kata1?: string | null;
  kata2?: string | null;
  vr1: boolean;
  vr2: boolean;
  hantei: boolean;
  selesai: boolean;
}

export interface UpdateMatchAthletesInput {
  match_id: number;
  atlet1_id: number | null;
  atlet2_id: number | null;
}

export interface PodiumAthlete {
  id: number;
  name: string;
  contingent: string;
  logo: string | null;
  is_priority: boolean;
}

export interface BaganPodium {
  j1: PodiumAthlete | null;
  j2: PodiumAthlete | null;
  j3a: PodiumAthlete | null;
  j3b: PodiumAthlete | null;
}

export interface SummaryWinnerAthlete {
  juara: string; // "1", "2", "3a", "3b"
  nama_atlet: string;
  utusan_nama: string;
  perguruan_nama: string;
  bagan_nama: string;
  bagan_kode: string | null;
  is_counted: boolean;
  keterangan: string | null;
}

export interface StandingItem {
  id: number;
  rank: number;
  nama: string;
  logo: string | null;
  gold: number;
  silver: number;
  bronze: number;
  total: number;
  uncounted_gold: number;
  winners: SummaryWinnerAthlete[];
}

export interface SummaryJuaraResult {
  event_name: string;
  utusan_standings: StandingItem[];
  perguruan_standings: StandingItem[];
  total_gold: number;
  total_silver: number;
  total_bronze: number;
  total_uncounted: number;
}

export interface SummaryBaganWinner {
  juara: string; // "1", "2", "3a", "3b"
  athlete_id: number;
  nama_atlet: string;
  perguruan_nama: string;
  utusan_nama: string;
  logo: string | null;
  is_uncounted: boolean;
}

export interface SummaryBaganItem {
  id: number;
  kode: string;
  nama_bagan: string;
  tipe_tanding: string; // "1" = Kata, "2" = Kumite
  is_selesai: boolean;
  winners: SummaryBaganWinner[];
}

export interface SummaryBaganResult {
  event_name: string;
  total_bagans: number;
  completed_bagans: number;
  total_gold: number;
  total_silver: number;
  total_bronze: number;
  bagans: SummaryBaganItem[];
}

export interface TimetableDay {
  id: number;
  event_id: number;
  order: number;
  tanggal: string | null;
}

export interface TimetableRow {
  id: number;
  day_id: number | null;
  order: number;
  row_type: 'slot' | 'label';
  time_label: string;
  label_text: string;
}

export interface TimetableCell {
  id: number;
  row_id: number;
  tatami_id: number;
  nomor_tanding_id: number | null;
  custom_text: string;
}

export interface EnrichedTimetableCell {
  id: number;
  row_id: number;
  tatami_id: number;
  tatami_number: number;
  nomor_tanding_id: number | null;
  nomor_tanding_nama: string | null;
  custom_text: string;
  atlet_count: number;
  matches: number;
  discipline: 'kata' | 'kumite';
  est_duration_minutes: number;
  progress_status: 'belum_disentuh' | 'running' | 'selesai' | 'festival' | 'break';
  progress_pct: number;
  finished_matches: number;
  remaining_matches: number;
  is_festival: boolean;
  is_break: boolean;
}

export interface AthleteConflictDetail {
  athlete_name: string;
  tatami1_number: number;
  category1_id: number;
  category1_name: string;
  tatami2_number: number;
  category2_id: number;
  category2_name: string;
}

export interface EnrichedTimetableRow {
  id: number;
  day_id: number;
  order: number;
  row_type: 'slot' | 'label';
  time_label: string;
  label_text: string;
  cells: EnrichedTimetableCell[];
  conflicts: AthleteConflictDetail[];
}

export interface EnrichedTimetableDay {
  id: number;
  event_id: number;
  order: number;
  tanggal: string | null;
  label: string;
  rows: EnrichedTimetableRow[];
}

export interface TimetableCategoryOption {
  id: number;
  nama: string;
  age_group: string;
  atlet_count: number;
  matches: number;
  discipline: 'kata' | 'kumite';
  est_duration_minutes: number;
  is_festival: boolean;
  progress_status: 'belum_disentuh' | 'running' | 'selesai';
  progress_pct: number;
  is_assigned: boolean;
  assigned_tatami: number | null;
  assigned_day_order: number | null;
}

export interface TatamiLiveStatus {
  tatami_id: number;
  tatami_number: number;
  is_running: boolean;
  category_name: string | null;
  round: number | null;
  urutan: number | null;
  atlet1_name: string | null;
  atlet2_name: string | null;
}

export interface KopSurat {
  id: number;
  event_id: number;
  logo: string | null;
  nama_organisasi: string;
  alamat: string;
  kontak: string;
}

export interface TimetableProgressSummary {
  total_matches: number;
  finished_matches: number;
  remaining_matches: number;
  running_categories_count: number;
  finished_categories_count: number;
  unstarted_categories_count: number;
  total_categories_count: number;
  overall_pct: number;
  total_conflicts: number;
  tatamis_live: TatamiLiveStatus[];
}

export interface TimetableEditorData {
  event_id: number;
  event_name: string;
  tatamis: TatamiItem[];
  days: EnrichedTimetableDay[];
  categories: TimetableCategoryOption[];
  summary: TimetableProgressSummary;
  kop_surat: KopSurat | null;
  nt_athletes?: Record<number, string[]>;
}

export interface SaveTimetableCellInput {
  tatami_id: number;
  nomor_tanding_id: number | null;
  custom_text: string;
}

export interface SaveTimetableRowInput {
  id: number | null;
  order: number;
  row_type: 'slot' | 'label';
  time_label: string;
  label_text: string;
  cells: SaveTimetableCellInput[];
}

export interface SaveTimetableDayInput {
  day_id: number;
  tanggal: string | null;
  rows: SaveTimetableRowInput[];
}

export interface SaveTimetablePayload {
  days: SaveTimetableDayInput[];
}

export interface TatamiBookletCategory {
  match_seq: number;
  category_id: number;
  category_name: string;
  discipline: string;
  matches_count: number;
  athletes_count: number;
  bagan_id: number | null;
  bagan_kode: string | null;
  bagan_nama: string | null;
  tipe_tanding: string | null;
  winners: SummaryBaganWinner[];
  matches: DetailBaganItem[];
}

export interface TatamiBookletData {
  event_id: number;
  event_name: string;
  venue?: string | null;
  kop_surat: KopSurat | null;
  tatami_id: number;
  tatami_number: number;
  day_id: number;
  day_order: number;
  day_label: string;
  day_date: string | null;
  unscheduled_list: string[];
  ordered_categories: TatamiBookletCategory[];
}

// --- FESTIVAL ROSTER TYPES ---

export interface FestivalAthlete {
  id: number;
  nama: string;
  perguruan: string;
  utusan: string;
  jenis_kelamin: string;
}

export interface FestivalMatch {
  match_num: number;
  aka: FestivalAthlete;
  ao: FestivalAthlete | null;
  is_clash: boolean;
}

export interface FestivalCategory {
  id: number;
  nama: string;
  display_name: string;
  discipline: "kata" | "kumite";
  tatami_id: number;
  matches: FestivalMatch[];
  atlet_count: number;
  match_count: number;
}

export interface FestivalTatamiSection {
  tatami_id: number;
  tatami_number: number;
  categories: FestivalCategory[];
  total_athletes: number;
  total_matches: number;
}

export interface FestivalRosterData {
  event_id: number;
  event_name: string;
  tatamis: TatamiItem[];
  sections: FestivalTatamiSection[];
  total_categories: number;
  total_athletes: number;
  total_matches: number;
  total_clashes: number;
  kop_surat?: KopSurat | null;
}

export interface FestivalRosterState {
  tatami_assignments: Record<string, number[]>;
  athlete_pairings: Record<string, number[]>;
}




// --- TEAM KUMITE (KUMITE BEREGU) TYPES ---

export interface TeamBoutItem {
  matchup_id: number;
  round: number;
  bout_id: number;
  atlet1_id: number | null;
  atlet1_nama: string | null;
  atlet1_kontingen: string | null;
  atlet2_id: number | null;
  atlet2_nama: string | null;
  atlet2_kontingen: string | null;
  score1: string | null;
  score2: string | null;
  scorekecil1: string | null;
  scorekecil2: string | null;
  pemenang: string | null;
  selesai: boolean;
}

export interface TeamCandidateAthlete {
  id: number;
  nama_atlet: string;
  perguruan_nama: string | null;
  utusan_nama: string | null;
}

export interface TeamMatchInfo {
  is_team_match: boolean;
  parent_match_id: number;
  active_bout_id: number;
  active_bout_round: number;
  team_aka_name: string;
  team_ao_name: string;
  team_aka_utusan: string;
  team_ao_utusan: string;
  team_aka_logo: string | null;
  team_ao_logo: string | null;
  team_aka_score: number;
  team_ao_score: number;
  team_draw_count: number;
  team_aka_points: number;
  team_ao_points: number;
  bouts: TeamBoutItem[];
  candidate_atlets_aka: TeamCandidateAthlete[];
  candidate_atlets_ao: TeamCandidateAthlete[];
  is_finished: boolean;
  winner_team: string | null;
}

export interface AssignBoutAthletesInput {
  bout_id: number;
  aka_atlet_id?: number | null;
  ao_atlet_id?: number | null;
  aka_custom_name?: string | null;
  ao_custom_name?: string | null;
}

export interface RepechageInfo {
  main_bagan_id: number;
  is_finalists_ready: boolean;
  finalist1_id?: number | null;
  finalist1_name?: string | null;
  finalist2_id?: number | null;
  finalist2_name?: string | null;
  pool_a_losers: string[];
  pool_b_losers: string[];
  repechage_bagan_id?: number | null;
  repechage_bagan_name?: string | null;
  can_generate: boolean;
}

export interface RepechageResult {
  repechage_bagan_id: number;
  nama_bagan: string;
  matches_created: number;
  pool_a_count: number;
  pool_b_count: number;
  message: string;
}

// --- ROUND ROBIN TOURNAMENT TYPES ---

export interface RoundRobinAthleteStanding {
  athlete_id: number;
  athlete_name: string;
  utusan_name: string;
  perguruan_name: string;
  logo: string | null;
  played: number;
  won: number;
  lost: number;
  drawn: number;
  points_for: number;
  points_against: number;
  points_diff: number;
  standings_points: number;
  rank: number;
}

export interface RoundRobinMatrixCell {
  match_id: number | null;
  round: number;
  urutan: number;
  row_athlete_id: number;
  col_athlete_id: number;
  is_self: boolean;
  is_finished: boolean;
  winner_id: number | null;
  row_score: string | null;
  col_score: string | null;
  row_result: string | null;
  assigned_tatami_id: number | null;
}

export interface RoundRobinData {
  bagan_id: number;
  nama_bagan: string;
  nomor_tanding_id: number | null;
  nomor_tanding_name: string | null;
  athletes: EnrichedAtletItem[];
  standings: RoundRobinAthleteStanding[];
  matrix_rows: RoundRobinMatrixCell[][];
  total_matches: number;
  finished_matches: number;
  is_completed: boolean;
  podium: BaganPodium;
}

export interface WasitItem {
  id: number;
  event_id: number;
  nama_wasit: string;
  perguruan_id: number | null;
  nama_perguruan: string | null;
  kab_kota: string | null;
  lisensi: string | null;
  no_lisensi: string | null;
  is_active: boolean;
  assigned_tatami_id: number | null;
  assigned_tatami_number: number | null;
  assigned_posisi: string | null;
}

export interface WasitTatamiAssignment {
  id: number;
  event_id: number;
  tatami_id: number;
  tatami_number: number | null;
  wasit_id: number;
  nama_wasit: string;
  lisensi: string | null;
  no_lisensi: string | null;
  perguruan_id: number | null;
  nama_perguruan: string | null;
  kab_kota: string | null;
  posisi: string;
  assigned_at: string | null;
}

export interface WasitConflict {
  wasit_id: number;
  nama_wasit: string;
  posisi: string;
  conflict_type: 'perguruan' | 'kontingen_daerah';
  reason: string;
  detail_bagan_id: number | null;
  match_kode: string | null;
  athlete_side: 'AKA' | 'AO';
  athlete_name: string;
  conflicting_value: string;
}

export interface TatamiPanelOverview {
  tatami_id: number;
  tatami_number: number;
  active_match: DetailBaganItem | null;
  category_name: string | null;
  assignments: WasitTatamiAssignment[];
  conflicts: WasitConflict[];
}

export interface SaveWasitPayload {
  id?: number | null;
  event_id: number;
  nama_wasit: string;
  perguruan_id?: number | null;
  kab_kota?: string | null;
  lisensi?: string | null;
  no_lisensi?: string | null;
  is_active: boolean;
}

export interface AssignWasitPayload {
  event_id: number;
  tatami_id: number;
  wasit_id: number;
  posisi: string;
}

export interface AutoAssignPanelPayload {
  event_id: number;
  tatami_id: number;
  wasit_ids: number[];
}

export interface MarshallingMatchItem {
  match_id: number;
  bagan_id: number | null;
  category_name: string;
  round: number;
  urutan: number;
  kode: string | null;
  atlet1_nama: string;
  atlet1_kontingen: string;
  atlet1_perguruan: string | null;
  atlet2_nama: string;
  atlet2_kontingen: string;
  atlet2_perguruan: string | null;
  status: 'LIVE' | 'CALL_1' | 'CALL_2' | 'PREPARE';
}

export interface TatamiQueue {
  tatami_id: number;
  tatami_number: number;
  current_match: MarshallingMatchItem | null;
  next_match: MarshallingMatchItem | null;
  upcoming_matches: MarshallingMatchItem[];
}

export interface UrgentCall {
  id: string;
  tatami_number: number;
  athlete_name: string;
  contingent: string;
  message: string;
  timestamp: string;
}

export interface MarshallingOverview {
  event_name: string;
  tatamis: TatamiQueue[];
  active_announcement: UrgentCall | null;
}

export interface SubmitUrgentCallPayload {
  tatami_number: number;
  athlete_name: string;
  contingent?: string | null;
  message: string;
}

export interface ResultsBookletEventInfo {
  nama_event: string;
  tanggal: string;
  tempat: string;
  nama_organisasi: string;
  logo: string | null;
  alamat: string | null;
  kontak: string | null;
  kota: string;
}

export interface ResultsBookletBeritaAcara {
  nomor_surat: string;
  hari_tanggal: string;
  total_kontingen: number;
  total_perguruan: number;
  total_atlet: number;
  total_partai: number;
  partai_selesai: number;
  catatan_sanksi: string;
  status_pelaksanaan: string;
  tanggal_pengesahan: string;
  kota_pengesahan: string;
}

export interface ResultsBookletSignatory {
  jabatan: string;
  peran: string;
  nama: string;
  gelar: string | null;
  nomor_lisensi: string | null;
  instansi: string | null;
}

export interface ResultsBookletData {
  event_info: ResultsBookletEventInfo;
  berita_acara: ResultsBookletBeritaAcara;
  signatories: ResultsBookletSignatory[];
  standings_kontingen: StandingItem[];
  standings_perguruan: StandingItem[];
  category_winners: SummaryBaganItem[];
  category_brackets?: TatamiBookletCategory[];
  referees: WasitItem[];
  total_gold: number;
  total_silver: number;
  total_bronze: number;
  grand_total: number;
}

export interface BobSourceCategory {
  bagan_id: number;
  nomor_tanding_id: number;
  nama_bagan: string;
  kode_bagan: string;
  juara_1_id: number | null;
  juara_1_nama: string | null;
  juara_1_kontingen: string | null;
  juara_1_perguruan: string | null;
  is_completed: boolean;
}

export interface CreateBobBaganPayload {
  event_id: number;
  nama_bob: string;
  source_bagan_ids: number[];
  shuffle_type?: 'kontingen' | 'perguruan';
  format_type?: 'elimination' | 'round_robin';
}

export interface BobBaganResult {
  nomor_tanding_id: number;
  bagan_id: number;
  athlete_count: number;
  nama_bob: string;
}





