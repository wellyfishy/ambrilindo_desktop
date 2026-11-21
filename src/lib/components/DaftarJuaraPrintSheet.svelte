<script lang="ts">
  import type { BaganDaysResult, SummaryBaganItem, SummaryBaganWinner } from "$lib/types";
  import { groupJuaraByDay } from "$lib/juaraDays";
  import { FORKI_LOGO_DATA_URL, AMBRILINDO_LOGO_DATA_URL } from "$lib/assets_base64";
  import type { SignatureInfo } from "$lib/cover_settings";
  import SignatureBlock from "./SignatureBlock.svelte";

  interface Props {
    bagans: SummaryBaganItem[];
    eventName?: string;
    pageLabel?: string;
    customLogoForki?: string;
    customLogoAmbrilindo?: string;
    showBadge?: boolean;
    logoMap?: Record<string, string>;
    /** Jadwal days: when given, the list is split per day under a "DAY n" header */
    baganDays?: BaganDaysResult | null;
    /** Signature after the last category (results booklet) */
    signature?: SignatureInfo | null;
  }

  let {
    bagans = [],
    eventName = "",
    pageLabel = "Daftar Juara Seluruh Nomor Tanding",
    customLogoForki = FORKI_LOGO_DATA_URL,
    customLogoAmbrilindo = AMBRILINDO_LOGO_DATA_URL,
    showBadge = true,
    logoMap = {},
    baganDays = null,
    signature = null,
  }: Props = $props();

  function getWinnerAffiliation(w?: SummaryBaganWinner): string {
    if (!w) return "-";
    const perguruan = w.perguruan_nama?.trim();
    const kontingen = w.utusan_nama?.trim();
    if (perguruan && kontingen && perguruan !== kontingen) {
      return `${perguruan} • ${kontingen}`;
    }
    return perguruan || kontingen || "-";
  }

  /** A team (beregu) category: one of its winners has a member list. */
  function isTeamCategory(cat: SummaryBaganItem): boolean {
    return cat.winners.some((w) => (w.members?.length ?? 0) > 0);
  }

  /** Member grid columns: 3 for short names, 2 when a name is long. */
  function memberCols(names: string[]): string {
    return names.some((n) => n.length > 24) ? "grid-cols-2" : "grid-cols-3";
  }

  // Per Jadwal day; within a day team (beregu) categories first, one full-width card each,
  // then individual ones, two per row.
  let dayGroups = $derived(
    groupJuaraByDay(bagans, baganDays).map((g) => ({
      ...g,
      teams: g.bagans.filter(isTeamCategory),
      individuals: g.bagans.filter((b) => !isTeamCategory(b)),
    }))
  );

  const PODIUM: { key: string; label: string; badge: string }[] = [
    { key: "1", label: "1", badge: "bg-amber-400 text-slate-950" },
    { key: "2", label: "2", badge: "bg-slate-300 text-slate-950" },
    { key: "3a", label: "3A", badge: "bg-amber-700 text-white" },
    { key: "3b", label: "3B", badge: "bg-amber-700 text-white" },
  ];

  function getCategoryNameClass(name: string): string {
    const len = (name || "").trim().length;
    if (len > 60) {
      return "text-[8.5px] leading-tight";
    } else if (len > 36) {
      return "text-[10px] leading-tight";
    } else {
      return "text-xs leading-snug";
    }
  }
</script>

<!-- Team (beregu) card: one band per podium place - medal, team and kontingen on the left,
     the members in a grid on the right. A name too long for its cell ends in "…". -->
{#snippet teamCard(cat: SummaryBaganItem)}
  <div class="border border-slate-300 rounded p-2 bg-white break-inside-avoid" style="break-inside: avoid; page-break-inside: avoid;">
    <div class="border-b border-slate-300 pb-1 mb-1 flex items-start gap-2">
      <span class="text-[10px] font-mono font-black px-1.5 py-0.5 rounded bg-slate-900 text-white shrink-0 mt-0.5">{cat.kode}</span>
      <span class="font-black text-black uppercase flex-1 break-words whitespace-normal {getCategoryNameClass(cat.nama_bagan)}">{cat.nama_bagan}</span>
    </div>
    <div class="divide-y divide-slate-200">
      {#each PODIUM as place}
        {@const w = cat.winners.find((x) => x.juara === place.key)}
        <div class="flex items-start gap-2 py-1">
          <span class="w-5 h-4 rounded-full {place.badge} font-black text-[8.5px] flex items-center justify-center shrink-0 mt-px">{place.label}</span>
          <div class="w-[42%] min-w-0 shrink-0 flex items-start gap-1.5">
            {#if w?.logo && logoMap[w.logo]}
              <img src={logoMap[w.logo]} alt="" class="object-contain shrink-0 mt-px" style="width: 14px; height: 14px; max-width: 14px; max-height: 14px;" width="14" height="14"
                onerror={(e) => { (e.currentTarget as HTMLElement).style.display = 'none'; }} />
            {/if}
            <div class="min-w-0">
              <div class="text-[10.5px] font-black leading-tight {w ? 'text-black' : 'text-slate-400'} truncate">{w?.nama_atlet || "-"}</div>
              {#if w}
                <div class="text-[9px] font-semibold text-slate-600 leading-tight truncate">{getWinnerAffiliation(w)}</div>
              {/if}
            </div>
          </div>
          {#if w?.members?.length}
            <div class="flex-1 min-w-0 grid {memberCols(w.members)} gap-x-3 gap-y-px text-[9.5px] leading-tight text-black">
              {#each w.members as m, i}
                <span class="truncate" title={m}><span class="text-slate-500 font-mono">{i + 1}.</span> {m}</span>
              {/each}
            </div>
          {/if}
        </div>
      {/each}
    </div>
  </div>
{/snippet}

<div class="daftar-juara-sheet font-sans text-slate-900 bg-white">
  <!-- Semantic Table structure for recurring header/footer on every printed page -->
  <table class="w-full table-fixed border-collapse border-none">
    <!-- Recurring Document Header (Repeats at top of every printed page in Chromium) -->
    <thead class="print-table-header">
      <tr>
        <th class="p-0 border-none font-normal text-left">
          <div class="border-b-2 border-slate-900 pb-2 mb-2.5 flex items-center justify-between gap-4">
            <div class="flex items-center space-x-3.5 min-w-0">
              <!-- Top-Left Official Logo Cluster: FORKI & Ambrilindo -->
              <div class="flex items-center space-x-2.5 shrink-0 bg-white p-0.5 rounded">
                <img
                  src={customLogoForki}
                  alt="Logo FORKI"
                  class="h-10 w-auto max-w-[40px] max-h-10 object-contain shrink-0" style="max-height: 40px; max-width: 40px; width: auto; height: auto;"
                />
                <div class="h-8 w-[1px] bg-slate-300 shrink-0"></div>
                <img
                  src={customLogoAmbrilindo}
                  alt="Logo Ambrilindo"
                  class="h-7 w-auto max-w-[110px] max-h-7 object-contain shrink-0" style="max-height: 28px; max-width: 110px; width: auto; height: auto;"
                />
              </div>

              <!-- Title & Subtitle -->
              <div class="min-w-0">
                <h3 class="text-sm font-black text-slate-950 uppercase tracking-tight leading-snug">
                  DAFTAR JUARA SETIAP KELAS / NOMOR PERTANDINGAN
                </h3>
                {#if eventName}
                  <p class="text-[11px] text-slate-700 font-bold uppercase tracking-wide truncate mt-0.5">
                    {eventName}
                  </p>
                {/if}
              </div>
            </div>

            <!-- Right Header Badge -->
            {#if showBadge}
              <div class="shrink-0 flex items-center">
                <span class="px-2.5 py-1 rounded text-[10px] font-black uppercase bg-blue-600 text-white tracking-wider shadow-xs">
                  PODIUM KATEGORI
                </span>
              </div>
            {/if}
          </div>
        </th>
      </tr>
    </thead>

    <!-- Table Body: Contains 2-Column Category Cards Grid -->
    <tbody>
      <tr>
        <td class="p-0 border-none">
          {#if bagans.length === 0}
            <div class="p-8 text-center text-slate-400 italic">
              Belum ada nomor pertandingan yang diselesaikan.
            </div>
          {:else}
            {#each dayGroups as group, gi (group.key)}
            {#if group.label}
              <!-- Day header: kept with the first card below it -->
              <div class="flex items-center gap-2 {gi > 0 ? 'mt-4' : ''} mb-2" style="break-after: avoid; page-break-after: avoid;">
                <span class="px-2 py-0.5 rounded bg-slate-900 text-white text-[11px] font-black uppercase tracking-wider shrink-0">{group.label.split(",")[0]}</span>
                {#if group.label.includes(",")}
                  <span class="text-[11px] font-bold text-slate-700 uppercase tracking-wide">{group.label.split(",").slice(1).join(",").trim()}</span>
                {/if}
                <span class="flex-1 border-t-2 border-slate-900"></span>
                <span class="text-[10px] font-mono font-bold text-slate-500 shrink-0">{group.bagans.length} kelas</span>
              </div>
            {/if}
            {#if group.teams.length > 0}
              <div class="space-y-2 mb-2.5 print:mb-2">
                {#each group.teams as cat (cat.id)}
                  {@render teamCard(cat)}
                {/each}
              </div>
            {/if}
            <div class="grid grid-cols-2 gap-2.5 print:gap-2 print:grid-cols-2">
              {#each group.individuals as cat}
                {@const w1 = cat.winners.find((w) => w.juara === "1")}
                {@const w2 = cat.winners.find((w) => w.juara === "2")}
                {@const w3a = cat.winners.find((w) => w.juara === "3a")}
                {@const w3b = cat.winners.find((w) => w.juara === "3b")}

                <div class="border border-slate-300 rounded p-2 bg-slate-50/60 print:bg-white break-inside-avoid" style="break-inside: avoid; page-break-inside: avoid;">
                  <!-- Header Kategori -->
                  <div class="border-b border-slate-300 pb-1 mb-1.5 flex items-start gap-2 min-h-[22px]">
                    <span class="text-[10px] font-mono font-black px-1.5 py-0.5 rounded bg-slate-900 text-white shrink-0 mt-0.5">
                      {cat.kode}
                    </span>
                    <span class="font-black text-black uppercase flex-1 break-words whitespace-normal {getCategoryNameClass(cat.nama_bagan)}">
                      {cat.nama_bagan}
                    </span>
                  </div>

                  <!-- 4 Podium Rows -->
                  <div class="space-y-1 text-[10.5px]">
                    <!-- Juara 1 -->
                    <div class="flex items-center space-x-1.5">
                      <span class="w-4 h-4 rounded-full bg-amber-400 text-slate-950 font-black text-[9px] flex items-center justify-center shrink-0">
                        1
                      </span>
                      {#if w1?.logo && logoMap[w1.logo]}
                        <img
                          src={logoMap[w1.logo]}
                          alt=""
                          class="w-3.5 h-3.5 max-w-3.5 max-h-3.5 object-contain shrink-0 rounded-xs" style="width: 14px; height: 14px; max-width: 14px; max-height: 14px; object-fit: contain;" width="14" height="14"
                          onerror={(e) => { (e.currentTarget as HTMLElement).style.display = 'none'; }}
                        />
                      {/if}
                      <span class="font-bold {w1?.nama_atlet ? 'text-black' : 'text-slate-400'} truncate flex-1">
                        {w1?.nama_atlet || "-"}
                      </span>
                      <span class="text-[10px] font-bold {w1?.nama_atlet ? 'text-black' : 'text-slate-400'} truncate max-w-[155px] text-right" title={getWinnerAffiliation(w1)}>
                        {getWinnerAffiliation(w1)}
                      </span>
                    </div>

                    <!-- Juara 2 -->
                    <div class="flex items-center space-x-1.5">
                      <span class="w-4 h-4 rounded-full bg-slate-300 text-slate-950 font-black text-[9px] flex items-center justify-center shrink-0">
                        2
                      </span>
                      {#if w2?.logo && logoMap[w2.logo]}
                        <img
                          src={logoMap[w2.logo]}
                          alt=""
                          class="w-3.5 h-3.5 max-w-3.5 max-h-3.5 object-contain shrink-0 rounded-xs" style="width: 14px; height: 14px; max-width: 14px; max-height: 14px; object-fit: contain;" width="14" height="14"
                          onerror={(e) => { (e.currentTarget as HTMLElement).style.display = 'none'; }}
                        />
                      {/if}
                      <span class="font-bold {w2?.nama_atlet ? 'text-black' : 'text-slate-400'} truncate flex-1">
                        {w2?.nama_atlet || "-"}
                      </span>
                      <span class="text-[10px] font-bold {w2?.nama_atlet ? 'text-black' : 'text-slate-400'} truncate max-w-[155px] text-right" title={getWinnerAffiliation(w2)}>
                        {getWinnerAffiliation(w2)}
                      </span>
                    </div>

                    <!-- Juara 3A -->
                    <div class="flex items-center space-x-1.5">
                      <span class="w-4 h-4 rounded-full bg-amber-700 text-white font-black text-[8px] flex items-center justify-center shrink-0">
                        3A
                      </span>
                      {#if w3a?.logo && logoMap[w3a.logo]}
                        <img
                          src={logoMap[w3a.logo]}
                          alt=""
                          class="w-3.5 h-3.5 max-w-3.5 max-h-3.5 object-contain shrink-0 rounded-xs" style="width: 14px; height: 14px; max-width: 14px; max-height: 14px; object-fit: contain;" width="14" height="14"
                          onerror={(e) => { (e.currentTarget as HTMLElement).style.display = 'none'; }}
                        />
                      {/if}
                      <span class="font-bold {w3a?.nama_atlet ? 'text-black' : 'text-slate-400'} truncate flex-1">
                        {w3a?.nama_atlet || "-"}
                      </span>
                      <span class="text-[10px] font-bold {w3a?.nama_atlet ? 'text-black' : 'text-slate-400'} truncate max-w-[155px] text-right" title={getWinnerAffiliation(w3a)}>
                        {getWinnerAffiliation(w3a)}
                      </span>
                    </div>

                    <!-- Juara 3B -->
                    <div class="flex items-center space-x-1.5">
                      <span class="w-4 h-4 rounded-full bg-amber-700 text-white font-black text-[8px] flex items-center justify-center shrink-0">
                        3B
                      </span>
                      {#if w3b?.logo && logoMap[w3b.logo]}
                        <img
                          src={logoMap[w3b.logo]}
                          alt=""
                          class="w-3.5 h-3.5 max-w-3.5 max-h-3.5 object-contain shrink-0 rounded-xs" style="width: 14px; height: 14px; max-width: 14px; max-height: 14px; object-fit: contain;" width="14" height="14"
                          onerror={(e) => { (e.currentTarget as HTMLElement).style.display = 'none'; }}
                        />
                      {/if}
                      <span class="font-bold {w3b?.nama_atlet ? 'text-black' : 'text-slate-400'} truncate flex-1">
                        {w3b?.nama_atlet || "-"}
                      </span>
                      <span class="text-[10px] font-bold {w3b?.nama_atlet ? 'text-black' : 'text-slate-400'} truncate max-w-[155px] text-right" title={getWinnerAffiliation(w3b)}>
                        {getWinnerAffiliation(w3b)}
                      </span>
                    </div>
                  </div>
                </div>
              {/each}
            </div>
            {/each}
          {/if}
          {#if signature}<SignatureBlock {signature} />{/if}
        </td>
      </tr>
    </tbody>

    <!-- Table Footer: Automatically appears at bottom of document -->
    <tfoot class="print-table-footer">
      <tr>
        <td class="p-0 border-none">
          <div class="mt-2 pt-1.5 border-t border-slate-200 flex items-center justify-between text-[9px] text-slate-500 font-mono">
            <span>Official Match Result Book • Ambrilindo Tournament Engine</span>
            <span>{pageLabel}</span>
          </div>
        </td>
      </tr>
    </tfoot>
  </table>
</div>

<style>
  @media print {
    @page {
      size: A4 portrait;
      margin: 8mm;
    }
    .print-table-header {
      display: table-header-group !important;
    }
    .print-table-footer {
      display: table-footer-group !important;
    }
    .break-inside-avoid {
      break-inside: avoid !important;
      page-break-inside: avoid !important;
    }
  }
</style>
