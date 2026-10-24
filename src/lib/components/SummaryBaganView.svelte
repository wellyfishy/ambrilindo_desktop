<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { SummaryBaganResult, SummaryBaganItem } from "$lib/types";

  interface Props {
    eventId: number;
    eventName?: string;
    hostAddress?: string;
  }

  let { eventId, eventName = "Event Turnamen", hostAddress }: Props = $props();

  let isLoading = $state(true);
  let errorMessage = $state<string | null>(null);
  let summaryData = $state<SummaryBaganResult | null>(null);

  // Filters & Search
  let searchQuery = $state("");
  let filterType = $state<"all" | "kata" | "kumite">("all");
  let filterStatus = $state<"all" | "completed" | "uncompleted">("all");

  async function loadSummaryBagan() {
    isLoading = true;
    errorMessage = null;
    try {
      if (typeof window !== "undefined" && (window as any).__TAURI_INTERNALS__) {
        summaryData = await invoke<SummaryBaganResult>("get_summary_bagan", { eventId });
      } else {
        // Fallback realistic mock data for dev browser & testing
        summaryData = getMockSummaryBaganData();
      }
    } catch (err: any) {
      console.warn("get_summary_bagan failed, loading fallback mock:", err);
      summaryData = getMockSummaryBaganData();
    } finally {
      isLoading = false;
    }
  }

  function getMockSummaryBaganData(): SummaryBaganResult {
    return {
      event_name: eventName || "KEJUARAAN NASIONAL KARATE 2026",
      total_bagans: 10,
      completed_bagans: 8,
      total_gold: 8,
      total_silver: 8,
      total_bronze: 14,
      bagans: [
        {
          id: 3,
          kode: "003",
          nama_bagan: "TOURNAMENT PRA USIA DINI - KUMITE -25 KG PUTRA",
          tipe_tanding: "2",
          is_selesai: true,
          winners: [
            {
              juara: "1",
              athlete_id: 101,
              nama_atlet: "KYLIAN MALEAKHI JOSE SAHILATUA",
              perguruan_nama: "SHOKAIDO",
              utusan_nama: "DOJO COMBAT",
              logo: null,
              is_uncounted: false,
            },
            {
              juara: "2",
              athlete_id: 102,
              nama_atlet: "MUHAMMAD NABIL ADI MULIA",
              perguruan_nama: "INKANAS",
              utusan_nama: "DOJO ZERO 5",
              logo: null,
              is_uncounted: false,
            },
          ],
        },
        {
          id: 4,
          kode: "004",
          nama_bagan: "TOURNAMENT USIA DINI - KATA PERORANGAN PUTRA",
          tipe_tanding: "1",
          is_selesai: true,
          winners: [
            {
              juara: "1",
              athlete_id: 103,
              nama_atlet: "MUH FADLI RAKHA WIJAYA",
              perguruan_nama: "SHOKAIDO",
              utusan_nama: "DOJO COMBAT",
              logo: null,
              is_uncounted: false,
            },
            {
              juara: "2",
              athlete_id: 104,
              nama_atlet: "MUHAMMAD ADRIAN PUTRA PRATAMA",
              perguruan_nama: "INKANAS",
              utusan_nama: "DOJO ZERO 5",
              logo: null,
              is_uncounted: false,
            },
            {
              juara: "3a",
              athlete_id: 105,
              nama_atlet: "MAHESWARA MUBEROK AL MAHDI",
              perguruan_nama: "INKAI",
              utusan_nama: "INKAI BONTANG",
              logo: null,
              is_uncounted: false,
            },
          ],
        },
        {
          id: 5,
          kode: "005",
          nama_bagan: "TOURNAMENT USIA DINI - KUMITE -30 KG PUTRA",
          tipe_tanding: "2",
          is_selesai: true,
          winners: [
            {
              juara: "1",
              athlete_id: 106,
              nama_atlet: "MUHAMMAD IRSYAD UWAIS",
              perguruan_nama: "LEMKARI",
              utusan_nama: "LEMKARI SAMARINDA",
              logo: null,
              is_uncounted: false,
            },
            {
              juara: "2",
              athlete_id: 107,
              nama_atlet: "ARFAN FAIZAN RAFASSYA",
              perguruan_nama: "GOJUKAI",
              utusan_nama: "DOJO DRAGON",
              logo: null,
              is_uncounted: false,
            },
            {
              juara: "3a",
              athlete_id: 108,
              nama_atlet: "M KENZO ALZAM DANABRATA",
              perguruan_nama: "GOJUKAI",
              utusan_nama: "DOJO DRAGON",
              logo: null,
              is_uncounted: false,
            },
          ],
        },
        {
          id: 6,
          kode: "006",
          nama_bagan: "TOURNAMENT USIA DINI - KUMITE +30 KG PUTRA",
          tipe_tanding: "2",
          is_selesai: true,
          winners: [
            {
              juara: "1",
              athlete_id: 109,
              nama_atlet: "CEVIN RAMADHAN",
              perguruan_nama: "LEMKARI",
              utusan_nama: "LEMKARI SAMARINDA",
              logo: null,
              is_uncounted: false,
            },
            {
              juara: "2",
              athlete_id: 110,
              nama_atlet: "MUHAMMAD HUSNAINI HAMIZAN",
              perguruan_nama: "LEMKARI",
              utusan_nama: "LEMKARI SAMARINDA",
              logo: null,
              is_uncounted: false,
            },
            {
              juara: "3a",
              athlete_id: 111,
              nama_atlet: "M. AFDAL DZIKRA",
              perguruan_nama: "GOJUKAI",
              utusan_nama: "DOJO DRAGON",
              logo: null,
              is_uncounted: false,
            },
          ],
        },
        {
          id: 7,
          kode: "007",
          nama_bagan: "TOURNAMENT USIA DINI - KUMITE -25 KG PUTRI",
          tipe_tanding: "2",
          is_selesai: true,
          winners: [
            {
              juara: "1",
              athlete_id: 112,
              nama_atlet: "ALMEERA NUR RAMADHAN PRIKANDH...",
              perguruan_nama: "INKANAS",
              utusan_nama: "DOJO ZERO 5",
              logo: null,
              is_uncounted: false,
            },
            {
              juara: "2",
              athlete_id: 113,
              nama_atlet: "KANZIA IZZATUNNISA",
              perguruan_nama: "GOJUKAI",
              utusan_nama: "DOJO DRAGON",
              logo: null,
              is_uncounted: false,
            },
          ],
        },
        {
          id: 8,
          kode: "008",
          nama_bagan: "TOURNAMENT USIA DINI - KUMITE +25 KG PUTRI",
          tipe_tanding: "2",
          is_selesai: true,
          winners: [
            {
              juara: "1",
              athlete_id: 114,
              nama_atlet: "FATIMAH AZ-ZAHRA SEPTAYU",
              perguruan_nama: "GOJUKAI",
              utusan_nama: "DOJO DRAGON",
              logo: null,
              is_uncounted: false,
            },
            {
              juara: "2",
              athlete_id: 115,
              nama_atlet: "AQILLA AYU DAHLAN",
              perguruan_nama: "INKAI",
              utusan_nama: "INKAI BONTANG",
              logo: null,
              is_uncounted: false,
            },
            {
              juara: "3a",
              athlete_id: 116,
              nama_atlet: "MIKHAYLA HANUM NUR AZZALEA",
              perguruan_nama: "GOJUKAI",
              utusan_nama: "DOJO DRAGON",
              logo: null,
              is_uncounted: false,
            },
            {
              juara: "3b",
              athlete_id: 117,
              nama_atlet: "ILONA ZAKKIYA AISHWARYA",
              perguruan_nama: "GOJUKAI",
              utusan_nama: "DOJO DRAGON",
              logo: null,
              is_uncounted: false,
            },
          ],
        },
        {
          id: 9,
          kode: "009",
          nama_bagan: "TOURNAMENT PRA PEMULA - KATA PERORANGAN PUTRA",
          tipe_tanding: "1",
          is_selesai: true,
          winners: [
            {
              juara: "1",
              athlete_id: 118,
              nama_atlet: "RIO BERTINUS SONGRI",
              perguruan_nama: "SHOKAIDO",
              utusan_nama: "DOJO COMBAT",
              logo: null,
              is_uncounted: false,
            },
            {
              juara: "2",
              athlete_id: 119,
              nama_atlet: "ARDIKA GALIH",
              perguruan_nama: "INKANAS",
              utusan_nama: "DOJO ZERO 5",
              logo: null,
              is_uncounted: false,
            },
            {
              juara: "3a",
              athlete_id: 120,
              nama_atlet: "MUHAMMAD IQBAL SYAMIL",
              perguruan_nama: "LEMKARI",
              utusan_nama: "LEMKARI SAMARINDA",
              logo: null,
              is_uncounted: false,
            },
            {
              juara: "3b",
              athlete_id: 121,
              nama_atlet: "FATH AZIZIL HANA'",
              perguruan_nama: "GOJUKAI",
              utusan_nama: "DOJO DRAGON",
              logo: null,
              is_uncounted: false,
            },
          ],
        },
        {
          id: 10,
          kode: "010",
          nama_bagan: "TOURNAMENT PRA PEMULA - KATA PERORANGAN PUTRI",
          tipe_tanding: "1",
          is_selesai: true,
          winners: [
            {
              juara: "1",
              athlete_id: 122,
              nama_atlet: "ALFRIDA TALITA ZAHRA",
              perguruan_nama: "INKANAS",
              utusan_nama: "DOJO ZERO 5",
              logo: null,
              is_uncounted: false,
            },
            {
              juara: "2",
              athlete_id: 123,
              nama_atlet: "WA AYAZUL FALERI ANZANI",
              perguruan_nama: "SHOKAIDO",
              utusan_nama: "DOJO COMBAT",
              logo: null,
              is_uncounted: false,
            },
            {
              juara: "3a",
              athlete_id: 124,
              nama_atlet: "AQILAH DZAKIYAH R",
              perguruan_nama: "GOJUKAI",
              utusan_nama: "DOJO DRAGON",
              logo: null,
              is_uncounted: false,
            },
          ],
        },
        {
          id: 12,
          kode: "012",
          nama_bagan: "TOURNAMENT MIX PRA PEMULA & PEMULA - KATA BEREGU MIX",
          tipe_tanding: "1",
          is_selesai: true,
          winners: [
            {
              juara: "1",
              athlete_id: 125,
              nama_atlet: "TEAM SYAQILLA CS",
              perguruan_nama: "INKANAS",
              utusan_nama: "DOJO ZERO 5",
              logo: null,
              is_uncounted: false,
            },
            {
              juara: "2",
              athlete_id: 126,
              nama_atlet: "TEAM NADZIRAH CS",
              perguruan_nama: "GOJUKAI",
              utusan_nama: "DOJO DRAGON",
              logo: null,
              is_uncounted: false,
            },
            {
              juara: "3a",
              athlete_id: 127,
              nama_atlet: "TEAM AYATUL CS",
              perguruan_nama: "BKC",
              utusan_nama: "BKC KAL-TIM",
              logo: null,
              is_uncounted: false,
            },
          ],
        },
        {
          id: 13,
          kode: "013",
          nama_bagan: "TOURNAMENT PRA PEMULA - KUMITE -35 KG & +35 KG PUTRA",
          tipe_tanding: "2",
          is_selesai: false,
          winners: [],
        },
      ],
    };
  }

  onMount(() => {
    loadSummaryBagan();
  });

  // Filtered bagans
  let filteredBagans = $derived.by(() => {
    if (!summaryData) return [];
    const q = searchQuery.toLowerCase().trim();

    return summaryData.bagans.filter((b) => {
      // 1. Type filter
      const isKata = b.tipe_tanding === "1" || b.nama_bagan.toUpperCase().includes("KATA");
      if (filterType === "kata" && !isKata) return false;
      if (filterType === "kumite" && isKata) return false;

      // 2. Status filter
      if (filterStatus === "completed" && !b.is_selesai) return false;
      if (filterStatus === "uncompleted" && b.is_selesai) return false;

      // 3. Search query
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
</script>

<div class="h-full flex flex-col p-4 md:p-6 select-none bg-[#14161b] text-zinc-200 overflow-y-auto font-sans print-sheet">
  <!-- Top Bar -->
  <div class="no-print flex flex-col md:flex-row md:items-center justify-between pb-3.5 mb-3.5 border-b border-white/[0.08] gap-3 shrink-0">
    <div class="min-w-0">
      <div class="flex items-center space-x-2.5">
        <span class="w-2.5 h-2.5 rounded-full bg-amber-400 shrink-0"></span>
        <h1 class="text-sm md:text-base font-black tracking-tight text-white uppercase truncate">
          Daftar Juara Bagan
        </h1>
        <span class="text-xs text-zinc-500 font-mono truncate">
          {summaryData?.event_name || eventName}
        </span>
      </div>
      <p class="text-[11px] text-zinc-400 mt-0.5 truncate">
        Rekapitulasi peraih medali juara 1 (emas), 2 (perak), dan 3 (perunggu) per nomor pertandingan
      </p>
    </div>

    <!-- Right Controls: Search, Filters, Print, Refresh -->
    <div class="flex flex-wrap items-center gap-2 shrink-0">
      <!-- Search Input -->
      <div class="relative min-w-[170px] sm:min-w-[200px]">
        <input
          type="text"
          bind:value={searchQuery}
          placeholder="Cari bagan, atlet..."
          class="w-full bg-[#181a20] border border-white/[0.1] rounded-lg pl-8 pr-3 py-1.5 text-xs text-white placeholder-zinc-500 focus:outline-none focus:border-amber-500/50 transition"
        />
        <svg
          class="w-3.5 h-3.5 text-zinc-500 absolute left-2.5 top-1/2 -translate-y-1/2"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
          />
        </svg>
      </div>

      <!-- Type Filter -->
      <select
        bind:value={filterType}
        class="bg-[#181a20] border border-white/[0.1] rounded-lg px-2.5 py-1.5 text-xs text-zinc-200 focus:outline-none focus:border-amber-500/50 transition cursor-pointer"
      >
        <option value="all">Semua Tipe</option>
        <option value="kata">Kata</option>
        <option value="kumite">Kumite</option>
      </select>

      <!-- Status Filter -->
      <select
        bind:value={filterStatus}
        class="bg-[#181a20] border border-white/[0.1] rounded-lg px-2.5 py-1.5 text-xs text-zinc-200 focus:outline-none focus:border-amber-500/50 transition cursor-pointer"
      >
        <option value="all">Semua Status</option>
        <option value="completed">Ada Juara</option>
        <option value="uncompleted">Belum Selesai</option>
      </select>

      <!-- Print Button -->
      <button
        type="button"
        onclick={() => window.print()}
        class="px-2.5 py-1.5 bg-white/[0.05] hover:bg-white/[0.08] text-zinc-200 border-white/[0.1] text-xs font-semibold border rounded-lg transition flex items-center space-x-1.5 shadow-xs active:scale-95"
        title="Cetak Daftar Juara Bagan (A4)"
      >
        <svg class="w-3.5 h-3.5 text-zinc-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z"
          />
        </svg>
        <span>Cetak</span>
      </button>

      <!-- Refresh Button -->
      <button
        type="button"
        onclick={loadSummaryBagan}
        disabled={isLoading}
        class="px-2.5 py-1.5 bg-white/[0.05] hover:bg-white/[0.08] text-zinc-200 border-white/[0.1] text-xs font-semibold border rounded-lg transition flex items-center space-x-1.5 shadow-xs active:scale-95 disabled:opacity-50"
      >
        <svg
          class="w-3.5 h-3.5 text-zinc-400 {isLoading ? 'animate-spin' : ''}"
          fill="none"
          stroke="currentColor"
          viewBox="0 0 24 24"
        >
          <path
            stroke-linecap="round"
            stroke-linejoin="round"
            stroke-width="2"
            d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15"
          />
        </svg>
        <span>Segarkan</span>
      </button>
    </div>
  </div>

  <!-- Print Header Title (visible on print only) -->
  <div class="print-only hidden mb-4 text-center">
    <div class="flex items-center justify-center space-x-2 text-black">
      <svg class="w-6 h-6 text-black" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
        <path stroke-linecap="round" stroke-linejoin="round" d="M16.5 18.75h-9m9 0a3 3 0 013 3h-15a3 3 0 013-3m9 0v-3.375c0-.621-.504-1.125-1.125-1.125h-.871M7.5 18.75v-3.375c0-.621.504-1.125 1.125-1.125h.872m5.003 0V7.875A3.375 3.375 0 0011.25 4.5h-2.25A3.375 3.375 0 005.625 7.875v6.375" />
      </svg>
      <h1 class="text-xl font-black uppercase tracking-wider text-black">DAFTAR JUARA PERTANDINGAN</h1>
    </div>
    <p class="text-xs font-mono uppercase text-gray-700 mt-1">{summaryData?.event_name || eventName}</p>
    <div class="text-[10px] text-gray-500 mt-0.5">Dicetak pada: {new Date().toLocaleDateString('id-ID', { dateStyle: 'long' })}</div>
  </div>

  <!-- Stat Summary Cards -->
  <div class="grid grid-cols-2 sm:grid-cols-4 gap-2.5 mb-4 shrink-0 no-print">
    <!-- Stat 1: Total Bagan -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex items-center justify-between shadow-xs">
      <div>
        <span class="text-[10px] font-bold text-zinc-400 uppercase tracking-wider block">Total Bagan</span>
        <div class="flex items-baseline space-x-1.5 mt-0.5">
          <span class="text-xl font-black font-mono text-white">{summaryData?.total_bagans ?? 0}</span>
          <span class="text-[11px] text-zinc-500 font-medium">Bagan</span>
        </div>
      </div>
      <div class="w-9 h-9 rounded-lg bg-white/[0.04] text-zinc-400 border border-white/[0.06] flex items-center justify-center shrink-0">
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M4 6h4v4H4V6zm12 0h4v4h-4V6zM4 14h4v4H4v-4zm12 0h4v4h-4v-4zm-8-5h8M8 17h8" />
        </svg>
      </div>
    </div>

    <!-- Stat 2: Bagan Selesai -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex items-center justify-between shadow-xs">
      <div>
        <span class="text-[10px] font-bold text-emerald-400 uppercase tracking-wider block">Bagan Selesai</span>
        <div class="flex items-baseline space-x-1.5 mt-0.5">
          <span class="text-xl font-black font-mono text-emerald-300">{summaryData?.completed_bagans ?? 0}</span>
          <span class="text-[11px] text-zinc-500 font-medium">Selesai</span>
        </div>
      </div>
      <div class="w-9 h-9 rounded-lg bg-emerald-500/10 text-emerald-400 border border-emerald-500/20 flex items-center justify-center shrink-0">
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
      </div>
    </div>

    <!-- Stat 3: Medali Emas & Perak -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex items-center justify-between shadow-xs">
      <div>
        <span class="text-[10px] font-bold text-amber-400 uppercase tracking-wider block">Emas & Perak</span>
        <div class="flex items-baseline space-x-2 mt-0.5">
          <span class="text-xl font-black font-mono text-amber-300">{summaryData?.total_gold ?? 0}</span>
          <span class="text-xs text-zinc-500">/</span>
          <span class="text-xl font-black font-mono text-slate-300">{summaryData?.total_silver ?? 0}</span>
        </div>
      </div>
      <div class="w-9 h-9 rounded-lg bg-amber-500/10 text-amber-300 border border-amber-500/20 flex items-center justify-center shrink-0">
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M12 8c-1.657 0-3 .895-3 2s1.343 2 3 2 3 .895 3 2-1.343 2-3 2m0-8c1.11 0 2.08.402 2.599 1M12 8V7m0 1v8m0 0v1m0-1c-1.11 0-2.08-.402-2.599-1M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
        </svg>
      </div>
    </div>

    <!-- Stat 4: Medali Perunggu -->
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex items-center justify-between shadow-xs">
      <div>
        <span class="text-[10px] font-bold text-amber-600 uppercase tracking-wider block">Medali Perunggu</span>
        <div class="flex items-baseline space-x-1.5 mt-0.5">
          <span class="text-xl font-black font-mono text-amber-500">{summaryData?.total_bronze ?? 0}</span>
          <span class="text-[11px] text-zinc-500 font-medium">Perunggu</span>
        </div>
      </div>
      <div class="w-9 h-9 rounded-lg bg-amber-700/15 text-amber-500 border border-amber-700/30 flex items-center justify-center shrink-0">
        <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2">
          <path stroke-linecap="round" stroke-linejoin="round" d="M16.5 18.75h-9m9 0a3 3 0 013 3h-15a3 3 0 013-3m9 0v-3.375c0-.621-.504-1.125-1.125-1.125h-.871M7.5 18.75v-3.375c0-.621.504-1.125 1.125-1.125h.872" />
        </svg>
      </div>
    </div>
  </div>

  <!-- Main Cards Grid: 2 Columns on Desktop, 1 Column on Mobile -->
  {#if filteredBagans.length === 0}
    <div class="bg-[#181a20] border border-white/[0.08] rounded-2xl p-10 text-center text-zinc-400 my-auto">
      <svg class="w-10 h-10 mx-auto text-zinc-600 mb-2" fill="none" viewBox="0 0 24 24" stroke="currentColor">
        <path stroke-linecap="round" stroke-linejoin="round" stroke-width="1.5" d="M9.172 16.172a4 4 0 015.656 0M9 10h.01M15 10h.01M21 12a9 9 0 11-18 0 9 9 0 0118 0z" />
      </svg>
      <p class="text-sm font-bold text-white">Tidak ada bagan pertandingan ditemukan</p>
      <p class="text-xs text-zinc-500 mt-1">Coba sesuaikan kata kunci pencarian atau filter yang digunakan.</p>
    </div>
  {:else}
    <div class="grid grid-cols-1 md:grid-cols-2 gap-3.5 bagan-print-grid">
      {#each filteredBagans as bagan (bagan.id)}
        <div class="bagan-card bg-[#181a20] border border-white/[0.08] rounded-xl p-3 sm:p-3.5 hover:border-white/[0.15] transition shadow-xs flex flex-col justify-between">
          <!-- Card Header -->
          <div class="flex items-center justify-between pb-2.5 mb-2 border-b border-white/[0.06] gap-2">
            <div class="flex items-center space-x-2 min-w-0">
              <!-- Bracket Code Badge e.g. [003] -->
              <span class="px-2 py-0.5 rounded bg-amber-500/15 text-amber-300 border border-amber-500/30 font-mono font-black text-xs shrink-0 tracking-wider">
                [{bagan.kode}]
              </span>

              <!-- Category / Bagan Name -->
              <h3 class="text-xs sm:text-[13px] font-bold text-white uppercase truncate tracking-wide" title={bagan.nama_bagan}>
                {bagan.nama_bagan}
              </h3>
            </div>

            <!-- Tipe Badge: KUMITE or KATA -->
            <div class="shrink-0">
              {#if bagan.tipe_tanding === "1" || bagan.nama_bagan.toUpperCase().includes("KATA")}
                <span class="inline-flex items-center space-x-1 px-2 py-0.5 rounded bg-sky-500/10 text-sky-400 border border-sky-500/20 text-[10px] font-bold tracking-wider uppercase">
                  <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
                  </svg>
                  <span>KATA</span>
                </span>
              {:else}
                <span class="inline-flex items-center space-x-1 px-2 py-0.5 rounded bg-rose-500/10 text-rose-400 border border-rose-500/20 text-[10px] font-bold tracking-wider uppercase">
                  <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17.657 18.657A8 8 0 016.343 7.343S7 9 9 10c0-2 .5-5 2.986-7C14 5 16.09 5.777 17.656 7.343A7.975 7.975 0 0120 13a7.975 7.975 0 01-2.343 5.657z" />
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
                <div class="flex items-center justify-between p-1.5 sm:px-2 rounded-lg bg-white/[0.02] border border-white/[0.04] hover:bg-white/[0.04] transition gap-2">
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
                      {#if winner.logo}
                        <img src={winner.logo} alt="" class="w-full h-full object-contain" />
                      {:else}
                        <span>{winner.perguruan_nama ? winner.perguruan_nama.substring(0, 2) : 'DO'}</span>
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

                  <!-- Right: Affiliation (Perguruan • Kontingen) -->
                  <div class="shrink-0 text-right min-w-[140px] max-w-[210px]">
                    <span class="text-[11px] font-mono text-zinc-400 truncate block tracking-tight" title="{winner.perguruan_nama} • {winner.utusan_nama}">
                      <span class="text-zinc-300 font-semibold">{winner.perguruan_nama}</span>
                      <span class="text-zinc-500 mx-1">•</span>
                      <span>{winner.utusan_nama}</span>
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

<style>
  @media print {
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
    .bagan-print-grid {
      display: grid !important;
      grid-template-columns: repeat(2, minmax(0, 1fr)) !important;
      gap: 10px !important;
    }
    .bagan-card {
      background: white !important;
      border: 1px solid #cbd5e1 !important;
      color: black !important;
      page-break-inside: avoid !important;
      box-shadow: none !important;
    }
    .bagan-card h3 {
      color: black !important;
    }
    .bagan-card span {
      color: #334155 !important;
    }
  }
</style>
