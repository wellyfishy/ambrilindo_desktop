<script lang="ts">
  import type { AppMode, DatabaseSummary, NetworkInfo } from "$lib/types";

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

  // Available IPs for creating a room
  let availableIps = $derived(networkInfo?.available_ips || []);
  let selectedHostIp = $state<string>("");

  $effect(() => {
    if (networkInfo?.local_ip && !selectedHostIp) {
      selectedHostIp = networkInfo.local_ip;
    }
  });

  async function handleJoinHost() {
    let cleanUrl = hostInputUrl.trim();
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
          <h1 class="text-xl font-black text-white tracking-tight">AMBRILINDO</h1>
          <span class="px-2 py-0.5 rounded text-[11px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1]">
            v3.0
          </span>
        </div>
        <p class="text-xs text-zinc-400 mt-0.5 font-normal">Karate Tournament Operations System</p>
      </div>
    </div>

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
            <span class="text-[10px] font-bold text-zinc-400">Database aktif</span>
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
        class="h-8 px-3 rounded-lg bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white text-xs font-semibold border border-white/[0.1] transition-colors motion-reduce:transition-none outline-none focus-visible:ring-2 focus-visible:ring-white/30 whitespace-nowrap"
      >
        <span>Ganti</span>
      </button>
    </div>
  </header>

  <!-- Arena Mobile Quick Jump Strip -->
  <section aria-label="Akses arena cepat" class="w-full max-w-6xl mx-auto my-5 p-3.5 rounded-xl surface-card border border-white/[0.08] shadow-sm flex flex-col sm:flex-row items-center justify-between gap-3 shrink-0">
    <div class="flex items-center gap-2.5">
      <span class="px-2 py-0.5 rounded text-[11px] font-semibold bg-white/[0.06] text-zinc-300 border border-white/[0.1]">
        Venue Mobile
      </span>
      <span class="text-xs font-semibold text-zinc-300">Akses Cepat Wasit &amp; LO</span>
    </div>
    <div class="flex items-center gap-2 shrink-0 flex-wrap">
      <a
        href="/jury/1"
        class="h-8 px-3.5 rounded-xl bg-white text-black hover:bg-zinc-200 font-bold text-xs flex items-center gap-2 shadow-sm transition-colors motion-reduce:transition-none active:scale-[0.98] outline-none focus-visible:ring-2 focus-visible:ring-white/40 whitespace-nowrap"
      >
        <span>Juri Tatami 1</span>
        <svg class="w-3.5 h-3.5 text-black" aria-hidden="true" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M14 5l7 7m0 0l-7 7m7-7H3"/>
        </svg>
      </a>
      <a
        href="/lo-kata/1"
        class="h-8 px-3.5 rounded-xl bg-white/[0.06] hover:bg-white/[0.1] text-white border border-white/[0.1] font-semibold text-xs flex items-center gap-2 transition-colors motion-reduce:transition-none active:scale-[0.98] outline-none focus-visible:ring-2 focus-visible:ring-white/30 whitespace-nowrap"
      >
        <span>LO Tatami 1</span>
        <svg class="w-3.5 h-3.5 text-zinc-300" aria-hidden="true" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M14 5l7 7m0 0l-7 7m7-7H3"/>
        </svg>
      </a>
      <a
        href="/coach-supervisor/1"
        class="h-8 px-3 rounded-xl bg-red-500/15 hover:bg-red-500/25 text-red-300 border border-red-500/30 font-semibold text-xs flex items-center gap-1.5 transition-colors motion-reduce:transition-none active:scale-[0.98] outline-none focus-visible:ring-2 focus-visible:ring-red-500/40 whitespace-nowrap"
        title="Panel Pengawas Pelatih Video Review"
      >
        <span class="w-1.5 h-1.5 rounded-full bg-red-400"></span>
        <span>Coach VR</span>
      </a>
      <a
        href="/wasit-vr/1"
        class="h-8 px-3 rounded-xl bg-amber-500/15 hover:bg-amber-500/25 text-amber-300 border border-amber-500/30 font-semibold text-xs flex items-center gap-1.5 transition-colors motion-reduce:transition-none active:scale-[0.98] outline-none focus-visible:ring-2 focus-visible:ring-amber-500/40 whitespace-nowrap"
        title="Layar Meja Wasit Video Review"
      >
        <span class="w-1.5 h-1.5 rounded-full bg-amber-400"></span>
        <span>Wasit VR</span>
      </a>

      <a
        href="/marshalling"
        class="h-8 px-3 rounded-xl bg-purple-500/15 hover:bg-purple-500/25 text-purple-300 border border-purple-500/30 font-semibold text-xs flex items-center gap-1.5 transition-colors motion-reduce:transition-none active:scale-[0.98] outline-none focus-visible:ring-2 focus-visible:ring-purple-500/40 whitespace-nowrap"
        title="Layar Panggilan Atlet & Antrean Ruang Pemanasan (Warm-up Hall)"
      >
        <span class="w-1.5 h-1.5 rounded-full bg-purple-400"></span>
        <span>Marshalling</span>
      </a>
      <a
        href="/results-booklet"
        class="h-8 px-3 rounded-xl bg-emerald-500/15 hover:bg-emerald-500/25 text-emerald-300 border border-emerald-500/30 font-semibold text-xs flex items-center gap-1.5 transition-colors motion-reduce:transition-none active:scale-[0.98] outline-none focus-visible:ring-2 focus-visible:ring-emerald-500/40 whitespace-nowrap"
        title="Buku Laporan Hasil Pertandingan & Lembar Berita Acara Resmi"
      >
        <span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
        <span>Buku Hasil (BA)</span>
      </a>
      <a
        href="/jury"
        class="h-8 px-3.5 rounded-xl bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white border border-white/[0.08] text-xs font-semibold flex items-center transition-colors motion-reduce:transition-none outline-none focus-visible:ring-2 focus-visible:ring-white/20 whitespace-nowrap"
      >
        <span>Pilih Tatami…</span>
      </a>
    </div>
  </section>

  <!-- Main Center Options: 3 Operational Command Panels -->
  <main class="w-full max-w-6xl mx-auto my-auto py-2 shrink-0">
    <div class="flex items-center justify-between mb-5">
      <div>
        <div class="flex items-center space-x-2">
          <h2 class="text-xl font-black text-white tracking-tight uppercase">PILIH MODE OPERASI</h2>
          <span class="px-2 py-0.5 rounded text-[11px] font-semibold bg-white/[0.06] text-zinc-300 border border-white/[0.1]">
            3 mode tersedia
          </span>
        </div>
        <p class="text-xs text-zinc-400 mt-1 font-normal">Pilih konfigurasi runtime sesuai posisi workstation di venue turnamen</p>
      </div>

      {#if networkInfo?.local_ip}
        <div class="hidden md:flex items-center gap-2 surface-card border border-white/[0.08] px-3 py-1.5 rounded-lg text-xs text-zinc-300">
          <span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
          <span>LAN IP: <span class="font-mono">{networkInfo.local_ip}</span></span>
        </div>
      {/if}
    </div>

    <div class="grid grid-cols-1 md:grid-cols-3 gap-4 items-stretch">
      <!-- Card 1: Mode Offline (Standalone) -->
      <div class="surface-card border border-white/[0.08] hover:border-white/[0.16] rounded-2xl p-5 flex flex-col justify-between shadow-md transition-colors motion-reduce:transition-none">
        <div>
          <div class="flex items-center justify-between mb-4">
            <span class="px-2 py-0.5 rounded text-[10px] font-bold bg-white/[0.04] text-zinc-400 border border-white/[0.08]">
              Standalone
            </span>
          </div>

          <h3 class="text-base font-black text-white tracking-tight">Offline</h3>

          <div class="mt-4 space-y-2 text-xs text-zinc-400">
            <div class="p-2.5 rounded-xl surface-input border border-white/[0.06] space-y-1.5 text-[11px]">
              <div class="flex items-center justify-between text-zinc-300">
                <span class="text-zinc-500">Engine</span>
                <span>SQLite Lokal</span>
              </div>
              <div class="flex items-center justify-between text-zinc-300">
                <span class="text-zinc-500">Jaringan</span>
                <span>Zero LAN</span>
              </div>
              <div class="flex items-center justify-between text-zinc-300">
                <span class="text-zinc-500">Fungsi</span>
                <span>Bagan &amp; Scoring 1 PC</span>
              </div>
            </div>
          </div>
        </div>

        <button
          onclick={() => onSelectMode('solo')}
          class="mt-6 h-10 w-full bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors motion-reduce:transition-none active:scale-[0.99] outline-none focus-visible:ring-2 focus-visible:ring-white/30 whitespace-nowrap"
        >
          <span>Masuk Mode Offline</span>
        </button>
      </div>

      <!-- Card 2: Gabung Room (LAN Client) -->
      <div class="surface-card border border-white/[0.08] hover:border-white/[0.16] rounded-2xl p-5 flex flex-col justify-between shadow-md transition-colors motion-reduce:transition-none">
        <div>
          <div class="flex items-center justify-between mb-4">
            <span class="px-2 py-0.5 rounded text-[10px] font-bold bg-white/[0.04] text-zinc-400 border border-white/[0.08]">
              Client LAN
            </span>
          </div>

          <h3 class="text-base font-black text-white tracking-tight">Gabung Room</h3>

          <div class="mt-4 space-y-2">
            <label for="host-ip-input" class="text-[10px] font-bold text-zinc-400 block">
              URL Host Server
            </label>
            <input
              id="host-ip-input"
              type="text"
              bind:value={hostInputUrl}
              placeholder="http://192.168.1.5:8000"
              class="w-full surface-input border border-white/[0.1] focus:border-white text-white text-xs font-mono px-3.5 py-2 rounded-xl outline-none transition-colors motion-reduce:transition-none focus-visible:ring-2 focus-visible:ring-white/30"
            />
            {#if hostError}
              <p role="alert" class="text-[11px] text-rose-400 font-mono mt-1 break-all">{hostError}</p>
            {/if}
          </div>
        </div>

        <button
          onclick={handleJoinHost}
          disabled={isConnectingHost}
          class="mt-6 h-10 w-full bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors motion-reduce:transition-none active:scale-[0.99] disabled:opacity-40 outline-none focus-visible:ring-2 focus-visible:ring-white/30 whitespace-nowrap"
        >
          <span>{isConnectingHost ? "Menghubungkan…" : "Hubungkan ke Host"}</span>
        </button>
      </div>

      <!-- Card 3: Buat Room (LAN Host Server) -->
      <div class="surface-card border border-white/[0.08] hover:border-white/[0.16] rounded-2xl p-5 flex flex-col justify-between shadow-md transition-colors motion-reduce:transition-none">
        <div>
          <div class="flex items-center justify-between mb-4">
            <span class="px-2 py-0.5 rounded text-[10px] font-bold bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
              Host Server :8000
            </span>
          </div>

          <h3 class="text-base font-black text-white tracking-tight">Buat Room</h3>

          <!-- IP Address Selector -->
          <div class="mt-4 space-y-2">
            <label for="host-ip-select" class="text-[10px] font-bold text-zinc-400 block">
              IP Interface Jaringan
            </label>
            {#if availableIps.length > 1}
              <div class="relative">
                <select
                  id="host-ip-select"
                  bind:value={selectedHostIp}
                  class="w-full surface-input border border-white/[0.1] hover:border-white/20 focus:border-white text-white text-xs font-mono px-3.5 py-2 pr-8 rounded-xl outline-none transition-colors motion-reduce:transition-none appearance-none cursor-pointer focus-visible:ring-2 focus-visible:ring-white/30"
                >
                  {#each availableIps as item}
                    <option value={item.ip} class="text-white py-1">
                      {item.ip} ({item.name})
                    </option>
                  {/each}
                </select>
                <div class="pointer-events-none absolute inset-y-0 right-0 flex items-center px-3 text-zinc-400">
                  <svg class="w-3.5 h-3.5" aria-hidden="true" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                  </svg>
                </div>
              </div>
            {:else if availableIps.length === 1}
              <div class="px-3.5 py-2 surface-input border border-white/[0.08] rounded-xl text-xs font-mono text-zinc-300 flex items-center justify-between">
                <span>{availableIps[0].ip}</span>
                <span class="text-[10px] text-zinc-500 font-sans">{availableIps[0].name}</span>
              </div>
            {:else}
              <input
                id="host-ip-select"
                type="text"
                bind:value={selectedHostIp}
                placeholder="192.168.1.1"
                class="w-full surface-input border border-white/[0.1] focus:border-white text-white text-xs font-mono px-3.5 py-2 rounded-xl outline-none transition-colors motion-reduce:transition-none focus-visible:ring-2 focus-visible:ring-white/30"
              />
            {/if}
          </div>
        </div>

        <button
          onclick={() => onSelectMode('host', undefined, selectedHostIp)}
          class="mt-6 h-10 w-full bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition-colors motion-reduce:transition-none active:scale-[0.99] outline-none focus-visible:ring-2 focus-visible:ring-white/50 whitespace-nowrap"
        >
          <span>Buat Room &amp; Mulai Host</span>
        </button>
      </div>
    </div>
  </main>

  <!-- Bottom Operational Status Footer -->
  <footer class="w-full max-w-6xl mx-auto pt-4 border-t border-white/[0.08] flex flex-col sm:flex-row items-center justify-between gap-2 text-xs text-zinc-400 shrink-0">
    <div class="flex items-center space-x-3">
      <span class="w-2 h-2 rounded-full bg-emerald-400 shrink-0"></span>
      <span class="text-[11px] text-zinc-300">Server LAN Standby</span>
      <span class="text-zinc-600 font-mono">/</span>
      <span class="text-[11px] text-zinc-400">Database SQLite Siap</span>
    </div>
    <span class="text-[11px] text-zinc-500">Ambrilindo Desktop v3.0</span>
  </footer>
</div>

<style>
  :root {
    --bg-app: #14161b;
    --bg-card: #181a20;
    --bg-input: #111215;
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
  button {
    white-space: nowrap;
  }
</style>
