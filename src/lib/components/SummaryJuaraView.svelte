<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { SummaryJuaraResult, StandingItem, SummaryWinnerAthlete } from "$lib/types";

  interface Props {
    eventId: number;
    eventName?: string;
    hostAddress?: string;
  }

  let { eventId, eventName = "", hostAddress = "" }: Props = $props();

  let summaryData = $state<SummaryJuaraResult | null>(null);
  let isLoading = $state(true);
  let activeTab = $state<"kontingen" | "perguruan">("kontingen");
  let searchQuery = $state("");
  let filterOnlyWithMedals = $state(true);
  let expandedRows = $state<Record<number, boolean>>({});

  async function loadSummaryData() {
    isLoading = true;
    try {
      const res = await invoke<SummaryJuaraResult>("get_summary_juara", { eventId });
      summaryData = res;
    } catch (err: any) {
      console.error("Gagal memuat summary juara:", err);
      // Fallback for mock/browser test environments
      summaryData = {
        event_name: eventName || "KEJUARAAN NASIONAL KARATE 2026",
        utusan_standings: [
          {
            id: 1,
            rank: 1,
            nama: "KKI SAMARINDA",
            logo: null,
            gold: 1,
            silver: 0,
            bronze: 1,
            total: 2,
            uncounted_gold: 1,
            winners: [
              {
                juara: "1",
                nama_atlet: "Rudi",
                utusan_nama: "KKI SAMARINDA",
                perguruan_nama: "KKI",
                bagan_nama: "Kata Perorangan Putra",
                bagan_kode: "A-01",
                is_counted: false,
                keterangan: "Tidak dihitung (Tanpa Lawan)",
              },
              {
                juara: "1",
                nama_atlet: "Isya",
                utusan_nama: "KKI SAMARINDA",
                perguruan_nama: "KKI",
                bagan_nama: "Kata Perorangan Putri",
                bagan_kode: "A-02",
                is_counted: true,
                keterangan: null,
              },
              {
                juara: "3a",
                nama_atlet: "Masud",
                utusan_nama: "KKI SAMARINDA",
                perguruan_nama: "KKI",
                bagan_nama: "Kumite -55 KG Putra",
                bagan_kode: "K-01",
                is_counted: true,
                keterangan: null,
              },
            ],
          },
          {
            id: 2,
            rank: 2,
            nama: "INKAI JAWA TIMUR",
            logo: null,
            gold: 1,
            silver: 1,
            bronze: 0,
            total: 2,
            uncounted_gold: 0,
            winners: [
              {
                juara: "1",
                nama_atlet: "Ahmad Fauzi",
                utusan_nama: "INKAI JAWA TIMUR",
                perguruan_nama: "INKAI",
                bagan_nama: "Kumite -67 KG Senior Putra",
                bagan_kode: "K-02",
                is_counted: true,
                keterangan: null,
              },
              {
                juara: "2",
                nama_atlet: "Budi Santoso",
                utusan_nama: "INKAI JAWA TIMUR",
                perguruan_nama: "INKAI",
                bagan_nama: "Kata Beregu Putra",
                bagan_kode: "A-03",
                is_counted: true,
                keterangan: null,
              },
            ],
          },
        ],
        perguruan_standings: [
          {
            id: 1,
            rank: 1,
            nama: "KKI",
            logo: null,
            gold: 1,
            silver: 0,
            bronze: 1,
            total: 2,
            uncounted_gold: 1,
            winners: [],
          },
          {
            id: 2,
            rank: 2,
            nama: "INKAI",
            logo: null,
            gold: 1,
            silver: 1,
            bronze: 0,
            total: 2,
            uncounted_gold: 0,
            winners: [],
          },
        ],
        total_gold: 2,
        total_silver: 1,
        total_bronze: 1,
        total_uncounted: 1,
      };
    } finally {
      isLoading = false;
    }
  }

  onMount(() => {
    loadSummaryData();
  });

  // Current list based on active tab
  let currentList = $derived.by(() => {
    if (!summaryData) return [];
    return activeTab === "kontingen"
      ? summaryData.utusan_standings
      : summaryData.perguruan_standings;
  });

  // Filtered list by search & medal presence
  let filteredList = $derived.by(() => {
    const q = searchQuery.toLowerCase().trim();
    return currentList.filter((item) => {
      const matchSearch = !q || item.nama.toLowerCase().includes(q);
      const matchMedal = !filterOnlyWithMedals || item.total > 0 || item.uncounted_gold > 0;
      return matchSearch && matchMedal;
    });
  });

  // Top 3 Podium items
  let top3 = $derived.by(() => {
    return currentList.filter((x) => x.total > 0).slice(0, 3);
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
</script>

<div class="h-full flex flex-col p-4 select-none bg-[#14161b] text-zinc-200 overflow-hidden font-sans print-sheet">
  <!-- Top Bar -->
  <div class="no-print flex items-center justify-between pb-3 mb-3 border-b border-white/[0.08] shrink-0">
    <div>
      <div class="flex items-center space-x-2.5">
        <span class="w-2.5 h-2.5 rounded-full bg-amber-400"></span>
        <h1 class="text-sm md:text-base font-black tracking-tight text-white uppercase">
          Summary Juara & Klasemen Medali
        </h1>
        <span class="text-xs text-zinc-500 font-mono">
          {summaryData?.event_name || eventName || "Turnamen Karate"}
        </span>
      </div>
      <p class="text-[11px] text-zinc-400 mt-0.5">
        Perhitungan klasemen Juara Umum resmi (Kategori tanpa lawan mendapatkan medali emas namun tidak dihitung poin juara umum)
      </p>
    </div>

    <!-- Right Actions -->
    <div class="flex items-center space-x-2">
      <button
        type="button"
        onclick={() => window.print()}
        class="px-3 py-1.5 bg-white/[0.05] hover:bg-white/[0.08] text-zinc-200 border-white/[0.1] text-xs font-semibold border rounded-lg transition flex items-center space-x-1.5 shadow-xs active:scale-95"
        title="Cetak Rekapan Juara (A4 Landscape)"
      >
        <svg class="w-3.5 h-3.5 text-zinc-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
        </svg>
        <span>Cetak (A4)</span>
      </button>

      <button
        type="button"
        onclick={loadSummaryData}
        class="px-3 py-1.5 bg-white/[0.05] hover:bg-white/[0.08] text-zinc-200 border-white/[0.1] text-xs font-semibold border rounded-lg transition flex items-center space-x-1.5 shadow-xs active:scale-95"
      >
        <svg class="w-3.5 h-3.5 text-zinc-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
        </svg>
        <span>Segarkan</span>
      </button>
    </div>
  </div>

  <!-- Overall Quick Stats Header Cards -->
  <div class="grid grid-cols-2 sm:grid-cols-4 gap-2.5 mb-3 shrink-0 no-print">
    <div class="bg-[#181a20] border border-amber-500/30 rounded-xl p-2.5 flex items-center space-x-3">
      <div class="w-9 h-9 rounded-lg bg-amber-500/20 text-amber-300 border border-amber-500/40 flex items-center justify-center text-lg font-black font-mono shrink-0">
        1
      </div>
      <div class="min-w-0">
        <div class="text-[10px] uppercase font-bold text-amber-400 tracking-wider">Emas Terhitung</div>
        <div class="text-lg font-black text-white leading-tight">{summaryData?.total_gold ?? 0}</div>
      </div>
    </div>

    <div class="bg-[#181a20] border border-slate-400/20 rounded-xl p-2.5 flex items-center space-x-3">
      <div class="w-9 h-9 rounded-lg bg-slate-300/15 text-slate-200 border border-slate-300/30 flex items-center justify-center text-lg font-black font-mono shrink-0">
        2
      </div>
      <div class="min-w-0">
        <div class="text-[10px] uppercase font-bold text-slate-300 tracking-wider">Perak Terhitung</div>
        <div class="text-lg font-black text-white leading-tight">{summaryData?.total_silver ?? 0}</div>
      </div>
    </div>

    <div class="bg-[#181a20] border border-amber-700/30 rounded-xl p-2.5 flex items-center space-x-3">
      <div class="w-9 h-9 rounded-lg bg-amber-700/20 text-amber-500 border border-amber-700/40 flex items-center justify-center text-lg font-black font-mono shrink-0">
        3
      </div>
      <div class="min-w-0">
        <div class="text-[10px] uppercase font-bold text-amber-600 tracking-wider">Perunggu Terhitung</div>
        <div class="text-lg font-black text-white leading-tight">{summaryData?.total_bronze ?? 0}</div>
      </div>
    </div>

    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-2.5 flex items-center space-x-3">
      <div class="w-9 h-9 rounded-lg bg-zinc-800 text-zinc-400 border border-white/10 flex items-center justify-center text-xs font-bold font-mono shrink-0">
        TL
      </div>
      <div class="min-w-0">
        <div class="text-[10px] uppercase font-bold text-zinc-400 tracking-wider">Tanpa Lawan</div>
        <div class="text-lg font-black text-zinc-300 leading-tight">
          {summaryData?.total_uncounted ?? 0}
          <span class="text-[10px] font-normal text-zinc-500 ml-1">(Emas mandiri)</span>
        </div>
      </div>
    </div>
  </div>

  <!-- Navigation Switcher & Search Bar -->
  <div class="flex items-center justify-between gap-3 mb-3 shrink-0 no-print">
    <!-- Tab Switcher -->
    <div class="flex items-center bg-[#101216] p-1 rounded-xl border border-white/[0.08]">
      <button
        type="button"
        onclick={() => (activeTab = "kontingen")}
        class="px-3.5 py-1.5 rounded-lg text-xs font-bold transition flex items-center space-x-1.5 {activeTab === 'kontingen' ? 'bg-amber-500 text-slate-950 shadow-sm' : 'text-zinc-400 hover:text-white'}"
      >
        <span>Berdasarkan Kontingen</span>
        <span class="text-[10px] font-mono opacity-80">({summaryData?.utusan_standings.length ?? 0})</span>
      </button>

      <button
        type="button"
        onclick={() => (activeTab = "perguruan")}
        class="px-3.5 py-1.5 rounded-lg text-xs font-bold transition flex items-center space-x-1.5 {activeTab === 'perguruan' ? 'bg-amber-500 text-slate-950 shadow-sm' : 'text-zinc-400 hover:text-white'}"
      >
        <span>Berdasarkan Perguruan</span>
        <span class="text-[10px] font-mono opacity-80">({summaryData?.perguruan_standings.length ?? 0})</span>
      </button>
    </div>

    <!-- Search & Filters -->
    <div class="flex items-center space-x-2">
      <div class="relative">
        <input
          type="text"
          bind:value={searchQuery}
          placeholder="Cari nama {activeTab === 'kontingen' ? 'kontingen' : 'perguruan'}..."
          class="w-56 text-xs bg-[#181a20] text-white placeholder-zinc-500 px-3 py-1.5 pl-8 rounded-lg border border-white/[0.1] focus:outline-none focus:ring-1 focus:ring-amber-500"
        />
        <svg class="w-3.5 h-3.5 text-zinc-500 absolute left-2.5 top-1/2 -translate-y-1/2" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" />
        </svg>
      </div>

      <button
        type="button"
        onclick={() => (filterOnlyWithMedals = !filterOnlyWithMedals)}
        class="px-2.5 py-1.5 text-xs font-semibold rounded-lg border transition {filterOnlyWithMedals ? 'bg-amber-500/15 border-amber-500/40 text-amber-300' : 'bg-white/[0.04] border-white/[0.1] text-zinc-400'}"
      >
        {filterOnlyWithMedals ? "Hanya Peraih Medali" : "Semua Terdaftar"}
      </button>

      <div class="h-4 w-px bg-white/[0.1] mx-0.5"></div>

      <button
        type="button"
        onclick={expandAll}
        class="px-2 py-1 text-[11px] font-medium text-zinc-400 hover:text-white hover:bg-white/[0.05] rounded transition"
        title="Buka semua rincian"
      >
        Buka Semua
      </button>
      <button
        type="button"
        onclick={collapseAll}
        class="px-2 py-1 text-[11px] font-medium text-zinc-400 hover:text-white hover:bg-white/[0.05] rounded transition"
        title="Tutup semua rincian"
      >
        Tutup Semua
      </button>
    </div>
  </div>

  <!-- Main Scrollable List Area -->
  <div class="flex-1 overflow-y-auto pr-1 space-y-2 scrollbar-thin">
    {#if isLoading}
      <div class="h-64 flex flex-col items-center justify-center space-y-2 text-zinc-400">
        <div class="w-6 h-6 border-2 border-amber-500 border-t-transparent rounded-full animate-spin"></div>
        <span class="text-xs">Memuat klasemen dan medali...</span>
      </div>
    {:else if filteredList.length === 0}
      <div class="h-64 flex flex-col items-center justify-center text-center p-6 bg-[#101216] rounded-xl border border-white/[0.06]">
        <div class="text-zinc-500 text-sm font-semibold">Tidak ada data yang cocok dengan kriteria pencarian.</div>
        <p class="text-zinc-600 text-xs mt-1">Coba sesuaikan kata kunci atau matikan filter "Hanya Peraih Medali".</p>
      </div>
    {:else}
      <!-- Standings Table Header -->
      <div class="grid grid-cols-12 gap-2 px-4 py-2 text-[10px] font-mono font-bold uppercase tracking-wider text-zinc-400 bg-[#101216] rounded-lg border border-white/[0.06]">
        <div class="col-span-1 text-center">Rank</div>
        <div class="col-span-6">{activeTab === "kontingen" ? "KONTINGEN / UTUSAN" : "PERGURUAN"}</div>
        <div class="col-span-1 text-center text-amber-400 font-black">EMAS</div>
        <div class="col-span-1 text-center text-slate-300 font-bold">PERAK</div>
        <div class="col-span-1 text-center text-amber-600 font-bold">PERUNGGU</div>
        <div class="col-span-1 text-center text-white font-black">TOTAL</div>
        <div class="col-span-1 text-right">RINCIAN</div>
      </div>

      <!-- Standings Rows -->
      {#each filteredList as item (item.id)}
        {@const isExpanded = !!expandedRows[item.id]}
        {@const hasMedals = item.total > 0 || item.uncounted_gold > 0}
        <div class="bg-[#181a20] border {item.rank === 1 && hasMedals ? 'border-amber-500/60 shadow-[0_0_15px_rgba(245,158,11,0.08)]' : (item.rank === 2 && hasMedals ? 'border-slate-300/40' : (item.rank === 3 && hasMedals ? 'border-amber-700/40' : 'border-white/[0.08]'))} rounded-xl overflow-hidden transition">
          <!-- Main Card Header Bar -->
          <div
            role="button"
            tabindex="0"
            onclick={() => toggleRow(item.id)}
            onkeydown={(e) => { if (e.key === 'Enter') toggleRow(item.id); }}
            class="grid grid-cols-12 gap-2 px-4 py-3 items-center cursor-pointer hover:bg-white/[0.03] transition select-none"
          >
            <!-- Rank Badge -->
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
                <span class="font-mono text-xs font-bold text-zinc-500">
                  {item.rank}
                </span>
              {/if}
            </div>

            <!-- Name & Logo -->
            <div class="col-span-6 flex items-center space-x-2.5 truncate pr-2">
              <span class="text-sm font-black text-white truncate tracking-tight">
                {item.nama}
              </span>
              {#if item.uncounted_gold > 0}
                <span class="text-[9.5px] font-bold px-1.5 py-0.2 rounded bg-amber-500/15 text-amber-300 border border-amber-500/30 shrink-0" title="{item.uncounted_gold} Medali Emas Tanpa Lawan (Tidak Dihitung ke Poin Juara Umum)">
                  +{item.uncounted_gold} Tanpa Lawan
                </span>
              {/if}
            </div>

            <!-- Gold -->
            <div class="col-span-1 text-center font-mono text-sm font-black text-amber-300">
              {item.gold}
            </div>

            <!-- Silver -->
            <div class="col-span-1 text-center font-mono text-sm font-bold text-slate-200">
              {item.silver}
            </div>

            <!-- Bronze -->
            <div class="col-span-1 text-center font-mono text-sm font-bold text-amber-600">
              {item.bronze}
            </div>

            <!-- Total -->
            <div class="col-span-1 text-center font-mono text-sm font-black text-white">
              {item.total}
            </div>

            <!-- Chevron Dropdown Toggle -->
            <div class="col-span-1 flex items-center justify-end space-x-1.5 text-zinc-400">
              <span class="text-[10px] font-mono">{item.winners.length}</span>
              <svg
                class="w-4 h-4 transform transition-transform {isExpanded ? 'rotate-180 text-amber-400' : 'text-zinc-500'}"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" />
              </svg>
            </div>
          </div>

          <!-- Expanded Accordion Content: List of Winning Athletes -->
          {#if isExpanded}
            <div class="px-4 py-3 bg-[#101216] border-t border-white/[0.08] transition">
              <div class="text-[10.5px] font-mono font-bold text-zinc-400 uppercase tracking-wider mb-2 flex items-center justify-between">
                <span>Rincian Atlet Peraih Medali</span>
                <span class="text-zinc-500">Total: {item.winners.length} atlet</span>
              </div>

              {#if item.winners.length === 0}
                <div class="py-3 text-center text-xs text-zinc-500 italic">
                  Belum ada perolehan medali yang tercatat untuk {item.nama}.
                </div>
              {:else}
                <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-2">
                  {#each item.winners as winner}
                    <div class="bg-[#181a20] border {winner.is_counted ? (winner.juara === '1' ? 'border-amber-500/30 bg-amber-500/[0.03]' : 'border-white/[0.08]') : 'border-amber-500/40 bg-amber-500/[0.05]'} rounded-lg p-2.5 flex items-start space-x-2.5">
                      <!-- Medal Number Badge -->
                      <div class="shrink-0 mt-0.5">
                        {#if winner.juara === "1"}
                          <span class="w-5 h-5 rounded-md bg-amber-400 text-slate-950 font-black font-mono text-[11px] flex items-center justify-center shadow-xs">
                            1
                          </span>
                        {:else if winner.juara === "2"}
                          <span class="w-5 h-5 rounded-md bg-slate-300 text-slate-950 font-black font-mono text-[11px] flex items-center justify-center shadow-xs">
                            2
                          </span>
                        {:else}
                          <span class="w-5 h-5 rounded-md bg-[#d97706] text-white font-black font-mono text-[11px] flex items-center justify-center shadow-xs">
                            3
                          </span>
                        {/if}
                      </div>

                      <!-- Athlete & Category Detail -->
                      <div class="min-w-0 flex-1 leading-tight">
                        <div class="flex items-center space-x-1.5 flex-wrap">
                          <span class="text-xs font-bold text-white truncate">
                            {winner.nama_atlet}
                          </span>
                          {#if !winner.is_counted}
                            <span class="text-[9px] font-bold px-1.5 py-0.2 rounded bg-amber-500/20 text-amber-300 border border-amber-500/40 font-mono">
                              (Tidak dihitung)
                            </span>
                          {/if}
                        </div>

                        <div class="text-[10px] text-zinc-400 truncate mt-0.5 font-medium">
                          {activeTab === "kontingen" ? winner.perguruan_nama : winner.utusan_nama}
                        </div>

                        <div class="text-[10px] font-semibold {winner.is_counted ? 'text-amber-300/90' : 'text-zinc-400'} truncate mt-1">
                          {#if winner.bagan_kode}
                            <span class="font-mono text-zinc-500 mr-1">({winner.bagan_kode})</span>
                          {/if}
                          {winner.bagan_nama}
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
    {/if}
  </div>
</div>
