<script lang="ts">
  import type { StandingItem } from "$lib/types";

  interface Props {
    standingsKontingen: StandingItem[];
    standingsPerguruan: StandingItem[];
    totalGold?: number;
    totalSilver?: number;
    totalBronze?: number;
    grandTotal?: number;
    eventName?: string;
    subTitle?: string;
    pageLabel?: string;
    pageLabelKontingen?: string;
    pageLabelPerguruan?: string;
    customLogoForki?: string;
    customLogoAmbrilindo?: string;
    showBadge?: boolean;
    logoMap?: Record<string, string>;
  }

  let {
    standingsKontingen = [],
    standingsPerguruan = [],
    totalGold,
    totalSilver,
    totalBronze,
    grandTotal,
    eventName = "",
    subTitle = "Rekapitulasi Resmi",
    pageLabel = "Rekapitulasi Perolehan Medali",
    pageLabelKontingen,
    pageLabelPerguruan,
    customLogoForki = "/img/forki-logo.png",
    customLogoAmbrilindo = "/img/ambrilindo-logo.png",
    showBadge = true,
    logoMap = {},
  }: Props = $props();

  // Computed totals for Kontingen
  let calculatedGold = $derived.by(() => {
    if (typeof totalGold === "number") return totalGold;
    return standingsKontingen.reduce((acc, curr) => acc + (curr.gold || 0), 0);
  });

  let calculatedSilver = $derived.by(() => {
    if (typeof totalSilver === "number") return totalSilver;
    return standingsKontingen.reduce((acc, curr) => acc + (curr.silver || 0), 0);
  });

  let calculatedBronze = $derived.by(() => {
    if (typeof totalBronze === "number") return totalBronze;
    return standingsKontingen.reduce((acc, curr) => acc + (curr.bronze || 0), 0);
  });

  let calculatedGrand = $derived.by(() => {
    if (typeof grandTotal === "number") return grandTotal;
    return calculatedGold + calculatedSilver + calculatedBronze;
  });

  // Computed totals for Perguruan
  let perguruanGold = $derived.by(() => {
    return standingsPerguruan.reduce((acc, curr) => acc + (curr.gold || 0), 0);
  });

  let perguruanSilver = $derived.by(() => {
    return standingsPerguruan.reduce((acc, curr) => acc + (curr.silver || 0), 0);
  });

  let perguruanBronze = $derived.by(() => {
    return standingsPerguruan.reduce((acc, curr) => acc + (curr.bronze || 0), 0);
  });

  let perguruanGrand = $derived.by(() => {
    return perguruanGold + perguruanSilver + perguruanBronze;
  });
</script>

<svelte:head>
  <style>
    @page {
      size: A4 portrait;
      margin: 8mm;
    }
  </style>
</svelte:head>

<div class="klasemen-medali-sheet font-sans text-slate-900 bg-white">
  <!-- ================= LEMBAR 1: KLASEMEN KONTINGEN / UTUSAN ================= -->
  <div class="print-page-sheet break-after-page mb-8 print:mb-0">
    <table class="w-full border-collapse border-none">
      <!-- Recurring Document Header for Kontingen Page -->
      <thead class="print-table-header">
        <tr>
          <th class="p-0 border-none font-normal text-left">
            <div class="border-b-2 border-slate-900 pb-3 mb-4 flex items-center justify-between gap-4">
              <div class="flex items-center space-x-2.5 shrink-0 bg-white p-0.5 rounded">
                <img
                  src={customLogoForki}
                  alt="Logo FORKI"
                  class="h-11 w-auto max-w-[44px] object-contain shrink-0"
                />
                <div class="h-8 w-[1px] bg-slate-300 shrink-0"></div>
                <img
                  src={customLogoAmbrilindo}
                  alt="Logo Ambrilindo"
                  class="h-7 w-auto max-w-[110px] object-contain shrink-0"
                />
              </div>

              <!-- Title & Subtitle -->
              <div class="min-w-0">
                <h3 class="text-sm font-black text-slate-950 uppercase tracking-tight leading-snug">
                  REKAPITULASI MEDALI KONTINGEN / UTUSAN
                </h3>
                {#if eventName}
                  <p class="text-[11px] text-slate-700 font-bold uppercase tracking-wide truncate mt-0.5">
                    {eventName} {#if subTitle}• {subTitle}{/if}
                  </p>
                {/if}
              </div>

              <!-- Right Header Badge -->
              {#if showBadge}
                <div class="shrink-0 flex items-center">
                  <span class="px-2.5 py-1 rounded text-[10px] font-black uppercase bg-blue-600 text-white tracking-wider shadow-xs">
                    KLASEMEN KONTINGEN
                  </span>
                </div>
              {/if}
            </div>
          </th>
        </tr>
      </thead>

      <!-- Table Body: Klasemen Kontingen / Utusan Daerah -->
      <tbody>
        <tr>
          <td class="p-0 border-none">
            <div class="mb-4">
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
                  {#if standingsKontingen.length === 0}
                    <tr>
                      <td colspan="6" class="px-3 py-4 text-center text-slate-400 italic">
                        Belum ada medali kontingen yang tercatat.
                      </td>
                    </tr>
                  {:else}
                    {#each standingsKontingen as k}
                      <tr class="{k.rank === 1 ? 'bg-amber-50/70 font-bold' : (k.rank <= 3 ? 'bg-slate-50/70' : '')}">
                        <td class="px-2.5 py-1 text-center font-mono font-bold border-r border-slate-200">
                          {#if k.rank === 1}
                            <span class="inline-block px-1.5 py-0.2 rounded bg-amber-500 text-slate-950 font-black text-[10px]">1</span>
                          {:else if k.rank === 2}
                            <span class="inline-block px-1.5 py-0.2 rounded bg-slate-300 text-slate-950 font-black text-[10px]">2</span>
                          {:else if k.rank === 3}
                            <span class="inline-block px-1.5 py-0.2 rounded bg-amber-700 text-white font-black text-[10px]">3</span>
                          {:else}
                            <span class="font-bold text-slate-700">{k.rank}</span>
                          {/if}
                        </td>
                        <td class="px-3 py-1 text-slate-900 border-r border-slate-200 font-bold truncate max-w-xs">
                          <div class="flex items-center space-x-2">
                            {#if k.logo && logoMap[k.logo]}
                              <img
                                src={logoMap[k.logo]}
                                alt=""
                                class="w-4 h-4 object-contain shrink-0 rounded-xs"
                                onerror={(e) => { (e.currentTarget as HTMLElement).style.display = 'none'; }}
                              />
                            {/if}
                            <span class="truncate">{k.nama}</span>
                          </div>
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
                    <td class="px-2.5 py-1.5 text-center text-amber-400 font-mono">{calculatedGold}</td>
                    <td class="px-2.5 py-1.5 text-center text-slate-200 font-mono">{calculatedSilver}</td>
                    <td class="px-2.5 py-1.5 text-center text-amber-500 font-mono">{calculatedBronze}</td>
                    <td class="px-2.5 py-1.5 text-center text-white font-mono">{calculatedGrand}</td>
                  </tr>
                </tfoot>
              </table>
            </div>
          </td>
        </tr>
      </tbody>

      <!-- Table Footer: Kontingen Sheet -->
      <tfoot class="print-table-footer">
        <tr>
          <td class="p-0 border-none">
            <div class="mt-6 pt-2 border-t border-slate-200 flex items-center justify-between text-[9px] text-slate-500 font-mono">
              <span>Official Match Result Book • Ambrilindo Tournament Engine</span>
              <span>{pageLabelKontingen || pageLabel || "Halaman Klasemen Kontingen"}</span>
            </div>
          </td>
        </tr>
      </tfoot>
    </table>
  </div>

  <!-- ================= LEMBAR 2: KLASEMEN PERGURUAN ================= -->
  <div class="print-page-sheet">
    <table class="w-full border-collapse border-none">
      <!-- Recurring Document Header for Perguruan Page -->
      <thead class="print-table-header">
        <tr>
          <th class="p-0 border-none font-normal text-left">
            <div class="border-b-2 border-slate-900 pb-3 mb-4 flex items-center justify-between gap-4">
              <div class="flex items-center space-x-2.5 shrink-0 bg-white p-0.5 rounded">
                <img
                  src={customLogoForki}
                  alt="Logo FORKI"
                  class="h-11 w-auto max-w-[44px] object-contain shrink-0"
                />
                <div class="h-8 w-[1px] bg-slate-300 shrink-0"></div>
                <img
                  src={customLogoAmbrilindo}
                  alt="Logo Ambrilindo"
                  class="h-7 w-auto max-w-[110px] object-contain shrink-0"
                />
              </div>

              <!-- Title & Subtitle -->
              <div class="min-w-0">
                <h3 class="text-sm font-black text-slate-950 uppercase tracking-tight leading-snug">
                  REKAPITULASI MEDALI PERGURUAN
                </h3>
                {#if eventName}
                  <p class="text-[11px] text-slate-700 font-bold uppercase tracking-wide truncate mt-0.5">
                    {eventName} {#if subTitle}• {subTitle}{/if}
                  </p>
                {/if}
              </div>

              <!-- Right Header Badge -->
              {#if showBadge}
                <div class="shrink-0 flex items-center">
                  <span class="px-2.5 py-1 rounded text-[10px] font-black uppercase bg-emerald-600 text-white tracking-wider shadow-xs">
                    KLASEMEN PERGURUAN
                  </span>
                </div>
              {/if}
            </div>
          </th>
        </tr>
      </thead>

      <!-- Table Body: Klasemen Perguruan (tanpa kata Aliran (FORKI)) -->
      <tbody>
        <tr>
          <td class="p-0 border-none">
            <div class="mb-4">
              <div class="flex items-center space-x-2 mb-2">
                <span class="w-2.5 h-2.5 rounded-full bg-emerald-600"></span>
                <h4 class="text-xs font-black uppercase tracking-wider text-slate-900">
                  Klasemen Perolehan Medali Perguruan
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
                  {#if standingsPerguruan.length === 0}
                    <tr>
                      <td colspan="6" class="px-3 py-4 text-center text-slate-400 italic">
                        Belum ada medali perguruan yang tercatat.
                      </td>
                    </tr>
                  {:else}
                    {#each standingsPerguruan as p}
                      <tr class="{p.rank === 1 ? 'bg-amber-50/70 font-bold' : (p.rank <= 3 ? 'bg-slate-50/70' : '')}">
                        <td class="px-2.5 py-1 text-center font-mono font-bold border-r border-slate-200">
                          {#if p.rank === 1}
                            <span class="inline-block px-1.5 py-0.2 rounded bg-amber-500 text-slate-950 font-black text-[10px]">1</span>
                          {:else if p.rank === 2}
                            <span class="inline-block px-1.5 py-0.2 rounded bg-slate-300 text-slate-950 font-black text-[10px]">2</span>
                          {:else if p.rank === 3}
                            <span class="inline-block px-1.5 py-0.2 rounded bg-amber-700 text-white font-black text-[10px]">3</span>
                          {:else}
                            <span class="font-bold text-slate-700">{p.rank}</span>
                          {/if}
                        </td>
                        <td class="px-3 py-1 text-slate-900 border-r border-slate-200 font-bold truncate max-w-xs">
                          <div class="flex items-center space-x-2">
                            {#if p.logo && logoMap[p.logo]}
                              <img
                                src={logoMap[p.logo]}
                                alt=""
                                class="w-4 h-4 object-contain shrink-0 rounded-xs"
                                onerror={(e) => { (e.currentTarget as HTMLElement).style.display = 'none'; }}
                              />
                            {/if}
                            <span class="truncate">{p.nama}</span>
                          </div>
                        </td>
                        <td class="px-2.5 py-1 text-center font-mono font-black text-amber-700 border-r border-slate-200">{p.gold}</td>
                        <td class="px-2.5 py-1 text-center font-mono font-bold text-slate-600 border-r border-slate-200">{p.silver}</td>
                        <td class="px-2.5 py-1 text-center font-mono font-bold text-amber-800 border-r border-slate-200">{p.bronze}</td>
                        <td class="px-2.5 py-1 text-center font-mono font-black text-slate-950">{p.total}</td>
                      </tr>
                    {/each}
                  {/if}
                </tbody>
                <tfoot class="bg-slate-900 text-white font-black text-xs border-t-2 border-slate-900">
                  <tr>
                    <td colspan="2" class="px-3 py-1.5 text-right uppercase tracking-wider">Total Keseluruhan</td>
                    <td class="px-2.5 py-1.5 text-center text-amber-400 font-mono">{perguruanGold}</td>
                    <td class="px-2.5 py-1.5 text-center text-slate-200 font-mono">{perguruanSilver}</td>
                    <td class="px-2.5 py-1.5 text-center text-amber-500 font-mono">{perguruanBronze}</td>
                    <td class="px-2.5 py-1.5 text-center text-white font-mono">{perguruanGrand}</td>
                  </tr>
                </tfoot>
              </table>
            </div>
          </td>
        </tr>
      </tbody>

      <!-- Table Footer: Perguruan Sheet -->
      <tfoot class="print-table-footer">
        <tr>
          <td class="p-0 border-none">
            <div class="mt-6 pt-2 border-t border-slate-200 flex items-center justify-between text-[9px] text-slate-500 font-mono">
              <span>Official Match Result Book • Ambrilindo Tournament Engine</span>
              <span>{pageLabelPerguruan || pageLabel || "Halaman Klasemen Perguruan"}</span>
            </div>
          </td>
        </tr>
      </tfoot>
    </table>
  </div>
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
    .break-after-page {
      break-after: page !important;
      page-break-after: always !important;
    }
  }
</style>
