<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { DetailBaganItem, BaganItem } from "$lib/types";

  interface Props {
    baganId: number;
    baganName: string;
    baganInfo?: BaganItem | null;
    eventName?: string;
    eventVenue?: string;
    eventDate?: string;
    onDispatchToTatami?: (tatamiNum: number, matchId: number) => void;
  }

  let {
    baganId,
    baganName,
    baganInfo = null,
    eventName = "",
    eventVenue = "Venue Pertandingan",
    eventDate = "",
    onDispatchToTatami,
  }: Props = $props();

  let matches = $state<DetailBaganItem[]>([]);
  let isLoading = $state(true);

  // Zoom & Viewport adaptivity state
  let zoom = $state(1.0);
  let userHasZoomed = $state(false);
  let contentWidth = $state(1120);
  let contentHeight = $state(920);

  let viewportEl = $state<HTMLDivElement | null>(null);
  let svgEl = $state<SVGSVGElement | null>(null);
  let gridEl = $state<HTMLDivElement | null>(null);

  function updateContentDimensions() {
    if (gridEl) {
      contentWidth = Math.max(1080, gridEl.scrollWidth);
      contentHeight = Math.max(880, gridEl.scrollHeight);
    }
  }

  function zoomIn() {
    userHasZoomed = true;
    zoom = Math.min(2.5, Math.round((zoom + 0.15) * 100) / 100);
    tick().then(() => requestAnimationFrame(drawConnectors));
  }

  function zoomOut() {
    userHasZoomed = true;
    zoom = Math.max(0.5, Math.round((zoom - 0.15) * 100) / 100);
    tick().then(() => requestAnimationFrame(drawConnectors));
  }

  function resetZoom() {
    userHasZoomed = true;
    zoom = 1.0;
    tick().then(() => requestAnimationFrame(drawConnectors));
  }

  function fitToScreen() {
    if (!viewportEl || !gridEl) return;
    updateContentDimensions();
    const vpWidth = viewportEl.clientWidth - 48;
    const vpHeight = viewportEl.clientHeight - 48;

    if (vpWidth <= 0 || vpHeight <= 0 || contentWidth <= 0 || contentHeight <= 0) return;

    const scaleX = vpWidth / contentWidth;
    const scaleY = vpHeight / contentHeight;
    const optimalScale = Math.min(scaleX, scaleY);

    zoom = Math.max(0.6, Math.min(1.8, Math.round(optimalScale * 20) / 20));
    tick().then(() => requestAnimationFrame(drawConnectors));
  }

  function handleWheel(e: WheelEvent) {
    if (e.ctrlKey || e.metaKey) {
      e.preventDefault();
      if (e.deltaY < 0) {
        zoomIn();
      } else {
        zoomOut();
      }
    }
  }

  async function loadBracketData() {
    isLoading = true;
    try {
      const data = await invoke<DetailBaganItem[]>("get_detail_bagan", { baganId });
      matches = data;
    } catch (err) {
      console.error("Gagal memuat detail bagan:", err);
      matches = [];
    } finally {
      isLoading = false;
      await tick();
      requestAnimationFrame(() => {
        updateContentDimensions();
        drawConnectors();
        if (!userHasZoomed) {
          fitToScreen();
        }
      });
    }
  }

  // Group matches by round (1 = Babak 16 Besar, 2 = Perempat Final, 3 = Semi Final, 4 = Final)
  let rounds = $derived.by(() => {
    const map = new Map<number, DetailBaganItem[]>();
    for (const m of matches) {
      const r = m.round || 1;
      if (!map.has(r)) map.set(r, []);
      map.get(r)!.push(m);
    }
    // Sort matches within each round by urutan
    for (const [_, list] of map) {
      list.sort((a, b) => (a.urutan || a.id) - (b.urutan || b.id));
    }
    return Array.from(map.entries()).sort((a, b) => a[0] - b[0]);
  });

  // Main elimination tree rounds (1 to 4)
  let elimRounds = $derived(rounds.filter(([r]) => r <= 4));
  let elimMatchesCount = $derived(matches.filter((m) => (m.round || 1) <= 4).length);

  function getRoundBadge(roundNum: number, totalElimRounds: number): string {
    if (roundNum === 4) return "FN";
    if (roundNum === 3) return totalElimRounds >= 4 ? "SF" : "FN";
    if (roundNum === 2) return totalElimRounds >= 4 ? "R2" : "SF";
    return "R1";
  }

  function getRoundTitle(roundNum: number, totalElimRounds: number): string {
    if (roundNum === 4) return "PARTAI FINAL";
    if (roundNum === 3) return totalElimRounds >= 4 ? "SEMI FINAL" : "PARTAI FINAL";
    if (roundNum === 2) return totalElimRounds >= 4 ? "PEREMPAT FINAL" : "SEMI FINAL";
    return "BABAK 16 BESAR";
  }

  function getSlotCountForRound(roundNum: number): number {
    switch (roundNum) {
      case 1: return 8;
      case 2: return 4;
      case 3: return 2;
      case 4: return 1;
      default: return 1;
    }
  }

  function getSlotHeightClass(roundNum: number): string {
    switch (roundNum) {
      case 1: return "h-[108px]";
      case 2: return "h-[216px]";
      case 3: return "h-[432px]";
      case 4: return "h-[864px]";
      default: return "h-[108px]";
    }
  }

  function getMatchForSlot(roundMatches: DetailBaganItem[], slotNum: number): DetailBaganItem | undefined {
    return roundMatches.find((m) => (m.urutan || 0) === slotNum) || roundMatches[slotNum - 1];
  }

  // Calculate podium / Hasil Juara standings (Juara 1, 2, 3A, 3B for Juara 3 Bersama)
  let podium = $derived.by(() => {
    const finalMatch = matches.find((m) => m.round === 4) || elimRounds[elimRounds.length - 1]?.[1]?.[0];
    const semiMatches = matches
      .filter((m) => m.round === 3)
      .sort((a, b) => (a.urutan || 0) - (b.urutan || 0));
    const semi1 = semiMatches[0];
    const semi2 = semiMatches[1];

    let j1 = { name: "Belum Ditentukan", contingent: "—" };
    let j2 = { name: "Belum Ditentukan", contingent: "—" };
    let j3a = { name: "Belum Ditentukan", contingent: "—" };
    let j3b = { name: "Belum Ditentukan", contingent: "—" };

    if (finalMatch && finalMatch.selesai) {
      if (finalMatch.pemenang === "1") {
        j1 = {
          name: finalMatch.atlet1_nama || (finalMatch.atlet1_id ? `Atlet #${finalMatch.atlet1_id}` : "BYE"),
          contingent: finalMatch.atlet1_kontingen || "—",
        };
        j2 = {
          name: finalMatch.atlet2_nama || (finalMatch.atlet2_id ? `Atlet #${finalMatch.atlet2_id}` : "BYE"),
          contingent: finalMatch.atlet2_kontingen || "—",
        };
      } else if (finalMatch.pemenang === "2") {
        j1 = {
          name: finalMatch.atlet2_nama || (finalMatch.atlet2_id ? `Atlet #${finalMatch.atlet2_id}` : "BYE"),
          contingent: finalMatch.atlet2_kontingen || "—",
        };
        j2 = {
          name: finalMatch.atlet1_nama || (finalMatch.atlet1_id ? `Atlet #${finalMatch.atlet1_id}` : "BYE"),
          contingent: finalMatch.atlet1_kontingen || "—",
        };
      }
    }

    // Semifinal 1 loser -> Juara 3A
    if (semi1 && semi1.selesai) {
      if (semi1.pemenang === "1") {
        j3a = {
          name: semi1.atlet2_nama || (semi1.atlet2_id ? `Atlet #${semi1.atlet2_id}` : "BYE"),
          contingent: semi1.atlet2_kontingen || "—",
        };
      } else if (semi1.pemenang === "2") {
        j3a = {
          name: semi1.atlet1_nama || (semi1.atlet1_id ? `Atlet #${semi1.atlet1_id}` : "BYE"),
          contingent: semi1.atlet1_kontingen || "—",
        };
      }
    }

    // Semifinal 2 loser -> Juara 3B
    if (semi2 && semi2.selesai) {
      if (semi2.pemenang === "1") {
        j3b = {
          name: semi2.atlet2_nama || (semi2.atlet2_id ? `Atlet #${semi2.atlet2_id}` : "BYE"),
          contingent: semi2.atlet2_kontingen || "—",
        };
      } else if (semi2.pemenang === "2") {
        j3b = {
          name: semi2.atlet1_nama || (semi2.atlet1_id ? `Atlet #${semi2.atlet1_id}` : "BYE"),
          contingent: semi2.atlet1_kontingen || "—",
        };
      }
    }

    return { j1, j2, j3a, j3b };
  });

  function seg(svg: SVGSVGElement, x1: number, y1: number, x2: number, y2: number, color: string, width: string) {
    const l = document.createElementNS("http://www.w3.org/2000/svg", "line");
    l.setAttribute("x1", x1.toString());
    l.setAttribute("y1", y1.toString());
    l.setAttribute("x2", x2.toString());
    l.setAttribute("y2", y2.toString());
    l.setAttribute("stroke", color);
    l.setAttribute("stroke-width", width);
    l.setAttribute("stroke-linecap", "round");
    svg.appendChild(l);
  }

  function drawConnectors() {
    if (!svgEl || !gridEl) return;
    svgEl.innerHTML = "";

    const outer = svgEl.getBoundingClientRect();
    const z = zoom || 1;

    const localWidth = Math.round(outer.width / z);
    const localHeight = Math.round(outer.height / z);

    if (localWidth > 0 && localHeight > 0) {
      svgEl.setAttribute("width", localWidth.toString());
      svgEl.setAttribute("height", localHeight.toString());
      svgEl.setAttribute("viewBox", `0 0 ${localWidth} ${localHeight}`);
    }

    const cols = Array.from(gridEl.querySelectorAll<HTMLElement>(".round-col-elim"));
    if (cols.length < 2) return;

    // High contrast slate connectors matching legacy print/screen
    const strokeColor = "#475569";
    const strokeWidth = "1.5";

    for (let ri = 0; ri < cols.length - 1; ri++) {
      const fromCol = cols[ri];
      const toCol = cols[ri + 1];

      // Match each slot in target round
      const toSlots = Array.from(toCol.querySelectorAll<HTMLElement>("[data-slot]"));

      for (let i = 0; i < toSlots.length; i++) {
        const slotOut = toSlots[i];
        const cardOut = slotOut.querySelector<HTMLElement>(".match-card");
        if (!cardOut) continue;

        // Slot i (0-based) is fed by slots (2*i + 1) and (2*i + 2) in previous round
        const slotA = fromCol.querySelector<HTMLElement>(`[data-slot="${i * 2 + 1}"]`);
        const slotB = fromCol.querySelector<HTMLElement>(`[data-slot="${i * 2 + 2}"]`);

        const cardA = slotA?.querySelector<HTMLElement>(".match-card");
        const cardB = slotB?.querySelector<HTMLElement>(".match-card");

        const rOut = cardOut.getBoundingClientRect();
        const yTo = Math.round((rOut.top + rOut.height / 2 - outer.top) / z);
        const xTo = Math.round((rOut.left - outer.left) / z);

        if (cardA && cardB) {
          const rA = cardA.getBoundingClientRect();
          const rB = cardB.getBoundingClientRect();

          const yA = Math.round((rA.top + rA.height / 2 - outer.top) / z);
          const yB = Math.round((rB.top + rB.height / 2 - outer.top) / z);
          const xFromA = Math.round((rA.right - outer.left) / z);
          const xFromB = Math.round((rB.right - outer.left) / z);

          const maxXFrom = Math.max(xFromA, xFromB);
          const xMid = Math.round(maxXFrom + (xTo - maxXFrom) / 2);
          const yMid = Math.round((yA + yB) / 2);

          // 1. Horizontal from upper card A to xMid
          seg(svgEl, xFromA, yA, xMid, yA, strokeColor, strokeWidth);
          // 2. Horizontal from lower card B to xMid
          seg(svgEl, xFromB, yB, xMid, yB, strokeColor, strokeWidth);
          // 3. Vertical crossbar between yA and yB at xMid
          seg(svgEl, xMid, yA, xMid, yB, strokeColor, strokeWidth);
          // 4. Center horizontal branch into target card Out
          seg(svgEl, xMid, yTo, xTo, yTo, strokeColor, strokeWidth);
        } else if (cardA) {
          const rA = cardA.getBoundingClientRect();
          const yA = Math.round((rA.top + rA.height / 2 - outer.top) / z);
          const xFrom = Math.round((rA.right - outer.left) / z);
          const xMid = Math.round(xFrom + (xTo - xFrom) / 2);

          seg(svgEl, xFrom, yA, xMid, yA, strokeColor, strokeWidth);
          if (yA !== yTo) {
            seg(svgEl, xMid, yA, xMid, yTo, strokeColor, strokeWidth);
          }
          seg(svgEl, xMid, yTo, xTo, yTo, strokeColor, strokeWidth);
        } else if (cardB) {
          const rB = cardB.getBoundingClientRect();
          const yB = Math.round((rB.top + rB.height / 2 - outer.top) / z);
          const xFrom = Math.round((rB.right - outer.left) / z);
          const xMid = Math.round(xFrom + (xTo - xFrom) / 2);

          seg(svgEl, xFrom, yB, xMid, yB, strokeColor, strokeWidth);
          if (yB !== yTo) {
            seg(svgEl, xMid, yB, xMid, yTo, strokeColor, strokeWidth);
          }
          seg(svgEl, xMid, yTo, xTo, yTo, strokeColor, strokeWidth);
        }
      }
    }
  }

  $effect(() => {
    if (baganId) {
      loadBracketData();
    }
  });

  $effect(() => {
    if (matches.length > 0) {
      tick().then(() => {
        updateContentDimensions();
        requestAnimationFrame(drawConnectors);
      });
    }
  });

  onMount(() => {
    loadBracketData();

    let ro: ResizeObserver | null = null;
    if (typeof ResizeObserver !== "undefined") {
      ro = new ResizeObserver(() => {
        updateContentDimensions();
        requestAnimationFrame(drawConnectors);
      });
      if (gridEl) ro.observe(gridEl);
      if (viewportEl) ro.observe(viewportEl);
    }

    const handleWindowResize = () => {
      updateContentDimensions();
      requestAnimationFrame(drawConnectors);
    };
    window.addEventListener("resize", handleWindowResize);

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement) return;
      if ((e.ctrlKey || e.metaKey) && (e.key === "+" || e.key === "=")) {
        e.preventDefault();
        zoomIn();
      } else if ((e.ctrlKey || e.metaKey) && (e.key === "-" || e.key === "_")) {
        e.preventDefault();
        zoomOut();
      } else if ((e.ctrlKey || e.metaKey) && e.key === "0") {
        e.preventDefault();
        resetZoom();
      }
    };
    window.addEventListener("keydown", handleKeyDown);

    if (typeof document !== "undefined" && document.fonts && document.fonts.ready) {
      document.fonts.ready.then(() => {
        updateContentDimensions();
        requestAnimationFrame(drawConnectors);
      });
    }

    return () => {
      ro?.disconnect();
      window.removeEventListener("resize", handleWindowResize);
      window.removeEventListener("keydown", handleKeyDown);
    };
  });
</script>

<div class="h-full flex flex-col p-4 select-none bg-slate-100 text-slate-800 overflow-hidden font-sans print-sheet">
  <!-- Top Control Bar (Hidden on Print) -->
  <div class="no-print flex items-center justify-between pb-2.5 mb-2 border-b border-slate-300 shrink-0">
    <div class="flex items-center space-x-2">
      <span class="text-xs font-black text-slate-800 uppercase tracking-wider">
        Bagan Pertandingan
      </span>
      <span class="text-slate-300">•</span>
      <span class="text-xs text-slate-500 font-mono">
        {elimMatchesCount} Partai Terdaftar
      </span>
    </div>

    <!-- Zoom & Action Toolbar -->
    <div class="flex items-center space-x-2">
      <div class="flex items-center bg-white border border-slate-300 rounded-lg p-0.5 text-xs font-mono shadow-xs">
        <button
          type="button"
          onclick={zoomOut}
          disabled={zoom <= 0.5}
          class="w-7 h-7 flex items-center justify-center text-slate-600 hover:text-slate-900 hover:bg-slate-100 rounded-md transition disabled:opacity-30 disabled:hover:bg-transparent active:scale-95"
          title="Perkecil Tampilan (Ctrl + -)"
        >
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M20 12H4" />
          </svg>
        </button>

        <button
          type="button"
          onclick={resetZoom}
          class="px-2 h-7 flex items-center justify-center font-bold text-slate-700 hover:text-slate-900 hover:bg-slate-100 rounded-md transition min-w-[50px] text-[11px]"
          title="Klik untuk reset ke 100% (Ctrl + 0)"
        >
          {Math.round(zoom * 100)}%
        </button>

        <button
          type="button"
          onclick={zoomIn}
          disabled={zoom >= 2.5}
          class="w-7 h-7 flex items-center justify-center text-slate-600 hover:text-slate-900 hover:bg-slate-100 rounded-md transition disabled:opacity-30 disabled:hover:bg-transparent active:scale-95"
          title="Perbesar Tampilan (Ctrl + +)"
        >
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M12 4v16m8-8H4" />
          </svg>
        </button>

        <div class="h-4 w-px bg-slate-200 mx-0.5"></div>

        <button
          type="button"
          onclick={fitToScreen}
          class="px-2.5 h-7 flex items-center space-x-1 font-semibold text-amber-700 hover:text-amber-800 hover:bg-amber-100/60 rounded-md transition active:scale-95"
          title="Sesuaikan skala otomatis dengan layar"
        >
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4" />
          </svg>
          <span class="text-[11px] font-sans">Sesuaikan</span>
        </button>
      </div>

      <button
        type="button"
        onclick={() => window.print()}
        class="px-3 py-1.5 bg-white hover:bg-slate-50 text-xs font-semibold text-slate-700 border border-slate-300 rounded-lg transition flex items-center space-x-1.5 shadow-xs active:scale-95"
        title="Cetak format kertas A4 Landscape"
      >
        <svg class="w-3.5 h-3.5 text-slate-500" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
        </svg>
        <span>Cetak (A4)</span>
      </button>

      <button
        type="button"
        onclick={loadBracketData}
        class="px-3 py-1.5 bg-white hover:bg-slate-50 text-xs font-semibold text-slate-700 border border-slate-300 rounded-lg transition shadow-xs active:scale-95"
      >
        Segarkan
      </button>
    </div>
  </div>

  <!-- Tournament Header Banner (Matches Legacy & Printed Reference) -->
  <div class="bracket-banner relative flex items-center justify-between pb-2.5 pt-1 border-b-2 border-slate-900 mb-3 shrink-0">
    <!-- Left Logo / Dojo Emblem -->
    <div class="flex items-center min-w-[140px]">
      <img
        src="/img/ambrilindo-logo.webp"
        alt="Ambrilindo Logo"
        class="h-10 max-h-10 w-auto object-contain"
      />
    </div>

    <!-- Center Category & Tournament Info -->
    <div class="text-center flex-1 px-4">
      <h1 class="text-base md:text-lg font-black text-slate-950 tracking-tight uppercase leading-tight">
        {#if baganInfo?.kode}
          ({baganInfo.kode})
        {/if}
        {baganName}
      </h1>
      <p class="text-[11px] font-semibold text-slate-600 mt-0.5 flex items-center justify-center gap-1.5 flex-wrap">
        <span>🏆</span>
        <span>{eventName || "AMBRILINDO KARATE CHAMPIONSHIP"}</span>
        <span>•</span>
        <span>{eventVenue}</span>
        {#if eventDate}
          <span>•</span>
          <span>{eventDate}</span>
        {/if}
      </p>
    </div>

    <!-- Right Spacer to preserve exact center alignment -->
    <div class="min-w-[140px] flex justify-end items-center text-right text-[10px] text-slate-400 font-mono no-print">
      <span>{elimMatchesCount} Partai</span>
    </div>
  </div>

  <!-- Interactive Bracket Tree Viewport -->
  <div
    bind:this={viewportEl}
    onwheel={handleWheel}
    class="flex-1 overflow-x-auto overflow-y-auto py-2 px-1 scrollbar-thin relative bg-white/70 rounded-xl border border-slate-200/80"
  >
    {#if isLoading}
      <div class="h-64 flex items-center justify-center text-sm text-slate-500 font-medium">
        Memuat struktur bagan turnamen...
      </div>
    {:else if elimRounds.length === 0}
      <div class="h-64 flex flex-col items-center justify-center text-center">
        <p class="text-sm font-bold text-slate-700">Belum ada partai untuk bagan ini</p>
        <p class="text-xs text-slate-400 mt-1 font-normal">Pengundian nomor tanding belum dilakukan.</p>
      </div>
    {:else}
      <!-- Scaled Scroll Wrapper -->
      <div
        class="zoom-container relative min-w-full min-h-full flex items-start"
        style="width: {contentWidth * zoom}px; height: {contentHeight * zoom}px;"
      >
        <div
          class="bracket-outer relative flex origin-top-left transition-transform duration-100 ease-out"
          style="transform: scale({zoom}); transform-origin: top left; width: {contentWidth}px; height: {contentHeight}px;"
        >
          <!-- Dynamic SVG Connector Canvas -->
          <svg bind:this={svgEl} class="absolute inset-0 pointer-events-none w-full h-full z-0 overflow-visible"></svg>

          <!-- Bracket Columns Grid Container -->
          <div bind:this={gridEl} class="bracket-grid flex items-start gap-9 relative z-10 min-w-max pb-4 pr-4">
            {#each elimRounds as [roundNum, roundMatches]}
              {@const slotCount = getSlotCountForRound(roundNum)}
              {@const slotHeightClass = getSlotHeightClass(roundNum)}

              <div class="round-col round-col-elim w-[246px] shrink-0 flex flex-col relative">
                <!-- Column Header with Black Circle Step Badge -->
                <div class="h-9 shrink-0 flex items-center justify-between pb-1.5 border-b-2 border-slate-900 mb-2">
                  <div class="flex items-center space-x-2">
                    <span class="w-5 h-5 rounded-full bg-slate-900 text-white text-[10px] font-black font-mono flex items-center justify-center shrink-0">
                      {getRoundBadge(roundNum, elimRounds.length)}
                    </span>
                    <span class="text-[12px] font-bold uppercase tracking-tight text-slate-900">
                      {getRoundTitle(roundNum, elimRounds.length)}
                    </span>
                  </div>
                </div>

                <!-- Matches Slots in this Round (Exact 864px height across rounds for perfect alignment) -->
                <div class="round-slots flex flex-col h-[864px] relative">
                  {#each Array.from({ length: slotCount }, (_, i) => i + 1) as slotNum}
                    {@const match = getMatchForSlot(roundMatches, slotNum)}
                    <div data-slot={slotNum} class="{slotHeightClass} flex items-center justify-center">
                      {#if match}
                        <div class="match-card w-full bg-white border border-slate-400/90 rounded-md overflow-hidden shadow-xs hover:border-slate-800 transition relative z-10 {roundNum === 4 ? 'border-amber-500 shadow-md' : ''}">
                          <!-- Match Card Header -->
                          <div class="flex items-center justify-between px-2.5 py-1 bg-slate-100 border-b border-slate-300 text-[10px] font-bold font-mono tracking-wider text-slate-800">
                            <span class="flex items-center space-x-1">
                              {#if roundNum === 4}
                                <span class="text-amber-700 font-extrabold flex items-center gap-1">
                                  <span>🏆</span> PARTAI FINAL
                                </span>
                              {:else if roundNum === 3}
                                <span>SEMI {match.urutan || slotNum}</span>
                              {:else}
                                <span>MATCH {match.urutan || slotNum}</span>
                              {/if}
                            </span>
                            {#if match.assigned_tatami_id}
                              <span class="text-[8px] font-bold text-white bg-red-600 px-1.5 py-0.5 rounded tracking-wide">
                                LIVE T{match.assigned_tatami_id}
                              </span>
                            {:else if match.selesai}
                              <span class="text-[8.5px] font-bold text-emerald-700 bg-emerald-100 border border-emerald-300 px-1.5 py-0.2 rounded flex items-center gap-0.5">
                                ✓ Selesai
                              </span>
                            {/if}
                          </div>

                          <!-- AKA (Red) Row -->
                          <div class="flex items-center justify-between px-2 py-1.5 min-h-[38px] transition relative {match.pemenang === '1' ? 'bg-[#fef3c7] border-l-4 border-amber-500' : 'bg-white'}">
                            <div class="flex items-center space-x-2 truncate mr-2 min-w-0">
                              <span class="text-[9px] font-mono font-black text-white bg-[#dc2626] px-1.5 py-0.5 rounded shrink-0 tracking-wider shadow-xs">
                                AKA
                              </span>
                              <div class="truncate min-w-0 leading-tight">
                                <div class="flex items-center space-x-1 truncate">
                                  {#if match.pemenang === '1'}
                                    <span class="text-amber-600 text-[11px]" title="Pemenang">👑</span>
                                  {/if}
                                  <span class="truncate block text-[11.5px] {match.atlet1_nama ? 'font-bold text-slate-900' : 'italic text-slate-400 font-medium'}">
                                    {match.atlet1_nama || (match.atlet1_id ? `Atlet #${match.atlet1_id}` : 'BYE')}
                                  </span>
                                  {#if match.vr1}
                                    <span class="text-[7.5px] font-mono font-bold px-1 bg-red-600 text-white rounded shrink-0">VR</span>
                                  {/if}
                                </div>
                                <div class="text-[9.5px] text-slate-500 truncate mt-0.5">
                                  {match.atlet1_kontingen || (match.atlet1_nama ? '-' : '—')}
                                </div>
                                {#if match.selesai && match.kata1 && match.kata1 !== '0 - Blank'}
                                  <div class="mt-0.5">
                                    <span class="text-[8.5px] font-medium px-1.5 py-0.2 bg-red-50 text-red-700 border border-red-200 rounded inline-block truncate max-w-[150px]">
                                      {match.kata1}
                                    </span>
                                  </div>
                                {/if}
                              </div>
                            </div>
                            <div class="shrink-0 flex items-center">
                              <span class="min-w-[24px] h-[22px] px-1.5 font-mono text-[11.5px] flex items-center justify-center rounded border {match.pemenang === '1' ? 'bg-[#fde047] border-amber-500 text-amber-950 font-black' : 'bg-slate-50 border-slate-300 text-slate-700 font-bold'}">
                                {match.score1 ?? '-'}
                              </span>
                            </div>
                          </div>

                          <!-- AO (Blue) Row -->
                          <div class="flex items-center justify-between px-2 py-1.5 min-h-[38px] transition relative border-t border-slate-200 {match.pemenang === '2' ? 'bg-[#fef3c7] border-l-4 border-amber-500' : 'bg-white'}">
                            <div class="flex items-center space-x-2 truncate mr-2 min-w-0">
                              <span class="text-[9px] font-mono font-black text-white bg-[#2563eb] px-1.5 py-0.5 rounded shrink-0 tracking-wider shadow-xs">
                                AO
                              </span>
                              <div class="truncate min-w-0 leading-tight">
                                <div class="flex items-center space-x-1 truncate">
                                  {#if match.pemenang === '2'}
                                    <span class="text-amber-600 text-[11px]" title="Pemenang">👑</span>
                                  {/if}
                                  <span class="truncate block text-[11.5px] {match.atlet2_nama ? 'font-bold text-slate-900' : 'italic text-slate-400 font-medium'}">
                                    {match.atlet2_nama || (match.atlet2_id ? `Atlet #${match.atlet2_id}` : 'BYE')}
                                  </span>
                                  {#if match.vr2}
                                    <span class="text-[7.5px] font-mono font-bold px-1 bg-blue-600 text-white rounded shrink-0">VR</span>
                                  {/if}
                                </div>
                                <div class="text-[9.5px] text-slate-500 truncate mt-0.5">
                                  {match.atlet2_kontingen || (match.atlet2_nama ? '-' : '—')}
                                </div>
                                {#if match.selesai && match.kata2 && match.kata2 !== '0 - Blank'}
                                  <div class="mt-0.5">
                                    <span class="text-[8.5px] font-medium px-1.5 py-0.2 bg-blue-50 text-blue-700 border border-blue-200 rounded inline-block truncate max-w-[150px]">
                                      {match.kata2}
                                    </span>
                                  </div>
                                {/if}
                              </div>
                            </div>
                            <div class="shrink-0 flex items-center">
                              <span class="min-w-[24px] h-[22px] px-1.5 font-mono text-[11.5px] flex items-center justify-center rounded border {match.pemenang === '2' ? 'bg-[#fde047] border-amber-500 text-amber-950 font-black' : 'bg-slate-50 border-slate-300 text-slate-700 font-bold'}">
                                {match.score2 ?? '-'}
                              </span>
                            </div>
                          </div>
                        </div>
                      {:else}
                        <div class="w-full border border-dashed border-slate-300 rounded-md h-12 flex items-center justify-center text-[9px] text-slate-400 font-mono bg-white/40">
                          Kosong
                        </div>
                      {/if}
                    </div>
                  {/each}

                  <!-- Column 4 Bottom Section: Ambrilindo Logo + Hasil Juara Podium Card -->
                  {#if roundNum === 4}
                    <div class="absolute bottom-0 right-0 w-full flex flex-col items-end z-20">
                      <!-- Ambrilindo Brand Logo -->
                      <div class="mb-2.5 flex items-center justify-end w-full pr-1">
                        <img
                          src="/img/ambrilindo-logo.webp"
                          alt="Ambrilindo"
                          class="h-9 max-w-[160px] object-contain"
                        />
                      </div>

                      <!-- Hasil Juara Podium Card (Matches Reference Image) -->
                      <div class="w-full bg-white border border-amber-500 rounded-lg overflow-hidden shadow-sm">
                        <div class="px-3 py-1.5 bg-[#fef3c7] border-b border-amber-300 flex items-center justify-between">
                          <div class="flex items-center space-x-1.5">
                            <span class="text-amber-700 text-xs">🏆</span>
                            <span class="text-[11.5px] font-bold text-amber-900 tracking-tight">Hasil Juara</span>
                          </div>
                        </div>

                        <div class="divide-y divide-slate-100 text-[11px]">
                          <!-- Juara 1 (Gold) -->
                          <div class="px-2.5 py-1.5 flex items-center space-x-2.5 bg-white">
                            <span class="w-5 h-5 rounded-full bg-gradient-to-br from-amber-300 to-amber-500 text-amber-950 font-black font-mono text-[10px] flex items-center justify-center shrink-0 shadow-xs">
                              1
                            </span>
                            <div class="min-w-0 truncate">
                              <div class="font-bold text-slate-900 truncate leading-tight">{podium.j1.name}</div>
                              <div class="text-[9.5px] text-slate-500 truncate">{podium.j1.contingent}</div>
                            </div>
                          </div>

                          <!-- Juara 2 (Silver) -->
                          <div class="px-2.5 py-1.5 flex items-center space-x-2.5 bg-white">
                            <span class="w-5 h-5 rounded-full bg-gradient-to-br from-slate-200 to-slate-400 text-slate-800 font-black font-mono text-[10px] flex items-center justify-center shrink-0 shadow-xs">
                              2
                            </span>
                            <div class="min-w-0 truncate">
                              <div class="font-bold text-slate-900 truncate leading-tight">{podium.j2.name}</div>
                              <div class="text-[9.5px] text-slate-500 truncate">{podium.j2.contingent}</div>
                            </div>
                          </div>

                          <!-- Juara 3A (Bronze) -->
                          <div class="px-2.5 py-1.5 flex items-center space-x-2.5 bg-white">
                            <span class="w-5 h-5 rounded-full bg-gradient-to-br from-orange-200 to-amber-600 text-orange-950 font-black font-mono text-[10px] flex items-center justify-center shrink-0 shadow-xs">
                              3
                            </span>
                            <div class="min-w-0 truncate">
                              <div class="font-bold text-slate-900 truncate leading-tight">{podium.j3a.name}</div>
                              <div class="text-[9.5px] text-slate-500 truncate">{podium.j3a.contingent}</div>
                            </div>
                          </div>

                          <!-- Juara 3B (Bronze) -->
                          <div class="px-2.5 py-1.5 flex items-center space-x-2.5 bg-white">
                            <span class="w-5 h-5 rounded-full bg-gradient-to-br from-orange-200 to-amber-600 text-orange-950 font-black font-mono text-[10px] flex items-center justify-center shrink-0 shadow-xs">
                              3
                            </span>
                            <div class="min-w-0 truncate">
                              <div class="font-bold text-slate-900 truncate leading-tight">{podium.j3b.name}</div>
                              <div class="text-[9.5px] text-slate-500 truncate">{podium.j3b.contingent}</div>
                            </div>
                          </div>
                        </div>
                      </div>
                    </div>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  @media print {
    @page {
      size: A4 landscape;
      margin: 6mm 8mm;
    }
    :global(body) {
      background: #ffffff !important;
      color: #000000 !important;
      -webkit-print-color-adjust: exact !important;
      print-color-adjust: exact !important;
    }
    :global(.no-print) {
      display: none !important;
    }
    .print-sheet {
      background: #ffffff !important;
      color: #000000 !important;
      padding: 0 !important;
      overflow: visible !important;
      width: 100% !important;
      height: auto !important;
    }
    .zoom-container {
      width: 100% !important;
      height: auto !important;
      min-width: 0 !important;
    }
    .bracket-outer {
      transform: scale(0.92) !important;
      transform-origin: top left !important;
      width: 100% !important;
      height: auto !important;
    }
    .bracket-grid {
      gap: 24px !important;
    }
    .round-col {
      width: 236px !important;
    }
    .match-card {
      background: #ffffff !important;
      border: 1px solid #1e293b !important;
      color: #000000 !important;
      box-shadow: none !important;
    }
    :global(svg line) {
      stroke: #0f172a !important;
      stroke-width: 1.2px !important;
    }
  }
</style>
