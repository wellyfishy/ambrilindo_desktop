<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/stores";
  import { apiCall, getConnectionConfig } from "$lib/api_client";
  import type { ResultsBookletData, ResultsBookletSignatory } from "$lib/types";

  let bookletData = $state<ResultsBookletData | null>(null);
  let isLoading = $state(true);
  let errorMessage = $state<string | null>(null);
  let isEditing = $state(false);

  // Editable local overrides for sign-off & header
  let editableEventName = $state("");
  let editableTempat = $state("");
  let editableTanggal = $state("");
  let editableKota = $state("");
  let editableNomorSurat = $state("");
  let editableCatatanSanksi = $state("");
  let editableSignatories = $state<ResultsBookletSignatory[]>([]);

  let eventId = $derived(
    parseInt($page.url.searchParams.get("event_id") || "1") || 1
  );

  async function loadData() {
    isLoading = true;
    errorMessage = null;
    try {
      const res = await apiCall<ResultsBookletData>("get_results_booklet_data", {
        eventId,
        event_id: eventId,
      });
      bookletData = res;
      editableEventName = res.event_info.nama_event;
      editableTempat = res.event_info.tempat;
      editableTanggal = res.event_info.tanggal;
      editableKota = res.event_info.kota;
      editableNomorSurat = res.berita_acara.nomor_surat;
      editableCatatanSanksi = res.berita_acara.catatan_sanksi;
      editableSignatories = JSON.parse(JSON.stringify(res.signatories));
    } catch (err: any) {
      console.error("Gagal memuat data buku hasil:", err);
      errorMessage = err?.message || String(err);
    } finally {
      isLoading = false;
    }
  }

  function handlePrint() {
    window.print();
  }

  function scrollToSection(id: string) {
    const el = document.getElementById(id);
    if (el) {
      el.scrollIntoView({ behavior: "smooth" });
    }
  }

  onMount(() => {
    loadData();
  });
</script>

<svelte:head>
  <title>Buku Hasil & Berita Acara Resmi • {editableEventName || 'Ambrilindo'}</title>
</svelte:head>

<div class="booklet-root min-h-screen bg-[#0b0d13] text-zinc-100 font-sans print:bg-white print:text-black">
  <!-- ================= SCREEN-ONLY STICKY TOOLBAR ================= -->
  <header class="no-print sticky top-0 z-50 bg-[#12151d]/95 backdrop-blur-md border-b border-white/[0.08] px-5 py-3 flex items-center justify-between gap-4 shadow-xl">
    <div class="flex items-center space-x-3 min-w-0">
      <div class="w-8 h-8 rounded-lg bg-amber-500/20 border border-amber-500/40 text-amber-400 font-black font-mono text-sm flex items-center justify-center shrink-0">
        BA
      </div>
      <div class="min-w-0">
        <h1 class="text-sm font-black text-white tracking-tight truncate uppercase">
          Buku Hasil & Berita Acara Resmi
        </h1>
        <p class="text-[11px] text-zinc-400 font-medium truncate">
          {editableEventName || 'Memuat...'} • {editableKota}
        </p>
      </div>
    </div>

    <!-- Quick Navigation Jump Buttons -->
    <div class="hidden lg:flex items-center space-x-1.5 bg-[#181a22] p-1 rounded-lg border border-white/[0.08] text-xs">
      <button
        type="button"
        onclick={() => scrollToSection("page-cover")}
        class="px-2.5 py-1 rounded text-zinc-300 hover:text-white hover:bg-white/[0.06] transition font-medium"
      >
        Sampul
      </button>
      <button
        type="button"
        onclick={() => scrollToSection("page-berita-acara")}
        class="px-2.5 py-1 rounded text-zinc-300 hover:text-white hover:bg-white/[0.06] transition font-medium"
      >
        Berita Acara & Pengesahan
      </button>
      <button
        type="button"
        onclick={() => scrollToSection("page-klasemen")}
        class="px-2.5 py-1 rounded text-zinc-300 hover:text-white hover:bg-white/[0.06] transition font-medium"
      >
        Klasemen Medali
      </button>
      <button
        type="button"
        onclick={() => scrollToSection("page-daftar-juara")}
        class="px-2.5 py-1 rounded text-zinc-300 hover:text-white hover:bg-white/[0.06] transition font-medium"
      >
        Daftar Juara
      </button>
      <button
        type="button"
        onclick={() => scrollToSection("page-wasit")}
        class="px-2.5 py-1 rounded text-zinc-300 hover:text-white hover:bg-white/[0.06] transition font-medium"
      >
        Dewan Wasit
      </button>
      <button
        type="button"
        onclick={() => scrollToSection("page-closing")}
        class="px-2.5 py-1 rounded text-zinc-300 hover:text-white hover:bg-white/[0.06] transition font-medium"
      >
        Penutup
      </button>
    </div>

    <!-- Action Buttons -->
    <div class="flex items-center space-x-2 shrink-0">
      <button
        type="button"
        onclick={() => (isEditing = !isEditing)}
        class="px-3 py-1.5 rounded-lg text-xs font-semibold border transition flex items-center space-x-1.5 {isEditing ? 'bg-amber-500/20 border-amber-500 text-amber-300 shadow-sm' : 'bg-white/[0.05] border-white/[0.1] text-zinc-300 hover:bg-white/[0.08]'}"
        title="Buka form untuk mengubah nama penandatangan, nomor surat, atau catatan sebelum cetak"
      >
        <span>✏️</span>
        <span>{isEditing ? 'Selesai Edit' : 'Edit Pengesahan'}</span>
      </button>

      <button
        type="button"
        onclick={handlePrint}
        class="px-4 py-1.5 rounded-lg text-xs font-bold bg-gradient-to-r from-red-600 to-amber-600 hover:from-red-500 hover:to-amber-500 text-white shadow-md active:scale-95 transition flex items-center space-x-1.5"
      >
        <svg class="w-4 h-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
        </svg>
        <span>Cetak Buku (A4 / PDF)</span>
      </button>
    </div>
  </header>

  <!-- Loading State -->
  {#if isLoading}
    <div class="h-96 flex flex-col items-center justify-center space-y-3 text-zinc-400">
      <div class="w-8 h-8 border-2 border-amber-500 border-t-transparent rounded-full animate-spin"></div>
      <span class="text-xs font-semibold uppercase tracking-wider">Menyiapkan Berita Acara & Rekapitulasi Buku Hasil...</span>
    </div>
  {:else if errorMessage}
    <div class="p-8 max-w-xl mx-auto my-12 bg-red-950/40 border border-red-500/40 rounded-xl text-center">
      <div class="text-red-400 font-bold mb-2">Gagal Memuat Data Hasil Kejuaraan</div>
      <p class="text-xs text-zinc-400 mb-4">{errorMessage}</p>
      <button
        type="button"
        onclick={loadData}
        class="px-4 py-2 bg-red-600 text-white text-xs font-bold rounded-lg shadow-sm"
      >
        Coba Lagi
      </button>
    </div>
  {:else if bookletData}
    <main class="booklet-document-container mx-auto py-8 print:py-0 print:m-0 max-w-[220mm]">

      <!-- ================= PAGE 1: SAMPUL RESMI (COVER) ================= -->
      <section id="page-cover" class="booklet-page booklet-page-landscape mb-8 print:mb-0 print:break-after-page">
        <div class="cover-canvas relative w-full aspect-[16/9] rounded-md overflow-hidden bg-[#111827] shadow-2xl border border-white/[0.08] print:border-none print:shadow-none print:w-full print:h-[100vh] print:aspect-auto">
          <!-- Background Template -->
          <img
            src="/img/cover/cover_clean_template.png"
            alt="Cover Template"
            class="absolute inset-0 w-full h-full object-cover object-center pointer-events-none"
          />

          <!-- Top-Right Organization Bar -->
          <div class="absolute top-[3.5%] right-[10%] text-right font-black tracking-wider text-white text-sm md:text-base drop-shadow-[0_2px_4px_rgba(0,0,0,0.9)] uppercase">
            {#if isEditing}
              <input
                type="text"
                bind:value={editableEventName}
                class="bg-black/60 text-white text-right px-2 py-0.5 rounded border border-amber-500/50 text-xs w-80"
              />
            {:else}
              <span>{bookletData.event_info.nama_organisasi || editableEventName}</span>
            {/if}
          </div>

          <!-- Top-Right Logo Box -->
          <div class="absolute top-[16%] right-[3.5%] w-[24%] h-[27%] flex items-center justify-center p-2 z-10">
            {#if bookletData.event_info.logo}
              <img
                src={bookletData.event_info.logo}
                alt="Logo Event"
                class="max-w-full max-h-full object-contain drop-shadow-[0_4px_8px_rgba(0,0,0,0.6)]"
              />
            {:else}
              <img
                src="/img/cover/wkf_logo.png"
                alt="WKF Logo"
                class="max-w-full max-h-full object-contain drop-shadow-[0_4px_8px_rgba(0,0,0,0.6)]"
              />
            {/if}
          </div>

          <!-- Main Title Group Box (Centered on right gray card) -->
          <div class="absolute top-[44%] left-[55%] w-[42%] text-left select-text z-10">
            <div class="inline-block px-2.5 py-0.5 rounded bg-gradient-to-r from-amber-600 to-amber-500 text-slate-950 font-black text-[11px] md:text-xs tracking-wider uppercase shadow-md mb-1.5">
              LAPORAN HASIL PERTANDINGAN & BERITA ACARA
            </div>

            <h2 class="text-xl md:text-2xl font-black text-white tracking-tight uppercase leading-tight drop-shadow-[0_2px_6px_rgba(0,0,0,0.9)] mb-1">
              {editableEventName}
            </h2>

            <div class="text-amber-300 font-bold text-xs md:text-sm drop-shadow-[0_1px_3px_rgba(0,0,0,0.9)] space-y-0.5">
              <div>📍 {editableTempat}</div>
              <div>📅 {editableTanggal}</div>
            </div>
          </div>

          <!-- Bottom Right Supported By -->
          <div class="absolute bottom-[4.5%] right-[5%] flex items-center space-x-2 z-10">
            <span class="text-[10px] font-bold uppercase tracking-widest text-zinc-300 drop-shadow">SUPPORTED BY:</span>
            <img src="/img/ambrilindo-logo-dark.png" alt="Ambrilindo" class="h-6 object-contain drop-shadow" />
          </div>
        </div>
      </section>

      <!-- ================= PAGE 2: BERITA ACARA & PENGESAHAN ================= -->
      <section id="page-berita-acara" class="booklet-page booklet-page-portrait mb-8 print:mb-0 print:break-after-page bg-white text-slate-900 p-8 rounded-md shadow-xl border border-slate-200 print:shadow-none print:border-none">
        <!-- Kop Surat Header -->
        <div class="border-b-2 border-slate-900 pb-3 mb-5 flex items-center justify-between gap-4">
          <div class="flex items-center space-x-4">
            <img
              src={bookletData.event_info.logo || '/img/cover/wkf_logo.png'}
              alt="Logo"
              class="w-16 h-16 object-contain shrink-0"
            />
            <div>
              <h2 class="text-base font-black tracking-tight text-slate-950 uppercase leading-tight">
                {bookletData.event_info.nama_organisasi || editableEventName}
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
              DOKUMEN RESMI PB / FORKI
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
        <div class="text-xs text-slate-800 leading-relaxed text-justify mb-4 space-y-2">
          <p>
            Pada hari ini, bertempat di <strong>{editableTempat}</strong>, telah diselesaikan seluruh rangkaian pertandingan dalam rangka <strong>{editableEventName}</strong>. Seluruh nomor pertandingan telah dipertandingkan dengan mempedomani Peraturan Pertandingan Resmi <em>World Karate Federation (WKF)</em> dan Pengurus Besar FORKI.
          </p>
          <p>
            Berdasarkan jalannya kejuaraan dan pencatatan oleh Sekretariat Pertandingan bersama Dewan Wasit, diperoleh rekapitulasi data penyelenggaraan teknis sebagai berikut:
          </p>
        </div>

        <!-- Event Statistics Table -->
        <div class="mb-4 overflow-hidden border border-slate-300 rounded">
          <table class="w-full text-xs text-left">
            <thead class="bg-slate-100 text-slate-900 uppercase font-black text-[10px] border-b border-slate-300">
              <tr>
                <th class="px-3 py-1.5 border-r border-slate-300">Parameter Penyelenggaraan</th>
                <th class="px-3 py-1.5 text-center border-r border-slate-300">Jumlah Terdaftar</th>
                <th class="px-3 py-1.5 text-center">Status / Catatan Teknis</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-slate-200">
              <tr>
                <td class="px-3 py-1.5 font-bold text-slate-800 border-r border-slate-200">Kontingen / Utusan Daerah & Dojo</td>
                <td class="px-3 py-1.5 font-mono font-bold text-center border-r border-slate-200">{bookletData.berita_acara.total_kontingen}</td>
                <td class="px-3 py-1.5 text-slate-700">Hadir & bertanding penuh</td>
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
        <div class="text-xs text-slate-800 leading-relaxed text-justify mb-6">
          <p>
            Dengan ini menyatakan bahwa seluruh perolehan medali, penetapan Juara Bagan/Kategori, dan penetapan Juara Umum Kontingen maupun Perguruan yang tercantum dalam Buku Hasil ini adalah <strong>BENAR, SAH, dan MENGIKAT</strong> sebagai keputusan mutlak yang disahkan bersama oleh Dewan Wasit dan Panitia Pelaksana.
          </p>
        </div>

        <!-- Date & City Line -->
        <div class="text-right text-xs font-bold text-slate-800 mb-4">
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
        <div class="grid grid-cols-2 gap-6 pt-2">
          {#each editableSignatories as sig, idx}
            <div class="border border-slate-300 rounded p-3 text-center flex flex-col justify-between min-h-[145px] bg-slate-50/50">
              <div>
                <div class="text-[11px] font-black uppercase text-slate-900 tracking-tight">
                  {sig.jabatan}
                </div>
                <div class="text-[10px] text-slate-600 font-medium">
                  {sig.peran}
                </div>
              </div>

              <!-- Signature Area with Seal Ring -->
              <div class="relative py-3 flex items-center justify-center">
                <div class="w-16 h-16 rounded-full border border-dashed border-slate-300 text-slate-300 text-[9px] font-bold uppercase flex items-center justify-center select-none opacity-40">
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
        <div class="mt-8 pt-3 border-t border-slate-200 flex items-center justify-between text-[9px] text-slate-500 font-mono">
          <span>Official Match Result Book • Ambrilindo Tournament Engine</span>
          <span>Halaman 2 • Lembar Pengesahan Resmi</span>
        </div>
      </section>

      <!-- ================= PAGE 3: KLASEMEN MEDALI (JUARA UMUM) ================= -->
      <section id="page-klasemen" class="booklet-page booklet-page-portrait mb-8 print:mb-0 print:break-after-page bg-white text-slate-900 p-8 rounded-md shadow-xl border border-slate-200 print:shadow-none print:border-none">
        <div class="border-b-2 border-slate-900 pb-2 mb-4 flex items-center justify-between">
          <div>
            <h3 class="text-sm font-black text-slate-950 uppercase tracking-tight">
              REKAPITULASI MEDALI & JUARA UMUM
            </h3>
            <p class="text-[11px] text-slate-600 font-medium">
              {editableEventName} • {editableTempat}
            </p>
          </div>
          <span class="px-2 py-0.5 rounded text-[10px] font-black uppercase bg-amber-500 text-slate-950">
            HALAMAN KLASEMEN
          </span>
        </div>

        <!-- Tabel 1: Juara Umum Kontingen -->
        <div class="mb-6">
          <div class="flex items-center space-x-2 mb-2">
            <span class="w-2.5 h-2.5 rounded-full bg-blue-600"></span>
            <h4 class="text-xs font-black uppercase tracking-wider text-slate-900">
              Klasemen Perolehan Medali Kontingen / Utusan Daerah
            </h4>
          </div>

          <table class="w-full text-xs text-left border border-slate-300">
            <thead class="bg-slate-900 text-white text-[10px] uppercase font-black">
              <tr>
                <th class="px-2.5 py-1.5 text-center w-12 border-r border-slate-700">Rank</th>
                <th class="px-3 py-1.5 border-r border-slate-700">Nama Kontingen / Utusan</th>
                <th class="px-2.5 py-1.5 text-center w-16 bg-amber-500 text-slate-950 border-r border-amber-600">Emas</th>
                <th class="px-2.5 py-1.5 text-center w-16 bg-slate-200 text-slate-900 border-r border-slate-300">Perak</th>
                <th class="px-2.5 py-1.5 text-center w-16 bg-amber-700 text-white border-r border-amber-800">Perunggu</th>
                <th class="px-2.5 py-1.5 text-center w-16 bg-slate-800 text-white">Total</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-slate-200">
              {#if bookletData.standings_kontingen.length === 0}
                <tr>
                  <td colspan="6" class="px-3 py-4 text-center text-slate-400 italic">
                    Belum ada medali yang tercatat.
                  </td>
                </tr>
              {:else}
                {#each bookletData.standings_kontingen as k}
                  <tr class="{k.rank === 1 ? 'bg-amber-50/70 font-bold' : (k.rank <= 3 ? 'bg-slate-50/70' : '')}">
                    <td class="px-2.5 py-1 text-center font-mono font-bold border-r border-slate-200">
                      {#if k.rank === 1}
                        <span class="inline-block px-1.5 py-0.2 rounded bg-amber-500 text-slate-950 font-black text-[10px]">1</span>
                      {:else if k.rank === 2}
                        <span class="inline-block px-1.5 py-0.2 rounded bg-slate-300 text-slate-950 font-black text-[10px]">2</span>
                      {:else if k.rank === 3}
                        <span class="inline-block px-1.5 py-0.2 rounded bg-amber-700 text-white font-black text-[10px]">3</span>
                      {:else}
                        <span>{k.rank}</span>
                      {/if}
                    </td>
                    <td class="px-3 py-1 text-slate-900 border-r border-slate-200 font-semibold truncate max-w-xs">
                      {k.nama}
                    </td>
                    <td class="px-2.5 py-1 text-center font-mono font-black text-amber-700 border-r border-slate-200">{k.gold}</td>
                    <td class="px-2.5 py-1 text-center font-mono font-bold text-slate-600 border-r border-slate-200">{k.silver}</td>
                    <td class="px-2.5 py-1 text-center font-mono font-bold text-amber-800 border-r border-slate-200">{k.bronze}</td>
                    <td class="px-2.5 py-1 text-center font-mono font-black text-slate-950">{k.total}</td>
                  </tr>
                {/each}
              {/if}
            </tbody>
            <tfoot class="bg-slate-900 text-white font-black text-xs border-t-2 border-slate-900">
              <tr>
                <td colspan="2" class="px-3 py-1.5 text-right uppercase tracking-wider">Total Keseluruhan</td>
                <td class="px-2.5 py-1.5 text-center text-amber-400 font-mono">{bookletData.total_gold}</td>
                <td class="px-2.5 py-1.5 text-center text-slate-200 font-mono">{bookletData.total_silver}</td>
                <td class="px-2.5 py-1.5 text-center text-amber-500 font-mono">{bookletData.total_bronze}</td>
                <td class="px-2.5 py-1.5 text-center text-white font-mono">{bookletData.grand_total}</td>
              </tr>
            </tfoot>
          </table>
        </div>

        <!-- Tabel 2: Juara Umum Perguruan -->
        <div class="mb-4">
          <div class="flex items-center space-x-2 mb-2">
            <span class="w-2.5 h-2.5 rounded-full bg-emerald-600"></span>
            <h4 class="text-xs font-black uppercase tracking-wider text-slate-900">
              Klasemen Perolehan Medali Perguruan / Aliran
            </h4>
          </div>

          <table class="w-full text-xs text-left border border-slate-300">
            <thead class="bg-slate-900 text-white text-[10px] uppercase font-black">
              <tr>
                <th class="px-2.5 py-1.5 text-center w-12 border-r border-slate-700">Rank</th>
                <th class="px-3 py-1.5 border-r border-slate-700">Nama Perguruan</th>
                <th class="px-2.5 py-1.5 text-center w-16 bg-amber-500 text-slate-950 border-r border-amber-600">Emas</th>
                <th class="px-2.5 py-1.5 text-center w-16 bg-slate-200 text-slate-900 border-r border-slate-300">Perak</th>
                <th class="px-2.5 py-1.5 text-center w-16 bg-amber-700 text-white border-r border-amber-800">Perunggu</th>
                <th class="px-2.5 py-1.5 text-center w-16 bg-slate-800 text-white">Total</th>
              </tr>
            </thead>
            <tbody class="divide-y divide-slate-200">
              {#if bookletData.standings_perguruan.length === 0}
                <tr>
                  <td colspan="6" class="px-3 py-4 text-center text-slate-400 italic">
                    Belum ada medali perguruan yang tercatat.
                  </td>
                </tr>
              {:else}
                {#each bookletData.standings_perguruan as p}
                  <tr class="{p.rank === 1 ? 'bg-amber-50/70 font-bold' : (p.rank <= 3 ? 'bg-slate-50/70' : '')}">
                    <td class="px-2.5 py-1 text-center font-mono font-bold border-r border-slate-200">
                      {#if p.rank === 1}
                        <span class="inline-block px-1.5 py-0.2 rounded bg-amber-500 text-slate-950 font-black text-[10px]">1</span>
                      {:else if p.rank === 2}
                        <span class="inline-block px-1.5 py-0.2 rounded bg-slate-300 text-slate-950 font-black text-[10px]">2</span>
                      {:else if p.rank === 3}
                        <span class="inline-block px-1.5 py-0.2 rounded bg-amber-700 text-white font-black text-[10px]">3</span>
                      {:else}
                        <span>{p.rank}</span>
                      {/if}
                    </td>
                    <td class="px-3 py-1 text-slate-900 border-r border-slate-200 font-semibold truncate max-w-xs">
                      {p.nama}
                    </td>
                    <td class="px-2.5 py-1 text-center font-mono font-black text-amber-700 border-r border-slate-200">{p.gold}</td>
                    <td class="px-2.5 py-1 text-center font-mono font-bold text-slate-600 border-r border-slate-200">{p.silver}</td>
                    <td class="px-2.5 py-1 text-center font-mono font-bold text-amber-800 border-r border-slate-200">{p.bronze}</td>
                    <td class="px-2.5 py-1 text-center font-mono font-black text-slate-950">{p.total}</td>
                  </tr>
                {/each}
              {/if}
            </tbody>
          </table>
        </div>

        <div class="mt-8 pt-3 border-t border-slate-200 flex items-center justify-between text-[9px] text-slate-500 font-mono">
          <span>Official Match Result Book • Ambrilindo Tournament Engine</span>
          <span>Halaman 3 • Rekapitulasi Perolehan Medali</span>
        </div>
      </section>

      <!-- ================= PAGE 4+: DAFTAR JUARA SELURUH KATEGORI ================= -->
      <section id="page-daftar-juara" class="booklet-page booklet-page-portrait mb-8 print:mb-0 print:break-after-page bg-white text-slate-900 p-8 rounded-md shadow-xl border border-slate-200 print:shadow-none print:border-none">
        <div class="border-b-2 border-slate-900 pb-2 mb-4 flex items-center justify-between">
          <div>
            <h3 class="text-sm font-black text-slate-950 uppercase tracking-tight">
              DAFTAR JUARA SETIAP KELAS / NOMOR PERTANDINGAN
            </h3>
            <p class="text-[11px] text-slate-600 font-medium">
              Podium Juara 1 s.d Juara 3 Bersama Seluruh Nomor Tanding
            </p>
          </div>
          <span class="px-2 py-0.5 rounded text-[10px] font-black uppercase bg-blue-600 text-white">
            PODIUM KATEGORI
          </span>
        </div>

        {#if bookletData.category_winners.length === 0}
          <div class="p-8 text-center text-slate-400 italic">
            Belum ada nomor pertandingan yang diselesaikan.
          </div>
        {:else}
          <div class="grid grid-cols-1 md:grid-cols-2 gap-3.5 print:grid-cols-2">
            {#each bookletData.category_winners as cat}
              {@const w1 = cat.winners.find((w) => w.juara === "1")}
              {@const w2 = cat.winners.find((w) => w.juara === "2")}
              {@const w3a = cat.winners.find((w) => w.juara === "3a")}
              {@const w3b = cat.winners.find((w) => w.juara === "3b")}
              <div class="border border-slate-300 rounded p-2.5 bg-slate-50/60 print:bg-white break-inside-avoid">
                <!-- Header Kategori -->
                <div class="border-b border-slate-200 pb-1.5 mb-2 flex items-center justify-between">
                  <span class="text-[10px] font-mono font-bold px-1.5 py-0.2 rounded bg-slate-900 text-white">
                    {cat.kode}
                  </span>
                  <span class="text-xs font-black text-slate-900 uppercase truncate ml-2">
                    {cat.nama_bagan}
                  </span>
                </div>

                <!-- 4 Podium Rows -->
                <div class="space-y-1 text-[11px]">
                  <!-- Juara 1 -->
                  <div class="flex items-center space-x-1.5">
                    <span class="w-4 h-4 rounded-full bg-amber-400 text-slate-950 font-black text-[9px] flex items-center justify-center shrink-0">
                      1
                    </span>
                    <span class="font-bold text-slate-950 truncate flex-1">
                      {w1?.nama_atlet || '-'}
                    </span>
                    <span class="text-[10px] text-slate-600 truncate max-w-[130px] font-medium">
                      {w1?.utusan_nama || w1?.perguruan_nama || '-'}
                    </span>
                  </div>

                  <!-- Juara 2 -->
                  <div class="flex items-center space-x-1.5">
                    <span class="w-4 h-4 rounded-full bg-slate-300 text-slate-950 font-black text-[9px] flex items-center justify-center shrink-0">
                      2
                    </span>
                    <span class="font-semibold text-slate-800 truncate flex-1">
                      {w2?.nama_atlet || '-'}
                    </span>
                    <span class="text-[10px] text-slate-600 truncate max-w-[130px] font-medium">
                      {w2?.utusan_nama || w2?.perguruan_nama || '-'}
                    </span>
                  </div>

                  <!-- Juara 3A -->
                  <div class="flex items-center space-x-1.5">
                    <span class="w-4 h-4 rounded-full bg-amber-700 text-white font-black text-[8px] flex items-center justify-center shrink-0">
                      3A
                    </span>
                    <span class="font-medium text-slate-700 truncate flex-1">
                      {w3a?.nama_atlet || '-'}
                    </span>
                    <span class="text-[10px] text-slate-500 truncate max-w-[130px]">
                      {w3a?.utusan_nama || w3a?.perguruan_nama || '-'}
                    </span>
                  </div>

                  <!-- Juara 3B -->
                  <div class="flex items-center space-x-1.5">
                    <span class="w-4 h-4 rounded-full bg-amber-700 text-white font-black text-[8px] flex items-center justify-center shrink-0">
                      3B
                    </span>
                    <span class="font-medium text-slate-700 truncate flex-1">
                      {w3b?.nama_atlet || '-'}
                    </span>
                    <span class="text-[10px] text-slate-500 truncate max-w-[130px]">
                      {w3b?.utusan_nama || w3b?.perguruan_nama || '-'}
                    </span>
                  </div>
                </div>
              </div>
            {/each}
          </div>
        {/if}

        <div class="mt-8 pt-3 border-t border-slate-200 flex items-center justify-between text-[9px] text-slate-500 font-mono">
          <span>Official Match Result Book • Ambrilindo Tournament Engine</span>
          <span>Halaman 4 • Rekapan Podium Seluruh Kategori</span>
        </div>
      </section>

      <!-- ================= PAGE 5: DEWAN WASIT & JURI ================= -->
      <section id="page-wasit" class="booklet-page booklet-page-portrait mb-8 print:mb-0 print:break-after-page bg-white text-slate-900 p-8 rounded-md shadow-xl border border-slate-200 print:shadow-none print:border-none">
        <div class="border-b-2 border-slate-900 pb-2 mb-4 flex items-center justify-between">
          <div>
            <h3 class="text-sm font-black text-slate-950 uppercase tracking-tight">
              DAFTAR DEWAN WASIT & JURI YANG BERTUGAS
            </h3>
            <p class="text-[11px] text-slate-600 font-medium">
              Korps Perwasitan Kejuaraan Karate • FORKI
            </p>
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
                <tr>
                  <td class="px-2.5 py-1 text-center font-mono text-slate-500 border-r border-slate-200">{idx + 1}</td>
                  <td class="px-3 py-1 font-bold text-slate-900 border-r border-slate-200">{ref.nama_wasit}</td>
                  <td class="px-3 py-1 text-slate-700 border-r border-slate-200">
                    <span class="font-semibold">{ref.lisensi || '-'}</span>
                    {#if ref.no_lisensi}
                      <span class="text-[10px] font-mono text-slate-500 ml-1">({ref.no_lisensi})</span>
                    {/if}
                  </td>
                  <td class="px-3 py-1 text-slate-700 border-r border-slate-200">{ref.nama_perguruan || '-'}</td>
                  <td class="px-3 py-1 text-slate-700 border-r border-slate-200">{ref.kab_kota || '-'}</td>
                  <td class="px-3 py-1 text-center font-bold text-[10px]">
                    {#if ref.assigned_tatami_number}
                      <span class="px-2 py-0.5 rounded bg-blue-100 text-blue-800">
                        Tatami {ref.assigned_tatami_number} • {ref.assigned_posisi || 'Juri'}
                      </span>
                    {:else}
                      <span class="text-slate-400 italic">Cadangan / Meja</span>
                    {/if}
                  </td>
                </tr>
              {/each}
            </tbody>
          </table>
        {/if}

        <div class="mt-8 pt-3 border-t border-slate-200 flex items-center justify-between text-[9px] text-slate-500 font-mono">
          <span>Official Match Result Book • Ambrilindo Tournament Engine</span>
          <span>Halaman 5 • Daftar Korps Wasit & Juri</span>
        </div>
      </section>

      <!-- ================= PAGE 6: SAMPUL PENUTUP (CLOSING) ================= -->
      <section id="page-closing" class="booklet-page booklet-page-landscape mb-8 print:mb-0 bg-gradient-to-br from-slate-900 via-slate-950 to-black text-white p-12 rounded-md shadow-2xl border border-white/[0.08] flex flex-col justify-between aspect-[16/9] print:shadow-none print:border-none print:w-full print:h-[100vh]">
        <div class="text-right text-xs font-mono font-bold text-amber-500 uppercase tracking-widest">
          {bookletData.event_info.nama_organisasi || editableEventName}
        </div>

        <div class="text-center my-auto space-y-4">
          <div class="text-4xl md:text-5xl font-black text-white tracking-wider uppercase drop-shadow-lg">
            TERIMA KASIH
          </div>
          <p class="text-sm md:text-base font-semibold text-zinc-300 max-w-xl mx-auto leading-relaxed">
            Atas sportivitas, dedikasi, dan perjuangan seluruh kontingen, atlet, wasit/juri, official, serta jajaran panitia pelaksana.
          </p>
          <div class="text-amber-400 font-bold text-sm tracking-wide">
            {editableEventName} • {editableKota} {editableTanggal}
          </div>
        </div>

        <div class="pt-6 border-t border-white/[0.1] flex items-center justify-between">
          <div class="text-[11px] font-bold uppercase tracking-widest text-zinc-400">
            JUNJUNG TINGGI SPORTIVITAS & SEMANGAT KARATE-DO
          </div>
          <div class="flex items-center space-x-2">
            <span class="text-[10px] font-bold text-zinc-500 uppercase">ENGINE POWERED BY:</span>
            <img src="/img/ambrilindo-logo-dark.png" alt="Ambrilindo" class="h-6 object-contain" />
          </div>
        </div>
      </section>

    </main>
  {/if}
</div>

<style>
  @media print {
    :global(body) {
      background: white !important;
      color: black !important;
      padding: 0 !important;
      margin: 0 !important;
      -webkit-print-color-adjust: exact !important;
      print-color-adjust: exact !important;
    }

    .no-print {
      display: none !important;
    }

    .booklet-root {
      background: white !important;
      color: black !important;
      min-height: 0 !important;
      padding: 0 !important;
      margin: 0 !important;
    }

    .booklet-document-container {
      max-width: 100% !important;
      padding: 0 !important;
      margin: 0 !important;
    }

    .booklet-page {
      box-shadow: none !important;
      border: none !important;
      margin: 0 !important;
      border-radius: 0 !important;
      page-break-after: always !important;
      break-after: page !important;
    }

    .booklet-page-portrait {
      width: 100% !important;
      min-height: 270mm !important;
      padding: 10mm !important;
      box-sizing: border-box !important;
    }

    .booklet-page-landscape {
      width: 100% !important;
      height: 100vh !important;
      max-height: 100vh !important;
      overflow: hidden !important;
      page-break-after: always !important;
      break-after: page !important;
    }
  }
</style>
