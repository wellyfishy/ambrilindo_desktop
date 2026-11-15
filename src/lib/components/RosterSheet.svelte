<!--
  "Roster 1 halaman": the event's run sheet, every scheduled day with every tatami.
  One source for Jadwal's roster print, the results booklet and Roster Bagan in Cetak,
  so a change here (or to the Jadwal data) shows in all three.
-->
<script lang="ts">
  import { cellDisplayName, missingSplitParts, SPLIT_PART_LABEL } from "$lib/timetable_split";
  import { computeColumnTimes, customTextAthletes, isFestivalText, parseTimeToMinutes } from "$lib/auto_schedule";
  import { FORKI_LOGO_DATA_URL, AMBRILINDO_LOGO_DATA_URL } from "$lib/assets_base64";
  import type {
    EnrichedTimetableCell,
    EnrichedTimetableDay,
    EnrichedTimetableRow,
    SplitPart,
    TatamiItem,
    TimetableCategoryOption,
  } from "$lib/types";

  interface Props {
    /** every Jadwal day; days without rows are left out */
    days: EnrichedTimetableDay[];
    tatamis: TatamiItem[];
    /** for "Kelas tidak ditandingkan" (festival categories are skipped) */
    categories: TimetableCategoryOption[];
    eventName: string;
    kopNama?: string;
    kopAlamat?: string;
    kopKontak?: string;
    kopLogo?: string | null;
    /** shown when no day has a schedule yet (Jadwal: the open day) */
    fallbackDay?: EnrichedTimetableDay | null;
    startTime?: string;
    /** shrink (CSS zoom) to fit the height of the box it sits in */
    autoFit?: boolean;
    /** zoom set by the caller (Jadwal fits the page itself) */
    zoom?: number;
    pageEl?: HTMLDivElement | null;
  }

  let {
    days,
    tatamis,
    categories,
    eventName,
    kopNama = "",
    kopAlamat = "",
    kopKontak = "",
    kopLogo = null,
    fallbackDay = null,
    startTime = "08:30",
    autoFit = false,
    zoom = 1,
    pageEl = $bindable(null),
  }: Props = $props();

  let rosterDays = $derived.by(() => {
    const withRows = days.filter((d) => d.rows.length > 0);
    return withRows.length > 0 ? withRows : fallbackDay ? [fallbackDay] : [];
  });

  function isFestivalSessionCell(c: EnrichedTimetableCell | undefined | null): boolean {
    return !!c && !c.nomor_tanding_id && isFestivalText(c.custom_text);
  }
  function textCellTitle(c: EnrichedTimetableCell): string {
    return isFestivalSessionCell(c) && customTextAthletes(c.custom_text) > 0 ? "FESTIVAL" : c.custom_text;
  }
  function textCellAthletes(c: EnrichedTimetableCell): number {
    return c.atlet_count || (isFestivalSessionCell(c) ? customTextAthletes(c.custom_text) : 0);
  }

  function getTatamiCellSeq(targetTatamiId: number, targetRowIdx: number, rows: EnrichedTimetableRow[]): number {
    let seq = 1;
    for (let r = 0; r < rows.length; r++) {
      const row = rows[r];
      if (row.row_type === "label") continue;
      const cell = row.cells.find((c) => c.tatami_id === targetTatamiId);
      if (r === targetRowIdx) return seq;
      if (cell && (cell.nomor_tanding_id || (cell.custom_text && !cell.is_break))) seq++;
    }
    return seq;
  }

  /** Per tatami of one day: bouts and athletes, the same numbers as the Jadwal grid header. */
  function dayTatamiTotals(day: EnrichedTimetableDay): Record<number, { matches: number; athletes: number }> {
    const start = parseTimeToMinutes(startTime);
    const out: Record<number, { matches: number; athletes: number }> = {};
    for (const t of tatamis) {
      const c = computeColumnTimes(day.rows, t.id, start);
      out[t.id] = { matches: c.matches, athletes: c.athletes };
    }
    return out;
  }

  let densityClass = $derived.by(() => {
    if (rosterDays.length === 0) return "density-normal";
    const totalRows = rosterDays.reduce((n, d) => n + d.rows.length + (rosterDays.length > 1 ? 2 : 0), 0);
    if (totalRows <= 20) return "density-spacious";
    if (totalRows <= 35) return "density-normal";
    if (totalRows <= 50) return "density-compact";
    return "density-ultra-compact";
  });

  let unassignedLabels = $derived.by(() => {
    const assigned = new Set<number>();
    const placed = new Map<number, (SplitPart | null | undefined)[]>();
    for (const d of days)
      for (const r of d.rows) {
        if (r.row_type !== "slot") continue;
        for (const c of r.cells) {
          if (!c.nomor_tanding_id) continue;
          assigned.add(c.nomor_tanding_id);
          placed.set(c.nomor_tanding_id, [...(placed.get(c.nomor_tanding_id) || []), c.part]);
        }
      }
    const out: string[] = [];
    for (const c of categories) {
      if (c.is_festival) continue;
      if (!assigned.has(c.id)) out.push(`${c.nama} (${c.atlet_count})`);
      else for (const part of missingSplitParts(placed.get(c.id) || [])) out.push(`${c.nama} (${SPLIT_PART_LABEL[part]})`);
    }
    return out;
  });

  const printedTimeStr = (() => {
    const now = new Date();
    return (
      now.toLocaleDateString("id-ID", { day: "numeric", month: "short", year: "numeric" }) +
      " " +
      String(now.getHours()).padStart(2, "0") +
      ":" +
      String(now.getMinutes()).padStart(2, "0")
    );
  })();

  /** The kop logo could not be loaded: show the default logos. */
  let kopLogoFailed = $state(false);
  $effect(() => {
    void kopLogo;
    kopLogoFailed = false;
  });

  // autoFit: shrink until the roster fits the height of its box
  let fitZoom = $state(1);
  $effect(() => {
    const el = pageEl;
    if (!autoFit || !el) return;
    void rosterDays.map((d) => d.rows.length).join(",");
    void densityClass;
    void tatamis.length;
    fitZoom = 1;
    const frame = requestAnimationFrame(() => {
      const need = el.scrollHeight;
      const have = el.clientHeight;
      fitZoom = need > have + 1 ? Math.max(0.3, have / need) : 1;
    });
    return () => cancelAnimationFrame(frame);
  });
  let appliedZoom = $derived(autoFit ? fitZoom : zoom);
</script>

{#if rosterDays.length > 0}
<div
  class="simple-roster-page {densityClass}"
  bind:this={pageEl}
  style="zoom: {appliedZoom}; height: {100 / appliedZoom}%;"
>
  <!-- Kop Surat -->
  <div class="simple-roster-header">
    <div class="simple-roster-kop-left">
      {#if kopLogo && !kopLogoFailed}
        <img src={kopLogo} class="simple-roster-logo" alt="" onerror={() => (kopLogoFailed = true)} />
      {:else}
        <div class="simple-roster-brand-logos">
          <img src={FORKI_LOGO_DATA_URL} alt="FORKI" class="brand-logo-forki" />
          <span class="brand-logo-divider"></span>
          <img src={AMBRILINDO_LOGO_DATA_URL} alt="Ambrilindo" class="brand-logo-ambrilindo" />
        </div>
      {/if}
      <div class="simple-roster-org-info">
        <h3 class="simple-roster-org-name">{kopNama || eventName}</h3>
        {#if kopAlamat || kopKontak}
          <p class="simple-roster-org-meta">
            {[kopAlamat, kopKontak].filter(Boolean).join(" • ")}
          </p>
        {/if}
        <div class="simple-roster-doc-title">ROSTER PERTANDINGAN / RUN SHEET</div>
      </div>
    </div>
    {#if rosterDays.length === 1}
      <div class="simple-roster-day-badge-box">
        <div class="simple-roster-day-label">{rosterDays[0].label}</div>
        {#if rosterDays[0].tanggal}
          <div class="simple-roster-day-date">{rosterDays[0].tanggal}</div>
        {/if}
      </div>
    {/if}
  </div>

  {#each rosterDays as day (day.id)}
  {@const dayTotals = dayTatamiTotals(day)}
  <div class="simple-roster-day-section">
    {#if rosterDays.length > 1}
      <div class="simple-roster-day-bar">
        <span class="day-bar-title">{day.label}</span>
        {#if day.tanggal}<span class="day-bar-date">{day.tanggal}</span>{/if}
      </div>
    {/if}
    <table class="simple-roster-table">
      <thead>
        <tr>
          {#each tatamis as t}
            {@const tMeta = dayTotals[t.id]}
            <th style="width: {100 / tatamis.length}%;">
              TATAMI {t.tatami_number}
              {#if tMeta}
                <span class="th-sub">({tMeta.matches} Partai • {tMeta.athletes} Atlet)</span>
              {/if}
            </th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#if day.rows.length === 0}
          <tr>
            <td colspan={tatamis.length} style="text-align:center; padding:8px; color:#94a3b8; font-style:italic;">
              Belum ada nomor tanding yang dijadwalkan pada hari ini.
            </td>
          </tr>
        {:else}
          {#each day.rows as row, rIdx}
            {#if row.row_type === "label"}
              <tr class="roster-ishoma-row">
                <td colspan={tatamis.length} class="roster-ishoma-cell">
                  <div class="roster-ishoma-bar">
                    <span class="roster-ishoma-label">{row.label_text || 'ISHOMA / ISTIRAHAT SIANG'}</span>
                  </div>
                </td>
              </tr>
            {:else}
              <tr>
                {#each tatamis as t}
                  {@const cell = row.cells.find((c) => c.tatami_id === t.id)}
                  {#if cell && cell.nomor_tanding_id}
                    {@const seq = getTatamiCellSeq(t.id, rIdx, day.rows)}
                    <td class="roster-match-cell">
                      <div class="roster-item-line">
                        <span class="roster-num-badge">#{seq}</span>
                        <span class="roster-item-title">{cellDisplayName(cell)}</span>
                        <span class="roster-atlet-count">({cell.atlet_count})</span>
                      </div>
                    </td>
                  {:else if cell && cell.custom_text}
                    {@const isFest = cell.is_festival || cell.custom_text.toUpperCase().includes('FESTIVAL')}
                    {@const seq = getTatamiCellSeq(t.id, rIdx, day.rows)}
                    <td class="roster-match-cell {isFest ? 'is-festival' : ''}">
                      <div class="roster-item-line">
                        <span class="roster-num-badge">#{seq}</span>
                        <span class="roster-item-title">{textCellTitle(cell)}</span>
                        {#if textCellAthletes(cell) > 0}
                          <span class="roster-atlet-count">({textCellAthletes(cell)})</span>
                        {/if}
                      </div>
                    </td>
                  {:else}
                    <td class="roster-match-cell is-empty">
                      <span>-</span>
                    </td>
                  {/if}
                {/each}
              </tr>
            {/if}
          {/each}
        {/if}
      </tbody>
    </table>
  </div>
  {/each}

  <div class="simple-roster-bottom-section">
    <div class="simple-roster-bottom-left">
      <div class="simple-roster-reminder-box">
        <span class="reminder-badge">PENTING</span>
        <span class="reminder-text">Jadwal bersifat estimasi. Atlet &amp; official wajib berada di area pemanasan 30 menit sebelum partai dimulai.</span>
      </div>

      {#if unassignedLabels.length > 0}
        <div class="simple-roster-unassigned">
          <span class="unassigned-title">Kelas tidak ditandingkan:</span>
          <span class="unassigned-items">
            {unassignedLabels.join(', ')}
          </span>
        </div>
      {/if}
    </div>

    <div class="simple-roster-bottom-right">
      <div class="simple-roster-ts">Dicetak: {printedTimeStr} • Ambrilindo Timetable Engine</div>
    </div>
  </div>
</div>
{/if}

<style>
  .simple-roster-page {
    box-sizing: border-box;
    padding: 1.5mm 2.5mm;
    margin: 0;
    width: 100%;
    background: #ffffff !important;
    color: #0f172a !important;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
    page-break-inside: avoid;
  }

  .simple-roster-brand-logos {
    display: flex;
    align-items: center;
    gap: 7px;
    flex-shrink: 0;
  }
  .brand-logo-forki {
    height: 36px;
    width: auto;
    max-width: 40px;
    object-fit: contain;
  }
  .brand-logo-divider {
    width: 1.2px;
    height: 26px;
    background: #cbd5e1;
  }
  .brand-logo-ambrilindo {
    height: 20px;
    width: auto;
    max-width: 100px;
    object-fit: contain;
  }

  /* Kop Surat in Simple Roster */
  .simple-roster-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1.5px solid #cbd5e1;
    padding-bottom: 3px;
    margin-bottom: 3px;
    gap: 8px;
  }
  .simple-roster-kop-left {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .simple-roster-logo {
    width: 52px;
    height: 52px;
    max-width: 80px;
    max-height: 54px;
    object-fit: contain;
    flex-shrink: 0;
  }
  .simple-roster-org-name {
    margin: 0;
    font-family: inherit;
    font-size: 13px;
    font-weight: 800;
    color: #0f172a;
    line-height: 1.15;
    text-transform: uppercase;
  }
  .simple-roster-org-meta {
    margin: 1px 0 0;
    font-size: 8.5px;
    color: #475569;
    line-height: 1.15;
  }
  .simple-roster-doc-title {
    margin-top: 1px;
    font-size: 10.5px;
    font-weight: 800;
    color: #1e293b;
    letter-spacing: 0.03em;
    text-transform: uppercase;
  }
  .simple-roster-day-badge-box {
    text-align: right;
    padding: 2px 6px;
    background: #f8fafc;
    border: 1px solid #cbd5e1;
    border-radius: 3px;
    min-width: 90px;
  }
  .simple-roster-day-label {
    font-size: 11.5px;
    font-weight: 800;
    color: #b91c1c;
    text-transform: uppercase;
  }
  .simple-roster-day-date {
    font-size: 8.5px;
    font-weight: 600;
    color: #475569;
    margin-top: 1px;
  }

  /* Day Section & Day Bar */
  .simple-roster-day-section {
    page-break-inside: avoid;
    margin-bottom: 3px;
  }
  .simple-roster-day-bar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    background: #f8fafc;
    border: 1px solid #cbd5e1;
    border-left: 3px solid #b91c1c;
    padding: 1.5px 5px;
    margin: 2px 0 1.5px 0;
  }
  .day-bar-title {
    font-size: 10px;
    font-weight: 800;
    color: #b91c1c;
    text-transform: uppercase;
    letter-spacing: 0.02em;
  }
  .day-bar-date {
    font-size: 8.5px;
    font-weight: 600;
    color: #475569;
  }

  /* Synchronized Run Sheet Table */
  .simple-roster-table {
    width: 100%;
    border-collapse: collapse;
    table-layout: fixed;
    border: 1px solid #94a3b8;
    background: #ffffff;
    margin-bottom: 2px;
  }
  .simple-roster-table th {
    background: #f1f5f9 !important;
    color: #0f172a !important;
    font-size: 9.5px;
    font-weight: 800;
    text-align: center;
    padding: 2px 3px;
    border: 1px solid #94a3b8;
    border-bottom: 1.5px solid #64748b;
    text-transform: uppercase;
  }
  .simple-roster-table th .th-sub {
    font-size: 8px;
    font-weight: 600;
    color: #475569;
    display: inline-block;
    margin-left: 3px;
  }
  .simple-roster-table td {
    border: 1px solid #cbd5e1;
    vertical-align: middle;
    box-sizing: border-box;
  }
  .simple-roster-table tbody tr:nth-child(even) td.roster-match-cell {
    background-color: #fafafa;
  }
  .simple-roster-table tbody tr:nth-child(odd) td.roster-match-cell {
    background-color: #ffffff;
  }

  /* Compact Density Scaling (Optimized for Portrait A4) */
  .simple-roster-page.density-spacious .roster-match-cell {
    padding: 4px 6px;
    font-size: 11px;
    line-height: 1.2;
  }
  .simple-roster-page.density-normal .roster-match-cell {
    padding: 3px 5px;
    font-size: 10px;
    line-height: 1.18;
  }
  .simple-roster-page.density-compact .roster-match-cell {
    padding: 2px 4px;
    font-size: 9px;
    line-height: 1.15;
  }
  .simple-roster-page.density-ultra-compact .roster-match-cell {
    padding: 1.2px 3px;
    font-size: 8px;
    line-height: 1.1;
  }

  /* Multi-line wrapping item (NO ELLIPSIS - FULL TEXT ALWAYS VISIBLE) */
  .roster-item-line {
    display: flex;
    align-items: flex-start;
    line-height: 1.2;
    width: 100%;
    gap: 3px;
    flex-wrap: nowrap;
  }
  .roster-num-badge {
    display: inline-block;
    min-width: 15px;
    text-align: center;
    font-weight: 800;
    font-size: 0.82em;
    background: #e2e8f0;
    color: #1e293b;
    padding: 0.5px 2.5px;
    border-radius: 2px;
    margin-top: 1px;
    flex-shrink: 0;
    line-height: 1.1;
  }
  .roster-item-title {
    font-weight: 700;
    color: #0f172a;
    flex: 1 1 auto;
    min-width: 0;
    white-space: normal !important;
    word-break: break-word !important;
    overflow-wrap: break-word !important;
    text-overflow: clip !important;
    overflow: visible !important;
    line-height: 1.2;
  }
  .roster-atlet-count {
    font-weight: 700;
    color: #0369a1;
    margin-left: 2px;
    margin-top: 1px;
    white-space: nowrap;
    flex-shrink: 0;
    align-self: flex-start;
    line-height: 1.1;
  }

  /* Festival Session */
  .roster-match-cell.is-festival {
    background: #dcfce7 !important;
    border-color: #86efac !important;
  }
  .roster-match-cell.is-festival .roster-item-title {
    color: #14532d !important;
    font-weight: 900 !important;
    letter-spacing: 0.04em;
  }
  .roster-match-cell.is-festival .roster-num-badge {
    background: #bbf7d0 !important;
    color: #14532d !important;
  }

  /* Empty Cell */
  .roster-match-cell.is-empty {
    background: #fcfcfc !important;
    text-align: center;
    color: #cbd5e1;
    font-size: 8px;
  }

  /* Aligned ISHOMA Row */
  .roster-ishoma-row {
    page-break-inside: avoid;
  }
  .roster-ishoma-cell {
    background: #f0fdf4 !important;
    border-top: 1.5px dashed #0284c7 !important;
    border-bottom: 1.5px dashed #0284c7 !important;
    border-left: 1px solid #94a3b8 !important;
    border-right: 1px solid #94a3b8 !important;
    padding: 1.5px 4px !important;
    text-align: center !important;
  }
  .roster-ishoma-bar {
    display: flex;
    align-items: center;
    justify-content: center;
    color: #0369a1;
    font-weight: 800;
    font-size: 8.8px;
    letter-spacing: 0.03em;
    text-transform: uppercase;
    line-height: 1.15;
  }
  .roster-ishoma-label {
    font-weight: 800;
  }

  /* Simple Roster Bottom Section & Footer (Pure Ambrilindo Local - NO SIGNATURES) */
  .simple-roster-bottom-section {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    border-top: 1.5px solid #94a3b8;
    padding-top: 3px;
    margin-top: auto;
    page-break-inside: avoid;
  }
  .simple-roster-bottom-left {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .simple-roster-bottom-right {
    flex-shrink: 0;
    text-align: right;
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    justify-content: center;
    gap: 2px;
  }
  .simple-roster-footer-logo {
    height: 28px !important;
    max-height: 28px !important;
    width: auto !important;
    max-width: 110px !important;
    object-fit: contain !important;
    display: block !important;
  }
  .simple-roster-reminder-box {
    display: flex;
    align-items: center;
    gap: 6px;
    background: #fffbeb;
    border: 1px solid #fef3c7;
    border-left: 3.5px solid #d97706;
    padding: 3px 8px;
    border-radius: 3px;
  }
  .reminder-badge {
    background: #d97706;
    color: #ffffff;
    font-size: 8.5px;
    font-weight: 800;
    padding: 1.5px 5px;
    border-radius: 2px;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    flex-shrink: 0;
  }
  .reminder-text {
    font-size: 9.5px;
    font-weight: 700;
    color: #92400e;
    line-height: 1.25;
  }
  .simple-roster-unassigned {
    font-size: 8px;
    line-height: 1.25;
    color: #1e293b;
    background: #f8fafc;
    border: 1px solid #cbd5e1;
    border-left: 3.5px solid #64748b;
    padding: 2px 6px;
    border-radius: 2px;
    word-break: break-word;
  }
  .unassigned-title {
    font-weight: 800;
    color: #b91c1c;
    text-transform: uppercase;
    margin-right: 4px;
  }
  .unassigned-items {
    font-weight: 600;
    color: #0f172a;
  }
  .simple-roster-ts {
    font-size: 7.5px;
    font-weight: 600;
    color: #475569;
    white-space: nowrap;
  }

  /* Compact sheet (rows as tall as their text; large bold text) */
  .simple-roster-page {
    display: flex;
    flex-direction: column;
    padding: 0;
    overflow: hidden;
  }
  /* Compact roster: rows as tall as their text plus a little breathing room; large bold text. */
  .simple-roster-day-section {
    margin-bottom: 0;
  }
  .simple-roster-bottom-section {
    margin-top: 4px;
  }
  .simple-roster-table th {
    padding: 3px 4px;
    font-size: 11px;
    font-weight: 900;
    line-height: 1.15;
  }
  .simple-roster-table th .th-sub {
    font-size: 8.5px;
    font-weight: 700;
  }
  .simple-roster-page td.roster-match-cell {
    padding: 3px 4px;
    font-size: 10.5px;
    line-height: 1.2;
    vertical-align: middle;
  }
  .roster-item-line {
    gap: 4px;
    align-items: center;
    line-height: 1.2;
  }
  .roster-item-title {
    font-weight: 800;
    margin-top: 0;
    line-height: 1.2;
  }
  .roster-atlet-count {
    font-weight: 800;
    margin-top: 0;
    line-height: 1.2;
    align-self: center;
  }
  .roster-num-badge {
    min-width: 18px;
    margin-top: 0;
    padding: 1px 3px;
    font-size: 9px;
    font-weight: 900;
    border-radius: 2px;
    line-height: 1.15;
  }
  .roster-match-cell.is-empty {
    font-size: 9px;
  }
  .roster-ishoma-cell {
    padding: 2px 4px !important;
  }
  .roster-ishoma-bar {
    font-size: 10px;
  }
</style>
