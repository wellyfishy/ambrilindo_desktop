<!--
  Tournament (elimination) bracket tree: the one template used by the bagan channel,
  the results booklet and the tatami booklet. Drawn at its native size
  (TREE_WIDTH x TREE_HEIGHT px); callers scale it.

  theme: "dark" / "light" follow the channel's theme toggle; "paper" is the print
  version: light palette, no hover/click and no screen-only status chips.
-->
<script lang="ts">
  import type { DetailBaganItem, EnrichedAtletItem, TatamiItem } from "$lib/types";
  import { isMatchLive, getRunningTatamiForMatch } from "$lib/liveSync";
  import {
    TREE_HEIGHT,
    TREE_COL_WIDTH,
    treeWidthFor,
    computeConnectorLines,
    elimRoundsOf,
    getMatchForSlot,
    getRoundBadge,
    getRoundTitle,
    getSlotCountForRound,
    getSlotHeightClass,
    getSlotHeightPx,
    MATCH_CARD_HEIGHT,
    type BracketMeta,
    type PodiumResult,
  } from "$lib/bracket_layout";

  interface Props {
    matches: DetailBaganItem[];
    meta: BracketMeta;
    podium: PodiumResult;
    theme?: "dark" | "light" | "paper";
    interactive?: boolean;
    logoMap?: Record<string, string>;
    isEditMode?: boolean;
    editedMatchups?: Record<number, { atlet1_id: number | null; atlet2_id: number | null }>;
    categoryAthletes?: EnrichedAtletItem[];
    tatamis?: TatamiItem[];
    liveSyncVersion?: number;
    onMatchClick?: (m: DetailBaganItem) => void;
    gridEl?: HTMLDivElement | null;
    /** wider columns for a wide screen (live drawing on a TV); default = the print width */
    colWidth?: number;
    /** podium card + Ambrilindo logo in the final column (hidden in live drawing) */
    showPodium?: boolean;
    /**
     * Bigger match cards (live drawing on a TV). Each card grows up to this factor, as far
     * as its slot allows (round 1 slots are the tightest), and keeps the column width.
     */
    cardScale?: number;
    /** no score boxes (drawing: nothing is scored yet, the names get the width) */
    hideScores?: boolean;
    /** leave out this many leading rounds (live drawing: rounds with only BYEs) */
    skipRounds?: number;
    /** show each athlete's kata before the bout is finished (LO Kata) */
    showKata?: boolean;
    /** outline this match (e.g. the one open in the control panel) */
    highlightMatchId?: number | null;
    /** tooltip of a clickable card (default: opens the control panel) */
    clickHint?: string;
  }

  let {
    matches,
    meta,
    podium,
    theme = "light",
    interactive = false,
    logoMap = {},
    isEditMode = false,
    editedMatchups = $bindable({}),
    categoryAthletes = [],
    tatamis = [],
    liveSyncVersion = 0,
    onMatchClick,
    gridEl = $bindable(null),
    colWidth = TREE_COL_WIDTH,
    showPodium = true,
    cardScale = 1,
    hideScores = false,
    skipRounds = 0,
    showKata = false,
    highlightMatchId = null,
    clickHint = "Klik untuk membuka Kontrol Pertandingan (Control Panel)",
  }: Props = $props();

  /** Card size for a round: never taller than its slot, so connector lines stay on centre */
  function cardZoom(roundNum: number): number {
    if (cardScale <= 1) return 1;
    return Math.max(1, Math.min(cardScale, (getSlotHeightPx(roundNum) * 0.92) / MATCH_CARD_HEIGHT));
  }

  let isDarkMode = $derived(theme === "dark");
  let elimRounds = $derived(elimRoundsOf(matches));
  // Repechage brackets keep their own layout; elimination trees may skip empty early rounds
  let skip = $derived(meta.isRepechageBagan ? 0 : Math.max(0, Math.min(skipRounds, elimRounds.length - 1)));
  let shownRounds = $derived(elimRounds.filter(([r]) => r > skip));
  let connectorLines = $derived(computeConnectorLines(elimRounds.length, meta.isRepechageBagan, colWidth, skip));
  let treeWidth = $derived(treeWidthFor(colWidth, 4 - skip));
</script>

<!-- Dynamic SVG Connector Canvas -->
<svg
    class="bracket-svg absolute inset-0 pointer-events-none w-full h-full z-0 overflow-visible"
  viewBox="0 0 {treeWidth} {TREE_HEIGHT}"
>
  {#each connectorLines as line}
    <line
      x1={line.x1}
      y1={line.y1}
      x2={line.x2}
      y2={line.y2}
      class="connector-line"
      stroke={isDarkMode ? "rgba(255, 255, 255, 0.75)" : "#0f172a"}
      stroke-width="1.6"
      stroke-linecap="round"
    />
  {/each}
</svg>

<!-- Bracket Columns Grid Container -->
<div bind:this={gridEl} class="bracket-grid flex items-start gap-8 relative z-10 min-w-max pb-2 pr-2">
  {#each shownRounds as [roundNum, roundMatches]}
    {@const slotCount = getSlotCountForRound(roundNum)}
    {@const slotHeightClass = getSlotHeightClass(roundNum)}
    {@const zoom = cardZoom(roundNum)}

    <div class="round-col round-col-elim shrink-0 flex flex-col relative" style="width: {colWidth}px;">
      <!-- Column Header with Black/Dark Circle Step Badge (Centered) -->
      <div class="col-header-line h-8 shrink-0 flex items-center justify-center pb-1 border-b-2 {isDarkMode ? 'border-white/[0.25]' : 'border-slate-900'} mb-1">
        <div class="flex items-center justify-center space-x-2 text-center w-full">
          <span class="col-header-badge w-5 h-5 rounded-full {isDarkMode ? 'bg-zinc-800 border border-white/40 text-white' : 'bg-slate-900 text-white'} text-[10px] font-black font-mono flex items-center justify-center shrink-0">
            {getRoundBadge(meta, roundNum, elimRounds.length)}
          </span>
          <span class="col-header-title text-[12px] font-black uppercase tracking-tight text-center {isDarkMode ? 'text-white' : 'text-slate-950'}">
            {getRoundTitle(meta, roundNum, elimRounds.length)}
          </span>
        </div>
      </div>

      <!-- Matches Slots in this Round (Exact 656px height across rounds for perfect alignment) -->
      <div class="round-slots flex flex-col h-[656px] relative">
        {#each Array.from({ length: slotCount }, (_, i) => i + 1) as slotNum}
          {@const match = getMatchForSlot(roundMatches, slotNum)}
          <div data-slot={slotNum} class="{slotHeightClass} flex items-center justify-center">
            {#if match}
              <div
                class="match-card w-full {!interactive ? '' : isEditMode ? 'cursor-default ring-1 ring-amber-500/50' : 'cursor-pointer hover:ring-2 hover:ring-amber-500/80'} transition-all {isDarkMode ? 'bg-[#181a20] border-zinc-700 hover:border-zinc-500' : 'bg-white border-slate-400/90 hover:border-slate-800'} border rounded-md overflow-hidden shadow-xs relative z-10 {roundNum === 4 ? (isDarkMode ? 'border-amber-500/80 shadow-[0_0_15px_rgba(245,158,11,0.15)]' : 'border-amber-500 shadow-md') : ''} {highlightMatchId != null && match.id === highlightMatchId ? 'ring-2 ring-emerald-400 shadow-[0_0_14px_rgba(52,211,153,0.45)]' : ''}"
                onclick={() => { if (interactive && !isEditMode) onMatchClick?.(match); }}
                role="button"
                tabindex="0"
                onkeydown={(e) => { if (interactive && e.key === 'Enter' && !isEditMode) onMatchClick?.(match); }}
                style={zoom !== 1 ? `zoom: ${zoom}; width: ${colWidth / zoom}px;` : undefined}
                title={!interactive ? undefined : isEditMode ? "Mode Edit: Pilih atlet peserta partai" : clickHint}
              >
                <!-- Match Card Header -->
                <div class="card-header-bar flex items-center justify-between px-1.5 h-[13px] {isDarkMode ? 'bg-[#101215] border-white/[0.12] text-zinc-100' : 'bg-slate-200 border-slate-400 text-slate-950'} border-b text-[8px] font-black font-mono tracking-wider">
                  <span class="flex items-center space-x-1">
                    {#if meta.isRepechageBagan}
                      <span class="font-black {match.urutan === 1 ? 'text-amber-400' : 'text-blue-400'}">
                        {match.urutan === 1 ? 'POOL A' : 'POOL B'} • {roundNum === elimRounds.length ? 'PERUNGGU' : `M${match.urutan || slotNum}`}
                      </span>
                    {:else if roundNum === 4}
                      <span class="final-title {isDarkMode ? 'text-amber-300' : 'text-amber-800'} font-black flex items-center gap-1">
                        <svg class="w-2.5 h-2.5 fill-current shrink-0" viewBox="0 0 24 24">
                          <path d="M19 5h-2V3H7v2H5c-1.1 0-2 .9-2 2v1c0 2.55 1.92 4.63 4.39 4.94A5.01 5.01 0 0011 15.9V19H7v2h10v-2h-4v-3.1a5.01 5.01 0 003.61-2.96C19.08 12.63 21 10.55 21 8V7c0-1.1-.9-2-2-2zM5 8V7h2v3.82C5.84 10.4 5 9.3 5 8zm14 0c0 1.3-.84 2.4-2 2.82V7h2v1z"/>
                        </svg>
                        <span>{roundNum === 4 && meta.poolCount === 2 && !meta.isFinalBagan ? "SEMI FINAL" : "PARTAI FINAL"}</span>
                      </span>
                    {:else if roundNum === 3}
                      <span class="font-black">SEMI {match.urutan || slotNum}</span>
                    {:else}
                      <span class="font-black">MATCH {match.urutan || slotNum}</span>
                    {/if}
                  </span>
                  {#if !interactive}
                    <!-- paper: no screen status chips -->
                  {:else if isEditMode}
                    <span class="text-[7.5px] font-black text-amber-300 bg-amber-500/20 border border-amber-500/40 px-1 py-0.2 rounded tracking-wide">
                      EDIT
                    </span>
                  {:else if match.selesai}
                    <span class="text-[7.5px] font-black text-emerald-400 bg-emerald-950/40 border border-emerald-500/30 px-1 rounded">
                      SELESAI
                    </span>
                  {:else if liveSyncVersion >= 0 && isMatchLive(match.id, tatamis)}
                    {@const liveTNum = getRunningTatamiForMatch(match.id, tatamis) || (tatamis?.find((t) => t.id === match.assigned_tatami_id || t.tatami_number === match.assigned_tatami_id)?.tatami_number ?? (match.assigned_tatami_id && match.assigned_tatami_id < 20 ? match.assigned_tatami_id : 1))}
                    <span class="text-[7.5px] font-black text-white bg-red-600 px-1 py-0.2 rounded tracking-wide animate-pulse shadow-xs">
                      LIVE T{liveTNum}
                    </span>
                  {/if}
                </div>

                {#if isEditMode && editedMatchups[match.id]}
                  <!-- Edit Mode AKA Dropdown -->
                  <div class="player-row flex items-center px-1.5 py-1 min-h-[30px] {isDarkMode ? 'bg-[#14161a]' : 'bg-slate-50'}">
                    <div class="flex items-center space-x-1 w-full min-w-0">
                      <span class="side-pill-aka w-[22px] inline-flex items-center justify-center text-[7px] font-mono font-black text-white bg-[#dc2626] py-0.5 rounded shrink-0 tracking-wider shadow-xs">
                        AKA
                      </span>
                      <select
                        bind:value={editedMatchups[match.id].atlet1_id}
                        onclick={(e) => e.stopPropagation()}
                        class="w-full text-[10px] font-bold py-0.5 px-1 rounded border {isDarkMode ? 'bg-[#1e2024] text-zinc-100 border-amber-500/50 focus:border-amber-400' : 'bg-white text-slate-900 border-amber-500/70 focus:border-amber-600'} focus:outline-none focus:ring-1 focus:ring-amber-500 truncate"
                      >
                        <option value={null}>- Kosong / BYE -</option>
                        {#if match.atlet1_id && !categoryAthletes.some((a) => a.id === match.atlet1_id)}
                          <option value={match.atlet1_id}>
                            {match.atlet1_nama || `Atlet #${match.atlet1_id}`} ({match.atlet1_kontingen || '-'}) [Saat ini]
                          </option>
                        {/if}
                        {#each categoryAthletes as a (a.id)}
                          <option value={a.id}>
                            {a.nama_atlet} ({a.perguruan_nama || a.utusan_nama || '-'}{a.is_priority ? ' ★' : ''})
                          </option>
                        {/each}
                      </select>
                    </div>
                  </div>
                {:else}
                  <!-- Regular AKA (Red) Row -->
                  <div class="player-row {match.pemenang === '1' ? 'player-row-winner ' + (isDarkMode ? 'bg-amber-500/15 border-l-4 border-amber-500' : 'bg-[#fef3c7] border-l-4 border-amber-500') : (isDarkMode ? 'bg-transparent text-zinc-100' : 'bg-white text-slate-900')} flex items-center justify-between px-1.5 py-[0.5px] min-h-[17px] transition relative">
                    <div class="flex items-center space-x-1 mr-1 min-w-0 flex-1">
                      <span class="side-pill-aka w-[22px] inline-flex items-center justify-center text-[7px] font-mono font-black text-white bg-[#dc2626] py-[0.5px] rounded shrink-0 tracking-wider shadow-xs">
                        AKA
                      </span>
                      {#if (match.atlet1_logo && logoMap[match.atlet1_logo]) || match.atlet1_is_priority}
                        <div class="logo-star-wrapper flex items-center shrink-0 space-x-0.5 {match.atlet1_is_priority ? 'has-priority mr-[2px]' : ''}">
                          {#if match.atlet1_logo && logoMap[match.atlet1_logo]}
                            <img
                              src={logoMap[match.atlet1_logo]}
                              alt="Logo"
                              class="player-dojo-logo w-3 h-3 max-w-3 max-h-3 object-contain shrink-0"
                            />
                          {/if}
                          {#if match.atlet1_is_priority}
                            <span class="seed-star-icon inline-flex items-center text-amber-500 shrink-0" title="Unggulan / Seeded">
                              <svg class="w-2.5 h-2.5 fill-current" viewBox="2 2 20 20">
                                <path d="M12 17.27L18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z"/>
                              </svg>
                            </span>
                          {/if}
                        </div>
                      {/if}
                      <div class="min-w-0 flex-1 leading-tight">
                        <div class="flex items-center space-x-1 min-w-0">
                          {#if match.pemenang === '1'}
                            <span class="text-amber-400 shrink-0" title="Pemenang">
                              <svg class="w-2.5 h-2.5 fill-current" viewBox="0 0 24 24">
                                <path d="M5 16L3 5l5.5 5L12 4l3.5 6L21 5l-2 11H5zm14 3c0 .6-.4 1-1 1H6c-.6 0-1-.4-1-1v-1h14v1z"/>
                              </svg>
                            </span>
                          {/if}
                          <span class="player-name-text truncate block text-[8.5px] leading-[10px] {match.atlet1_nama ? (isDarkMode ? (match.pemenang === '1' ? 'font-black text-amber-300' : 'font-black text-white') : 'font-black text-black') : (isDarkMode ? 'italic text-zinc-400 font-bold' : 'italic text-slate-600 font-bold')}">
                            {match.atlet1_nama || (match.atlet1_id ? `Atlet #${match.atlet1_id}` : 'BYE')}
                          </span>
                          {#if match.vr1}
                            <span class="text-[7px] font-mono font-black px-0.5 bg-red-600 text-white rounded shrink-0">VR</span>
                          {/if}
                        </div>
                        <div class="player-sub-text truncate min-w-0 text-[7px] font-bold leading-[8.5px] mt-[0.5px] {isDarkMode ? 'text-zinc-200' : 'text-slate-900'}">
                          {match.atlet1_kontingen || (match.atlet1_nama ? '-' : '—')}
                        </div>
                        {#if (match.selesai || showKata) && match.kata1 && match.kata1 !== '0 - Blank'}
                          <div class="kata-row flex items-center leading-none mt-[0.5px]">
                            <span
                              class="kata-badge-aka text-[6px] font-black px-1 py-0 inline-flex items-center leading-[7.5px] {isDarkMode ? 'bg-red-500/25 text-red-200 border-red-500/50' : 'bg-red-100 text-red-900 border-red-400'} border rounded max-w-[130px] truncate"
                              title="Kata: {match.kata1}"
                            >
                              {match.kata1}
                            </span>
                          </div>
                        {/if}
                      </div>
                    </div>
                    <div class="shrink-0 flex items-center" class:hidden={hideScores}>
                      <span class="score-box min-w-[18px] h-[15px] px-0.5 font-mono text-[8.5px] flex items-center justify-center rounded border {match.pemenang === '1' ? (isDarkMode ? 'bg-amber-400/20 border-amber-400 text-amber-300 font-black' : 'bg-[#fde047] border-amber-500 text-amber-950 font-black') : (isDarkMode ? 'bg-white/[0.08] border-white/20 text-white font-black' : 'bg-slate-100 border-slate-400 text-black font-black')}">
                        {match.score1 ?? '-'}
                      </span>
                    </div>
                  </div>
                {/if}

                {#if isEditMode && editedMatchups[match.id]}
                  <!-- Edit Mode AO Dropdown -->
                  <div class="player-row flex items-center px-1.5 py-1 min-h-[30px] border-t {isDarkMode ? 'border-zinc-700/60 bg-[#14161a]' : 'border-slate-200 bg-slate-50'}">
                    <div class="flex items-center space-x-1 w-full min-w-0">
                      <span class="side-pill-ao w-[22px] inline-flex items-center justify-center text-[7px] font-mono font-black text-white bg-[#2563eb] py-0.5 rounded shrink-0 tracking-wider shadow-xs">
                        AO
                      </span>
                      <select
                        bind:value={editedMatchups[match.id].atlet2_id}
                        onclick={(e) => e.stopPropagation()}
                        class="w-full text-[10px] font-bold py-0.5 px-1 rounded border {isDarkMode ? 'bg-[#1e2024] text-zinc-100 border-amber-500/50 focus:border-amber-400' : 'bg-white text-slate-900 border-amber-500/70 focus:border-amber-600'} focus:outline-none focus:ring-1 focus:ring-amber-500 truncate"
                      >
                        <option value={null}>- Kosong / BYE -</option>
                        {#if match.atlet2_id && !categoryAthletes.some((a) => a.id === match.atlet2_id)}
                          <option value={match.atlet2_id}>
                            {match.atlet2_nama || `Atlet #${match.atlet2_id}`} ({match.atlet2_kontingen || '-'}) [Saat ini]
                          </option>
                        {/if}
                        {#each categoryAthletes as a (a.id)}
                          <option value={a.id}>
                            {a.nama_atlet} ({a.perguruan_nama || a.utusan_nama || '-'}{a.is_priority ? ' ★' : ''})
                          </option>
                        {/each}
                      </select>
                    </div>
                  </div>
                {:else}
                  <!-- Regular AO (Blue) Row -->
                  <div class="player-row {match.pemenang === '2' ? 'player-row-winner ' + (isDarkMode ? 'bg-amber-500/15 border-l-4 border-amber-500' : 'bg-[#fef3c7] border-l-4 border-amber-500') : (isDarkMode ? 'bg-transparent text-zinc-100' : 'bg-white text-slate-900')} flex items-center justify-between px-1.5 py-[0.5px] min-h-[17px] transition relative border-t {isDarkMode ? 'border-zinc-700/60' : 'border-slate-300'}">
                    <div class="flex items-center space-x-1 mr-1 min-w-0 flex-1">
                      <span class="side-pill-ao w-[22px] inline-flex items-center justify-center text-[7px] font-mono font-black text-white bg-[#2563eb] py-[0.5px] rounded shrink-0 tracking-wider shadow-xs">
                        AO
                      </span>
                      {#if (match.atlet2_logo && logoMap[match.atlet2_logo]) || match.atlet2_is_priority}
                        <div class="logo-star-wrapper flex items-center shrink-0 space-x-0.5 {match.atlet2_is_priority ? 'has-priority mr-[2px]' : ''}">
                          {#if match.atlet2_logo && logoMap[match.atlet2_logo]}
                            <img
                              src={logoMap[match.atlet2_logo]}
                              alt="Logo"
                              class="player-dojo-logo w-3 h-3 max-w-3 max-h-3 object-contain shrink-0"
                            />
                          {/if}
                          {#if match.atlet2_is_priority}
                            <span class="seed-star-icon inline-flex items-center text-amber-500 shrink-0" title="Unggulan / Seeded">
                              <svg class="w-2.5 h-2.5 fill-current" viewBox="2 2 20 20">
                                <path d="M12 17.27L18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z"/>
                              </svg>
                            </span>
                          {/if}
                        </div>
                      {/if}
                      <div class="min-w-0 flex-1 leading-tight">
                        <div class="flex items-center space-x-1 min-w-0">
                          {#if match.pemenang === '2'}
                            <span class="text-amber-400 shrink-0" title="Pemenang">
                              <svg class="w-2.5 h-2.5 fill-current" viewBox="2 2 20 20">
                                <path d="M5 16L3 5l5.5 5L12 4l3.5 6L21 5l-2 11H5zm14 3c0 .6-.4 1-1 1H6c-.6 0-1-.4-1-1v-1h14v1z"/>
                              </svg>
                            </span>
                          {/if}
                          <span class="player-name-text truncate block text-[8.5px] leading-[10px] {match.atlet2_nama ? (isDarkMode ? (match.pemenang === '2' ? 'font-black text-amber-300' : 'font-black text-white') : 'font-black text-black') : (isDarkMode ? 'italic text-zinc-400 font-bold' : 'italic text-slate-600 font-bold')}">
                            {match.atlet2_nama || (match.atlet2_id ? `Atlet #${match.atlet2_id}` : 'BYE')}
                          </span>
                          {#if match.vr2}
                            <span class="text-[7px] font-mono font-black px-0.5 bg-blue-600 text-white rounded shrink-0">VR</span>
                          {/if}
                        </div>
                        <div class="player-sub-text truncate min-w-0 text-[7px] font-bold leading-[8.5px] mt-[0.5px] {isDarkMode ? 'text-zinc-200' : 'text-slate-900'}">
                          {match.atlet2_kontingen || (match.atlet2_nama ? '-' : '—')}
                        </div>
                        {#if (match.selesai || showKata) && match.kata2 && match.kata2 !== '0 - Blank'}
                          <div class="kata-row flex items-center leading-none mt-[0.5px]">
                            <span
                              class="kata-badge-ao text-[6px] font-black px-1 py-0 inline-flex items-center leading-[7.5px] {isDarkMode ? 'bg-blue-500/25 text-blue-200 border-blue-500/50' : 'bg-blue-100 text-blue-900 border-blue-400'} border rounded max-w-[130px] truncate"
                              title="Kata: {match.kata2}"
                            >
                              {match.kata2}
                            </span>
                          </div>
                        {/if}
                      </div>
                    </div>
                    <div class="shrink-0 flex items-center" class:hidden={hideScores}>
                      <span class="score-box min-w-[18px] h-[15px] px-0.5 font-mono text-[8.5px] flex items-center justify-center rounded border {match.pemenang === '2' ? (isDarkMode ? 'bg-amber-400/20 border-amber-400 text-amber-300 font-black' : 'bg-[#fde047] border-amber-500 text-amber-950 font-black') : (isDarkMode ? 'bg-white/[0.08] border-white/20 text-white font-black' : 'bg-slate-100 border-slate-400 text-black font-black')}">
                        {match.score2 ?? '-'}
                      </span>
                    </div>
                  </div>
                {/if}
              </div>
            {:else}
              <div class="empty-slot w-full border border-dashed {isDarkMode ? 'border-zinc-700 text-zinc-400 bg-white/[0.02]' : 'border-slate-400 text-slate-600 bg-white/40'} rounded-md h-9 flex items-center justify-center text-[9px] font-mono font-bold">
                Kosong
              </div>
            {/if}
          </div>
        {/each}

        <!-- Column 4 Bottom Section: Ambrilindo Logo + Hasil Juara Podium Card -->
        {#if roundNum === 4 && showPodium}
          <div class="absolute bottom-0 right-0 w-full flex flex-col items-end z-20">
            <!-- Ambrilindo Brand Logo (Enlarged, Right-Aligned) -->
            <div class="mb-2.5 flex items-center justify-end w-full pr-1">
              <img
                src="/img/ambrilindo-logo.webp"
                alt="Ambrilindo"
                class="podium-brand-logo h-11 max-w-[180px] object-contain"
              />
            </div>

            <!-- Hasil Juara Podium Card -->
            <div class="podium-card w-full {isDarkMode ? 'bg-[#181a20] border-amber-500/80 shadow-lg' : 'bg-white border-amber-500 shadow-sm'} border-2 rounded-lg overflow-hidden">
              <div class="podium-header px-2.5 py-1 {isDarkMode ? 'bg-amber-500/20 border-amber-500/40' : 'bg-[#fef3c7] border-amber-300'} border-b flex items-center justify-between">
                <div class="flex items-center space-x-1.5">
                  <span class="{isDarkMode ? 'text-amber-400' : 'text-amber-700'} flex items-center">
                    <svg class="w-3.5 h-3.5 fill-current" viewBox="0 0 24 24">
                      <path d="M19 5h-2V3H7v2H5c-1.1 0-2 .9-2 2v1c0 2.55 1.92 4.63 4.39 4.94A5.01 5.01 0 0011 15.9V19H7v2h10v-2h-4v-3.1a5.01 5.01 0 003.61-2.96C19.08 12.63 21 10.55 21 8V7c0-1.1-.9-2-2-2zM5 8V7h2v3.82C5.84 10.4 5 9.3 5 8zm14 0c0 1.3-.84 2.4-2 2.82V7h2v1z"/>
                    </svg>
                  </span>
                  <span class="text-[11px] font-black {isDarkMode ? 'text-amber-300' : 'text-amber-950'} tracking-tight">Hasil Juara</span>
                </div>
              </div>

              <div class="divide-y {isDarkMode ? 'divide-zinc-700/60' : 'divide-slate-200'} text-[10px]">
                <!-- Juara 1 (Gold) -->
                <div class="podium-row px-2 py-1 flex items-center space-x-1.5 {isDarkMode ? 'bg-transparent' : 'bg-white'}">
                  <span class="w-4 h-4 rounded-full bg-gradient-to-br from-amber-300 to-amber-500 text-amber-950 font-black font-mono text-[9px] flex items-center justify-center shrink-0 shadow-xs">
                    1
                  </span>
                  {#if (podium.j1.logo && logoMap[podium.j1.logo]) || podium.j1.is_priority}
                    <div class="logo-star-wrapper flex items-center shrink-0 space-x-0.5 {podium.j1.is_priority ? 'has-priority mr-[3px]' : ''}">
                      {#if podium.j1.logo && logoMap[podium.j1.logo]}
                        <img
                          src={logoMap[podium.j1.logo]}
                          alt="Logo"
                          class="podium-dojo-logo w-3.5 h-3.5 max-w-3.5 max-h-3.5 object-contain shrink-0"
                        />
                      {/if}
                      {#if podium.j1.is_priority}
                        <span class="seed-star-icon inline-flex items-center text-amber-500 shrink-0" title="Unggulan / Seeded">
                          <svg class="w-2.5 h-2.5 fill-current" viewBox="2 2 20 20">
                            <path d="M12 17.27L18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z"/>
                          </svg>
                        </span>
                      {/if}
                    </div>
                  {/if}
                  <div class="min-w-0 truncate">
                    <div class="podium-name font-black {isDarkMode ? 'text-white' : 'text-slate-950'} truncate leading-tight">{podium.j1.name}</div>
                    <div class="podium-sub text-[8.5px] font-bold {isDarkMode ? 'text-zinc-200' : 'text-slate-800'} truncate leading-tight">{podium.j1.contingent}</div>
                  </div>
                </div>

                <!-- Juara 2 (Silver) -->
                <div class="podium-row px-2 py-1 flex items-center space-x-1.5 {isDarkMode ? 'bg-transparent' : 'bg-white'}">
                  <span class="w-4 h-4 rounded-full bg-gradient-to-br from-slate-200 to-slate-400 text-slate-800 font-black font-mono text-[9px] flex items-center justify-center shrink-0 shadow-xs">
                    2
                  </span>
                  {#if (podium.j2.logo && logoMap[podium.j2.logo]) || podium.j2.is_priority}
                    <div class="logo-star-wrapper flex items-center shrink-0 space-x-0.5 {podium.j2.is_priority ? 'has-priority mr-[3px]' : ''}">
                      {#if podium.j2.logo && logoMap[podium.j2.logo]}
                        <img
                          src={logoMap[podium.j2.logo]}
                          alt="Logo"
                          class="podium-dojo-logo w-3.5 h-3.5 max-w-3.5 max-h-3.5 object-contain shrink-0"
                        />
                      {/if}
                      {#if podium.j2.is_priority}
                        <span class="seed-star-icon inline-flex items-center text-amber-500 shrink-0" title="Unggulan / Seeded">
                          <svg class="w-2.5 h-2.5 fill-current" viewBox="2 2 20 20">
                            <path d="M12 17.27L18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z"/>
                          </svg>
                        </span>
                      {/if}
                    </div>
                  {/if}
                  <div class="min-w-0 truncate">
                    <div class="podium-name font-black {isDarkMode ? 'text-white' : 'text-slate-950'} truncate leading-tight">{podium.j2.name}</div>
                    <div class="podium-sub text-[8.5px] font-bold {isDarkMode ? 'text-zinc-200' : 'text-slate-800'} truncate leading-tight">{podium.j2.contingent}</div>
                  </div>
                </div>

                <!-- Juara 3A (Bronze) -->
                <div class="podium-row px-2 py-1 flex items-center space-x-1.5 {isDarkMode ? 'bg-transparent' : 'bg-white'}">
                  <span class="w-4 h-4 rounded-full bg-gradient-to-br from-orange-200 to-amber-600 text-orange-950 font-black font-mono text-[9px] flex items-center justify-center shrink-0 shadow-xs">
                    3
                  </span>
                  {#if (podium.j3a.logo && logoMap[podium.j3a.logo]) || podium.j3a.is_priority}
                    <div class="logo-star-wrapper flex items-center shrink-0 space-x-0.5 {podium.j3a.is_priority ? 'has-priority mr-[3px]' : ''}">
                      {#if podium.j3a.logo && logoMap[podium.j3a.logo]}
                        <img
                          src={logoMap[podium.j3a.logo]}
                          alt="Logo"
                          class="podium-dojo-logo w-3.5 h-3.5 max-w-3.5 max-h-3.5 object-contain shrink-0"
                        />
                      {/if}
                      {#if podium.j3a.is_priority}
                        <span class="seed-star-icon inline-flex items-center text-amber-500 shrink-0" title="Unggulan / Seeded">
                          <svg class="w-2.5 h-2.5 fill-current" viewBox="2 2 20 20">
                            <path d="M12 17.27L18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z"/>
                          </svg>
                        </span>
                      {/if}
                    </div>
                  {/if}
                  <div class="min-w-0 truncate">
                    <div class="podium-name font-black {isDarkMode ? 'text-white' : 'text-slate-950'} truncate leading-tight">{podium.j3a.name}</div>
                    <div class="podium-sub text-[8.5px] font-bold {isDarkMode ? 'text-zinc-200' : 'text-slate-800'} truncate leading-tight">{podium.j3a.contingent}</div>
                  </div>
                </div>

                <!-- Juara 3B (Bronze) -->
                <div class="podium-row px-2 py-1 flex items-center space-x-1.5 {isDarkMode ? 'bg-transparent' : 'bg-white'}">
                  <span class="w-4 h-4 rounded-full bg-gradient-to-br from-orange-200 to-amber-600 text-orange-950 font-black font-mono text-[9px] flex items-center justify-center shrink-0 shadow-xs">
                    3
                  </span>
                  {#if (podium.j3b.logo && logoMap[podium.j3b.logo]) || podium.j3b.is_priority}
                    <div class="logo-star-wrapper flex items-center shrink-0 space-x-0.5 {podium.j3b.is_priority ? 'has-priority mr-[3px]' : ''}">
                      {#if podium.j3b.logo && logoMap[podium.j3b.logo]}
                        <img
                          src={logoMap[podium.j3b.logo]}
                          alt="Logo"
                          class="podium-dojo-logo w-3.5 h-3.5 max-w-3.5 max-h-3.5 object-contain shrink-0"
                        />
                      {/if}
                      {#if podium.j3b.is_priority}
                        <span class="seed-star-icon inline-flex items-center text-amber-500 shrink-0" title="Unggulan / Seeded">
                          <svg class="w-2.5 h-2.5 fill-current" viewBox="2 2 20 20">
                            <path d="M12 17.27L18.18 21l-1.64-7.03L22 9.24l-7.19-.61L12 2 9.19 8.63 2 9.24l5.46 4.73L5.82 21z"/>
                          </svg>
                        </span>
                      {/if}
                    </div>
                  {/if}
                  <div class="min-w-0 truncate">
                    <div class="podium-name font-black {isDarkMode ? 'text-white' : 'text-slate-950'} truncate leading-tight">{podium.j3b.name}</div>
                    <div class="podium-sub text-[8.5px] font-bold {isDarkMode ? 'text-zinc-200' : 'text-slate-800'} truncate leading-tight">{podium.j3b.contingent}</div>
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

<style>
  .kata-row {
    display: flex;
    align-items: center;
    line-height: 1;
    margin-top: 0.5px;
  }

  .player-dojo-logo {
    width: 12px;
    height: 12px;
    max-width: 12px;
    max-height: 12px;
    object-fit: contain;
    flex-shrink: 0;
  }
</style>
