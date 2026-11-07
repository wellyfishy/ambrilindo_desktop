<script lang="ts">
  import type { DetailBaganItem } from "$lib/types";
  import { FORKI_LOGO_DATA_URL, AMBRILINDO_LOGO_DATA_URL } from "$lib/assets_base64";

  interface Props {
    baganId?: number;
    baganKode?: string | null;
    categoryName: string;
    discipline?: string;
    poolCount?: number;
    athletesCount?: number;
    tatamiNumber?: number | string;
    venue?: string;
    matches: DetailBaganItem[];
    layoutTemplate?: string | null;
    showHeader?: boolean;
    interactive?: boolean;
    isDarkMode?: boolean;
    onMatchClick?: (m: DetailBaganItem) => void;
  }

  let {
    baganId,
    baganKode,
    categoryName,
    discipline = "kumite",
    poolCount = 1,
    athletesCount = 4,
    tatamiNumber,
    venue = "Gedung Olahraga SMPN & SMAN 1 Samarinda",
    matches = [],
    layoutTemplate = "quadrant",
    showHeader = false,
    interactive = false,
    isDarkMode = false,
    onMatchClick = () => {},
  }: Props = $props();

  let derivedDiscipline = $derived.by(() => {
    if (discipline) return discipline.toLowerCase();
    if (categoryName.toLowerCase().includes("kata")) return "kata";
    return "kumite";
  });

  let derivedGroupCount = $derived.by(() => {
    if (poolCount && poolCount > 0) return poolCount;
    const maxFinalUrutan = Math.max(
      ...matches.filter((m) => m.round === 2).map((m) => m.urutan ?? 0),
      0
    );
    if (maxFinalUrutan > 0) return maxFinalUrutan;
    const maxSemiUrutan = Math.max(
      ...matches.filter((m) => m.round === 1).map((m) => m.urutan ?? 0),
      0
    );
    if (maxSemiUrutan > 0) return Math.ceil(maxSemiUrutan / 2);
    return 1;
  });

  let groups = $derived.by(() => {
    const count = Math.min(6, Math.max(1, derivedGroupCount));
    const result = [];
    for (let g = 1; g <= count; g++) {
      const semi1 = matches.find((m) => m.round === 1 && m.urutan === 2 * g - 1) || null;
      const semi2 = matches.find((m) => m.round === 1 && m.urutan === 2 * g) || null;
      const finalMatch = matches.find((m) => m.round === 2 && m.urutan === g) || null;
      const isCompleted = !!(finalMatch && finalMatch.selesai && finalMatch.pemenang);
      result.push({
        groupNum: g,
        poolLabel: String.fromCharCode(64 + g),
        matches: { semi1, semi2, final: finalMatch },
        isCompleted,
      });
    }
    return result;
  });

  let totalSlots = $derived(groups.length <= 4 ? 4 : 6);
  let effectiveLayout = $derived(layoutTemplate === "bands" ? "bands" : "quadrant");
</script>

<div class="festival-sheet-container w-full h-full flex flex-col justify-between {isDarkMode ? 'dark-mode text-white' : 'text-slate-900'}">
  {#if showHeader}
    <!-- Top Tournament Header Banner (Exact match to BaganView) -->
    <div class="bracket-banner relative flex items-center justify-center pb-2 pt-1 border-b-2 {isDarkMode ? 'border-white/[0.15]' : 'border-slate-900'} mb-1.5 shrink-0">
      <!-- Top-Left Official Logo Cluster: FORKI & Ambrilindo -->
      <div class="banner-logos-left absolute left-1 top-1.5 flex items-center space-x-2 shrink-0 z-10">
        <img
          src={FORKI_LOGO_DATA_URL}
          alt="Logo FORKI"
          class="banner-logo-forki h-8 w-auto max-w-[34px] max-h-8 object-contain shrink-0"
        />
        <div class="banner-logo-divider h-6 w-[1.2px] bg-slate-300 shrink-0"></div>
        <img
          src={AMBRILINDO_LOGO_DATA_URL}
          alt="Logo Ambrilindo"
          class="banner-logo-ambrilindo h-5 w-auto max-w-[100px] max-h-5 object-contain shrink-0"
        />
      </div>

      <!-- Center Category & Tournament Info (True Center) -->
      <div class="banner-title-wrapper text-center w-full px-4 sm:px-28 md:px-36">
        <h1 class="text-base md:text-lg font-black {isDarkMode ? 'text-white' : 'text-slate-950'} tracking-tight uppercase leading-tight truncate">
          {#if baganKode}
            ({baganKode})
          {/if}
          {categoryName}
        </h1>
        <!-- Pure Judul Saja: Subtext festival dihilangkan sesuai instruksi operator -->
      </div>
    </div>
  {/if}

  <!-- Main Canvas Wrapper -->
  <div class="festival-canvas-wrapper flex-1 min-h-0 w-full overflow-hidden flex flex-col">
    {#if effectiveLayout === "bands"}
      <!-- Template B: Full-Width Horizontal Bands -->
      <div class="festival-bands-grid flex flex-col justify-between flex-1 gap-1 border-2 {isDarkMode ? 'border-zinc-700 bg-[#101216]' : 'border-slate-900 bg-white'} rounded-lg p-1 overflow-hidden">
        {#each groups as grp}
          <div class="group-band flex flex-col justify-between p-1.5 {isDarkMode ? 'bg-[#15171e]' : 'bg-white'} flex-1 overflow-hidden border border-slate-200 dark:border-zinc-800 rounded">
            <!-- Group Header Line -->
            <div class="group-header-line flex items-center justify-between px-2 py-0.5 border-b-2 {isDarkMode ? 'border-zinc-700 bg-white/[0.02]' : 'border-slate-900 bg-slate-50'} mb-1 h-5 shrink-0">
              <div class="flex items-center space-x-1.5">
                <span class="group-header-badge w-4 h-4 rounded-full {isDarkMode ? 'bg-zinc-800 border border-white/40 text-white' : 'bg-slate-900 text-white'} text-[9px] font-black font-mono flex items-center justify-center shrink-0">
                  {grp.groupNum}
                </span>
                <span class="group-header-title text-[10px] font-black uppercase tracking-tight {isDarkMode ? 'text-white' : 'text-slate-950'}">
                  GRUP {grp.groupNum} (POOL {grp.poolLabel})
                </span>
              </div>
              <div class="flex items-center space-x-2">
                <span class="text-[9px] font-mono font-bold text-slate-500">4 ATLET</span>
                {#if grp.isCompleted}
                  <span class="no-print px-1.5 py-0.2 rounded font-mono font-bold text-[8px] uppercase bg-amber-500/20 text-amber-300 border border-amber-500/30 flex items-center gap-1">
                    <span>🏆</span>
                    <span>SELESAI</span>
                  </span>
                {/if}
              </div>
            </div>

            <!-- Band Layout 2-Column Bracket -->
            <div class="band-grid-2col flex items-stretch justify-between gap-0 px-0.5 py-0.5 flex-1 min-h-0">
              <!-- Semifinals -->
              <div class="col-wrap band-col-semi shrink-0 flex flex-col justify-between min-h-0">
                <div class="stage-header-line h-4 shrink-0 flex items-center justify-center pb-0.5 border-b-2 {isDarkMode ? 'border-white/[0.25]' : 'border-slate-900'} mb-1">
                  <span class="stage-header-title text-[8.5px] font-black uppercase tracking-tight {isDarkMode ? 'text-white' : 'text-slate-950'}">
                    SEMIFINAL
                  </span>
                </div>
                <div class="semi-cards-flex flex flex-col justify-between flex-1 min-h-0 gap-1">
                  {@render matchCard(grp.matches.semi1, `SEMIFINAL 1`)}
                  {@render matchCard(grp.matches.semi2, `SEMIFINAL 2`)}
                </div>
              </div>

              <!-- Connector Fork -->
              <div class="connector-fork-col band-connector-fork shrink-0 flex flex-col pointer-events-none relative">
                <div class="stage-header-spacer h-4 shrink-0 mb-1"></div>
                <div class="connector-track-flex flex flex-col justify-between flex-1 min-h-0 relative w-full">
                  <div class="connector-slot connector-slot-top shrink-0 flex items-center justify-start w-full relative">
                    <div class="connector-arm-left w-[calc(50%+1px)] -ml-[1px] border-b-[2px] {isDarkMode ? 'border-white/[0.35]' : 'border-slate-900'}"></div>
                  </div>
                  <div class="connector-spine-track absolute left-1/2 w-1/2 flex items-center pointer-events-none">
                    <div class="connector-spine-bar absolute inset-y-0 left-0 border-l-[2px] {isDarkMode ? 'border-white/[0.35]' : 'border-slate-900'}"></div>
                    <div class="connector-arm-right w-[calc(100%+1px)] border-b-[2px] border-amber-600"></div>
                  </div>
                  <div class="connector-slot connector-slot-bottom shrink-0 flex items-center justify-start w-full relative">
                    <div class="connector-arm-left w-[calc(50%+1px)] -ml-[1px] border-b-[2px] {isDarkMode ? 'border-white/[0.35]' : 'border-slate-900'}"></div>
                  </div>
                </div>
              </div>

              <!-- Final Juara -->
              <div class="col-wrap band-col-final shrink-0 flex flex-col justify-between min-h-0">
                <div class="stage-header-line h-4 shrink-0 flex items-center justify-center pb-0.5 border-b-2 {isDarkMode ? 'border-amber-400' : 'border-slate-900'} mb-1">
                  <span class="stage-header-title text-[8.5px] font-black uppercase tracking-tight text-amber-700 dark:text-amber-400">
                    PARTAI FINAL
                  </span>
                </div>
                <div class="final-card-centered flex flex-col justify-center flex-1 min-h-0">
                  {@render matchCard(grp.matches.final, `FINAL GRUP ${grp.groupNum}`, true)}
                </div>
              </div>
            </div>
          </div>
        {/each}
      </div>
    {:else}
      <!-- Template A: Uniform Quadrant Grid (Default 4, Max 6) -->
      <div class="festival-split-canvas w-full h-full border-2 {isDarkMode ? 'border-zinc-700 bg-[#101216]' : 'border-slate-900 bg-white'} rounded-lg overflow-hidden shadow-xs flex flex-col flex-1">
        <div class="festival-quadrants-grid {totalSlots === 4 ? 'festival-quadrants-grid-4' : 'festival-quadrants-grid-6'} grid grid-cols-2 divide-x-2 divide-y-2 {isDarkMode ? 'divide-zinc-700' : 'divide-slate-900'} flex-1">
          {#each Array.from({ length: totalSlots }) as _, idx}
            {@const gNum = idx + 1}
            {@const grp = groups.find((g) => g.groupNum === gNum)}
            {#if grp}
              <div class="group-quadrant flex flex-col justify-between p-1.5 md:p-2 {isDarkMode ? 'bg-[#15171e]' : 'bg-white'} flex-1 overflow-hidden">
                <!-- Quadrant Top Header Bar -->
                <div class="group-header-line flex items-center justify-between px-2 py-0.5 border-b-2 {isDarkMode ? 'border-zinc-700 bg-white/[0.02]' : 'border-slate-900 bg-slate-50'} mb-1 h-5 md:h-6 shrink-0">
                  <div class="flex items-center space-x-1.5 md:space-x-2">
                    <span class="group-header-badge w-4 h-4 rounded-full {isDarkMode ? 'bg-zinc-800 border border-white/40 text-white' : 'bg-slate-900 text-white'} text-[9px] font-black font-mono flex items-center justify-center shrink-0">
                      {grp.groupNum}
                    </span>
                    <span class="group-header-title text-[10px] md:text-[10.5px] font-black uppercase tracking-tight {isDarkMode ? 'text-white' : 'text-slate-950'}">
                      GRUP {grp.groupNum} (POOL {grp.poolLabel})
                    </span>
                  </div>

                  <div class="flex items-center space-x-2">
                    <span class="text-[9px] font-mono font-bold text-slate-500">4 ATLET</span>
                    {#if grp.isCompleted}
                      <span class="no-print px-1.5 py-0.2 rounded font-mono font-bold text-[8px] uppercase bg-amber-500/20 text-amber-300 border border-amber-500/30 flex items-center gap-1">
                        <span>🏆</span>
                        <span>SELESAI</span>
                      </span>
                    {/if}
                  </div>
                </div>

                <!-- 2-Stage Tournament Bracket (Semifinal -> Fork -> Final) -->
                <div class="quadrant-grid-2col flex items-stretch justify-between gap-0 px-0.5 py-0.5 flex-1 min-h-0">
                  <!-- Semifinals -->
                  <div class="col-wrap col-semi flex-1 shrink-0 flex flex-col justify-between min-h-0">
                    <div class="stage-header-line h-4 shrink-0 flex items-center justify-center pb-0.5 border-b-2 {isDarkMode ? 'border-white/[0.25]' : 'border-slate-900'} mb-1">
                      <div class="flex items-center justify-center space-x-1 text-center w-full">
                        <span class="stage-header-badge w-3 h-3 rounded-full {isDarkMode ? 'bg-zinc-800 border border-white/40 text-white' : 'bg-slate-900 text-white'} text-[7.5px] font-black font-mono flex items-center justify-center shrink-0">
                          1
                        </span>
                        <span class="stage-header-title text-[8.5px] font-black uppercase tracking-tight text-center {isDarkMode ? 'text-white' : 'text-slate-950'}">
                          SEMIFINAL
                        </span>
                      </div>
                    </div>

                    <div class="semi-cards-flex flex flex-col justify-between flex-1 min-h-0 gap-1">
                      {@render matchCard(grp.matches.semi1, `SEMIFINAL 1`)}
                      {@render matchCard(grp.matches.semi2, `SEMIFINAL 2`)}
                    </div>
                  </div>

                  <!-- Connector Fork -->
                  <div class="connector-fork-col quadrant-connector-fork shrink-0 flex flex-col pointer-events-none relative">
                    <div class="stage-header-spacer h-4 shrink-0 mb-1"></div>
                    <div class="connector-track-flex flex flex-col justify-between flex-1 min-h-0 relative w-full">
                      <div class="connector-slot connector-slot-top shrink-0 flex items-center justify-start w-full relative">
                        <div class="connector-arm-left w-[calc(50%+1px)] -ml-[1px] border-b-[2px] {isDarkMode ? 'border-white/[0.35]' : 'border-slate-900'}"></div>
                      </div>
                      <div class="connector-spine-track absolute left-1/2 w-1/2 flex items-center pointer-events-none">
                        <div class="connector-spine-bar absolute inset-y-0 left-0 border-l-[2px] {isDarkMode ? 'border-white/[0.35]' : 'border-slate-900'}"></div>
                        <div class="connector-arm-right w-[calc(100%+1px)] border-b-[2px] border-amber-600"></div>
                      </div>
                      <div class="connector-slot connector-slot-bottom shrink-0 flex items-center justify-start w-full relative">
                        <div class="connector-arm-left w-[calc(50%+1px)] -ml-[1px] border-b-[2px] {isDarkMode ? 'border-white/[0.35]' : 'border-slate-900'}"></div>
                      </div>
                    </div>
                  </div>

                  <!-- Final Juara -->
                  <div class="col-wrap col-final flex-1 shrink-0 flex flex-col justify-between min-h-0">
                    <div class="stage-header-line h-4 shrink-0 flex items-center justify-center pb-0.5 border-b-2 {isDarkMode ? 'border-amber-400' : 'border-slate-900'} mb-1">
                      <div class="flex items-center justify-center space-x-1 text-center w-full">
                        <span class="stage-header-badge w-3 h-3 rounded-full {isDarkMode ? 'bg-amber-400 text-slate-950' : 'bg-slate-900 text-white'} text-[7.5px] font-black font-mono flex items-center justify-center shrink-0">
                          2
                        </span>
                        <span class="stage-header-title text-[8.5px] font-black uppercase tracking-tight text-center {isDarkMode ? 'text-amber-400' : 'text-slate-950'}">
                          FINAL JUARA
                        </span>
                      </div>
                    </div>

                    <div class="final-card-centered flex flex-col justify-center flex-1 min-h-0">
                      {@render matchCard(grp.matches.final, `FINAL GRUP ${gNum}`, true)}
                    </div>
                  </div>
                </div>
              </div>
            {:else}
              <!-- Clean Empty Quadrant Slot with official text -->
              <div class="group-quadrant group-quadrant-empty flex flex-col items-center justify-center p-4 {isDarkMode ? 'bg-[#12141a]' : 'bg-slate-50/80'} flex-1 h-full w-full overflow-hidden border border-slate-300">
                <div class="flex flex-col items-center justify-center text-center w-full">
                  <span class="empty-group-badge w-7 h-7 rounded-full border-2 {isDarkMode ? 'border-zinc-400 text-white bg-zinc-800' : 'border-slate-800 text-slate-950 bg-slate-200'} flex items-center justify-center font-mono font-black text-xs mb-1.5 shadow-xs">
                    {gNum}
                  </span>
                  <span class="empty-group-title text-[10.5px] font-mono font-black {isDarkMode ? 'text-white' : 'text-slate-950'} uppercase tracking-wide">
                    [GRUP {gNum} TIDAK DIGUNAKAN]
                  </span>
                  <p class="empty-group-desc text-[9px] font-bold {isDarkMode ? 'text-zinc-300' : 'text-slate-700'} mt-1">
                    Peserta terakomodasi pada grup sebelumnya
                  </p>
                </div>
              </div>
            {/if}
          {/each}
        </div>
      </div>
    {/if}
  </div>
</div>

<!-- Match Card Snippet -->
{#snippet matchCard(m: DetailBaganItem | null, stageLabel: string, isFinal: boolean = false)}
  {#if m}
    {@const isWaitingFinalists = isFinal && (!m.atlet1_id || !m.atlet2_id)}
    {@const isByeMatch = (!m.atlet1_id && !!m.atlet2_id) || (!!m.atlet1_id && !m.atlet2_id)}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div
      onclick={() => {
        if (!interactive) return;
        if (isWaitingFinalists) {
          alert('Pertandingan final belum dapat dibuka karena pemenang semifinal belum ditentukan.');
          return;
        }
        if (!isByeMatch) {
          onMatchClick(m);
        }
      }}
      class="match-card festival-match-card w-full {interactive ? (isWaitingFinalists ? 'cursor-not-allowed opacity-85' : isByeMatch ? 'cursor-default' : 'cursor-pointer hover:ring-2 hover:ring-amber-500/80') : 'cursor-default'} transition-all {isDarkMode ? 'bg-[#181a20] border-zinc-700 hover:border-zinc-500' : 'bg-white border-slate-400/90 hover:border-slate-800'} border rounded-md overflow-hidden shadow-xs relative z-10 {isFinal ? (isDarkMode ? 'border-amber-500/80 shadow-[0_0_10px_rgba(245,158,11,0.15)]' : 'border-amber-500 shadow-xs') : ''}"
      title={interactive ? (isWaitingFinalists ? 'Menunggu pemenang semifinal' : isByeMatch ? 'Partai BYE (Lolos Otomatis)' : 'Klik untuk membuka Kontrol Pertandingan (Control Panel & Scoreboard)') : ''}
    >
      <!-- Header bar -->
      <div class="card-header-bar px-1.5 py-[1px] {isDarkMode ? 'bg-white/[0.04] border-zinc-700' : 'bg-slate-100 border-slate-300'} border-b flex items-center justify-between text-[7.5px] font-mono font-bold {isDarkMode ? 'text-zinc-400' : 'text-slate-700'}">
        <span class="truncate">{stageLabel} {m.kode ? `(${m.kode})` : ''}</span>
        <div class="flex items-center space-x-1 tabular-nums">
          {#if m.selesai}
            <span class="text-emerald-500 dark:text-emerald-400 font-black">✓ SELESAI</span>
          {/if}
          <span>#{m.id}</span>
        </div>
      </div>

      <!-- AKA (Red) -->
      <div class="player-row {m.pemenang === '1' ? (isDarkMode ? 'bg-red-950/40 border-l-4 border-red-500' : 'bg-[#fee2e2] border-l-4 border-red-600') : (isDarkMode ? 'bg-[#181a20]' : 'bg-white')} flex items-center justify-between px-1.5 py-[1px] min-h-[18px]">
        <div class="flex items-center space-x-1 mr-1 min-w-0 flex-1">
          <span class="side-pill-aka w-[22px] inline-flex items-center justify-center text-[7px] font-mono font-black text-white bg-[#dc2626] py-[0.5px] rounded shrink-0">AKA</span>
          <div class="min-w-0 flex-1 leading-tight">
            <span class="player-name-text truncate block text-[8.5px] font-black {isDarkMode ? 'text-white' : 'text-black'}">
              {m.atlet1_nama || (m.atlet1_id ? `Atlet #${m.atlet1_id}` : (isFinal ? '...' : 'BYE'))}
            </span>
            <div class="player-sub-text truncate min-w-0 text-[7px] font-bold {isDarkMode ? 'text-zinc-400' : 'text-slate-800'}">
              {m.atlet1_kontingen || '-'}
            </div>
          </div>
        </div>
        <div class="shrink-0 flex items-center">
          <span class="score-box min-w-[18px] h-[15px] px-0.5 font-mono text-[8.5px] flex items-center justify-center rounded border {m.pemenang === '1' ? (isDarkMode ? 'bg-amber-400 text-black border-amber-300 font-black' : 'bg-[#fde047] text-black border-amber-500 font-black') : (isDarkMode ? 'bg-white/[0.05] border-zinc-700 text-zinc-300 font-bold' : 'bg-slate-100 border-slate-400 text-slate-800 font-bold')}">
            {m.score1 ?? '-'}
          </span>
        </div>
      </div>

      <!-- AO (Blue) -->
      <div class="player-row {m.pemenang === '2' ? (isDarkMode ? 'bg-blue-950/40 border-l-4 border-blue-500' : 'bg-[#dbeafe] border-l-4 border-blue-600') : (isDarkMode ? 'bg-[#181a20]' : 'bg-white')} flex items-center justify-between px-1.5 py-[1px] min-h-[18px] border-t {isDarkMode ? 'border-zinc-800' : 'border-slate-200'}">
        <div class="flex items-center space-x-1 mr-1 min-w-0 flex-1">
          <span class="side-pill-ao w-[22px] inline-flex items-center justify-center text-[7px] font-mono font-black text-white bg-[#2563eb] py-[0.5px] rounded shrink-0">AO</span>
          <div class="min-w-0 flex-1 leading-tight">
            <span class="player-name-text truncate block text-[8.5px] font-black {isDarkMode ? 'text-white' : 'text-black'}">
              {m.atlet2_nama || (m.atlet2_id ? `Atlet #${m.atlet2_id}` : (isFinal ? '...' : 'BYE'))}
            </span>
            <div class="player-sub-text truncate min-w-0 text-[7px] font-bold {isDarkMode ? 'text-zinc-400' : 'text-slate-800'}">
              {m.atlet2_kontingen || '-'}
            </div>
          </div>
        </div>
        <div class="shrink-0 flex items-center">
          <span class="score-box min-w-[18px] h-[15px] px-0.5 font-mono text-[8.5px] flex items-center justify-center rounded border {m.pemenang === '2' ? (isDarkMode ? 'bg-amber-400 text-black border-amber-300 font-black' : 'bg-[#fde047] text-black border-amber-500 font-black') : (isDarkMode ? 'bg-white/[0.05] border-zinc-700 text-zinc-300 font-bold' : 'bg-slate-100 border-slate-400 text-slate-800 font-bold')}">
            {m.score2 ?? '-'}
          </span>
        </div>
      </div>
    </div>
  {:else}
    <div class="empty-slot festival-empty-slot w-full border border-dashed {isDarkMode ? 'border-zinc-700 text-zinc-400 bg-white/[0.02]' : 'border-slate-400 text-slate-600 bg-white/40'} rounded-md h-[40px] flex items-center justify-center text-[8.5px] font-mono font-bold">
      Kosong
    </div>
  {/if}
{/snippet}

<style>
  /* Screen & Container Layout */
  .festival-sheet-container {
    box-sizing: border-box;
  }

  /* Screen Banner Styles (when showHeader=true) */
  .bracket-banner {
    position: relative;
    border-bottom: 2px solid #0f172a;
    padding-bottom: 4px;
    height: 14mm;
    box-sizing: border-box;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
  }

  .banner-logos-left {
    position: absolute;
    left: 4px;
    top: 50%;
    transform: translateY(-50%);
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .banner-logo-forki {
    height: 30px;
    width: auto;
    object-fit: contain;
  }

  .banner-logo-divider {
    height: 20px;
    width: 1.2px;
    background: #cbd5e1;
  }

  .banner-logo-ambrilindo {
    height: 16px;
    width: auto;
    object-fit: contain;
  }

  .banner-title-wrapper {
    width: 100%;
    text-align: center;
    padding-left: 120px;
    padding-right: 120px;
  }

  .festival-bands-grid {
    height: 100%;
  }

  .festival-quadrants-grid {
    height: 100%;
  }

  .festival-quadrants-grid-4 {
    grid-template-rows: repeat(2, 1fr) !important;
  }

  .festival-quadrants-grid-6 {
    grid-template-rows: repeat(3, 1fr) !important;
  }

  /* Band 2-Column Split */
  .band-grid-2col {
    display: flex;
    align-items: stretch;
    justify-content: space-between;
    height: 100%;
  }

  .band-col-semi {
    width: 48%;
  }

  .band-col-final {
    width: 46%;
  }

  .band-connector-fork {
    width: 6%;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }

  /* Quadrant 2-Column Split */
  .quadrant-grid-2col {
    display: flex;
    align-items: stretch;
    justify-content: space-between;
    height: 100%;
  }

  .quadrant-connector-fork {
    width: 20px;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }

  /* Connector Spine & Tracks */
  .connector-track-flex {
    height: 100%;
    position: relative;
  }

  .connector-slot-top,
  .connector-slot-bottom {
    height: 50%;
  }

  .connector-spine-track {
    top: 25%;
    bottom: 25%;
  }

  /* Print Styles - High Contrast A4 Landscape (100% Fill, Zero Overflow) */
  @media print {
    .festival-sheet-container {
      width: 100% !important;
      height: 100% !important;
      color: #000 !important;
      background: #fff !important;
    }

    .bracket-banner {
      position: relative !important;
      border-bottom: 2px solid #000000 !important;
      border-top: none !important;
      border-left: none !important;
      border-right: none !important;
      padding: 1mm 3mm !important;
      margin-bottom: 1.5mm !important;
      height: 15mm !important;
      min-height: 14mm !important;
      max-height: 15.5mm !important;
      box-sizing: border-box !important;
      display: flex !important;
      align-items: center !important;
      justify-content: center !important;
      width: 100% !important;
      background: #ffffff !important;
    }

    .banner-logos-left {
      position: absolute !important;
      left: 2mm !important;
      top: 2.5mm !important;
      transform: none !important;
      bottom: auto !important;
      display: flex !important;
      align-items: center !important;
      gap: 1.8mm !important;
      background: transparent !important;
      border: none !important;
      padding: 0 !important;
      box-shadow: none !important;
      z-index: 10 !important;
    }

    .banner-logo-forki {
      height: 9.5mm !important;
      max-height: 9.5mm !important;
      max-width: 11mm !important;
      object-fit: contain !important;
      display: block !important;
    }

    .banner-logo-divider {
      height: 7.5mm !important;
      width: 1.2px !important;
      background: #94a3b8 !important;
      display: block !important;
    }

    .banner-logo-ambrilindo {
      height: 5.5mm !important;
      max-height: 5.5mm !important;
      max-width: 25mm !important;
      object-fit: contain !important;
      display: block !important;
    }

    .banner-title-wrapper {
      width: 100% !important;
      text-align: center !important;
      padding-left: 44mm !important;
      padding-right: 44mm !important;
    }

    .banner-title-wrapper h1 {
      font-size: 12.5pt !important;
      font-weight: 950 !important;
      line-height: 1.15 !important;
      color: #000000 !important;
      text-transform: uppercase !important;
      margin: 0 !important;
      padding: 0 !important;
      letter-spacing: 0.3px !important;
      text-align: center !important;
    }


    .festival-split-canvas,
    .festival-bands-grid {
      border: 2px solid #000 !important;
      background: #fff !important;
    }

    .group-quadrant,
    .group-band {
      background: #fff !important;
      border-color: #000 !important;
    }

    .group-header-line {
      border-bottom: 2px solid #000 !important;
      background: #f8fafc !important;
    }

    .group-header-title {
      color: #000 !important;
    }

    .stage-header-line {
      border-bottom: 2px solid #000 !important;
    }

    .stage-header-title {
      color: #000 !important;
    }

    .festival-match-card {
      border: 1px solid #000 !important;
      background: #fff !important;
    }

    .player-row {
      color: #000 !important;
    }

    .player-name-text {
      color: #000 !important;
    }

    .side-pill-aka {
      background: #dc2626 !important;
      color: #fff !important;
      -webkit-print-color-adjust: exact !important;
      print-color-adjust: exact !important;
    }

    .side-pill-ao {
      background: #2563eb !important;
      color: #fff !important;
      -webkit-print-color-adjust: exact !important;
      print-color-adjust: exact !important;
    }

    .score-box {
      border: 1px solid #000 !important;
      background: #f1f5f9 !important;
      color: #000 !important;
      -webkit-print-color-adjust: exact !important;
      print-color-adjust: exact !important;
    }

    .connector-arm-left,
    .connector-spine-bar,
    .connector-arm-right {
      border-color: #000 !important;
      -webkit-print-color-adjust: exact !important;
      print-color-adjust: exact !important;
    }

    .group-quadrant-empty {
      border: 1px dashed #64748b !important;
      background: #f8fafc !important;
    }

    .empty-group-badge {
      border: 2px solid #000 !important;
      color: #000 !important;
      background: #e2e8f0 !important;
    }

    .empty-group-title {
      color: #000 !important;
    }
  }
</style>
