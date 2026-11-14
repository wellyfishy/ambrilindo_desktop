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

  let hostInputUrl = $state("http://192.168.1.");
  let isConnectingHost = $state(false);
  let hostError = $state<string | null>(null);

  // Real-time IP Availability State
  let ipAvailabilities = $state<IpAvailabilityInfo[]>([]);
  let isCheckingIps = $state(false);
  let isStoppingServer = $state(false);
  let selectedHostIp = $state<string>("0.0.0.0");

  let isOurServerRunning = $derived(
    ipAvailabilities.some((item) => item.status === "current_host" || item.is_current_host)
  );

  let detectedActiveEvent = $derived(
    ipAvailabilities.find((item) => item.status === "active_event" && !item.is_current_host && item.event_name) ||
    ipAvailabilities.find((item) => item.status === "active_event" && !item.is_current_host)
  );

  let selectedIpInfo = $derived(
    ipAvailabilities.find((item) => item.ip === selectedHostIp) || null
  );

  onMount(async () => {
    await refreshIpAvailability();
  });

  async function refreshIpAvailability() {
    isCheckingIps = true;
    try {
      const list = await invoke<IpAvailabilityInfo[]>("check_ip_availability");
      if (Array.isArray(list) && list.length > 0) {
        ipAvailabilities = list;
        if (!selectedHostIp || selectedHostIp === "0.0.0.0") {
          const hasCurrentHost = list.find((i) => i.status === "current_host" || i.is_current_host);
          if (hasCurrentHost) {
            // Keep current selection
          } else {
            const hasZero = list.find((i) => i.ip === "0.0.0.0");
            if (hasZero && hasZero.status === "available") {
              selectedHostIp = "0.0.0.0";
            } else {
              const firstAvail = list.find((i) => i.status === "available");
              if (firstAvail) {
                selectedHostIp = firstAvail.ip;
              }
            }
          }
        }
      }
    } catch (_) {
      // Fallback from networkInfo
      if (networkInfo?.available_ips) {
        ipAvailabilities = [
          {
            ip: "0.0.0.0",
            name: "Semua Interface (Default)",
            status: "available",
            event_name: null,
            is_default: true,
          },
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
        onSelectMode("client", cleanUrl);
      } else {
        throw new Error("Respon host tidak valid");
      }
    } catch (e: any) {
      hostError = `Gagal terhubung ke host (${cleanUrl}): ${e.message || "Timeout / Offline"}`;
    } finally {
      isConnectingHost = false;
    }
  }

  function handleStartHost() {
    onSelectMode("host", undefined, selectedHostIp);
  }
</script>

<div class="h-full w-full min-h-screen flex flex-col justify-between p-6 md:p-8 select-none app-frame text-zinc-200 overflow-y-auto font-sans relative">
  <!-- Top Operational Header Bar -->
  <header class="w-full max-w-6xl mx-auto flex flex-col sm:flex-row items-start sm:items-center justify-between gap-4 border-b border-white/[0.08] pb-5 shrink-0">
    <div class="flex items-center space-x-3.5">
      <div class="w-10 h-10 rounded-xl bg-white text-black font-black flex items-center justify-center text-sm shadow-md shrink-0">
        <svg class="w-5 h-5 text-black" aria-hidden="true" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round">
          <path d="M12 2L3 7v6c0 5.55 3.84 10.74 9 12 5.16-1.26 9-6.45 9-12V7l-9-5z"/>
          <path d="M12 8v8"/>
          <path d="M8 12h8"/>
        </svg>
      </div>
      <div>
        <div class="flex items-center space-x-2">
          <h1 class="text-xl font-black text-white tracking-tight">AMBRILINDO ARENA</h1>
          <span class="px-2 py-0.5 rounded text-[11px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1]">
            v3.0
          </span>
        </div>
        <p class="text-xs text-zinc-400 mt-0.5 font-normal">Sistem Manajemen &amp; Scoring Turnamen Karate</p>
      </div>
    </div>

    <div class="flex items-center gap-2">
    <!-- Active DB & Environment Card -->
    <div class="flex items-center gap-3 surface-card px-4 py-2 rounded-xl border border-white/[0.08] shadow-sm">
      <div class="flex items-center gap-2.5">
        <svg class="w-4 h-4 text-zinc-400 shrink-0" aria-hidden="true" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <ellipse cx="12" cy="5" rx="9" ry="3"/>
          <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/>
          <path d="M3 12c0 1.66 4 3 9 3s9-1.34 9-3"/>
        </svg>
        <div class="text-left leading-tight">
          <div class="flex items-center gap-1.5">
            <span class="text-[10px] font-bold text-zinc-400">Database Turnamen</span>
            {#if currentDb?.events?.length}
              <span class="px-1.5 py-0.2 rounded text-[10px] font-semibold bg-white/[0.04] text-zinc-400 border border-white/[0.06]">
                {currentDb.events.length} event
              </span>
            {/if}
          </div>
          <span class="text-xs font-mono font-semibold text-zinc-200 truncate max-w-[220px] block mt-0.5">
            {currentDb ? currentDb.db_path.split(/[/\\]/).pop() : "Belum Dimuat"}
          </span>
        </div>
      </div>
      <button
        onclick={onPickDatabase}
        class="h-8 px-3 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white text-xs font-semibold border border-white/[0.1] transition-colors outline-none focus-visible:ring-2 focus-visible:ring-white/30 whitespace-nowrap cursor-pointer"
      >
        <span>Ganti</span>
      </button>
    </div>
    <button
      type="button"
      onclick={openLicensePanel}
      class="h-11 px-3.5 rounded-xl border flex items-center gap-2 text-xs font-semibold transition cursor-pointer {license.status?.state === 'active'
        ? 'bg-emerald-500/[0.06] border-emerald-500/30 text-emerald-200 hover:bg-emerald-500/10'
        : 'bg-amber-500/[0.06] border-amber-500/30 text-amber-200 hover:bg-amber-500/10'}"
    >
      <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true">
        <path stroke-linecap="round" stroke-linejoin="round" d="M15 7a2 2 0 012 2m4 0a6 6 0 01-7.743 5.743L11 17H9v2H7v2H4a1 1 0 01-1-1v-2.586a1 1 0 01.293-.707l5.964-5.964A6 6 0 1121 9z" />
      </svg>
      <span>{license.status?.state === "active" ? "Lisensi Aktif" : "Aktivasi Lisensi"}</span>
    </button>
    </div>
  </header>

  <!-- Main 2-Option Command Panels -->
  <main class="w-full max-w-5xl mx-auto my-auto py-6 shrink-0">
    <div class="flex items-center justify-between mb-6">
      <div>
        <div class="flex items-center space-x-2">
          <h2 class="text-xl font-black text-white tracking-tight uppercase">PILIH PERAN KOMPUTER</h2>
          <span class="px-2 py-0.5 rounded text-[11px] font-semibold bg-white/[0.06] text-zinc-300 border border-white/[0.1]">
            2 Pilihan Utama
          </span>
        </div>
        <p class="text-xs text-zinc-400 mt-1 font-normal">
          Tentukan peran laptop ini di arena pertandingan (Komputer Utama vs Operator Tambahan)
        </p>
      </div>

      <button
        onclick={refreshIpAvailability}
        disabled={isCheckingIps}
        class="flex items-center gap-2 px-3 py-1.5 rounded-xl bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white border border-white/[0.08] text-xs transition"
        title="Pindai ulang ketersediaan interface IP dan port 8000"
      >
        <svg class="w-3.5 h-3.5 {isCheckingIps ? 'animate-spin' : ''}" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
        <span>{isCheckingIps ? "Memindai IP…" : "Refresh Jaringan"}</span>
      </button>
    </div>

    <div class="grid grid-cols-1 md:grid-cols-2 gap-6 items-stretch">
      <!-- Card 1: Komputer Utama (Server & Operator Cockpit) -->
      <div class="surface-card border {selectedIpInfo?.status === 'active_event' ? 'border-amber-500/40 ring-1 ring-amber-500/30' : 'border-white/[0.12]'} rounded-2xl p-6 flex flex-col justify-between shadow-xl relative">
        <div>
          <div class="flex items-center justify-between mb-4">
            <span class="px-2.5 py-1 rounded-full text-[10px] font-bold bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 flex items-center gap-1.5">
              <span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
              <span>OPSI 1 &bull; DIREKOMENDASIKAN</span>
            </span>
            <span class="text-[10px] font-mono text-zinc-500">Port :8000</span>
          </div>

          <h3 class="text-lg font-black text-white tracking-tight">Komputer Utama (Server &amp; Operator)</h3>
          <p class="text-xs text-zinc-400 mt-1 leading-relaxed">
            Membuka database master turnamen. Laptop ini berfungsi sebagai server hub LAN sekaligus meja operator scoring, drawing bagan, dan proyektor HDMI.
          </p>

          <!-- IP Availability & Interface Picker -->
          <div class="mt-5 space-y-2.5">
            <div class="flex items-center justify-between">
              <label for="ip-select-group" class="text-[10px] font-bold text-zinc-300 uppercase tracking-wider block">
                Pilih IP Interface Ruang Pertandingan
              </label>
              <span class="text-[10px] text-zinc-500 font-mono">Bind 0.0.0.0</span>
            </div>

            <div id="ip-select-group" class="space-y-1.5 max-h-56 overflow-y-auto pr-1">
              {#each ipAvailabilities as item}
                <button
                  type="button"
                  onclick={() => (selectedHostIp = item.ip)}
                  class="w-full text-left p-2.5 rounded-xl border transition-all flex items-center justify-between text-xs {selectedHostIp === item.ip
                    ? 'bg-white/[0.08] border-white/40 ring-1 ring-white/20'
                    : 'bg-[#121318] hover:bg-white/[0.04] border-white/[0.06] text-zinc-300'}"
                >
                  <div class="flex items-center gap-2.5 truncate mr-2">
                    <span class="w-3.5 h-3.5 rounded-full flex items-center justify-center shrink-0 border {selectedHostIp === item.ip ? 'border-white bg-white text-black' : 'border-zinc-600'}">
                      {#if selectedHostIp === item.ip}
                        <span class="w-1.5 h-1.5 rounded-full bg-black"></span>
                      {/if}
                    </span>
                    <div class="truncate">
                      <div class="font-mono font-bold text-zinc-100 flex items-center gap-2">
                        <span>{item.ip}</span>
                        {#if item.ip === "0.0.0.0"}
                          <span class="text-[9px] font-sans px-1 rounded bg-white/[0.08] text-zinc-300">Default Semua</span>
                        {/if}
                      </div>
                      <span class="text-[10px] text-zinc-500 truncate block mt-0.5">{item.name}</span>
                    </div>
                  </div>

                  <!-- Status Pill -->
                  <div class="shrink-0">
                    {#if item.status === "current_host" || item.is_current_host}
                      <span class="px-2 py-0.5 rounded text-[10px] font-bold bg-emerald-500/20 text-emerald-300 border border-emerald-500/40 flex items-center gap-1">
                        <span class="w-1.5 h-1.5 rounded-full bg-emerald-400 animate-pulse"></span>
                        <span>Aktif (Server Anda)</span>
                      </span>
                    {:else if item.status === "active_event"}
                      <span class="px-2 py-0.5 rounded text-[10px] font-bold bg-amber-500/15 text-amber-300 border border-amber-500/30 flex items-center gap-1" title="Terdeteksi server Ambrilindo lain aktif">
                        <span>(!)</span>
                        <span>Event Aktif</span>
                      </span>
                    {:else if item.status === "occupied"}
                      <span class="px-2 py-0.5 rounded text-[10px] font-bold bg-rose-500/15 text-rose-300 border border-rose-500/30 flex items-center gap-1" title="Port 8000 sedang terpakai aplikasi lain">
                        <span>(!)</span>
                        <span>Port Sibuk</span>
                      </span>
                    {:else}
                      <span class="px-2 py-0.5 rounded text-[10px] font-bold bg-emerald-500/15 text-emerald-400 border border-emerald-500/25 flex items-center gap-1">
                        <span>✓</span>
                        <span>Siap</span>
                      </span>
                    {/if}
                  </div>
                </button>
              {/each}
            </div>

            <!-- Active Status / Collision Warning Banner -->
            {#if selectedIpInfo?.status === "current_host" || selectedIpInfo?.is_current_host}
              <div class="p-3.5 rounded-xl bg-emerald-500/15 border border-emerald-500/30 text-emerald-300 text-xs mt-3">
                <div class="flex items-center justify-between">
                  <div class="flex items-center gap-2 font-bold">
                    <span class="w-2 h-2 rounded-full bg-emerald-400 animate-pulse"></span>
                    <span>Server Aktif di Laptop Ini</span>
                  </div>
                  <span class="text-[10px] font-mono bg-emerald-500/20 px-1.5 py-0.5 rounded text-emerald-200">
                    Port 8000
                  </span>
                </div>
                <p class="text-[11px] text-zinc-300 mt-1.5 leading-snug">
                  Room turnamen {selectedIpInfo.event_name ? `"${selectedIpInfo.event_name}"` : ""} sedang aktif melayani koneksi. Anda dapat melanjutkan ke panel kendali atau menghentikan server jika ingin memulai ulang.
                </p>
              </div>
            {:else if selectedIpInfo?.status === "active_event"}
              <div class="p-3.5 rounded-xl bg-amber-500/10 border border-amber-500/30 text-amber-300 text-xs mt-3">
                <div class="flex items-center gap-1.5 font-bold">
                  <span class="w-2 h-2 rounded-full bg-amber-400"></span>
                  <span>Peringatan Tabrakan Host:</span>
                </div>
                <p class="text-[11px] text-zinc-300 mt-1 leading-snug">
                  Terdeteksi server turnamen lain aktif di jaringan ini ({selectedIpInfo.event_name ? `"${selectedIpInfo.event_name}"` : selectedIpInfo.ip}).
                  Jika Anda ingin bergabung ke turnamen tersebut, gunakan opsi <b>Operator Tambahan</b> di sebelah kanan.
                </p>
              </div>
            {/if}
          </div>
        </div>

        <div class="mt-6 space-y-2">
          <button
            onclick={handleStartHost}
            class="h-11 w-full bg-white text-black hover:bg-zinc-200 font-black text-xs rounded-xl shadow-lg transition-all active:scale-[0.99] outline-none focus-visible:ring-2 focus-visible:ring-white/50 cursor-pointer flex items-center justify-center gap-2"
          >
            {#if isOurServerRunning}
              <span>Lanjutkan Mengelola Turnamen</span>
              <svg class="w-4 h-4 text-black" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
                <path stroke-linecap="round" stroke-linejoin="round" d="M13 7l5 5m0 0l-5 5m5-5H6"/>
              </svg>
            {:else}
              <span>Mulai Sebagai Komputer Utama</span>
              <svg class="w-4 h-4 text-black" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
                <path stroke-linecap="round" stroke-linejoin="round" d="M14 5l7 7m0 0l-7 7m7-7H3"/>
              </svg>
            {/if}
          </button>

          {#if isOurServerRunning}
            <button
              type="button"
              onclick={handleStopServer}
              disabled={isStoppingServer}
              class="w-full py-2 rounded-xl text-xs font-semibold text-rose-400 hover:text-rose-300 hover:bg-rose-500/10 border border-rose-500/25 transition cursor-pointer flex items-center justify-center gap-1.5"
            >
              <span>{isStoppingServer ? "Menghentikan Server…" : "Hentikan Server & Tutup Room"}</span>
            </button>
          {/if}
        </div>
      </div>

      <!-- Card 2: Operator Tambahan (Gabung LAN Client) -->
      <div class="surface-card border border-white/[0.12] rounded-2xl p-6 flex flex-col justify-between shadow-xl relative">
        <div>
          <div class="flex items-center justify-between mb-4">
            <span class="px-2.5 py-1 rounded-full text-[10px] font-bold bg-blue-500/10 text-blue-400 border border-blue-500/20">
              OPSI 2 &bull; LAPTOP KEDUA / TATAMI 2-6
            </span>
            <span class="text-[10px] font-mono text-zinc-500">Client Mode</span>
          </div>

          <h3 class="text-lg font-black text-white tracking-tight">Operator Tambahan (Gabung LAN)</h3>
          <p class="text-xs text-zinc-400 mt-1 leading-relaxed">
            Terhubung ke Komputer Utama di venue turnamen. Cocok untuk laptop operator Tatami 2, Tatami 3, meja pendaftaran, atau sekretariat pertandingan.
          </p>

          <!-- Quick Join Banner if another host was probed -->
          {#if detectedActiveEvent}
            <div class="mt-5 p-3.5 rounded-xl bg-blue-500/10 border border-blue-500/30 text-blue-300 text-xs">
              <div class="flex items-center justify-between">
                <div class="flex items-center gap-1.5 font-bold">
                  <span class="w-2 h-2 rounded-full bg-blue-400 animate-pulse"></span>
                  <span>Server Turnamen Terdeteksi!</span>
                </div>
                <span class="text-[10px] font-mono bg-blue-500/20 px-1.5 py-0.5 rounded text-blue-200">
                  {detectedActiveEvent.ip}
                </span>
              </div>
              <p class="text-[11px] text-zinc-300 mt-1">
                {detectedActiveEvent.event_name ? `Event: ${detectedActiveEvent.event_name}` : "Ambrilindo Arena Host Siap"}
              </p>
              <button
                type="button"
                onclick={() => handleJoinHost(`http://${detectedActiveEvent.ip}:8000`)}
                disabled={isConnectingHost}
                class="mt-2.5 w-full py-2 rounded-lg bg-blue-500 hover:bg-blue-400 text-white font-bold text-xs shadow-md transition cursor-pointer flex items-center justify-center gap-1.5"
              >
                <span>⚡ Gabung Cepat ke Server Ini</span>
              </button>
            </div>
          {/if}

          <!-- Manual Host Input -->
          <div class="mt-5 space-y-2">
            <label for="host-ip-input" class="text-[10px] font-bold text-zinc-300 uppercase tracking-wider block">
              Alamat Host Server Manual
            </label>
            <div class="relative">
              <input
                id="host-ip-input"
                type="text"
                bind:value={hostInputUrl}
                placeholder="http://192.168.1.5:8000"
                class="w-full surface-input border border-white/[0.12] focus:border-white text-white text-xs font-mono px-3.5 py-2.5 rounded-xl outline-none transition"
              />
            </div>
            {#if hostError}
              <p role="alert" class="text-[11px] text-rose-400 font-mono mt-1 break-all">{hostError}</p>
            {/if}
            <p class="text-[11px] text-zinc-500">
              Ketik alamat IP laptop Komputer Utama yang tertera pada layar laptop 1 atau scan QR.
            </p>
          </div>
        </div>

        <button
          onclick={() => handleJoinHost()}
          disabled={isConnectingHost}
          class="mt-6 h-11 w-full bg-white/[0.06] hover:bg-white/[0.1] text-white font-bold text-xs rounded-xl border border-white/[0.12] transition-all active:scale-[0.99] disabled:opacity-40 cursor-pointer flex items-center justify-center gap-2"
        >
          <span>{isConnectingHost ? "Menghubungkan ke Host…" : "Hubungkan ke Host Server"}</span>
          <svg class="w-4 h-4 text-zinc-300" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
            <path stroke-linecap="round" stroke-linejoin="round" d="M14 5l7 7m0 0l-7 7m7-7H3"/>
          </svg>
        </button>
      </div>
    </div>
  </main>

  <!-- Bottom Operational Status Footer -->
  <footer class="w-full max-w-5xl mx-auto pt-4 border-t border-white/[0.08] flex flex-col sm:flex-row items-center justify-between gap-2 text-xs text-zinc-400 shrink-0">
    <div class="flex items-center space-x-3">
      <span class="w-2 h-2 rounded-full bg-emerald-400 shrink-0"></span>
      <span class="text-[11px] text-zinc-300">Siap Operasional Venue</span>
      <span class="text-zinc-600 font-mono">/</span>
      <span class="text-[11px] text-zinc-400">Database SQLite Terintegrasi</span>
    </div>
    <span class="text-[11px] text-zinc-500">Ambrilindo Desktop v3.0 &bull; FORKI &amp; WKF Tournament System</span>
  </footer>
</div>

<style>
  :root {
    --bg-app: #101217;
    --bg-card: #15181f;
    --bg-input: #0e0f14;
    --text-app: #f4f4f5;
  }
  .app-frame {
    background-color: var(--bg-app);
    color: var(--text-app);
  }
  .surface-card {
    background-color: var(--bg-card);
    color: var(--text-app);
  }
  .surface-input {
    background-color: var(--bg-input);
    color: var(--text-app);
  }
</style>
