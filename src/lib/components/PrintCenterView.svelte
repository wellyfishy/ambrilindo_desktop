<!--
  #cetak channel: one place to preview and print documents: the results booklet (with a
  customizable cover and a day filter for the bracket attachment) and the Roster Bagan of
  one Jadwal day and tatami. The kop surat (organisation, venue, contact) is set here too.
-->
<script lang="ts">
  import ResultsBookletView from "$lib/components/ResultsBookletView.svelte";
  import { invoke } from "$lib/api_client";
  import type { BaganDaysResult, KopSurat, TatamiItem } from "$lib/types";
  import {
    clearCoverSettings,
    defaultCoverSettings,
    loadCoverSettings,
    resolveCoverText,
    saveCoverSettings,
    type CoverSettings,
  } from "$lib/cover_settings";

  interface Props {
    eventId: number;
    eventName?: string;
    hostAddress?: string;
    onShowToast?: (msg: string) => void;
  }

  let { eventId, eventName = "", hostAddress = "", onShowToast }: Props = $props();

  type DocumentKind = "booklet_hasil" | "roster_bagan";
  let documentKind = $state<DocumentKind>("booklet_hasil");
  const DOCUMENTS: { value: DocumentKind; label: string }[] = [
    { value: "booklet_hasil", label: "Buku hasil turnamen" },
    { value: "roster_bagan", label: "Roster Bagan" },
  ];

  // Roster Bagan: one Jadwal day and one tatami
  let tatamiList = $state<TatamiItem[]>([]);
  let rosterDayId = $state<number | null>(null);
  let rosterTatamiId = $state<number | null>(null);
  $effect(() => {
    const id = eventId;
    tatamiList = [];
    rosterTatamiId = null;
    invoke<TatamiItem[]>("get_tatamis", { eventId: id })
      .then((list) => {
        if (id !== eventId) return;
        tatamiList = list;
        rosterTatamiId = list[0]?.id ?? null;
      })
      .catch(() => {});
  });
  $effect(() => {
    const days = baganDays?.days ?? [];
    if (!days.some((d) => d.id === rosterDayId)) rosterDayId = days[0]?.id ?? null;
  });

  let settings = $state<CoverSettings>(defaultCoverSettings());
  let loadedFor = $state<number | null>(null);
  let storageWarning = $state(false);
  let eventDefaults = $state({ namaEvent: "", tempat: "", tanggalRaw: "", hasEventLogo: false });

  // Each document keeps its own cover (same options): load the saved one.
  let coverDoc = $derived<"hasil" | "roster_bagan">(documentKind === "roster_bagan" ? "roster_bagan" : "hasil");
  let loadedDoc = $state<"hasil" | "roster_bagan">("hasil");
  $effect(() => {
    const id = eventId;
    const doc = coverDoc;
    settings = loadCoverSettings(id, doc);
    storageWarning = false;
    loadedFor = id;
    loadedDoc = doc;
  });

  // Remember edits for this event and document.
  $effect(() => {
    const snapshot = $state.snapshot(settings);
    if (loadedFor !== eventId || loadedDoc !== coverDoc) return;
    storageWarning = !saveCoverSettings(eventId, snapshot as CoverSettings, coverDoc);
  });

  let placeholders = $derived(resolveCoverText(defaultCoverSettings(), eventDefaults));

  // Kop surat (organisation, venue, contact): shared by every printout of the event,
  // including the bracket pages ("Alamat venue", first line = venue name).
  let kopNama = $state("");
  let kopAlamat = $state("");
  let kopKontak = $state("");
  let kopLogo = $state<string | null>(null);
  let kopSaved = $state({ nama: "", alamat: "", kontak: "" });
  let isSavingKop = $state(false);
  let kopDirty = $derived(kopNama !== kopSaved.nama || kopAlamat !== kopSaved.alamat || kopKontak !== kopSaved.kontak);
  let bookletKey = $state(0);

  $effect(() => {
    const id = eventId;
    invoke<KopSurat | null>("get_kop_surat", { eventId: id })
      .then((kop) => {
        if (id !== eventId) return;
        kopNama = kop?.nama_organisasi ?? "";
        kopAlamat = kop?.alamat ?? "";
        kopKontak = kop?.kontak ?? "";
        kopLogo = kop?.logo ?? null;
        kopSaved = { nama: kopNama, alamat: kopAlamat, kontak: kopKontak };
      })
      .catch(() => {});
  });

  async function saveKop() {
    if (isSavingKop) return;
    isSavingKop = true;
    try {
      await invoke<KopSurat>("save_kop_surat", {
        eventId,
        namaOrganisasi: kopNama.trim(),
        alamat: kopAlamat.trim(),
        kontak: kopKontak.trim(),
        logo: kopLogo,
      });
      kopNama = kopNama.trim();
      kopAlamat = kopAlamat.trim();
      kopKontak = kopKontak.trim();
      kopSaved = { nama: kopNama, alamat: kopAlamat, kontak: kopKontak };
      bookletKey++; // reload the preview with the new kop surat
      onShowToast?.("Kop surat disimpan.");
    } catch (err: any) {
      onShowToast?.(`Gagal menyimpan kop surat: ${err?.message || err}`);
    } finally {
      isSavingKop = false;
    }
  }

  // Jadwal days, for picking which days' brackets go into "Lampiran resmi"
  let baganDays = $state<BaganDaysResult | null>(null);
  $effect(() => {
    const id = eventId;
    baganDays = null;
    invoke<BaganDaysResult>("get_bagan_days", { eventId: id })
      .then((r) => {
        if (id === eventId) baganDays = r;
      })
      .catch(() => {});
  });

  let dayOptions = $derived([
    ...(baganDays?.days ?? []).map((d) => ({ value: String(d.id), label: d.label })),
    { value: "none", label: "Belum terjadwal" },
  ]);

  function isDayChecked(value: string): boolean {
    return !settings.lampiranDays || settings.lampiranDays.includes(value);
  }

  function toggleDay(value: string) {
    const all = dayOptions.map((d) => d.value);
    const current = settings.lampiranDays ?? all;
    const next = current.includes(value) ? current.filter((v) => v !== value) : [...current, value];
    settings.lampiranDays = all.every((v) => next.includes(v)) ? null : next;
  }

  type TextKey = "federasi" | "badge" | "judul" | "namaEvent" | "tempat" | "tanggal";
  const TEXT_FIELDS: { key: TextKey; label: string }[] = [
    { key: "federasi", label: "Teks atas" },
    { key: "badge", label: "Label" },
    { key: "judul", label: "Judul" },
    { key: "namaEvent", label: "Nama event" },
    { key: "tempat", label: "Tempat" },
    { key: "tanggal", label: "Tanggal" },
  ];

  function setText(key: TextKey, value: string) {
    // Empty means "use the default", shown as the placeholder.
    settings[key] = value === "" ? null : value;
  }

  /** Reads an image file and scales it down so it stays small enough to remember. */
  function readImage(file: File, maxWidth: number, type: "image/jpeg" | "image/png"): Promise<string> {
    return new Promise((resolve, reject) => {
      const reader = new FileReader();
      reader.onerror = () => reject(reader.error);
      reader.onload = () => {
        const img = new Image();
        img.onerror = () => reject(new Error("Gambar tidak bisa dibaca."));
        img.onload = () => {
          const scale = Math.min(1, maxWidth / img.naturalWidth);
          const canvas = document.createElement("canvas");
          canvas.width = Math.round(img.naturalWidth * scale);
          canvas.height = Math.round(img.naturalHeight * scale);
          const ctx = canvas.getContext("2d");
          if (!ctx) return resolve(String(reader.result));
          ctx.drawImage(img, 0, 0, canvas.width, canvas.height);
          resolve(canvas.toDataURL(type, 0.9));
        };
        img.src = String(reader.result);
      };
      reader.readAsDataURL(file);
    });
  }

  async function handleLogoFile(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0];
    if (!file) return;
    try {
      settings.customLogo = await readImage(file, 900, "image/png");
      settings.logoMode = "custom";
    } catch (err: any) {
      onShowToast?.(`Gagal memuat logo: ${err?.message || err}`);
    }
  }

  async function handleBackgroundFile(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0];
    if (!file) return;
    try {
      settings.customBackground = await readImage(file, 2000, "image/jpeg");
    } catch (err: any) {
      onShowToast?.(`Gagal memuat gambar latar: ${err?.message || err}`);
    }
  }

  async function handleSignatureFile(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    try {
      // PNG keeps the strokes sharp; 900 px is plenty for a 60 mm wide signature.
      settings.ttdImage = await readImage(file, 900, "image/png");
      settings.ttdEnabled = true;
    } catch (err: any) {
      onShowToast?.(`Gagal memuat tanda tangan: ${err?.message || err}`);
    } finally {
      input.value = "";
    }
  }

  function resetCover() {
    clearCoverSettings(eventId, coverDoc);
    settings = defaultCoverSettings();
    onShowToast?.("Sampul dikembalikan ke default.");
  }

  const LOGO_MODES: { value: CoverSettings["logoMode"]; label: string }[] = [
    { value: "event", label: "Logo event" },
    { value: "custom", label: "Unggah" },
    { value: "none", label: "Tanpa" },
  ];
</script>

<div class="h-full flex overflow-hidden bg-[#14161b] text-zinc-200 font-sans select-none">
  <!-- Settings -->
  <aside class="w-80 shrink-0 border-r border-white/[0.08] bg-[#111215] flex flex-col min-h-0">
    <div class="px-5 pt-5 pb-4 border-b border-white/[0.08] shrink-0">
      <h2 class="text-xl font-black text-white tracking-tight uppercase">Cetak</h2>
      <div class="mt-3 grid grid-cols-2 gap-1 p-1 rounded-xl bg-[#181a20] border border-white/[0.08]">
        {#each DOCUMENTS as doc (doc.value)}
          <button
            type="button"
            onclick={() => (documentKind = doc.value)}
            class="py-1.5 rounded-lg text-xs font-semibold transition {documentKind === doc.value ? 'bg-white text-black' : 'text-zinc-400 hover:text-white hover:bg-white/[0.06]'}"
          >
            {doc.label}
          </button>
        {/each}
      </div>
    </div>

    <div class="flex-1 overflow-y-auto px-5 py-4 space-y-5 scrollbar-thin">
      <div class="space-y-3 pb-4 border-b border-white/[0.08]">
        <span class="text-sm font-semibold text-zinc-200">Kop surat &amp; venue</span>
        <label class="block">
          <span class="text-xs text-zinc-400">Nama organisasi</span>
          <input type="text" bind:value={kopNama} placeholder={eventName || "Panitia pelaksana"} class="mt-1 w-full bg-[#181a20] text-sm text-white px-2.5 py-1.5 rounded-lg border border-white/[0.1] focus:border-white/30 outline-none placeholder:text-zinc-500 select-text" />
        </label>
        <label class="block">
          <span class="text-xs text-zinc-400">Alamat venue</span>
          <textarea rows="2" bind:value={kopAlamat} placeholder={"GOR Segiri\nSamarinda"} class="mt-1 w-full resize-none bg-[#181a20] text-sm text-white px-2.5 py-1.5 rounded-lg border border-white/[0.1] focus:border-white/30 outline-none placeholder:text-zinc-500 select-text"></textarea>
          <span class="text-[11px] text-zinc-500">Baris pertama dicetak sebagai venue di bagan dan buku.</span>
        </label>
        <label class="block">
          <span class="text-xs text-zinc-400">Kontak / tanggal pelaksanaan</span>
          <input type="text" bind:value={kopKontak} class="mt-1 w-full bg-[#181a20] text-sm text-white px-2.5 py-1.5 rounded-lg border border-white/[0.1] focus:border-white/30 outline-none select-text" />
        </label>
        <button
          type="button"
          onclick={saveKop}
          disabled={!kopDirty || isSavingKop}
          class="w-full h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {isSavingKop ? "Menyimpan..." : kopDirty ? "Simpan kop surat" : "Tersimpan"}
        </button>
      </div>

      {#if documentKind === "roster_bagan"}
        <div class="space-y-3">
          <label class="block">
            <span class="text-xs text-zinc-400">Hari</span>
            <select bind:value={rosterDayId} class="mt-1 w-full bg-[#181a20] text-sm text-white px-2.5 py-1.5 rounded-lg border border-white/[0.1] focus:border-white/30 outline-none">
              {#each baganDays?.days ?? [] as day (day.id)}
                <option value={day.id}>{day.label}</option>
              {/each}
            </select>
          </label>
          <label class="block">
            <span class="text-xs text-zinc-400">Tatami</span>
            <select bind:value={rosterTatamiId} class="mt-1 w-full bg-[#181a20] text-sm text-white px-2.5 py-1.5 rounded-lg border border-white/[0.1] focus:border-white/30 outline-none">
              {#each tatamiList as t (t.id)}
                <option value={t.id}>Tatami {t.tatami_number}</option>
              {/each}
            </select>
          </label>
          <p class="text-[11px] text-zinc-500">Sampul, roster dari Jadwal, lalu bagan yang dimainkan di tatami ini pada hari itu.</p>
        </div>
      {/if}
      <div class="flex items-center justify-between">
        <span class="text-sm font-semibold text-zinc-200">Sampul pembuka</span>
        <label class="inline-flex items-center gap-2 text-xs text-zinc-300 cursor-pointer">
          <input type="checkbox" bind:checked={settings.enabled} class="w-4 h-4 accent-white cursor-pointer" />
          Sertakan
        </label>
      </div>

      <!-- Logo, background and "supported by" are shared by both covers. -->
      {#if settings.enabled || settings.penutupEnabled}
        {#if settings.enabled}
        <div class="space-y-3">
          {#each TEXT_FIELDS as field (field.key)}
            <label class="block">
              <span class="text-xs text-zinc-400">{field.label}</span>
              <input
                type="text"
                value={settings[field.key] ?? ""}
                oninput={(e) => setText(field.key, (e.currentTarget as HTMLInputElement).value)}
                placeholder={placeholders[field.key] || "—"}
                class="mt-1 w-full bg-[#181a20] text-sm text-white px-2.5 py-1.5 rounded-lg border outline-none placeholder:text-zinc-500 select-text {settings[field.key] !== null ? 'border-amber-500/40' : 'border-white/[0.1] focus:border-white/30'}"
              />
            </label>
          {/each}
          <p class="text-[11px] text-zinc-500">Kosongkan kolom untuk memakai data event.</p>
        </div>
        {/if}

        <div>
          <span class="text-xs text-zinc-400">Logo</span>
          <div class="mt-1 grid grid-cols-3 gap-1 p-1 rounded-xl bg-[#181a20] border border-white/[0.08]">
            {#each LOGO_MODES as mode (mode.value)}
              <button
                type="button"
                onclick={() => (settings.logoMode = mode.value)}
                class="py-1.5 rounded-lg text-[11px] font-semibold transition {settings.logoMode === mode.value ? 'bg-white text-black' : 'text-zinc-400 hover:text-white hover:bg-white/[0.06]'}"
              >
                {mode.label}
              </button>
            {/each}
          </div>
          {#if settings.logoMode === "event" && !eventDefaults.hasEventLogo}
            <p class="mt-1.5 text-[11px] text-amber-300/80">Event ini belum punya logo kop surat.</p>
          {/if}
          {#if settings.logoMode === "custom"}
            <input
              type="file"
              accept="image/*"
              onchange={handleLogoFile}
              class="mt-2 w-full text-[11px] text-zinc-400 file:mr-2 file:px-2.5 file:py-1 file:rounded-lg file:border-0 file:bg-white/[0.08] file:text-zinc-200 file:text-[11px] file:font-semibold"
            />
          {/if}
        </div>

        <div>
          <span class="text-xs text-zinc-400">Gambar latar</span>
          <div class="mt-1 flex items-center gap-2">
            <span class="flex-1 text-xs text-zinc-300 truncate">
              {settings.customBackground ? "Gambar unggahan" : "Default (Ambrilindo)"}
            </span>
            {#if settings.customBackground}
              <button
                type="button"
                onclick={() => (settings.customBackground = null)}
                class="h-7 px-2.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] text-[11px] font-semibold rounded-lg hover:bg-white/[0.08]"
              >
                Pakai default
              </button>
            {/if}
          </div>
          <input
            type="file"
            accept="image/*"
            onchange={handleBackgroundFile}
            class="mt-2 w-full text-[11px] text-zinc-400 file:mr-2 file:px-2.5 file:py-1 file:rounded-lg file:border-0 file:bg-white/[0.08] file:text-zinc-200 file:text-[11px] file:font-semibold"
          />
          <p class="mt-1 text-[11px] text-zinc-500">Rasio 16:9, misalnya 2000 × 1125 px.</p>
        </div>

        <label class="flex items-center gap-2 text-xs text-zinc-300 cursor-pointer">
          <input type="checkbox" bind:checked={settings.showSupportedBy} class="w-4 h-4 accent-white cursor-pointer" />
          Tampilkan "Supported by"
        </label>

        {#if storageWarning}
          <p class="text-[11px] text-amber-300 bg-amber-500/10 border border-amber-500/20 rounded-lg px-2.5 py-2">
            Gambar terlalu besar untuk diingat. Sampul tetap dipakai sampai aplikasi ditutup.
          </p>
        {/if}
      {/if}
      {#if documentKind === "booklet_hasil" && (baganDays?.days.length ?? 0) > 0}
        <div class="pt-4 border-t border-white/[0.08] space-y-2">
          <div class="flex items-center justify-between">
            <span class="text-sm font-semibold text-zinc-200">Lampiran bagan</span>
            <label class="inline-flex items-center gap-2 text-xs text-zinc-300 cursor-pointer">
              <input
                type="checkbox"
                checked={!settings.lampiranDays}
                onchange={() => (settings.lampiranDays = settings.lampiranDays ? null : [])}
                class="w-4 h-4 accent-white cursor-pointer"
              />
              Semua hari
            </label>
          </div>
          <div class="space-y-1.5">
            {#each dayOptions as day (day.value)}
              <label class="flex items-center gap-2 text-xs text-zinc-300 cursor-pointer">
                <input type="checkbox" checked={isDayChecked(day.value)} onchange={() => toggleDay(day.value)} class="w-4 h-4 accent-white cursor-pointer" />
                {day.label}
              </label>
            {/each}
          </div>
          {#if settings.lampiranDays && settings.lampiranDays.length === 0}
            <p class="text-[11px] text-amber-300/80">Tidak ada hari dipilih: lampiran bagan kosong.</p>
          {/if}
        </div>
      {/if}

      {#if documentKind === "booklet_hasil"}
      <div class="pt-4 border-t border-white/[0.08] space-y-3">
          <div class="flex items-center justify-between">
            <span class="text-sm font-semibold text-zinc-200">Sampul penutup</span>
            <label class="inline-flex items-center gap-2 text-xs text-zinc-300 cursor-pointer">
              <input type="checkbox" bind:checked={settings.penutupEnabled} class="w-4 h-4 accent-white cursor-pointer" />
              Sertakan
            </label>
          </div>
          {#if settings.penutupEnabled}
            <label class="block">
              <span class="text-xs text-zinc-400">Judul</span>
              <input
                type="text"
                value={settings.penutupJudul ?? ""}
                oninput={(e) => (settings.penutupJudul = (e.currentTarget as HTMLInputElement).value || null)}
                placeholder={placeholders.penutupJudul}
                class="mt-1 w-full bg-[#181a20] text-sm text-white px-2.5 py-1.5 rounded-lg border outline-none placeholder:text-zinc-500 select-text {settings.penutupJudul !== null ? 'border-amber-500/40' : 'border-white/[0.1] focus:border-white/30'}"
              />
            </label>
            <label class="block">
              <span class="text-xs text-zinc-400">Ucapan</span>
              <textarea
                rows="3"
                value={settings.penutupPesan ?? ""}
                oninput={(e) => (settings.penutupPesan = (e.currentTarget as HTMLTextAreaElement).value || null)}
                placeholder={placeholders.penutupPesan}
                class="mt-1 w-full resize-none bg-[#181a20] text-sm text-white select-text px-2.5 py-1.5 rounded-lg border outline-none placeholder:text-zinc-500 {settings.penutupPesan !== null ? 'border-amber-500/40' : 'border-white/[0.1] focus:border-white/30'}"
              ></textarea>
            </label>
            <p class="text-[11px] text-zinc-500">Latar, logo, label, nama event dan tanggal mengikuti sampul pembuka.</p>
          {/if}
        </div>

        <div class="pt-4 border-t border-white/[0.08] space-y-3">
          <div class="flex items-center justify-between">
            <span class="text-sm font-semibold text-zinc-200">Tanda tangan</span>
            <label class="inline-flex items-center gap-2 text-xs text-zinc-300 cursor-pointer">
              <input type="checkbox" bind:checked={settings.ttdEnabled} class="w-4 h-4 accent-white cursor-pointer" />
              Sertakan
            </label>
          </div>
          {#if settings.ttdEnabled}
            <p class="text-[11px] text-zinc-500">Di bawah klasemen kontingen, klasemen perguruan, dan daftar juara.</p>
            <label class="block">
              <span class="text-xs text-zinc-400">Jabatan</span>
              <input
                type="text"
                value={settings.ttdJabatan ?? ""}
                oninput={(e) => (settings.ttdJabatan = (e.currentTarget as HTMLInputElement).value || null)}
                placeholder="Technical Delegate"
                class="mt-1 w-full bg-[#181a20] text-sm text-white px-2.5 py-1.5 rounded-lg border border-white/[0.1] focus:border-white/30 outline-none placeholder:text-zinc-500 select-text"
              />
            </label>
            <label class="block">
              <span class="text-xs text-zinc-400">Nama</span>
              <input
                type="text"
                value={settings.ttdNama ?? ""}
                oninput={(e) => (settings.ttdNama = (e.currentTarget as HTMLInputElement).value || null)}
                placeholder="Nama lengkap dan gelar"
                class="mt-1 w-full bg-[#181a20] text-sm text-white px-2.5 py-1.5 rounded-lg border border-white/[0.1] focus:border-white/30 outline-none placeholder:text-zinc-500 select-text"
              />
            </label>
            <label class="block">
              <span class="text-xs text-zinc-400">Tempat, tanggal (opsional)</span>
              <input
                type="text"
                value={settings.ttdTempatTanggal ?? ""}
                oninput={(e) => (settings.ttdTempatTanggal = (e.currentTarget as HTMLInputElement).value || null)}
                placeholder="Tana Paser, 21 November 2026"
                class="mt-1 w-full bg-[#181a20] text-sm text-white px-2.5 py-1.5 rounded-lg border border-white/[0.1] focus:border-white/30 outline-none placeholder:text-zinc-500 select-text"
              />
            </label>
            <div>
              <span class="text-xs text-zinc-400">Gambar tanda tangan</span>
              {#if settings.ttdImage}
                <div class="mt-1 flex items-center gap-2">
                  <div class="flex-1 h-16 rounded-lg bg-white flex items-center justify-center overflow-hidden">
                    <img src={settings.ttdImage} alt="Tanda tangan" class="max-h-14 max-w-full object-contain" />
                  </div>
                  <button
                    type="button"
                    onclick={() => (settings.ttdImage = null)}
                    class="h-9 px-3 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] transition"
                  >
                    Hapus
                  </button>
                </div>
              {/if}
              <label class="mt-1 h-9 px-3.5 inline-flex items-center bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] transition cursor-pointer">
                {settings.ttdImage ? "Ganti gambar" : "Unggah gambar"}
                <input type="file" accept="image/png,image/jpeg,image/webp" class="hidden" onchange={handleSignatureFile} />
              </label>
            </div>
          {/if}
        </div>
      {/if}

    </div>

    <div class="px-5 py-3 border-t border-white/[0.08] shrink-0">
      <button
        type="button"
        onclick={resetCover}
        class="w-full h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl hover:bg-white/[0.08] transition"
      >
        Kembalikan sampul ke default
      </button>
    </div>
  </aside>

  <!-- Live preview (the booklet's own toolbar holds Print and Save as PDF) -->
  <div class="flex-1 min-w-0 min-h-0">
    {#if documentKind === "booklet_hasil"}
      {#key bookletKey}
      <ResultsBookletView
        {eventId}
        {eventName}
        {hostAddress}
        coverSettings={settings}
        onCoverDefaults={(d) => (eventDefaults = d)}
      />
      {/key}
    {:else if documentKind === "roster_bagan"}
      {#if rosterDayId && rosterTatamiId}
        {#key bookletKey}
          <ResultsBookletView
            {eventId}
            {eventName}
            {hostAddress}
            coverSettings={settings}
            onCoverDefaults={(d) => (eventDefaults = d)}
            mode="roster_bagan"
            rosterSelection={{ dayId: rosterDayId, tatamiId: rosterTatamiId }}
          />
        {/key}
      {:else}
        <div class="h-full flex items-center justify-center text-center p-8">
          <div>
            <p class="text-sm font-semibold text-zinc-200">Belum ada jadwal</p>
            <p class="mt-1 text-xs text-zinc-500">Buat hari dan tatami di channel Jadwal dulu.</p>
          </div>
        </div>
      {/if}
    {/if}
  </div>
</div>
