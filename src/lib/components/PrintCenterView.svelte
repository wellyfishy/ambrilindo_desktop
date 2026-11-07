<!--
  #cetak channel: one place to preview and print documents. For now the only
  document is the results booklet, with a customizable cover.
-->
<script lang="ts">
  import ResultsBookletView from "$lib/components/ResultsBookletView.svelte";
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

  type DocumentKind = "booklet_hasil";
  let documentKind = $state<DocumentKind>("booklet_hasil");

  let settings = $state<CoverSettings>(defaultCoverSettings());
  let loadedFor = $state<number | null>(null);
  let storageWarning = $state(false);
  let eventDefaults = $state({ namaEvent: "", tempat: "", tanggalRaw: "", hasEventLogo: false });

  // Load the saved cover for the selected event.
  $effect(() => {
    const id = eventId;
    settings = loadCoverSettings(id);
    storageWarning = false;
    loadedFor = id;
  });

  // Remember edits for this event.
  $effect(() => {
    const snapshot = $state.snapshot(settings);
    if (loadedFor !== eventId) return;
    storageWarning = !saveCoverSettings(eventId, snapshot as CoverSettings);
  });

  let placeholders = $derived(resolveCoverText(defaultCoverSettings(), eventDefaults));

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

  function resetCover() {
    clearCoverSettings(eventId);
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
      <h2 class="text-base font-black text-white tracking-tight uppercase">Cetak</h2>
      <label class="block mt-3">
        <span class="text-[10px] font-bold tracking-wider uppercase text-zinc-400">Dokumen</span>
        <select
          bind:value={documentKind}
          class="mt-1.5 w-full bg-[#181a20] text-xs text-white px-2.5 py-2 rounded-xl border border-white/[0.1] focus:border-white/30 outline-none"
        >
          <option value="booklet_hasil">Booklet Hasil Turnamen</option>
        </select>
      </label>
    </div>

    <div class="flex-1 overflow-y-auto px-5 py-4 space-y-5 scrollbar-thin">
      <div class="flex items-center justify-between">
        <span class="text-[10px] font-bold tracking-wider uppercase text-zinc-400">Sampul</span>
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
              <span class="text-[11px] text-zinc-400">{field.label}</span>
              <input
                type="text"
                value={settings[field.key] ?? ""}
                oninput={(e) => setText(field.key, (e.currentTarget as HTMLInputElement).value)}
                placeholder={placeholders[field.key] || "—"}
                class="mt-1 w-full bg-[#181a20] text-xs text-white px-2.5 py-1.5 rounded-lg border outline-none placeholder:text-zinc-500 {settings[field.key] !== null ? 'border-amber-500/40' : 'border-white/[0.1] focus:border-white/30'}"
              />
            </label>
          {/each}
          <p class="text-[10px] text-zinc-500">Kosongkan kolom untuk memakai data event.</p>
        </div>
        {/if}

        <div>
          <span class="text-[11px] text-zinc-400">Logo</span>
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
            <p class="mt-1.5 text-[10px] text-amber-300/80">Event ini belum punya logo kop surat.</p>
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
          <span class="text-[11px] text-zinc-400">Gambar latar</span>
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
          <p class="mt-1 text-[10px] text-zinc-500">Rasio 16:9, misalnya 2000 × 1125 px.</p>
        </div>

        <label class="flex items-center gap-2 text-xs text-zinc-300 cursor-pointer">
          <input type="checkbox" bind:checked={settings.showSupportedBy} class="w-4 h-4 accent-white cursor-pointer" />
          Tampilkan "SUPPORTED BY"
        </label>

        {#if storageWarning}
          <p class="text-[11px] text-amber-300 bg-amber-500/10 border border-amber-500/20 rounded-lg px-2.5 py-2">
            Gambar terlalu besar untuk diingat. Sampul tetap dipakai sampai aplikasi ditutup.
          </p>
        {/if}
      {/if}
      <div class="pt-4 border-t border-white/[0.08] space-y-3">
          <div class="flex items-center justify-between">
            <span class="text-[10px] font-bold tracking-wider uppercase text-zinc-400">Sampul penutup</span>
            <label class="inline-flex items-center gap-2 text-xs text-zinc-300 cursor-pointer">
              <input type="checkbox" bind:checked={settings.penutupEnabled} class="w-4 h-4 accent-white cursor-pointer" />
              Sertakan
            </label>
          </div>
          {#if settings.penutupEnabled}
            <label class="block">
              <span class="text-[11px] text-zinc-400">Judul</span>
              <input
                type="text"
                value={settings.penutupJudul ?? ""}
                oninput={(e) => (settings.penutupJudul = (e.currentTarget as HTMLInputElement).value || null)}
                placeholder={placeholders.penutupJudul}
                class="mt-1 w-full bg-[#181a20] text-xs text-white px-2.5 py-1.5 rounded-lg border outline-none placeholder:text-zinc-500 {settings.penutupJudul !== null ? 'border-amber-500/40' : 'border-white/[0.1] focus:border-white/30'}"
              />
            </label>
            <label class="block">
              <span class="text-[11px] text-zinc-400">Ucapan</span>
              <textarea
                rows="3"
                value={settings.penutupPesan ?? ""}
                oninput={(e) => (settings.penutupPesan = (e.currentTarget as HTMLTextAreaElement).value || null)}
                placeholder={placeholders.penutupPesan}
                class="mt-1 w-full resize-none bg-[#181a20] text-xs text-white px-2.5 py-1.5 rounded-lg border outline-none placeholder:text-zinc-500 {settings.penutupPesan !== null ? 'border-amber-500/40' : 'border-white/[0.1] focus:border-white/30'}"
              ></textarea>
            </label>
            <p class="text-[10px] text-zinc-500">Latar, logo, label, nama event dan tanggal mengikuti sampul pembuka.</p>
          {/if}
        </div>

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
      <ResultsBookletView
        {eventId}
        {eventName}
        {hostAddress}
        coverSettings={settings}
        onCoverDefaults={(d) => (eventDefaults = d)}
      />
    {/if}
  </div>
</div>
