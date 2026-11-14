<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "$lib/api_client";
  import { license, openLicensePanel } from "$lib/license.svelte";
  import type { AppMode, DatabaseSummary, NetworkInfo, IpAvailabilityInfo } from "$lib/types";

  interface Props {
    currentDb: DatabaseSummary | null;
    networkInfo: NetworkInfo | null;
    onSelectMode: (mode: AppMode, hostUrl?: string, selectedIp?: string) => void;
    onPickDatabase: () => void;
  }

  let { currentDb, networkInfo, onSelectMode, onPickDatabase }: Props = $props();

  // ---------------------------------------------------------------------------
  // Last session (one-click resume)
  // ---------------------------------------------------------------------------
  interface LastSession {
    mode: "host" | "client";
    ip?: string;
    hostUrl?: string;
    at: number;
  }
  const LAST_SESSION_KEY = "ambrilindo_last_session";

  function readLastSession(): LastSession | null {
    try {
      const raw = localStorage.getItem(LAST_SESSION_KEY);
      const val = raw ? (JSON.parse(raw) as LastSession) : null;
      return val && (val.mode === "host" || val.mode === "client") ? val : null;
    } catch {
      return null;
    }
  }
  function saveLastSession(s: Omit<LastSession, "at">) {
    try {
      localStorage.setItem(LAST_SESSION_KEY, JSON.stringify({ ...s, at: Date.now() }));
    } catch {
      // Storage unavailable: resume shortcut is just skipped.
    }
  }

  const initialSession = readLastSession();
  let lastSession = $state<LastSession | null>(initialSession);

  // ---------------------------------------------------------------------------
  // State
  // ---------------------------------------------------------------------------
  // Prefill with the last host this laptop joined.
  let hostInputUrl = $state(initialSession?.mode === "client" ? displayHost(initialSession.hostUrl) : "");
  let isConnectingHost = $state(false);
  let hostError = $state<string | null>(null);

  let ipAvailabilities = $state<IpAvailabilityInfo[]>([]);
  let isCheckingIps = $state(false);
  let isStoppingServer = $state(false);
  let selectedHostIp = $state<string>("0.0.0.0");
  let networkOpen = $state(false);

  let isOurServerRunning = $derived(
    ipAvailabilities.some((item) => item.status === "current_host" || item.is_current_host)
  );

  let detectedActiveEvent = $derived(
    ipAvailabilities.find((item) => item.status === "active_event" && !item.is_current_host && item.event_name) ||
    ipAvailabilities.find((item) => item.status === "active_event" && !item.is_current_host)
  );

  let selectedIpInfo = $derived(ipAvailabilities.find((item) => item.ip === selectedHostIp) || null);

  /** The network needs attention: open the advanced section on its own. */
  let networkProblem = $derived(
    selectedIpInfo?.status === "occupied" || selectedIpInfo?.status === "active_event"
  );
  $effect(() => {
    if (networkProblem) networkOpen = true;
  });

  let licenseActive = $derived(license.status?.state === "active");
  let dbName = $derived(currentDb ? currentDb.db_path.split(/[/\\]/).pop() : null);

  onMount(async () => {
    await refreshIpAvailability();
  });

  function displayHost(url?: string): string {
    return (url || "").replace(/^https?:\/\//, "").replace(/:8000\/?$/, "").replace(/\/$/, "");
  }

  async function refreshIpAvailability() {
    isCheckingIps = true;
    try {
      const list = await invoke<IpAvailabilityInfo[]>("check_ip_availability");
      if (Array.isArray(list) && list.length > 0) {
        ipAvailabilities = list;
        if (!selectedHostIp || selectedHostIp === "0.0.0.0") {
          const hasCurrentHost = list.find((i) => i.status === "current_host" || i.is_current_host);
          if (!hasCurrentHost) {
            const hasZero = list.find((i) => i.ip === "0.0.0.0");
            if (hasZero && hasZero.status === "available") {
              selectedHostIp = "0.0.0.0";
            } else {
              const firstAvail = list.find((i) => i.status === "available");
              if (firstAvail) selectedHostIp = firstAvail.ip;
            }
          }
        }
      }
    } catch (_) {
      // Fallback from networkInfo
      if (networkInfo?.available_ips) {
        ipAvailabilities = [
          { ip: "0.0.0.0", name: "Semua jaringan", status: "available", event_name: null, is_default: true },
          ...networkInfo.available_ips.map((item) => ({
            ip: item.ip,
            name: item.name,
            status: "available",
            event_name: null,
            is_default: item.is_default,
          })),
        ];
      }
    } finally {
      isCheckingIps = false;
    }
  }

  async function handleStopServer() {
    isStoppingServer = true;
    try {
      await invoke("stop_arena_server");
      await refreshIpAvailability();
    } catch (e: any) {
      alert("Gagal menghentikan server: " + (e?.message || e));
    } finally {
      isStoppingServer = false;
    }
  }

  async function handleJoinHost(customUrl?: string) {
    let cleanUrl = (customUrl || hostInputUrl).trim();
    if (!cleanUrl) {
      hostError = "Isi alamat komputer utama, misalnya 192.168.1.5.";
      return;
    }
    if (!cleanUrl.startsWith("http://") && !cleanUrl.startsWith("https://")) {
      cleanUrl = "http://" + cleanUrl;
    }
    if (!cleanUrl.includes(":", 7)) {
      cleanUrl = cleanUrl + ":8000";
    }

    isConnectingHost = true;
    hostError = null;

    try {
      const res = await fetch(`${cleanUrl}/api/health`, { signal: AbortSignal.timeout(4000) });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const data = await res.json();
      if (data.status === "ok") {
        saveLastSession({ mode: "client", hostUrl: cleanUrl });
        onSelectMode("client", cleanUrl);
      } else {
        throw new Error("respon tidak dikenali");
      }
    } catch (e: any) {
      hostError = `Tidak bisa terhubung ke ${displayHost(cleanUrl)}. Pastikan komputer utama sudah dimulai dan satu jaringan.`;
      console.warn("Gagal terhubung ke host:", e?.message || e);
    } finally {
      isConnectingHost = false;
    }
  }

  function handleStartHost(ip: string = selectedHostIp) {
    saveLastSession({ mode: "host", ip });
    onSelectMode("host", undefined, ip);
  }

  function resumeLastSession() {
    if (!lastSession) return;
    if (lastSession.mode === "host") handleStartHost(lastSession.ip || "0.0.0.0");
    else handleJoinHost(lastSession.hostUrl);
  }

  function forgetLastSession() {
    try {
      localStorage.removeItem(LAST_SESSION_KEY);
    } catch {}
    lastSession = null;
  }

  function relativeTime(ts: number): string {
    const mins = Math.round((Date.now() - ts) / 60000);
    if (mins < 1) return "baru saja";
    if (mins < 60) return `${mins} menit lalu`;
    const hours = Math.round(mins / 60);
    if (hours < 24) return `${hours} jam lalu`;
    const days = Math.round(hours / 24);
    return `${days} hari lalu`;
  }

  function ipLabel(item: IpAvailabilityInfo): string {
    return item.ip === "0.0.0.0" ? "Otomatis (semua jaringan)" : item.ip;
  }
</script>

<div class="landing h-full w-full min-h-screen flex flex-col select-none bg-[#14161b] text-zinc-200 overflow-y-auto font-sans">
  <!-- Top bar: brand + database + license -->
  <header class="w-full max-w-5xl mx-auto px-6 pt-6 pb-5 flex flex-wrap items-center justify-between gap-4 border-b border-white/[0.08]">
    <div class="flex items-center gap-3">
      <div class="w-10 h-10 rounded-xl bg-red-600 text-white flex items-center justify-center shadow-md shrink-0" aria-hidden="true">
        <svg class="w-5 h-5" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 2L3 7v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V7l-9-5z" />
        </svg>
      </div>
      <div class="leading-tight">
        <h1 class="text-lg font-black text-white tracking-tight">AMBRILINDO ARENA</h1>
        <span class="text-xs font-mono text-zinc-500">v3.0</span>
      </div>
    </div>

    <div class="flex flex-wrap items-center gap-2">
      <!-- Database chip -->
      <div class="h-10 pl-3 pr-1.5 rounded-xl bg-[#181a20] border border-white/[0.08] flex items-center gap-2.5 text-sm">
        <svg class="w-4 h-4 text-zinc-400 shrink-0" aria-hidden="true" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <ellipse cx="12" cy="5" rx="9" ry="3" /><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5" /><path d="M3 12c0 1.66 4 3 9 3s9-1.34 9-3" />
        </svg>
        {#if dbName}
          <span class="font-mono text-zinc-100 truncate max-w-[180px]" title={currentDb?.db_path}>{dbName}</span>
          {#if currentDb?.events?.length}
            <span class="text-zinc-500 whitespace-nowrap">· {currentDb.events.length} event</span>
          {/if}
        {:else}
          <span class="text-amber-300 whitespace-nowrap">Database belum dimuat</span>
        {/if}
        <button
          type="button"
          onclick={onPickDatabase}
          class="h-7 px-2.5 rounded-lg text-xs font-semibold text-zinc-200 hover:text-white bg-white/[0.04] hover:bg-white/[0.08] border border-white/[0.1] transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
        >
          Ganti
        </button>
      </div>

      <!-- License chip -->
      <button
        type="button"
        onclick={openLicensePanel}
        class="h-10 px-3 rounded-xl border flex items-center gap-2 text-sm font-semibold transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40 {licenseActive
          ? 'bg-emerald-500/[0.08] border-emerald-500/30 text-emerald-200 hover:bg-emerald-500/[0.14]'
          : 'bg-amber-500/[0.08] border-amber-500/30 text-amber-200 hover:bg-amber-500/[0.14]'}"
      >
        {#if licenseActive}
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" /></svg>
          <span>Lisensi aktif</span>
        {:else}
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><rect x="5" y="11" width="14" height="10" rx="2" /><path d="M8 11V7a4 4 0 018 0v4" /></svg>
          <span>Aktivasi lisensi</span>
        {/if}
      </button>
    </div>
  </header>

  <main class="w-full max-w-5xl mx-auto px-6 py-8 flex-1 flex flex-col justify-center">
    <!-- Resume last session -->
    {#if lastSession && !isOurServerRunning}
      <div class="mb-6 flex items-center justify-between gap-3 rounded-xl bg-[#181a20] border border-white/[0.08] px-4 py-3">
        <div class="flex items-center gap-3 min-w-0">
          <svg class="w-5 h-5 text-zinc-400 shrink-0" aria-hidden="true" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><path stroke-linecap="round" stroke-linejoin="round" d="M12 8v4l3 2m6-2a9 9 0 11-3.2-6.9M21 3v5h-5" /></svg>
          <p class="text-sm text-zinc-300 truncate">
            Terakhir:
            <span class="text-white font-semibold">{lastSession.mode === "host" ? "Komputer utama" : "Operator tambahan"}</span>
            {#if lastSession.mode === "client" && lastSession.hostUrl}
              <span class="font-mono text-zinc-400">· {displayHost(lastSession.hostUrl)}</span>
            {/if}
            <span class="text-zinc-500">· {relativeTime(lastSession.at)}</span>
          </p>
        </div>
        <div class="flex items-center gap-1 shrink-0">
          <button
            type="button"
            onclick={forgetLastSession}
            class="h-9 px-3 rounded-xl text-xs font-semibold text-zinc-400 hover:text-white hover:bg-white/[0.06] transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
          >
            Lupakan
          </button>
          <button
            type="button"
            onclick={resumeLastSession}
            disabled={isConnectingHost}
            class="h-9 px-4 rounded-xl bg-white/[0.06] hover:bg-white/[0.1] text-white border border-white/[0.12] text-sm font-semibold transition cursor-pointer disabled:opacity-50 flex items-center gap-1.5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
          >
            Lanjutkan
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M13 7l5 5m0 0l-5 5m5-5H6" /></svg>
          </button>
        </div>
      </div>
    {/if}

    <h2 class="text-xl font-black text-white tracking-tight mb-4">Komputer ini dipakai sebagai</h2>

    <div class="grid grid-cols-1 md:grid-cols-2 gap-4 items-start">
      <!-- ============ Komputer utama ============ -->
      <section
        aria-labelledby="role-host"
        class="rounded-2xl bg-[#181a20] border p-5 flex flex-col gap-4 shadow-lg {isOurServerRunning
          ? 'border-emerald-500/40'
          : networkProblem
            ? 'border-amber-500/40'
            : 'border-white/[0.16]'}"
      >
        <div class="flex items-start gap-3">
          <div class="w-11 h-11 rounded-xl bg-white/[0.06] border border-white/[0.1] flex items-center justify-center shrink-0" aria-hidden="true">
            <svg class="w-6 h-6 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><rect x="3" y="4" width="18" height="7" rx="2" /><rect x="3" y="13" width="18" height="7" rx="2" /><path stroke-linecap="round" d="M7 7.5h.01M7 16.5h.01" /></svg>
          </div>
          <div>
            <h3 id="role-host" class="text-base font-bold text-white">Komputer utama</h3>
            <p class="text-sm text-zinc-400 mt-0.5">Pegang database, layani laptop lain</p>
          </div>
        </div>

        <!-- Network status line (icon + text, never color alone) -->
        <div class="flex items-center gap-2 text-sm" role="status">
          {#if isCheckingIps}
            <svg class="w-4 h-4 text-zinc-400 animate-spin" viewBox="0 0 24 24" fill="none" aria-hidden="true"><circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4" /><path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z" /></svg>
            <span class="text-zinc-400">Memeriksa jaringan…</span>
          {:else if isOurServerRunning}
            <span class="w-2 h-2 rounded-full bg-emerald-400 animate-pulse" aria-hidden="true"></span>
            <span class="text-emerald-300">Server sedang aktif di laptop ini{selectedIpInfo?.event_name ? ` · ${selectedIpInfo.event_name}` : ""}</span>
          {:else if selectedIpInfo?.status === "occupied"}
            <svg class="w-4 h-4 text-rose-300" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v4m0 4h.01M10.3 3.9L1.8 18a2 2 0 001.7 3h17a2 2 0 001.7-3L13.7 3.9a2 2 0 00-3.4 0z" /></svg>
            <span class="text-rose-300">Port 8000 dipakai aplikasi lain</span>
          {:else if selectedIpInfo?.status === "active_event"}
            <svg class="w-4 h-4 text-amber-300" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v4m0 4h.01M10.3 3.9L1.8 18a2 2 0 001.7 3h17a2 2 0 001.7-3L13.7 3.9a2 2 0 00-3.4 0z" /></svg>
            <span class="text-amber-300">Server lain sudah aktif di jaringan ini</span>
          {:else}
            <svg class="w-4 h-4 text-emerald-400" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" /></svg>
            <span class="text-zinc-300">Jaringan siap · {selectedIpInfo ? ipLabel(selectedIpInfo) : "IP otomatis"}</span>
          {/if}
        </div>

        {#if selectedIpInfo?.status === "active_event"}
          <p class="text-sm text-zinc-400 -mt-1">
            Untuk ikut turnamen itu, pilih <span class="text-white font-semibold">Operator tambahan</span>.
          </p>
        {/if}

        <button
          type="button"
          onclick={() => handleStartHost()}
          class="h-11 w-full bg-white text-black hover:bg-zinc-200 font-bold text-sm rounded-xl shadow-md transition active:scale-[0.99] cursor-pointer flex items-center justify-center gap-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/60 focus-visible:ring-offset-2 focus-visible:ring-offset-[#181a20]"
        >
          {isOurServerRunning ? "Lanjutkan mengelola turnamen" : "Mulai sebagai komputer utama"}
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M13 7l5 5m0 0l-5 5m5-5H6" /></svg>
        </button>

        {#if isOurServerRunning}
          <button
            type="button"
            onclick={handleStopServer}
            disabled={isStoppingServer}
            class="h-10 w-full rounded-xl text-sm font-semibold text-rose-300 hover:text-rose-200 hover:bg-rose-500/10 border border-rose-500/25 transition cursor-pointer disabled:opacity-50 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-rose-400/50"
          >
            {isStoppingServer ? "Menghentikan server…" : "Hentikan server"}
          </button>
        {/if}

        <!-- Advanced: network interface (hidden unless needed) -->
        <div class="border-t border-white/[0.06] pt-3">
          <button
            type="button"
            onclick={() => (networkOpen = !networkOpen)}
            aria-expanded={networkOpen}
            aria-controls="network-panel"
            class="w-full flex items-center justify-between text-sm text-zinc-400 hover:text-white transition cursor-pointer rounded-lg py-1 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
          >
            <span class="flex items-center gap-1.5">
              <svg class="w-4 h-4 transition-transform {networkOpen ? 'rotate-90' : ''}" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M9 5l7 7-7 7" /></svg>
              Pengaturan jaringan
            </span>
            <span class="font-mono text-xs text-zinc-500">port 8000</span>
          </button>

          {#if networkOpen}
            <div id="network-panel" class="mt-3 space-y-2">
              <div class="flex items-center justify-between">
                <span class="text-xs font-semibold text-zinc-400">Alamat jaringan</span>
                <button
                  type="button"
                  onclick={refreshIpAvailability}
                  disabled={isCheckingIps}
                  class="h-8 px-2.5 rounded-lg text-xs font-semibold text-zinc-300 hover:text-white bg-white/[0.04] hover:bg-white/[0.08] border border-white/[0.1] transition cursor-pointer disabled:opacity-50 flex items-center gap-1.5 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
                >
                  <svg class="w-3.5 h-3.5 {isCheckingIps ? 'animate-spin' : ''}" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" /></svg>
                  {isCheckingIps ? "Memindai…" : "Pindai ulang"}
                </button>
              </div>

              <div role="radiogroup" aria-label="Alamat jaringan server" class="space-y-1.5 max-h-56 overflow-y-auto pr-1">
                {#if ipAvailabilities.length === 0 && !isCheckingIps}
                  <p class="text-sm text-zinc-500 px-1 py-2">Belum ada alamat jaringan terdeteksi. Klik Pindai ulang.</p>
                {/if}
                {#each ipAvailabilities as item}
                  {@const selected = selectedHostIp === item.ip}
                  <button
                    type="button"
                    role="radio"
                    aria-checked={selected}
                    onclick={() => (selectedHostIp = item.ip)}
                    class="w-full text-left px-3 py-2.5 rounded-xl border transition flex items-center justify-between gap-2 text-sm cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40 {selected
                      ? 'bg-white/[0.08] border-white/40'
                      : 'bg-[#14161b] hover:bg-white/[0.04] border-white/[0.06]'}"
                  >
                    <span class="flex items-center gap-2.5 min-w-0">
                      <span class="w-4 h-4 rounded-full border flex items-center justify-center shrink-0 {selected ? 'border-white bg-white' : 'border-zinc-600'}" aria-hidden="true">
                        {#if selected}<span class="w-1.5 h-1.5 rounded-full bg-black"></span>{/if}
                      </span>
                      <span class="min-w-0">
                        <span class="block font-mono font-semibold text-zinc-100">{ipLabel(item)}</span>
                        {#if item.ip !== "0.0.0.0"}<span class="block text-xs text-zinc-500 truncate">{item.name}</span>{/if}
                      </span>
                    </span>
                    <span class="shrink-0 text-xs font-semibold">
                      {#if item.status === "current_host" || item.is_current_host}
                        <span class="text-emerald-300">Aktif di sini</span>
                      {:else if item.status === "active_event"}
                        <span class="text-amber-300">Server lain</span>
                      {:else if item.status === "occupied"}
                        <span class="text-rose-300">Port dipakai</span>
                      {:else}
                        <span class="text-zinc-400">Siap</span>
                      {/if}
                    </span>
                  </button>
                {/each}
              </div>
            </div>
          {/if}
        </div>
      </section>

      <!-- ============ Operator tambahan ============ -->
      <section aria-labelledby="role-client" class="rounded-2xl bg-[#181a20] border border-white/[0.1] p-5 flex flex-col gap-4 shadow-lg">
        <div class="flex items-start gap-3">
          <div class="w-11 h-11 rounded-xl bg-white/[0.06] border border-white/[0.1] flex items-center justify-center shrink-0" aria-hidden="true">
            <svg class="w-6 h-6 text-zinc-200" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2"><rect x="4" y="5" width="16" height="11" rx="2" /><path stroke-linecap="round" d="M2 20h20" /></svg>
          </div>
          <div>
            <h3 id="role-client" class="text-base font-bold text-white">Operator tambahan</h3>
            <p class="text-sm text-zinc-400 mt-0.5">Gabung ke komputer utama</p>
          </div>
        </div>

        {#if detectedActiveEvent}
          <div class="rounded-xl bg-blue-500/10 border border-blue-500/30 px-3.5 py-3 flex items-center justify-between gap-3">
            <div class="min-w-0">
              <p class="text-sm font-semibold text-blue-200 flex items-center gap-1.5">
                <svg class="w-4 h-4 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M5 12.55a11 11 0 0114 0M8.5 16a6 6 0 017 0M12 20h.01M2 8.82a15 15 0 0120 0" /></svg>
                <span class="truncate">{detectedActiveEvent.event_name || "Server Ambrilindo"}</span>
              </p>
              <p class="text-xs font-mono text-blue-300/80 mt-0.5">{detectedActiveEvent.ip}</p>
            </div>
            <button
              type="button"
              onclick={() => handleJoinHost(`http://${detectedActiveEvent.ip}:8000`)}
              disabled={isConnectingHost}
              class="h-9 px-4 rounded-xl bg-blue-500 hover:bg-blue-400 text-white font-bold text-sm transition cursor-pointer disabled:opacity-50 shrink-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-blue-300"
            >
              Gabung
            </button>
          </div>
        {/if}

        <form
          class="space-y-2"
          onsubmit={(e) => {
            e.preventDefault();
            handleJoinHost();
          }}
        >
          <label for="host-ip-input" class="block text-sm font-semibold text-zinc-300">Alamat komputer utama</label>
          <div class="flex gap-2">
            <input
              id="host-ip-input"
              type="text"
              inputmode="decimal"
              autocomplete="off"
              spellcheck="false"
              bind:value={hostInputUrl}
              oninput={() => (hostError = null)}
              placeholder="192.168.1.5"
              aria-invalid={!!hostError}
              aria-describedby={hostError ? "host-error" : "host-hint"}
              class="flex-1 min-w-0 h-11 bg-[#14161b] border text-white text-sm font-mono px-3.5 rounded-xl outline-none transition select-text {hostError
                ? 'border-rose-500/60'
                : 'border-white/[0.12] focus:border-white'}"
            />
            <button
              type="submit"
              disabled={isConnectingHost}
              class="h-11 px-4 rounded-xl bg-white/[0.06] hover:bg-white/[0.1] text-white border border-white/[0.12] font-semibold text-sm transition cursor-pointer disabled:opacity-50 shrink-0 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
            >
              {isConnectingHost ? "Menghubungkan…" : "Hubungkan"}
            </button>
          </div>
          {#if hostError}
            <p id="host-error" role="alert" class="text-sm text-rose-300">{hostError}</p>
          {:else}
            <p id="host-hint" class="text-xs text-zinc-500">Alamat IP tertera di layar komputer utama.</p>
          {/if}
        </form>
      </section>
    </div>
  </main>

  <footer class="w-full max-w-5xl mx-auto px-6 py-4 border-t border-white/[0.08] flex items-center justify-between gap-2 text-xs text-zinc-500">
    <span>Ambrilindo Desktop v3.0</span>
    <span>FORKI &amp; WKF</span>
  </footer>
</div>

<style>
  @media (prefers-reduced-motion: reduce) {
    .landing :global(.animate-spin),
    .landing :global(.animate-pulse) {
      animation: none;
    }
  }
</style>
