<script lang="ts">
  import { apiCall, getConnectionConfig } from "$lib/api_client";
  import type { ResultsBookletData, ResultsBookletSignatory, TatamiBookletCategory, DetailBaganItem } from "$lib/types";
  import { printWithOrientation } from "$lib/print_helper";
  import { resolveMediaLogos } from "$lib/media_helper";
  import DaftarJuaraPrintSheet from "$lib/components/DaftarJuaraPrintSheet.svelte";
  import KlasemenMedaliPrintSheet from "$lib/components/KlasemenMedaliPrintSheet.svelte";

  interface Props {
    eventId: number;
    eventName?: string;
    hostAddress?: string;
  }

  let { eventId, eventName = "", hostAddress = "" }: Props = $props();

  let bookletData = $state<ResultsBookletData | null>(null);
  let isLoading = $state(true);
  let errorMessage = $state<string | null>(null);
  let isEditing = $state(false);
  let logoMap = $state<Record<string, string>>({});

  // Print Mode & In-App View Filter state
  let printMode = $state<"portrait" | "landscape">("portrait");
  let viewFilter = $state<"all" | "portrait" | "landscape">("all");
  let bookletPruneEmptyRounds = $state(true);

  // Editable local overrides for sign-off & header
  let editableOrganisasi = $state("FEDERASI OLAHRAGA KARATE-DO INDONESIA KALIMANTAN TIMUR");
  let editableEventName = $state("");
  let editableTempat = $state("");
  let editableTanggal = $state("");
  let editableKota = $state("");
  let editableNomorSurat = $state("");
  let editableCatatanSanksi = $state("");
  let editableSignatories = $state<ResultsBookletSignatory[]>([]);

  $effect(() => {
    if (eventId) {
      loadData();
    }
  });

  async function loadData() {
    if (!eventId) return;
    isLoading = true;
    errorMessage = null;
    try {
      const res = await apiCall<ResultsBookletData>("get_results_booklet_data", {
        eventId,
        event_id: eventId,
      });
      bookletData = res;
      editableOrganisasi = "FEDERASI OLAHRAGA KARATE-DO INDONESIA KALIMANTAN TIMUR";
      editableEventName = res.event_info.nama_event;
      editableTempat = res.event_info.tempat;
      editableTanggal = res.event_info.tanggal;
      editableKota = res.event_info.kota;
      editableNomorSurat = res.berita_acara.nomor_surat;
      editableCatatanSanksi = res.berita_acara.catatan_sanksi;
      editableSignatories = JSON.parse(JSON.stringify(res.signatories));

      // Resolve event logo, contingent logos, perguruan logos, category winners logos, and bracket match logos
      const allLogos: string[] = [];
      if (res.event_info?.logo) {
        allLogos.push(res.event_info.logo);
      }
      if (res.standings_kontingen) {
        for (const k of res.standings_kontingen) {
          if (k.logo) allLogos.push(k.logo);
        }
      }
      if (res.standings_perguruan) {
        for (const p of res.standings_perguruan) {
          if (p.logo) allLogos.push(p.logo);
        }
      }
      if (res.category_winners) {
        for (const cat of res.category_winners) {
          for (const w of cat.winners) {
            if (w.logo) allLogos.push(w.logo);
          }
        }
      }
      if (res.category_brackets) {
        for (const b of res.category_brackets) {
          for (const m of b.matches) {
            if (m.atlet1_logo) allLogos.push(m.atlet1_logo);
            if (m.atlet2_logo) allLogos.push(m.atlet2_logo);
          }
        }
      }

      if (allLogos.length > 0) {
        const cleanHost = hostAddress || getConnectionConfig().hostUrl;
        resolveMediaLogos(allLogos, cleanHost).then((resolved) => {
          logoMap = resolved;
        });
      }
    } catch (err: any) {
      console.error("Gagal memuat data buku hasil:", err);
      errorMessage = err?.message || String(err);
    } finally {
      isLoading = false;
    }
  }

  function printPortrait() {
    printMode = "portrait";
    setTimeout(() => {
      printWithOrientation("portrait", "8mm");
    }, 80);
  }

  function printLandscape() {
    printMode = "landscape";
    setTimeout(() => {
      printWithOrientation("landscape", "6mm");
    }, 80);
  }

  function scrollToSection(id: string) {
    const el = document.getElementById(id);
    if (el) {
      el.scrollIntoView({ behavior: "smooth" });
    }
  }

  // Bracket Helpers for Landscape Attachment
  function groupMatchesByRound(matches: DetailBaganItem[], pruneEmpty: boolean = true): Map<number, DetailBaganItem[]> {
    const map = new Map<number, DetailBaganItem[]>();
    const validMatches = matches.filter((m) => !m.round || m.round <= 4);

    let startRound = 1;
    if (pruneEmpty) {
      let minActiveRound: number | null = null;
      for (const m of validMatches) {
        if (m.atlet1_nama || m.atlet2_nama) {
          const r = m.round || 1;
          if (minActiveRound === null || r < minActiveRound) {
            minActiveRound = r;
          }
        }
      }
      if (minActiveRound !== null) {
        startRound = minActiveRound;
      }
    }

    validMatches.forEach((m) => {
      const r = m.round || 1;
      if (r >= startRound) {
        if (!map.has(r)) map.set(r, []);
        map.get(r)!.push(m);
      }
    });
    for (const [_, list] of map) {
      list.sort((a, b) => (a.urutan || a.id) - (b.urutan || b.id));
    }
    return map;
  }

  function getRoundTitleText(colIdx: number, totalCols: number): string {
    const stepFromFinal = totalCols - 1 - colIdx;
    if (stepFromFinal === 0) return "PARTAI FINAL";
    if (stepFromFinal === 1) return "SEMI FINAL";
    if (stepFromFinal === 2) return "PEREMPAT FINAL";
    if (stepFromFinal === 3) return "BABAK 16 BESAR";
    return `BABAK ${colIdx + 1}`;
  }

  function formatMatchTitle(colIdx: number, totalCols: number, match: DetailBaganItem, idx: number): string {
    const stepFromFinal = totalCols - 1 - colIdx;
    const num = match.urutan || (idx + 1);
    if (stepFromFinal === 0) return "PARTAI FINAL";
    if (stepFromFinal === 1) return `SEMI ${num}`;
    return `MATCH ${num}`;
  }

  // Filtered brackets that actually have matches
  let activeBrackets = $derived.by(() => {
    if (!bookletData?.category_brackets) return [];
    return bookletData.category_brackets.filter((b) => b.matches && b.matches.length > 0);
  });
</script>

<svelte:head>
  <style>
    @page {
      size: A4 portrait;
      margin: 8mm;
    }
  </style>
</svelte:head>

<div class="h-full flex flex-col bg-[#0b0d13] text-zinc-100 font-sans overflow-hidden select-none print-view-root {printMode === 'portrait' ? 'is-printing-portrait' : 'is-printing-landscape'}">
  <!-- ================= IN-APP STICKY HEADER TOOLBAR ================= -->
  <header class="no-print bg-[#12151d] border-b border-white/[0.08] px-4 py-2.5 flex flex-wrap items-center justify-between gap-3 shrink-0 shadow-lg z-20">
    <!-- Left: Identity & Title -->
    <div class="flex items-center space-x-3 min-w-0">
      <div class="w-8 h-8 rounded-lg bg-amber-500/20 border border-amber-500/40 text-amber-400 font-black font-mono text-xs flex items-center justify-center shrink-0">
        BA
      </div>
      <div class="min-w-0">
        <div class="flex items-center space-x-2">
          <span class="text-xs font-black text-white tracking-tight uppercase truncate">
            Buku Hasil & Berita Acara Resmi
          </span>
          <span class="text-[10px] font-mono px-1.5 py-0.2 rounded bg-amber-500/10 text-amber-300 border border-amber-500/20">
            OFFICIAL
          </span>
        </div>
        <p class="text-[11px] text-zinc-400 truncate">
          {editableEventName || 'Memuat event...'} • {editableKota}
        </p>
      </div>
    </div>

    <!-- Center: View Filter Tabs -->
    <div class="flex items-center bg-[#181a22] p-1 rounded-xl border border-white/[0.08] text-xs">
      <button
        type="button"
        onclick={() => (viewFilter = "all")}
        class="px-3 py-1 rounded-lg font-bold transition cursor-pointer {viewFilter === 'all' ? 'bg-white text-slate-950 shadow-sm' : 'text-zinc-400 hover:text-white hover:bg-white/[0.06]'}"
      >
        Semua Lembar
      </button>
      <button
        type="button"
        onclick={() => (viewFilter = "portrait")}
        class="px-3 py-1 rounded-lg font-bold transition flex items-center space-x-1.5 cursor-pointer {viewFilter === 'portrait' ? 'bg-amber-500 text-slate-950 shadow-sm' : 'text-zinc-400 hover:text-amber-300 hover:bg-white/[0.06]'}"
        title="Tampilkan hanya lembar dokumen berita acara resmi (A4 Portrait)"
      >
        <span>📄</span>
        <span>Dokumen Hasil (Portrait)</span>
      </button>
      <button
        type="button"
        onclick={() => (viewFilter = "landscape")}
        class="px-3 py-1 rounded-lg font-bold transition flex items-center space-x-1.5 cursor-pointer {viewFilter === 'landscape' ? 'bg-sky-500 text-white shadow-sm' : 'text-zinc-400 hover:text-sky-300 hover:bg-white/[0.06]'}"
        title="Tampilkan hanya lembar lampiran bagan pertandingan (A4 Landscape)"
      >
        <span>🌿</span>
        <span>Lampiran Bagan (Landscape)</span>
      </button>
    </div>

    <!-- Right: Quick Navigation & Action Buttons -->
    <div class="flex items-center space-x-2 shrink-0">
      <!-- Edit Mode Toggle -->
      <button
        type="button"
        onclick={() => (isEditing = !isEditing)}
        class="px-3 py-1.5 rounded-xl text-xs font-semibold border transition flex items-center space-x-1.5 cursor-pointer {isEditing ? 'bg-amber-500/20 border-amber-500 text-amber-300 shadow-sm' : 'bg-white/[0.05] border-white/[0.1] text-zinc-300 hover:bg-white/[0.08]'}"
        title="Buka form untuk menyunting pengesahan, kota, dan nomor surat"
      >
        <span>✏️</span>
        <span>{isEditing ? 'Selesai Edit' : 'Edit Pengesahan'}</span>
      </button>

      <!-- PRINT ACTION 1: A4 PORTRAIT (Official Results Document) -->
      <button
        type="button"
        onclick={printPortrait}
        class="px-3.5 py-1.5 rounded-xl bg-amber-500 hover:bg-amber-400 text-slate-950 text-xs font-black tracking-wide shadow-md transition flex items-center space-x-1.5 active:scale-95 cursor-pointer"
        title="Cetak Sampul, Berita Acara, Klasemen, Juara, Wasit & Penutup (Halaman 1–7) dalam format A4 Portrait"
      >
        <svg class="w-3.5 h-3.5 text-slate-950" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
        </svg>
        <span>📄 Cetak Buku Hasil (Portrait)</span>
      </button>

      <!-- PRINT ACTION 2: A4 LANDSCAPE (Tournament Brackets Attachment) -->
      <button
        type="button"
        onclick={printLandscape}
        class="px-3.5 py-1.5 rounded-xl bg-sky-500 hover:bg-sky-400 text-white text-xs font-black tracking-wide shadow-md transition flex items-center space-x-1.5 active:scale-95 cursor-pointer"
        title="Cetak Seluruh Lampiran Bagan Pertandingan Pohon dalam format A4 Landscape penuh"
      >
        <svg class="w-3.5 h-3.5 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 17V7m0 10a2 2 0 01-2 2H5a2 2 0 01-2-2V7a2 2 0 012-2h2a2 2 0 012 2m0 10a2 2 0 002 2h2a2 2 0 002-2M9 7a2 2 0 012-2h2a2 2 0 012 2m0 10V7m0 10a2 2 0 002 2h2a2 2 0 002-2V7a2 2 0 00-2-2h-2a2 2 0 00-2 2" />
        </svg>
        <span>🌿 Cetak Lampiran Bagan (Landscape)</span>
      </button>
    </div>
  </header>

  <!-- ================= IN-APP DOCUMENT WORKSPACE CANVAS ================= -->
  <div class="flex-1 overflow-y-auto p-4 sm:p-8 bg-[#0b0d13] print:bg-white print:p-0 select-text">
    {#if isLoading}
      <div class="min-h-[500px] flex flex-col items-center justify-center text-zinc-400">
        <svg class="w-10 h-10 text-amber-500 animate-spin mb-3" fill="none" viewBox="0 0 24 24">
          <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
          <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
        </svg>
        <p class="text-xs font-mono tracking-wider uppercase text-zinc-300">Memuat Dokumen Buku Hasil & Berita Acara...</p>
      </div>
    {:else if errorMessage}
      <div class="max-w-md mx-auto my-12 bg-red-950/40 border border-red-500/30 p-6 rounded-2xl text-center">
        <h3 class="text-sm font-bold text-red-300 mb-2">Gagal Memuat Data Buku Hasil</h3>
        <p class="text-xs text-red-200/80 mb-4">{errorMessage}</p>
        <button
          type="button"
          onclick={loadData}
          class="px-4 py-2 bg-red-600 hover:bg-red-500 text-white text-xs font-bold rounded-xl transition"
        >
          Coba Lagi
        </button>
      </div>
    {:else if bookletData}
      <main class="booklet-workspace-container mx-auto">

        <!-- ========================================================================= -->
        <!-- SEKSI 1: DOKUMEN RESMI BERITA ACARA (A4 PORTRAIT SHEET SIMULATION) -->
        <!-- ========================================================================= -->
        {#if viewFilter === "all" || viewFilter === "portrait"}
          <div class="booklet-section-portrait">

            <!-- ================= HALAMAN 1: SAMPUL RESMI (A4 PORTRAIT) ================= -->
            <section id="page-cover" class="booklet-page booklet-page-portrait">
              <div class="paper-sheet paper-sheet-portrait relative">
                <!-- Outer Official Frame Border -->
                <div class="h-full border-4 border-slate-900 rounded-sm p-6 flex flex-col justify-between relative bg-white overflow-hidden">
                  <!-- Decorative Inner Border -->
                  <div class="absolute inset-2 border border-slate-300 pointer-events-none"></div>

                  <!-- Header Organisasi Resmi -->
                  <div class="text-center pt-2 relative z-10">
                    <div class="inline-block px-3 py-1 rounded bg-slate-900 text-amber-400 font-mono text-[10px] font-bold uppercase tracking-widest mb-3">
                      DOKUMEN RESMI PENGURUS PROVINSI FORKI
                    </div>
                    <div class="text-sm sm:text-base font-black text-slate-950 tracking-wider uppercase leading-snug drop-shadow-sm">
                      {#if isEditing}
                        <input
                          type="text"
                          bind:value={editableOrganisasi}
                          class="bg-slate-100 text-slate-900 text-center px-3 py-1 rounded border border-amber-500 text-xs w-full max-w-lg font-bold"
                        />
                      {:else}
                        <span>{editableOrganisasi}</span>
                      {/if}
                    </div>
                  </div>

                  <!-- Cluster Logo Header (FORKI, Kejuaraan, & WKF) -->
                  <div class="flex items-center justify-center space-x-6 py-6 border-y-2 border-slate-900 my-4 bg-slate-50/60 rounded-md">
                    <img
                      src="/img/forki-logo.png"
                      alt="Logo FORKI"
                      class="h-16 sm:h-20 w-auto object-contain drop-shadow"
                    />
                    <div class="h-14 w-[1px] bg-slate-300"></div>
                    <div class="h-16 sm:h-20 flex items-center justify-center max-w-[140px]">
                      {#if bookletData.event_info.logo && logoMap[bookletData.event_info.logo]}
                        <img
                          src={logoMap[bookletData.event_info.logo]}
                          alt="Logo Kejuaraan"
                          class="max-h-full max-w-full object-contain drop-shadow"
                          onerror={(e) => { (e.currentTarget as HTMLImageElement).src = '/img/forki-logo.png'; }}
                        />
                      {:else}
                        <img
                          src="/img/forki-logo.png"
                          alt="Logo FORKI"
                          class="max-h-full max-w-full object-contain drop-shadow"
                        />
                      {/if}
                    </div>
                    <div class="h-14 w-[1px] bg-slate-300"></div>
                    <img
                      src="/img/cover/wkf_logo.png"
                      alt="Logo WKF"
                      class="h-12 sm:h-14 w-auto object-contain drop-shadow opacity-90"
                      onerror={(e) => { (e.currentTarget as HTMLImageElement).style.display = 'none'; }}
                    />
                  </div>

                  <!-- Central Hero Title Card -->
                  <div class="text-center my-auto px-4 py-8 bg-slate-950 text-white rounded-lg border-2 border-amber-500 shadow-xl relative">
                    <div class="inline-block px-3 py-0.5 rounded bg-amber-500 text-slate-950 font-black text-xs uppercase tracking-widest mb-3">
                      LAPORAN HASIL PERTANDINGAN & BERITA ACARA
                    </div>
                    <h1 class="text-2xl sm:text-3xl font-black text-white tracking-tight uppercase leading-tight mb-4 drop-shadow">
                      {editableEventName}
                    </h1>
                    <div class="h-[1px] w-24 bg-amber-500 mx-auto mb-4"></div>
                    <div class="space-y-1 text-xs sm:text-sm font-semibold text-zinc-300">
                      <div>📍 {editableTempat}</div>
                      <div>📅 {editableTanggal}</div>
                      <div class="text-amber-400 font-bold uppercase tracking-wider text-[11px] mt-1">
                        KOTA {editableKota} • PROVINSI KALIMANTAN TIMUR
                      </div>
                    </div>
                  </div>

                  <!-- Bottom Supported By Bar (No conflict, clean solid patch) -->
                  <div class="pt-6 border-t-2 border-slate-900 mt-4 flex items-center justify-between text-slate-800 text-xs">
                    <div class="space-y-0.5">
                      <div class="font-black text-[11px] uppercase tracking-wider text-slate-900">
                        AMBRILINDO TOURNAMENT ENGINE
                      </div>
                      <div class="text-[9px] text-slate-500 font-mono">
                        Validasi & Digitalisasi PB / PENGPROV FORKI
                      </div>
                    </div>

                    <div class="flex items-center space-x-2 bg-slate-100 px-3 py-1.5 rounded-lg border border-slate-300">
                      <span class="text-[10px] font-black uppercase tracking-wider text-slate-700">SUPPORTED BY:</span>
                      <img src="/img/ambrilindo-logo.png" alt="Ambrilindo" class="h-6 object-contain" />
                    </div>
                  </div>
                </div>
              </div>
            </section>

            <!-- ================= HALAMAN 2: BERITA ACARA & PENGESAHAN ================= -->
            <section id="page-berita-acara" class="booklet-page booklet-page-portrait">
              <div class="paper-sheet paper-sheet-portrait">
                <!-- Kop Surat Header -->
                <div class="border-b-2 border-slate-900 pb-3 mb-5 flex items-center justify-between gap-4">
                  <div class="flex items-center space-x-4">
                    <div class="flex items-center space-x-2.5 shrink-0 bg-white p-0.5 rounded">
                      <img
                        src="/img/forki-logo.png"
                        alt="Logo FORKI"
                        class="w-14 h-14 object-contain shrink-0"
                      />
                      <div class="h-10 w-[1px] bg-slate-300"></div>
                      <img
                        src="/img/ambrilindo-logo.png"
                        alt="Logo Ambrilindo"
                        class="h-8 w-auto max-w-[110px] object-contain shrink-0"
                      />
                    </div>
                    <div>
                      <h2 class="text-sm sm:text-base font-black tracking-tight text-slate-950 uppercase leading-tight">
                        {editableOrganisasi}
                      </h2>
                      <div class="text-[11px] text-slate-600 font-medium leading-normal">
                        {#if bookletData.event_info.alamat}
                          <div>{bookletData.event_info.alamat}</div>
                        {/if}
                        {#if bookletData.event_info.kontak}
                          <div>Kontak / Sekretariat: {bookletData.event_info.kontak}</div>
                        {/if}
                      </div>
                    </div>
                  </div>
                  <div class="text-right shrink-0">
                    <span class="inline-block px-2 py-0.5 rounded text-[10px] font-black uppercase tracking-wider bg-slate-900 text-white">
                      DOKUMEN RESMI FORKI
                    </span>
                  </div>
                </div>

                <!-- Berita Acara Title -->
                <div class="text-center mb-5">
                  <h3 class="text-base font-black text-slate-950 tracking-wider uppercase underline underline-offset-4">
                    BERITA ACARA PENYELENGGARAAN & PENGESAHAN HASIL AKHIR
                  </h3>
                  <div class="text-xs font-mono font-bold text-slate-700 mt-1">
                    {#if isEditing}
                      <input
                        type="text"
                        bind:value={editableNomorSurat}
                        class="border border-slate-400 px-2 py-0.5 rounded text-xs text-center w-64"
                      />
                    {:else}
                      <span>Nomor: {editableNomorSurat}</span>
                    {/if}
                  </div>
                </div>

                <!-- Preamble Statement -->
                <div class="text-xs text-slate-800 leading-relaxed text-justify mb-5 space-y-2">
                  <p>
                    Pada hari ini, <strong>{bookletData.berita_acara.hari_tanggal}</strong>, bertempat di <strong>{editableTempat}</strong>, telah diselesaikan seluruh rangkaian pertandingan kejuaraan karate resmi dengan nama kegiatan:
                  </p>
                  <p class="text-sm font-black text-center uppercase tracking-wide text-slate-950 py-1 bg-slate-100 rounded border border-slate-300">
                    {editableEventName}
                  </p>
                  <p>
                    Penyelenggaraan telah berjalan sesuai dengan Ketentuan Pertandingan World Karate Federation (WKF) serta Pedoman Pengurus Besar Federasi Olahraga Karate-Do Indonesia (PB FORKI). Berdasarkan data rekapitulasi digital Ambrilindo Tournament Engine, dilaporkan ringkasan teknis sebagai berikut:
                  </p>
                </div>

                <!-- Technical Summary Table -->
                <div class="mb-5">
                  <table class="w-full text-xs text-left border border-slate-300">
                    <thead class="bg-slate-900 text-white text-[11px] uppercase font-bold">
                      <tr>
                        <th class="px-3 py-1.5 border-r border-slate-700">Parameter Teknis Pelaksanaan</th>
                        <th class="px-3 py-1.5 text-center w-28 border-r border-slate-700">Realisasi</th>
                        <th class="px-3 py-1.5">Keterangan</th>
                      </tr>
                    </thead>
                    <tbody class="divide-y divide-slate-200">
                      <tr>
                        <td class="px-3 py-1.5 font-bold text-slate-800 border-r border-slate-200">Kontingen / Utusan Daerah Terdaftar</td>
                        <td class="px-3 py-1.5 font-mono font-bold text-center border-r border-slate-200">{bookletData.berita_acara.total_kontingen}</td>
                        <td class="px-3 py-1.5 text-slate-700">Utusan Daerah & Kontingen Resmi</td>
                      </tr>
                      <tr>
                        <td class="px-3 py-1.5 font-bold text-slate-800 border-r border-slate-200">Perguruan Karate Terdaftar</td>
                        <td class="px-3 py-1.5 font-mono font-bold text-center border-r border-slate-200">{bookletData.berita_acara.total_perguruan}</td>
                        <td class="px-3 py-1.5 text-slate-700">Anggota resmi FORKI</td>
                      </tr>
                      <tr>
                        <td class="px-3 py-1.5 font-bold text-slate-800 border-r border-slate-200">Total Atlet Bertanding</td>
                        <td class="px-3 py-1.5 font-mono font-bold text-center border-r border-slate-200">{bookletData.berita_acara.total_atlet}</td>
                        <td class="px-3 py-1.5 text-slate-700">Lolos verifikasi administrasi & timbang badan</td>
                      </tr>
                      <tr>
                        <td class="px-3 py-1.5 font-bold text-slate-800 border-r border-slate-200">Partai Pertandingan (Bagan & Pool)</td>
                        <td class="px-3 py-1.5 font-mono font-bold text-center border-r border-slate-200">{bookletData.berita_acara.partai_selesai} / {bookletData.berita_acara.total_partai}</td>
                        <td class="px-3 py-1.5 font-bold text-emerald-800">{bookletData.berita_acara.status_pelaksanaan}</td>
                      </tr>
                      <tr>
                        <td class="px-3 py-1.5 font-bold text-slate-800 border-r border-slate-200">Catatan Khusus / Insiden / Protes</td>
                        <td colspan="2" class="px-3 py-1.5 text-slate-700">
                          {#if isEditing}
                            <textarea
                              bind:value={editableCatatanSanksi}
                              rows="2"
                              class="w-full border border-slate-400 p-1 rounded text-xs"
                            ></textarea>
                          {:else}
                            <span>{editableCatatanSanksi}</span>
                          {/if}
                        </td>
                      </tr>
                    </tbody>
                  </table>
                </div>

                <!-- Official Ratification Clause -->
                <div class="text-xs text-slate-800 leading-relaxed text-justify mb-5">
                  <p>
                    Dengan ini menyatakan bahwa seluruh perolehan medali, penetapan Juara Bagan/Kategori, dan penetapan Juara Umum Kontingen maupun Perguruan yang tercantum dalam Buku Hasil ini adalah <strong>BENAR, SAH, dan MENGIKAT</strong> sebagai keputusan mutlak yang disahkan bersama oleh Dewan Wasit dan Panitia Pelaksana.
                  </p>
                </div>

                <!-- Date & City Line -->
                <div class="text-right text-xs font-bold text-slate-800 mb-3">
                  {#if isEditing}
                    <div class="flex items-center justify-end space-x-2">
                      <input
                        type="text"
                        bind:value={editableKota}
                        class="border border-slate-400 px-2 py-0.5 rounded text-xs w-32"
                        placeholder="Kota"
                      />
                      <span>,</span>
                      <input
                        type="text"
                        bind:value={bookletData.berita_acara.tanggal_pengesahan}
                        class="border border-slate-400 px-2 py-0.5 rounded text-xs w-40"
                        placeholder="Tanggal Pengesahan"
                      />
                    </div>
                  {:else}
                    <div>{editableKota}, {bookletData.berita_acara.tanggal_pengesahan}</div>
                  {/if}
                </div>

                <!-- 4-Box Signatory Grid -->
                <div class="grid grid-cols-2 gap-4 pt-1">
                  {#each editableSignatories as sig}
                    <div class="border border-slate-300 rounded p-3 text-center flex flex-col justify-between min-h-[140px] bg-slate-50/50">
                      <div>
                        <div class="text-[11px] font-black uppercase text-slate-900 tracking-tight">
                          {sig.jabatan}
                        </div>
                        <div class="text-[10px] text-slate-600 font-medium">
                          {sig.peran}
                        </div>
                      </div>

                      <!-- Signature Area with Seal Ring -->
                      <div class="relative py-2 flex items-center justify-center">
                        <div class="w-14 h-14 rounded-full border border-dashed border-slate-300 text-slate-300 text-[8px] font-bold uppercase flex items-center justify-center select-none opacity-40">
                          STEMPEL
                        </div>
                      </div>

                      <div>
                        {#if isEditing}
                          <input
                            type="text"
                            bind:value={sig.nama}
                            class="border border-slate-400 px-2 py-0.5 rounded text-xs text-center font-bold w-full mb-1"
                            placeholder="Nama Lengkap & Gelar"
                          />
                          <input
                            type="text"
                            bind:value={sig.nomor_lisensi}
                            class="border border-slate-400 px-2 py-0.5 rounded text-[10px] text-center w-full"
                            placeholder="No. Lisensi / NIK"
                          />
                        {:else}
                          <div class="text-xs font-black text-slate-950 underline underline-offset-2">
                            {sig.nama}
                          </div>
                          {#if sig.nomor_lisensi}
                            <div class="text-[10px] font-mono text-slate-600">
                              {sig.nomor_lisensi}
                            </div>
                          {/if}
                        {/if}
                      </div>
                    </div>
                  {/each}
                </div>

                <!-- Footer Meta -->
                <div class="mt-6 pt-3 border-t border-slate-200 flex items-center justify-between text-[9px] text-slate-500 font-mono">
                  <span>Official Match Result Book • Ambrilindo Tournament Engine</span>
                  <span>Halaman 2 • Lembar Pengesahan Resmi</span>
                </div>
              </div>
            </section>

            <!-- ================= HALAMAN 3-4: KLASEMEN MEDALI (KONTINGEN & PERGURUAN) ================= -->
            <section id="page-klasemen" class="booklet-page booklet-page-portrait">
              <div class="paper-sheet paper-sheet-portrait">
                <KlasemenMedaliPrintSheet
                  standingsKontingen={bookletData.standings_kontingen}
                  standingsPerguruan={bookletData.standings_perguruan}
                  totalGold={bookletData.total_gold}
                  totalSilver={bookletData.total_silver}
                  totalBronze={bookletData.total_bronze}
                  grandTotal={bookletData.grand_total}
                  eventName={editableEventName}
                  subTitle="{editableTempat} • {editableTanggal}"
                  pageLabelKontingen="Halaman 3 • Rekapitulasi Medali Kontingen"
                  pageLabelPerguruan="Halaman 4 • Rekapitulasi Medali Perguruan"
                  {logoMap}
                />
              </div>
            </section>

            <!-- ================= HALAMAN 5+: DAFTAR JUARA SELURUH KATEGORI ================= -->
            <section id="page-daftar-juara" class="booklet-page booklet-page-portrait">
              <div class="paper-sheet paper-sheet-portrait">
                <DaftarJuaraPrintSheet
                  bagans={bookletData.category_winners}
                  eventName={editableEventName}
                  pageLabel="Halaman 5 • Rekapan Podium Seluruh Kategori"
                  {logoMap}
                />
              </div>
            </section>

            <!-- ================= HALAMAN 6: DEWAN WASIT & JURI ================= -->
            <section id="page-wasit" class="booklet-page booklet-page-portrait">
              <div class="paper-sheet paper-sheet-portrait">
                <div class="border-b-2 border-slate-900 pb-2 mb-4 flex items-center justify-between">
                  <div class="flex items-center space-x-3.5 min-w-0">
                    <div class="flex items-center space-x-2 shrink-0 bg-white p-0.5 rounded">
                      <img
                        src="/img/forki-logo.png"
                        alt="Logo FORKI"
                        class="h-10 w-auto max-w-[40px] object-contain shrink-0"
                      />
                      <div class="h-7 w-[1px] bg-slate-300 shrink-0"></div>
                      <img
                        src="/img/ambrilindo-logo.png"
                        alt="Logo Ambrilindo"
                        class="h-6 w-auto max-w-[100px] object-contain shrink-0"
                      />
                    </div>
                    <div>
                      <h3 class="text-sm font-black text-slate-950 uppercase tracking-tight">
                        DAFTAR DEWAN WASIT & JURI YANG BERTUGAS
                      </h3>
                      <p class="text-[11px] text-slate-600 font-medium">
                        Korps Perwasitan Kejuaraan Karate • FORKI
                      </p>
                    </div>
                  </div>
                  <span class="px-2 py-0.5 rounded text-[10px] font-black uppercase bg-slate-900 text-white">
                    DEWAN WASIT
                  </span>
                </div>

                {#if bookletData.referees.length === 0}
                  <div class="p-8 text-center text-slate-400 italic">
                    Belum ada dewan wasit yang terdaftar dalam sistem.
                  </div>
                {:else}
                  <table class="w-full text-xs text-left border border-slate-300">
                    <thead class="bg-slate-900 text-white text-[10px] uppercase font-black">
                      <tr>
                        <th class="px-2.5 py-1.5 text-center w-10 border-r border-slate-700">No</th>
                        <th class="px-3 py-1.5 border-r border-slate-700">Nama Lengkap</th>
                        <th class="px-3 py-1.5 border-r border-slate-700">Lisensi / Tingkatan</th>
                        <th class="px-3 py-1.5 border-r border-slate-700">Perguruan</th>
                        <th class="px-3 py-1.5 border-r border-slate-700">Asal Daerah</th>
                        <th class="px-3 py-1.5 text-center">Penugasan</th>
                      </tr>
                    </thead>
                    <tbody class="divide-y divide-slate-200">
                      {#each bookletData.referees as ref, idx}
                        <tr class="hover:bg-slate-50">
                          <td class="px-2.5 py-1 text-center font-mono border-r border-slate-200">{idx + 1}</td>
                          <td class="px-3 py-1 font-bold text-slate-900 border-r border-slate-200">{ref.nama_wasit}</td>
                          <td class="px-3 py-1 text-slate-700 border-r border-slate-200 font-mono text-[11px]">
                            {ref.lisensi || "-"} {ref.no_lisensi ? `(${ref.no_lisensi})` : ""}
                          </td>
                          <td class="px-3 py-1 text-slate-700 border-r border-slate-200">{ref.nama_perguruan || "-"}</td>
                          <td class="px-3 py-1 text-slate-700 border-r border-slate-200">{ref.kab_kota || "-"}</td>
                          <td class="px-3 py-1 text-center font-semibold text-emerald-800">
                            {ref.assigned_tatami_number ? `Tatami ${ref.assigned_tatami_number} (${ref.assigned_posisi || "Juri"})` : "Aktif Bertugas"}
                          </td>
                        </tr>
                      {/each}
                    </tbody>
                  </table>
                {/if}

                <div class="mt-8 pt-3 border-t border-slate-200 flex items-center justify-between text-[9px] text-slate-500 font-mono">
                  <span>Official Match Result Book • Ambrilindo Tournament Engine</span>
                  <span>Halaman 6 • Susunan Korps Wasit / Juri</span>
                </div>
              </div>
            </section>

            <!-- ================= HALAMAN 7: LEMBAR PENUTUP RESMI ================= -->
            <section id="page-closing" class="booklet-page booklet-page-portrait">
              <div class="paper-sheet paper-sheet-portrait">
                <div class="h-full border-4 border-slate-900 rounded-sm p-6 flex flex-col justify-between relative bg-white overflow-hidden">
                  <div class="absolute inset-2 border border-slate-300 pointer-events-none"></div>

                  <!-- Header Penutup -->
                  <div class="flex items-center justify-between border-b-2 border-slate-900 pb-4 relative z-10">
                    <div class="flex items-center space-x-3">
                      <img src="/img/forki-logo.png" alt="Logo FORKI" class="h-10 w-auto object-contain" />
                      <div class="h-6 w-[1px] bg-slate-300"></div>
                      <img src="/img/ambrilindo-logo.png" alt="Ambrilindo" class="h-6 object-contain" />
                    </div>
                    <span class="text-xs font-mono font-bold tracking-widest text-slate-900 uppercase">
                      LEMBAR PENUTUP DOKUMEN RESMI
                    </span>
                  </div>

                  <!-- Isi Ucapan Apresiasi -->
                  <div class="my-auto text-center space-y-4 px-4 py-8 bg-slate-50 border border-slate-200 rounded-lg relative z-10">
                    <div class="inline-block px-3 py-1 rounded bg-slate-900 text-amber-400 font-mono text-[10px] font-bold uppercase tracking-wider mb-2">
                      FINAL TOURNAMENT RECAPITULATION
                    </div>
                    <h2 class="text-2xl sm:text-3xl font-black uppercase tracking-tight text-slate-950">
                      TERIMA KASIH ATAS PARTISIPASI & SPORTIVITAS
                    </h2>
                    <p class="text-xs sm:text-sm text-slate-700 max-w-lg mx-auto leading-relaxed font-medium">
                      Seluruh data hasil pertandingan, perolehan medali, penetapan juara, dan daftar penugasan wasit/juri dalam dokumen ini telah divalidasi dan ditutup secara resmi melalui sistem <strong>Ambrilindo Tournament Engine</strong>.
                    </p>
                    <div class="inline-block px-4 py-1.5 rounded-full bg-slate-200/80 border border-slate-300 text-xs font-mono text-slate-800">
                      Generated at: {new Date().toLocaleString('id-ID')}
                    </div>
                  </div>

                  <!-- Footer Penutup -->
                  <div class="border-t-2 border-slate-900 pt-4 flex items-center justify-between text-xs text-slate-600 font-mono relative z-10">
                    <span class="font-bold text-slate-900">AMBRILINDO TOURNAMENT ENGINE</span>
                    <span>Halaman 7 • Lembar Penutup & Kompilasi Akhir</span>
                  </div>
                </div>
              </div>
            </section>

          </div>
        {/if}


        <!-- ========================================================================= -->
        <!-- SEKSI 2: LAMPIRAN BAGAN PERTANDINGAN (A4 LANDSCAPE SHEET SIMULATION) -->
        <!-- ========================================================================= -->
        {#if viewFilter === "all" || viewFilter === "landscape"}
          <div class="booklet-section-landscape mt-8 print:mt-0">
            <!-- Screen-Only Section Separator -->
            <div class="no-print max-w-[297mm] mx-auto mb-6 px-4 py-3 rounded-xl bg-gradient-to-r from-sky-950/80 to-slate-900 border border-sky-500/30 flex items-center justify-between shadow-lg">
              <div class="flex items-center space-x-3">
                <span class="text-xl">🌿</span>
                <div>
                  <h4 class="text-xs font-black uppercase text-sky-300 tracking-wider">
                    LAMPIRAN RESMI: BAGAN PERTANDINGAN (A4 LANDSCAPE)
                  </h4>
                  <p class="text-[11px] text-zinc-400">
                    Seluruh bagan pohon eliminasi kategori ({activeBrackets.length} nomor tanding) dalam format A4 Landscape penuh.
                  </p>
                </div>
              </div>
              <button
                type="button"
                onclick={printLandscape}
                class="px-3 py-1 bg-sky-500 hover:bg-sky-400 text-white font-bold text-xs rounded-lg transition shadow flex items-center space-x-1.5 cursor-pointer"
              >
                <span>Cetak Lampiran Bagan</span>
              </button>
            </div>

            {#if activeBrackets.length === 0}
              <div class="paper-sheet paper-sheet-landscape text-center p-12 text-slate-400 italic">
                Belum ada bagan pertandingan yang dibuat untuk turnamen ini.
              </div>
            {:else}
              {#each activeBrackets as bCat, bIdx}
                {@const roundMap = groupMatchesByRound(bCat.matches, bookletPruneEmptyRounds)}
                {@const sortedRounds = Array.from(roundMap.keys()).sort((a, b) => a - b)}
                {@const totalRounds = sortedRounds.length}
                <section id="page-bagan-{bCat.category_id}" class="booklet-page booklet-page-landscape">
                  <div class="paper-sheet paper-sheet-landscape">
                    <!-- 1. Header Lembar Bagan -->
                    <div class="border-b-2 border-slate-900 pb-2 mb-3 flex items-center justify-between">
                      <div class="flex items-center space-x-3 min-w-0">
                        <div class="flex items-center space-x-2 shrink-0 bg-white p-0.5 rounded">
                          <img
                            src="/img/forki-logo.png"
                            alt="Logo FORKI"
                            class="h-8 w-auto object-contain shrink-0"
                          />
                          <div class="h-6 w-[1px] bg-slate-300 shrink-0"></div>
                          {#if bookletData.event_info.logo && logoMap[bookletData.event_info.logo]}
                            <img
                              src={logoMap[bookletData.event_info.logo]}
                              alt="Logo Event"
                              class="h-7 w-auto max-w-[80px] object-contain shrink-0"
                              onerror={(e) => { (e.currentTarget as HTMLImageElement).src = '/img/forki-logo.png'; }}
                            />
                          {:else}
                            <img
                              src="/img/ambrilindo-logo.png"
                              alt="Ambrilindo"
                              class="h-5 w-auto object-contain shrink-0"
                            />
                          {/if}
                        </div>
                        <div class="min-w-0">
                          <h2 class="text-sm font-black text-slate-950 uppercase tracking-tight truncate">
                            {bCat.bagan_kode ? `(${bCat.bagan_kode}) ` : ''}{bCat.category_name}
                          </h2>
                          <p class="text-[10px] text-slate-600 font-medium truncate">
                            {editableEventName} • {editableTempat} • {editableTanggal}
                          </p>
                        </div>
                      </div>
                      <div class="flex items-center space-x-2 shrink-0">
                        <span class="px-2 py-0.5 rounded text-[9px] font-black uppercase bg-slate-900 text-white">
                          {bCat.discipline.toUpperCase()}
                        </span>
                        <span class="px-2 py-0.5 rounded text-[9px] font-mono font-bold bg-amber-100 text-amber-900 border border-amber-300">
                          {bCat.matches.length} PARTAI
                        </span>
                      </div>
                    </div>

                    <!-- 2. Bracket Grid Columns -->
                    <div class="bracket-grid-wrapper flex-1 overflow-x-auto py-2">
                      <div class="flex items-stretch space-x-3 min-w-max h-full">
                        {#each sortedRounds as rNum, rIdx}
                          {@const isFinalRound = (rIdx === sortedRounds.length - 1)}
                          {@const rMatches = roundMap.get(rNum) || []}
                          <div class="bracket-col flex flex-col justify-around min-w-[210px] max-w-[240px]">
                            <!-- Column Title -->
                            <div class="text-center pb-2 mb-2 border-b border-slate-300 font-mono text-[10px] font-bold text-slate-700 uppercase">
                              <span class="{isFinalRound ? 'text-amber-800 font-black' : ''}">
                                {getRoundTitleText(rIdx, totalRounds)}
                              </span>
                            </div>

                            <!-- Match Cards in Column -->
                            <div class="flex flex-col justify-around flex-1 space-y-2">
                              {#each rMatches as m, mIdx}
                                <div class="bracket-card border {isFinalRound ? 'border-amber-500 bg-amber-50/40' : 'border-slate-300 bg-white'} rounded shadow-xs p-1.5 text-xs">
                                  <div class="flex items-center justify-between pb-1 mb-1 border-b border-slate-200 text-[9px] font-mono text-slate-500">
                                    <span class="font-bold">{formatMatchTitle(rIdx, totalRounds, m, mIdx)}</span>
                                    <span>Partai #{m.urutan || m.id}</span>
                                  </div>

                                  <!-- AKA Row -->
                                  <div class="flex items-center justify-between p-1 rounded mb-0.5 {m.pemenang === '1' ? 'bg-red-50 border-l-2 border-red-600 font-bold' : ''}">
                                    <div class="flex items-center space-x-1.5 min-w-0 pr-1">
                                      <span class="w-1.5 h-1.5 rounded-full bg-red-600 shrink-0"></span>
                                      {#if m.atlet1_logo && logoMap[m.atlet1_logo]}
                                        <img src={logoMap[m.atlet1_logo]} alt="" class="w-3.5 h-3.5 object-contain shrink-0" />
                                      {/if}
                                      <div class="min-w-0">
                                        <div class="text-[10px] truncate leading-tight {m.pemenang === '1' ? 'text-red-950 font-black' : 'text-slate-800'}">
                                          {#if m.pemenang === '1'}<span>👑 </span>{/if}
                                          {m.atlet1_nama || 'BYE'}
                                        </div>
                                        <div class="text-[8.5px] text-slate-500 truncate leading-none">
                                          {m.atlet1_kontingen || '—'}
                                        </div>
                                      </div>
                                    </div>
                                    <div class="font-mono text-xs font-black text-right shrink-0">
                                      {m.score1 ?? '-'}
                                    </div>
                                  </div>

                                  <!-- AO Row -->
                                  <div class="flex items-center justify-between p-1 rounded {m.pemenang === '2' ? 'bg-blue-50 border-l-2 border-blue-600 font-bold' : ''}">
                                    <div class="flex items-center space-x-1.5 min-w-0 pr-1">
                                      <span class="w-1.5 h-1.5 rounded-full bg-blue-600 shrink-0"></span>
                                      {#if m.atlet2_logo && logoMap[m.atlet2_logo]}
                                        <img src={logoMap[m.atlet2_logo]} alt="" class="w-3.5 h-3.5 object-contain shrink-0" />
                                      {/if}
                                      <div class="min-w-0">
                                        <div class="text-[10px] truncate leading-tight {m.pemenang === '2' ? 'text-blue-950 font-black' : 'text-slate-800'}">
                                          {#if m.pemenang === '2'}<span>👑 </span>{/if}
                                          {m.atlet2_nama || 'BYE'}
                                        </div>
                                        <div class="text-[8.5px] text-slate-500 truncate leading-none">
                                          {m.atlet2_kontingen || '—'}
                                        </div>
                                      </div>
                                    </div>
                                    <div class="font-mono text-xs font-black text-right shrink-0">
                                      {m.score2 ?? '-'}
                                    </div>
                                  </div>
                                </div>
                              {/each}
                            </div>
                          </div>
                        {/each}
                      </div>
                    </div>

                    <!-- 3. Podium Rekapan Juara Kategori (Jika Ada) -->
                    {#if bCat.winners && bCat.winners.length > 0}
                      <div class="mt-2 pt-2 border-t border-slate-200 flex items-center space-x-4 bg-slate-50/80 p-2 rounded text-[10px]">
                        <span class="font-black text-slate-900 uppercase">PODIUM AKHIR:</span>
                        <div class="flex flex-wrap items-center gap-3">
                          {#each bCat.winners as w}
                            <div class="flex items-center space-x-1">
                              {#if w.juara === '1'}
                                <span class="px-1.5 py-0.2 rounded bg-amber-500 text-slate-950 font-black font-mono">1</span>
                              {:else if w.juara === '2'}
                                <span class="px-1.5 py-0.2 rounded bg-slate-300 text-slate-950 font-black font-mono">2</span>
                              {:else}
                                <span class="px-1.5 py-0.2 rounded bg-amber-700 text-white font-black font-mono">3</span>
                              {/if}
                              <span class="font-bold text-slate-900">{w.nama_atlet}</span>
                              <span class="text-slate-500">({w.utusan_nama || w.perguruan_nama || '-'})</span>
                            </div>
                          {/each}
                        </div>
                      </div>
                    {/if}

                    <!-- 4. Footer Lembar Bagan -->
                    <div class="mt-3 pt-2 border-t border-slate-200 flex items-center justify-between text-[9px] text-slate-500 font-mono">
                      <span>Official Tournament Results • Lampiran Bagan Pertandingan</span>
                      <span>Lampiran {bIdx + 1}/{activeBrackets.length} • ({bCat.bagan_kode || bCat.category_id}) {bCat.category_name}</span>
                    </div>
                  </div>
                </section>
              {/each}
            {/if}
          </div>
        {/if}

      </main>
    {/if}
  </div>
</div>

<style>
  /* ==========================================================================
     In-App Screen Paper Simulation Styles
     ========================================================================== */
  .paper-sheet {
    box-sizing: border-box;
    transition: box-shadow 0.2s ease;
  }

  .paper-sheet-portrait {
    width: 210mm;
    min-height: 297mm;
    max-width: 100%;
    margin-left: auto;
    margin-right: auto;
    margin-bottom: 2.5rem;
    background-color: #ffffff;
    color: #0f172a;
    padding: 10mm;
    border-radius: 2px;
    box-shadow: 0 12px 30px -4px rgba(0, 0, 0, 0.45);
    display: flex;
    flex-direction: column;
    justify-content: space-between;
  }

  .paper-sheet-landscape {
    width: 297mm;
    min-height: 210mm;
    max-width: 100%;
    margin-left: auto;
    margin-right: auto;
    margin-bottom: 2.5rem;
    background-color: #ffffff;
    color: #0f172a;
    padding: 8mm;
    border-radius: 2px;
    box-shadow: 0 12px 30px -4px rgba(0, 0, 0, 0.45);
    display: flex;
    flex-direction: column;
    justify-content: space-between;
  }

  /* ==========================================================================
     Dual-Action Print Engine Styles (@media print)
     ========================================================================== */
  @media print {
    :global(body) {
      background: #ffffff !important;
      color: #000000 !important;
      padding: 0 !important;
      margin: 0 !important;
      -webkit-print-color-adjust: exact !important;
      print-color-adjust: exact !important;
    }

    .no-print {
      display: none !important;
    }

    .print-view-root {
      background: #ffffff !important;
      color: #000000 !important;
      height: auto !important;
      overflow: visible !important;
    }

    .booklet-workspace-container {
      max-width: 100% !important;
      padding: 0 !important;
      margin: 0 !important;
    }

    /* Print Mode 1: PORTRAIT ONLY */
    .is-printing-portrait .booklet-section-landscape,
    .is-printing-portrait .booklet-page-landscape,
    .is-printing-portrait .paper-sheet-landscape {
      display: none !important;
    }

    .is-printing-portrait .booklet-section-portrait,
    .is-printing-portrait .booklet-page-portrait,
    .is-printing-portrait .paper-sheet-portrait {
      display: flex !important;
    }

    /* Print Mode 2: LANDSCAPE ONLY */
    .is-printing-landscape .booklet-section-portrait,
    .is-printing-landscape .booklet-page-portrait,
    .is-printing-landscape .paper-sheet-portrait {
      display: none !important;
    }

    .is-printing-landscape .booklet-section-landscape,
    .is-printing-landscape .booklet-page-landscape,
    .is-printing-landscape .paper-sheet-landscape {
      display: flex !important;
    }

    .booklet-page {
      box-shadow: none !important;
      border: none !important;
      margin: 0 !important;
      border-radius: 0 !important;
      page-break-after: always !important;
      break-after: page !important;
      width: 100% !important;
    }

    .paper-sheet-portrait {
      width: 100% !important;
      min-height: 275mm !important;
      box-shadow: none !important;
      border: none !important;
      margin: 0 !important;
      padding: 0 !important;
      break-after: page !important;
      page-break-after: always !important;
    }

    .paper-sheet-landscape {
      width: 100% !important;
      min-height: 195mm !important;
      box-shadow: none !important;
      border: none !important;
      margin: 0 !important;
      padding: 0 !important;
      break-after: page !important;
      page-break-after: always !important;
    }
  }
</style>
