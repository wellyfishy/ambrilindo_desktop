import { apiCall, isClientMode } from "./api_client";

export type LicenseModule = "kata" | "kumite" | "drawing";

export interface LicenseStatus {
  /** none | active | expired | invalid | clock_tampered */
  state: string;
  message: string | null;
  buyer: string | null;
  org: string | null;
  modules: string[];
  maxDevices: number | null;
  expiresAt: number | null;
  keyHint: string | null;
  key: string | null;
  deviceId: string;
  deviceName: string;
  lastCheckedAt: number | null;
  serverUrl: string;
}

export const MODULE_LABELS: Record<LicenseModule, string> = {
  kata: "Ambrilindo Kata",
  kumite: "Ambrilindo Kumite",
  drawing: "Ambrilindo Drawing",
};

/** Shared, reactive license state. In client mode this is the host's license. */
export const license = $state<{ status: LicenseStatus | null; loaded: boolean; panelOpen: boolean }>({
  status: null,
  loaded: false,
  panelOpen: false,
});

function hasTauri(): boolean {
  return typeof window !== "undefined" && !!(window as any).__TAURI_INTERNALS__;
}

export async function loadLicense(): Promise<LicenseStatus | null> {
  try {
    license.status = await apiCall<LicenseStatus>("get_license_status");
  } catch {
    // An older host without license support, or no connection yet: keep what we had.
  }
  license.loaded = true;
  return license.status;
}

let listening = false;
/** Keeps `license.status` in sync with the Rust side (startup refresh, activation). */
export async function watchLicense(): Promise<void> {
  await loadLicense();
  if (listening || !hasTauri() || isClientMode()) return;
  listening = true;
  const { listen } = await import("@tauri-apps/api/event");
  await listen<LicenseStatus>("license_changed", (ev) => {
    license.status = ev.payload;
    license.loaded = true;
  });
}

export function hasModule(module: LicenseModule, status: LicenseStatus | null = license.status): boolean {
  if (!status || status.state !== "active") return false;
  if (status.expiresAt && Date.now() / 1000 > status.expiresAt) return false;
  return status.modules.includes(module);
}

/** Kata or Kumite, by a bracket's tipe_tanding / name (mirrors license::discipline_of in Rust). */
export function disciplineOf(tipeTanding?: string | number | null, name?: string | null): LicenseModule | null {
  const tt = String(tipeTanding ?? "").trim().toLowerCase();
  if (tt === "1" || tt === "kata") return "kata";
  if (tt === "2" || tt === "kumite") return "kumite";
  const n = (name ?? "").toLowerCase();
  if (n.includes("kata")) return "kata";
  if (n.includes("kumite")) return "kumite";
  return null;
}

export function canOpenDiscipline(module: LicenseModule | null): boolean {
  return module ? hasModule(module) : hasModule("kata") || hasModule("kumite");
}

export function lockedMessage(module: LicenseModule | null): string {
  const label = module ? MODULE_LABELS[module] : "Kata / Kumite";
  return `Modul ${label} terkunci. Buka menu Lisensi untuk aktivasi.`;
}

/** Errors raised by the Rust license gate start with "Modul … terkunci". */
export function isLicenseError(err: unknown): boolean {
  const msg = typeof err === "string" ? err : (err as any)?.message ?? "";
  return /terkunci/i.test(msg) && /modul/i.test(msg);
}

export function errorText(err: unknown): string {
  return typeof err === "string" ? err : (err as any)?.message ?? String(err);
}

export function openLicensePanel(): void {
  license.panelOpen = true;
}
