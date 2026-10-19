<script lang="ts">
  import type { AppMode, DatabaseSummary, NetworkInfo } from "$lib/types";

  interface Props {
    currentDb: DatabaseSummary | null;
    networkInfo: NetworkInfo | null;
    onSelectMode: (mode: AppMode, hostUrl?: string) => void;
    onPickDatabase: () => void;
  }

  let { currentDb, networkInfo, onSelectMode, onPickDatabase }: Props = $props();

  let hostInputUrl = $state("http://192.168.1.");
  let isConnectingHost = $state(false);
  let hostError = $state<string | null>(null);

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

<div class="min-h-screen w-full bg-[#0d0e12] text-zinc-200 flex flex-col justify-between font-sans select-none p-8 md:p-14 relative overflow-hidden">
  <!-- Subtle ambient background illumination -->
  <div class="absolute -top-40 left-1/2 -translate-x-1/2 w-[800px] h-[400px] bg-gradient-to-b from-amber-500/10 via-amber-500/[0.02] to-transparent blur-3xl pointer-events-none rounded-full"></div>

  <!-- Top Navigation Bar -->
  <header class="max-w-6xl mx-auto w-full flex items-center justify-between border-b border-white/[0.08] pb-6 relative z-10">
    <div class="flex items-center space-x-3.5">
      <!-- Minimalist Brand Glyphs -->
      <div class="w-10 h-10 rounded-xl bg-white text-black font-black flex items-center justify-center text-base tracking-tighter shadow-md">
        A
      </div>
      <div>
        <div class="flex items-center space-x-2.5">
          <h1 class="text-base font-black tracking-[0.18em] text-white uppercase">AMBRILINDO</h1>
          <span class="text-[10px] font-mono font-bold tracking-wider px-2 py-0.5 rounded-full bg-white/[0.06] text-zinc-400 border border-white/[0.08]">
            v3.0
          </span>
        </div>
        <p class="text-[11px] text-zinc-500 font-medium">Karate Tournament Operations System</p>
      </div>
    </div>

    <!-- Active DB Chip -->
    <div class="flex items-center space-x-3 bg-white/[0.03] hover:bg-white/[0.05] transition px-3 py-1.5 rounded-xl border border-white/[0.08] shadow-sm">
      <div class="flex items-center space-x-2">
        <svg class="w-3.5 h-3.5 text-amber-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <ellipse cx="12" cy="5" rx="9" ry="3"/>
          <path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/>
          <path d="M3 12c0 1.66 4 3 9 3s9-1.34 9-3"/>
        </svg>
        <div class="text-left leading-tight">
          <span class="text-[9px] font-bold tracking-widest text-zinc-500 uppercase block">Database</span>
          <span class="text-xs font-mono font-semibold text-zinc-200 truncate max-w-[180px] block">
            {currentDb ? currentDb.db_path.split(/[\\/]/).pop() : 'Belum Dimuat'}
          </span>
        </div>
      </div>
      <button
        onclick={onPickDatabase}
        class="text-[11px] font-medium text-zinc-400 hover:text-white px-2 py-1 rounded-md bg-white/[0.04] hover:bg-white/[0.08] transition border border-white/[0.06]"
      >
        Ganti
      </button>
    </div>
  </header>

  <!-- Main Center Options -->
  <main class="max-w-6xl mx-auto w-full my-auto py-12 relative z-10">
    <div class="text-center mb-12">
      <h2 class="text-2xl md:text-3xl font-black text-white tracking-tight">PILIH MODE OPERASI</h2>
    </div>

    <div class="grid grid-cols-1 md:grid-cols-3 gap-6">
      <!-- Card 1: Mode Offline -->
      <div class="bg-[#14161b] hover:bg-[#181a20] border border-white/[0.08] hover:border-white/[0.18] rounded-2xl p-7 flex flex-col justify-between transition-all duration-200 group shadow-lg hover:shadow-2xl">
        <div>
          <div class="flex items-center justify-between mb-5">
            <div class="w-10 h-10 rounded-xl bg-white/[0.04] border border-white/[0.08] text-zinc-300 flex items-center justify-center group-hover:text-white group-hover:border-white/[0.16] transition">
              <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.75">
                <rect x="2" y="3" width="20" height="14" rx="2" ry="2"/>
                <line x1="8" y1="21" x2="16" y2="21"/>
                <line x1="12" y1="17" x2="12" y2="21"/>
              </svg>
            </div>
            <span class="text-[10px] font-mono font-bold tracking-widest uppercase px-2.5 py-0.5 rounded-full bg-white/[0.04] text-zinc-400 border border-white/[0.06]">
              OFFLINE
            </span>
          </div>

          <h3 class="text-lg font-bold text-white tracking-tight">Offline</h3>
          <p class="text-xs text-zinc-400 mt-2 leading-relaxed font-normal">
            Kelola turnamen secara lokal dari satu laptop tanpa ketergantungan koneksi jaringan luar.
          </p>

          <ul class="mt-6 space-y-2 text-xs text-zinc-400 font-normal">
            <li class="flex items-center space-x-2">
              <svg class="w-3.5 h-3.5 text-zinc-500 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5" /></svg>
              <span>Operator tunggal & pengujian bagan</span>
            </li>
            <li class="flex items-center space-x-2">
              <svg class="w-3.5 h-3.5 text-zinc-500 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5" /></svg>
              <span>Manajemen bagan & atlet langsung di SQLite</span>
            </li>
            <li class="flex items-center space-x-2">
              <svg class="w-3.5 h-3.5 text-zinc-500 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5" /></svg>
              <span>Proyeksi scoreboard ke monitor sekunder</span>
            </li>
          </ul>
        </div>

        <button
          onclick={() => onSelectMode('solo')}
          class="mt-8 w-full py-2.5 bg-white/[0.08] hover:bg-white text-zinc-200 hover:text-black font-semibold text-xs rounded-xl border border-white/[0.1] hover:border-white transition-all active:scale-[0.98]"
        >
          Masuk Mode Offline
        </button>
      </div>

      <!-- Card 2: Gabung Room (LAN Client) -->
      <div class="bg-[#14161b] hover:bg-[#181a20] border border-white/[0.08] hover:border-emerald-500/40 rounded-2xl p-7 flex flex-col justify-between transition-all duration-200 group shadow-lg hover:shadow-2xl">
        <div>
          <div class="flex items-center justify-between mb-5">
            <div class="w-10 h-10 rounded-xl bg-white/[0.04] border border-white/[0.08] text-zinc-300 flex items-center justify-center group-hover:text-emerald-400 group-hover:border-emerald-500/30 transition">
              <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.75">
                <circle cx="18" cy="5" r="3"/>
                <circle cx="6" cy="12" r="3"/>
                <circle cx="18" cy="19" r="3"/>
                <line x1="8.59" y1="13.51" x2="15.42" y2="17.49"/>
                <line x1="15.41" y1="6.51" x2="8.59" y2="10.49"/>
              </svg>
            </div>
            <span class="text-[10px] font-mono font-bold tracking-widest uppercase px-2.5 py-0.5 rounded-full bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">
              LAN CLIENT
            </span>
          </div>

          <h3 class="text-lg font-bold text-white tracking-tight">Gabung Room</h3>
          <p class="text-xs text-zinc-400 mt-2 leading-relaxed font-normal">
            Hubungkan laptop juri, operator meja, atau layar tatami ke Host Server di venue.
          </p>

          <div class="mt-5 space-y-1.5">
            <label for="host-ip-input" class="text-[10px] font-mono font-bold uppercase tracking-wider text-zinc-500 block">
              ALAMAT HOST
            </label>
            <input
              id="host-ip-input"
              type="text"
              bind:value={hostInputUrl}
              placeholder="http://192.168.1.5:8000"
              class="w-full bg-black/40 border border-white/[0.1] focus:border-emerald-500 text-white text-xs font-mono px-3 py-2 rounded-lg outline-none transition"
            />
            {#if hostError}
              <p class="text-[11px] text-red-400 font-medium pt-1">{hostError}</p>
            {/if}
          </div>
        </div>

        <button
          onclick={handleJoinHost}
          disabled={isConnectingHost}
          class="mt-8 w-full py-2.5 bg-emerald-600/80 hover:bg-emerald-500 text-white font-semibold text-xs rounded-xl border border-emerald-500/40 transition-all active:scale-[0.98] disabled:opacity-50"
        >
          {isConnectingHost ? 'Menghubungkan...' : 'Hubungkan ke Host'}
        </button>
      </div>

      <!-- Card 3: Buat Room (LAN Host Server) -->
      <div class="bg-[#14161b] hover:bg-[#181a20] border border-white/[0.08] hover:border-amber-500/40 rounded-2xl p-7 flex flex-col justify-between transition-all duration-200 group shadow-lg hover:shadow-2xl">
        <div>
          <div class="flex items-center justify-between mb-5">
            <div class="w-10 h-10 rounded-xl bg-white/[0.04] border border-white/[0.08] text-zinc-300 flex items-center justify-center group-hover:text-amber-400 group-hover:border-amber-500/30 transition">
              <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.75">
                <rect x="2" y="2" width="20" height="8" rx="2" ry="2"/>
                <rect x="2" y="14" width="20" height="8" rx="2" ry="2"/>
                <line x1="6" y1="6" x2="6.01" y2="6"/>
                <line x1="6" y1="18" x2="6.01" y2="18"/>
              </svg>
            </div>
            <span class="text-[10px] font-mono font-bold tracking-widest uppercase px-2.5 py-0.5 rounded-full bg-amber-500/10 text-amber-400 border border-amber-500/20">
              PORT 8000
            </span>
          </div>

          <h3 class="text-lg font-bold text-white tracking-tight">Buat Room</h3>
          <p class="text-xs text-zinc-400 mt-2 leading-relaxed font-normal">
            Jadikan laptop ini sebagai server utama turnamen. Buka port 8000 untuk sinkronisasi LAN juri tablet.
          </p>

          <ul class="mt-6 space-y-2 text-xs text-zinc-400 font-normal">
            <li class="flex items-center space-x-2">
              <svg class="w-3.5 h-3.5 text-zinc-500 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5" /></svg>
              <span>Server Axum lokal otomatis aktif di port 8000</span>
            </li>
            <li class="flex items-center space-x-2">
              <svg class="w-3.5 h-3.5 text-zinc-500 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5" /></svg>
              <span>QR code instan untuk perangkat tablet juri</span>
            </li>
            <li class="flex items-center space-x-2">
              <svg class="w-3.5 h-3.5 text-zinc-500 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5"><path stroke-linecap="round" stroke-linejoin="round" d="M4.5 12.75l6 6 9-13.5" /></svg>
              <span>Broadcast WebSocket tatami sub-milidetik</span>
            </li>
          </ul>
        </div>

        <button
          onclick={() => onSelectMode('host')}
          class="mt-8 w-full py-2.5 bg-amber-500 hover:bg-amber-400 text-slate-950 font-bold text-xs rounded-xl shadow transition-all active:scale-[0.98]"
        >
          Buat Room & Mulai Host
        </button>
      </div>
    </div>
  </main>

  <!-- Bottom Minimalist Footer -->
  <footer class="max-w-6xl mx-auto w-full pt-6 border-t border-white/[0.08] flex items-center justify-between text-[11px] text-zinc-500 relative z-10">
    <div class="flex items-center space-x-2">
      <span class="w-1.5 h-1.5 rounded-full bg-emerald-500"></span>
      <span>Axum + Tokio Embedded Engine</span>
    </div>
    <span>Ambrilindo Desktop</span>
  </footer>
</div>
