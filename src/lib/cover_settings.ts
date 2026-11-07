/**
 * Settings for the landscape cover of the results booklet ("sampul").
 *
 * Text fields are overrides: `null` means "use the default", which follows the
 * event's own data (name, venue, dates, logo). Settings are remembered per event on
 * this computer (localStorage); they only affect printing.
 */
export interface CoverSettings {
  enabled: boolean;
  federasi: string | null;
  badge: string | null;
  judul: string | null;
  namaEvent: string | null;
  tempat: string | null;
  tanggal: string | null;
  showSupportedBy: boolean;
  /** "event" = kop surat logo of the event, "custom" = uploaded, "none" = no logo card. */
  logoMode: "event" | "custom" | "none";
  customLogo: string | null;
  /** Uploaded background (data URL); null = the default Ambrilindo cover template. */
  customBackground: string | null;
  /** Back cover: same design as the front, with a thank-you message. */
  penutupEnabled: boolean;
  penutupJudul: string | null;
  penutupPesan: string | null;
}

export const DEFAULT_COVER_BACKGROUND = "/img/cover/cover_clean_template.png";
export const DEFAULT_FEDERASI = "FEDERASI OLAHRAGA KARATE-DO INDONESIA";
export const DEFAULT_BADGE = "OFFICIAL MATCH RESULT BOOK";
export const DEFAULT_PENUTUP_JUDUL = "TERIMA KASIH";
export const DEFAULT_PENUTUP_PESAN =
  "Atas partisipasi seluruh atlet, pelatih, official, wasit, juri, dan panitia pelaksana.";

export function defaultCoverSettings(): CoverSettings {
  return {
    enabled: true,
    federasi: null,
    badge: null,
    judul: null,
    namaEvent: null,
    tempat: null,
    tanggal: null,
    showSupportedBy: true,
    logoMode: "event",
    customLogo: null,
    customBackground: null,
    penutupEnabled: true,
    penutupJudul: null,
    penutupPesan: null,
  };
}

const storageKey = (eventId: number) => `ambrilindo_cover_settings_${eventId}`;

export function loadCoverSettings(eventId: number): CoverSettings {
  const base = defaultCoverSettings();
  try {
    const raw = localStorage.getItem(storageKey(eventId));
    if (raw) return { ...base, ...JSON.parse(raw) };
  } catch (_) {}
  return base;
}

/** Returns false when the browser refused to store it (e.g. a very large image). */
export function saveCoverSettings(eventId: number, settings: CoverSettings): boolean {
  try {
    localStorage.setItem(storageKey(eventId), JSON.stringify(settings));
    return true;
  } catch (_) {
    return false;
  }
}

export function clearCoverSettings(eventId: number) {
  try {
    localStorage.removeItem(storageKey(eventId));
  } catch (_) {}
}

const BULAN = [
  "Januari", "Februari", "Maret", "April", "Mei", "Juni",
  "Juli", "Agustus", "September", "Oktober", "November", "Desember",
];

function parseIsoDate(s: string): { d: number; m: number; y: number } | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})/.exec(s.trim());
  if (!m) return null;
  return { y: Number(m[1]), m: Number(m[2]), d: Number(m[3]) };
}

/**
 * "2026-10-03" -> "3 Oktober 2026"; "2026-10-03 s.d 2026-10-04" -> "3 - 4 Oktober 2026".
 * Anything that is not an ISO date is returned unchanged.
 */
export function formatTanggalIndonesia(raw: string | null | undefined): string {
  if (!raw) return "";
  const parts = raw.split(/\s+s\.d\.?\s+/i);
  const a = parseIsoDate(parts[0] || "");
  if (!a) return raw;
  const fmt = (x: { d: number; m: number; y: number }) => `${x.d} ${BULAN[x.m - 1] || ""} ${x.y}`;
  if (parts.length < 2) return fmt(a);
  const b = parseIsoDate(parts[parts.length - 1]);
  if (!b) return fmt(a);
  if (a.y === b.y && a.m === b.m) return `${a.d} - ${b.d} ${BULAN[a.m - 1]} ${a.y}`;
  if (a.y === b.y) return `${a.d} ${BULAN[a.m - 1]} - ${fmt(b)}`;
  return `${fmt(a)} - ${fmt(b)}`;
}

export interface CoverEventDefaults {
  namaEvent: string;
  tempat: string;
  tanggalRaw: string;
}

/** The text actually shown on the cover: overrides first, then event data. */
export function resolveCoverText(settings: CoverSettings, ev: CoverEventDefaults) {
  const multiDay = /\s+s\.d\.?\s+/i.test(ev.tanggalRaw || "");
  const tanggalFormatted = formatTanggalIndonesia(ev.tanggalRaw);
  return {
    federasi: settings.federasi ?? DEFAULT_FEDERASI,
    badge: settings.badge ?? DEFAULT_BADGE,
    judul: settings.judul ?? (multiDay ? "RESULT MATCH: ALL DAYS" : "RESULT MATCH: DAY 1"),
    namaEvent: settings.namaEvent ?? (ev.namaEvent || "").toUpperCase(),
    tempat: settings.tempat ?? ev.tempat ?? "",
    tanggal: settings.tanggal ?? (tanggalFormatted ? `Tanggal ${tanggalFormatted}` : ""),
    penutupJudul: settings.penutupJudul ?? DEFAULT_PENUTUP_JUDUL,
    penutupPesan: settings.penutupPesan ?? DEFAULT_PENUTUP_PESAN,
  };
}
