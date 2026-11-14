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

<div class="h-full flex flex-col p-6 select-none bg-[#14161b] text-zinc-200 overflow-y-auto font-sans print-sheet">
  <!-- Screen View (Hidden when printing) -->
  <div class="no-print flex-1 flex flex-col min-h-0">
    <!-- Header -->
    <div class="flex flex-wrap items-center justify-between gap-3 pb-4 border-b border-white/[0.08] shrink-0">
      <div class="min-w-0">
        <h1 class="text-xl font-black tracking-tight text-white uppercase">Juara umum</h1>
        <p class="text-xs text-zinc-400 mt-1">Emas tanpa lawan tidak dihitung ke poin juara umum.</p>
      </div>
      <button
        type="button"
        onclick={() => printWithOrientation("portrait", "8mm")}
        class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors flex items-center gap-1.5 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
        title="Cetak klasemen medali (A4 portrait)"
      >
        <svg class="w-4 h-4 text-zinc-400" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" /></svg>
        <span>Cetak</span>
      </button>
    </div>

  <!-- Medal tally -->
  <div class="grid grid-cols-2 sm:grid-cols-4 gap-2 my-4 shrink-0 no-print" aria-label="Jumlah medali terhitung">
    {#each [["Emas", summaryData?.total_gold ?? 0, "bg-amber-400"], ["Perak", summaryData?.total_silver ?? 0, "bg-slate-300"], ["Perunggu", summaryData?.total_bronze ?? 0, "bg-amber-700"], ["Emas tanpa lawan", summaryData?.total_uncounted ?? 0, "bg-zinc-500"]] as [label, count, dot]}
      <div class="rounded-xl px-3 py-2.5 border border-white/[0.08] bg-[#181a20]">
        <span class="flex items-center gap-1.5 text-xs font-semibold text-zinc-400">
          <span class="w-2.5 h-2.5 rounded-full {dot}" aria-hidden="true"></span>{label}
        </span>
        <span class="block mt-1 text-2xl font-black font-mono tabular-nums text-white">{count}</span>
      </div>
    {/each}
  </div>

  <!-- Filter & Tab Switcher Toolbar (DESIGN_SYSTEM.md Section 4.C) -->
  <div class="no-print mb-3 flex flex-wrap items-center gap-2">
    <!-- Tab Switcher (Perguruan ALWAYS first, then Kontingen) -->
    <div class="flex items-center bg-[#181a20] p-1 rounded-xl border border-white/[0.08]" role="radiogroup" aria-label="Klasemen berdasarkan">
      <button
        type="button"
        role="radio"
        aria-checked={activeTab === "perguruan"}
        onclick={() => (activeTab = "perguruan")}
        class="h-8 px-3.5 rounded-lg text-xs font-semibold transition-all flex items-center gap-1.5 cursor-pointer {activeTab === 'perguruan' ? 'bg-white text-black shadow-sm' : 'text-zinc-400 hover:text-white'}"
      >
        <span>Perguruan</span>
        <span class="font-mono opacity-70 tabular-nums">{summaryData?.perguruan_standings.length ?? 0}</span>
      </button>

      <button
        type="button"
        role="radio"
        aria-checked={activeTab === "kontingen"}
        onclick={() => (activeTab = "kontingen")}
        class="h-8 px-3.5 rounded-lg text-xs font-semibold transition-all flex items-center gap-1.5 cursor-pointer {activeTab === 'kontingen' ? 'bg-white text-black shadow-sm' : 'text-zinc-400 hover:text-white'}"
      >
        <span>Kontingen</span>
        <span class="font-mono opacity-70 tabular-nums">{summaryData?.utusan_standings.length ?? 0}</span>
      </button>
    </div>

    <!-- Search & Filter Controls -->
    <div class="flex flex-1 flex-wrap items-center gap-2">
      <!-- Search Input -->
      <div class="relative flex-1 min-w-[200px]">
        <input
          type="text"
          bind:value={searchQuery}
          placeholder="Cari nama {activeTab === 'perguruan' ? 'perguruan' : 'kontingen'}"
          aria-label="Cari {activeTab === 'perguruan' ? 'perguruan' : 'kontingen'}"
          class="w-full bg-[#181a20] border border-white/[0.1] rounded-xl pl-9 pr-3.5 h-9 text-sm text-white placeholder-zinc-500 focus:outline-none focus:border-white transition-colors select-text"
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
        role="switch"
        aria-checked={filterOnlyWithMedals}
        onclick={() => (filterOnlyWithMedals = !filterOnlyWithMedals)}
        class="h-9 px-3 text-xs font-semibold rounded-xl border transition-colors flex items-center gap-2 cursor-pointer bg-white/[0.04] border-white/[0.1] text-zinc-200 hover:bg-white/[0.08]"
      >
        <span class="relative inline-block w-7 h-4 rounded-full transition {filterOnlyWithMedals ? 'bg-emerald-400' : 'bg-zinc-600'}" aria-hidden="true">
          <span class="absolute top-0.5 w-3 h-3 rounded-full bg-white transition-all {filterOnlyWithMedals ? 'left-3.5' : 'left-0.5'}"></span>
        </span>
        <span>Hanya peraih medali</span>
      </button>

      <div class="h-4 w-px bg-white/[0.1] mx-0.5 hidden sm:block"></div>

      <!-- Expand / Collapse All -->
      <div class="flex items-center space-x-1">
        <button
          type="button"
          onclick={expandAll}
          class="h-9 px-3 text-xs font-semibold text-zinc-300 hover:text-white hover:bg-white/[0.05] rounded-xl border border-white/[0.08] transition-colors cursor-pointer"
          title="Buka seluruh rincian pemenang"
        >
          Buka semua
        </button>
        <button
          type="button"
          onclick={collapseAll}
          class="h-9 px-3 text-xs font-semibold text-zinc-300 hover:text-white hover:bg-white/[0.05] rounded-xl border border-white/[0.08] transition-colors cursor-pointer"
          title="Tutup seluruh rincian pemenang"
        >
          Tutup semua
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
      <p class="text-sm text-zinc-300" role="status">Memuat klasemen…</p>
    </div>
  {:else if errorMessage}
    <div class="bg-rose-500/10 border border-rose-500/20 rounded-2xl p-6 text-center text-rose-300 my-auto max-w-lg mx-auto">
      <svg class="w-10 h-10 mx-auto text-rose-400 mb-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
      </svg>
      <h3 class="text-sm font-bold text-white mb-1">Klasemen gagal dimuat</h3>
      <p class="text-xs text-rose-300/90">{errorMessage}</p>
      <button
        type="button"
        onclick={loadSummaryData}
        class="mt-4 px-4 py-2 bg-rose-500/20 hover:bg-rose-500/30 text-rose-200 text-xs font-semibold rounded-xl border border-rose-500/30 transition-colors"
      >
        Coba lagi
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
      <h3 class="text-sm font-bold text-white">Belum ada perolehan medali</h3>
      <p class="text-sm text-zinc-400 mt-1.5 leading-relaxed">Klasemen terisi otomatis saat bagan selesai di tatami.</p>
    </div>
  {:else if filteredList.length === 0}
    <!-- Empty State: Filter / Search yielding no results -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-2xl p-10 text-center text-zinc-400 my-auto max-w-md mx-auto shadow-md">
      <svg class="w-10 h-10 mx-auto text-zinc-600 mb-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9.172 16.172a4 4 0 015.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
      </svg>
      <p class="text-sm font-bold text-white">Tidak ada yang cocok</p>
      <button
        type="button"
        onclick={resetFilters}
        class="mt-4 px-3.5 py-1.5 bg-white/[0.05] hover:bg-white/[0.1] text-zinc-200 text-xs font-semibold rounded-xl border border-white/[0.1] transition-colors active:scale-95 cursor-pointer"
      >
        Reset filter
      </button>
    </div>
  {:else}
    <!-- Main Standings Table (DESIGN_SYSTEM.md Section 4.D) -->
    <div class="rounded-2xl border border-white/[0.08] bg-[#181a20] overflow-hidden shadow-lg flex-1 flex flex-col min-h-0">
      <!-- Standings Table Header (Sticky) -->
      <div class="sticky top-0 z-10 bg-[#111215] text-zinc-400 text-xs font-semibold border-b border-white/[0.08] px-4 py-2.5 grid grid-cols-12 gap-2 items-center shrink-0 select-none">
        <div class="col-span-1 text-center">Peringkat</div>
        <div class="col-span-5 sm:col-span-6 truncate">{activeTab === "perguruan" ? "Perguruan" : "Kontingen"}</div>
        <div class="col-span-1 flex items-center justify-center gap-1"><span class="w-2 h-2 rounded-full bg-amber-400" aria-hidden="true"></span>Emas</div>
        <div class="col-span-1 flex items-center justify-center gap-1"><span class="w-2 h-2 rounded-full bg-slate-300" aria-hidden="true"></span>Perak</div>
        <div class="col-span-1 flex items-center justify-center gap-1"><span class="w-2 h-2 rounded-full bg-amber-700" aria-hidden="true"></span>Perunggu</div>
        <div class="col-span-1 text-center text-zinc-200">Total</div>
        <div class="col-span-2 sm:col-span-1 text-right pr-1"><span class="sr-only">Rincian</span></div>
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
              aria-expanded={isExpanded}
              aria-label="{item.nama}: {item.gold} emas, {item.silver} perak, {item.bronze} perunggu. {isExpanded ? 'Tutup' : 'Buka'} rincian"
              onclick={() => toggleRow(item.id)}
              onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); toggleRow(item.id); } }}
              class="grid grid-cols-12 gap-2 px-4 py-3 items-center cursor-pointer select-none focus-visible:outline-none focus-visible:bg-white/[0.04]"
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
                <span class="text-[13px] font-semibold text-white truncate print:text-black">
                  {item.nama}
                </span>
                {#if item.uncounted_gold > 0}
                  <span
                    class="text-[11px] font-semibold px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-300 border border-amber-500/25 shrink-0 print:text-amber-800 print:border-amber-400"
                    title="{item.uncounted_gold} Medali Emas Tanpa Lawan (Tidak Dihitung ke Poin Juara Umum)"
                  >
                    +{item.uncounted_gold} tanpa lawan
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
                <span class="text-xs font-mono text-zinc-500 hidden sm:inline tabular-nums" title="Jumlah atlet peraih medali">{item.winners.length}</span>
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
                <div class="text-xs font-semibold text-zinc-400 mb-2.5 flex items-center justify-between">
                  <span>Atlet peraih medali</span>
                  <span class="text-zinc-500 tabular-nums">{item.winners.length} atlet</span>
                </div>

                {#if item.winners.length === 0}
                  <div class="py-3 text-center text-sm text-zinc-500">
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
                            <span class="text-[13px] font-semibold text-white truncate select-text">
                              {winner.nama_atlet}
                            </span>
                            {#if !winner.is_counted}
                              <span class="text-[11px] font-semibold px-1.5 rounded bg-amber-500/15 text-amber-300 border border-amber-500/35">
                                Tanpa lawan
                              </span>
                            {/if}
                          </div>

                          <!-- STRICT DOMAIN RULE: Perguruan first, then Kontingen -->
                          <div class="text-xs text-zinc-400 truncate mt-1 select-text">
                            <span class="text-zinc-200">{winner.utusan_nama}</span>
                            <span class="text-zinc-600 mx-1" aria-hidden="true">·</span>
                            <span>{winner.perguruan_nama}</span>
                          </div>

                          <!-- Match Bracket Info -->
                          <div class="text-xs {winner.is_counted ? 'text-zinc-300' : 'text-zinc-500'} truncate mt-1">
                            {#if winner.bagan_kode}
                              <span class="font-mono text-zinc-500 mr-1">{winner.bagan_kode}</span>
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
