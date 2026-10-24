export type AppMode = 'landing' | 'solo' | 'host' | 'client';

export interface ActiveChannel {
  type: 'tatami' | 'bagan' | 'roster' | 'contingent' | 'sync' | 'overview' | 'master';
  id: number | string;
  name: string;
  subtitle?: string;
}

export interface NetworkInfo {
  port: number;
  local_ip: string;
  server_url: string;
  qr_svg: string;
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

