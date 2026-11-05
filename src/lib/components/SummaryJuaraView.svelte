<script lang="ts">
  import { invoke } from "$lib/api_client";
  import type { SummaryJuaraResult, StandingItem } from "$lib/types";
  import { printWithOrientation } from "$lib/print_helper";
  import { resolveMediaLogos } from "$lib/media_helper";
  import KlasemenMedaliPrintSheet from "./KlasemenMedaliPrintSheet.svelte";

  interface Props {
    eventId: number;
    eventName?: string;
    hostAddress?: string;
  }

  let { eventId, eventName = "", hostAddress = "" }: Props = $props();

  let summaryData = $state<SummaryJuaraResult | null>(null);
  let isLoading = $state(true);
  let errorMessage = $state<string | null>(null);
  let logoMap = $state<Record<string, string>>({});

  // Default to perguruan first per FORKI domain guidelines
  let activeTab = $state<"perguruan" | "kontingen">("perguruan");
  let searchQuery = $state("");
  let filterOnlyWithMedals = $state(true);
  let expandedRows = $state<Record<number, boolean>>({});

  // Reactivity: Auto-load when eventId changes
  $effect(() => {
    if (eventId) {
      loadSummaryData();
    }
  });

  async function loadSummaryData() {
    if (!eventId) return;
    isLoading = true;
    errorMessage = null;
    try {
      const res = await invoke<SummaryJuaraResult>("get_summary_juara", { eventId });
      summaryData = res;
      if (res) {
        const allLogos: string[] = [];
        for (const k of res.utusan_standings) {
          if (k.logo) allLogos.push(k.logo);
        }
        for (const p of res.perguruan_standings) {
          if (p.logo) allLogos.push(p.logo);
        }
        if (allLogos.length > 0) {
          resolveMediaLogos(allLogos, hostAddress).then((resolved) => {
            logoMap = resolved;
          });
        }
      }
    } catch (err: any) {
      console.error("Gagal memuat summary juara:", err);
      errorMessage = typeof err === "string" ? err : err?.message || "Gagal memuat klasemen juara.";
    } finally {
      isLoading = false;
    }
  }



  // Current list based on active tab (Perguruan or Kontingen)
  let currentList = $derived.by(() => {
    if (!summaryData) return [];
    return activeTab === "perguruan"
      ? summaryData.perguruan_standings
      : summaryData.utusan_standings;
  });

  // Filtered list by search query & medal presence
  let filteredList = $derived.by(() => {
    const q = searchQuery.toLowerCase().trim();
    return currentList.filter((item) => {
      const matchSearch = !q || item.nama.toLowerCase().includes(q);
      const matchMedal = !filterOnlyWithMedals || item.total > 0 || item.uncounted_gold > 0;
      return matchSearch && matchMedal;
    });
  });

  function toggleRow(id: number) {
    expandedRows[id] = !expandedRows[id];
  }

  function expandAll() {
    const map: Record<number, boolean> = {};
    for (const item of filteredList) {
      map[item.id] = true;
    }
    expandedRows = map;
  }

  function collapseAll() {
    expandedRows = {};
  }

  function resetFilters() {
    searchQuery = "";
    filterOnlyWithMedals = true;
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
  <!-- Screen View (Hidden when printing) -->
  <div class="no-print flex-1 flex flex-col min-h-0">
    <!-- Top Bar / Header Section -->
    <div class="flex flex-col md:flex-row md:items-center justify-between pb-3.5 mb-3.5 border-b border-white/[0.08] gap-3 shrink-0">
      <div class="min-w-0">
        <div class="flex items-center space-x-2.5">
          <span class="inline-flex items-center px-2 py-0.5 rounded-md bg-amber-500/10 text-amber-400 border border-amber-500/20 font-mono text-[10px] font-bold uppercase tracking-wider shrink-0">
            # summary-juara
          </span>
          <h1 class="text-base sm:text-lg font-black tracking-tight text-white uppercase truncate font-sans">
            Klasemen Medali & Juara Umum
          </h1>
          <span class="text-xs text-zinc-500 font-mono truncate hidden sm:inline">
            {summaryData?.event_name || eventName || "Turnamen Karate"}
          </span>
        </div>
        <p class="text-xs text-zinc-400 mt-1 font-normal truncate">
          Perhitungan klasemen Juara Umum resmi berdasarkan hasil bagan tunggal & final (kategori tanpa lawan mendapat emas mandiri tanpa poin juara umum)
        </p>
      </div>

      <!-- Action Buttons (No manual refresh needed, realtime on action) -->
      <div class="flex items-center space-x-2 shrink-0 self-start md:self-auto">
        <!-- Print Button -->
        <button
          type="button"
          onclick={() => printWithOrientation("portrait", "8mm")}
          class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors flex items-center space-x-1.5 active:scale-95 cursor-pointer shadow-xs"
          title="Cetak Rekapan Juara (A4 Portrait)"
        >
          <svg class="w-3.5 h-3.5 text-zinc-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
          </svg>
          <span>Cetak</span>
        </button>
      </div>
    </div>

  <!-- 4 KPI Metrics (DESIGN_SYSTEM.md Section 4.B) -->
  <div class="grid grid-cols-2 lg:grid-cols-4 gap-3 mb-4 shrink-0 no-print">
    <!-- Stat 1: Emas Resmi -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 sm:p-3.5 flex items-center justify-between shadow-xs">
      <div>
        <span class="text-[10px] font-bold text-amber-400 uppercase tracking-wider block">Emas Terhitung</span>
        <div class="flex items-baseline space-x-1.5 mt-0.5">
          <span class="text-2xl font-black font-mono text-amber-300 tabular-nums">{summaryData?.total_gold ?? 0}</span>
          <span class="text-[10px] font-mono text-amber-500/80">Poin Juara Umum</span>
        </div>
      </div>
      <div class="w-9 h-9 rounded-xl bg-amber-500/10 text-amber-300 border border-amber-500/20 flex items-center justify-center shrink-0">
        <svg class="w-4 h-4 text-amber-400" fill="currentColor" viewBox="0 0 24 24">
          <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
        </svg>
      </div>
    </div>

    <!-- Stat 2: Perak Resmi -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 sm:p-3.5 flex items-center justify-between shadow-xs">
      <div>
        <span class="text-[10px] font-bold text-slate-300 uppercase tracking-wider block">Perak Terhitung</span>
        <div class="flex items-baseline space-x-1.5 mt-0.5">
          <span class="text-2xl font-black font-mono text-slate-200 tabular-nums">{summaryData?.total_silver ?? 0}</span>
          <span class="text-[10px] font-mono text-slate-400">Medali Perak</span>
        </div>
      </div>
      <div class="w-9 h-9 rounded-xl bg-slate-300/10 text-slate-300 border border-slate-300/20 flex items-center justify-center shrink-0">
        <svg class="w-4 h-4 text-slate-300" fill="currentColor" viewBox="0 0 24 24">
          <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
        </svg>
      </div>
    </div>

    <!-- Stat 3: Perunggu Resmi -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 sm:p-3.5 flex items-center justify-between shadow-xs">
      <div>
        <span class="text-[10px] font-bold text-amber-600 uppercase tracking-wider block">Perunggu Terhitung</span>
        <div class="flex items-baseline space-x-1.5 mt-0.5">
          <span class="text-2xl font-black font-mono text-amber-500 tabular-nums">{summaryData?.total_bronze ?? 0}</span>
          <span class="text-[10px] font-mono text-amber-600/80">Medali Perunggu</span>
        </div>
      </div>
      <div class="w-9 h-9 rounded-xl bg-amber-700/15 text-amber-500 border border-amber-700/30 flex items-center justify-center shrink-0">
        <svg class="w-4 h-4 text-amber-500" fill="currentColor" viewBox="0 0 24 24">
          <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z" />
        </svg>
      </div>
    </div>

    <!-- Stat 4: Tanpa Lawan (BYE) -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 sm:p-3.5 flex items-center justify-between shadow-xs">
      <div>
        <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider block">Tanpa Lawan (BYE)</span>
        <div class="flex items-baseline space-x-1.5 mt-0.5">
          <span class="text-2xl font-black font-mono text-zinc-300 tabular-nums">{summaryData?.total_uncounted ?? 0}</span>
          <span class="text-[10px] font-mono text-zinc-500">Emas Mandiri</span>
        </div>
      </div>
      <div class="w-9 h-9 rounded-xl bg-white/[0.04] text-zinc-400 border border-white/[0.06] flex items-center justify-center shrink-0">
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M9 12l2 2 4-4m5.618-4.016A11.955 11.955 0 0112 2.944a11.955 11.955 0 01-8.618 3.04A12.02 12.02 0 003 9c0 5.591 3.824 10.29 9 11.622 5.176-1.332 9-6.03 9-11.622 0-1.042-.133-2.052-.382-3.016z" />
        </svg>
      </div>
    </div>
  </div>

  <!-- Filter & Tab Switcher Toolbar (DESIGN_SYSTEM.md Section 4.C) -->
  <div class="no-print bg-[#181a20] border border-white/[0.08] rounded-2xl p-3 mb-4 flex flex-wrap items-center justify-between gap-3 shadow-sm">
    <!-- Tab Switcher (Perguruan ALWAYS first, then Kontingen) -->
    <div class="flex items-center bg-[#111216] p-1 rounded-xl border border-white/[0.08]">
      <button
        type="button"
        onclick={() => (activeTab = "perguruan")}
        class="h-8 px-3.5 rounded-lg text-xs font-bold transition-all flex items-center space-x-1.5 cursor-pointer {activeTab === 'perguruan' ? 'bg-white text-black shadow-sm font-semibold' : 'text-zinc-400 hover:text-white'}"
      >
        <span>Berdasarkan Perguruan</span>
        <span class="text-[10px] font-mono opacity-80">({summaryData?.perguruan_standings.length ?? 0})</span>
      </button>

      <button
        type="button"
        onclick={() => (activeTab = "kontingen")}
        class="h-8 px-3.5 rounded-lg text-xs font-bold transition-all flex items-center space-x-1.5 cursor-pointer {activeTab === 'kontingen' ? 'bg-white text-black shadow-sm font-semibold' : 'text-zinc-400 hover:text-white'}"
      >
        <span>Berdasarkan Kontingen</span>
        <span class="text-[10px] font-mono opacity-80">({summaryData?.utusan_standings.length ?? 0})</span>
      </button>
    </div>

    <!-- Search & Filter Controls -->
    <div class="flex flex-wrap items-center gap-2">
      <!-- Search Input -->
      <div class="relative min-w-[200px]">
        <input
          type="text"
          bind:value={searchQuery}
          placeholder="Cari nama {activeTab === 'perguruan' ? 'perguruan' : 'kontingen'}..."
          class="w-full bg-[#111216] border border-white/[0.1] rounded-xl pl-9 pr-3.5 h-8 text-xs text-white placeholder-zinc-500 focus:outline-none focus:border-amber-500/50 transition-colors"
        />
        <svg
          class="w-3.5 h-3.5 text-zinc-500 absolute left-3 top-1/2 -translate-y-1/2 pointer-events-none"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
        </svg>
      </div>

      <!-- Filter Medals Only Button -->
      <button
        type="button"
        onclick={() => (filterOnlyWithMedals = !filterOnlyWithMedals)}
        class="h-8 px-3 text-xs font-semibold rounded-xl border transition-colors flex items-center space-x-1.5 cursor-pointer {filterOnlyWithMedals ? 'bg-amber-500/10 border-amber-500/25 text-amber-300' : 'bg-white/[0.04] border-white/[0.1] text-zinc-400 hover:text-white'}"
      >
        <span>{filterOnlyWithMedals ? "Hanya Peraih Medali" : "Semua Terdaftar"}</span>
      </button>

      <div class="h-4 w-px bg-white/[0.1] mx-0.5 hidden sm:block"></div>

      <!-- Expand / Collapse All -->
      <div class="flex items-center space-x-1">
        <button
          type="button"
          onclick={expandAll}
          class="h-8 px-2.5 text-[11px] font-medium text-zinc-400 hover:text-white hover:bg-white/[0.05] rounded-xl border border-white/[0.06] transition-colors cursor-pointer"
          title="Buka seluruh rincian pemenang"
        >
          Buka Semua
        </button>
        <button
          type="button"
          onclick={collapseAll}
          class="h-8 px-2.5 text-[11px] font-medium text-zinc-400 hover:text-white hover:bg-white/[0.05] rounded-xl border border-white/[0.06] transition-colors cursor-pointer"
          title="Tutup seluruh rincian pemenang"
        >
          Tutup Semua
        </button>
      </div>
    </div>
  </div>

  <!-- Loading State -->
  {#if isLoading}
    <div class="flex-1 flex flex-col items-center justify-center p-12 text-zinc-400">
      <svg class="w-8 h-8 text-amber-400 animate-spin mb-3" fill="none" viewBox="0 0 24 24">
        <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
        <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
      </svg>
      <p class="text-xs font-mono tracking-wider uppercase text-zinc-300">Memuat Klasemen Medali...</p>
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
        onclick={loadSummaryData}
        class="mt-4 px-4 py-2 bg-rose-500/20 hover:bg-rose-500/30 text-rose-200 text-xs font-semibold rounded-xl border border-rose-500/30 transition-colors"
      >
        Coba Muat Ulang
      </button>
    </div>
  {:else if (summaryData?.total_gold ?? 0) === 0 && (summaryData?.total_silver ?? 0) === 0 && (summaryData?.total_bronze ?? 0) === 0}
    <!-- Empty State: Zero Medals Awarded Yet -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-2xl p-10 text-center text-zinc-400 my-auto max-w-lg mx-auto shadow-md">
      <div class="w-14 h-14 rounded-2xl bg-amber-500/10 border border-amber-500/20 text-amber-400 flex items-center justify-center mx-auto mb-3">
        <svg class="w-7 h-7" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 8v13m0-13V6a2 2 0 112 2h-2zm0 0V5.5A2.5 2.5 0 109.5 8H12zm-7 4h14M5 12a2 2 0 110-4h14a2 2 0 110 4M5 12v7a2 2 0 002 2h10a2 2 0 002-2v-7" />
        </svg>
      </div>
      <h3 class="text-sm font-bold text-white">Belum Ada Data Perolehan Medali</h3>
      <p class="text-xs text-zinc-400 mt-1.5 leading-relaxed">
        Pertandingan belum dimulai atau belum ada juara bagan yang disahkan untuk event <span class="text-amber-400 font-semibold">{eventName}</span>.
      </p>
      <p class="text-[11px] text-zinc-500 mt-1 leading-relaxed">
        Perolehan medali dan klasemen Juara Umum akan terhitung otomatis saat pertandingan selesai di Tatami.
      </p>
    </div>
  {:else if filteredList.length === 0}
    <!-- Empty State: Filter / Search yielding no results -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-2xl p-10 text-center text-zinc-400 my-auto max-w-md mx-auto shadow-md">
      <svg class="w-10 h-10 mx-auto text-zinc-600 mb-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9.172 16.172a4 4 0 015.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
      </svg>
      <p class="text-sm font-bold text-white">Tidak Ada Data yang Cocok</p>
      <p class="text-xs text-zinc-500 mt-1">Coba sesuaikan kata kunci pencarian atau matikan filter "Hanya Peraih Medali".</p>
      <button
        type="button"
        onclick={resetFilters}
        class="mt-4 px-3.5 py-1.5 bg-white/[0.05] hover:bg-white/[0.1] text-zinc-200 text-xs font-semibold rounded-xl border border-white/[0.1] transition-colors active:scale-95 cursor-pointer"
      >
        Reset Filter
      </button>
    </div>
  {:else}
    <!-- Main Standings Table (DESIGN_SYSTEM.md Section 4.D) -->
    <div class="rounded-2xl border border-white/[0.08] bg-[#181a20] overflow-hidden shadow-lg flex-1 flex flex-col min-h-0">
      <!-- Standings Table Header (Sticky) -->
      <div class="sticky top-0 z-10 bg-[#111215] text-zinc-400 font-mono text-[11px] uppercase tracking-wider border-b border-white/[0.08] px-4 py-2.5 grid grid-cols-12 gap-2 items-center shrink-0 select-none">
        <div class="col-span-1 text-center font-bold">Rank</div>
        <div class="col-span-5 sm:col-span-6 font-bold truncate">
          {activeTab === "perguruan" ? "PERGURUAN (FORKI)" : "KONTINGEN / UTUSAN"}
        </div>
        <div class="col-span-1 text-center text-amber-400 font-black">EMAS</div>
        <div class="col-span-1 text-center text-slate-300 font-bold">PERAK</div>
        <div class="col-span-1 text-center text-amber-600 font-bold">PERUNGGU</div>
        <div class="col-span-1 text-center text-white font-black">TOTAL</div>
        <div class="col-span-2 sm:col-span-1 text-right font-bold pr-1">RINCIAN</div>
      </div>

      <!-- Standings Table Body (Scrollable) -->
      <div class="overflow-y-auto divide-y divide-white/[0.06] flex-1">
        {#each filteredList as item (item.id)}
          {@const isExpanded = !!expandedRows[item.id]}
          {@const hasMedals = item.total > 0 || item.uncounted_gold > 0}

          <div class="transition-colors hover:bg-white/[0.02]">
            <!-- Row Click Header -->
            <div
              role="button"
              tabindex="0"
              onclick={() => toggleRow(item.id)}
              onkeydown={(e) => { if (e.key === 'Enter') toggleRow(item.id); }}
              class="grid grid-cols-12 gap-2 px-4 py-3 items-center cursor-pointer select-none"
            >
              <!-- Rank Column -->
              <div class="col-span-1 flex items-center justify-center">
                {#if item.rank === 1 && hasMedals}
                  <span class="w-6 h-6 rounded-full bg-amber-400 text-slate-950 font-black font-mono text-xs flex items-center justify-center shadow-md">
                    1
                  </span>
                {:else if item.rank === 2 && hasMedals}
                  <span class="w-6 h-6 rounded-full bg-slate-300 text-slate-950 font-black font-mono text-xs flex items-center justify-center shadow-md">
                    2
                  </span>
                {:else if item.rank === 3 && hasMedals}
                  <span class="w-6 h-6 rounded-full bg-[#d97706] text-white font-black font-mono text-xs flex items-center justify-center shadow-md">
                    3
                  </span>
                {:else}
                  <span class="font-mono text-xs font-bold text-zinc-500 tabular-nums">
                    {item.rank}
                  </span>
                {/if}
              </div>

              <!-- Name & Uncounted Gold Badge -->
              <div class="col-span-5 sm:col-span-6 flex items-center space-x-2.5 truncate pr-2">
                {#if item.logo && logoMap[item.logo]}
                  <img
                    src={logoMap[item.logo]}
                    alt=""
                    class="w-5 h-5 object-contain shrink-0 rounded bg-white/10 p-0.5 print:bg-transparent"
                    onerror={(e) => { (e.currentTarget as HTMLElement).style.display = 'none'; }}
                  />
                {/if}
                <span class="text-xs sm:text-sm font-black text-white truncate tracking-tight uppercase print:text-black">
                  {item.nama}
                </span>
                {#if item.uncounted_gold > 0}
                  <span
                    class="text-[9px] font-bold px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-300 border border-amber-500/25 shrink-0 font-mono tracking-tight print:text-amber-800 print:border-amber-400"
                    title="{item.uncounted_gold} Medali Emas Tanpa Lawan (Tidak Dihitung ke Poin Juara Umum)"
                  >
                    +{item.uncounted_gold} Tanpa Lawan
                  </span>
                {/if}
              </div>

              <!-- Gold Medals -->
              <div class="col-span-1 text-center font-mono text-sm font-black text-amber-300 tabular-nums print:text-amber-800">
                {item.gold}
              </div>

              <!-- Silver Medals -->
              <div class="col-span-1 text-center font-mono text-sm font-bold text-slate-200 tabular-nums print:text-slate-700">
                {item.silver}
              </div>

              <!-- Bronze Medals -->
              <div class="col-span-1 text-center font-mono text-sm font-bold text-amber-600 tabular-nums print:text-amber-900">
                {item.bronze}
              </div>

              <!-- Total Medals -->
              <div class="col-span-1 text-center font-mono text-sm font-black text-white tabular-nums print:text-black">
                {item.total}
              </div>

              <!-- Rincian Accordion Toggle Button -->
              <div class="col-span-2 sm:col-span-1 flex items-center justify-end space-x-1.5 text-zinc-400 pr-1">
                <span class="text-[10px] font-mono text-zinc-500 hidden sm:inline">[{item.winners.length}]</span>
                <svg
                  class="w-4 h-4 transform transition-transform duration-200 {isExpanded ? 'rotate-180 text-amber-400' : 'text-zinc-500'}"
                  fill="none"
                  stroke="currentColor"
                  viewBox="0 0 24 24"
                >
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
                </svg>
              </div>
            </div>

            <!-- Expanded Medalists Athlete Cards Grid -->
            {#if isExpanded}
              <div class="px-4 py-3 bg-[#111216]/60 border-t border-white/[0.06] transition-colors">
                <div class="text-[10px] font-mono font-bold text-zinc-400 uppercase tracking-wider mb-2.5 flex items-center justify-between">
                  <span>Rincian Atlet Peraih Medali</span>
                  <span class="text-zinc-500">Total: {item.winners.length} atlet</span>
                </div>

                {#if item.winners.length === 0}
                  <div class="py-3 text-center text-xs text-zinc-500 italic">
                    Belum ada perolehan medali yang tercatat untuk {item.nama}.
                  </div>
                {:else}
                  <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-2.5">
                    {#each item.winners as winner}
                      <div class="bg-[#181a20] border {winner.is_counted ? (winner.juara === '1' ? 'border-amber-500/30 bg-amber-500/[0.03]' : 'border-white/[0.08]') : 'border-amber-500/40 bg-amber-500/[0.05]'} rounded-xl p-2.5 flex items-start space-x-2.5">
                        <!-- Medal Position Badge -->
                        <div class="shrink-0 mt-0.5">
                          {#if winner.juara === "1"}
                            <span class="w-6 h-6 rounded-lg bg-amber-400/20 text-amber-300 border border-amber-400/40 font-black font-mono text-xs flex items-center justify-center shadow-xs">
                              1
                            </span>
                          {:else if winner.juara === "2"}
                            <span class="w-6 h-6 rounded-lg bg-slate-300/15 text-slate-200 border border-slate-300/30 font-black font-mono text-xs flex items-center justify-center shadow-xs">
                              2
                            </span>
                          {:else}
                            <span class="w-6 h-6 rounded-lg bg-amber-700/20 text-amber-500 border border-amber-700/40 font-black font-mono text-xs flex items-center justify-center shadow-xs">
                              3
                            </span>
                          {/if}
                        </div>

                        <!-- Athlete & Category Details -->
                        <div class="min-w-0 flex-1 leading-tight">
                          <div class="flex items-center space-x-1.5 flex-wrap">
                            <span class="text-xs font-bold text-white uppercase truncate">
                              {winner.nama_atlet}
                            </span>
                            {#if !winner.is_counted}
                              <span class="text-[9px] font-bold px-1.5 py-0.2 rounded bg-amber-500/20 text-amber-300 border border-amber-500/40 font-mono">
                                (Tanpa Lawan)
                              </span>
                            {/if}
                          </div>

                          <!-- STRICT DOMAIN RULE: Perguruan first, then Kontingen -->
                          <div class="text-[10px] font-mono text-zinc-400 truncate mt-1 tracking-tight">
                            <span class="text-zinc-200 font-semibold">{winner.perguruan_nama}</span>
                            <span class="text-zinc-500 mx-1">•</span>
                            <span class="text-zinc-400">{winner.utusan_nama}</span>
                          </div>

                          <!-- Match Bracket Info -->
                          <div class="text-[10px] font-semibold {winner.is_counted ? 'text-amber-300/90' : 'text-zinc-400'} truncate mt-1">
                            {#if winner.bagan_kode}
                              <span class="font-mono text-amber-400/80 mr-1">[{winner.bagan_kode}]</span>
                            {/if}
                            <span>{winner.bagan_nama}</span>
                          </div>
                        </div>
                      </div>
                    {/each}
                  </div>
                {/if}
              </div>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {/if}
  </div>

  <!-- ================= PRINT VIEW: MENGAMBIL DARI KOMPONEN BERSAMA KLASEMEN ================= -->
  <section class="print-only hidden booklet-page-portrait bg-white text-slate-900 p-8 print:p-0 rounded-md shadow-none border-none">
    <KlasemenMedaliPrintSheet
      standingsKontingen={summaryData?.utusan_standings ?? []}
      standingsPerguruan={summaryData?.perguruan_standings ?? []}
      totalGold={summaryData?.total_gold ?? 0}
      totalSilver={summaryData?.total_silver ?? 0}
      totalBronze={summaryData?.total_bronze ?? 0}
      grandTotal={(summaryData?.total_gold ?? 0) + (summaryData?.total_silver ?? 0) + (summaryData?.total_bronze ?? 0)}
      eventName={summaryData?.event_name || eventName}
      pageLabelKontingen="Klasemen Perolehan Medali Kontingen"
      pageLabelPerguruan="Klasemen Perolehan Medali Perguruan"
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
    :global(.print-sheet div) {
      border-color: #cbd5e1 !important;
    }
    .no-print {
      display: none !important;
    }
    .print-only {
      display: block !important;
    }
  }
</style>
