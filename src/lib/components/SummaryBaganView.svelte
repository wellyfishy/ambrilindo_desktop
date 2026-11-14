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

  // Stat cards double as the status filter.
  let statusTabs = $derived<{ key: typeof filterStatus; label: string; count: number }[]>([
    { key: "all", label: "Semua bagan", count: summaryData?.total_bagans ?? 0 },
    { key: "completed", label: "Sudah ada juara", count: summaryData?.completed_bagans ?? 0 },
    { key: "uncompleted", label: "Belum selesai", count: Math.max(0, (summaryData?.total_bagans ?? 0) - (summaryData?.completed_bagans ?? 0)) },
  ]);
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
  <!-- ================= SCREEN VIEW: INTERACTIVE DARK THEME UI ================= -->
  <div class="no-print flex flex-col flex-1 min-h-0">
    <!-- Header -->
    <div class="flex flex-wrap items-center justify-between gap-3 pb-4 border-b border-white/[0.08] shrink-0">
      <div class="flex items-center gap-2.5 min-w-0">
        <h1 class="text-xl font-black tracking-tight text-white uppercase">Juara per bagan</h1>
        <span class="px-2 py-0.5 rounded text-[11px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1] tabular-nums">{summaryData?.total_bagans ?? 0}</span>
      </div>
      <button
        type="button"
        onclick={() => printWithOrientation("portrait", "8mm")}
        class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-colors flex items-center gap-1.5 cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
        title="Cetak daftar juara yang sedang ditampilkan (A4 portrait)"
      >
        <svg class="w-4 h-4 text-zinc-400" fill="none" stroke="currentColor" stroke-width="2" viewBox="0 0 24 24" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" /></svg>
        <span>Cetak</span>
      </button>
    </div>

    <!-- Status: stat cards that are also the filter, plus the medal tally -->
    <div class="flex flex-wrap items-stretch gap-2 my-4 shrink-0">
      <div class="grid grid-cols-3 gap-2 flex-1 min-w-[320px]" role="radiogroup" aria-label="Filter status bagan">
        {#each statusTabs as tab (tab.key)}
          {@const selected = filterStatus === tab.key}
          <button
            type="button"
            role="radio"
            aria-checked={selected}
            onclick={() => (filterStatus = tab.key)}
            class="text-left rounded-xl px-3 py-2.5 border transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40 {selected ? 'bg-white/[0.08] border-white/50' : 'bg-[#181a20] border-white/[0.08] hover:border-white/20'}"
          >
            <span class="block text-xs font-semibold text-zinc-400">{tab.label}</span>
            <span class="block mt-1 text-2xl font-black font-mono tabular-nums text-white">{tab.count}</span>
          </button>
        {/each}
      </div>
      <div class="rounded-xl px-4 py-2.5 border border-white/[0.08] bg-[#181a20] flex items-center gap-5" aria-label="Jumlah medali">
        {#each [["Emas", summaryData?.total_gold ?? 0, "bg-amber-400"], ["Perak", summaryData?.total_silver ?? 0, "bg-slate-300"], ["Perunggu", summaryData?.total_bronze ?? 0, "bg-amber-700"]] as [label, count, dot]}
          <div>
            <span class="flex items-center gap-1.5 text-xs font-semibold text-zinc-400">
              <span class="w-2.5 h-2.5 rounded-full {dot}" aria-hidden="true"></span>{label}
            </span>
            <span class="block mt-1 text-2xl font-black font-mono tabular-nums text-white">{count}</span>
          </div>
        {/each}
      </div>
    </div>

    <!-- Filter & Search Toolbar (DESIGN_SYSTEM.md Section 4.C) -->
    <div class="mb-4 flex flex-wrap items-center gap-2 shrink-0">
      <!-- Search Bar -->
      <div class="relative flex-1 min-w-[200px]">
        <input
          type="text"
          bind:value={searchQuery}
          placeholder="Cari bagan, kode, atlet, perguruan, atau kontingen"
          aria-label="Cari bagan atau juara"
          class="w-full bg-[#181a20] border border-white/[0.1] rounded-xl pl-9 pr-3.5 h-9 text-sm text-white placeholder-zinc-500 focus:outline-none focus:border-white transition-colors select-text"
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
        class="h-9 bg-[#181a20] border border-white/[0.1] hover:border-white/20 rounded-xl px-3 text-sm text-zinc-200 focus:outline-none focus:border-white transition-colors cursor-pointer"
      >
        <option value="all">Semua divisi</option>
        <option value="prestasi">Prestasi</option>
        <option value="festival">Festival</option>
        <option value="bob">Best of the best (BOB)</option>
      </select>

      <!-- Type Filter (Kata / Kumite) -->
      <select
        bind:value={filterType}
        class="h-9 bg-[#181a20] border border-white/[0.1] hover:border-white/20 rounded-xl px-3 text-sm text-zinc-200 focus:outline-none focus:border-white transition-colors cursor-pointer"
      >
        <option value="all">Kata dan kumite</option>
        <option value="kata">Kata</option>
        <option value="kumite">Kumite</option>
      </select>

      <!-- Clear Filter Button (if filtered) -->
      {#if searchQuery || filterType !== "all" || filterDivision !== "all" || filterStatus !== "all"}
        <button
          type="button"
          onclick={resetFilters}
          class="h-9 px-3 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-400 hover:text-white text-xs font-semibold rounded-xl border border-white/[0.08] transition-colors flex items-center space-x-1 cursor-pointer"
        >
          <span>Reset filter</span>
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
        <p class="text-sm text-zinc-300" role="status">Memuat daftar juara…</p>
      </div>
    {:else if errorMessage}
      <div class="bg-rose-500/10 border border-rose-500/20 rounded-2xl p-6 text-center text-rose-300 my-auto max-w-lg mx-auto">
        <svg class="w-10 h-10 mx-auto text-rose-400 mb-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
        </svg>
        <h3 class="text-sm font-bold text-white mb-1">Daftar juara gagal dimuat</h3>
        <p class="text-xs text-rose-300/90">{errorMessage}</p>
        <button
          type="button"
          onclick={loadSummaryBagan}
          class="mt-4 px-4 py-2 bg-rose-500/20 hover:bg-rose-500/30 text-rose-200 text-xs font-semibold rounded-xl border border-rose-500/30 transition-colors"
        >
          Coba lagi
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
        <h3 class="text-sm font-bold text-white">Belum ada bagan</h3>
        <p class="text-sm text-zinc-400 mt-1.5 leading-relaxed">
          Lakukan drawing dari <strong class="text-zinc-200">Nomor tanding</strong> atau tombol <strong class="text-zinc-200">Drawing</strong> di daftar bagan.
        </p>
      </div>
    {:else if filteredBagans.length === 0}
      <!-- Empty State: Filter / Search yielding no results -->
      <div class="bg-[#181a20] border border-white/[0.08] rounded-2xl p-10 text-center text-zinc-400 my-auto max-w-md mx-auto shadow-md">
        <svg class="w-10 h-10 mx-auto text-zinc-600 mb-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9.172 16.172a4 4 0 015.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
        <p class="text-sm font-bold text-white">Tidak ada bagan yang cocok</p>
        <button
          type="button"
          onclick={resetFilters}
          class="mt-4 px-3.5 py-1.5 bg-white/[0.05] hover:bg-white/[0.1] text-zinc-200 text-xs font-semibold rounded-xl border border-white/[0.1] transition-colors active:scale-95 cursor-pointer"
        >
          Reset filter
        </button>
      </div>
    {:else}
      <!-- Main Bagan Cards Grid (Screen Mode) -->
      <div class="grid grid-cols-1 md:grid-cols-2 gap-3.5 items-start">
        {#each filteredBagans as bagan (bagan.id)}
          {@const div = getDivision(bagan)}
          {@const kata = isKata(bagan)}

          <div class="bg-[#181a20] border border-white/[0.08] hover:border-white/20 rounded-xl p-3 sm:p-3.5 transition-colors shadow-xs flex flex-col justify-between">
            <!-- Card Header -->
            <div class="flex items-center justify-between pb-2.5 mb-2.5 border-b border-white/[0.06] gap-2">
              <div class="flex items-center space-x-2 min-w-0">
                <!-- Bracket Code Badge e.g. [003] -->
                <span class="px-1.5 py-0.5 rounded-md bg-white/[0.06] text-zinc-300 border border-white/[0.1] font-mono font-bold text-xs shrink-0">
                  {bagan.kode}
                </span>

                <!-- Category / Bagan Name -->
                <h3 class="text-[13px] font-bold text-white break-words leading-tight" title={bagan.nama_bagan}>
                  {bagan.nama_bagan}
                </h3>
              </div>

              <span class="shrink-0 text-xs text-zinc-500 whitespace-nowrap">
                {kata ? "Kata" : "Kumite"}{#if div === "festival"} · <span class="text-amber-300/90">Festival</span>{:else if div === "bob"} · <span class="px-1.5 rounded bg-purple-500/15 text-purple-300 border border-purple-500/30 font-semibold">BOB</span>{/if}
              </span>
            </div>

            <!-- Card Body: Winners List -->
            {#if bagan.winners.length === 0}
              <div class="py-3 text-center text-sm text-zinc-500 rounded-lg border border-dashed border-white/[0.08]">
                Belum ada juara
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
                      <div class="w-6 h-6 rounded bg-white/[0.04] border border-white/[0.08] flex items-center justify-center text-[11px] font-bold text-zinc-400 shrink-0 uppercase overflow-hidden" aria-hidden="true">
                        {#if winner.logo && logoMap[winner.logo]}
                          <img src={logoMap[winner.logo]} alt="" class="w-full h-full object-contain" />
                        {:else}
                          <span>{winner.perguruan_nama ? winner.perguruan_nama.substring(0, 2) : "DO"}</span>
                        {/if}
                      </div>

                      <!-- Athlete Name -->
                      <div class="min-w-0">
                        <span class="text-[13px] font-semibold text-white truncate block select-text" title={winner.nama_atlet}>
                          {winner.nama_atlet}
                        </span>
                        {#if winner.is_uncounted}
                          <span class="text-[11px] text-amber-300/90 block">
                            Tanpa lawan
                          </span>
                        {/if}
                      </div>
                    </div>

                    <!-- Right: Affiliation (STRICT: Perguruan first, then Kontingen) -->
                    <div class="shrink-0 text-right min-w-[140px] max-w-[220px]">
                      <span class="text-xs text-zinc-200 truncate block select-text" title="{winner.utusan_nama} · {winner.perguruan_nama}">{winner.utusan_nama || "—"}</span>
                      <span class="text-[11px] text-zinc-500 truncate block select-text">{winner.perguruan_nama || ""}</span>
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
