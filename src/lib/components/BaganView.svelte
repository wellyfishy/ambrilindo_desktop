<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import type { DetailBaganItem } from "$lib/types";

  interface Props {
    baganId: number;
    baganName: string;
    onDispatchToTatami?: (tatamiNum: number, matchId: number) => void;
  }

  let { baganId, baganName, onDispatchToTatami }: Props = $props();

  let matches = $state<DetailBaganItem[]>([]);
  let isLoading = $state(true);

  let svgEl = $state<SVGSVGElement | null>(null);
  let gridEl = $state<HTMLDivElement | null>(null);

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
      requestAnimationFrame(drawConnectors);
    }
  }

  // Group matches by round (1 = Babak 1, 2 = Perempat Final, 3 = Semi Final, 4 = Final, 5 = Perebutan Juara 3)
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

  // Standalone bronze medal match (Round 5)
  let bronzeRound = $derived(rounds.find(([r]) => r === 5));

  function getRoundTitle(roundNum: number, totalElimRounds: number): string {
    if (roundNum === 5) return "PEREBUTAN JUARA 3";
    if (roundNum === 4) return "FINAL";
    if (roundNum === 3) return totalElimRounds >= 4 ? "SEMI FINAL" : "FINAL";
    if (roundNum === 2) return totalElimRounds >= 4 ? "PEREMPAT FINAL" : "SEMI FINAL";
    return "BABAK 1";
  }

  function getSlotCountForRound(roundNum: number): number {
    switch (roundNum) {
      case 1: return 8;
      case 2: return 4;
      case 3: return 2;
      case 4: return 1;
      case 5: return 1;
      default: return 1;
    }
  }

  function getSlotHeightClass(roundNum: number): string {
    switch (roundNum) {
      case 1: return "h-[64px]";
      case 2: return "h-[128px]";
      case 3: return "h-[256px]";
      case 4: return "h-[512px]";
      case 5: return "h-[512px]";
      default: return "h-[64px]";
    }
  }

  function getMatchLabel(roundNum: number, urutan: number): string {
    switch (roundNum) {
      case 1: return `R1 #${urutan}`;
      case 2: return `QF #${urutan}`;
      case 3: return `SF #${urutan}`;
      case 4: return "FINAL";
      case 5: return "JUARA 3";
      default: return `#${urutan}`;
    }
  }

  function getMatchForSlot(roundMatches: DetailBaganItem[], slotNum: number): DetailBaganItem | undefined {
    return roundMatches.find((m) => (m.urutan || 0) === slotNum) || roundMatches[slotNum - 1];
  }

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
    if (outer.width > 0 && outer.height > 0) {
      svgEl.setAttribute("width", Math.round(outer.width).toString());
      svgEl.setAttribute("height", Math.round(outer.height).toString());
    }

    const cols = Array.from(gridEl.querySelectorAll<HTMLElement>(".round-col-elim"));
    if (cols.length < 2) return;

    // Dark-theme bracket connector styling
    const strokeColor = "rgba(255, 255, 255, 0.35)";
    const strokeWidth = "1.5";

    for (let ri = 0; ri < cols.length - 1; ri++) {
      const fromCol = cols[ri];
      const toCol = cols[ri + 1];

      // Match each slot in target round (round 2 has 4 slots, round 3 has 2 slots, round 4 has 1 slot)
      const toSlots = Array.from(toCol.querySelectorAll<HTMLElement>("[data-slot]"));

      for (let i = 0; i < toSlots.length; i++) {
        const slotOut = toSlots[i];
        const cardOut = slotOut.querySelector<HTMLElement>(".match-card");
        if (!cardOut) continue;

        // In binary tree elimination, slot i (0-based) is fed by slots (2*i + 1) and (2*i + 2) in fromCol
        const slotA = fromCol.querySelector<HTMLElement>(`[data-slot="${i * 2 + 1}"]`);
        const slotB = fromCol.querySelector<HTMLElement>(`[data-slot="${i * 2 + 2}"]`);

        const cardA = slotA?.querySelector<HTMLElement>(".match-card");
        const cardB = slotB?.querySelector<HTMLElement>(".match-card");

        const rOut = cardOut.getBoundingClientRect();
        const yTo = Math.round(rOut.top + rOut.height / 2 - outer.top);
        const xTo = Math.round(rOut.left - outer.left);

        if (cardA && cardB) {
          const rA = cardA.getBoundingClientRect();
          const rB = cardB.getBoundingClientRect();

          const yA = Math.round(rA.top + rA.height / 2 - outer.top);
          const yB = Math.round(rB.top + rB.height / 2 - outer.top);
          const xFromA = Math.round(rA.right - outer.left);
          const xFromB = Math.round(rB.right - outer.left);

          const maxXFrom = Math.max(xFromA, xFromB);
          const xMid = Math.round(maxXFrom + (xTo - maxXFrom) / 2);
          const yMid = Math.round((yA + yB) / 2);

          // 1. Horizontal from upper card A to xMid
          seg(svgEl, xFromA, yA, xMid, yA, strokeColor, strokeWidth);
          // 2. Horizontal from lower card B to xMid
          seg(svgEl, xFromB, yB, xMid, yB, strokeColor, strokeWidth);
          // 3. Vertical crossbar between yA and yB at xMid
          seg(svgEl, xMid, yA, xMid, yB, strokeColor, strokeWidth);
          // 4. Vertical alignment if midpoint differs slightly from target y
          if (yMid !== yTo) {
            seg(svgEl, xMid, yMid, xMid, yTo, strokeColor, strokeWidth);
          }
          // 5. Horizontal into target card Out
          seg(svgEl, xMid, yTo, xTo, yTo, strokeColor, strokeWidth);
        } else if (cardA) {
          const rA = cardA.getBoundingClientRect();
          const yA = Math.round(rA.top + rA.height / 2 - outer.top);
          const xFrom = Math.round(rA.right - outer.left);
          const xMid = Math.round(xFrom + (xTo - xFrom) / 2);

          seg(svgEl, xFrom, yA, xMid, yA, strokeColor, strokeWidth);
          if (yA !== yTo) {
            seg(svgEl, xMid, yA, xMid, yTo, strokeColor, strokeWidth);
          }
          seg(svgEl, xMid, yTo, xTo, yTo, strokeColor, strokeWidth);
        } else if (cardB) {
          const rB = cardB.getBoundingClientRect();
          const yB = Math.round(rB.top + rB.height / 2 - outer.top);
          const xFrom = Math.round(rB.right - outer.left);
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
        requestAnimationFrame(drawConnectors);
      });
    }
  });

  onMount(() => {
    loadBracketData();

    let ro: ResizeObserver | null = null;
    if (typeof ResizeObserver !== "undefined") {
      ro = new ResizeObserver(() => {
        requestAnimationFrame(drawConnectors);
      });
      if (gridEl) ro.observe(gridEl);
    }

    const handleWindowResize = () => {
      requestAnimationFrame(drawConnectors);
    };
    window.addEventListener("resize", handleWindowResize);

    if (typeof document !== "undefined" && document.fonts && document.fonts.ready) {
      document.fonts.ready.then(() => {
        requestAnimationFrame(drawConnectors);
      });
    }

    return () => {
      ro?.disconnect();
      window.removeEventListener("resize", handleWindowResize);
    };
  });
</script>

<div class="h-full flex flex-col p-5 select-none bg-[#14161b] text-zinc-200 overflow-hidden font-sans print-sheet">
  <!-- Top Bracket Info (Hidden on Print) -->
  <div class="no-print flex items-center justify-between pb-3 border-b border-white/[0.08] shrink-0">
    <div>
      <h2 class="text-lg font-black text-white tracking-tight uppercase">{baganName}</h2>
      <p class="text-xs text-zinc-400 mt-0.5 font-normal">
        Struktur bagan eliminasi turnamen • {matches.length} Partai terdaftar
      </p>
    </div>

    <div class="flex items-center space-x-2">
      <button
        onclick={() => window.print()}
        class="px-3 py-1.5 bg-white/[0.04] hover:bg-white/[0.08] text-xs font-medium text-zinc-300 border border-white/[0.08] rounded-xl transition flex items-center space-x-1.5 shadow-sm active:scale-95"
        title="Cetak format kertas A4 Landscape"
      >
        <svg class="w-3.5 h-3.5 text-zinc-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
        </svg>
        <span>Cetak (A4)</span>
      </button>

      <button
        onclick={loadBracketData}
        class="px-3 py-1.5 bg-white/[0.04] hover:bg-white/[0.08] text-xs font-medium text-zinc-300 border border-white/[0.08] rounded-xl transition shadow-sm active:scale-95"
      >
        Segarkan
      </button>
    </div>
  </div>

  <!-- Print-Only Header for A4 Landscape -->
  <div class="hidden print-header mb-3 pb-2 border-b-2 border-slate-900">
    <div class="flex items-center justify-between">
      <div>
        <h1 class="text-base font-black uppercase tracking-tight text-slate-900">{baganName}</h1>
        <p class="text-[9.5px] text-slate-600 font-sans">Ambrilindo Karate Tournament System • Bagan Resmi Pertandingan</p>
      </div>
      <div class="text-right text-[9px] text-slate-500 font-mono">
        <span>{matches.length} Partai Eliminasi</span>
      </div>
    </div>
  </div>

  <!-- Interactive Bracket Tree Viewport (Horizontally & vertically scrollable) -->
  <div class="flex-1 overflow-x-auto overflow-y-auto py-3 px-1 scrollbar-thin">
    {#if isLoading}
      <div class="h-64 flex items-center justify-center text-sm text-zinc-500">
        Memuat struktur bagan...
      </div>
    {:else if elimRounds.length === 0}
      <div class="h-64 flex flex-col items-center justify-center text-center">
        <p class="text-sm font-bold text-zinc-300">Belum ada partai untuk bagan ini</p>
        <p class="text-xs text-zinc-500 mt-1 font-normal">Pengundian nomor tanding belum dilakukan.</p>
      </div>
    {:else}
      <div class="bracket-outer relative min-w-max flex pb-4">
        <!-- Dynamic SVG Connector Canvas -->
        <svg bind:this={svgEl} class="absolute inset-0 pointer-events-none w-full h-full z-0 overflow-visible"></svg>

        <!-- Bracket Columns Container -->
        <div bind:this={gridEl} class="bracket-grid flex items-start gap-8 relative z-10 min-w-max">
          {#each elimRounds as [roundNum, roundMatches]}
            {@const slotCount = getSlotCountForRound(roundNum)}
            {@const slotHeightClass = getSlotHeightClass(roundNum)}

            <div class="round-col round-col-elim w-56 shrink-0 flex flex-col">
              <!-- Round Header -->
              <div class="h-8 shrink-0 bg-[#181a20] border border-white/[0.08] rounded-lg px-2.5 py-1 mb-2.5 flex items-center justify-between shadow-sm">
                <span class="text-[11px] font-mono font-bold uppercase tracking-wider text-white">
                  {getRoundTitle(roundNum, elimRounds.length)}
                </span>
                <span class="text-[9px] text-zinc-500 font-mono">
                  {roundMatches.length} Partai
                </span>
              </div>

              <!-- Matches Slots in this Round (Exact 512px height across rounds for perfect connector alignment) -->
              <div class="round-slots flex flex-col h-[512px]">
                {#each Array.from({ length: slotCount }, (_, i) => i + 1) as slotNum}
                  {@const match = getMatchForSlot(roundMatches, slotNum)}
                  <div data-slot={slotNum} class="{slotHeightClass} flex items-center justify-center">
                    {#if match}
                      <div class="match-card w-full bg-[#181a20] border {match.selesai ? 'border-emerald-500/40' : 'border-white/[0.08]'} rounded-md overflow-hidden shadow-sm hover:border-white/20 transition relative z-10">
                        <!-- Micro Header -->
                        <div class="flex items-center justify-between px-2 py-0.5 bg-white/[0.02] border-b border-white/[0.05] text-[8.5px] font-mono text-zinc-400">
                          <span class="font-bold tracking-wider">{getMatchLabel(roundNum, match.urutan || slotNum)}</span>
                          {#if match.assigned_tatami_id}
                            <span class="text-[8px] font-bold text-amber-400 bg-amber-500/10 px-1 rounded">
                              T{match.assigned_tatami_id}
                            </span>
                          {:else}
                            <span class="text-[7.5px] {match.selesai ? 'text-emerald-400 font-bold' : 'text-zinc-600'}">
                              {match.selesai ? 'SELESAI' : 'STANDBY'}
                            </span>
                          {/if}
                        </div>

                        <!-- AKA (Red) Row -->
                        <div class="flex items-center justify-between px-2 py-0.5 text-[10.5px] leading-tight {match.pemenang === '1' ? 'bg-red-500/15 font-semibold text-white' : 'text-zinc-200'}">
                          <div class="flex items-center space-x-1.5 truncate mr-1.5 min-w-0">
                            <span class="text-[8.5px] font-mono font-bold text-red-400 shrink-0">AKA</span>
                            {#if match.vr1}
                              <span class="text-[7px] font-mono font-bold px-0.5 bg-amber-500/20 text-amber-300 rounded shrink-0">VR</span>
                            {/if}
                            <span class="truncate block font-medium">
                              {match.atlet1_nama || (match.atlet1_id ? `Atlet #${match.atlet1_id}` : 'BYE')}
                            </span>
                            {#if match.atlet1_kontingen}
                              <span class="text-[8.5px] text-zinc-400 truncate shrink-0">
                                ({match.atlet1_kontingen})
                              </span>
                            {/if}
                          </div>
                          <span class="font-mono text-[11px] font-bold shrink-0">{match.score1 || '0'}</span>
                        </div>

                        <!-- AO (Blue) Row -->
                        <div class="flex items-center justify-between px-2 py-0.5 text-[10.5px] leading-tight border-t border-white/[0.04] {match.pemenang === '2' ? 'bg-blue-500/15 font-semibold text-white' : 'text-zinc-200'}">
                          <div class="flex items-center space-x-1.5 truncate mr-1.5 min-w-0">
                            <span class="text-[8.5px] font-mono font-bold text-blue-400 shrink-0">AO</span>
                            {#if match.vr2}
                              <span class="text-[7px] font-mono font-bold px-0.5 bg-amber-500/20 text-amber-300 rounded shrink-0">VR</span>
                            {/if}
                            <span class="truncate block font-medium">
                              {match.atlet2_nama || (match.atlet2_id ? `Atlet #${match.atlet2_id}` : 'BYE')}
                            </span>
                            {#if match.atlet2_kontingen}
                              <span class="text-[8.5px] text-zinc-400 truncate shrink-0">
                                ({match.atlet2_kontingen})
                              </span>
                            {/if}
                          </div>
                          <span class="font-mono text-[11px] font-bold shrink-0">{match.score2 || '0'}</span>
                        </div>
                      </div>
                    {:else}
                      <div class="w-full border border-dashed border-white/[0.05] rounded-md h-11 flex items-center justify-center text-[9px] text-zinc-600 font-mono">
                        Kosong
                      </div>
                    {/if}
                  </div>
                {/each}
              </div>
            </div>
          {/each}

          <!-- Round 5: Perebutan Juara 3 (Bronze Match) -->
          {#if bronzeRound && bronzeRound[1].length > 0}
            <div class="round-col round-col-bronze w-56 shrink-0 flex flex-col border-l border-white/[0.08] pl-6">
              <!-- Round Header -->
              <div class="h-8 shrink-0 bg-amber-950/20 border border-amber-500/30 rounded-lg px-2.5 py-1 mb-2.5 flex items-center justify-between shadow-sm">
                <span class="text-[11px] font-mono font-bold uppercase tracking-wider text-amber-400">
                  PEREBUTAN JUARA 3
                </span>
                <span class="text-[9px] text-amber-500/70 font-mono">
                  Perunggu
                </span>
              </div>

              <!-- Match in Perebutan Juara 3 centered -->
              <div class="round-slots flex flex-col h-[512px]">
                {#each bronzeRound[1] as match}
                  <div class="h-[512px] flex items-center justify-center">
                    <div class="match-card w-full bg-[#181a20] border {match.selesai ? 'border-amber-500/40' : 'border-white/[0.08]'} rounded-md overflow-hidden shadow-sm hover:border-amber-500/30 transition relative z-10">
                      <!-- Micro Header -->
                      <div class="flex items-center justify-between px-2 py-0.5 bg-amber-500/10 border-b border-amber-500/20 text-[8.5px] font-mono text-amber-300">
                        <span class="font-bold tracking-wider">JUARA 3</span>
                        {#if match.assigned_tatami_id}
                          <span class="text-[8px] font-bold text-amber-400 bg-amber-500/20 px-1 rounded">
                            T{match.assigned_tatami_id}
                          </span>
                        {:else}
                          <span class="text-[7.5px] {match.selesai ? 'text-amber-400 font-bold' : 'text-zinc-600'}">
                            {match.selesai ? 'SELESAI' : 'STANDBY'}
                          </span>
                        {/if}
                      </div>

                      <!-- AKA (Red) Row -->
                      <div class="flex items-center justify-between px-2 py-0.5 text-[10.5px] leading-tight {match.pemenang === '1' ? 'bg-red-500/15 font-semibold text-white' : 'text-zinc-200'}">
                        <div class="flex items-center space-x-1.5 truncate mr-1.5 min-w-0">
                          <span class="text-[8.5px] font-mono font-bold text-red-400 shrink-0">AKA</span>
                          {#if match.vr1}
                            <span class="text-[7px] font-mono font-bold px-0.5 bg-amber-500/20 text-amber-300 rounded shrink-0">VR</span>
                          {/if}
                          <span class="truncate block font-medium">
                            {match.atlet1_nama || (match.atlet1_id ? `Atlet #${match.atlet1_id}` : 'BYE')}
                          </span>
                          {#if match.atlet1_kontingen}
                            <span class="text-[8.5px] text-zinc-400 truncate shrink-0">
                              ({match.atlet1_kontingen})
                            </span>
                          {/if}
                        </div>
                        <span class="font-mono text-[11px] font-bold shrink-0">{match.score1 || '0'}</span>
                      </div>

                      <!-- AO (Blue) Row -->
                      <div class="flex items-center justify-between px-2 py-0.5 text-[10.5px] leading-tight border-t border-white/[0.04] {match.pemenang === '2' ? 'bg-blue-500/15 font-semibold text-white' : 'text-zinc-200'}">
                        <div class="flex items-center space-x-1.5 truncate mr-1.5 min-w-0">
                          <span class="text-[8.5px] font-mono font-bold text-blue-400 shrink-0">AO</span>
                          {#if match.vr2}
                            <span class="text-[7px] font-mono font-bold px-0.5 bg-amber-500/20 text-amber-300 rounded shrink-0">VR</span>
                          {/if}
                          <span class="truncate block font-medium">
                            {match.atlet2_nama || (match.atlet2_id ? `Atlet #${match.atlet2_id}` : 'BYE')}
                          </span>
                          {#if match.atlet2_kontingen}
                            <span class="text-[8.5px] text-zinc-400 truncate shrink-0">
                              ({match.atlet2_kontingen})
                            </span>
                          {/if}
                        </div>
                        <span class="font-mono text-[11px] font-bold shrink-0">{match.score2 || '0'}</span>
                      </div>
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          {/if}
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  @media print {
    @page {
      size: A4 landscape;
      margin: 8mm 6mm;
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
    .print-header {
      display: block !important;
    }
    .print-sheet {
      background: #ffffff !important;
      color: #000000 !important;
      padding: 0 !important;
      overflow: visible !important;
      width: 100% !important;
      height: auto !important;
    }
    .bracket-grid {
      gap: 16px !important;
    }
    .round-col {
      width: 195px !important;
    }
    .match-card {
      background: #ffffff !important;
      border: 1px solid #1e293b !important;
      color: #000000 !important;
      box-shadow: none !important;
    }
    .match-card * {
      color: #0f172a !important;
    }
    :global(svg line) {
      stroke: #0f172a !important;
      stroke-width: 1.2px !important;
    }
  }
</style>
