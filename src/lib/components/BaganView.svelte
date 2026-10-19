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
      case 1: return "h-[82px]";
      case 2: return "h-[164px]";
      case 3: return "h-[328px]";
      case 4: return "h-[656px]";
      case 5: return "h-[656px]";
      default: return "h-[82px]";
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
    const cols = Array.from(gridEl.querySelectorAll<HTMLElement>(".round-col-elim"));
    if (cols.length < 2) return;

    // Dark-theme bracket connector styling
    const strokeColor = "rgba(255, 255, 255, 0.28)";
    const strokeWidth = "1.5";

    for (let ri = 0; ri < cols.length - 1; ri++) {
      const fromCards = Array.from(cols[ri].querySelectorAll<HTMLElement>(".match-card"));
      const toCards = Array.from(cols[ri + 1].querySelectorAll<HTMLElement>(".match-card"));

      if (fromCards.length === 0 || toCards.length === 0) continue;

      for (let i = 0; i < toCards.length; i++) {
        const cardA = fromCards[i * 2];
        const cardB = fromCards[i * 2 + 1];
        const cardOut = toCards[i];
        if (!cardOut) continue;

        const rOut = cardOut.getBoundingClientRect();
        const yTo = Math.round(rOut.top + rOut.height / 2 - outer.top);
        const xTo = Math.round(rOut.left - outer.left);

        if (cardA && cardB) {
          const rA = cardA.getBoundingClientRect();
          const rB = cardB.getBoundingClientRect();

          const yA = Math.round(rA.top + rA.height / 2 - outer.top);
          const yB = Math.round(rB.top + rB.height / 2 - outer.top);
          const xFrom = Math.round(rA.right - outer.left);
          const xMid = Math.round(xFrom + (xTo - xFrom) / 2);
          const yMid = Math.round((yA + yB) / 2);

          // 1. Horizontal from upper card A to xMid
          seg(svgEl, xFrom, yA, xMid, yA, strokeColor, strokeWidth);
          // 2. Horizontal from lower card B to xMid
          seg(svgEl, xFrom, yB, xMid, yB, strokeColor, strokeWidth);
          // 3. Vertical crossbar between yA and yB at xMid
          seg(svgEl, xMid, yA, xMid, yB, strokeColor, strokeWidth);
          // 4. Vertical micro-adjustment if needed
          if (yMid !== yTo) {
            seg(svgEl, xMid, yMid, xMid, yTo, strokeColor, strokeWidth);
          }
          // 5. Horizontal into target card
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

<div class="h-full flex flex-col p-6 select-none bg-[#14161b] text-zinc-200 overflow-hidden font-sans print-sheet">
  <!-- Top Bracket Info (Hidden on Print) -->
  <div class="no-print flex items-center justify-between pb-4 border-b border-white/[0.08] shrink-0">
    <div>
      <h2 class="text-xl font-black text-white tracking-tight uppercase">{baganName}</h2>
      <p class="text-xs text-zinc-400 mt-0.5 font-normal">
        Struktur bagan eliminasi turnamen • {matches.length} Partai terdaftar
      </p>
    </div>

    <div class="flex items-center space-x-2">
      <button
        onclick={() => window.print()}
        class="px-3 py-1.5 bg-white/[0.04] hover:bg-white/[0.08] text-xs font-medium text-zinc-300 border border-white/[0.08] rounded-xl transition flex items-center space-x-1.5"
        title="Cetak format kertas A4"
      >
        <svg class="w-3.5 h-3.5 text-zinc-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
        </svg>
        <span>Cetak (A4)</span>
      </button>

      <button
        onclick={loadBracketData}
        class="px-3 py-1.5 bg-white/[0.04] hover:bg-white/[0.08] text-xs font-medium text-zinc-300 border border-white/[0.08] rounded-xl transition"
      >
        Segarkan
      </button>
    </div>
  </div>

  <!-- Interactive Bracket Tree Viewport (Horizontally & vertically scrollable) -->
  <div class="flex-1 overflow-x-auto overflow-y-auto py-4 px-2 scrollbar-thin">
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
      <div class="bracket-outer relative min-w-max flex pb-6">
        <!-- Dynamic SVG Connector Canvas -->
        <svg bind:this={svgEl} class="absolute inset-0 pointer-events-none w-full h-full z-0 overflow-visible"></svg>

        <!-- Bracket Columns Container -->
        <div bind:this={gridEl} class="bracket-grid flex items-start gap-12 relative z-10 min-w-max">
          {#each elimRounds as [roundNum, roundMatches]}
            {@const slotCount = getSlotCountForRound(roundNum)}
            {@const slotHeightClass = getSlotHeightClass(roundNum)}

            <div class="round-col round-col-elim w-64 shrink-0 flex flex-col">
              <!-- Round Header -->
              <div class="h-10 shrink-0 bg-[#181a20] border border-white/[0.08] rounded-xl px-3 py-1 mb-4 flex items-center justify-between shadow-sm">
                <span class="text-xs font-mono font-bold uppercase tracking-wider text-white">
                  {getRoundTitle(roundNum, elimRounds.length)}
                </span>
                <span class="text-[10px] text-zinc-500 font-mono">
                  {roundMatches.length} Partai
                </span>
              </div>

              <!-- Matches Slots in this Round (Exact multiple heights for geometric alignment) -->
              <div class="round-slots flex flex-col h-[656px]">
                {#each Array.from({ length: slotCount }, (_, i) => i + 1) as slotNum}
                  {@const match = getMatchForSlot(roundMatches, slotNum)}
                  <div class="{slotHeightClass} flex items-center justify-center">
                    {#if match}
                      <div class="match-card w-full bg-[#181a20] border {match.selesai ? 'border-emerald-500/40' : 'border-white/[0.08]'} rounded-lg overflow-hidden shadow-sm hover:border-white/20 transition relative z-10">
                        <!-- Micro Header -->
                        <div class="flex items-center justify-between px-2.5 py-0.5 bg-white/[0.02] border-b border-white/[0.05] text-[9px] font-mono text-zinc-400">
                          <span class="font-bold tracking-wider">#{match.urutan || match.id}</span>
                          {#if match.assigned_tatami_id}
                            <span class="text-[9px] font-bold text-amber-400 bg-amber-500/10 px-1 rounded">
                              T{match.assigned_tatami_id}
                            </span>
                          {:else}
                            <span class="text-[8px] {match.selesai ? 'text-emerald-400 font-bold' : 'text-zinc-600'}">
                              {match.selesai ? 'SELESAI' : 'STANDBY'}
                            </span>
                          {/if}
                        </div>

                        <!-- AKA (Red) Row -->
                        <div class="flex items-center justify-between px-2.5 py-1 text-[11px] {match.pemenang === '1' ? 'bg-red-500/15 font-semibold text-white' : 'text-zinc-200'}">
                          <div class="flex items-center space-x-1.5 truncate mr-1.5 min-w-0">
                            <span class="text-[9px] font-mono font-bold text-red-400 shrink-0">AKA</span>
                            {#if match.vr1}
                              <span class="text-[8px] font-mono font-bold px-0.5 bg-amber-500/20 text-amber-300 rounded shrink-0">VR</span>
                            {/if}
                            <span class="truncate block font-medium">
                              {match.atlet1_nama || (match.atlet1_id ? `Atlet #${match.atlet1_id}` : 'BYE')}
                            </span>
                            {#if match.atlet1_kontingen}
                              <span class="text-[9px] text-zinc-500 truncate shrink-0">
                                ({match.atlet1_kontingen})
                              </span>
                            {/if}
                          </div>
                          <span class="font-mono text-xs font-bold shrink-0">{match.score1 || '0'}</span>
                        </div>

                        <!-- AO (Blue) Row -->
                        <div class="flex items-center justify-between px-2.5 py-1 text-[11px] border-t border-white/[0.04] {match.pemenang === '2' ? 'bg-blue-500/15 font-semibold text-white' : 'text-zinc-200'}">
                          <div class="flex items-center space-x-1.5 truncate mr-1.5 min-w-0">
                            <span class="text-[9px] font-mono font-bold text-blue-400 shrink-0">AO</span>
                            {#if match.vr2}
                              <span class="text-[8px] font-mono font-bold px-0.5 bg-amber-500/20 text-amber-300 rounded shrink-0">VR</span>
                            {/if}
                            <span class="truncate block font-medium">
                              {match.atlet2_nama || (match.atlet2_id ? `Atlet #${match.atlet2_id}` : 'BYE')}
                            </span>
                            {#if match.atlet2_kontingen}
                              <span class="text-[9px] text-zinc-500 truncate shrink-0">
                                ({match.atlet2_kontingen})
                              </span>
                            {/if}
                          </div>
                          <span class="font-mono text-xs font-bold shrink-0">{match.score2 || '0'}</span>
                        </div>
                      </div>
                    {:else}
                      <div class="w-full border border-dashed border-white/[0.05] rounded-lg h-14 flex items-center justify-center text-[10px] text-zinc-600 font-mono">
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
            <div class="round-col round-col-bronze w-64 shrink-0 flex flex-col border-l border-white/[0.08] pl-8">
              <!-- Round Header -->
              <div class="h-10 shrink-0 bg-amber-950/20 border border-amber-500/30 rounded-xl px-3 py-1 mb-4 flex items-center justify-between shadow-sm">
                <span class="text-xs font-mono font-bold uppercase tracking-wider text-amber-400">
                  PEREBUTAN JUARA 3
                </span>
                <span class="text-[10px] text-amber-500/70 font-mono">
                  Medali Perunggu
                </span>
              </div>

              <!-- Match in Perebutan Juara 3 centered -->
              <div class="round-slots flex flex-col h-[656px]">
                {#each bronzeRound[1] as match}
                  <div class="h-[656px] flex items-center justify-center">
                    <div class="match-card w-full bg-[#181a20] border {match.selesai ? 'border-amber-500/40' : 'border-white/[0.08]'} rounded-lg overflow-hidden shadow-sm hover:border-amber-500/30 transition relative z-10">
                      <!-- Micro Header -->
                      <div class="flex items-center justify-between px-2.5 py-0.5 bg-amber-500/10 border-b border-amber-500/20 text-[9px] font-mono text-amber-300">
                        <span class="font-bold tracking-wider">BRONZE #{match.urutan || match.id}</span>
                        {#if match.assigned_tatami_id}
                          <span class="text-[9px] font-bold text-amber-400 bg-amber-500/20 px-1 rounded">
                            T{match.assigned_tatami_id}
                          </span>
                        {:else}
                          <span class="text-[8px] {match.selesai ? 'text-amber-400 font-bold' : 'text-zinc-600'}">
                            {match.selesai ? 'SELESAI' : 'STANDBY'}
                          </span>
                        {/if}
                      </div>

                      <!-- AKA (Red) Row -->
                      <div class="flex items-center justify-between px-2.5 py-1 text-[11px] {match.pemenang === '1' ? 'bg-red-500/15 font-semibold text-white' : 'text-zinc-200'}">
                        <div class="flex items-center space-x-1.5 truncate mr-1.5 min-w-0">
                          <span class="text-[9px] font-mono font-bold text-red-400 shrink-0">AKA</span>
                          {#if match.vr1}
                            <span class="text-[8px] font-mono font-bold px-0.5 bg-amber-500/20 text-amber-300 rounded shrink-0">VR</span>
                          {/if}
                          <span class="truncate block font-medium">
                            {match.atlet1_nama || (match.atlet1_id ? `Atlet #${match.atlet1_id}` : 'BYE')}
                          </span>
                          {#if match.atlet1_kontingen}
                            <span class="text-[9px] text-zinc-500 truncate shrink-0">
                              ({match.atlet1_kontingen})
                            </span>
                          {/if}
                        </div>
                        <span class="font-mono text-xs font-bold shrink-0">{match.score1 || '0'}</span>
                      </div>

                      <!-- AO (Blue) Row -->
                      <div class="flex items-center justify-between px-2.5 py-1 text-[11px] border-t border-white/[0.04] {match.pemenang === '2' ? 'bg-blue-500/15 font-semibold text-white' : 'text-zinc-200'}">
                        <div class="flex items-center space-x-1.5 truncate mr-1.5 min-w-0">
                          <span class="text-[9px] font-mono font-bold text-blue-400 shrink-0">AO</span>
                          {#if match.vr2}
                            <span class="text-[8px] font-mono font-bold px-0.5 bg-amber-500/20 text-amber-300 rounded shrink-0">VR</span>
                          {/if}
                          <span class="truncate block font-medium">
                            {match.atlet2_nama || (match.atlet2_id ? `Atlet #${match.atlet2_id}` : 'BYE')}
                          </span>
                          {#if match.atlet2_kontingen}
                            <span class="text-[9px] text-zinc-500 truncate shrink-0">
                              ({match.atlet2_kontingen})
                            </span>
                          {/if}
                        </div>
                        <span class="font-mono text-xs font-bold shrink-0">{match.score2 || '0'}</span>
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
    :global(body) {
      background: #ffffff !important;
      color: #000000 !important;
    }
    :global(.no-print) {
      display: none !important;
    }
    .print-sheet {
      background: #ffffff !important;
      color: #000000 !important;
      padding: 0 !important;
      overflow: visible !important;
    }
    .match-card {
      background: #ffffff !important;
      border: 1px solid #334155 !important;
      color: #000000 !important;
      box-shadow: none !important;
    }
  }
</style>
