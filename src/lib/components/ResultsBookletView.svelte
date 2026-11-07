<script lang="ts">
  import { apiCall, getConnectionConfig } from "$lib/api_client";
  import type { ResultsBookletData, ResultsBookletSignatory, TatamiBookletCategory, DetailBaganItem } from "$lib/types";
  import { printWithOrientation } from "$lib/print_helper";
  import { resolveMediaLogos } from "$lib/media_helper";
  import { FORKI_LOGO_DATA_URL, AMBRILINDO_LOGO_DATA_URL } from "$lib/assets_base64";
  import DaftarJuaraPrintSheet from "$lib/components/DaftarJuaraPrintSheet.svelte";
  import KlasemenMedaliPrintSheet from "$lib/components/KlasemenMedaliPrintSheet.svelte";
  import ResultCoverSheet from "$lib/components/ResultCoverSheet.svelte";
  import { loadCoverSettings, resolveCoverText, type CoverSettings } from "$lib/cover_settings";

  interface Props {
    eventId: number;
    eventName?: string;
    hostAddress?: string;
    /** Cover settings being edited live (from the #cetak channel). */
    coverSettings?: CoverSettings | null;
    /** Reports the event data the cover falls back to (for form placeholders). */
    onCoverDefaults?: (d: { namaEvent: string; tempat: string; tanggalRaw: string; hasEventLogo: boolean }) => void;
  }

  let { eventId, eventName = "", hostAddress = "", coverSettings = null, onCoverDefaults }: Props = $props();


  let bookletData = $state<ResultsBookletData | null>(null);
  let isLoading = $state(true);
  let errorMessage = $state<string | null>(null);
  let isEditing = $state(false);
  let logoMap = $state<Record<string, string>>({});

  // Print Mode & In-App View Filter state
  let printMode = $state<"combined" | "portrait" | "landscape">("combined");
  let viewFilter = $state<"all" | "portrait" | "landscape">("all");
  let zoomLevel = $state<"100" | "85" | "fit">("100");
  let bookletPruneEmptyRounds = $state(true);
  let isJumpMenuOpen = $state(false);
  let isExportingPdf = $state(false);
  let exportSuccessMessage = $state<string | null>(null);

  // Editable local overrides for sign-off & header
  let editableOrganisasi = $state("FEDERASI OLAHRAGA KARATE-DO INDONESIA KALIMANTAN TIMUR");
  let editableEventName = $state("");
  let editableTempat = $state("");
  let editableTanggal = $state("");
  let editableKota = $state("");
  let editableNomorSurat = $state("");
  let editableCatatanSanksi = $state("");
  let editableSignatories = $state<ResultsBookletSignatory[]>([]);

  // Cover: settings come from the #cetak channel when embedded there; otherwise the
  // ones saved for this event (or the defaults) are used.
  let savedCoverSettings = $derived(loadCoverSettings(eventId));
  let cover = $derived<CoverSettings>(coverSettings ?? savedCoverSettings);
  let coverText = $derived(
    resolveCoverText(cover, {
      namaEvent: editableEventName || bookletData?.event_info?.nama_event || eventName,
      tempat: editableTempat,
      tanggalRaw: editableTanggal,
    })
  );
  $effect(() => {
    if (!bookletData || !onCoverDefaults) return;
    onCoverDefaults({
      namaEvent: editableEventName || bookletData.event_info?.nama_event || eventName,
      tempat: editableTempat,
      tanggalRaw: editableTanggal,
      hasEventLogo: !!bookletData.event_info?.logo,
    });
  });

  let coverLogoUrl = $derived.by(() => {
    if (cover.logoMode === "none") return null;
    if (cover.logoMode === "custom") return cover.customLogo;
    const path = bookletData?.event_info?.logo;
    return path ? logoMap[path] || null : null;
  });

  let topKontingen = $derived(bookletData?.standings_kontingen?.[0]);
  let topPerguruan = $derived(bookletData?.standings_perguruan?.[0]);
  let totalKategori = $derived(bookletData?.category_winners?.length || 0);
  let top3Kontingen = $derived(bookletData?.standings_kontingen?.slice(0, 3) || []);
  let top3Perguruan = $derived(bookletData?.standings_perguruan?.slice(0, 3) || []);
  let totalMedaliEmas = $derived(bookletData?.standings_kontingen?.reduce((acc, k) => acc + (k.gold || 0), 0) || 0);
  let totalMedaliPerak = $derived(bookletData?.standings_kontingen?.reduce((acc, k) => acc + (k.silver || 0), 0) || 0);
  let totalMedaliPerunggu = $derived(bookletData?.standings_kontingen?.reduce((acc, k) => acc + (k.bronze || 0), 0) || 0);

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

  

  function printCombined() {
    printMode = "combined";
    setTimeout(() => {
      printWithOrientation("combined", "8mm");
    }, 80);
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

  /**
   * The exported HTML is rendered from a temp file by headless Edge, so font files
   * referenced by URL (the cover's Oswald/Roboto/Inter) would not load there.
   * Inline every @font-face source as a data URL.
   */
  async function inlineFontFaces(css: string): Promise<string> {
    const urlRe = /url\((["']?)([^"')]+)\1\)/g;
    const urls = new Set<string>();
    for (const block of css.match(/@font-face\s*{[^}]*}/g) || []) {
      for (const m of block.matchAll(urlRe)) {
        if (!m[2].startsWith("data:")) urls.add(m[2]);
      }
    }
    const inlined: Record<string, string> = {};
    await Promise.all(
      [...urls].map(async (u) => {
        try {
          const res = await fetch(new URL(u, document.baseURI).href);
          if (!res.ok) return;
          const blob = await res.blob();
          inlined[u] = await new Promise<string>((resolve) => {
            const reader = new FileReader();
            reader.onload = () => resolve(String(reader.result));
            reader.readAsDataURL(blob);
          });
        } catch (_) {}
      })
    );
    return css.replace(/@font-face\s*{[^}]*}/g, (block) =>
      block.replace(urlRe, (full, q, u) => (inlined[u] ? `url("${inlined[u]}")` : full))
    );
  }

  async function preparePrintHtml(mode: "combined" | "portrait" | "landscape"): Promise<string> {
    const container = document.querySelector(".booklet-workspace-container");
    if (!container) return "";
    const cloned = container.cloneNode(true) as HTMLElement;
    cloned.querySelectorAll(".no-print").forEach((el) => el.remove());

    // In-memory Canvas inliner: ensures any non-data-URL images are serialized to base64
    if (typeof document !== "undefined") {
      const origImgs = container.querySelectorAll("img");
      const cloneImgs = cloned.querySelectorAll("img");
      for (let i = 0; i < origImgs.length; i++) {
        const orig = origImgs[i] as HTMLImageElement;
        const clone = cloneImgs[i] as HTMLImageElement;
        if (clone && !clone.src.startsWith("data:") && orig.naturalWidth > 0 && orig.naturalHeight > 0) {
          try {
            const canvas = document.createElement("canvas");
            canvas.width = orig.naturalWidth;
            canvas.height = orig.naturalHeight;
            const ctx = canvas.getContext("2d");
            if (ctx) {
              ctx.drawImage(orig, 0, 0);
              // Photos (e.g. the cover background) as JPEG: a 2000 px PNG is ~4 MB.
              const isLargePhoto = orig.naturalWidth * orig.naturalHeight > 1_000_000;
              clone.src = isLargePhoto ? canvas.toDataURL("image/jpeg", 0.9) : canvas.toDataURL("image/png");
            }
          } catch (_) {}
        }
      }
    }

    // Extract compiled CSS rules from document.styleSheets (Tailwind + component styles)
    let inlinedCss = "";
    if (typeof document !== "undefined") {
      try {
        for (const sheet of Array.from(document.styleSheets)) {
          try {
            const rules = sheet.cssRules || sheet.rules;
            if (!rules) continue;
            for (const rule of Array.from(rules)) {
              // Filter out rogue or un-named @page rules that might conflict with our named pages
              if (rule.type === CSSRule.PAGE_RULE || rule.cssText.startsWith("@page")) {
                continue;
              }
              inlinedCss += rule.cssText + "\n";
            }
          } catch (_) {}
        }
      } catch (_) {}
    }

    inlinedCss = await inlineFontFaces(inlinedCss);

    return `<!DOCTYPE html>
<html>
<head>
  <meta charset="utf-8">
  <title>${editableEventName || "Buku Hasil Turnamen"}</title>
  <style>
${inlinedCss}

    @page {
      margin: 0;
    }
    @page cover-sheet {
      size: 297mm 167.0625mm;
      margin: 0;
    }
    .booklet-page-cover {
      page: cover-sheet !important;
      break-after: page !important;
      page-break-after: always !important;
      display: block !important;
      width: 100% !important;
      margin: 0 !important;
      padding: 0 !important;
      /* The 16:9 sheet is 167.06 mm tall; a hair over the page height spills a blank page. */
      height: 167mm !important;
      max-height: 167mm !important;
      overflow: hidden !important;
    }
    .paper-sheet-cover {
      width: 100% !important;
      max-width: none !important;
      box-shadow: none !important;
      margin: 0 !important;
    }
    @page portrait-sheet {
      size: A4 portrait;
      margin: 8mm;
    }
    @page landscape-sheet {
      size: A4 landscape;
      margin: 6mm;
    }
    body {
      background: #ffffff !important;
      color: #000000 !important;
      margin: 0 !important;
      padding: 0 !important;
      font-family: system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
      -webkit-print-color-adjust: exact !important;
      print-color-adjust: exact !important;
    }
    img {
      max-width: 100%;
    }
    .booklet-page {
      box-shadow: none !important;
      border: none !important;
      margin: 0 !important;
      border-radius: 0 !important;
      width: 100% !important;
    }
    .booklet-page-portrait, .paper-sheet-portrait {
      page: portrait-sheet !important;
      break-after: page !important;
      page-break-after: always !important;
      width: 100% !important;
      min-height: 275mm !important;
      box-shadow: none !important;
      border: none !important;
      margin: 0 !important;
      padding: 0 !important;
    }
    /* Prevent double page breaks on nested paper-sheet-portrait */
    .booklet-page-portrait .paper-sheet-portrait {
      break-after: auto !important;
      page-break-after: auto !important;
      min-height: 0 !important;
      width: 100% !important;
      max-width: 210mm !important;
      box-shadow: none !important;
      border: none !important;
      margin: 0 !important;
      padding: 0 !important;
    }
    /* Full-page sheets (Cover and Lembar Penutup) must fill 280mm exact printable A4 height */
    .sheet-full-portrait {
      height: 280mm !important;
      min-height: 280mm !important;
      max-height: 280mm !important;
      box-sizing: border-box !important;
    }
    .booklet-page-landscape {
      page: landscape-sheet !important;
      break-after: page !important;
      page-break-after: always !important;
      width: 100% !important;
      min-height: 195mm !important;
      box-shadow: none !important;
      border: none !important;
      margin: 0 !important;
      padding: 0 !important;
    }
    /* Prevent double page breaks on nested paper-sheet-landscape */
    .booklet-page-landscape .paper-sheet-landscape {
      break-after: auto !important;
      page-break-after: auto !important;
      width: 100% !important;
      max-width: 285mm !important;
      box-shadow: none !important;
      border: none !important;
      margin: 0 !important;
      padding: 0 !important;
    }
    ${mode === "portrait" ? ".booklet-section-landscape, .booklet-page-landscape, .paper-sheet-landscape { display: none !important; }" : ""}
    ${mode === "landscape" ? ".booklet-section-portrait, .booklet-page-portrait, .paper-sheet-portrait, .booklet-page-cover { display: none !important; }" : ""}
  </style>
</head>
<body class="bg-white text-black">
  <main class="booklet-workspace-container ${mode === 'portrait' ? 'is-printing-portrait' : mode === 'landscape' ? 'is-printing-landscape' : 'is-printing-combined'}">
    ${cloned.innerHTML}
  </main>
</body>
</html>`;
  }

  async function handleExportPdf() {
    if (!bookletData) return;
    isExportingPdf = true;
    exportSuccessMessage = null;

    try {
      const mode = viewFilter === "portrait" ? "portrait" : viewFilter === "landscape" ? "landscape" : "combined";
      const htmlContent = await preparePrintHtml(mode);
      const cleanEventName = (editableEventName || "Turnamen").replace(/[^a-zA-Z0-9]/g, "_");
      const suggestedFilename = `Buku_Hasil_${cleanEventName}_${new Date().toISOString().slice(0, 10)}.pdf`;

      let isTauri = false;
      try {
        isTauri = typeof window !== "undefined" && Boolean((window as any).__TAURI_INTERNALS__);
      } catch (_) {}

      if (isTauri) {
        const savedPath = await apiCall<string | null>("export_html_to_pdf", {
          htmlContent,
          html_content: htmlContent,
          suggestedFilename,
          suggested_filename: suggestedFilename,
        });

        if (savedPath) {
          exportSuccessMessage = `PDF berhasil disimpan: ${savedPath}`;
          setTimeout(() => { exportSuccessMessage = null; }, 6000);
        }
      } else {
        const host = hostAddress || getConnectionConfig().hostUrl || window.location.origin;
        const res = await fetch(`${host.replace(/\/$/, "")}/api/export/html-to-pdf`, {
          method: "POST",
          headers: { "Content-Type": "application/json" },
          body: JSON.stringify({ html_content: htmlContent, filename: suggestedFilename }),
        });

        if (!res.ok) {
          throw new Error(`Server returned ${res.status}: ${await res.text()}`);
        }

        const blob = await res.blob();
        const url = URL.createObjectURL(blob);
        const a = document.createElement("a");
        a.href = url;
        a.download = suggestedFilename;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        URL.revokeObjectURL(url);
        exportSuccessMessage = `PDF berhasil diunduh: ${suggestedFilename}`;
        setTimeout(() => { exportSuccessMessage = null; }, 6000);
      }
    } catch (err: any) {
      console.error("Gagal export PDF:", err);
      alert("Gagal menyimpan PDF: " + (err?.message || err));
    } finally {
      isExportingPdf = false;
    }
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
  function getFestivalGroups(matches: DetailBaganItem[]) {
    const groups = [];
    for (let g = 1; g <= 6; g++) {
      const semi1 = matches.find((m) => m.round === 1 && m.urutan === 2 * g - 1) || null;
      const semi2 = matches.find((m) => m.round === 1 && m.urutan === 2 * g) || null;
      const finalMatch = matches.find((m) => m.round === 2 && m.urutan === g) || null;
      if (!semi1 && !semi2 && !finalMatch) continue;

      let emas: { name: string; contingent: string; logo: string | null } | null = null;
      let perak: { name: string; contingent: string; logo: string | null } | null = null;
      let perunggu1: { name: string; contingent: string; logo: string | null } | null = null;
      let perunggu2: { name: string; contingent: string; logo: string | null } | null = null;

      if (finalMatch && finalMatch.selesai && finalMatch.pemenang) {
        if (finalMatch.pemenang === "1") {
          emas = { name: finalMatch.atlet1_nama || "-", contingent: finalMatch.atlet1_kontingen || "-", logo: finalMatch.atlet1_logo || null };
          perak = { name: finalMatch.atlet2_nama || "-", contingent: finalMatch.atlet2_kontingen || "-", logo: finalMatch.atlet2_logo || null };
        } else if (finalMatch.pemenang === "2") {
          emas = { name: finalMatch.atlet2_nama || "-", contingent: finalMatch.atlet2_kontingen || "-", logo: finalMatch.atlet2_logo || null };
          perak = { name: finalMatch.atlet1_nama || "-", contingent: finalMatch.atlet1_kontingen || "-", logo: finalMatch.atlet1_logo || null };
        }
      }

      if (semi1 && semi1.selesai && semi1.pemenang) {
        const loserName = semi1.pemenang === "1" ? semi1.atlet2_nama : semi1.atlet1_nama;
        const loserKont = semi1.pemenang === "1" ? semi1.atlet2_kontingen : semi1.atlet1_kontingen;
        const loserLogo = semi1.pemenang === "1" ? semi1.atlet2_logo : semi1.atlet1_logo;
        if (loserName && !loserName.toUpperCase().includes("BYE")) {
          perunggu1 = { name: loserName, contingent: loserKont || "-", logo: loserLogo || null };
        }
      }

      if (semi2 && semi2.selesai && semi2.pemenang) {
        const loserName = semi2.pemenang === "1" ? semi2.atlet2_nama : semi2.atlet1_nama;
        const loserKont = semi2.pemenang === "1" ? semi2.atlet2_kontingen : semi2.atlet1_kontingen;
        const loserLogo = semi2.pemenang === "1" ? semi2.atlet2_logo : semi2.atlet1_logo;
        if (loserName && !loserName.toUpperCase().includes("BYE")) {
          perunggu2 = { name: loserName, contingent: loserKont || "-", logo: loserLogo || null };
        }
      }

      groups.push({ groupNum: g, semi1, semi2, finalMatch, podium: { emas, perak, perunggu1, perunggu2 } });
    }
    return groups;
  }
</script>



<svelte:head>
  <style>
    @page {
      margin: 0;
    }
    @page portrait-sheet {
      size: A4 portrait;
      margin: 8mm;
    }
    @page landscape-sheet {
      size: A4 landscape;
      margin: 6mm;
    }
    @page cover-sheet {
      size: 297mm 167.0625mm;
      margin: 0;
    }
    .booklet-page-cover {
      page: cover-sheet;
      break-after: page;
      page-break-after: always;
    }
    .booklet-page-portrait {
      page: portrait-sheet;
      break-after: page;
      page-break-after: always;
    }
    .booklet-page-landscape {
      page: landscape-sheet;
      break-after: page;
      page-break-after: always;
    }
  </style>
</svelte:head>

<div class="h-full flex flex-col bg-[#0b0d13] text-zinc-100 font-sans overflow-hidden select-none print-view-root {printMode === 'portrait' ? 'is-printing-portrait' : printMode === 'landscape' ? 'is-printing-landscape' : 'is-printing-combined'}">
  <!-- ================= IN-APP STICKY HEADER TOOLBAR ================= -->
  <header class="no-print bg-[#12151d] border-b border-white/[0.08] px-4 py-2.5 flex flex-wrap items-center justify-between gap-3 shrink-0 shadow-lg z-20">
    <!-- Left: Identity, Title & Jump Menu -->
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

      <!-- Quick Jump Navigator Dropdown -->
      <div class="relative ml-2">
        <button
          type="button"
          onclick={() => (isJumpMenuOpen = !isJumpMenuOpen)}
          class="px-2.5 py-1 rounded-lg bg-white/[0.05] hover:bg-white/[0.1] border border-white/[0.1] text-zinc-300 text-xs font-semibold flex items-center space-x-1.5 transition cursor-pointer"
        >
          <span>📑</span>
          <span>Lompat Halaman</span>
          <span class="text-[9px] text-zinc-500">▼</span>
        </button>

        {#if isJumpMenuOpen}
          <div
            class="absolute left-0 mt-1 w-64 bg-[#181a22] border border-white/[0.12] rounded-xl shadow-2xl py-1 z-50 text-xs text-zinc-200 divide-y divide-white/[0.06]"
          >
            <div class="py-1">
              <button
                type="button"
                onclick={() => { scrollToSection('page-cover'); isJumpMenuOpen = false; }}
                class="w-full text-left px-3 py-1.5 hover:bg-white/[0.08] flex items-center justify-between cursor-pointer"
              >
                <span>Halaman 1: Sampul Depan Resmi</span>
                <span class="text-[9px] font-mono text-amber-400/80">PORTRAIT</span>
              </button>
              <button
                type="button"
                onclick={() => { scrollToSection('page-klasemen'); isJumpMenuOpen = false; }}
                class="w-full text-left px-3 py-1.5 hover:bg-white/[0.08] flex items-center justify-between cursor-pointer"
              >
                <span>Halaman 2: Rekapitulasi Medali</span>
                <span class="text-[9px] font-mono text-amber-400/80">PORTRAIT</span>
              </button>
              <button
                type="button"
                onclick={() => { scrollToSection('page-daftar-juara'); isJumpMenuOpen = false; }}
                class="w-full text-left px-3 py-1.5 hover:bg-white/[0.08] flex items-center justify-between cursor-pointer"
              >
                <span>Halaman 3: Rekapan Podium Juara</span>
                <span class="text-[9px] font-mono text-amber-400/80">PORTRAIT</span>
              </button>
              <button
                type="button"
                onclick={() => { scrollToSection('page-back-cover'); isJumpMenuOpen = false; }}
                class="w-full text-left px-3 py-1.5 hover:bg-white/[0.08] flex items-center justify-between cursor-pointer"
              >
                <span>Halaman 4: Sampul Penutup</span>
                <span class="text-[9px] font-mono text-amber-400/80">16:9</span>
              </button>
            </div>
            {#if activeBrackets.length > 0}
              <div class="py-1">
                <button
                  type="button"
                  onclick={() => { scrollToSection('page-bagan-' + activeBrackets[0].category_id); isJumpMenuOpen = false; }}
                  class="w-full text-left px-3 py-1.5 hover:bg-white/[0.08] flex items-center justify-between cursor-pointer text-sky-300 font-semibold"
                >
                  <span>Lampiran Bagan ({activeBrackets.length} Kategori)</span>
                  <span class="text-[9px] font-mono text-sky-400">LANDSCAPE</span>
                </button>
              </div>
            {/if}
          </div>
        {/if}
      </div>
    </div>

    <!-- Center: View Filter Tabs & Zoom Controls -->
    <div class="flex items-center space-x-2">
      <!-- Section Filters -->
      <div class="flex items-center bg-[#181a22] p-1 rounded-xl border border-white/[0.08] text-xs">
        <button
          type="button"
          onclick={() => (viewFilter = "all")}
          class="px-3 py-1 rounded-lg font-bold transition cursor-pointer {viewFilter === 'all' ? 'bg-white text-slate-950 shadow-sm' : 'text-zinc-400 hover:text-white hover:bg-white/[0.06]'}"
          title="Tampilkan seluruh lembar gabungan (Portrait + Landscape berurutan)"
        >
          📖 Semua Lembar (Gabungan)
        </button>
        <button
          type="button"
          onclick={() => (viewFilter = "portrait")}
          class="px-2.5 py-1 rounded-lg font-bold transition flex items-center space-x-1.5 cursor-pointer {viewFilter === 'portrait' ? 'bg-amber-500 text-slate-950 shadow-sm' : 'text-zinc-400 hover:text-amber-300 hover:bg-white/[0.06]'}"
          title="Tampilkan hanya lembar dokumen berita acara resmi (A4 Portrait)"
        >
          <span>📄</span>
          <span>Dokumen (Portrait)</span>
        </button>
        <button
          type="button"
          onclick={() => (viewFilter = "landscape")}
          class="px-2.5 py-1 rounded-lg font-bold transition flex items-center space-x-1.5 cursor-pointer {viewFilter === 'landscape' ? 'bg-sky-500 text-white shadow-sm' : 'text-zinc-400 hover:text-sky-300 hover:bg-white/[0.06]'}"
          title="Tampilkan hanya lembar lampiran bagan pertandingan (A4 Landscape)"
        >
          <span>🌿</span>
          <span>Bagan (Landscape)</span>
        </button>
      </div>

      <!-- Zoom Level Selector -->
      <div class="flex items-center bg-[#181a22] p-1 rounded-xl border border-white/[0.08] text-[11px] font-mono text-zinc-400">
        <button
          type="button"
          onclick={() => (zoomLevel = "100")}
          class="px-2 py-0.5 rounded font-bold transition cursor-pointer {zoomLevel === '100' ? 'bg-white/[0.15] text-white' : 'hover:text-white'}"
          title="Ukuran Asli 100%"
        >
          100%
        </button>
        <button
          type="button"
          onclick={() => (zoomLevel = "85")}
          class="px-2 py-0.5 rounded font-bold transition cursor-pointer {zoomLevel === '85' ? 'bg-white/[0.15] text-white' : 'hover:text-white'}"
          title="Skala Nyaman 85%"
        >
          85%
        </button>
        <button
          type="button"
          onclick={() => (zoomLevel = "fit")}
          class="px-2 py-0.5 rounded font-bold transition cursor-pointer {zoomLevel === 'fit' ? 'bg-white/[0.15] text-white' : 'hover:text-white'}"
          title="Sesuaikan Lebar Layar (Fit Width)"
        >
          Fit
        </button>
      </div>
    </div>

    <!-- Right: Edit Toggle, Primary Print & Direct PDF Actions -->
    <div class="flex items-center space-x-2 shrink-0">
      <!-- EXPORT ACTION 1: 1-CLICK SAVE AS PDF (.pdf) -->
      <button
        type="button"
        onclick={handleExportPdf}
        disabled={isExportingPdf}
        class="px-3.5 py-1.5 rounded-xl bg-emerald-600 hover:bg-emerald-500 disabled:opacity-50 text-white text-xs font-black tracking-wide shadow-md transition flex items-center space-x-1.5 active:scale-95 cursor-pointer"
        title="Simpan dokumen buku hasil langsung ke file PDF (.pdf) dengan orientasi gabungan Portrait dan Landscape"
      >
        {#if isExportingPdf}
          <svg class="w-3.5 h-3.5 text-white animate-spin" fill="none" viewBox="0 0 24 24">
            <circle class="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" stroke-width="4"></circle>
            <path class="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8v8H4z"></path>
          </svg>
          <span>Menyimpan PDF...</span>
        {:else}
          <svg class="w-3.5 h-3.5 text-white" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 10v6m0 0l-3-3m3 3l3-3m2 8H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z" />
          </svg>
          <span>📥 Simpan PDF (.pdf)</span>
        {/if}
      </button>

      <!-- PRINT ACTION: UNIFIED PRINT DIALOG (PORTRAIT + LANDSCAPE) -->
      <button
        type="button"
        onclick={printCombined}
        class="px-4 py-1.5 rounded-xl bg-amber-500 hover:bg-amber-400 text-slate-950 text-xs font-black tracking-wide shadow-md transition flex items-center space-x-1.5 active:scale-95 cursor-pointer"
        title="Buka dialog cetak Windows / WebView2 untuk mencetak seluruh buku (Portrait & Landscape otomatis)"
      >
        <svg class="w-3.5 h-3.5 text-slate-950" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
        </svg>
        <span>🖨️ Cetak Buku (Dialog)</span>
      </button>
    </div>
  </header>

  {#if exportSuccessMessage}
    <div class="no-print bg-emerald-950/90 border-b border-emerald-500/40 px-4 py-2 flex items-center justify-between text-xs text-emerald-200 shadow-md">
      <div class="flex items-center space-x-2">
        <span>✅</span>
        <span class="font-bold">{exportSuccessMessage}</span>
      </div>
      <button type="button" onclick={() => (exportSuccessMessage = null)} class="text-emerald-400 hover:text-white cursor-pointer">✕</button>
    </div>
  {/if}


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
      <div class="booklet-zoom-wrapper transition-transform duration-200 {zoomLevel === 'fit' ? 'w-full' : ''}" style="{zoomLevel === '85' ? 'transform: scale(0.85); transform-origin: top center;' : ''}">
      <main class="booklet-workspace-container mx-auto">

        <!-- ========================================================================= -->
        <!-- SEKSI 1: DOKUMEN RESMI BERITA ACARA (A4 PORTRAIT SHEET SIMULATION) -->
        <!-- ========================================================================= -->
        {#if viewFilter === "all" || viewFilter === "portrait"}
          <div class="booklet-section-portrait">

            <!-- ================= HALAMAN 1: SAMPUL (16:9 LANDSCAPE, see ResultCoverSheet) ================= -->
            {#if cover.enabled}
              <section id="page-cover" class="booklet-page booklet-page-cover">
                <div class="no-print max-w-[297mm] mx-auto mb-2 flex items-center justify-between px-1 text-[11px] font-mono text-zinc-400">
                  <span class="inline-flex items-center px-2 py-0.5 rounded bg-amber-500/10 text-amber-300 border border-amber-500/20 font-bold">
                    SAMPUL 16:9 (297 × 167 mm)
                  </span>
                  <span class="text-zinc-500">Halaman 1 • Sampul</span>
                </div>
                <div class="paper-sheet paper-sheet-cover">
                  <ResultCoverSheet
                    federasi={coverText.federasi}
                    badge={coverText.badge}
                    judul={coverText.judul}
                    namaEvent={coverText.namaEvent}
                    tempat={coverText.tempat}
                    tanggal={coverText.tanggal}
                    showSupportedBy={cover.showSupportedBy}
                    logoUrl={coverLogoUrl}
                    backgroundUrl={cover.customBackground}
                  />
                </div>
              </section>
            {/if}

<!-- ================= HALAMAN 2: KLASEMEN MEDALI (KONTINGEN & PERGURUAN) ================= -->
            <section id="page-klasemen" class="booklet-page booklet-page-portrait">
              <div class="no-print max-w-[210mm] mx-auto mb-2 flex items-center justify-between px-1 text-[11px] font-mono text-zinc-400">
                <span class="inline-flex items-center space-x-1.5 px-2 py-0.5 rounded bg-amber-500/10 text-amber-300 border border-amber-500/20 font-bold">
                  <span>📄</span> <span>A4 PORTRAIT (210 × 297 mm)</span>
                </span>
                <span class="text-zinc-500">Halaman 2 • Rekapitulasi Medali Kontingen & Perguruan</span>
              </div>
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
                  pageLabelKontingen="Halaman 2 • Rekapitulasi Medali Kontingen"
                  pageLabelPerguruan="Halaman 2B • Rekapitulasi Medali Perguruan"
                  {logoMap}
                />
              </div>
            </section>

            
<!-- ================= HALAMAN 3: DAFTAR JUARA SELURUH KATEGORI ================= -->
            <section id="page-daftar-juara" class="booklet-page booklet-page-portrait">
              <div class="no-print max-w-[210mm] mx-auto mb-2 flex items-center justify-between px-1 text-[11px] font-mono text-zinc-400">
                <span class="inline-flex items-center space-x-1.5 px-2 py-0.5 rounded bg-amber-500/10 text-amber-300 border border-amber-500/20 font-bold">
                  <span>📄</span> <span>A4 PORTRAIT (210 × 297 mm)</span>
                </span>
                <span class="text-zinc-500">Halaman 3 • Rekapitulasi Juara Seluruh Nomor Pertandingan</span>
              </div>
              <div class="paper-sheet paper-sheet-portrait">
                <DaftarJuaraPrintSheet
                  bagans={bookletData.category_winners}
                  eventName={editableEventName}
                  pageLabel="Halaman 3 • Rekapan Podium Seluruh Kategori"
                  {logoMap}
                />
              </div>
            </section>

            
<!-- ================= HALAMAN 4: SAMPUL PENUTUP (replaces the old closing sheet; same design as the front cover) ================= -->
            {#if cover.penutupEnabled}
              <section id="page-back-cover" class="booklet-page booklet-page-cover">
                <div class="no-print max-w-[297mm] mx-auto mb-2 flex items-center justify-between px-1 text-[11px] font-mono text-zinc-400">
                  <span class="inline-flex items-center px-2 py-0.5 rounded bg-amber-500/10 text-amber-300 border border-amber-500/20 font-bold">
                    SAMPUL 16:9 (297 × 167 mm)
                  </span>
                  <span class="text-zinc-500">Halaman 4 • Sampul penutup</span>
                </div>
                <div class="paper-sheet paper-sheet-cover">
                  <ResultCoverSheet
                    federasi={coverText.federasi}
                    badge={coverText.badge}
                    judul={coverText.penutupJudul}
                    namaEvent={coverText.namaEvent}
                    tempat={coverText.penutupPesan}
                    tanggal={coverText.tanggal}
                    showSupportedBy={cover.showSupportedBy}
                    logoUrl={coverLogoUrl}
                    backgroundUrl={cover.customBackground}
                  />
                </div>
              </section>
            {/if}

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
                  <div class="no-print max-w-[297mm] mx-auto mb-2 flex items-center justify-between px-1 text-[11px] font-mono text-zinc-400">
                    <span class="inline-flex items-center space-x-1.5 px-2 py-0.5 rounded bg-sky-500/10 text-sky-300 border border-sky-500/20 font-bold">
                      <span>🌿</span> <span>A4 LANDSCAPE (297 × 210 mm)</span>
                    </span>
                    <span class="text-zinc-500">Lampiran Bagan {bIdx + 1} dari {activeBrackets.length} • {bCat.category_name}</span>
                  </div>
                  <div class="paper-sheet paper-sheet-landscape">
                    <!-- 1. Header Lembar Bagan -->
                    <div class="border-b-2 border-slate-900 pb-2 mb-3 flex items-center justify-between">
                      <div class="flex items-center space-x-3 min-w-0">
                        <div class="flex items-center space-x-2 shrink-0 bg-white p-0.5 rounded">
                          <img
                            src={FORKI_LOGO_DATA_URL}
                            alt="Logo FORKI"
                            class="h-7 w-auto max-w-[32px] max-h-7 object-contain shrink-0" style="max-height: 28px; max-width: 32px; width: auto; height: auto;"
                          />
                          <div class="h-6 w-[1px] bg-slate-300 shrink-0"></div>
                          {#if bookletData.event_info.logo && logoMap[bookletData.event_info.logo]}
                            <img
                              src={logoMap[bookletData.event_info.logo]}
                              alt="Logo Event"
                              class="h-7 w-auto max-w-[70px] max-h-7 object-contain shrink-0" style="max-height: 28px; max-width: 70px; width: auto; height: auto;"
                              onerror={(e) => { (e.currentTarget as HTMLImageElement).src = FORKI_LOGO_DATA_URL; }}
                            />
                          {:else}
                            <img
                              src={AMBRILINDO_LOGO_DATA_URL}
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
                    {#if bCat.category_name.toUpperCase().includes("FESTIVAL") || (bCat.bagan_nama && bCat.bagan_nama.toUpperCase().includes("FESTIVAL"))}
                      {@const festGroups = getFestivalGroups(bCat.matches)}
                      {@const totalSlots = festGroups.length <= 4 ? 4 : 6}
                      <!-- Festival 4/6 Quadrant Grid in Booklet -->
                      <div class="festival-split-canvas flex-1 border-2 border-slate-900 rounded overflow-hidden my-1 flex flex-col justify-between">
                        <div class="grid grid-cols-2 divide-x-2 divide-y-2 divide-slate-900 h-full {totalSlots === 4 ? 'grid-rows-2' : 'grid-rows-3'}">
                          {#each Array.from({ length: totalSlots }) as _, idx}
                            {@const gNum = idx + 1}
                            {@const fg = festGroups.find((g) => g.groupNum === gNum)}
                            <div class="p-1.5 flex flex-col justify-between bg-white text-[9px] {totalSlots === 4 ? 'min-h-[145px]' : 'min-h-[95px]'}">
                              <!-- Group Title Bar -->
                              <div class="flex items-center justify-between pb-0.5 border-b border-slate-800 font-mono font-bold {fg ? '' : 'opacity-40'}">
                                <span>GRUP {gNum} (POOL {String.fromCharCode(64 + gNum)})</span>
                                {#if fg}
                                  <span class="text-[7.5px] px-1 rounded {fg.finalMatch?.selesai ? 'bg-amber-100 text-amber-900' : 'bg-emerald-100 text-emerald-900'}">
                                    {fg.finalMatch?.selesai ? 'SELESAI' : 'BERJALAN'}
                                  </span>
                                {:else}
                                  <span class="text-[7.5px] px-1 rounded bg-slate-100 text-slate-500 font-normal">
                                    KOSONG
                                  </span>
                                {/if}
                              </div>

                              {#if fg}
                                <div class="flex items-stretch justify-between gap-2 mt-1 flex-1">
                                  <!-- Semifinals -->
                                  <div class="w-[48%] flex flex-col justify-around space-y-1">
                                    {#if fg.semi1}
                                      <div class="border border-slate-400 rounded p-1 {fg.semi1.pemenang ? 'bg-slate-50' : ''}">
                                        <div class="text-[7px] text-slate-500 font-mono flex justify-between">
                                          <span>SEMI 1</span>
                                          <span>P-{fg.semi1.id}</span>
                                        </div>
                                        <div class="flex justify-between items-center text-[8px] {fg.semi1.pemenang === '1' ? 'font-black bg-amber-100' : ''}">
                                          <span class="truncate">{fg.semi1.atlet1_nama || 'BYE'}</span>
                                          <span class="font-mono ml-1">{fg.semi1.score1 ?? '-'}</span>
                                        </div>
                                        <div class="flex justify-between items-center text-[8px] {fg.semi1.pemenang === '2' ? 'font-black bg-amber-100' : ''}">
                                          <span class="truncate">{fg.semi1.atlet2_nama || 'BYE'}</span>
                                          <span class="font-mono ml-1">{fg.semi1.score2 ?? '-'}</span>
                                        </div>
                                      </div>
                                    {/if}
                                    {#if fg.semi2}
                                      <div class="border border-slate-400 rounded p-1 {fg.semi2.pemenang ? 'bg-slate-50' : ''}">
                                        <div class="text-[7px] text-slate-500 font-mono flex justify-between">
                                          <span>SEMI 2</span>
                                          <span>P-{fg.semi2.id}</span>
                                        </div>
                                        <div class="flex justify-between items-center text-[8px] {fg.semi2.pemenang === '1' ? 'font-black bg-amber-100' : ''}">
                                          <span class="truncate">{fg.semi2.atlet1_nama || 'BYE'}</span>
                                          <span class="font-mono ml-1">{fg.semi2.score1 ?? '-'}</span>
                                        </div>
                                        <div class="flex justify-between items-center text-[8px] {fg.semi2.pemenang === '2' ? 'font-black bg-amber-100' : ''}">
                                          <span class="truncate">{fg.semi2.atlet2_nama || 'BYE'}</span>
                                          <span class="font-mono ml-1">{fg.semi2.score2 ?? '-'}</span>
                                        </div>
                                      </div>
                                    {/if}
                                  </div>

                                  <!-- Final & Podium -->
                                  <div class="w-[50%] flex flex-col justify-between">
                                    {#if fg.finalMatch}
                                      <div class="border-1.5 border-amber-600 rounded p-1 bg-amber-50/20">
                                        <div class="text-[7px] text-amber-900 font-mono font-bold flex justify-between">
                                          <span>FINAL GRUP {gNum}</span>
                                          <span>P-{fg.finalMatch.id}</span>
                                        </div>
                                        <div class="flex justify-between items-center text-[8px] {fg.finalMatch.pemenang === '1' ? 'font-black bg-amber-200' : ''}">
                                          <span class="truncate">{fg.finalMatch.atlet1_nama || 'Pemenang S1'}</span>
                                          <span class="font-mono ml-1">{fg.finalMatch.score1 ?? '-'}</span>
                                        </div>
                                        <div class="flex justify-between items-center text-[8px] {fg.finalMatch.pemenang === '2' ? 'font-black bg-amber-200' : ''}">
                                          <span class="truncate">{fg.finalMatch.atlet2_nama || 'Pemenang S2'}</span>
                                          <span class="font-mono ml-1">{fg.finalMatch.score2 ?? '-'}</span>
                                        </div>
                                      </div>
                                    {/if}

                                    <!-- Mini Podium Card -->
                                    <div class="border border-amber-500 rounded bg-white text-[7.5px] overflow-hidden mt-1">
                                      <div class="bg-amber-100 text-amber-950 font-bold px-1 py-0.2 uppercase text-[7px]">Hasil Juara</div>
                                      <div class="px-1 py-0.5 divide-y divide-slate-100">
                                        <div class="flex items-center gap-1"><span class="w-2.5 h-2.5 rounded-full bg-amber-400 text-black font-bold flex items-center justify-center text-[6px]">1</span><span class="truncate font-bold">{fg.podium.emas?.name || '—'}</span></div>
                                        <div class="flex items-center gap-1"><span class="w-2.5 h-2.5 rounded-full bg-slate-300 text-black font-bold flex items-center justify-center text-[6px]">2</span><span class="truncate">{fg.podium.perak?.name || '—'}</span></div>
                                        <div class="flex items-center gap-1"><span class="w-2.5 h-2.5 rounded-full bg-amber-700 text-white font-bold flex items-center justify-center text-[6px]">3</span><span class="truncate">{fg.podium.perunggu1?.name || '—'}</span></div>
                                        <div class="flex items-center gap-1"><span class="w-2.5 h-2.5 rounded-full bg-amber-700 text-white font-bold flex items-center justify-center text-[6px]">3</span><span class="truncate">{fg.podium.perunggu2?.name || '—'}</span></div>
                                      </div>
                                    </div>
                                  </div>
                                </div>
                              {:else}
                                <div class="flex-1 flex flex-col items-center justify-center p-3 text-center my-1 rounded border border-dashed border-slate-300 bg-slate-50/50">
                                  <div class="w-6 h-6 rounded-full border border-slate-400 text-slate-400 font-black text-[10px] flex items-center justify-center mb-1">
                                    {gNum}
                                  </div>
                                  <span class="text-[9px] font-mono font-black text-slate-400 tracking-wider">
                                    [GRUP {gNum} TIDAK DIGUNAKAN]
                                  </span>
                                  <span class="text-[7.5px] text-slate-400 mt-0.5">
                                    Peserta terakomodasi pada grup sebelumnya
                                  </span>
                                </div>
                              {/if}
                            </div>
                          {/each}
                        </div>
                      </div>
                    {:else}
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
                                        <img src={logoMap[m.atlet1_logo]} alt="" class="w-3.5 h-3.5 max-w-3.5 max-h-3.5 object-contain shrink-0" style="width: 14px; height: 14px; max-width: 14px; max-height: 14px; object-fit: contain;" width="14" height="14" />
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
                                        <img src={logoMap[m.atlet2_logo]} alt="" class="w-3.5 h-3.5 max-w-3.5 max-h-3.5 object-contain shrink-0" style="width: 14px; height: 14px; max-width: 14px; max-height: 14px; object-fit: contain;" width="14" height="14" />
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

                    {/if}

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
      </div>
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

  .paper-sheet-cover {
    width: 297mm;
    max-width: 100%;
    margin-left: auto;
    margin-right: auto;
    margin-bottom: 2.5rem;
    box-shadow: 0 12px 30px -4px rgba(0, 0, 0, 0.45);
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

    .booklet-zoom-wrapper {
      transform: none !important;
    }

    .booklet-workspace-container {
      max-width: 100% !important;
      padding: 0 !important;
      margin: 0 !important;
    }

    /* Print Mode 0: COMBINED (Default) - Shows both portrait and landscape */
    .is-printing-combined .booklet-section-portrait {
      display: block !important;
    }
    .is-printing-combined .booklet-page-portrait {
      display: flex !important;
      page: portrait-sheet !important;
      break-after: page !important;
      page-break-after: always !important;
    }

    .is-printing-combined .booklet-section-landscape {
      display: block !important;
    }
    .is-printing-combined .booklet-page-landscape {
      display: flex !important;
      page: landscape-sheet !important;
      break-after: page !important;
      page-break-after: always !important;
    }

    .booklet-page-cover {
      display: block !important;
      page: cover-sheet !important;
      break-after: page !important;
      page-break-after: always !important;
      /* The 16:9 sheet is 167.06 mm tall; a hair over the page height spills a blank page. */
      height: 167mm !important;
      max-height: 167mm !important;
      overflow: hidden !important;
    }
    .paper-sheet-cover {
      width: 100% !important;
      max-width: none !important;
      box-shadow: none !important;
      margin: 0 !important;
    }
    .is-printing-landscape .booklet-page-cover {
      display: none !important;
    }

    /* Print Mode 1: PORTRAIT ONLY */
    .is-printing-portrait .booklet-section-landscape,
    .is-printing-portrait .booklet-page-landscape {
      display: none !important;
    }
    .is-printing-portrait .booklet-section-portrait,
    .is-printing-portrait .booklet-page-portrait {
      display: flex !important;
    }

    /* Print Mode 2: LANDSCAPE ONLY */
    .is-printing-landscape .booklet-section-portrait,
    .is-printing-landscape .booklet-page-portrait {
      display: none !important;
    }
    .is-printing-landscape .booklet-section-landscape,
    .is-printing-landscape .booklet-page-landscape {
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
      box-shadow: none !important;
      border: none !important;
      margin: 0 !important;
      padding: 0 !important;
      break-after: auto !important;
      page-break-after: auto !important;
    }

    .paper-sheet-landscape {
      width: 100% !important;
      max-width: 285mm !important;
      min-height: 195mm !important;
      box-shadow: none !important;
      border: none !important;
      margin: 0 !important;
      padding: 0 !important;
      break-after: auto !important;
      page-break-after: auto !important;
    }
  }
</style>
