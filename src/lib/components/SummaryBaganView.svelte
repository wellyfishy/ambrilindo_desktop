<script lang="ts">
  import { invoke } from "$lib/api_client";
  import type { SummaryBaganResult, SummaryBaganItem, SummaryBaganWinner } from "$lib/types";
  import { printWithOrientation } from "$lib/print_helper";
  import { resolveMediaLogos } from "$lib/media_helper";
  import DaftarJuaraPrintSheet from "./DaftarJuaraPrintSheet.svelte";

  interface Props {
    eventId: number;
    eventName?: string;
    hostAddress?: string;
  }

  let { eventId, eventName = "Event Turnamen", hostAddress }: Props = $props();

  let isLoading = $state(true);
  let errorMessage = $state<string | null>(null);
  let summaryData = $state<SummaryBaganResult | null>(null);
  let logoMap = $state<Record<string, string>>({});

  // Filters & Search
  let searchQuery = $state("");
  let filterType = $state<"all" | "kata" | "kumite">("all");
  let filterDivision = $state<"all" | "prestasi" | "festival" | "bob">("all");
  let filterStatus = $state<"all" | "completed" | "uncompleted">("all");

  // Reactivity on eventId change
  $effect(() => {
    if (eventId) {
      loadSummaryBagan();
    }
  });

  async function loadSummaryBagan() {
    if (!eventId) return;
    isLoading = true;
    errorMessage = null;
    try {
      summaryData = await invoke<SummaryBaganResult>("get_summary_bagan", { eventId });
      if (summaryData?.bagans) {
        const allLogos: string[] = [];
        for (const b of summaryData.bagans) {
          for (const w of b.winners) {
            if (w.logo) allLogos.push(w.logo);
          }
        }
        if (allLogos.length > 0) {
          resolveMediaLogos(allLogos, hostAddress).then((resolved) => {
            logoMap = resolved;
          });
        }
      }
    } catch (err: any) {
      console.error("Gagal memuat summary bagan:", err);
      errorMessage = typeof err === "string" ? err : err?.message || "Gagal memuat rekap bagan.";
    } finally {
      isLoading = false;
    }
  }

  function getDivision(bagan: SummaryBaganItem): "festival" | "bob" | "prestasi" {
    const name = (bagan.nama_bagan || "").toUpperCase();
    if (name.includes("FESTIVAL")) return "festival";
    if (name.includes("BOB") || name.includes("BEST OF THE BEST")) return "bob";
    return "prestasi";
  }

  function isKata(bagan: SummaryBaganItem): boolean {
    return bagan.tipe_tanding === "1" || (bagan.nama_bagan || "").toUpperCase().includes("KATA");
  }

  function getWinnerAffiliation(w?: SummaryBaganWinner): string {
    if (!w) return "-";
    if (w.perguruan_nama && w.utusan_nama) {
      return `${w.perguruan_nama} • ${w.utusan_nama}`;
    }
    return w.perguruan_nama || w.utusan_nama || "-";
  }

  // Filtered bagans
  let filteredBagans = $derived.by(() => {
    if (!summaryData?.bagans) return [];
    const q = searchQuery.toLowerCase().trim();

    return summaryData.bagans.filter((b) => {
      // 1. Division filter
      const div = getDivision(b);
      if (filterDivision === "prestasi" && div !== "prestasi") return false;
      if (filterDivision === "festival" && div !== "festival") return false;
      if (filterDivision === "bob" && div !== "bob") return false;

      // 2. Type filter (Kata / Kumite)
      const kata = isKata(b);
      if (filterType === "kata" && !kata) return false;
      if (filterType === "kumite" && kata) return false;

      // 3. Status filter
      if (filterStatus === "completed" && !b.is_selesai) return false;
      if (filterStatus === "uncompleted" && b.is_selesai) return false;

      // 4. Search query
      if (!q) return true;

      const matchBagan = b.nama_bagan.toLowerCase().includes(q) || b.kode.toLowerCase().includes(q);
      if (matchBagan) return true;

      const matchWinner = b.winners.some(
        (w) =>
          w.nama_atlet.toLowerCase().includes(q) ||
          w.perguruan_nama.toLowerCase().includes(q) ||
          w.utusan_nama.toLowerCase().includes(q)
      );
      return matchWinner;
    });
  });

  function resetFilters() {
    searchQuery = "";
    filterType = "all";
    filterDivision = "all";
    filterStatus = "all";
  }
</script>

<svelte:head>
  <style>
    @page {
      size: A4 portrait;
      margin: 8mm;
    }
  </style>
</svelte:head>

<div class="h-full flex flex-col p-4 md:p-6 select-none bg-[#14161b] text-zinc-200 overflow-y-auto font-sans print-sheet">
  <!-- ================= SCREEN VIEW: INTERACTIVE DARK THEME UI ================= -->
  <div class="no-print flex flex-col flex-1 min-h-0">
    <!-- Top Bar / Header Section -->
    <div class="flex flex-col md:flex-row md:items-center justify-between pb-3.5 mb-3.5 border-b border-white/[0.08] gap-3 shrink-0">
      <div class="min-w-0">
        <div class="flex items-center space-x-2.5">
          <span class="inline-flex items-center px-2 py-0.5 rounded-md bg-amber-500/10 text-amber-400 border border-amber-500/20 font-mono text-[10px] font-bold uppercase tracking-wider shrink-0">
            # summary-bagan
          </span>
          <h1 class="text-base sm:text-lg font-black tracking-tight text-white uppercase truncate font-sans">
            Rekap Bagan & Daftar Juara
          </h1>
          <span class="text-xs text-zinc-500 font-mono truncate hidden sm:inline">
            {summaryData?.event_name || eventName}
          </span>
        </div>
        <p class="text-xs text-zinc-400 mt-1 font-normal truncate">
          Rekapitulasi peraih medali juara 1 (emas), 2 (perak), dan 3 (perunggu) per nomor pertandingan
        </p>
      </div>

      <!-- Action Buttons (Single source: Print only, exports reside in # buku-hasil-turnamen) -->
      <div class="flex items-center space-x-2 shrink-0 self-start md:self-auto">
        <button
          type="button"
          onclick={() => printWithOrientation("portrait", "8mm")}
          class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors flex items-center space-x-1.5 active:scale-95 cursor-pointer shadow-xs"
          title="Cetak Daftar Juara Persis Buku Hasil Turnamen (A4 Portrait)"
        >
          <svg class="w-3.5 h-3.5 text-zinc-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
          </svg>
          <span>Cetak</span>
        </button>
      </div>
    </div>

    <!-- 4 KPI Metrics (DESIGN_SYSTEM.md Section 4.B) -->
    <div class="grid grid-cols-2 lg:grid-cols-4 gap-3 mb-4 shrink-0">
      <!-- Stat 1: Total Bagan -->
      <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 sm:p-3.5 flex items-center justify-between shadow-xs">
        <div>
          <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider block">Total Bagan</span>
          <div class="flex items-baseline space-x-1.5 mt-0.5">
            <span class="text-2xl font-black font-mono text-white tabular-nums">{summaryData?.total_bagans ?? 0}</span>
            <span class="text-[10px] font-mono text-zinc-500">Bagan</span>
          </div>
        </div>
        <div class="w-9 h-9 rounded-xl bg-white/[0.04] text-zinc-400 border border-white/[0.06] flex items-center justify-center shrink-0">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M4 6h4v4H4V6zm12 0h4v4h-4V6zM4 14h4v4H4v-4zm12 0h4v4h-4v-4zm-8-5h8M8 17h8" />
          </svg>
        </div>
      </div>

      <!-- Stat 2: Bagan Selesai -->
      <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 sm:p-3.5 flex items-center justify-between shadow-xs">
        <div>
          <span class="text-[10px] font-bold text-emerald-400 uppercase tracking-wider block">Bagan Selesai</span>
          <div class="flex items-baseline space-x-1.5 mt-0.5">
            <span class="text-2xl font-black font-mono text-emerald-300 tabular-nums">{summaryData?.completed_bagans ?? 0}</span>
            <span class="text-[10px] font-mono text-emerald-500/80">Ada Juara</span>
          </div>
        </div>
        <div class="w-9 h-9 rounded-xl bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 flex items-center justify-center shrink-0">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
            <path stroke-linecap="round" stroke-linejoin="round" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
          </svg>
        </div>
      </div>

      <!-- Stat 3: Medali Emas & Perak -->
      <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 sm:p-3.5 flex items-center justify-between shadow-xs">
        <div>
          <span class="text-[10px] font-bold text-amber-400 uppercase tracking-wider block">Emas & Perak</span>
          <div class="flex items-baseline space-x-2 mt-0.5">
            <span class="text-2xl font-black font-mono text-amber-300 tabular-nums">{summaryData?.total_gold ?? 0}</span>
            <span class="text-xs text-zinc-500 font-mono">/</span>
            <span class="text-2xl font-black font-mono text-slate-300 tabular-nums">{summaryData?.total_silver ?? 0}</span>
          </div>
        </div>
        <div class="w-9 h-9 rounded-xl bg-amber-500/10 text-amber-300 border border-amber-500/20 flex items-center justify-center shrink-0">
          <svg class="w-4 h-4 text-amber-400" fill="currentColor" viewBox="0 0 24 24">
            <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
          </svg>
        </div>
      </div>

      <!-- Stat 4: Medali Perunggu -->
      <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 sm:p-3.5 flex items-center justify-between shadow-xs">
        <div>
          <span class="text-[10px] font-bold text-amber-600 uppercase tracking-wider block">Medali Perunggu</span>
          <div class="flex items-baseline space-x-1.5 mt-0.5">
            <span class="text-2xl font-black font-mono text-amber-500 tabular-nums">{summaryData?.total_bronze ?? 0}</span>
            <span class="text-[10px] font-mono text-amber-600/80">Perunggu</span>
          </div>
        </div>
        <div class="w-9 h-9 rounded-xl bg-amber-700/15 text-amber-500 border border-amber-700/30 flex items-center justify-center shrink-0">
          <svg class="w-4 h-4 text-amber-500" fill="currentColor" viewBox="0 0 24 24">
            <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
          </svg>
        </div>
      </div>
    </div>

    <!-- Filter & Search Toolbar (DESIGN_SYSTEM.md Section 4.C) -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-2xl p-3 mb-4 flex flex-wrap items-center gap-2.5 shadow-sm shrink-0">
      <!-- Search Bar -->
      <div class="relative flex-1 min-w-[200px]">
        <input
          type="text"
          bind:value={searchQuery}
          placeholder="Cari bagan, kode, atlet, perguruan, kontingen..."
          class="w-full bg-[#111216] border border-white/[0.1] rounded-xl pl-9 pr-3.5 h-9 text-xs text-white placeholder-zinc-500 focus:outline-none focus:border-amber-500/50 transition-colors"
        />
        <svg
          class="w-4 h-4 text-zinc-500 absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
        </svg>
      </div>

      <!-- Division Filter (Prestasi / Festival / BOB) -->
      <select
        bind:value={filterDivision}
        class="h-9 bg-[#111216] border border-white/[0.1] hover:border-white/20 rounded-xl px-3 text-xs text-zinc-200 focus:outline-none focus:border-amber-500/50 transition-colors cursor-pointer"
      >
        <option value="all">Semua Divisi</option>
        <option value="prestasi">Prestasi</option>
        <option value="festival">Festival</option>
        <option value="bob">Best of the Best (BOB)</option>
      </select>

      <!-- Type Filter (Kata / Kumite) -->
      <select
        bind:value={filterType}
        class="h-9 bg-[#111216] border border-white/[0.1] hover:border-white/20 rounded-xl px-3 text-xs text-zinc-200 focus:outline-none focus:border-amber-500/50 transition-colors cursor-pointer"
      >
        <option value="all">Semua Tipe</option>
        <option value="kata">Kata</option>
        <option value="kumite">Kumite</option>
      </select>

      <!-- Status Filter -->
      <select
        bind:value={filterStatus}
        class="h-9 bg-[#111216] border border-white/[0.1] hover:border-white/20 rounded-xl px-3 text-xs text-zinc-200 focus:outline-none focus:border-amber-500/50 transition-colors cursor-pointer"
      >
        <option value="all">Semua Status</option>
        <option value="completed">Sudah Selesai (Ada Juara)</option>
        <option value="uncompleted">Belum Selesai</option>
      </select>

      <!-- Clear Filter Button (if filtered) -->
      {#if searchQuery || filterType !== "all" || filterDivision !== "all" || filterStatus !== "all"}
        <button
          type="button"
          onclick={resetFilters}
          class="h-9 px-3 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-400 hover:text-white text-xs font-semibold rounded-xl border border-white/[0.08] transition-colors flex items-center space-x-1 cursor-pointer"
          title="Reset Filter"
        >
          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
          </svg>
          <span>Reset</span>
        </button>
      {/if}
    </div>

    <!-- Loading & Error States -->
    {#if isLoading}
      <div class="flex-1 flex flex-col items-center justify-center p-12 text-zinc-400">
        <svg class="w-8 h-8 text-amber-400 animate-spin mb-3" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
        </svg>
        <p class="text-xs font-mono tracking-wider uppercase text-zinc-300">Memuat Rekapitulasi Bagan...</p>
      </div>
    {:else if errorMessage}
      <div class="bg-rose-500/10 border border-rose-500/20 rounded-2xl p-6 text-center text-rose-300 my-auto max-w-lg mx-auto">
        <svg class="w-10 h-10 mx-auto text-rose-400 mb-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
        </svg>
        <h3 class="text-sm font-bold text-white mb-1">Terjadi Kesalahan</h3>
        <p class="text-xs text-rose-300/90">{errorMessage}</p>
        <button
          type="button"
          onclick={loadSummaryBagan}
          class="mt-4 px-4 py-2 bg-rose-500/20 hover:bg-rose-500/30 text-rose-200 text-xs font-semibold rounded-xl border border-rose-500/30 transition-colors"
        >
          Coba Muat Ulang
        </button>
      </div>
    {:else if (summaryData?.total_bagans ?? 0) === 0}
      <!-- Empty State: Zero Brackets Generated for Event -->
      <div class="bg-[#181a20] border border-white/[0.08] rounded-2xl p-10 text-center text-zinc-400 my-auto max-w-lg mx-auto shadow-md">
        <div class="w-14 h-14 rounded-2xl bg-amber-500/10 border border-amber-500/20 text-amber-400 flex items-center justify-center mx-auto mb-3">
          <svg class="w-7 h-7" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
            <path stroke-linecap="round" stroke-linejoin="round" d="M19 11H5m14 0a2 2 0 012 2v6a2 2 0 01-2 2H5a2 2 0 01-2-2v-6a2 2 0 012-2m14 0V9a2 2 0 00-2-2M5 11V9a2 2 0 012-2m0 0V5a2 2 0 012-2h6a2 2 0 012 2v2M7 7h10" />
          </svg>
        </div>
        <h3 class="text-sm font-bold text-white">Bagan Pertandingan Belum Di-generate</h3>
        <p class="text-xs text-zinc-400 mt-1.5 leading-relaxed">
          Belum ada bagan pertandingan yang di-drawing untuk event <span class="text-amber-400 font-semibold">{eventName}</span>.
        </p>
        <p class="text-[11px] text-zinc-500 mt-1 leading-relaxed">
          Silakan buka channel <strong class="text-zinc-300"># nomor-tanding</strong> atau <strong class="text-zinc-300"># bagan</strong> untuk melakukan drawing peserta ke dalam bagan pertandingan.
        </p>
      </div>
    {:else if filteredBagans.length === 0}
      <!-- Empty State: Filter / Search yielding no results -->
      <div class="bg-[#181a20] border border-white/[0.08] rounded-2xl p-10 text-center text-zinc-400 my-auto max-w-md mx-auto shadow-md">
        <svg class="w-10 h-10 mx-auto text-zinc-600 mb-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9.172 16.172a4 4 0 015.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
        <p class="text-sm font-bold text-white">Tidak Ada Bagan yang Cocok</p>
        <p class="text-xs text-zinc-500 mt-1">Coba sesuaikan kata kunci pencarian atau ganti filter yang aktif.</p>
        <button
          type="button"
          onclick={resetFilters}
          class="mt-4 px-3.5 py-1.5 bg-white/[0.05] hover:bg-white/[0.1] text-zinc-200 text-xs font-semibold rounded-xl border border-white/[0.1] transition-colors active:scale-95 cursor-pointer"
        >
          Reset Filter
        </button>
      </div>
    {:else}
      <!-- Main Bagan Cards Grid (Screen Mode) -->
      <div class="grid grid-cols-1 md:grid-cols-2 gap-3.5">
        {#each filteredBagans as bagan (bagan.id)}
          {@const div = getDivision(bagan)}
          {@const kata = isKata(bagan)}

          <div class="bg-[#181a20] border border-white/[0.08] hover:border-white/20 rounded-xl p-3 sm:p-3.5 transition-colors shadow-xs flex flex-col justify-between">
            <!-- Card Header -->
            <div class="flex items-center justify-between pb-2.5 mb-2.5 border-b border-white/[0.06] gap-2">
              <div class="flex items-center space-x-2 min-w-0">
                <!-- Bracket Code Badge e.g. [003] -->
                <span class="px-2 py-0.5 rounded-md bg-amber-500/15 text-amber-300 border border-amber-500/30 font-mono font-black text-xs shrink-0 tracking-wider">
                  [{bagan.kode}]
                </span>

                <!-- Category / Bagan Name -->
                <h3 class="text-xs sm:text-[13px] font-bold text-white uppercase break-words leading-tight tracking-wide" title={bagan.nama_bagan}>
                  {bagan.nama_bagan}
                </h3>
              </div>

              <!-- Tipe Badges: Kata/Kumite & Division -->
              <div class="flex items-center space-x-1.5 shrink-0">
                <!-- Division Badge -->
                {#if div === "festival"}
                  <span class="px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-300 border border-amber-500/20 text-[9px] font-bold tracking-wider uppercase">
                    FESTIVAL
                  </span>
                {:else if div === "bob"}
                  <span class="px-1.5 py-0.5 rounded bg-purple-500/10 text-purple-300 border border-purple-500/20 text-[9px] font-bold tracking-wider uppercase">
                    BOB
                  </span>
                {/if}

                <!-- Kata / Kumite Badge -->
                {#if kata}
                  <span class="inline-flex items-center space-x-1 px-2 py-0.5 rounded-md bg-sky-500/10 text-sky-400 border border-sky-500/20 text-[10px] font-bold tracking-wider uppercase">
                    <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
                    </svg>
                    <span>KATA</span>
                  </span>
                {:else}
                  <span class="inline-flex items-center space-x-1 px-2 py-0.5 rounded-md bg-rose-500/10 text-rose-400 border border-rose-500/20 text-[10px] font-bold tracking-wider uppercase">
                    <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                      <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17.657 18.657A8 8 0 016.343 7.343S7 9 10c0-2 .5-5 2.986-7C14 5 16.09 5.777 17.656 7.343A7.975 7.975 0 0120 13a7.975 7.975 0 01-2.343 5.657z" />
                    </svg>
                    <span>KUMITE</span>
                  </span>
                {/if}
              </div>
            </div>

            <!-- Card Body: Winners List -->
            {#if bagan.winners.length === 0}
              <div class="py-3 text-center text-xs text-zinc-500 italic bg-white/[0.01] rounded-lg border border-dashed border-white/[0.05]">
                Pertandingan belum selesai / belum ada data juara
              </div>
            {:else}
              <div class="space-y-1.5">
                {#each bagan.winners as winner (winner.juara + '_' + winner.athlete_id)}
                  <div class="flex items-center justify-between p-1.5 sm:px-2 rounded-lg bg-white/[0.02] border border-white/[0.04] hover:bg-white/[0.05] transition-colors gap-2">
                    <!-- Left: Medal Badge + Logo + Athlete Name -->
                    <div class="flex items-center space-x-2 min-w-0">
                      <!-- Medal Position Badge -->
                      {#if winner.juara === "1"}
                        <div class="w-8 h-6 rounded bg-amber-400/20 text-amber-300 border border-amber-400/40 font-mono font-black text-xs flex items-center justify-center space-x-0.5 shrink-0 shadow-xs">
                          <svg class="w-3 h-3 text-amber-400" fill="currentColor" viewBox="0 0 24 24">
                            <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
                          </svg>
                          <span>1.</span>
                        </div>
                      {:else if winner.juara === "2"}
                        <div class="w-8 h-6 rounded bg-slate-300/15 text-slate-200 border border-slate-300/30 font-mono font-black text-xs flex items-center justify-center space-x-0.5 shrink-0 shadow-xs">
                          <svg class="w-3 h-3 text-slate-300" fill="currentColor" viewBox="0 0 24 24">
                            <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
                          </svg>
                          <span>2.</span>
                        </div>
                      {:else}
                        <div class="w-8 h-6 rounded bg-amber-700/20 text-amber-500 border border-amber-700/40 font-mono font-black text-xs flex items-center justify-center space-x-0.5 shrink-0 shadow-xs">
                          <svg class="w-3 h-3 text-amber-500" fill="currentColor" viewBox="0 0 24 24">
                            <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
                          </svg>
                          <span>3.</span>
                        </div>
                      {/if}

                      <!-- Avatar / Logo Icon -->
                      <div class="w-6 h-6 rounded bg-white/[0.04] border border-white/[0.08] flex items-center justify-center text-[10px] font-bold text-zinc-400 shrink-0 uppercase overflow-hidden">
                        {#if winner.logo && logoMap[winner.logo]}
                          <img src={logoMap[winner.logo]} alt="" class="w-full h-full object-contain" />
                        {:else}
                          <span>{winner.perguruan_nama ? winner.perguruan_nama.substring(0, 2) : "DO"}</span>
                        {/if}
                      </div>

                      <!-- Athlete Name -->
                      <div class="min-w-0">
                        <span class="text-xs font-bold text-white uppercase tracking-tight truncate block" title={winner.nama_atlet}>
                          {winner.nama_atlet}
                        </span>
                        {#if winner.is_uncounted}
                          <span class="text-[9px] text-amber-400/90 font-mono font-medium block">
                            (Tanpa Lawan)
                          </span>
                        {/if}
                      </div>
                    </div>

                    <!-- Right: Affiliation (STRICT: Perguruan first, then Kontingen) -->
                    <div class="shrink-0 text-right min-w-[140px] max-w-[220px]">
                      <span class="text-[11px] font-mono text-zinc-300 truncate block tracking-tight" title="{winner.perguruan_nama} • {winner.utusan_nama}">
                        <span class="text-white font-bold">{winner.perguruan_nama}</span>
                        <span class="text-zinc-500 mx-1">•</span>
                        <span class="text-zinc-200 font-bold">{winner.utusan_nama}</span>
                      </span>
                    </div>
                  </div>
                {/each}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    {/if}
  </div>

  <!-- ================= PRINT VIEW: MENGAMBIL DARI KOMPONEN BERSAMA DAFTAR JUARA ================= -->
  <section class="print-only hidden booklet-page-portrait bg-white text-slate-900 p-8 print:p-0 rounded-md shadow-none border-none">
    <DaftarJuaraPrintSheet
      bagans={filteredBagans}
      eventName={summaryData?.event_name || eventName}
      pageLabel="Daftar Juara Seluruh Nomor Tanding"
      {logoMap}
    />
  </section>
</div>

<style>
  @media print {
    @page {
      size: A4 portrait;
      margin: 8mm;
    }
    :global(body) {
      background: white !important;
      color: black !important;
    }
    .print-sheet {
      background: white !important;
      color: black !important;
      padding: 0 !important;
      overflow: visible !important;
    }
    .no-print {
      display: none !important;
    }
    .print-only {
      display: block !important;
    }
    .booklet-page-portrait {
      width: 100% !important;
      min-height: 270mm !important;
      padding: 0 !important;
      box-sizing: border-box !important;
    }
  }
</style>
