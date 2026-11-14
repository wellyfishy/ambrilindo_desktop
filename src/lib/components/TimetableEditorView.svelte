<script lang="ts">
  import { cellDisplayName, missingSplitParts, splitPartShare, SPLIT_PART_LABEL, MIN_SPLIT_ENTRIES } from "$lib/timetable_split";
  import { onMount } from "svelte";
  import { registerUnsavedGuard } from "$lib/unsavedGuard.svelte";
  import { FORKI_LOGO_DATA_URL, AMBRILINDO_LOGO_DATA_URL } from "$lib/assets_base64";
  import { invoke } from "$lib/api_client";
  import { printWithOrientation } from "$lib/print_helper";
  import BracketPaperPage from "$lib/components/BracketPaperPage.svelte";
  import { computePodium, deriveBracketMeta, isFestivalName, winnersToPodium } from "$lib/bracket_layout";
  import {
    AUTO_SORT_MODES,
    computeColumnTimes,
    customTextAthletes,
    customTextMatches,
    customTextMinutes,
    festivalSessionText,
    formatMinutesToTime,
    getAgeGroupRank,
    isFestivalText,
    isJedaText,
    labelRowWindow,
    parseTimeToMinutes,
    rowConflicts,
    runAutoSchedule,
    textCell,
    type AutoSortMode,
  } from "$lib/auto_schedule";
  import type {
    AthleteConflictDetail,
    DetailBaganItem,
    EnrichedTimetableCell,
    EnrichedTimetableDay,
    EnrichedTimetableRow,
    KopSurat,
    SaveTimetableCellInput,
    SaveTimetableDayInput,
    SaveTimetablePayload,
    SaveTimetableRowInput,
    TatamiBookletCategory,
    SplitPart,
    TatamiBookletData,
    TatamiItem,
    TatamiLiveStatus,
    TimetableCategoryOption,
    TimetableEditorData,
    TimetableProgressSummary,
    FestivalSessionTatami,
  } from "$lib/types";

  interface Props {
    eventId: number;
    eventName: string;
    hostAddress: string;
    onShowToast?: (msg: string) => void;
  }

  let { eventId, eventName, hostAddress, onShowToast }: Props = $props();

  // --- Main Data State ---
  let isLoading = $state(true);
  let isSaving = $state(false);
  let isDirty = $state(false);
  /** Open header dropdown (Cetak, Atur Jadwal, + Sesi). */
  let openMenu = $state<"cetak" | "atur" | "sesi" | `tatami-${number}` | null>(null);

  // --- Confirmation Modal State (Explicit Bulletproof Confirmation) ---
  let showConfirmModal = $state(false);
  let isConfirmLoading = $state(false);
  let confirmModalConfig = $state<{
    action: "reset_all" | "delete_day" | "delete_tatami" | "clear_day" | "discard_changes" | null;
    title: string;
    message: string;
    confirmText: string;
    isDestructive: boolean;
    tatamiId?: number;
    dayId?: number;
  }>({
    action: null,
    title: "",
    message: "",
    confirmText: "Lanjutkan",
    isDestructive: true,
  });

  function openConfirmModal(config: {
    action: "reset_all" | "delete_day" | "delete_tatami" | "clear_day" | "discard_changes";
    title: string;
    message: string;
    confirmText?: string;
    isDestructive?: boolean;
    tatamiId?: number;
    dayId?: number;
  }) {
    confirmModalConfig = {
      action: config.action,
      title: config.title,
      message: config.message,
      confirmText: config.confirmText || "Lanjutkan",
      isDestructive: config.isDestructive !== false,
      tatamiId: config.tatamiId,
      dayId: config.dayId,
    };
    showConfirmModal = true;
  }

  function closeConfirmModal() {
    if (isConfirmLoading) return;
    showConfirmModal = false;
    confirmModalConfig.action = null;
  }

  async function handleExecuteConfirmedAction() {
    if (!confirmModalConfig.action || isConfirmLoading) return;
    isConfirmLoading = true;
    try {
      if (confirmModalConfig.action === "reset_all") {
        await invoke("reset_timetable", { eventId });
        notify("Seluruh jadwal pertandingan berhasil di-reset!");
        showConfirmModal = false;
        await loadTimetable();
      } else if (confirmModalConfig.action === "delete_day") {
        if (!activeDay) return;
        if (activeDay.id > 0) {
          await invoke("delete_timetable_day", { dayId: activeDay.id });
        }
        days = days.filter((_, idx) => idx !== activeDayIndex);
        days.forEach((d, idx) => {
          d.order = idx;
          d.label = `Day ${idx + 1}`;
        });
        activeDayIndex = Math.max(0, Math.min(activeDayIndex, days.length - 1));
        markDirty();
        notify("Hari berhasil dihapus dan urutan hari diperbarui.");
        showConfirmModal = false;
        await loadTimetable();
      } else if (confirmModalConfig.action === "delete_tatami") {
        const tid = confirmModalConfig.tatamiId;
        if (tid) {
          await invoke("delete_tatami", { tatamiId: tid });
          tatamis = tatamis.filter((x) => x.id !== tid);
          days.forEach((d) => {
            d.rows.forEach((r) => {
              r.cells = r.cells.filter((c) => c.tatami_id !== tid);
            });
          });
          markDirty();
          notify("Tatami berhasil dihapus.");
        }
        showConfirmModal = false;
      } else if (confirmModalConfig.action === "clear_day") {
        if (activeDay) {
          activeDay.rows.forEach((r) => {
            if (r.row_type === "slot") {
              r.cells.forEach((c) => {
                c.nomor_tanding_id = null;
                c.nomor_tanding_nama = null;
                c.part = null;
                c.custom_text = "";
                c.atlet_count = 0;
                c.matches = 0;
                c.est_duration_minutes = 0;
                c.is_festival = false;
                c.is_break = false;
                c.progress_status = "belum_disentuh";
                c.progress_pct = 0;
                c.finished_matches = 0;
                c.remaining_matches = 0;
              });
            }
          });
          markDirty();
          notify(`Jadwal pada ${activeDay.label} telah dikosongkan.`);
        }
        showConfirmModal = false;
      } else if (confirmModalConfig.action === "discard_changes") {
        showConfirmModal = false;
        autoResultBanner = null;
        backupDaysForUndo = null;
        await loadTimetable();
        notify("Perubahan yang belum disimpan dibuang.");
      }
    } catch (err: any) {
      notify(`Gagal menjalankan aksi: ${err?.message || err}`);
    } finally {
      isConfirmLoading = false;
    }
  }

  // --- Buku LO Tatami Data State ---
  let tatamiBookletLoading = $state(false);
  let tatamiBookletData = $state<TatamiBookletData | null>(null);
  let bookletLogoMap = $state<Record<string, string>>({});

  let eventDateRangeStr = $derived.by(() => {
    if (!tatamiBookletData) return "";
    const dateList = days
      .map((d) => d.tanggal)
      .filter((t): t is string => Boolean(t && t.trim()))
      .sort();
    const fmt = (iso: string) => {
      try {
        const parts = iso.split("-");
        if (parts.length === 3) {
          const months = [
            "Januari", "Februari", "Maret", "April", "Mei", "Juni",
            "Juli", "Agustus", "September", "Oktober", "November", "Desember"
          ];
          const d = parseInt(parts[2], 10);
          const m = months[parseInt(parts[1], 10) - 1];
          const y = parts[0];
          return `${d} ${m} ${y}`;
        }
      } catch {}
      return iso;
    };
    if (dateList.length >= 2 && dateList[0] !== dateList[dateList.length - 1]) {
      return `${fmt(dateList[0])} - ${fmt(dateList[dateList.length - 1])}`;
    }
    if (dateList.length === 1) {
      return fmt(dateList[0]);
    }
    return tatamiBookletData.day_date || activeDay?.tanggal || "3 Oktober 2026";
  });

  let venueLine1 = $derived.by(() => {
    if (!tatamiBookletData) return "";
    const raw = tatamiBookletData.kop_surat?.alamat || tatamiBookletData.venue || kopAlamat || "";
    const lines = raw.split("\n").map((l: string) => l.trim()).filter(Boolean);
    if (lines.length === 1 && lines[0].includes(",")) {
      return lines[0].split(",", 2)[0].trim();
    }
    return lines[0] || "Gedung Olahraga SMPN & SMAN 1 Samarinda";
  });

  let venueLine2 = $derived.by(() => {
    if (!tatamiBookletData) return "";
    const raw = tatamiBookletData.kop_surat?.alamat || tatamiBookletData.venue || kopAlamat || "";
    const lines = raw.split("\n").map((l: string) => l.trim()).filter(Boolean);
    if (lines.length === 1 && lines[0].includes(",")) {
      return lines[0].split(",", 2)[1].trim();
    }
    if (lines.length >= 2) {
      return lines.slice(1).join(", ");
    }
    return "";
  });

  let tatamis = $state<TatamiItem[]>([]);
  let days = $state<EnrichedTimetableDay[]>([]);
  let categories = $state<TimetableCategoryOption[]>([]);
  /** Roster-format festival: per-tatami load for the single FESTIVAL session (null otherwise). */
  let festivalSession = $state<FestivalSessionTatami[] | null>(null);
  let kopSurat = $state<KopSurat | null>(null);
  let summary = $state<TimetableProgressSummary>({
    total_matches: 0,
    finished_matches: 0,
    remaining_matches: 0,
    running_categories_count: 0,
    finished_categories_count: 0,
    unstarted_categories_count: 0,
    total_categories_count: 0,
    overall_pct: 0,
    total_conflicts: 0,
    tatamis_live: [],
  });

  let activeDayIndex = $state(0);
  let startTimeStr = $state("08:30");

  // --- Category Pool Sidebar State ---
  let isSidebarOpen = $state(true);
  let poolSearch = $state("");
  let poolGroupFilter = $state("ALL"); // 'ALL' | 'TOURNAMENT' | 'FESTIVAL' | or Age Group

  // --- Auto-Jadwal Cerdas State ---
  let autoScheduleModalOpen = $state(false);
  let autoIncludeFestival = $state(true);
  let autoFestivalDur = $state(45); // in minutes
  let autoIncludeIshoma = $state(true);
  let autoOverwriteExisting = $state(true);
  let autoSortMode = $state<AutoSortMode>("kata_then_kumite");
  let autoEqualMatches = $state(true);
  let autoPreventConflicts = $state(true);
  let autoSelectedAgeGroups = $state<Record<string, boolean>>({});
  let autoResultBanner = $state<{ title: string; summary: string } | null>(null);
  let backupDaysForUndo = $state<EnrichedTimetableDay[] | null>(null);
  let ntAthletesMap = $state<Record<number, string[]>>({});

  // --- Kop Surat Modal State ---
  let kopModalOpen = $state(false);
  let isSavingKop = $state(false);
  let kopNama = $state("");
  let kopAlamat = $state("");
  let kopKontak = $state("");
  let kopLogo = $state<string | null>(null);

  // --- Quick Session Prompts State ---
  let festivalPromptOpen = $state(false);
  let festivalTitleInput = $state("FESTIVAL");
  let festivalAthletesInput = $state(15);

  let ishomaPromptOpen = $state(false);
  let ishomaTimeInput = $state("12:00 - 13:00");
  let ishomaTitleInput = $state("ISHOMA / ISTIRAHAT SIANG");

  // --- Print / Export Modals State ---
  let printMode = $state<"none" | "simple_roster" | "detailed_timeline" | "call_sheet" | "booklet">("none");
  let printSelectedTatami = $state<TatamiItem | null>(null);

  // --- Conflict Modal ---

  let conflictModalOpen = $state(false);
  let activeConflictRow = $state<EnrichedTimetableRow | null>(null);

  let deleteDayModalOpen = $state(false);
  let isDeletingDay = $state(false);

  const ORDERED_GROUPS = [
    "Pra Usia Dini",
    "Usia Dini",
    "Pra Pemula",
    "Pemula",
    "Kadet",
    "Junior",
    "Under-21",
    "Senior",
    "Veteran",
    "Lainnya",
  ];

  function notify(msg: string) {
    if (onShowToast) onShowToast(msg);
  }

  function markDirty() {
    isDirty = true;
  }

  // --- Data Loading ---
  async function loadTimetable() {
    if (!eventId) return;
    isLoading = true;
    try {
      const data = await invoke<TimetableEditorData>("get_timetable_editor_data", { eventId });
      tatamis = data.tatamis;
      days = data.days;
      festivalSession = data.festival_session ?? null;
      // In the roster format festival is one session per tatami, never single categories.
      categories = festivalSession ? data.categories.filter((c) => !c.is_festival) : data.categories;
      summary = data.summary;
      kopSurat = data.kop_surat;
      ntAthletesMap = data.nt_athletes || {};

      if (kopSurat) {
        kopNama = kopSurat.nama_organisasi;
        kopAlamat = kopSurat.alamat;
        kopKontak = kopSurat.kontak;
        kopLogo = kopSurat.logo;
      } else {
        kopNama = eventName;
      }

      // Initialize selected age groups for auto-scheduler
      const ageSet: Record<string, boolean> = {};
      ORDERED_GROUPS.forEach((g) => (ageSet[g] = true));
      autoSelectedAgeGroups = ageSet;

      if (activeDayIndex >= days.length) {
        activeDayIndex = Math.max(0, days.length - 1);
      }
      isDirty = false;
    } catch (err: any) {
      console.error("Gagal memuat jadwal:", err);
      notify(`Gagal memuat jadwal: ${err}`);
    } finally {
      isLoading = false;
    }
  }

  // --- Derived Calculations ---
  let activeDay = $derived(days[activeDayIndex] || null);

  let totalSlotsCount = $derived.by(() => {
    return days.reduce((sum, d) => sum + d.rows.filter((r) => r.row_type === "slot").length, 0);
  });

  /** Split categories with a part (e.g. the final) not on the timetable yet. */
  let missingPartsByCat = $derived.by(() => {
    const placed = new Map<number, (SplitPart | null | undefined)[]>();
    days.forEach((d) =>
      d.rows.forEach((r) => {
        if (r.row_type !== "slot") return;
        r.cells.forEach((c) => {
          if (!c.nomor_tanding_id) return;
          const list = placed.get(c.nomor_tanding_id) || [];
          list.push(c.part);
          placed.set(c.nomor_tanding_id, list);
        });
      })
    );
    const out = new Map<number, ("top" | "bottom" | "final")[]>();
    placed.forEach((parts, id) => {
      const missing = missingSplitParts(parts);
      if (missing.length > 0) out.set(id, missing);
    });
    return out;
  });

  let assignedCategoryIds = $derived.by(() => {
    const ids = new Set<number>();
    days.forEach((d) => {
      d.rows.forEach((r) => {
        if (r.row_type === "slot") {
          r.cells.forEach((c) => {
            if (c.nomor_tanding_id) ids.add(c.nomor_tanding_id);
          });
        }
      });
    });
    return ids;
  });

  let hasFestivalCategories = $derived(categories.some((c) => c.is_festival));

  function isFestivalSessionCell(c: EnrichedTimetableCell | undefined | null): boolean {
    return !!c && !c.nomor_tanding_id && isFestivalText(c.custom_text);
  }

  /** Cells of the roster-format FESTIVAL session, one per tatami (empty where the roster has none). */
  function buildFestivalSessionCells(): EnrichedTimetableCell[] {
    return tatamis.map((t) => {
      const plan = festivalSession?.find((f) => f.tatami_id === t.id);
      const tNumber = t.tatami_number ?? 1;
      if (!plan) return { ...textCell(t.id, tNumber, ""), custom_text: "" };
      return textCell(t.id, tNumber, festivalSessionText(plan.athletes, plan.matches, plan.est_minutes));
    });
  }

  /** Printed title / athlete count of a text cell: the roster festival session is just "FESTIVAL". */
  function textCellTitle(c: EnrichedTimetableCell): string {
    return isFestivalSessionCell(c) && customTextAthletes(c.custom_text) > 0 ? "FESTIVAL" : c.custom_text;
  }
  function textCellAthletes(c: EnrichedTimetableCell): number {
    return c.atlet_count || (isFestivalSessionCell(c) ? customTextAthletes(c.custom_text) : 0);
  }

  /** A FESTIVAL session already exists on another day (the roster runs once). */
  let festivalSessionOnOtherDay = $derived(
    days.some((d, i) => i !== activeDayIndex && d.rows.some((r) => r.cells.some(isFestivalSessionCell)))
  );

  let unscheduledTournamentCount = $derived.by(() => {
    return categories.filter((c) => !assignedCategoryIds.has(c.id)).length;
  });

  let poolFilteredCategories = $derived.by(() => {
    return categories.filter((c) => {
      const q = poolSearch.trim().toLowerCase();
      const matchSearch = !q || c.nama.toLowerCase().includes(q) || c.age_group.toLowerCase().includes(q);

      let matchGroup = true;
      if (poolGroupFilter === "ALL") {
        matchGroup = true;
      } else if (poolGroupFilter === "UNASSIGNED") {
        matchGroup = !assignedCategoryIds.has(c.id);
      } else if (poolGroupFilter === "ASSIGNED") {
        matchGroup = assignedCategoryIds.has(c.id);
      } else if (poolGroupFilter === "TOURNAMENT") {
        matchGroup = !c.is_festival;
      } else if (poolGroupFilter === "VALID_TOURNAMENT") {
        matchGroup = !c.is_festival && c.atlet_count > 1;
      } else if (poolGroupFilter === "SOLO") {
        matchGroup = !c.is_festival && c.atlet_count <= 1;
      } else if (poolGroupFilter === "FESTIVAL") {
        matchGroup = c.is_festival;
      } else {
        matchGroup = c.age_group === poolGroupFilter;
      }

      return matchSearch && matchGroup;
    });
  });

  // Start/end of every cell and each tatami's totals, with the same time model the
  // auto-scheduler plans with (src/lib/auto_schedule.ts).
  let columnTimes = $derived.by(() => {
    const out: Record<number, ReturnType<typeof computeColumnTimes>> = {};
    if (!activeDay) return out;
    const start = parseTimeToMinutes(startTimeStr);
    tatamis.forEach((t) => (out[t.id] = computeColumnTimes(activeDay.rows, t.id, start)));
    return out;
  });

  let tatamiTimelines = $derived.by(() => {
    const timelines: Record<number, { totalMatches: number; totalAthletes: number; finishTime: string }> = {};
    for (const [tid, c] of Object.entries(columnTimes)) {
      timelines[+tid] = { totalMatches: c.matches, totalAthletes: c.athletes, finishTime: formatMinutesToTime(c.finish) };
    }
    return timelines;
  });

  let cellTimes = $derived.by(() => {
    const map: Record<string, { start: string; end: string; minutes: number }> = {};
    for (const [tid, c] of Object.entries(columnTimes)) {
      for (const [rIdx, t] of Object.entries(c.slots)) {
        map[`${rIdx}:${tid}`] = {
          start: formatMinutesToTime(t.start),
          end: formatMinutesToTime(t.end),
          minutes: t.end - t.start,
        };
      }
    }
    return map;
  });

  // Unscheduled categories first; scheduled ones stay listed below, dimmed.
  let poolSortedCategories = $derived(
    [...poolFilteredCategories].sort(
      (a, b) => Number(assignedCategoryIds.has(a.id)) - Number(assignedCategoryIds.has(b.id))
    )
  );

  function getTatamiCellSeq(targetTatamiId: number, targetRowIdx: number, dayRows?: EnrichedTimetableRow[]): number {
    const rows = dayRows ?? activeDay?.rows;
    if (!rows) return 1;
    let seq = 1;
    for (let r = 0; r < rows.length; r++) {
      const row = rows[r];
      if (row.row_type === "label") continue;
      const cell = row.cells.find((c) => c.tatami_id === targetTatamiId);
      if (r === targetRowIdx) {
        return seq;
      }
      if (cell && (cell.nomor_tanding_id || (cell.custom_text && !cell.is_break))) {
        seq++;
      }
    }
    return seq;
  }

  // "Roster 1 halaman" covers the whole event: every day that has a schedule (or the open
  // day when nothing is scheduled yet), each with every tatami.
  let rosterDays = $derived.by(() => {
    const withRows = days.filter((d) => d.rows.length > 0);
    return withRows.length > 0 ? withRows : activeDay ? [activeDay] : [];
  });

  /** Per tatami of one day: bouts and athletes, the same numbers as the grid header. */
  function dayTatamiTotals(day: EnrichedTimetableDay): Record<number, { matches: number; athletes: number }> {
    const start = parseTimeToMinutes(startTimeStr);
    const out: Record<number, { matches: number; athletes: number }> = {};
    for (const t of tatamis) {
      const c = computeColumnTimes(day.rows, t.id, start);
      out[t.id] = { matches: c.matches, athletes: c.athletes };
    }
    return out;
  }

  let simpleRosterDensityClass = $derived.by(() => {
    if (rosterDays.length === 0) return "density-normal";
    // a day bar + table header count as rows too
    const totalRows = rosterDays.reduce((n, d) => n + d.rows.length + (rosterDays.length > 1 ? 2 : 0), 0);
    if (totalRows <= 20) return "density-spacious";
    if (totalRows <= 35) return "density-normal";
    if (totalRows <= 50) return "density-compact";
    return "density-ultra-compact";
  });

  // "Roster 1 halaman": the sheet has the exact A4-portrait content box used in print, and
  // the roster is shrunk (CSS zoom) when it is taller than one page.
  let rosterPageEl = $state<HTMLDivElement | null>(null);
  let rosterFitZoom = $state(1);
  /** A short roster prints on A4 landscape; one that does not fit there at full size goes portrait. */
  let rosterLandscape = $state(true);
  /** The kop logo could not be loaded (e.g. a local path on a client laptop): show the default logos. */
  let kopLogoFailed = $state(false);
  $effect(() => {
    void kopLogo;
    kopLogoFailed = false;
  });

  $effect(() => {
    const el = rosterPageEl;
    if (printMode !== "simple_roster" || !el) return;
    void rosterDays.map((d) => d.rows.length).join(",");
    void simpleRosterDensityClass;
    void tatamis.length;
    // 1. try landscape at full size; 2. too tall -> portrait, shrunk if still needed
    rosterFitZoom = 1;
    rosterLandscape = true;
    let frame = requestAnimationFrame(() => {
      if (el.scrollHeight <= el.clientHeight + 1) return;
      rosterLandscape = false;
      frame = requestAnimationFrame(() => {
        const need = el.scrollHeight;
        const have = el.clientHeight;
        rosterFitZoom = need > have + 1 ? Math.max(0.3, have / need) : 1;
      });
    });
    return () => cancelAnimationFrame(frame);
  });

  let unassignedLabelsForPrint = $derived.by(() => {
    const out: string[] = [];
    for (const c of categories) {
      if (c.is_festival) continue;
      if (!assignedCategoryIds.has(c.id)) out.push(`${c.nama} (${c.atlet_count})`);
      else for (const part of missingPartsByCat.get(c.id) || []) out.push(`${c.nama} (${SPLIT_PART_LABEL[part]})`);
    }
    return out;
  });

  let unassignedCategoriesForPrint = $derived.by(() => {
    return categories.filter((c) => !assignedCategoryIds.has(c.id) && !c.is_festival);
  });

  let printedTimeStr = $derived.by(() => {
    const now = new Date();
    return (
      now.toLocaleDateString("id-ID", { day: "numeric", month: "short", year: "numeric" }) +
      " " +
      String(now.getHours()).padStart(2, "0") +
      ":" +
      String(now.getMinutes()).padStart(2, "0")
    );
  });

  // --- Tatami Booklet Landscape Roster Columns ---
  let tatamiRosterEntries = $derived.by(() => {
    if (!activeDay || !tatamiBookletData) return [];
    const targetId = printSelectedTatami?.id || tatamiBookletData.tatami_id;
    const targetNum = tatamiBookletData.tatami_number;
    const list: {
      type: "match" | "label";
      seq?: number;
      label?: string;
      nomor_tanding_nama?: string;
      atlet_count?: number;
      isKata?: boolean;
      isFestival?: boolean;
    }[] = [];

    activeDay.rows.forEach((row, rIdx) => {
      if (row.row_type === "label") {
        list.push({
          type: "label",
          label: row.label_text || "ISHOMA / ISTIRAHAT SIANG",
        });
      } else {
        const cell = row.cells.find((c) => c.tatami_id === targetId || c.tatami_id === targetNum);
        if (cell && cell.is_break && !cell.nomor_tanding_id && !isJedaText(cell.custom_text)) {
          list.push({ type: "label", label: cell.custom_text });
        } else if (cell && (cell.nomor_tanding_id || (cell.custom_text && !cell.is_break))) {
          const seq = getTatamiCellSeq(cell.tatami_id, rIdx);
          const name = cellDisplayName(cell);
          const isKata = name.toUpperCase().includes("KATA");
          const isFest = Boolean(cell.is_festival || name.toUpperCase().includes("FESTIVAL"));
          list.push({
            type: "match",
            seq,
            nomor_tanding_nama: name,
            atlet_count: cell.atlet_count || 0,
            isKata,
            isFestival: isFest,
          });
        }
      }
    });

    return list;
  });

  let tatamiRosterColumns = $derived.by(() => {
    const entries = tatamiRosterEntries;
    if (entries.length === 0) return { col1: [], col2: [], labelItem: null };

    // Split naturally by first ISHOMA / label if present in middle region
    const labelIdx = entries.findIndex((e) => e.type === "label");
    if (labelIdx !== -1 && labelIdx >= 2 && labelIdx <= entries.length - 2) {
      return {
        col1: entries.slice(0, labelIdx),
        labelItem: entries[labelIdx],
        col2: entries.slice(labelIdx + 1),
      };
    }

    // Otherwise split evenly
    const mid = Math.ceil(entries.length / 2);
    return {
      col1: entries.slice(0, mid),
      labelItem: null,
      col2: entries.slice(mid),
    };
  });

  // --- Auto-Jadwal Cerdas (Smart Auto-Scheduler Engine) ---
  function openAutoScheduleModal() {
    autoScheduleModalOpen = true;
  }

  function toggleAgeGroup(grp: string) {
    autoSelectedAgeGroups[grp] = !autoSelectedAgeGroups[grp];
  }

  function selectAllAgeGroups(val: boolean) {
    const updated: Record<string, boolean> = {};
    ORDERED_GROUPS.forEach((g) => (updated[g] = val));
    autoSelectedAgeGroups = updated;
  }

  function detectRowConflicts(cells: EnrichedTimetableCell[]): AthleteConflictDetail[] {
    return rowConflicts(cells, ntAthletesMap);
  }

  function executeAutoScheduler() {
    if (!activeDay) return;
    if (tatamis.length === 0) {
      notify("Tidak ada tatami aktif untuk event ini.");
      autoScheduleModalOpen = false;
      return;
    }

    const assignedOnOtherDays = new Set<number>();
    days.forEach((d, dIdx) => {
      if (dIdx === activeDayIndex) return;
      d.rows.forEach((r) => r.cells.forEach((c) => c.nomor_tanding_id && assignedOnOtherDays.add(c.nomor_tanding_id)));
    });

    const result = runAutoSchedule(
      {
        dayId: activeDay.id,
        categories: $state.snapshot(categories) as TimetableCategoryOption[],
        tatamis: tatamis.map((t) => ({ id: t.id, tatami_number: t.tatami_number })),
        dayRows: $state.snapshot(activeDay.rows) as EnrichedTimetableRow[],
        assignedOnOtherDays,
        ntAthletes: $state.snapshot(ntAthletesMap),
      },
      {
        startMin: parseTimeToMinutes(startTimeStr),
        sortMode: autoSortMode,
        includeFestival: autoIncludeFestival,
        festivalSessionMinutes: autoFestivalDur,
        festivalSession: festivalSession
          ? festivalSession.map((f) => ({
              tatamiId: f.tatami_id,
              text: festivalSessionText(f.athletes, f.matches, f.est_minutes),
              minutes: f.est_minutes,
            }))
          : null,
        festivalSessionElsewhere: festivalSessionOnOtherDay,
        includeLunch: autoIncludeIshoma,
        balanceFinish: autoEqualMatches,
        preventClashes: autoPreventConflicts,
        rebuildDay: autoOverwriteExisting,
        isAgeGroupSelected: (c) => {
          const grp = ORDERED_GROUPS[getAgeGroupRank(c.age_group, c.nama)] || "Lainnya";
          return autoSelectedAgeGroups[grp] !== false && autoSelectedAgeGroups[c.age_group] !== false;
        },
      }
    );

    if (result.scheduled === 0 && !result.festivalSessionAdded) {
      const empty = result.skippedEmpty.length > 0 ? ` (${result.skippedEmpty.length} kategori tanpa atlet dilewati)` : "";
      notify(`Tidak ada kategori yang bisa dijadwalkan otomatis${empty}.`);
      autoScheduleModalOpen = false;
      return;
    }

    backupDaysForUndo = $state.snapshot(days) as EnrichedTimetableDay[];
    activeDay.rows = result.rows;
    summary.total_conflicts = days.reduce(
      (acc, d) => acc + d.rows.reduce((a, r) => a + (r.conflicts?.length || 0), 0),
      0
    );
    markDirty();
    autoScheduleModalOpen = false;

    const parts = [`${result.scheduled} kategori`, `${result.clashes.length} bentrok atlet`];
    if (result.festivalSessionAdded) parts.unshift("sesi FESTIVAL");
    if (result.skippedEmpty.length > 0) parts.push(`${result.skippedEmpty.length} kategori tanpa atlet dilewati`);
    autoResultBanner = {
      title: `Auto-jadwal diterapkan: ${parts.join(" • ")}`,
      summary: result.perTatami
        .map((t) => `Tatami ${t.tatamiNumber}: ${t.matches} partai, selesai ${formatMinutesToTime(t.finish)}`)
        .join(" • "),
    };
    notify(`Auto-jadwal selesai: ${result.scheduled} kategori, ${result.clashes.length} bentrok.`);
  }

  function undoLastAutoSchedule() {
    if (backupDaysForUndo) {
      days = backupDaysForUndo;
      backupDaysForUndo = null;
      autoResultBanner = null;
      markDirty();
      notify("Auto-jadwal berhasil diurungkan.");
    }
  }

  // --- Quick Session Prompts ---
  function openFestivalSessionPrompt() {
    if (!festivalSession) {
      festivalPromptOpen = true;
      return;
    }
    if (!activeDay) return;
    if (activeDay.rows.some((r) => r.cells.some(isFestivalSessionCell)) || festivalSessionOnOtherDay) {
      notify("Sesi FESTIVAL sudah ada di jadwal.");
      return;
    }
    activeDay.rows = [
      ...activeDay.rows,
      {
        id: 0,
        day_id: activeDay.id,
        order: activeDay.rows.length,
        row_type: "slot",
        time_label: "",
        label_text: "",
        cells: buildFestivalSessionCells(),
        conflicts: [],
      },
    ];
    markDirty();
    notify("Sesi FESTIVAL ditambahkan di semua tatami (sesuai roster festival).");
  }

  function insertFestivalSession() {
    if (!activeDay) return;
    const festAthletesPerTatami = Math.round(festivalAthletesInput / Math.max(1, tatamis.length));

    const newCells: EnrichedTimetableCell[] = tatamis.map((t) => ({
      id: 0,
      row_id: 0,
      tatami_id: t.id,
      tatami_number: t.tatami_number ?? 1,
      nomor_tanding_id: null,
      nomor_tanding_nama: null,
      custom_text: `${festivalTitleInput.trim()} (${festAthletesPerTatami} Atlet)`,
      atlet_count: festAthletesPerTatami,
      matches: festAthletesPerTatami,
      discipline: "kata",
      est_duration_minutes: 45,
      progress_status: "festival",
      progress_pct: 0,
      finished_matches: 0,
      remaining_matches: festAthletesPerTatami,
      is_festival: true,
      is_break: false,
    }));

    activeDay.rows = [
      ...activeDay.rows,
      {
        id: 0,
        day_id: activeDay.id,
        order: activeDay.rows.length,
        row_type: "slot",
        time_label: "Sesi Festival",
        label_text: "",
        cells: newCells,
        conflicts: [],
      },
    ];

    festivalPromptOpen = false;
    markDirty();
    notify("Sesi Festival serentak berhasil ditambahkan.");
  }

  function insertIshomaSession() {
    if (!activeDay) return;

    activeDay.rows = [
      ...activeDay.rows,
      {
        id: 0,
        day_id: activeDay.id,
        order: activeDay.rows.length,
        row_type: "label",
        time_label: ishomaTimeInput,
        label_text: ishomaTitleInput,
        cells: [],
        conflicts: [],
      },
    ];

    ishomaPromptOpen = false;
    markDirty();
    notify("Sesi ISHOMA serentak berhasil ditambahkan.");
  }

  // --- Tatami Addition Directly in Timetable ---
  async function handleAddTatami() {
    if (!eventId) return;
    try {
      const nextNum = tatamis.length + 1;
      const created = await invoke<TatamiItem>("create_tatami", {
        eventId,
        tatamiNumber: nextNum,
      });
      tatamis = [...tatamis, created];

      // Add empty cell for this tatami across all rows
      days.forEach((d) => {
        d.rows.forEach((r) => {
          if (r.row_type === "slot") {
            r.cells.push({
              id: 0,
              row_id: r.id,
              tatami_id: created.id,
              tatami_number: nextNum,
              nomor_tanding_id: null,
              nomor_tanding_nama: null,
              custom_text: "",
              atlet_count: 0,
              matches: 0,
              discipline: "kumite",
              est_duration_minutes: 0,
              progress_status: "belum_disentuh",
              progress_pct: 0,
              finished_matches: 0,
              remaining_matches: 0,
              is_festival: false,
              is_break: false,
            });
          }
        });
      });

      markDirty();
      notify(`Tatami ${nextNum} berhasil ditambahkan`);
    } catch (err: any) {
      notify(`Gagal menambah tatami: ${err}`);
    }
  }

  function promptRemoveTatami(tatamiId: number) {
    if (tatamis.length <= 1) {
      notify("Minimal turnamen memiliki 1 tatami.");
      return;
    }
    const t = tatamis.find((x) => x.id === tatamiId);
    openConfirmModal({
      action: "delete_tatami",
      title: `Hapus Tatami ${t?.tatami_number || tatamiId}`,
      message: `Apakah Anda yakin ingin menghapus Tatami ${t?.tatami_number}? Seluruh jadwal kategori pada tatami ini akan dilepas dan dikembalikan ke status belum terjadwal di Pool Kategori.`,
      confirmText: "Hapus Tatami",
      isDestructive: true,
      tatamiId,
    });
  }

  // --- LO Tatami Booklet Fetcher ---
  async function resolveBookletLogos(data: TatamiBookletData) {
    const needed = new Set<string>();
    if (data.kop_surat?.logo) needed.add(data.kop_surat.logo);
    for (const cat of data.ordered_categories || []) {
      for (const m of cat.matches || []) {
        if (m.atlet1_logo) needed.add(m.atlet1_logo);
        if (m.atlet2_logo) needed.add(m.atlet2_logo);
      }
      for (const w of cat.winners || []) {
        if (w.logo) needed.add(w.logo);
      }
    }
    for (const path of needed) {
      if (bookletLogoMap[path]) continue;
      if (path.startsWith("http://") || path.startsWith("https://") || path.startsWith("data:")) {
        bookletLogoMap[path] = path;
        continue;
      }
      try {
        const dataUrl = await invoke<string | null>("get_media_data_url", { relativePath: path });
        if (dataUrl) {
          bookletLogoMap[path] = dataUrl;
          continue;
        }
      } catch {
        // Fallback to HTTP if Tauri IPC unavailable
      }
      if (hostAddress) {
        bookletLogoMap[path] = `${hostAddress.replace(/\/$/, "")}/media/${path.replace(/^\//, "")}`;
      }
    }
  }

  async function openTatamiBooklet(tatami: TatamiItem) {
    if (!activeDay) return;
    printSelectedTatami = tatami;
    tatamiBookletLoading = true;
    try {
      const data = await invoke<TatamiBookletData>("get_tatami_booklet_data", {
        eventId,
        dayId: activeDay.id,
        tatamiId: tatami.id,
      });
      tatamiBookletData = data;
      printMode = "booklet";
      await resolveBookletLogos(data);
    } catch (err: any) {
      console.error("Gagal memuat Buku Tatami:", err);
      notify(`Gagal memuat Buku Tatami: ${err}`);
    } finally {
      tatamiBookletLoading = false;
    }
  }

  // --- Kop Surat Modal Save ---
  async function handleSaveKop() {
    if (!eventId) return;
    isSavingKop = true;
    try {
      const saved = await invoke<KopSurat>("save_kop_surat", {
        eventId,
        namaOrganisasi: kopNama,
        alamat: kopAlamat,
        kontak: kopKontak,
        logo: kopLogo,
      });
      kopSurat = saved;
      kopModalOpen = false;
      notify("Kop surat berhasil disimpan!");
    } catch (err: any) {
      notify(`Gagal menyimpan kop surat: ${err}`);
    } finally {
      isSavingKop = false;
    }
  }

  // --- Day & Row Management ---
  async function handleAddDay() {
    if (!eventId) return;
    try {
      const newDay = await invoke<{ id: number; order: number; tanggal: string | null }>("add_timetable_day", { eventId });
      const dayNum = newDay.order + 1;
      const enrichedNewDay: EnrichedTimetableDay = {
        id: newDay.id,
        event_id: eventId,
        order: newDay.order,
        tanggal: newDay.tanggal,
        label: `Day ${dayNum}`,
        rows: [],
      };
      days = [...days, enrichedNewDay];
      activeDayIndex = days.length - 1;
      notify(`Hari ${dayNum} berhasil ditambahkan`);
    } catch (err: any) {
      notify(`Gagal menambah hari: ${err}`);
    }
  }

  function promptDeleteCurrentDay() {
    if (!activeDay || days.length <= 1) {
      notify("Tidak dapat menghapus hari (minimal harus ada 1 hari).");
      return;
    }
    openConfirmModal({
      action: "delete_day",
      title: `Hapus ${activeDay.label}`,
      message: `Apakah Anda yakin ingin menghapus ${activeDay.label}? Semua sesi pertandingan pada hari ini akan dihapus dan urutan hari berikutnya akan otomatis diurutkan ulang mulai dari Day 1.`,
      confirmText: "Hapus Hari",
      isDestructive: true,
      dayId: activeDay.id,
    });
  }

  function promptDiscardChanges() {
    openConfirmModal({
      action: "discard_changes",
      title: "Batalkan Perubahan",
      message: "Buang semua perubahan jadwal yang belum disimpan dan muat ulang jadwal terakhir yang tersimpan?",
      confirmText: "Buang Perubahan",
      isDestructive: true,
    });
  }

  function promptClearDaySlots() {
    if (!activeDay) return;
    openConfirmModal({
      action: "clear_day",
      title: `Kosongkan ${activeDay.label}`,
      message: `Kosongkan semua sesi kategori pada ${activeDay.label}? Nomor tanding akan kembali ke status belum terjadwal di Pool Kategori.`,
      confirmText: "Kosongkan Sesi",
      isDestructive: true,
    });
  }

  function emptyCellFor(t: TatamiItem): EnrichedTimetableCell {
    return {
      id: 0,
      row_id: 0,
      tatami_id: t.id,
      tatami_number: t.tatami_number ?? 1,
      nomor_tanding_id: null,
      nomor_tanding_nama: null,
      custom_text: "",
      atlet_count: 0,
      matches: 0,
      discipline: "kumite",
      est_duration_minutes: 0,
      progress_status: "belum_disentuh",
      progress_pct: 0,
      finished_matches: 0,
      remaining_matches: 0,
      is_festival: false,
      is_break: false,
      part: null,
    };
  }

  function newSlotRow(): EnrichedTimetableRow {
    return {
      id: 0,
      day_id: activeDay?.id ?? null,
      order: 0,
      row_type: "slot",
      // Left empty: the start/end time of every category is calculated automatically.
      time_label: "",
      label_text: "",
      cells: tatamis.map(emptyCellFor),
      conflicts: [],
    } as EnrichedTimetableRow;
  }

  function addSlotRow() {
    if (!activeDay) return;
    activeDay.rows = [...activeDay.rows, newSlotRow()];
    markDirty();
  }

  /** Inserts an empty session directly below `rowIdx`. */
  function insertSlotRowAfter(rowIdx: number) {
    if (!activeDay) return;
    const rows = [...activeDay.rows];
    rows.splice(rowIdx + 1, 0, newSlotRow());
    activeDay.rows = rows;
    markDirty();
  }

  function addLabelRow(label = "ISHOMA / ISTIRAHAT SIANG") {
    if (!activeDay) return;
    activeDay.rows = [
      ...activeDay.rows,
      {
        id: 0,
        day_id: activeDay.id,
        order: activeDay.rows.length,
        row_type: "label",
        time_label: "12:00 - 13:00",
        label_text: label,
        cells: [],
        conflicts: [],
      },
    ];
    markDirty();
  }

  function moveRow(rowIdx: number, direction: "up" | "down") {
    if (!activeDay) return;
    const targetIdx = direction === "up" ? rowIdx - 1 : rowIdx + 1;
    if (targetIdx < 0 || targetIdx >= activeDay.rows.length) return;

    const rowsCopy = [...activeDay.rows];
    const temp = rowsCopy[rowIdx];
    rowsCopy[rowIdx] = rowsCopy[targetIdx];
    rowsCopy[targetIdx] = temp;
    activeDay.rows = rowsCopy;
    markDirty();
  }

  function deleteRow(rowIdx: number) {
    if (!activeDay) return;
    activeDay.rows = activeDay.rows.filter((_, idx) => idx !== rowIdx);
    markDirty();
  }

  // --- Assign from Category Pool Sidebar ---
  function quickAssignFromPool(cat: TimetableCategoryOption, targetTatamiId: number) {
    if (!activeDay) return;

    // Find the first row that has an empty cell in that tatami, or create a new row
    let targetRow = activeDay.rows.find((r) => {
      if (r.row_type !== "slot") return false;
      const cell = r.cells.find((c) => c.tatami_id === targetTatamiId);
      return cell && !cell.nomor_tanding_id && !cell.custom_text;
    });

    if (!targetRow) {
      addSlotRow();
      targetRow = activeDay.rows[activeDay.rows.length - 1];
    }

    const rowIdx = activeDay.rows.indexOf(targetRow);
    const tNum = tatamis.find((t) => t.id === targetTatamiId)?.tatami_number ?? "";
    placeCategory(cat, rowIdx, targetTatamiId);
    notify(`${cat.nama} ditempatkan di Tatami ${tNum}, sesi #${rowIdx + 1}.`);
  }

  // ==================== MANUAL EDITING: PLACE, MOVE, SWAP ====================
  // Fields that make up a cell's content. Moving/swapping copies these and leaves
  // the cell's identity (id, row, tatami) where it is.
  const CELL_CONTENT_KEYS = [
    "nomor_tanding_id",
    "nomor_tanding_nama",
    "custom_text",
    "atlet_count",
    "matches",
    "discipline",
    "est_duration_minutes",
    "progress_status",
    "progress_pct",
    "finished_matches",
    "remaining_matches",
    "is_festival",
    "is_break",
    "part",
  ] as const;

  /** The cell of `row` for `tatamiId`, created if the tatami was added after the row. */
  function ensureCell(row: EnrichedTimetableRow, tatamiId: number): EnrichedTimetableCell | null {
    let cell = row.cells.find((c) => c.tatami_id === tatamiId);
    if (!cell) {
      const t = tatamis.find((x) => x.id === tatamiId);
      if (!t) return null;
      row.cells = [...row.cells, emptyCellFor(t)];
      cell = row.cells[row.cells.length - 1];
    }
    return cell;
  }

  function cellHasContent(cell: EnrichedTimetableCell | undefined | null): boolean {
    return !!cell && (!!cell.nomor_tanding_id || !!cell.custom_text);
  }

  /** Re-checks athlete clashes for one session after a manual change. */
  function refreshRowConflicts(rowIdx: number) {
    const row = activeDay?.rows[rowIdx];
    if (!row || row.row_type !== "slot") return;
    row.conflicts = detectRowConflicts(row.cells);
  }

  function placeCategory(cat: TimetableCategoryOption, rowIdx: number, tatamiId: number, part: SplitPart | null = null) {
    const row = activeDay?.rows[rowIdx];
    if (!row) return;
    const cell = ensureCell(row, tatamiId);
    if (!cell) return;
    const share = splitPartShare(cat.atlet_count, cat.matches, cat.est_duration_minutes, part);
    cell.nomor_tanding_id = cat.id;
    cell.nomor_tanding_nama = cat.nama;
    cell.part = part;
    cell.atlet_count = share.entries;
    cell.matches = share.matches;
    cell.discipline = cat.discipline;
    cell.est_duration_minutes = share.minutes;
    cell.is_festival = cat.is_festival;
    cell.is_break = false;
    cell.custom_text = "";
    cell.progress_status = cat.progress_status;
    cell.progress_pct = cat.progress_pct;
    refreshRowConflicts(rowIdx);
    markDirty();
  }

  // --- Category picker (click an empty cell) ---
  let pickerTarget = $state<{ rowIdx: number; tatamiId: number } | null>(null);
  let catPickSearch = $state("");
  let pickerShowFestival = $state(false);

  let pickerOptions = $derived.by(() => {
    if (!pickerTarget) return [];
    const q = catPickSearch.trim().toLowerCase();
    return categories
      .filter((c) => !assignedCategoryIds.has(c.id))
      .filter((c) => pickerShowFestival || !c.is_festival)
      .filter((c) => !q || c.nama.toLowerCase().includes(q) || c.age_group.toLowerCase().includes(q))
      // Categories without athletes go last: there is nothing to schedule yet.
      .sort(
        (a, b) =>
          Number(a.atlet_count === 0) - Number(b.atlet_count === 0) ||
          getAgeGroupRank(a.age_group) - getAgeGroupRank(b.age_group) ||
          a.nama.localeCompare(b.nama)
      );
  });

  /** Parts of split categories still to place (e.g. a final kept for a later session). */
  let pickerPartOptions = $derived.by(() => {
    if (!pickerTarget) return [];
    const q = catPickSearch.trim().toLowerCase();
    const out: { cat: TimetableCategoryOption; part: "top" | "bottom" | "final" }[] = [];
    missingPartsByCat.forEach((parts, id) => {
      const cat = categories.find((c) => c.id === id);
      if (!cat || (q && !cat.nama.toLowerCase().includes(q) && !cat.age_group.toLowerCase().includes(q))) return;
      parts.forEach((part) => out.push({ cat, part }));
    });
    return out;
  });

  function openPicker(rowIdx: number, tatamiId: number) {
    pickerTarget = { rowIdx, tatamiId };
    catPickSearch = "";
  }

  function closePicker() {
    pickerTarget = null;
  }

  function pickCategory(cat: TimetableCategoryOption) {
    if (!pickerTarget) return;
    placeCategory(cat, pickerTarget.rowIdx, pickerTarget.tatamiId);
    pickerTarget = null;
  }

  function focusOnMount(node: HTMLInputElement) {
    node.focus();
  }

  // --- Place mode: pick a category in the pool, then click an empty cell ---
  let placeSource = $state<TimetableCategoryOption | null>(null);

  function startPlace(cat: TimetableCategoryOption) {
    moveSource = null;
    placeSource = placeSource?.id === cat.id ? null : cat;
  }

  function cancelPlace() {
    placeSource = null;
  }

  function dropPlaceOn(rowIdx: number, tatamiId: number) {
    const cat = placeSource;
    placeSource = null;
    if (!cat) return;
    placeCategory(cat, rowIdx, tatamiId);
    const tNum = tatamis.find((t) => t.id === tatamiId)?.tatami_number ?? "";
    notify(`${cat.nama} ditempatkan di Tatami ${tNum}, sesi #${rowIdx + 1}.`);
  }

  // --- Move / swap mode ---
  let moveSource = $state<{ rowIdx: number; tatamiId: number; name: string } | null>(null);

  function startMove(rowIdx: number, tatamiId: number) {
    const cell = activeDay?.rows[rowIdx]?.cells.find((c) => c.tatami_id === tatamiId);
    if (!cellHasContent(cell)) return;
    placeSource = null;
    moveSource = { rowIdx, tatamiId, name: cell!.nomor_tanding_nama || cell!.custom_text || "Kategori" };
  }

  function cancelMove() {
    moveSource = null;
  }

  /** Moves the selected cell to the target; swaps when the target is occupied. */
  function dropMoveOn(rowIdx: number, tatamiId: number) {
    if (!activeDay || !moveSource) return;
    const src = moveSource;
    moveSource = null;
    if (src.rowIdx === rowIdx && src.tatamiId === tatamiId) return;
    const fromRow = activeDay.rows[src.rowIdx];
    const toRow = activeDay.rows[rowIdx];
    if (!fromRow || !toRow || toRow.row_type !== "slot") return;
    const from = ensureCell(fromRow, src.tatamiId);
    const to = ensureCell(toRow, tatamiId);
    if (!from || !to) return;
    const swapped = cellHasContent(to);
    for (const key of CELL_CONTENT_KEYS) {
      const tmp = (to as any)[key];
      (to as any)[key] = (from as any)[key];
      (from as any)[key] = tmp;
    }
    refreshRowConflicts(src.rowIdx);
    if (rowIdx !== src.rowIdx) refreshRowConflicts(rowIdx);
    markDirty();
    const tNum = tatamis.find((t) => t.id === tatamiId)?.tatami_number ?? "";
    notify(swapped ? `Ditukar dengan isi Tatami ${tNum}, sesi #${rowIdx + 1}.` : `Dipindah ke Tatami ${tNum}, sesi #${rowIdx + 1}.`);
  }

  function handleEditorKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (openMenu) {
        openMenu = null;
        e.preventDefault();
      } else if (pickerTarget) {
        closePicker();
        e.preventDefault();
      } else if (moveSource) {
        cancelMove();
        e.preventDefault();
      } else if (placeSource) {
        cancelPlace();
        e.preventDefault();
      }
      return;
    }
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      if (isDirty && !isSaving) handleSaveTimetable();
    }
  }

  /** Opens the first session with an athlete clash (switching day if needed). */
  function showFirstConflict() {
    for (let d = 0; d < days.length; d++) {
      const idx = (activeDayIndex + d) % days.length;
      const row = days[idx]?.rows.find((r) => r.row_type === "slot" && r.conflicts.length > 0);
      if (row) {
        activeDayIndex = idx;
        activeConflictRow = row;
        conflictModalOpen = true;
        return;
      }
    }
  }

  function showUnscheduled() {
    isSidebarOpen = true;
    poolGroupFilter = "UNASSIGNED";
  }

  function clearCell(rowIdx: number, tatamiId: number) {
    if (!activeDay) return;
    const cell = activeDay.rows[rowIdx]?.cells.find((c) => c.tatami_id === tatamiId);
    if (!cell) return;

    cell.nomor_tanding_id = null;
    cell.nomor_tanding_nama = null;
    cell.part = null;
    cell.custom_text = "";
    cell.atlet_count = 0;
    cell.matches = 0;
    cell.est_duration_minutes = 0;
    cell.is_festival = false;
    cell.is_break = false;
    cell.progress_status = "belum_disentuh";
    refreshRowConflicts(rowIdx);
    markDirty();
  }

  // ==================== SPLIT A CATEGORY OVER TWO TATAMIS ====================
  // Pool atas on this tatami, pool bawah on another in the same session; the final goes with
  // one of them or is left to place in a later session (it then shows in the cell picker).
  let splitTarget = $state<{ rowIdx: number; tatamiId: number } | null>(null);
  let splitOtherTatami = $state<number | null>(null);
  let splitFinal = $state<"top" | "bottom" | "separate">("top");

  function splitCandidates(rowIdx: number, tatamiId: number) {
    const row = activeDay?.rows[rowIdx];
    return tatamis
      .filter((t) => t.id !== tatamiId)
      .map((t) => ({ t, free: !cellHasContent(row?.cells.find((c) => c.tatami_id === t.id)) }));
  }

  function openSplit(rowIdx: number, tatamiId: number) {
    splitTarget = { rowIdx, tatamiId };
    splitOtherTatami = splitCandidates(rowIdx, tatamiId).find((x) => x.free)?.t.id ?? null;
    splitFinal = "top";
  }

  function applySplit() {
    if (!activeDay || !splitTarget || splitOtherTatami == null) return;
    const { rowIdx, tatamiId } = splitTarget;
    const cell = activeDay.rows[rowIdx]?.cells.find((c) => c.tatami_id === tatamiId);
    const cat = categories.find((c) => c.id === cell?.nomor_tanding_id);
    if (!cell || !cat) return;
    placeCategory(cat, rowIdx, tatamiId, splitFinal === "top" ? "top_final" : "top");
    placeCategory(cat, rowIdx, splitOtherTatami, splitFinal === "bottom" ? "bottom_final" : "bottom");
    const tNum = (id: number) => tatamis.find((t) => t.id === id)?.tatami_number ?? "";
    notify(
      splitFinal === "separate"
        ? `${cat.nama} dibagi ke Tatami ${tNum(tatamiId)} dan ${tNum(splitOtherTatami)}. Final belum terjadwal: klik sel kosong lalu pilih ${cat.nama} (FINAL).`
        : `${cat.nama} dibagi ke Tatami ${tNum(tatamiId)} dan ${tNum(splitOtherTatami)}.`
    );
    splitTarget = null;
  }

  /** Puts the whole category back on this cell and removes its other parts everywhere. */
  function mergeSplit(rowIdx: number, tatamiId: number) {
    if (!activeDay) return;
    const cell = activeDay.rows[rowIdx]?.cells.find((c) => c.tatami_id === tatamiId);
    const cat = categories.find((c) => c.id === cell?.nomor_tanding_id);
    if (!cell || !cat) return;
    days.forEach((d) =>
      d.rows.forEach((r) => {
        let touched = false;
        r.cells.forEach((c) => {
          if (c !== cell && c.nomor_tanding_id === cat.id && c.part) {
            Object.assign(c, { ...emptyCellFor(tatamis.find((t) => t.id === c.tatami_id) ?? ({ id: c.tatami_id, tatami_number: c.tatami_number } as TatamiItem)), id: c.id, row_id: c.row_id });
            touched = true;
          }
        });
        if (touched && r.row_type === "slot") r.conflicts = detectRowConflicts(r.cells);
      })
    );
    placeCategory(cat, rowIdx, tatamiId, null);
    notify(`${cat.nama} digabung kembali di Tatami ${cell.tatami_number}.`);
  }

  // --- Save Timetable Transaction ---
  async function handleSaveTimetable(): Promise<boolean> {
    if (!eventId) return false;
    isSaving = true;
    try {
      const daysPayload: SaveTimetableDayInput[] = days.map((d) => {
        const rowsPayload: SaveTimetableRowInput[] = d.rows.map((r, rIdx) => {
          const cellsPayload: SaveTimetableCellInput[] =
            r.row_type === "slot"
              ? r.cells.map((c) => ({
                  tatami_id: c.tatami_id,
                  nomor_tanding_id: c.nomor_tanding_id,
                  custom_text: c.custom_text || "",
                  part: c.nomor_tanding_id ? (c.part ?? null) : null,
                }))
              : [];

          return {
            id: r.id > 0 ? r.id : null,
            order: rIdx,
            row_type: r.row_type,
            time_label: r.time_label || "",
            label_text: r.label_text || "",
            cells: cellsPayload,
          };
        });

        return {
          day_id: d.id,
          tanggal: d.tanggal,
          rows: rowsPayload,
        };
      });

      await invoke("save_timetable_data", { eventId, payload: { days: daysPayload } });
      notify("Jadwal tersimpan.");
      await loadTimetable();
      return true;
    } catch (err: any) {
      console.error("Gagal menyimpan jadwal:", err);
      notify(`Gagal menyimpan jadwal: ${err}`);
      return false;
    } finally {
      isSaving = false;
    }
  }

  function promptResetAllTimetable() {
    if (!eventId) return;
    openConfirmModal({
      action: "reset_all",
      title: "Reset Seluruh Jadwal Pertandingan",
      message:
        "Apakah Anda yakin ingin me-reset seluruh jadwal pertandingan? Semua sesi dan baris di semua hari akan dikosongkan secara permanen, dan semua nomor tanding akan kembali ke status belum terjadwal di Pool Kategori.",
      confirmText: "Reset Seluruh Jadwal",
      isDestructive: true,
    });
  }

  // Trigger browser print
  function triggerPrint() {
    if (printMode === "simple_roster") {
      // "Simpan PDF" names the file after the page title: ROSTER JADWAL {nama event}
      const prevTitle = document.title;
      const name = (eventName || "").replace(/[\\/:*?"<>|]+/g, " ").replace(/\s+/g, " ").trim();
      document.title = `ROSTER JADWAL ${name}`.trim();
      const restore = () => {
        document.title = prevTitle;
        window.removeEventListener("afterprint", restore);
      };
      window.addEventListener("afterprint", restore);
      printWithOrientation(rosterLandscape ? "landscape" : "portrait", "6mm");
      setTimeout(restore, 1000);
      return;
    }
    if (printMode === "booklet") {
      document.body.classList.add("is-printing");
      void document.body.offsetHeight;
      setTimeout(() => {
        window.print();
        setTimeout(() => document.body.classList.remove("is-printing"), 500);
      }, 120);
    } else {
      window.print();
    }
  }

  $effect(() => {
    if (eventId) {
      loadTimetable();
    }
  });

  // Ask before the shell navigates away from unsaved edits.
  onMount(() =>
    registerUnsavedGuard({
      label: "Jadwal pertandingan",
      isDirty: () => isDirty,
      save: handleSaveTimetable,
    })
  );

  onMount(() => {
    loadTimetable();
    const handleBeforePrint = () => {
      if (printMode === "booklet") {
        document.body.classList.add("is-printing");
        void document.body.offsetHeight;
      }
    };
    const handleAfterPrint = () => {
      if (printMode === "booklet") {
        document.body.classList.remove("is-printing");
      }
    };
    window.addEventListener("beforeprint", handleBeforePrint);
    window.addEventListener("afterprint", handleAfterPrint);
    return () => {
      window.removeEventListener("beforeprint", handleBeforePrint);
      window.removeEventListener("afterprint", handleAfterPrint);
    };
  });
</script>

<svelte:window onkeydown={handleEditorKeydown} />

{#if printMode === "none"}

<div class="h-full flex flex-col p-6 select-none bg-[#14161b] text-zinc-200 overflow-hidden font-sans relative">
  <!-- Top Live Tatamis Strip (Matching Ambrilindo Local Live Banner) -->
  {#if summary.tatamis_live && summary.tatamis_live.some((t) => t.is_running)}
    <div class="mb-3 px-3.5 py-2 bg-[#181a20] border border-white/[0.08] rounded-xl flex items-center justify-between shrink-0 overflow-x-auto gap-3 scrollbar-none">
      <div class="flex items-center space-x-2 shrink-0">
        <span class="w-2 h-2 rounded-full bg-emerald-400" aria-hidden="true"></span>
        <span class="text-xs font-semibold text-zinc-400">Live</span>
      </div>

      <div class="flex items-center space-x-4 overflow-x-auto text-xs scrollbar-none">
        {#each summary.tatamis_live as tLive}
          <div class="flex items-center space-x-2 shrink-0 px-2.5 py-1 rounded-lg border {tLive.is_running ? 'bg-emerald-500/[0.08] border-emerald-500/30 text-emerald-300' : 'bg-white/[0.02] border-white/[0.06] text-zinc-400'}">
            <span class="font-bold">Tatami {tLive.tatami_number}:</span>
            {#if tLive.is_running}
              <span class="font-semibold text-white">{tLive.category_name}</span>
              <span class="text-[11px] text-zinc-400">({tLive.atlet1_name || 'Aka'} vs {tLive.atlet2_name || 'Ao'})</span>
            {:else}
              <span class="text-[11px] text-zinc-500">Siap</span>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {/if}

  <!-- Header Bar -->
  <div class="flex items-center justify-between gap-3 pb-4 border-b border-white/[0.08] shrink-0">
    <div class="flex items-center gap-2.5 min-w-0">
      <h2 class="text-xl font-black text-white tracking-tight uppercase whitespace-nowrap">
        Jadwal Pertandingan
      </h2>
      <span class="px-2 py-0.5 rounded text-[11px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1] whitespace-nowrap">
        {days.length} hari · {totalSlotsCount} sesi
      </span>
      {#if isDirty}
        <span class="inline-flex items-center gap-1.5 px-2 py-0.5 rounded text-xs font-semibold bg-amber-500/15 text-amber-300 border border-amber-500/35 whitespace-nowrap" role="status">
          <span class="w-1.5 h-1.5 rounded-full bg-amber-400" aria-hidden="true"></span>
          Belum disimpan
        </span>
      {/if}
    </div>

    <div class="flex items-center gap-2 shrink-0">
      {#if openMenu}
        <!-- Click outside closes the open menu -->
        <div class="fixed inset-0 z-30" onclick={() => (openMenu = null)} aria-hidden="true"></div>
      {/if}

      <!-- Cetak -->
      <div class="relative">
        <button
          onclick={() => (openMenu = openMenu === "cetak" ? null : "cetak")}
          class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition flex items-center gap-1.5 {openMenu === 'cetak' ? 'relative z-40' : ''}"
          aria-haspopup="menu"
          aria-expanded={openMenu === "cetak"}
        >
          <svg class="w-3.5 h-3.5 text-zinc-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z"/></svg>
          <span>Cetak</span>
          <svg class="w-3 h-3 text-zinc-500" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" /></svg>
        </button>
        {#if openMenu === "cetak"}
          <div class="absolute right-0 mt-2 w-60 bg-[#181a20] border border-white/10 rounded-xl shadow-2xl py-1.5 z-40" role="menu">
            <button role="menuitem" class="w-full px-3.5 py-2 text-left text-xs font-semibold text-zinc-300 hover:text-white hover:bg-white/[0.06] transition flex items-center gap-2 disabled:opacity-40 disabled:pointer-events-none" onclick={() => { openMenu = null; printMode = "simple_roster"; }}>
              Roster 1 halaman
            </button>
            {#each tatamis as t}
              <button
                role="menuitem"
                class="w-full px-3.5 py-2 text-left text-xs font-semibold text-zinc-300 hover:text-white hover:bg-white/[0.06] transition flex items-center gap-2 disabled:opacity-40 disabled:pointer-events-none"
                disabled={!activeDay || tatamiBookletLoading}
                onclick={() => { openMenu = null; openTatamiBooklet(t); }}
              >
                Buku Tatami {t.tatami_number}
              </button>
            {/each}
            <div class="my-1 border-t border-white/[0.08]"></div>
            <button role="menuitem" class="w-full px-3.5 py-2 text-left text-xs font-semibold text-zinc-300 hover:text-white hover:bg-white/[0.06] transition flex items-center gap-2 disabled:opacity-40 disabled:pointer-events-none" onclick={() => { openMenu = null; kopModalOpen = true; }}>
              Kop Surat…
            </button>
          </div>
        {/if}
      </div>

      <!-- Atur Jadwal -->
      <div class="relative">
        <button
          onclick={() => (openMenu = openMenu === "atur" ? null : "atur")}
          class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition flex items-center gap-1.5 {openMenu === 'atur' ? 'relative z-40' : ''}"
          aria-haspopup="menu"
          aria-expanded={openMenu === "atur"}
        >
          <svg class="w-3.5 h-3.5 text-amber-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M13 10V3L4 14h7v7l9-11h-7z"/></svg>
          <span>Atur Jadwal</span>
          <svg class="w-3 h-3 text-zinc-500" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" /></svg>
        </button>
        {#if openMenu === "atur"}
          <div class="absolute right-0 mt-2 w-60 bg-[#181a20] border border-white/10 rounded-xl shadow-2xl py-1.5 z-40" role="menu">
            <button role="menuitem" class="w-full px-3.5 py-2 text-left text-xs font-semibold text-zinc-300 hover:text-white hover:bg-white/[0.06] transition flex items-center gap-2 disabled:opacity-40 disabled:pointer-events-none" onclick={() => { openMenu = null; openAutoScheduleModal(); }}>
              <svg class="w-3.5 h-3.5 text-amber-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M13 10V3L4 14h7v7l9-11h-7z"/></svg>
              <span>Auto-Jadwal…</span>
            </button>
            <div class="my-1 border-t border-white/[0.08]"></div>
            <button role="menuitem" class="w-full px-3.5 py-2 text-left text-xs font-semibold text-zinc-300 hover:text-white hover:bg-white/[0.06] transition flex items-center gap-2 disabled:opacity-40 disabled:pointer-events-none" disabled={!activeDay} onclick={() => { openMenu = null; promptClearDaySlots(); }}>
              Kosongkan {activeDay?.label ?? "hari ini"}
            </button>
            <button
              role="menuitem"
              class="w-full px-3.5 py-2 text-left text-xs font-semibold text-rose-400 hover:text-rose-200 hover:bg-rose-500/10 transition flex items-center gap-2"
              onclick={() => { openMenu = null; promptResetAllTimetable(); }}
            >
              Reset seluruh jadwal
            </button>
          </div>
        {/if}
      </div>

      {#if isDirty}
        <button onclick={promptDiscardChanges} disabled={isSaving} class="h-9 px-3.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition flex items-center gap-1.5 disabled:opacity-50">
          Batalkan
        </button>
      {/if}

      <button
        onclick={handleSaveTimetable}
        disabled={isSaving || !isDirty}
        title="Simpan (Ctrl+S)"
        class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition active:scale-95 flex items-center gap-1.5 disabled:opacity-40 disabled:active:scale-100 disabled:hover:bg-white"
      >
        {#if isSaving}
          <span class="w-3.5 h-3.5 border-2 border-black border-t-transparent rounded-full animate-spin"></span>
          <span>Menyimpan...</span>
        {:else}
          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M5 13l4 4L19 7" />
          </svg>
          <span>Simpan</span>
        {/if}
      </button>
    </div>
  </div>

  <!-- Auto-Schedule Result Banner with Undo -->
  {#if autoResultBanner}
    <div class="my-3 px-4 py-2.5 bg-emerald-500/10 border border-emerald-500/30 rounded-xl flex items-center justify-between text-xs shrink-0 animate-in fade-in">
      <div class="flex items-center space-x-2.5">
        <svg class="w-4 h-4 text-emerald-400 shrink-0" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7"/></svg>
        <div>
          <strong class="text-emerald-300 font-bold">{autoResultBanner.title}</strong>
          <p class="text-[11px] text-zinc-400 font-mono mt-0.5">{autoResultBanner.summary}</p>
        </div>
      </div>
      <div class="flex items-center space-x-2">
        <button
          onclick={undoLastAutoSchedule}
          class="px-2.5 py-1 bg-white/[0.06] hover:bg-white/[0.12] text-zinc-200 rounded-lg font-semibold text-[11px] border border-white/10"
        >
          Urungkan (Undo)
        </button>
        <button
          onclick={() => (autoResultBanner = null)}
          class="p-1 text-zinc-500 hover:text-white"
          title="Tutup banner"
        >
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
        </button>
      </div>
    </div>
  {/if}

  <!-- Compact stats strip (one line, leaves the height to the grid) -->
  <div class="flex flex-wrap items-center gap-x-5 gap-y-1.5 my-3 px-3.5 py-2 bg-[#181a20] border border-white/[0.08] rounded-xl shrink-0 text-xs min-h-9">
    {#if isLoading}
      <span class="text-zinc-500 font-mono text-[11px]">Memuat ringkasan…</span>
    {:else}
    <span class="text-zinc-400">Sesi <strong class="font-mono tabular-nums text-white">{totalSlotsCount}</strong></span>
    <span class="text-zinc-400">Tatami <strong class="font-mono tabular-nums text-white">{tatamis.length}</strong></span>
    <span class="text-zinc-400">
      Pertandingan <strong class="font-mono tabular-nums text-white">{summary.finished_matches}</strong><span class="font-mono tabular-nums text-zinc-500">/{summary.total_matches}</span>
      <span class="ml-1 font-mono tabular-nums text-emerald-400">{summary.overall_pct}%</span>
    </span>
    {#if unscheduledTournamentCount > 0}
      <button
        type="button"
        onclick={showUnscheduled}
        class="inline-flex items-center gap-1.5 h-7 px-2 -mx-1 rounded-lg text-amber-300 hover:bg-amber-500/10 transition cursor-pointer"
        title="Tampilkan kategori yang belum terjadwal"
      >
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v4m0 4h.01M10.3 3.9L1.8 18a2 2 0 001.7 3h17a2 2 0 001.7-3L13.7 3.9a2 2 0 00-3.4 0z" /></svg>
        Belum terjadwal <strong class="font-mono tabular-nums">{unscheduledTournamentCount}</strong>
      </button>
    {:else}
      <span class="inline-flex items-center gap-1.5 text-emerald-400">
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" /></svg>
        Semua terjadwal
      </span>
    {/if}
    {#if summary.total_conflicts > 0}
      <button
        type="button"
        onclick={showFirstConflict}
        class="inline-flex items-center gap-1.5 h-7 px-2 -mx-1 rounded-lg text-rose-300 hover:bg-rose-500/10 transition cursor-pointer"
        title="Lihat sesi yang bentrok"
      >
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M12 9v4m0 4h.01M10.3 3.9L1.8 18a2 2 0 001.7 3h17a2 2 0 001.7-3L13.7 3.9a2 2 0 00-3.4 0z" /></svg>
        Bentrok atlet <strong class="font-mono tabular-nums">{summary.total_conflicts}</strong>
      </button>
    {:else}
      <span class="text-zinc-400">Bentrok atlet <strong class="font-mono tabular-nums text-white">0</strong></span>
    {/if}
    <span class="ml-auto text-[11px] text-zinc-500">
      <span class="font-mono">Esc</span> batal · <span class="font-mono">Ctrl+S</span> simpan
    </span>
    {/if}
  </div>

  <!-- Main Body: Category Pool Sidebar + Timetable Grid -->
  <div class="flex-1 flex min-h-0 gap-3 overflow-hidden">
    <!-- 1. Left Category Pool Sidebar (Collapsible) -->
    {#if isSidebarOpen}
      <aside class="w-72 bg-[#181a20] border border-white/[0.08] rounded-2xl flex flex-col shrink-0 p-3 overflow-hidden shadow-lg">
        <div class="flex items-center justify-between pb-2 border-b border-white/[0.06] shrink-0">
          <div>
            <h4 class="text-sm font-bold text-white">Kategori <span class="text-zinc-500 font-normal tabular-nums">({categories.length})</span></h4>
            {#if isLoading}
              <span class="text-xs text-zinc-500">Memuat…</span>
            {:else}
              <span class="text-xs {unscheduledTournamentCount === 0 ? 'text-emerald-400' : 'text-amber-300'}">
                {unscheduledTournamentCount === 0 ? 'Semua terjadwal' : `${unscheduledTournamentCount} belum terjadwal`}
              </span>
            {/if}
          </div>
          <button
            onclick={() => (isSidebarOpen = false)}
            class="text-zinc-400 hover:text-white p-1 rounded-lg hover:bg-white/[0.06] transition flex items-center space-x-1"
            title="Sembunyikan daftar kategori"
            aria-label="Sembunyikan daftar kategori"
          >
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M15 19l-7-7 7-7" />
            </svg>
          </button>
        </div>

        <!-- Filter & Search in Pool -->
        <div class="py-2 space-y-1.5 shrink-0 border-b border-white/[0.06]">
          <input
            type="text"
            bind:value={poolSearch}
            placeholder="Cari kategori"
            aria-label="Cari kategori"
            class="w-full h-8 bg-[#14161b] text-sm text-white px-2.5 rounded-lg border border-white/[0.08] focus:border-white/40 outline-none select-text"
          />

          <select
            bind:value={poolGroupFilter}
            aria-label="Filter kategori"
            class="w-full h-8 bg-[#14161b] text-sm text-zinc-300 px-2 rounded-lg border border-white/[0.08] focus:border-white/40 outline-none"
          >
            <option value="ALL">Semua Kategori ({categories.length})</option>
            <option value="UNASSIGNED">Belum Terjadwal ({unscheduledTournamentCount})</option>
            <option value="ASSIGNED">Sudah Terjadwal ({categories.length - unscheduledTournamentCount})</option>
            <option value="VALID_TOURNAMENT">Turnamen Valid (&gt;1 Atlet)</option>
            <option value="SOLO">Tunggal (Solo)</option>
            <option value="TOURNAMENT">Turnamen Saja</option>
            <option value="FESTIVAL">Festival Saja</option>
            {#each ORDERED_GROUPS as grp}
              <option value={grp}>{grp}</option>
            {/each}
          </select>
        </div>

        <!-- Chips Pool List (Ultra-Compact) -->
        <div class="flex-1 overflow-y-auto space-y-1.5 pt-2 pr-1 scrollbar-thin">
          {#each poolSortedCategories as cat (cat.id)}
            {@const isAssigned = assignedCategoryIds.has(cat.id)}
            {@const missing = missingPartsByCat.get(cat.id)}
            {@const isPicked = placeSource?.id === cat.id}
            <div
              class="rounded-xl border transition {isPicked
                ? 'bg-sky-500/10 border-sky-400 ring-2 ring-sky-400/30'
                : isAssigned && !missing
                  ? 'bg-[#14161b]/40 border-white/[0.04]'
                  : 'bg-[#14161b] border-white/[0.08] hover:border-white/25'}"
            >
              <button
                type="button"
                onclick={() => startPlace(cat)}
                disabled={isAssigned && !missing}
                aria-pressed={isPicked}
                class="w-full text-left px-2.5 pt-2 pb-1.5 rounded-xl cursor-pointer disabled:cursor-default focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
                title={isAssigned && !missing ? "Sudah terjadwal" : "Pilih, lalu klik sel kosong di jadwal"}
              >
                <span class="block text-[13px] font-semibold leading-snug line-clamp-2 {isAssigned && !missing ? 'text-zinc-500' : 'text-white'}" title={cat.nama}>{cat.nama}</span>
                <span class="mt-0.5 flex items-center gap-1.5 text-[11px] text-zinc-500">
                  <span class={cat.discipline === "kata" ? "text-purple-300/90" : "text-sky-300/90"}>{cat.discipline === "kata" ? "Kata" : "Kumite"}</span>
                  <span aria-hidden="true">·</span>
                  <span class="truncate">{cat.age_group}</span>
                  <span aria-hidden="true">·</span>
                  <span class="shrink-0 tabular-nums">{cat.atlet_count} atlet</span>
                </span>
              </button>

              <div class="px-2.5 pb-2 flex items-center gap-1 flex-wrap">
                {#if isAssigned && missing}
                  <span class="text-[11px] font-semibold text-amber-300">{missing.map((p) => SPLIT_PART_LABEL[p]).join(", ")} belum terjadwal</span>
                {:else if isAssigned}
                  <span class="inline-flex items-center gap-1 text-[11px] text-emerald-400/80">
                    <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="3" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" /></svg>
                    Terjadwal
                  </span>
                {:else}
                  <span class="text-[11px] text-zinc-500 mr-0.5">Taruh cepat</span>
                  {#each tatamis as t (t.id)}
                    <button
                      type="button"
                      onclick={() => quickAssignFromPool(cat, t.id)}
                      class="h-7 min-w-9 px-2 rounded-lg bg-white/[0.06] hover:bg-white hover:text-black text-zinc-200 text-[11px] font-bold font-mono transition cursor-pointer focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
                      title={`Taruh di sesi kosong pertama Tatami ${t.tatami_number}`}
                      aria-label={`Taruh ${cat.nama} di Tatami ${t.tatami_number}`}
                    >
                      T{t.tatami_number}
                    </button>
                  {/each}
                {/if}
              </div>
            </div>
          {/each}
          {#if poolFilteredCategories.length === 0}
            <div class="py-10 text-center text-zinc-500 text-sm">
              Tidak ada kategori
            </div>
          {/if}
        </div>
      </aside>
    {:else}
      <!-- Compact Vertical Toggle Button adjacent to Category Pool / Grid -->
      <button
        onclick={() => (isSidebarOpen = true)}
        class="h-auto my-auto py-3 px-1.5 bg-[#181a20] hover:bg-white/[0.08] text-zinc-400 hover:text-white border border-white/[0.08] rounded-r-xl flex flex-col items-center justify-center space-y-2 shrink-0 shadow-lg group transition cursor-pointer"
        title="Tampilkan daftar kategori"
        aria-label="Tampilkan daftar kategori"
      >
        <svg class="w-3.5 h-3.5 text-zinc-400 group-hover:text-white transition" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
        </svg>
        <span class="text-[11px] font-semibold [writing-mode:vertical-rl] rotate-180">Kategori</span>
        {#if unscheduledTournamentCount > 0}
          <span class="px-1 py-0.5 rounded text-[11px] font-mono font-bold bg-amber-500/20 text-amber-300">
            {unscheduledTournamentCount}
          </span>
        {/if}
      </button>
    {/if}

    <!-- 2. Right Timetable Workspace Grid -->
    <div class="flex-1 flex flex-col min-w-0 overflow-hidden">
      <!-- Day tabs | start time, date, + Sesi, + Tatami -->
      <div class="flex flex-wrap items-center justify-between gap-2 pb-3 shrink-0">
        <div class="flex items-center gap-1.5 overflow-x-auto scrollbar-none">
          {#each days as day, idx}
            <div class="flex items-center rounded-xl border transition {activeDayIndex === idx ? 'bg-white text-black border-white shadow-sm' : 'bg-white/[0.04] text-zinc-300 border-white/[0.08] hover:bg-white/[0.08]'}">
              <button onclick={() => (activeDayIndex = idx)} class="h-8 pl-3.5 {activeDayIndex === idx && days.length > 1 ? 'pr-1.5' : 'pr-3.5'} text-xs font-semibold cursor-pointer">
                {day.label}
              </button>
              {#if activeDayIndex === idx && days.length > 1}
                <button
                  onclick={promptDeleteCurrentDay}
                  disabled={isDeletingDay}
                  class="h-8 pr-2.5 pl-1 text-zinc-500 hover:text-rose-600 transition cursor-pointer"
                  title="Hapus {day.label}"
                  aria-label="Hapus {day.label}"
                >
                  <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M6 18L18 6M6 6l12 12" /></svg>
                </button>
              {/if}
            </div>
          {/each}
          <button
            onclick={handleAddDay}
            class="h-8 px-2.5 bg-white/[0.03] hover:bg-white/[0.06] text-zinc-400 hover:text-white text-xs font-semibold rounded-xl border border-dashed border-white/[0.15] transition cursor-pointer whitespace-nowrap"
          >
            + Hari
          </button>
        </div>

        <div class="flex items-center gap-2">
          <label class="h-8 flex items-center gap-1.5 bg-[#181a20] px-2.5 rounded-xl border border-white/[0.08]">
            <span class="text-[10px] font-mono text-zinc-400 uppercase">Mulai</span>
            <input type="time" bind:value={startTimeStr} class="bg-transparent text-xs text-white outline-none font-mono cursor-pointer" />
          </label>
          {#if activeDay}
            <label class="h-8 flex items-center gap-1.5 bg-[#181a20] px-2.5 rounded-xl border border-white/[0.08]">
              <span class="text-[10px] font-mono text-zinc-400 uppercase">Tanggal</span>
              <input type="date" bind:value={activeDay.tanggal} onchange={markDirty} class="bg-transparent text-xs text-white outline-none font-mono cursor-pointer" />
            </label>
          {/if}

          <div class="relative">
            <button
              onclick={() => (openMenu = openMenu === "sesi" ? null : "sesi")}
              class="h-8 px-2.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white text-xs font-semibold rounded-xl border border-white/[0.08] transition flex items-center gap-1.5 cursor-pointer {openMenu === 'sesi' ? 'relative z-40' : ''}"
              aria-haspopup="menu"
              aria-expanded={openMenu === "sesi"}
            >
              <span>+ Sesi</span>
              <svg class="w-3 h-3 text-zinc-500" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7" /></svg>
            </button>
            {#if openMenu === "sesi"}
              <div class="absolute right-0 mt-2 w-48 bg-[#181a20] border border-white/10 rounded-xl shadow-2xl py-1.5 z-40" role="menu">
                <button role="menuitem" class="w-full px-3.5 py-2 text-left text-xs font-semibold text-zinc-300 hover:text-white hover:bg-white/[0.06] transition flex items-center gap-2 disabled:opacity-40 disabled:pointer-events-none" disabled={!activeDay} onclick={() => { openMenu = null; addSlotRow(); }}>Sesi kosong</button>
                <button role="menuitem" class="w-full px-3.5 py-2 text-left text-xs font-semibold text-zinc-300 hover:text-white hover:bg-white/[0.06] transition flex items-center gap-2 disabled:opacity-40 disabled:pointer-events-none" disabled={!activeDay} onclick={() => { openMenu = null; openFestivalSessionPrompt(); }}>{festivalSession ? "Sesi Festival" : "Sesi Festival…"}</button>
                <button role="menuitem" class="w-full px-3.5 py-2 text-left text-xs font-semibold text-zinc-300 hover:text-white hover:bg-white/[0.06] transition flex items-center gap-2 disabled:opacity-40 disabled:pointer-events-none" disabled={!activeDay} onclick={() => { openMenu = null; ishomaPromptOpen = true; }}>ISHOMA…</button>
              </div>
            {/if}
          </div>

          <button onclick={handleAddTatami} class="h-8 px-2.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white text-xs font-semibold rounded-xl border border-white/[0.08] transition flex items-center gap-1.5 cursor-pointer">+ Tatami</button>
        </div>
      </div>

      {#if placeSource}
        <div class="mb-2 px-3.5 py-2 rounded-xl border border-sky-500/30 bg-sky-500/10 flex items-center justify-between gap-3 shrink-0" role="status">
          <span class="text-sm text-sky-200 truncate">
            Menempatkan <strong class="text-white">{placeSource.nama}</strong>. Klik sel kosong di jadwal.
          </span>
          <button
            type="button"
            onclick={cancelPlace}
            class="shrink-0 px-2.5 py-1 rounded-lg text-xs font-semibold text-sky-200 border border-sky-500/30 hover:bg-sky-500/20 transition"
          >
            Batal <span class="font-mono text-[11px] opacity-70">Esc</span>
          </button>
        </div>
      {/if}

      {#if moveSource}
        <div class="mb-2 px-3.5 py-2 rounded-xl border border-sky-500/30 bg-sky-500/10 flex items-center justify-between gap-3 shrink-0" role="status">
          <span class="text-xs text-sky-200 truncate">
            Memindahkan <strong class="text-white">{moveSource.name}</strong>. Klik sel tujuan; sel yang sudah terisi akan ditukar.
          </span>
          <button
            type="button"
            onclick={cancelMove}
            class="shrink-0 px-2.5 py-1 rounded-lg text-xs font-semibold text-sky-200 border border-sky-500/30 hover:bg-sky-500/20 transition"
          >
            Batal <span class="font-mono text-[10px] opacity-70">Esc</span>
          </button>
        </div>
      {/if}

      <!-- Timetable Grid Table Container -->
      <div class="flex-1 overflow-y-auto min-h-0 rounded-2xl border border-white/[0.08] bg-[#181a20] shadow-lg flex flex-col scrollbar-thin">
        {#if isLoading}
          <div class="flex-1 flex items-center justify-center py-20 text-zinc-500">
            <div class="w-5 h-5 border-2 border-white/20 border-t-white rounded-full animate-spin mr-3"></div>
            <span class="text-xs font-mono">Memuat jadwal pertandingan...</span>
          </div>
        {:else if !activeDay || activeDay.rows.length === 0}
          <div class="flex-1 flex flex-col items-center justify-center py-20 text-zinc-500">
            <span class="text-sm font-semibold text-zinc-300">Belum ada sesi di hari ini</span>
            <div class="flex items-center space-x-2 mt-3">
              <button
                onclick={openAutoScheduleModal}
                class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition flex items-center gap-1.5"
              >
                <svg class="w-4 h-4 text-amber-600" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="2.5" aria-hidden="true">
                  <path stroke-linecap="round" stroke-linejoin="round" d="M13 10V3L4 14h7v7l9-11h-7z" />
                </svg>
                <span>Auto-jadwal</span>
              </button>
              <button
                onclick={addSlotRow}
                class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] hover:bg-white/[0.08] font-semibold text-xs rounded-xl transition"
              >
                Tambah sesi manual
              </button>
            </div>
          </div>
        {:else}
          <table class="w-full text-left border-collapse">
            <thead class="bg-[#111215] text-zinc-400 font-mono text-[11px] uppercase tracking-wider sticky top-0 z-10 border-b border-white/[0.08]">
              <tr>
                <th class="py-3 px-4 w-40 min-w-36 font-semibold">Sesi</th>
                {#each tatamis as tatami}
                  {@const tMeta = tatamiTimelines[tatami.id]}
                  <th class="py-2.5 px-3 font-semibold border-l border-white/[0.06]">
                    <div class="flex items-center justify-between">
                      <div>
                        <span class="text-white font-bold">Tatami {tatami.tatami_number}</span>
                        {#if tMeta}
                          <span class="block text-[11px] font-mono text-zinc-400 font-normal normal-case tracking-normal">
                            <span title="Perkiraan selesai">{tMeta.totalMatches} partai · ~{tMeta.finishTime}</span>
                          </span>
                        {/if}
                      </div>

                      <!-- Per-tatami menu (print booklet, remove) -->
                      <div class="relative">
                        <button
                          type="button"
                          onclick={() => (openMenu = openMenu === `tatami-${tatami.id}` ? null : `tatami-${tatami.id}`)}
                          aria-haspopup="menu"
                          aria-expanded={openMenu === `tatami-${tatami.id}`}
                          aria-label={`Menu Tatami ${tatami.tatami_number}`}
                          class="w-7 h-7 rounded-lg text-zinc-400 hover:text-white hover:bg-white/[0.08] flex items-center justify-center transition cursor-pointer {openMenu === `tatami-${tatami.id}` ? 'relative z-40 text-white bg-white/[0.08]' : ''}"
                        >
                          {#if tatamiBookletLoading && printSelectedTatami?.id === tatami.id}
                            <span class="w-3.5 h-3.5 border-2 border-zinc-400 border-t-transparent rounded-full animate-spin"></span>
                          {:else}
                            <svg class="w-4 h-4" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><circle cx="12" cy="5" r="1.8" /><circle cx="12" cy="12" r="1.8" /><circle cx="12" cy="19" r="1.8" /></svg>
                          {/if}
                        </button>
                        {#if openMenu === `tatami-${tatami.id}`}
                          <div class="absolute right-0 mt-1 w-52 bg-[#181a20] border border-white/10 rounded-xl shadow-2xl py-1.5 z-40 normal-case tracking-normal font-sans" role="menu">
                            <button
                              role="menuitem"
                              class="w-full px-3.5 py-2 text-left text-sm font-semibold text-zinc-200 hover:text-white hover:bg-white/[0.06] transition disabled:opacity-40"
                              disabled={tatamiBookletLoading}
                              onclick={() => { openMenu = null; openTatamiBooklet(tatami); }}
                            >
                              Cetak buku tatami
                            </button>
                            {#if tatamis.length > 1}
                              <button
                                role="menuitem"
                                class="w-full px-3.5 py-2 text-left text-sm font-semibold text-rose-300 hover:bg-rose-500/10 transition"
                                onclick={() => { openMenu = null; promptRemoveTatami(tatami.id); }}
                              >
                                Hapus tatami…
                              </button>
                            {/if}
                          </div>
                        {/if}
                      </div>
                    </div>
                  </th>
                {/each}
                <th class="py-3 px-2 w-20 text-center font-semibold border-l border-white/[0.06]"><span class="sr-only">Aksi sesi</span></th>
              </tr>
            </thead>
            <tbody class="divide-y divide-white/[0.04] text-xs">
              {#each activeDay.rows as row, rIdx}
                {#if row.row_type === "label"}
                  <!-- ISHOMA / Break Banner Row -->
                  <tr class="bg-amber-500/[0.04] hover:bg-amber-500/[0.07] transition group">
                    <td class="py-3 px-4 align-middle">
                      <input
                        type="text"
                        bind:value={row.time_label}
                        onchange={markDirty}
                        placeholder="Waktu break"
                        class="w-full bg-[#14161b] text-amber-300 font-mono text-xs px-2.5 py-1.5 rounded-lg border border-amber-500/20 outline-none"
                      />
                    </td>
                    <td colspan={tatamis.length} class="py-3 px-4 align-middle border-l border-white/[0.06]">
                      <div class="flex items-center space-x-2">
                        <span class="px-1.5 py-0.5 rounded text-[11px] font-semibold bg-amber-500/20 text-amber-300">Jeda</span>
                        <input
                          type="text"
                          bind:value={row.label_text}
                          onchange={markDirty}
                          placeholder="Keterangan Break / Upacara"
                          class="flex-1 bg-[#14161b] text-amber-200 font-bold text-xs px-3 py-1.5 rounded-lg border border-amber-500/20 outline-none uppercase"
                        />
                      </div>
                    </td>
                    <td class="py-3 px-3 text-center align-middle border-l border-white/[0.06]">
                      <div class="grid grid-cols-2 gap-0.5 place-items-center opacity-70 group-hover:opacity-100 focus-within:opacity-100 transition">
                        <button onclick={() => moveRow(rIdx, "up")} disabled={rIdx === 0} class="w-7 h-7 rounded-md flex items-center justify-center text-zinc-400 hover:text-white hover:bg-white/[0.06] disabled:opacity-20" title="Geser ke atas" aria-label="Geser sesi ke atas">
                          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 15l7-7 7 7"/></svg>
                        </button>
                        <button onclick={() => moveRow(rIdx, "down")} disabled={rIdx === activeDay.rows.length - 1} class="w-7 h-7 rounded-md flex items-center justify-center text-zinc-400 hover:text-white hover:bg-white/[0.06] disabled:opacity-20" title="Geser ke bawah" aria-label="Geser sesi ke bawah">
                          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/></svg>
                        </button>
                        <button onclick={() => deleteRow(rIdx)} class="w-7 h-7 rounded-md flex items-center justify-center text-zinc-500 hover:text-rose-300 hover:bg-rose-500/10" title="Hapus sesi" aria-label="Hapus sesi">
                          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
                        </button>
                      </div>
                    </td>
                  </tr>
                {:else}
                  <!-- Slot Row -->
                  <tr class="hover:bg-white/[0.02] transition group {row.conflicts.length > 0 ? 'bg-rose-500/[0.03]' : ''}">
                    <!-- Time Column -->
                    <td class="py-3 px-4 align-top w-40 min-w-36">
                      <div class="space-y-1">
                        <span class="text-sm font-bold text-zinc-200 block px-0.5">
                          Sesi <span class="font-mono tabular-nums">{rIdx + 1}</span>
                        </span>
                        <input
                          type="text"
                          bind:value={row.time_label}
                          onchange={markDirty}
                          placeholder="Catatan (opsional)"
                          title="Label bebas untuk sesi ini. Jam mulai tiap kategori dihitung otomatis."
                          class="w-full bg-[#14161b] text-zinc-200 text-xs px-2 py-1 rounded-lg border border-white/[0.08] focus:border-white/40 outline-none placeholder:text-zinc-600 select-text"
                        />
                        {#if row.conflicts.length > 0}
                          <button
                            onclick={() => {
                              activeConflictRow = row;
                              conflictModalOpen = true;
                            }}
                            class="w-full mt-1 px-2 py-1 bg-rose-500/10 border border-rose-500/40 rounded-md text-xs font-semibold text-rose-300 text-left flex items-center space-x-1 hover:bg-rose-500/20 transition cursor-pointer"
                          >
                            <svg class="w-3 h-3 text-rose-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"/>
                            </svg>
                            <span class="truncate">{row.conflicts.length} bentrok</span>
                          </button>
                        {/if}
                      </div>
                    </td>

                    <!-- Tatami Cells -->
                    {#each tatamis as tatami}
                      {@const cell = row.cells.find((c) => c.tatami_id === tatami.id)}
                      {@const time = cellTimes[`${rIdx}:${tatami.id}`]}
                      {@const isMoveSource = moveSource?.rowIdx === rIdx && moveSource?.tatamiId === tatami.id}
                      <td class="py-2.5 px-3 align-top border-l border-white/[0.06] relative">
                        {#if placeSource && !cellHasContent(cell)}
                          <button
                            type="button"
                            onclick={() => dropPlaceOn(rIdx, tatami.id)}
                            class="group/target absolute inset-1.5 z-[5] rounded-xl border-2 border-dashed border-sky-400/40 bg-[#14161b]/60 hover:bg-sky-500/15 hover:border-sky-400 transition flex items-center justify-center"
                            aria-label="Tempatkan {placeSource.nama} di Tatami {tatami.tatami_number} sesi {rIdx + 1}"
                          >
                            <span class="px-2 py-0.5 rounded-full bg-sky-500/20 group-hover/target:bg-sky-400 text-sky-200 group-hover/target:text-black text-[11px] font-bold transition">
                              Taruh di sini
                            </span>
                          </button>
                        {/if}
                        {#if moveSource && !isMoveSource}
                          <button
                            type="button"
                            onclick={() => dropMoveOn(rIdx, tatami.id)}
                            class="group/target absolute inset-1.5 z-[5] rounded-xl border-2 border-dashed border-sky-400/40 bg-[#14161b]/60 hover:bg-sky-500/15 hover:border-sky-400 transition flex items-center justify-center"
                            aria-label="{cellHasContent(cell) ? 'Tukar dengan' : 'Pindahkan ke'} Tatami {tatami.tatami_number} sesi {rIdx + 1}"
                          >
                            <span class="px-2 py-0.5 rounded-full bg-sky-500/20 group-hover/target:bg-sky-400 text-sky-200 group-hover/target:text-black text-[11px] font-bold transition">
                              {cellHasContent(cell) ? "Tukar" : "Pindah ke sini"}
                            </span>
                          </button>
                        {/if}
                        {#if cell && cell.nomor_tanding_id}
                          <!-- Category Card -->
                          <div class="bg-[#14161b] border rounded-xl p-2.5 relative group/cell transition shadow-sm {isMoveSource ? 'border-sky-400 ring-2 ring-sky-400/30' : 'border-white/[0.1] hover:border-white/20'}">
                            <div class="flex items-center justify-between gap-1.5 mb-1">
                              <div class="flex items-center gap-1.5 min-w-0">
                                <span class="px-1.5 py-0.5 rounded text-[11px] font-semibold shrink-0 {cell.discipline === 'kata' ? 'bg-purple-500/15 text-purple-300' : 'bg-sky-500/15 text-sky-300'}">
                                  {cell.discipline === "kata" ? "Kata" : "Kumite"}
                                </span>
                              </div>

                            <div class="flex items-center gap-0.5 opacity-40 group-hover/cell:opacity-100 focus-within:opacity-100 transition">
                              {#if cell.part}
                                <button
                                  onclick={() => mergeSplit(rIdx, tatami.id)}
                                  class="w-6 h-6 flex items-center justify-center text-zinc-400 hover:text-amber-300 rounded-md hover:bg-amber-500/10 transition"
                                  title="Gabung kembali jadi satu tatami"
                                  aria-label="Gabung kembali pool atas, pool bawah dan final ke tatami ini"
                                >
                                  <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M8 7h8M8 17h8M12 7v10" />
                                  </svg>
                                </button>
                              {:else if !cell.is_festival && tatamis.length > 1 && cell.atlet_count >= MIN_SPLIT_ENTRIES}
                                <button
                                  onclick={() => openSplit(rIdx, tatami.id)}
                                  class="w-6 h-6 flex items-center justify-center text-zinc-400 hover:text-amber-300 rounded-md hover:bg-amber-500/10 transition"
                                  title="Bagi ke 2 tatami (pool atas / pool bawah)"
                                  aria-label="Bagi kategori ini ke 2 tatami"
                                >
                                  <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 4v16M4 8h5M4 16h5M15 8h5M15 16h5" />
                                  </svg>
                                </button>
                              {/if}
                              <button
                                onclick={() => startMove(rIdx, tatami.id)}
                                class="w-6 h-6 flex items-center justify-center text-zinc-400 hover:text-sky-300 rounded-md hover:bg-sky-500/10 transition"
                                title="Pindah / tukar ke sel lain"
                                aria-label="Pindah atau tukar kategori ini"
                              >
                                <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16V4m0 0L3 8m4-4l4 4m6 0v12m0 0l4-4m-4 4l-4-4" />
                                </svg>
                              </button>
                              <button
                                onclick={() => clearCell(rIdx, tatami.id)}
                                class="w-6 h-6 flex items-center justify-center text-zinc-400 hover:text-rose-300 rounded-md hover:bg-rose-500/10 transition"
                                title="Keluarkan dari jadwal"
                                aria-label="Hapus kategori dari tatami"
                              >
                                <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                                </svg>
                              </button>
                            </div>
                            </div>

                            <div class="font-bold text-white text-xs leading-snug line-clamp-2">
                              {cell.nomor_tanding_nama || `Kategori #${cell.nomor_tanding_id}`}
                            </div>
                            {#if cell.part}
                              <span class="inline-block mt-1 px-1.5 py-0.5 rounded text-[11px] font-semibold bg-amber-500/20 text-amber-300">{SPLIT_PART_LABEL[cell.part]}</span>
                            {/if}
                            {#if time}
                              <div class="mt-1 text-[11px] font-mono tabular-nums text-zinc-300" title="Perkiraan {cell.est_duration_minutes} menit">{time.start}–{time.end}</div>
                            {/if}

                            <div class="flex items-center justify-between mt-2 pt-1.5 border-t border-white/[0.04] text-[11px] text-zinc-400">
                              <span class="truncate">{cell.atlet_count} atlet · {cell.matches} partai</span>
                              <span class="shrink-0 tabular-nums text-zinc-500 ml-2">{cell.est_duration_minutes}m</span>
                            </div>

                            {#if cell.progress_status === 'selesai'}
                              <div class="mt-1 flex items-center gap-1 text-[11px] font-semibold text-emerald-400">
                                <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" stroke-width="3" aria-hidden="true"><path stroke-linecap="round" stroke-linejoin="round" d="M5 13l4 4L19 7" /></svg>
                                <span>Selesai</span>
                              </div>
                            {:else if cell.progress_status === 'running'}
                              <div class="mt-1 flex items-center gap-1 text-[11px] font-semibold text-amber-300">
                                <svg class="w-3 h-3" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M8 5v14l11-7z" /></svg>
                                <span>Sedang main</span>
                              </div>
                            {/if}
                          </div>
                        {:else if cell && cell.custom_text}
                          {@const festCard = isFestivalSessionCell(cell)}
                          <!-- Custom Text Card -->
                          <div class="rounded-xl p-2.5 relative group/cell transition {festCard ? 'bg-emerald-500/[0.12] border border-emerald-500/40 hover:border-emerald-400/70' : 'bg-[#14161b] border border-amber-500/20 hover:border-amber-500/40'}">
                            <div class="flex items-center justify-between mb-1">
                              <span class="px-1.5 py-0.5 rounded text-[11px] font-semibold {festCard ? 'bg-emerald-500/25 text-emerald-200' : 'bg-amber-500/20 text-amber-300'}">
                                {cell.is_festival ? 'Festival' : isJedaText(cell.custom_text) ? 'Jeda' : cell.is_break ? 'Istirahat' : 'Catatan'}
                              </span>
                              <div class="flex items-center gap-0.5 opacity-40 group-hover/cell:opacity-100 focus-within:opacity-100 transition">
                                <button
                                  onclick={() => startMove(rIdx, tatami.id)}
                                  class="w-6 h-6 flex items-center justify-center text-zinc-400 hover:text-sky-300 rounded-md hover:bg-sky-500/10 transition"
                                  title="Pindah / tukar ke sel lain"
                                  aria-label="Pindah atau tukar isi sel ini"
                                >
                                  <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M7 16V4m0 0L3 8m4-4l4 4m6 0v12m0 0l4-4m-4 4l-4-4" />
                                  </svg>
                                </button>
                                <button
                                  onclick={() => clearCell(rIdx, tatami.id)}
                                  class="w-6 h-6 flex items-center justify-center text-zinc-400 hover:text-rose-300 rounded-md hover:bg-rose-500/10 transition"
                                  title="Keluarkan dari jadwal"
                                  aria-label="Hapus isi sel"
                                >
                                  <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                    <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                                  </svg>
                                </button>
                              </div>
                            </div>
                            {#if festCard}
                              <div class="font-black text-emerald-100 text-sm tracking-wide">FESTIVAL</div>
                              <div class="mt-1 text-[11px] text-emerald-300/80">
                                {customTextAthletes(cell.custom_text)} atlet · {customTextMatches(cell.custom_text)} partai · {customTextMinutes(cell.custom_text)}m
                              </div>
                            {:else}
                              <div class="font-bold text-amber-200 text-xs line-clamp-2">
                                {cell.custom_text}
                              </div>
                            {/if}
                            {#if time}
                              <div class="mt-1.5 text-[11px] font-mono tabular-nums {festCard ? 'text-emerald-300/70' : 'text-amber-300/70'}">{time.start}–{time.end}</div>
                            {/if}
                          </div>
                        {:else}
                          <!-- Empty Placeholder -->
                          <button
                            type="button"
                            onclick={() => openPicker(rIdx, tatami.id)}
                            class="w-full h-18 rounded-xl border border-dashed border-white/[0.1] hover:border-white/30 hover:bg-white/[0.03] text-zinc-500 hover:text-zinc-200 transition flex items-center justify-center text-xs font-semibold focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-white/40"
                            aria-label="Pilih kategori untuk Tatami {tatami.tatami_number} sesi {rIdx + 1}"
                          >
                            + Pilih kategori
                          </button>
                        {/if}
                      </td>
                    {/each}

                    <!-- Row Actions (Only in Edit Mode) -->
                    <td class="py-3 px-2 text-center align-top border-l border-white/[0.06]">
                      <div class="grid grid-cols-2 gap-0.5 place-items-center opacity-70 group-hover:opacity-100 focus-within:opacity-100 transition">
                        <button onclick={() => moveRow(rIdx, "up")} disabled={rIdx === 0} class="w-7 h-7 rounded-md flex items-center justify-center text-zinc-400 hover:text-white hover:bg-white/[0.06] disabled:opacity-20" title="Geser ke atas" aria-label="Geser sesi ke atas">
                          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 15l7-7 7 7"/></svg>
                        </button>
                        <button onclick={() => moveRow(rIdx, "down")} disabled={rIdx === activeDay.rows.length - 1} class="w-7 h-7 rounded-md flex items-center justify-center text-zinc-400 hover:text-white hover:bg-white/[0.06] disabled:opacity-20" title="Geser ke bawah" aria-label="Geser sesi ke bawah">
                          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/></svg>
                        </button>
                        <button onclick={() => insertSlotRowAfter(rIdx)} class="w-7 h-7 rounded-md flex items-center justify-center text-zinc-400 hover:text-emerald-300 hover:bg-emerald-500/10" title="Sisipkan sesi kosong di bawah" aria-label="Sisipkan sesi kosong di bawah">
                          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 5v14m7-7H5"/></svg>
                        </button>
                        <button onclick={() => deleteRow(rIdx)} class="w-7 h-7 rounded-md flex items-center justify-center text-zinc-500 hover:text-rose-300 hover:bg-rose-500/10" title="Hapus sesi" aria-label="Hapus sesi">
                          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
                        </button>
                      </div>
                    </td>
                  </tr>
                {/if}
              {/each}
            </tbody>
          </table>
          <div class="p-3 border-t border-white/[0.06]">
            <button
              type="button"
              onclick={addSlotRow}
              class="w-full py-2 rounded-xl border border-dashed border-white/[0.12] text-zinc-400 hover:text-white hover:border-white/30 hover:bg-white/[0.03] text-xs font-semibold transition"
            >
              + Tambah sesi di akhir hari
            </button>
          </div>
        {/if}
      </div>
    </div>
  </div>
</div>

<!-- Category Picker (click an empty cell in edit mode) -->
{#if pickerTarget}
  {@const pickTatami = tatamis.find((t) => t.id === pickerTarget?.tatamiId)}
  <div class="fixed inset-0 z-50 flex items-start justify-center pt-[12vh] p-4 bg-black/70 backdrop-blur-xs">
    <button type="button" class="absolute inset-0 cursor-default" aria-label="Tutup pemilih kategori" onclick={closePicker}></button>
    <div
      class="relative bg-[#181a20] border border-white/[0.12] rounded-2xl shadow-2xl w-full max-w-xl flex flex-col max-h-[70vh]"
      role="dialog"
      aria-modal="true"
      aria-labelledby="picker-title"
    >
      <div class="px-4 pt-4 pb-3 border-b border-white/[0.08]">
        <div class="flex items-center justify-between">
          <h3 id="picker-title" class="text-sm font-black uppercase tracking-tight text-white">
            Isi Tatami <span class="font-mono tabular-nums">{pickTatami?.tatami_number}</span> • Sesi <span class="font-mono tabular-nums">#{pickerTarget.rowIdx + 1}</span>
          </h3>
          <span class="text-[10px] font-mono text-zinc-500">{pickerOptions.length} belum terjadwal</span>
        </div>
        <input
          use:focusOnMount
          type="text"
          bind:value={catPickSearch}
          onkeydown={(e) => {
            if (e.key === "Enter" && pickerPartOptions.length > 0 && pickerTarget) {
              e.preventDefault();
              const first = pickerPartOptions[0];
              placeCategory(first.cat, pickerTarget.rowIdx, pickerTarget.tatamiId, first.part);
              pickerTarget = null;
            } else if (e.key === "Enter" && pickerOptions.length > 0) {
              e.preventDefault();
              pickCategory(pickerOptions[0]);
            }
          }}
          placeholder="Cari nama kategori atau kelompok umur… (Enter = pilih teratas)"
          class="mt-3 w-full bg-[#14161b] text-sm text-white px-3 py-2 rounded-lg border border-white/[0.1] focus:border-white/30 outline-none"
        />
        <label class="mt-2 inline-flex items-center gap-2 text-[11px] text-zinc-400 cursor-pointer">
          <input type="checkbox" bind:checked={pickerShowFestival} class="accent-amber-500" />
          Tampilkan kategori festival
        </label>
      </div>
      <div class="flex-1 overflow-y-auto p-2 scrollbar-thin">
        {#each pickerPartOptions as opt (`${opt.cat.id}:${opt.part}`)}
          {@const share = splitPartShare(opt.cat.atlet_count, opt.cat.matches, opt.cat.est_duration_minutes, opt.part)}
          <button
            type="button"
            onclick={() => {
              if (!pickerTarget) return;
              placeCategory(opt.cat, pickerTarget.rowIdx, pickerTarget.tatamiId, opt.part);
              pickerTarget = null;
            }}
            class="w-full text-left px-3 py-2 rounded-lg hover:bg-white/[0.06] focus:bg-white/[0.06] outline-none transition flex items-center gap-3"
          >
            <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold uppercase shrink-0 {opt.cat.discipline === 'kata' ? 'bg-purple-500/20 text-purple-300' : 'bg-sky-500/20 text-sky-300'}">{opt.cat.discipline}</span>
            <span class="flex-1 min-w-0">
              <span class="block text-xs font-bold text-white leading-snug">{opt.cat.nama}</span>
              <span class="inline-block mt-0.5 px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-amber-500/20 text-amber-300">{SPLIT_PART_LABEL[opt.part]}</span>
            </span>
            <span class="shrink-0 text-right text-[10px] font-mono text-zinc-400 tabular-nums">
              {share.entries} atlet • {share.matches} match<br />~{share.minutes} menit
            </span>
          </button>
        {/each}
        {#each pickerOptions as cat (cat.id)}
          <button
            type="button"
            onclick={() => pickCategory(cat)}
            class="w-full text-left px-3 py-2 rounded-lg hover:bg-white/[0.06] focus:bg-white/[0.06] outline-none transition flex items-center gap-3"
          >
            <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold uppercase shrink-0 {cat.discipline === 'kata' ? 'bg-purple-500/20 text-purple-300' : 'bg-sky-500/20 text-sky-300'}">{cat.discipline}</span>
            <span class="flex-1 min-w-0">
              <span class="block text-xs font-bold text-white leading-snug">{cat.nama}</span>
              <span class="block text-[10px] font-mono text-zinc-500">{cat.age_group}</span>
            </span>
            <span class="shrink-0 text-right text-[10px] font-mono text-zinc-400 tabular-nums">
              {cat.atlet_count} atlet • {cat.matches} match<br />~{cat.est_duration_minutes} menit
            </span>
          </button>
        {:else}
          <div class="py-10 text-center text-xs text-zinc-500">
            {pickerPartOptions.length > 0 ? "" : catPickSearch ? "Tidak ada kategori yang cocok." : "Semua kategori sudah terjadwal."}
          </div>
        {/each}
      </div>
    </div>
  </div>
{/if}

<!-- Bagi kategori ke 2 tatami -->
{#if splitTarget && activeDay}
  {@const srcCell = activeDay.rows[splitTarget.rowIdx]?.cells.find((c) => c.tatami_id === splitTarget?.tatamiId)}
  {@const srcCat = categories.find((c) => c.id === srcCell?.nomor_tanding_id)}
  {@const candidates = splitCandidates(splitTarget.rowIdx, splitTarget.tatamiId)}
  {@const thisNum = tatamis.find((t) => t.id === splitTarget?.tatamiId)?.tatami_number}
  {@const otherNum = tatamis.find((t) => t.id === splitOtherTatami)?.tatami_number}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs"
    role="presentation"
    onclick={(e) => { if (e.target === e.currentTarget) splitTarget = null; }}
  >
    <div class="bg-[#181a20] border border-white/10 rounded-2xl p-6 shadow-2xl max-w-md w-full" role="dialog" aria-modal="true" aria-labelledby="split-title">
      <div class="flex items-center justify-between pb-3 border-b border-white/[0.08]">
        <h3 id="split-title" class="text-base font-black text-white uppercase tracking-tight">Bagi ke 2 Tatami</h3>
        <button onclick={() => (splitTarget = null)} class="text-zinc-400 hover:text-white p-1" title="Tutup dialog" aria-label="Tutup dialog">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
        </button>
      </div>
      {#if srcCat}
        {@const top = splitPartShare(srcCat.atlet_count, srcCat.matches, srcCat.est_duration_minutes, "top")}
        {@const bottom = splitPartShare(srcCat.atlet_count, srcCat.matches, srcCat.est_duration_minutes, "bottom")}
        <div class="py-4 space-y-4 text-xs text-zinc-300">
          <div class="font-bold text-white text-sm leading-snug">{srcCat.nama}</div>

          <div class="grid grid-cols-2 gap-2">
            <div class="bg-[#14161b] border border-white/[0.08] rounded-xl p-3">
              <div class="text-[10px] font-bold tracking-wider uppercase text-amber-300">Pool Atas</div>
              <div class="mt-1 font-mono text-white">Tatami {thisNum}</div>
              <div class="mt-1 font-mono text-[10px] text-zinc-500">{top.entries} atlet • {top.matches} match</div>
            </div>
            <div class="bg-[#14161b] border border-white/[0.08] rounded-xl p-3">
              <div class="text-[10px] font-bold tracking-wider uppercase text-amber-300">Pool Bawah</div>
              <select
                bind:value={splitOtherTatami}
                class="mt-1 w-full bg-[#181a20] text-xs text-white px-2 py-1 rounded-lg border border-white/[0.1] focus:border-white outline-none font-mono"
                aria-label="Tatami untuk pool bawah"
              >
                {#each candidates as c (c.t.id)}
                  <option value={c.t.id} disabled={!c.free}>Tatami {c.t.tatami_number}{c.free ? "" : " (terisi)"}</option>
                {/each}
              </select>
              <div class="mt-1 font-mono text-[10px] text-zinc-500">{bottom.entries} atlet • {bottom.matches} match</div>
            </div>
          </div>

          <fieldset class="space-y-1.5">
            <legend class="text-[10px] font-bold tracking-wider uppercase text-zinc-400 mb-1.5">Final</legend>
            <label class="flex items-center gap-2 cursor-pointer">
              <input type="radio" bind:group={splitFinal} value="top" class="accent-white" />
              Tatami {thisNum}, setelah pool atas
            </label>
            <label class="flex items-center gap-2 cursor-pointer">
              <input type="radio" bind:group={splitFinal} value="bottom" class="accent-white" disabled={splitOtherTatami == null} />
              Tatami {otherNum ?? "-"}, setelah pool bawah
            </label>
            <label class="flex items-center gap-2 cursor-pointer">
              <input type="radio" bind:group={splitFinal} value="separate" class="accent-white" />
              Sesi terpisah (pilih selnya nanti)
            </label>
          </fieldset>

          {#if candidates.every((c) => !c.free)}
            <div class="text-rose-300 text-[11px]">Semua tatami lain sudah terisi di sesi ini. Kosongkan satu sel dulu.</div>
          {/if}
        </div>
        <div class="flex justify-end gap-2 pt-3 border-t border-white/[0.08]">
          <button onclick={() => (splitTarget = null)} class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl">Batal</button>
          <button
            onclick={applySplit}
            disabled={splitOtherTatami == null}
            class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md disabled:opacity-40"
          >Bagi</button>
        </div>
      {/if}
    </div>
  </div>
{/if}

<!-- Auto-Jadwal Cerdas Modal -->
{#if autoScheduleModalOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs animate-in fade-in duration-100">
    <div class="bg-[#181a20] border border-amber-500/30 rounded-2xl p-6 shadow-2xl max-w-lg w-full flex flex-col max-h-[85vh]">
      <div class="flex items-center justify-between pb-3 border-b border-white/[0.08] shrink-0">
        <div class="flex items-center space-x-2">
          <svg class="w-5 h-5 text-amber-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
          </svg>
          <h3 class="text-base font-black text-white uppercase tracking-tight">Auto-Jadwal Cerdas</h3>
        </div>
        <button onclick={() => (autoScheduleModalOpen = false)} class="text-zinc-400 hover:text-white p-1" title="Tutup dialog">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
        </button>
      </div>

      <div class="py-4 space-y-4 overflow-y-auto pr-1 scrollbar-thin">
        <p class="text-xs text-zinc-400">
          Algoritma mendistribusikan seluruh nomor tanding ke arena Tatami secara merata, menyelaraskan durasi waktu, serta <strong>mencegah bentrok jadwal atlet</strong>.
        </p>

        <!-- Start Time -->
        <div>
          <label for="auto-start-time-input" class="block text-[11px] font-mono text-zinc-400 uppercase mb-1">Jam Mulai Acara:</label>
          <input
            id="auto-start-time-input"
            type="time"
            bind:value={startTimeStr}
            class="w-full bg-[#14161b] text-white text-xs px-3 py-2 rounded-xl border border-white/10 outline-none"
          />
        </div>

        <!-- Overwrite Schedule Option -->
        <div class="p-3 rounded-xl bg-[#14161b] border border-white/[0.06]">
          <label class="flex items-start space-x-2.5 cursor-pointer">
            <input type="checkbox" bind:checked={autoOverwriteExisting} class="rounded accent-amber-500 mt-0.5" />
            <div>
              <span class="text-xs font-semibold text-white block">Susun Ulang Hari Ini</span>
              <span class="text-[10px] text-zinc-400 font-mono block mt-0.5">
                {autoOverwriteExisting
                  ? "Seluruh isi hari ini diganti. Kategori di hari lain tidak dipindah."
                  : "Isi hari ini dipertahankan; kategori yang belum terjadwal ditambahkan di bawahnya."}
              </span>
            </div>
          </label>
        </div>

        <!-- Festival Option -->
        <div class="p-3 rounded-xl bg-[#14161b] border border-white/[0.06] space-y-2">
          <label class="flex items-center space-x-2 cursor-pointer">
            <input type="checkbox" bind:checked={autoIncludeFestival} class="rounded accent-amber-500" />
            <span class="text-xs font-semibold text-white">
              {festivalSession
                ? "Sertakan Sesi FESTIVAL (1 sesi per tatami, dari roster festival)"
                : hasFestivalCategories
                  ? "Sertakan Kategori Festival"
                  : "Sertakan Sesi Festival Serentak di Tatami"}
            </span>
          </label>
          {#if autoIncludeFestival && !hasFestivalCategories && !festivalSession}
            <div class="pl-6">
              <label for="auto-festival-dur-select" class="block text-[10px] font-mono text-zinc-400 mb-1">Durasi Sesi Festival (Menit):</label>
              <select
                id="auto-festival-dur-select"
                bind:value={autoFestivalDur}
                class="bg-[#181a20] text-xs text-white px-2.5 py-1.5 rounded-lg border border-white/10 outline-none"
              >
                <option value={30}>30 Menit</option>
                <option value={45}>45 Menit</option>
                <option value={60}>60 Menit</option>
                <option value={90}>90 Menit</option>
              </select>
            </div>
          {/if}
        </div>

        <!-- ISHOMA Option -->
        <div class="p-3 rounded-xl bg-[#14161b] border border-white/[0.06]">
          <label class="flex items-center space-x-2 cursor-pointer">
            <input type="checkbox" bind:checked={autoIncludeIshoma} class="rounded accent-amber-500" />
            <span class="text-xs font-semibold text-white">Sertakan Istirahat Siang (ISHOMA 12:00 - 13:00)</span>
          </label>
        </div>

        <!-- Equal Matches & Conflict Prevention Options -->
        <div class="grid grid-cols-1 sm:grid-cols-2 gap-2">
          <div class="p-3 rounded-xl bg-[#14161b] border border-white/[0.06]">
            <label class="flex items-center space-x-2 cursor-pointer">
              <input type="checkbox" bind:checked={autoEqualMatches} class="rounded accent-amber-500" />
              <span class="text-xs font-semibold text-white">Ratakan Jam Selesai Antar Tatami</span>
            </label>
          </div>
          <div class="p-3 rounded-xl bg-[#14161b] border border-white/[0.06]">
            <label class="flex items-center space-x-2 cursor-pointer">
              <input type="checkbox" bind:checked={autoPreventConflicts} class="rounded accent-amber-500" />
              <span class="text-xs font-semibold text-white">Cegah Bentrok Jadwal Atlet</span>
            </label>
          </div>
        </div>

        <!-- Mode Prioritas Pengurutan -->
        <div>
          <label for="auto-sort-mode-select" class="block text-[11px] font-mono text-zinc-400 uppercase mb-1.5">Pola Prioritas Pengurutan:</label>
          <select
            id="auto-sort-mode-select"
            bind:value={autoSortMode}
            class="w-full bg-[#14161b] text-xs text-white px-3 py-2 rounded-xl border border-white/10 outline-none"
          >
            {#each AUTO_SORT_MODES as m}
              <option value={m.value}>{m.label}</option>
            {/each}
          </select>
        </div>

        <!-- Age Groups Checkboxes -->
        <div>
          <div class="flex items-center justify-between mb-1.5">
            <span class="text-[11px] font-mono text-zinc-400 uppercase">Kelompok Usia:</span>
            <div class="flex items-center space-x-2 text-[10px]">
              <button onclick={() => selectAllAgeGroups(true)} class="text-amber-400 hover:underline">Semua</button>
              <span>•</span>
              <button onclick={() => selectAllAgeGroups(false)} class="text-zinc-500 hover:underline">Kosongkan</button>
            </div>
          </div>
          <div class="grid grid-cols-2 gap-1.5">
            {#each ORDERED_GROUPS as grp}
              <label class="flex items-center space-x-2 px-2.5 py-1.5 rounded-lg bg-[#14161b] border border-white/[0.06] text-xs cursor-pointer">
                <input
                  type="checkbox"
                  checked={autoSelectedAgeGroups[grp] !== false}
                  onchange={() => toggleAgeGroup(grp)}
                  class="rounded accent-amber-500"
                />
                <span class="truncate">{grp}</span>
              </label>
            {/each}
          </div>
        </div>
      </div>

      <div class="flex items-center justify-end space-x-2 pt-3 border-t border-white/[0.08] shrink-0">
        <button
          onclick={() => (autoScheduleModalOpen = false)}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-300 hover:text-white text-xs font-semibold rounded-xl border border-white/10"
        >
          Batal
        </button>
        <button
          onclick={executeAutoScheduler}
          class="h-9 px-4 bg-gradient-to-r from-amber-500 to-amber-600 hover:from-amber-600 hover:to-amber-700 text-black font-black text-xs rounded-xl shadow-md transition"
        >
          Jalankan &amp; Terapkan
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Sesi Festival Prompt Modal -->
{#if festivalPromptOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs animate-in fade-in duration-100">
    <div class="bg-[#181a20] border border-amber-500/30 rounded-2xl p-6 shadow-2xl max-w-sm w-full">
      <div class="flex items-center justify-between pb-2 border-b border-white/[0.08]">
        <h3 class="text-base font-bold text-white uppercase">Tambah Sesi Festival</h3>
        <button onclick={() => (festivalPromptOpen = false)} class="text-zinc-400 hover:text-white p-1" title="Tutup dialog">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
        </button>
      </div>

      <div class="my-4 space-y-3">
        <div>
          <label for="fest-title-input" class="block text-[10px] font-mono text-zinc-400 uppercase mb-1">Judul Sesi:</label>
          <input
            id="fest-title-input"
            type="text"
            bind:value={festivalTitleInput}
            class="w-full bg-[#14161b] text-white text-xs px-3 py-2 rounded-xl border border-white/10 outline-none"
          />
        </div>
        <div>
          <label for="fest-ath-input" class="block text-[10px] font-mono text-zinc-400 uppercase mb-1">Total Atlet Festival:</label>
          <input
            id="fest-ath-input"
            type="number"
            bind:value={festivalAthletesInput}
            min="1"
            class="w-full bg-[#14161b] text-white text-xs px-3 py-2 rounded-xl border border-white/10 outline-none"
          />
        </div>
      </div>

      <div class="flex items-center justify-end space-x-2">
        <button onclick={() => (festivalPromptOpen = false)} class="h-9 px-3.5 bg-white/[0.04] text-zinc-300 text-xs font-semibold rounded-xl border border-white/10">Batal</button>
        <button onclick={insertFestivalSession} class="h-9 px-4 bg-amber-500 text-black font-bold text-xs rounded-xl shadow-md">Terapkan</button>
      </div>
    </div>
  </div>
{/if}

<!-- ISHOMA Prompt Modal -->
{#if ishomaPromptOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs animate-in fade-in duration-100">
    <div class="bg-[#181a20] border border-amber-500/30 rounded-2xl p-6 shadow-2xl max-w-sm w-full">
      <div class="flex items-center justify-between pb-2 border-b border-white/[0.08]">
        <h3 class="text-base font-bold text-white uppercase">Tambah Sesi ISHOMA</h3>
        <button onclick={() => (ishomaPromptOpen = false)} class="text-zinc-400 hover:text-white p-1" title="Tutup dialog">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
        </button>
      </div>
      <div class="my-4 space-y-3">
        <div>
          <label for="ishoma-time-input" class="block text-[10px] font-mono text-zinc-400 uppercase mb-1">Waktu Break:</label>
          <input
            id="ishoma-time-input"
            type="text"
            bind:value={ishomaTimeInput}
            class="w-full bg-[#14161b] text-white text-xs px-3 py-2 rounded-xl border border-white/10 outline-none"
          />
        </div>
        <div>
          <label for="ishoma-title-input" class="block text-[10px] font-mono text-zinc-400 uppercase mb-1">Keterangan:</label>
          <input
            id="ishoma-title-input"
            type="text"
            bind:value={ishomaTitleInput}
            class="w-full bg-[#14161b] text-white text-xs px-3 py-2 rounded-xl border border-white/10 outline-none"
          />
        </div>
      </div>
      <div class="flex items-center justify-end space-x-2">
        <button onclick={() => (ishomaPromptOpen = false)} class="h-9 px-3.5 bg-white/[0.04] text-zinc-300 text-xs font-semibold rounded-xl border border-white/10">Batal</button>
        <button onclick={insertIshomaSession} class="h-9 px-4 bg-amber-500 text-black font-bold text-xs rounded-xl shadow-md">Terapkan</button>
      </div>
    </div>
  </div>
{/if}

<!-- Kop Surat Settings Modal -->
{#if kopModalOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs animate-in fade-in duration-100">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl p-6 shadow-2xl max-w-md w-full">
      <div class="flex items-center justify-between pb-3 border-b border-white/[0.08]">
        <h3 class="text-base font-bold text-white uppercase">Pengaturan Kop Surat Panitia</h3>
        <button onclick={() => (kopModalOpen = false)} class="text-zinc-400 hover:text-white p-1" title="Tutup dialog">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
        </button>
      </div>

      <div class="my-4 space-y-3">
        <div>
          <label for="kop-nama-input" class="block text-[10px] font-mono text-zinc-400 uppercase mb-1">Nama Organisasi / Event:</label>
          <input
            id="kop-nama-input"
            type="text"
            bind:value={kopNama}
            class="w-full bg-[#14161b] text-white text-xs px-3 py-2 rounded-xl border border-white/10 outline-none"
          />
        </div>
        <div>
          <label for="kop-alamat-input" class="block text-[10px] font-mono text-zinc-400 uppercase mb-1">Alamat Venue:</label>
          <input
            id="kop-alamat-input"
            type="text"
            bind:value={kopAlamat}
            class="w-full bg-[#14161b] text-white text-xs px-3 py-2 rounded-xl border border-white/10 outline-none"
          />
        </div>
        <div>
          <label for="kop-kontak-input" class="block text-[10px] font-mono text-zinc-400 uppercase mb-1">Kontak / Tanggal Pelaksanaan:</label>
          <input
            id="kop-kontak-input"
            type="text"
            bind:value={kopKontak}
            class="w-full bg-[#14161b] text-white text-xs px-3 py-2 rounded-xl border border-white/10 outline-none"
          />
        </div>
      </div>

      <div class="flex items-center justify-end space-x-2 pt-2 border-t border-white/[0.08]">
        <button onclick={() => (kopModalOpen = false)} class="h-9 px-3.5 bg-white/[0.04] text-zinc-300 text-xs font-semibold rounded-xl border border-white/10">Batal</button>
        <button onclick={handleSaveKop} disabled={isSavingKop} class="h-9 px-4 bg-white text-black font-bold text-xs rounded-xl shadow-md">
          {isSavingKop ? 'Menyimpan...' : 'Simpan Kop'}
        </button>
      </div>
    </div>
  </div>
{/if}
{:else}
<!-- Printable Document Views (Simple Roster, Call Sheet, Booklet) -->
  <div class="print-overlay-wrapper fixed inset-0 z-50 bg-[#111215] flex flex-col overflow-y-auto p-6 select-text text-black">
    <!-- Screen-Only Toolbar -->
    <div class="no-print mb-4 pb-3 border-b border-white/10 flex items-center justify-between text-zinc-200 mx-auto w-full {printMode === 'simple_roster' ? (rosterLandscape ? 'max-w-[297mm]' : 'max-w-[210mm]') : 'max-w-5xl'}">
      <div class="flex items-center space-x-3">
        <h3 class="text-sm font-black text-white uppercase tracking-tight">
          {printMode === "simple_roster" ? "Cetak Roster" : "Pratinjau Dokumen Siap Cetak"}
        </h3>
        {#if printMode === "simple_roster"}
          <span class="px-2 py-0.5 rounded text-[11px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1]">
            A4 {rosterLandscape ? "LANDSCAPE" : "PORTRAIT"}{rosterFitZoom < 1 ? ` • DIPERKECIL ${Math.round(rosterFitZoom * 100)}%` : ""}
          </span>
        {/if}
        {#if printMode === "booklet" && tatamiBookletData}
          <span class="px-2.5 py-0.5 rounded text-[11px] font-mono font-bold bg-amber-500/10 text-amber-300 border border-amber-500/30">
            TATAMI {tatamiBookletData.tatami_number} • {2 + tatamiBookletData.ordered_categories.filter((c) => c.bagan_id && c.matches && c.matches.length > 0).length} HALAMAN (A4 LANDSCAPE)
          </span>
        {/if}
      </div>
      <div class="flex items-center space-x-2">
        {#if printMode === "booklet" && tatamis.length > 1}
          <!-- Quick Tatami Switcher in Preview -->
          <div class="flex items-center bg-white/[0.04] p-1 rounded-xl border border-white/10 space-x-1">
            {#each tatamis as t}
              <button
                onclick={() => openTatamiBooklet(t)}
                disabled={tatamiBookletLoading}
                class="px-2.5 py-1 rounded-lg text-xs font-bold transition flex items-center space-x-1 {printSelectedTatami?.id === t.id ? 'bg-white text-black shadow-sm' : 'text-zinc-400 hover:text-white hover:bg-white/[0.08]'}"
              >
                <span>Tatami {t.tatami_number}</span>
              </button>
            {/each}
          </div>
        {/if}

        <button
          onclick={() => (printMode = "none")}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl"
        >
          Tutup
        </button>
        <button
          onclick={triggerPrint}
          class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md flex items-center space-x-1.5"
        >
          <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
          </svg>
          <span>Cetak / Simpan PDF</span>
        </button>
      </div>
    </div>

    <!-- Print Paper Page Container (White Paper A4 Landscape) -->
    <div class="print-page-box {printMode === 'simple_roster' ? `roster-a4-sheet${rosterLandscape ? ' is-landscape' : ''}` : printMode === 'booklet' ? 'max-w-5xl mx-auto w-full booklet-preview-container' : 'max-w-5xl mx-auto w-full bg-white p-6 shadow-2xl rounded-lg text-black font-sans'}">
      {#if printMode === "simple_roster" && rosterDays.length > 0}
        <!-- Simple 1-Page Synchronized Roster Table (1:1 Ambrilindo Local Spec) -->
        <div
          class="simple-roster-page {simpleRosterDensityClass}"
          bind:this={rosterPageEl}
          style="zoom: {rosterFitZoom}; height: {100 / rosterFitZoom}%;"
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
          <!-- Day Section -->
          <div class="simple-roster-day-section">
            {#if rosterDays.length > 1}
              <div class="simple-roster-day-bar">
                <span class="day-bar-title">{day.label}</span>
                {#if day.tanggal}<span class="day-bar-date">{day.tanggal}</span>{/if}
              </div>
            {/if}
            <!-- Synchronized Run Sheet Table -->
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

          <!-- Bottom Section (Ambrilindo Local Spec - Pure - NO SIGNATURES) -->
          <div class="simple-roster-bottom-section">
            <div class="simple-roster-bottom-left">
              <div class="simple-roster-reminder-box">
                <span class="reminder-badge">PENTING</span>
                <span class="reminder-text">Jadwal bersifat estimasi. Atlet &amp; official wajib berada di area pemanasan 30 menit sebelum partai dimulai.</span>
              </div>

              {#if unassignedLabelsForPrint.length > 0}
                <div class="simple-roster-unassigned">
                  <span class="unassigned-title">Kelas tidak ditandingkan:</span>
                  <span class="unassigned-items">
                    {unassignedLabelsForPrint.join(', ')}
                  </span>
                </div>
              {/if}
            </div>

            <div class="simple-roster-bottom-right">
              <div class="simple-roster-ts">Dicetak: {printedTimeStr} • Ambrilindo Timetable Engine</div>
            </div>
          </div>
        </div>
      {:else if printMode === "detailed_timeline" && activeDay}
        <!-- Detailed Timeline Print (1:1 Ambrilindo Local Spec) -->
        <div class="detailed-print-page">
          <div class="detailed-header">
            <div style="display:flex; align-items:center; gap:10px;">
              {#if kopLogo}
                <img src={kopLogo} class="simple-roster-logo" alt="Logo Event" style="height:40px; width:auto; max-width:55px; object-fit:contain;" />
              {/if}
              <div>
                <h3 style="margin:0; font-family:inherit; font-weight:800; font-size:14px; color:#0f172a; text-transform:uppercase;">
                  {kopNama || eventName}
                </h3>
                {#if kopAlamat || kopKontak}
                  <p style="margin:2px 0 0; font-size:9.5px; color:#475569;">
                    {[kopAlamat, kopKontak].filter(Boolean).join(" • ")}
                  </p>
                {/if}
                <div style="margin-top:2px; font-size:11px; font-weight:800; color:#b91c1c; text-transform:uppercase;">
                  TIMELINE DETAIL PERTANDINGAN DENGAN ESTIMASI WAKTU
                </div>
              </div>
            </div>
            <div style="text-align:right; font-size:8.5px; color:#475569;">
              Dicetak: {printedTimeStr}<br />
              <strong>Ambrilindo Timetable Engine</strong>
            </div>
          </div>

          <div class="detailed-day-section">
            <div class="detailed-day-bar">
              <span class="detailed-day-title">{activeDay.label}</span>
              {#if activeDay.tanggal}
                <span class="detailed-day-date">{activeDay.tanggal}</span>
              {/if}
            </div>

            <div class="detailed-grid">
              {#each tatamis as t}
                {@const tMeta = tatamiTimelines[t.id]}
                <div class="detailed-col" style="flex: 1; min-width: 0;">
                  <div class="detailed-col-header">
                    <span>TATAMI {t.tatami_number}</span>
                    {#if tMeta}
                      <span class="detailed-col-sub">({tMeta.totalMatches} Partai • {tMeta.totalAthletes} Atlet)</span>
                    {/if}
                  </div>
                  <div class="detailed-col-body">
                    {#each activeDay.rows as row, rIdx}
                      {#if row.row_type === "label"}
                        {@const win = labelRowWindow(row)}
                        <div class="detailed-card is-break">
                          <div class="detailed-time">
                            {row.time_label || "12:00 - 13:00"} <span class="detailed-dur">({win.end - win.start} menit)</span>
                          </div>
                          <div class="detailed-title">{row.label_text || "ISHOMA / ISTIRAHAT SIANG"}</div>
                        </div>
                      {:else}
                        {@const cell = row.cells.find((c) => c.tatami_id === t.id)}
                        {@const time = cellTimes[`${rIdx}:${t.id}`]}
                        {#if cell && cell.nomor_tanding_id}
                          {@const seq = getTatamiCellSeq(t.id, rIdx)}
                          <div class="detailed-card">
                            <div class="detailed-time">
                              {time ? `${time.start}–${time.end}` : `Sesi ${rIdx + 1}`} <span class="detailed-dur">({time?.minutes ?? cell.est_duration_minutes} menit)</span>
                            </div>
                            <div class="detailed-title">
                              <span class="roster-num-badge">#{seq}</span>
                              {cellDisplayName(cell)} ({cell.atlet_count} Atlet • {cell.matches} Partai)
                            </div>
                          </div>
                        {:else if cell && cell.custom_text}
                          {@const isFest = cell.is_festival || cell.custom_text.toUpperCase().includes("FESTIVAL")}
                          {@const seq = getTatamiCellSeq(t.id, rIdx)}
                          <div class="detailed-card {isFest ? 'is-festival' : ''}">
                            <div class="detailed-time">
                              {time ? `${time.start}–${time.end}` : `Sesi ${rIdx + 1}`} <span class="detailed-dur">({time?.minutes ?? 0} menit)</span>
                            </div>
                            <div class="detailed-title">
                              <span class="roster-num-badge">#{seq}</span>
                              {textCellTitle(cell)}
                            </div>
                          </div>
                        {/if}
                      {/if}
                    {/each}
                  </div>
                </div>
              {/each}
            </div>
          </div>

          <div style="margin-top:14px; border-top:1px solid #cbd5e1; padding-top:6px; font-size:9px; color:#64748b; text-align:center;">
            Jadwal dan estimasi waktu bersifat panduan dinamis sesuai jalannya pertandingan di lapangan. Atlet &amp; official wajib bersiap 30 menit sebelum partai dimulai.
          </div>
        </div>
      {:else if printMode === "call_sheet" && activeDay && printSelectedTatami}
        <!-- Call Sheet Table for Tatami -->
        <div class="mb-3 flex items-center justify-between text-xs font-mono border-b pb-2">
          <span>HARI: {activeDay.label}</span>
          <span>TATAMI: {printSelectedTatami.tatami_number}</span>
        </div>

        <table class="w-full text-left border-collapse border border-zinc-400 text-xs">
          <thead>
            <tr class="bg-zinc-100">
              <th class="border border-zinc-400 p-2 w-12 text-center">Partai</th>
              <th class="border border-zinc-400 p-2">Kategori / Nomor Pertandingan</th>
              <th class="border border-zinc-400 p-2 w-28 text-center">Disiplin</th>
              <th class="border border-zinc-400 p-2 w-24 text-center">Status</th>
            </tr>
          </thead>
          <tbody>
            {#each activeDay.rows as row, idx}
              {@const cell = row.cells.find((c) => c.tatami_id === printSelectedTatami?.id)}
              {#if cell && cell.nomor_tanding_id}
                <tr>
                  <td class="border border-zinc-400 p-2 text-center font-bold font-mono">#{idx + 1}</td>
                  <td class="border border-zinc-400 p-2">
                    <div class="font-bold">{cellDisplayName(cell)}</div>
                    <div class="text-[10px] text-zinc-600">{cell.atlet_count} Atlet • {cell.matches} Match</div>
                  </td>
                  <td class="border border-zinc-400 p-2 text-center uppercase font-mono">{cell.discipline}</td>
                  <td class="border border-zinc-400 p-2 text-center">
                    <div class="border border-zinc-400 h-5 w-5 mx-auto rounded"></div>
                  </td>
                </tr>
              {/if}
            {/each}
          </tbody>
        </table>
      {:else if printMode === "booklet" && tatamiBookletData}
        {@const numRows = activeDay?.rows?.length || 0}
        {@const densityClass = numRows <= 15 ? 'density-spacious' : (numRows <= 28 ? 'density-normal' : (numRows <= 42 ? 'density-compact' : 'density-ultra-compact'))}
        <!-- BUKU TATAMI LO: 1. Sampul Resmi -> 2. Roster 1 Halaman -> 3. Bagan-bagan Terjadwal -->
        <div class="booklet-document-flow">
          <!-- ================= HALAMAN 1: SAMPUL RESMI EVENT ================= -->
          <div class="booklet-page booklet-cover-page">
            <div class="booklet-page-label-bar no-print">
              <span>Halaman 1 • Sampul Resmi Turnamen ({tatamiBookletData.day_label} - Tatami {tatamiBookletData.tatami_number})</span>
            </div>
            <div class="sampul-page-container">
              <!-- Top Ribbon Organization Text: Strictly within header container -->
              <div class="sampul-org-bar">
                {tatamiBookletData.kop_surat?.nama_organisasi || "FEDERASI OLAHRAGA KARATE-DO INDONESIA PROVINSI KALIMANTAN TIMUR"}
              </div>

              <!-- Event Logo Container: Top Right at middle boundary of grey container -->
              <div class="sampul-logo-container">
                <img
                  src={bookletLogoMap[tatamiBookletData.kop_surat?.logo || ""] || tatamiBookletData.kop_surat?.logo || kopLogo || "/img/cover/wkf_logo.png"}
                  class="sampul-event-logo"
                  alt="Logo Event"
                  onerror={(e) => { (e.currentTarget as HTMLImageElement).src = '/img/cover/wkf_logo.png'; }}
                />
              </div>

              <!-- Main Title Group inside Grey Container (BAGAN: / DAY X - TATAMI Y / NAMA EVENT / TANGGAL AWAL - AKHIR) -->
              <div class="sampul-main-group">
                <div class="sampul-bagan-label">BAGAN:</div>
                <div class="sampul-tatami-title">{tatamiBookletData.day_label?.toUpperCase() || 'DAY 1'} - TATAMI {tatamiBookletData.tatami_number}</div>
                <div class="sampul-event-title">{tatamiBookletData.event_name || eventName}</div>
                <div class="sampul-event-dates">{eventDateRangeStr || tatamiBookletData.day_date || activeDay?.tanggal || '3 Oktober 2026'}</div>
              </div>

              <!-- Supported by branding -->
              <div class="sampul-supported-by">
                SUPPORTED BY:
              </div>
            </div>
          </div>

          <!-- ================= HALAMAN 2: ROSTER PERTANDINGAN / RUN SHEET (1:1 A4 LANDSCAPE) ================= -->
          <div class="booklet-page booklet-roster-page">
            <div class="booklet-page-label-bar no-print">
              <span>Halaman 2 • Roster Pertandingan / Run Sheet ({tatamiBookletData.day_label} - Tatami {tatamiBookletData.tatami_number})</span>
            </div>
            <div class="roster-page-container">
              <div class="simple-roster-page {densityClass}">
                <!-- Header Kop Surat -->
                <div class="simple-roster-header">
                  <div class="simple-roster-kop-left">
                    <img
                      src={bookletLogoMap[tatamiBookletData.kop_surat?.logo || ""] || tatamiBookletData.kop_surat?.logo || kopLogo || "/img/cover/wkf_logo.png"}
                      class="simple-roster-logo"
                      alt="Logo Event"
                      onerror={(e) => { (e.currentTarget as HTMLImageElement).src = '/img/cover/wkf_logo.png'; }}
                    />
                    <div class="simple-roster-org-info">
                      <h3 class="simple-roster-org-name">{tatamiBookletData.kop_surat?.nama_organisasi || eventName}</h3>
                      {#if tatamiBookletData.kop_surat?.alamat || kopAlamat || venueLine1}
                        <p class="simple-roster-org-meta">
                          {tatamiBookletData.kop_surat?.alamat || kopAlamat || venueLine1}
                        </p>
                      {/if}
                      <div class="simple-roster-doc-title">ROSTER PERTANDINGAN / RUN SHEET</div>
                    </div>
                  </div>
                  <div class="simple-roster-day-badge-box">
                    <div class="simple-roster-day-label">{tatamiBookletData.day_label?.toUpperCase() || 'DAY 1'}</div>
                    <div class="simple-roster-day-date">{tatamiBookletData.day_date || activeDay?.tanggal || ''}</div>
                  </div>
                </div>

                <!-- Day Section -->
                {#if activeDay}
                  <div class="simple-roster-day-section">
                    <table class="simple-roster-table">
                      <thead>
                        <tr>
                          {#each tatamis as t}
                            {@const isTarget = t.tatami_number === tatamiBookletData.tatami_number}
                            {@const tMeta = tatamiTimelines[t.id]}
                            <th style="width: {100 / Math.max(1, tatamis.length)}%;" class={isTarget ? 'is-booklet-target-tatami' : ''}>
                              <div class="th-tatami-header">
                                <span class="th-tatami-title">TATAMI {t.tatami_number}</span>
                                {#if isTarget}
                                  <span class="th-target-badge">TATAMI INI</span>
                                {/if}
                                {#if tMeta}
                                  <span class="th-sub">({tMeta.totalMatches} Partai • {tMeta.totalAthletes} Atlet)</span>
                                {/if}
                              </div>
                            </th>
                          {/each}
                        </tr>
                      </thead>
                      <tbody>
                        {#if activeDay.rows.length === 0}
                          <tr>
                            <td colspan={tatamis.length} style="text-align:center; padding:8px; color:#94a3b8; font-style:italic;">
                              Belum ada nomor tanding yang dijadwalkan pada hari ini.
                            </td>
                          </tr>
                        {:else}
                          {#each activeDay.rows as row, rIdx}
                            {#if row.row_type === "label"}
                              <tr class="roster-ishoma-row">
                                <td colspan={tatamis.length} class="roster-ishoma-cell">
                                  <div class="roster-ishoma-bar">
                                    <span class="roster-ishoma-icon">🍱</span>
                                    <span class="roster-ishoma-label">{row.label_text || 'ISHOMA / ISTIRAHAT SIANG'}</span>
                                  </div>
                                </td>
                              </tr>
                            {:else}
                              <tr>
                                {#each tatamis as t}
                                  {@const isTarget = t.tatami_number === tatamiBookletData.tatami_number}
                                  {@const cell = row.cells.find((c) => c.tatami_id === t.id)}
                                  {#if cell && cell.nomor_tanding_id}
                                    {@const seq = getTatamiCellSeq(t.id, rIdx)}
                                    <td class="roster-match-cell {isTarget ? 'is-booklet-target-tatami' : ''}">
                                      <div class="roster-item-line">
                                        <span class="roster-num-badge">#{seq}</span>
                                        <span class="roster-item-title">
                                          {cell.discipline === 'kata' ? '🥋 ' : '🥊 '}
                                          {cellDisplayName(cell)}
                                        </span>
                                        {#if cell.atlet_count}
                                          <span class="roster-atlet-count">({cell.atlet_count})</span>
                                        {/if}
                                      </div>
                                    </td>
                                  {:else if cell && cell.custom_text}
                                    {@const isFest = cell.is_festival || cell.custom_text.toUpperCase().includes('FESTIVAL')}
                                    {@const seq = getTatamiCellSeq(t.id, rIdx)}
                                    <td class="roster-match-cell {isTarget ? 'is-booklet-target-tatami' : ''} {isFest ? 'is-festival' : ''}">
                                      <div class="roster-item-line">
                                        <span class="roster-num-badge">#{seq}</span>
                                        <span class="roster-item-title">
                                          {isFest ? '🥊 ' : ''}{textCellTitle(cell)}
                                        </span>
                                        {#if textCellAthletes(cell) > 0}
                                          <span class="roster-atlet-count">({textCellAthletes(cell)})</span>
                                        {/if}
                                      </div>
                                    </td>
                                  {:else}
                                    <td class="roster-match-cell is-empty {isTarget ? 'is-booklet-target-tatami' : ''}">
                                      <span>—</span>
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
                {/if}

                <!-- Bottom Section: Reminder + Unassigned + Ambrilindo Footer -->
                <div class="simple-roster-bottom-section">
                  <div class="simple-roster-bottom-left">
                    <div class="simple-roster-reminder-box">
                      <span class="reminder-badge"><i class="fas fa-exclamation-triangle"></i> PENTING</span>
                      <span class="reminder-text">Jadwal bersifat estimasi. Atlet &amp; official wajib berada di area pemanasan 30 menit sebelum partai dimulai.</span>
                    </div>

                    {#if unassignedLabelsForPrint.length > 0}
                      <div class="simple-roster-unassigned">
                        <span class="unassigned-title">KELAS TIDAK DITANDINGKAN:</span>
                        <span class="unassigned-items">
                          {unassignedLabelsForPrint.join(', ')}
                        </span>
                      </div>
                    {/if}
                  </div>

                  <div class="simple-roster-bottom-right">
                    <img src="/img/ambrilindo-logo.png" class="simple-roster-footer-logo" alt="Ambrilindo" />
                    <div class="simple-roster-ts">Dicetak: {printedTimeStr} • Ambrilindo</div>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <!-- ================= HALAMAN 3+: BAGAN-BAGAN PERTANDINGAN TERJADWAL ================= -->
          {#each tatamiBookletData.ordered_categories.filter((c) => c.bagan_id && c.matches && c.matches.length > 0) as bCat, bIdx}
            {@const bName = bCat.bagan_nama || bCat.category_name}
            {@const bMeta = deriveBracketMeta(bName, bCat.bagan_pool)}
            <div class="booklet-page booklet-bracket-page">
              <div class="booklet-page-label-bar no-print">
                <span>Halaman {bIdx + 3} • Bagan {bIdx + 1}/{tatamiBookletData.ordered_categories.filter((c) => c.bagan_id && c.matches && c.matches.length > 0).length}: ({bCat.bagan_kode || bCat.category_id}) {bCat.category_name}</span>
              </div>
              <!-- Same page as printing from the bagan channel (BracketPaperPage). -->
              <div class="bracket-page-container bracket-page-paper">
                <BracketPaperPage
                  kind={isFestivalName(bName) ? "festival" : "elim"}
                  matches={bCat.matches}
                  categoryName={bName}
                  baganKode={bCat.bagan_kode}
                  eventName={tatamiBookletData.event_name}
                  eventVenue={venueLine1}
                  eventDate={tatamiBookletData.day_date || activeDay?.tanggal || eventDateRangeStr}
                  logoMap={bookletLogoMap}
                  meta={bMeta}
                  podium={computePodium(bCat.matches, bMeta, winnersToPodium(bCat.winners))}
                  festivalGroupCount={bCat.bagan_pool ?? 0}
                  festivalLayout={bCat.bagan_layout_type}
                  widthMm={262}
                  heightMm={184}
                />
              </div>
            </div>
          {/each}
        </div>
      {/if}
    </div>
  </div>
{/if}

<!-- Conflict Details Modal -->
{#if conflictModalOpen && activeConflictRow}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs animate-in fade-in duration-100">
    <div class="bg-[#181a20] border border-rose-500/30 rounded-2xl p-6 shadow-2xl w-full max-w-lg">
      <div class="flex items-center justify-between pb-3 border-b border-white/[0.08]">
        <div class="flex items-center space-x-2">
          <svg class="w-5 h-5 text-rose-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"/>
          </svg>
          <h3 class="text-base font-bold text-white uppercase tracking-tight">
            Bentrok Jadwal Atlet ({activeConflictRow.conflicts.length})
          </h3>
        </div>
        <button onclick={() => (conflictModalOpen = false)} class="text-zinc-400 hover:text-white p-1" title="Tutup dialog">
          <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
        </button>
      </div>

      <div class="my-4 space-y-2.5 max-h-80 overflow-y-auto scrollbar-thin">
        {#each activeConflictRow.conflicts as conf}
          <div class="p-3 rounded-xl bg-rose-500/[0.06] border border-rose-500/20 text-xs">
            <div class="font-bold text-rose-300 text-sm">{conf.athlete_name}</div>
            <div class="mt-2 space-y-1 text-zinc-300">
              <div class="flex items-center justify-between">
                <span class="text-zinc-400">Tatami {conf.tatami1_number}:</span>
                <span class="font-semibold text-white">{conf.category1_name}</span>
              </div>
              <div class="flex items-center justify-between">
                <span class="text-zinc-400">Tatami {conf.tatami2_number}:</span>
                <span class="font-semibold text-white">{conf.category2_name}</span>
              </div>
            </div>
          </div>
        {/each}
      </div>

      <div class="flex justify-end pt-2">
        <button onclick={() => (conflictModalOpen = false)} class="h-9 px-4 bg-white text-black font-bold text-xs rounded-xl hover:bg-zinc-200 transition">
          Tutup
        </button>
      </div>
    </div>
  </div>
{/if}

<!-- Explicit Bulletproof Confirmation Modal (Zero Premature Execution) -->
{#if showConfirmModal}
  <div
    class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-xs select-none animate-in fade-in duration-150"
    onclick={(e) => {
      if (e.target === e.currentTarget && !isConfirmLoading) closeConfirmModal();
    }}
    onkeydown={(e) => {
      if (e.key === "Escape" && !isConfirmLoading) closeConfirmModal();
    }}
    role="dialog"
    aria-modal="true"
    tabindex="-1"
  >
    <div
      class="bg-[#181a20] border border-white/10 rounded-2xl p-6 shadow-2xl max-w-md w-full relative animate-in zoom-in-95 duration-150 text-zinc-200"
    >
      <div class="flex items-start space-x-3.5">
        <div
          class="w-10 h-10 rounded-xl flex items-center justify-center shrink-0 {confirmModalConfig.isDestructive
            ? 'bg-rose-500/15 text-rose-400 border border-rose-500/25'
            : 'bg-amber-500/15 text-amber-400 border border-amber-500/25'}"
        >
          {#if confirmModalConfig.isDestructive}
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"
              />
            </svg>
          {:else}
            <svg class="w-5 h-5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"
              />
            </svg>
          {/if}
        </div>

        <div class="flex-1 min-w-0">
          <h3 class="text-base font-bold text-white tracking-tight uppercase">
            {confirmModalConfig.title}
          </h3>
          <p class="text-xs text-zinc-300 mt-1.5 leading-relaxed break-words whitespace-pre-line">
            {confirmModalConfig.message}
          </p>
        </div>
      </div>

      <div class="flex items-center justify-end space-x-2.5 mt-6 pt-4 border-t border-white/[0.08]">
        <button
          type="button"
          onclick={closeConfirmModal}
          disabled={isConfirmLoading}
          class="h-9 px-4 bg-white/[0.06] hover:bg-white/[0.1] text-zinc-300 font-semibold text-xs rounded-xl border border-white/[0.1] transition-all disabled:opacity-50 cursor-pointer"
        >
          Batal
        </button>

        <button
          type="button"
          onclick={handleExecuteConfirmedAction}
          disabled={isConfirmLoading}
          class="h-9 px-4 {confirmModalConfig.isDestructive
            ? 'bg-rose-600 hover:bg-rose-500 text-white'
            : 'bg-white text-black hover:bg-zinc-200'} font-bold text-xs rounded-xl shadow-md transition-all active:scale-95 flex items-center space-x-1.5 disabled:opacity-50 cursor-pointer"
        >
          {#if isConfirmLoading}
            <span class="w-3.5 h-3.5 border-2 border-current border-t-transparent rounded-full animate-spin"></span>
            <span>Memproses...</span>
          {:else}
            <span>{confirmModalConfig.confirmText}</span>
          {/if}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  /* =========================================================
     🖨️ AMBRILINDO LOCAL 1:1 SPEC - SIMPLE ROSTER PRINT STYLES
     Clean, ink-friendly, NO black fills, aligned ISHOMA row
     ========================================================= */
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

  /* A4 portrait sheet for "Roster 1 halaman": 210 x 297 mm with the 6 mm print margin,
     so the preview is the printed page. The roster fills the 198 x 285 mm content box. */
  .roster-a4-sheet {
    width: 210mm;
    height: 297mm;
    padding: 6mm;
    margin: 0 auto;
    box-sizing: border-box;
    background: #ffffff;
    color: #0f172a;
    box-shadow: 0 10px 25px -5px rgba(0, 0, 0, 0.45);
    flex-shrink: 0;
    overflow: hidden;
  }
  .roster-a4-sheet.is-landscape {
    width: 297mm;
    height: 210mm;
  }
  .roster-a4-sheet .simple-roster-page {
    display: flex;
    flex-direction: column;
    padding: 0;
    overflow: hidden;
  }
  /* Compact roster: rows as tall as their text plus a little breathing room; large bold text. */
  .roster-a4-sheet .simple-roster-day-section {
    margin-bottom: 0;
  }
  .roster-a4-sheet .simple-roster-bottom-section {
    margin-top: 4px;
  }
  .roster-a4-sheet .simple-roster-table th {
    padding: 3px 4px;
    font-size: 11px;
    font-weight: 900;
    line-height: 1.15;
  }
  .roster-a4-sheet .simple-roster-table th .th-sub {
    font-size: 8.5px;
    font-weight: 700;
  }
  .roster-a4-sheet .simple-roster-page td.roster-match-cell {
    padding: 3px 4px;
    font-size: 10.5px;
    line-height: 1.2;
    vertical-align: middle;
  }
  .roster-a4-sheet .roster-item-line {
    gap: 4px;
    align-items: center;
    line-height: 1.2;
  }
  .roster-a4-sheet .roster-item-title {
    font-weight: 800;
    margin-top: 0;
    line-height: 1.2;
  }
  .roster-a4-sheet .roster-atlet-count {
    font-weight: 800;
    margin-top: 0;
    line-height: 1.2;
    align-self: center;
  }
  .roster-a4-sheet .roster-num-badge {
    min-width: 18px;
    margin-top: 0;
    padding: 1px 3px;
    font-size: 9px;
    font-weight: 900;
    border-radius: 2px;
    line-height: 1.15;
  }
  .roster-a4-sheet .roster-match-cell.is-empty {
    font-size: 9px;
  }
  .roster-a4-sheet .roster-ishoma-cell {
    padding: 2px 4px !important;
  }
  .roster-a4-sheet .roster-ishoma-bar {
    font-size: 10px;
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

  /* =========================================================
     🖨️ DETAILED TIMELINE PRINT STYLES (Ambrilindo Local Spec)
     ========================================================= */
  .detailed-print-page {
    box-sizing: border-box;
    padding: 4mm 6mm;
    background: #fff !important;
    color: #0f172a !important;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, "Helvetica Neue", Arial, sans-serif;
  }
  .detailed-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 2px solid #cbd5e1;
    padding-bottom: 6px;
    margin-bottom: 8px;
  }
  .detailed-day-section {
    margin-bottom: 12px;
    page-break-inside: avoid;
  }
  .detailed-day-bar {
    background: #f8fafc;
    border: 1px solid #cbd5e1;
    border-left: 3px solid #b91c1c;
    padding: 3px 8px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 6px;
  }
  .detailed-day-title {
    font-family: inherit;
    font-weight: 800;
    font-size: 12px;
    color: #b91c1c;
    text-transform: uppercase;
  }
  .detailed-day-date {
    font-size: 10px;
    font-weight: 600;
    color: #475569;
  }
  .detailed-grid {
    display: flex;
    gap: 10px;
  }
  .detailed-col {
    border: 1px solid #cbd5e1;
    border-radius: 4px;
    background: #fff;
    overflow: hidden;
  }
  .detailed-col-header {
    background: #f1f5f9;
    padding: 4px 8px;
    border-bottom: 1px solid #cbd5e1;
    font-size: 11px;
    font-weight: 800;
    color: #0f172a;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .detailed-col-sub {
    font-size: 9px;
    font-weight: 600;
    color: #64748b;
  }
  .detailed-col-body {
    padding: 6px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .detailed-card {
    border: 1px solid #e2e8f0;
    border-left: 3.5px solid #3b82f6;
    background: #fafafa;
    border-radius: 3px;
    padding: 4px 6px;
    font-size: 9.5px;
  }
  .detailed-card.is-break {
    border-left: 3.5px solid #0284c7;
    background: #f0fdf4;
    border-top: 1px dashed #0284c7;
    border-bottom: 1px dashed #0284c7;
  }
  .detailed-card.is-festival {
    border-left: 3.5px solid #f59e0b;
    background: #fffbeb;
  }
  .detailed-time {
    font-size: 8.5px;
    font-weight: 700;
    color: #2563eb;
    margin-bottom: 2px;
  }
  .detailed-dur {
    color: #64748b;
    font-weight: 600;
  }
  .detailed-title {
    font-weight: 700;
    color: #0f172a;
    line-height: 1.2;
  }

  /* =========================================================
     📖 BUKU TATAMI LO PRINT & VIEW STYLES (1:1 AMBRILINDO LOCAL)
     ========================================================= */
  .booklet-preview-container {
    padding: 0;
    margin: 0 auto;
    width: 100%;
    max-width: 1000px;
    display: flex;
    flex-direction: column;
    gap: 32px;
  }

  .booklet-document-flow {
    display: flex;
    flex-direction: column;
    gap: 32px;
    width: 100%;
  }

  .booklet-page {
    position: relative;
    background: #ffffff;
    box-shadow: 0 12px 35px rgba(0, 0, 0, 0.45);
    border-radius: 6px;
    overflow: hidden;
    page-break-after: always;
    break-after: page;
    box-sizing: border-box;
    width: 100%;
    max-width: 1000px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
  }

  .booklet-page-label-bar {
    background: #181a20;
    color: #94a3b8;
    padding: 6px 12px;
    font-size: 11px;
    font-weight: 700;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, sans-serif;
    letter-spacing: 0.05em;
    text-transform: uppercase;
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    flex-shrink: 0;
  }

  /* PAGE 1: SAMPUL (COVER) */
  .booklet-cover-page {
    background: #111827;
    width: 100%;
    overflow: hidden;
  }

  .sampul-page-container {
    width: 100%;
    aspect-ratio: 285 / 198;
    position: relative;
    background: #111827 url("/img/cover/cover_clean_template.png") no-repeat center center;
    background-size: 100% 100%;
    overflow: hidden;
    user-select: text;
    box-sizing: border-box;
  }

  .sampul-bg-img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: fill;
    z-index: 1;
    pointer-events: none;
  }

  .sampul-org-bar {
    position: absolute;
    top: 3.2%;
    height: 9.8%;
    left: 38%;
    right: 8%;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    text-align: right;
    font-family: "Arial Narrow", "Helvetica Neue Condensed", Arial, sans-serif;
    font-size: clamp(10px, 1.3vw, 13px);
    font-weight: 800;
    color: #ffffff;
    line-height: 1;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    text-shadow: 0 2px 4px rgba(0, 0, 0, 0.8);
    z-index: 10;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .sampul-logo-container {
    position: absolute;
    top: 15.5%;
    right: 3.5%;
    width: 28%;
    height: 30%;
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 15;
  }

  .sampul-event-logo {
    max-width: 100%;
    max-height: 100%;
    object-fit: contain;
    filter: drop-shadow(0 4px 10px rgba(0, 0, 0, 0.45));
  }

  .sampul-main-group {
    position: absolute;
    top: 66%;
    left: 57.5%;
    width: 39%;
    transform: translateY(-50%);
    display: flex;
    flex-direction: column;
    justify-content: center;
    box-sizing: border-box;
    padding: 0;
    z-index: 10;
    gap: 4px;
  }

  .sampul-bagan-label {
    font-family: "Oswald", "Arial Black", sans-serif;
    font-size: clamp(20px, 2.7vw, 32px);
    font-weight: 900;
    color: #ffffff;
    letter-spacing: 0.05em;
    line-height: 1.05;
    text-transform: uppercase;
    text-shadow: 0 2px 8px rgba(0, 0, 0, 0.8);
    margin: 0;
  }

  .sampul-tatami-title {
    font-family: "Oswald", "Arial Black", sans-serif;
    font-size: clamp(22px, 3.1vw, 36px);
    font-weight: 900;
    color: #ffffff;
    letter-spacing: 0.03em;
    line-height: 1.1;
    text-transform: uppercase;
    text-shadow: 0 2px 8px rgba(0, 0, 0, 0.85);
    margin: 0 0 4px 0;
  }

  .sampul-event-title {
    font-family: "Inter", Arial, sans-serif;
    font-size: clamp(12px, 1.5vw, 17px);
    font-weight: 800;
    color: #facc15;
    letter-spacing: 0.02em;
    line-height: 1.2;
    text-transform: uppercase;
    text-shadow: 0 2px 6px rgba(0, 0, 0, 0.8);
    margin: 0;
  }

  .sampul-event-dates {
    font-family: "Inter", Arial, sans-serif;
    font-size: clamp(10px, 1.2vw, 14px);
    font-weight: 700;
    color: #facc15;
    letter-spacing: 0.03em;
    line-height: 1.2;
    text-transform: uppercase;
    text-shadow: 0 1px 4px rgba(0, 0, 0, 0.8);
    margin: 0;
  }

  .sampul-supported-by {
    position: absolute;
    top: 83%;
    left: 80.5%;
    font-family: "Calibri", "Inter", sans-serif;
    font-size: clamp(8px, 0.9vw, 10px);
    font-weight: 800;
    color: #ffffff;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    text-shadow: 0 1px 4px rgba(0, 0, 0, 0.8);
    z-index: 10;
  }

  /* PAGE 2: ROSTER PERTANDINGAN (ALL TATAMIS - 100% A4 PORTRAIT) */
  .booklet-roster-page {
    background: #ffffff;
    padding: 0;
    box-sizing: border-box;
    width: 100%;
    overflow: hidden;
  }

  .roster-page-container {
    width: 210mm;
    max-width: 100%;
    aspect-ratio: 210 / 297;
    margin: 0 auto;
    background: #ffffff;
    padding: 8mm 10mm;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    overflow: hidden;
  }

  .roster-page-container .simple-roster-page {
    width: 100%;
    height: 100%;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 0;
    min-height: 0;
  }

  .roster-page-container .simple-roster-day-section {
    flex: 1 1 0%;
    min-height: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    margin-top: 2px;
  }

  .roster-page-container .simple-roster-table {
    width: 100%;
    height: 100%;
    flex: 1;
    border-collapse: collapse;
    table-layout: fixed;
  }

  /* Roster layout helpers */

  .roster-ishoma-row td {
    background: #fef3c7 !important;
    border: 1.5px dashed #d97706 !important;
    padding: 3px 6px !important;
  }

  .roster-ishoma-cell {
    text-align: center;
    font-weight: 900;
    color: #92400e;
    font-size: 10px;
  }

  .roster-ishoma-bar {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    letter-spacing: 0.04em;
  }

  .roster-footer-ts {
    font-size: 6.5px;
    font-family: monospace;
    color: #64748b;
  }

  /* BRACKET FOOTER */
  .bracket-page-footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-top: 1.2px solid #111827;
    padding-top: 3px;
    margin-top: 3px;
    height: 20px;
    font-size: 7.5px;
    box-sizing: border-box;
    flex-shrink: 0;
  }

  .bracket-sig-block {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .bracket-sig-label {
    font-weight: 800;
    color: #111827;
  }

  .bracket-sig-dots {
    color: #64748b;
    letter-spacing: 1px;
  }

  .bracket-footer-brand {
    font-family: monospace;
    font-size: 6.5px;
    color: #64748b;
    text-align: center;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 350px;
  }

  /* PAGE 3+: BAGAN PERTANDINGAN (1:1 BAGAN-DETAIL.HTML PRINT GEOMETRY) */
  .booklet-bracket-page {
    background: #ffffff;
    padding: 0;
    width: 100%;
    box-sizing: border-box;
    overflow: hidden;
  }

  .bracket-page-container.bracket-page-paper {
    padding: 0;
    display: flex;
    justify-content: center;
  }

  .bracket-page-container {
    width: 100%;
    aspect-ratio: 285 / 198;
    background: #ffffff;
    box-sizing: border-box;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    padding: 8px 12px;
  }

  .bracket-print-header {
    display: grid;
    grid-template-columns: 140px 1fr 140px;
    align-items: center;
    padding: 0 4px 3px 4px;
    border-bottom: 1.5px solid #111827;
    margin-bottom: 3px;
    height: 38px;
    box-sizing: border-box;
    flex-shrink: 0;
  }

  .bracket-print-logo-left {
    display: flex;
    align-items: center;
    justify-content: flex-start;
    height: 100%;
  }

  .bracket-print-logo-left img {
    max-height: 34px;
    width: auto;
    max-width: 130px;
    object-fit: contain;
  }

  .bracket-print-title {
    text-align: center;
    width: 100%;
    margin: 0 auto;
  }

  .bracket-print-title p {
    font-size: 8.5px;
    color: #4b5563;
    margin: 1px 0 0 0;
    text-align: center;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .bracket-print-spacer {
    width: 100%;
    height: 100%;
  }

  .bracket-scroll-viewport {
    background: #ffffff;
    border: none;
    padding: 0;
    margin: 0;
    min-height: 0;
    height: 100%;
    flex: 1 1 0%;
    overflow: hidden;
    width: 100%;
    max-width: 100%;
    box-sizing: border-box;
  }

  .booklet-bracket-outer {
    padding: 0;
    min-width: 0;
    width: 100%;
    height: 100%;
    min-height: 0;
    position: relative;
    box-sizing: border-box;
    overflow: hidden;
  }

  .bracket-svg {
    position: absolute;
    top: 0;
    left: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    overflow: visible;
    display: block;
    z-index: 1;
  }

  .bracket-grid {
    display: flex;
    flex-direction: row;
    justify-content: space-between;
    gap: 20px;
    width: 100%;
    height: 100%;
    min-height: 0;
    position: relative;
    z-index: 2;
  }

  .bracket-grid.rounds-1,
  .bracket-grid.is-single-round {
    justify-content: center;
    align-items: center;
  }

  .bracket-grid.rounds-2 {
    justify-content: center;
    gap: 48px;
  }

  .bracket-grid.rounds-3 {
    justify-content: center;
    gap: 24px;
  }

  .bracket-grid.rounds-4 {
    justify-content: space-between;
    gap: 14px;
  }

  .round-col {
    display: flex;
    flex-direction: column;
    flex: 1 1 0;
    min-width: 0;
    max-width: none;
    height: 100%;
    position: relative;
  }

  .round-col.is-final {
    flex: 1.15 1 0;
    position: relative;
    height: 100%;
  }

  .round-label {
    color: #111827;
    border-bottom: 1.2px solid #111827;
    font-size: 8px;
    font-weight: 800;
    margin-bottom: 3px;
    padding-bottom: 2px;
    line-height: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    font-family: var(--font-mono);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .round-step {
    background: #111827;
    color: #ffffff;
    border: 1px solid #111827;
    width: 13px;
    height: 13px;
    border-radius: 9999px;
    font-size: 6.5px;
    font-weight: 800;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    font-family: monospace;
  }

  .round-step.is-final-step {
    background: #111827;
    border-color: #111827;
  }

  .round-matches {
    display: flex;
    flex-direction: column;
    justify-content: space-around;
    flex: 1;
    gap: 2px;
    height: 100%;
    padding: 0;
  }

  .match {
    background: #ffffff;
    border: 1px solid #475569;
    border-radius: 3px;
    box-shadow: none;
    margin: 0;
    page-break-inside: avoid;
    break-inside: avoid;
    box-sizing: border-box;
    overflow: hidden;
  }

  .match-header {
    background: #f1f5f9;
    color: #1e293b;
    border-bottom: 1px solid #cbd5e1;
    padding: 2px 6px;
    font-size: 8px;
    font-weight: 800;
    line-height: 1.15;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .match-num {
    color: #0f172a;
    font-size: 8px;
    font-weight: 800;
    font-family: var(--font-mono);
    text-transform: uppercase;
  }

  .player {
    background: #ffffff;
    min-height: 25px;
    padding: 3px 6px;
    gap: 5px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    box-sizing: border-box;
  }

  .player.is-winner {
    background: #fef3c7 !important;
    border-left: 3.5px solid #d97706 !important;
  }

  .player-side {
    font-family: var(--font-mono);
    font-size: 9px;
    font-weight: 800;
    letter-spacing: 0.02em;
    text-transform: uppercase;
    min-width: 28px;
    padding: 2.5px 5px;
    border-radius: 3px;
    text-align: center;
    line-height: 1;
    color: #ffffff;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
  }

  .player-side.aka {
    background: #dc2626;
    border: 0.5px solid #b91c1c;
    color: #ffffff;
  }

  .player-side.ao {
    background: #2563eb;
    border: 0.5px solid #1d4ed8;
    color: #ffffff;
  }

  .player-dojo-logo {
    width: 24px;
    height: 24px;
    object-fit: contain;
    flex-shrink: 0;
    border-radius: 0;
    background: transparent;
  }

  .player-info {
    flex: 1;
    min-width: 0;
    line-height: 1.15;
  }

  .player-name {
    color: #0f172a;
    font-size: 10px;
    font-weight: 800;
    line-height: 1.2;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .player-name.empty {
    color: #94a3b8 !important;
    font-style: italic;
  }

  .player-sub {
    color: #1e293b;
    font-size: 8px;
    font-weight: 600;
    line-height: 1.15;
    margin-top: 1px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .player-meta-extra {
    margin-top: 1px;
    display: flex;
    gap: 3px;
  }

  .meta-kata {
    font-size: 7.5px;
    padding: 0.5px 3px;
    border-radius: 2px;
    border: 0.5px solid #94a3b8;
    background: #f1f5f9;
    color: #334155;
    font-weight: 700;
  }

  .meta-kata.aka {
    background: #fef2f2;
    border-color: #fca5a5;
    color: #991b1b;
  }

  .meta-kata.ao {
    background: #eff6ff;
    border-color: #93c5fd;
    color: #1e40af;
  }

  .meta-vr-badge {
    height: 12px;
    font-size: 7px;
    padding: 0 3px;
    border-radius: 2px;
    background: #2563eb;
    color: #ffffff;
    font-weight: 800;
    margin-left: 3px;
  }

  .player-score-wrap {
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 20px;
    flex-shrink: 0;
  }

  .player-score {
    background: #f8fafc;
    color: #0f172a;
    border: 1px solid #94a3b8;
    height: 20px;
    min-width: 20px;
    font-size: 9.5px;
    font-weight: 800;
    padding: 0 3px;
    border-radius: 3px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-family: var(--font-mono);
  }

  .player-score-small {
    font-size: 7.5px;
    color: #64748b;
  }

  .winner-crown {
    color: #d97706;
    margin-right: 2px;
  }

  .priority-star {
    color: #d97706;
    margin-right: 2px;
  }

  /* PODIUM CARD (1:1 BAGAN-DETAIL.HTML) */
  .cal-final-podium-wrap {
    position: absolute;
    bottom: 0;
    right: 0;
    width: 100%;
    z-index: 10;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    page-break-inside: avoid;
    break-inside: avoid;
  }

  .cal-podium-brand {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    padding: 0 4px;
    background: transparent;
    border: none;
    width: 100%;
  }

  .podium-brand-logo {
    width: auto;
    max-width: 130px;
    height: auto;
    max-height: 38px;
    display: block;
    object-fit: contain;
  }

  .cal-podium-card {
    background: #ffffff;
    border: 1.5px solid #d97706;
    border-radius: 4px;
    position: relative;
    width: 100%;
    margin-top: 0;
    display: block;
    overflow: hidden;
    page-break-inside: avoid;
    break-inside: avoid;
  }

  .cal-podium-header {
    background: #fef3c7;
    border-bottom: 1px solid #d97706;
    padding: 2px 6px;
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .cal-podium-header-title {
    color: #92400e;
    font-size: 8.5px;
    font-weight: 900;
    letter-spacing: 0.02em;
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .cal-podium-body {
    display: flex;
    flex-direction: column;
    padding: 2px 0;
    background: #ffffff;
  }

  .cal-podium-row {
    padding: 2.5px 6px;
    border-bottom: 0.5px solid #f1f5f9;
    gap: 6px;
    display: flex;
    align-items: center;
  }

  .cal-podium-row:last-child {
    border-bottom: none;
  }

  .cal-podium-rank {
    flex-shrink: 0;
  }

  .medal {
    width: 14px;
    height: 14px;
    font-size: 7px;
    font-weight: 800;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-family: var(--font-mono);
    line-height: 1;
  }

  .medal.gold {
    background: #fde047;
    color: #854d0e;
    border: 1px solid #ca8a04;
  }

  .medal.silver {
    background: #e2e8f0;
    color: #334155;
    border: 1px solid #94a3b8;
  }

  .medal.bronze {
    background: #fed7aa;
    color: #9a3412;
    border: 1px solid #f97316;
  }

  .cal-podium-meta {
    flex: 1;
    min-width: 0;
    line-height: 1.15;
  }

  .cal-podium-name {
    color: #0f172a;
    font-size: 8.5px;
    font-weight: 800;
    line-height: 1.15;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .cal-podium-sub {
    color: #1e293b;
    font-size: 7.5px;
    font-weight: 600;
    line-height: 1.15;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .cal-podium-blank-line {
    font-family: monospace;
    font-size: 8px;
    color: #64748b;
    font-weight: 700;
    line-height: 1.1;
    letter-spacing: -0.5px;
  }

  .cal-podium-blank-sub {
    font-family: monospace;
    font-size: 7px;
    color: #94a3b8;
    font-weight: 600;
    line-height: 1.1;
    letter-spacing: -0.5px;
  }

  /* =========================================================
     BODY.IS-PRINTING REFLOW STYLES (Enforces print geometry before window.print)
     ========================================================= */
  :global(body.is-printing) {
    background: #ffffff !important;
    color: #0f172a !important;
  }

  :global(body.is-printing .app-shell),
  :global(body.is-printing .workspace-column),
  :global(body.is-printing .main-content) {
    overflow: visible !important;
    height: auto !important;
    position: static !important;
    width: 100% !important;
    max-width: 100% !important;
    margin: 0 !important;
    padding: 0 !important;
    background: transparent !important;
  }

  :global(body.is-printing) .no-print,
  :global(body.is-printing) .booklet-page-label-bar {
    display: none !important;
  }

  :global(body.is-printing) .print-overlay-wrapper,
  :global(body.is-printing) .print-page-box,
  :global(body.is-printing) .booklet-preview-container,
  :global(body.is-printing) .booklet-document-flow {
    position: static !important;
    overflow: visible !important;
    height: auto !important;
    width: 100% !important;
    max-width: 100% !important;
    padding: 0 !important;
    margin: 0 !important;
    display: block !important;
    border: none !important;
    box-shadow: none !important;
    gap: 0 !important;
  }

  /* Page 1: Sampul */
  :global(body.is-printing) .booklet-cover-page {
    width: 100% !important;
    max-width: 285mm !important;
    height: 198mm !important;
    max-height: 198mm !important;
    margin: 0 auto !important;
    padding: 0 !important;
    overflow: hidden !important;
    box-shadow: none !important;
    box-sizing: border-box !important;
    page-break-after: always !important;
    break-after: page !important;
    page-break-inside: avoid !important;
    break-inside: avoid !important;
  }

  :global(body.is-printing) .booklet-cover-page .sampul-page-container {
    width: 100% !important;
    height: 100% !important;
    max-height: 100% !important;
    aspect-ratio: auto !important;
    border-radius: 0 !important;
    background: #111827 url("/img/cover/cover_clean_template.png") no-repeat center center !important;
    background-size: 100% 100% !important;
    overflow: hidden !important;
    position: relative !important;
    box-sizing: border-box !important;
  }

  :global(body.is-printing) .sampul-org-bar {
    position: absolute !important;
    top: 3.2% !important;
    height: 9.8% !important;
    left: 38% !important;
    right: 8% !important;
    display: flex !important;
    align-items: center !important;
    justify-content: flex-end !important;
    text-align: right !important;
    font-family: "Arial Narrow", "Helvetica Neue Condensed", Arial, sans-serif !important;
    font-size: 11.5pt !important;
    font-weight: 800 !important;
    color: #ffffff !important;
    line-height: 1 !important;
    letter-spacing: 0.02em !important;
    text-transform: uppercase !important;
    text-shadow: none !important;
    z-index: 10 !important;
    white-space: nowrap !important;
    overflow: hidden !important;
    text-overflow: ellipsis !important;
  }

  :global(body.is-printing) .sampul-logo-container {
    position: absolute !important;
    top: 15.5% !important;
    right: 3.5% !important;
    width: 28% !important;
    height: 30% !important;
    display: flex !important;
    align-items: center !important;
    justify-content: center !important;
    z-index: 10 !important;
  }

  :global(body.is-printing) .sampul-event-logo {
    max-width: 100% !important;
    max-height: 100% !important;
    object-fit: contain !important;
    filter: drop-shadow(0 4px 10px rgba(0, 0, 0, 0.45)) !important;
  }

  :global(body.is-printing) .sampul-main-group {
    position: absolute !important;
    top: 66% !important;
    left: 57.5% !important;
    width: 39% !important;
    transform: translateY(-50%) !important;
    z-index: 10 !important;
    display: flex !important;
    flex-direction: column !important;
    justify-content: center !important;
    gap: 4px !important;
  }

  :global(body.is-printing) .sampul-bagan-label {
    font-family: 'Roboto', 'Oswald', sans-serif !important;
    font-size: 26pt !important;
    font-weight: 900 !important;
    color: #ffffff !important;
    letter-spacing: 0.04em !important;
    line-height: 1.1 !important;
    text-transform: uppercase !important;
    text-shadow: none !important;
    margin: 0 !important;
  }

  :global(body.is-printing) .sampul-tatami-title {
    font-family: 'Roboto', 'Oswald', sans-serif !important;
    font-size: 26pt !important;
    font-weight: 900 !important;
    color: #ffffff !important;
    letter-spacing: 0.03em !important;
    line-height: 1.15 !important;
    text-transform: uppercase !important;
    text-shadow: none !important;
    margin: 0 0 6px 0 !important;
  }

  :global(body.is-printing) .sampul-event-title {
    font-family: 'Inter', Arial, sans-serif !important;
    font-size: 13.5pt !important;
    font-weight: 800 !important;
    color: #facc15 !important;
    line-height: 1.25 !important;
    text-transform: uppercase !important;
    text-shadow: none !important;
    margin: 0 !important;
  }

  :global(body.is-printing) .sampul-event-dates {
    font-family: 'Inter', Arial, sans-serif !important;
    font-size: 11.5pt !important;
    font-weight: 700 !important;
    color: #facc15 !important;
    line-height: 1.2 !important;
    text-shadow: none !important;
    margin: 0 !important;
  }

  :global(body.is-printing) .sampul-supported-by {
    position: absolute !important;
    top: 84.0% !important;
    left: 81.0% !important;
    font-family: 'Calibri', 'Inter', sans-serif !important;
    font-size: 11pt !important;
    font-weight: 800 !important;
    color: #ffffff !important;
    letter-spacing: 0.06em !important;
    text-transform: uppercase !important;
    text-shadow: none !important;
    z-index: 10 !important;
  }

  /* Page 2: Roster (100% A4 Landscape 285mm x 198mm) */
  :global(body.is-printing) .booklet-roster-page {
    width: 100% !important;
    max-width: 285mm !important;
    height: 198mm !important;
    max-height: 198mm !important;
    margin: 0 auto !important;
    padding: 4mm 6mm !important;
    overflow: hidden !important;
    display: flex !important;
    flex-direction: column !important;
    justify-content: space-between !important;
    box-sizing: border-box !important;
    box-shadow: none !important;
    page-break-after: always !important;
    break-after: page !important;
    page-break-inside: avoid !important;
    break-inside: avoid !important;
  }

  :global(body.is-printing) .booklet-roster-page .roster-page-container {
    width: 100% !important;
    height: 100% !important;
    max-height: 100% !important;
    margin: 0 auto !important;
    border-radius: 0 !important;
    box-sizing: border-box !important;
    padding: 0 !important;
    overflow: hidden !important;
    display: flex !important;
    flex-direction: column !important;
    justify-content: space-between !important;
  }

  :global(body.is-printing) .simple-roster-day-section {
    flex: 1 !important;
    min-height: 0 !important;
    display: flex !important;
    flex-direction: column !important;
    margin-top: 3px !important;
  }

  :global(body.is-printing) .simple-roster-table {
    width: 100% !important;
    height: 100% !important;
    flex: 1 !important;
    border-collapse: collapse !important;
    border: 1.5px solid #111827 !important;
    table-layout: fixed !important;
  }

  :global(body.is-printing) th.is-booklet-target-tatami {
    background: #dbeafe !important;
    border-top: 2.5px solid #1d4ed8 !important;
  }

  :global(body.is-printing) td.is-booklet-target-tatami {
    background: #eff6ff !important;
    font-weight: 600 !important;
  }

  :global(body.is-printing) .th-target-badge {
    font-size: 7.5px !important;
    font-weight: 800 !important;
    background: #1d4ed8 !important;
    color: #ffffff !important;
    padding: 1px 4px !important;
    border-radius: 2px !important;
    margin-left: 4px !important;
  }

  /* Page 3+: Bagan (100% A4 Landscape 285mm x 198mm) */
  :global(body.is-printing) .booklet-bracket-page {
    width: 100% !important;
    max-width: 285mm !important;
    height: 198mm !important;
    max-height: 198mm !important;
    margin: 0 auto !important;
    padding: 4mm 6mm !important;
    overflow: hidden !important;
    box-sizing: border-box !important;
    box-shadow: none !important;
    page-break-after: always !important;
    break-after: page !important;
    page-break-inside: avoid !important;
    break-inside: avoid !important;
  }

  :global(body.is-printing) .booklet-bracket-page:last-child,
  :global(body.is-printing) .booklet-page:last-child {
    page-break-after: avoid !important;
    break-after: avoid !important;
  }

  :global(body.is-printing) .bracket-page-container {
    width: 100% !important;
    height: 100% !important;
    max-height: 100% !important;
    margin: 0 auto !important;
    padding: 0 !important;
    box-sizing: border-box !important;
    overflow: hidden !important;
    display: flex !important;
    flex-direction: column !important;
  }

  :global(body.is-printing) .bracket-print-header {
    display: grid !important;
    grid-template-columns: 180px 1fr 180px !important;
    align-items: center !important;
    padding: 0 4px 3px 4px !important;
    border-bottom: 1.5px solid #111827 !important;
    margin-bottom: 3px !important;
    height: 42px !important;
    box-sizing: border-box !important;
    flex-shrink: 0 !important;
  }

  :global(body.is-printing) .bracket-scroll-viewport {
    height: 154mm !important;
    max-height: 154mm !important;
    overflow: hidden !important;
    width: 100% !important;
    flex: 1 !important;
  }

  :global(body.is-printing) .booklet-bracket-outer {
    height: 100% !important;
    width: 100% !important;
    overflow: hidden !important;
    position: relative !important;
    padding: 0 !important;
  }

  :global(body.is-printing) .bracket-grid {
    display: flex !important;
    flex-direction: row !important;
    justify-content: space-between !important;
    gap: 16px !important;
    width: 100% !important;
    height: 100% !important;
    position: relative !important;
    z-index: 2 !important;
  }

  :global(body.is-printing) .bracket-grid.rounds-1,
  :global(body.is-printing) .bracket-grid.is-single-round {
    justify-content: center !important;
    align-items: center !important;
  }

  :global(body.is-printing) .bracket-grid.rounds-2 {
    justify-content: center !important;
    gap: 56px !important;
  }

  :global(body.is-printing) .bracket-grid.rounds-3 {
    justify-content: center !important;
    gap: 24px !important;
  }

  :global(body.is-printing) .bracket-grid.rounds-4 {
    justify-content: space-between !important;
    gap: 14px !important;
  }

  :global(body.is-printing) .round-col {
    display: flex !important;
    flex-direction: column !important;
    flex: 1 1 0 !important;
    min-width: 0 !important;
    height: 100% !important;
    position: relative !important;
  }

  :global(body.is-printing) .round-col.is-final {
    flex: 1.15 1 0 !important;
    height: 100% !important;
    position: relative !important;
  }

  :global(body.is-printing) .round-matches {
    display: flex !important;
    flex-direction: column !important;
    justify-content: space-around !important;
    flex: 1 !important;
    gap: 2px !important;
    height: 100% !important;
    padding: 0 !important;
  }

  :global(body.is-printing) .cal-final-podium-wrap {
    position: absolute !important;
    bottom: 0 !important;
    right: 0 !important;
    width: 100% !important;
    max-width: 280px !important;
    z-index: 5 !important;
  }

  :global(body.is-printing) .match {
    background: #ffffff !important;
    border: 1px solid #475569 !important;
    border-radius: 4px !important;
    box-shadow: none !important;
    margin: 0 !important;
    width: 100% !important;
  }

  :global(body.is-printing) .match-header {
    background: #f1f5f9 !important;
    color: #1e293b !important;
    border-bottom: 1px solid #cbd5e1 !important;
    padding: 2px 6px !important;
    font-size: 8.5px !important;
    font-weight: 800 !important;
    display: flex !important;
    align-items: center !important;
    justify-content: space-between !important;
  }

  :global(body.is-printing) .player {
    background: #ffffff !important;
    min-height: 25px !important;
    padding: 2.5px 5px !important;
    gap: 5px !important;
    display: flex !important;
    flex-direction: row !important;
    align-items: center !important;
    flex-wrap: nowrap !important;
  }

  :global(body.is-printing) .player-side {
    font-family: var(--font-mono) !important;
    font-size: 8.5px !important;
    font-weight: 800 !important;
    letter-spacing: 0.02em !important;
    text-transform: uppercase !important;
    min-width: 26px !important;
    padding: 2px 4px !important;
    border-radius: 3px !important;
    text-align: center !important;
    line-height: 1 !important;
    color: #ffffff !important;
    display: inline-flex !important;
    align-items: center !important;
    justify-content: center !important;
    flex-shrink: 0 !important;
  }

  :global(body.is-printing) .player-dojo-logo {
    width: 20px !important;
    height: 20px !important;
    object-fit: contain !important;
    border: none !important;
    background: transparent !important;
    flex-shrink: 0 !important;
    padding: 0 !important;
    border-radius: 0 !important;
  }

  :global(body.is-printing) .player-info {
    flex: 1 1 0% !important;
    min-width: 0 !important;
    display: flex !important;
    flex-direction: column !important;
    justify-content: center !important;
    overflow: hidden !important;
    line-height: 1.15 !important;
  }

  :global(body.is-printing) .player-name {
    color: #0f172a !important;
    font-size: 10px !important;
    font-weight: 800 !important;
    line-height: 1.2 !important;
    white-space: nowrap !important;
    overflow: hidden !important;
    text-overflow: ellipsis !important;
  }

  :global(body.is-printing) .player-sub {
    color: #475569 !important;
    font-size: 6.5px !important;
    font-weight: 600 !important;
    line-height: 1.15 !important;
    letter-spacing: -0.015em !important;
    white-space: nowrap !important;
    overflow: hidden !important;
    text-overflow: ellipsis !important;
  }

  :global(body.is-printing) .player-score-wrap {
    flex-shrink: 0 !important;
  }

  :global(body.is-printing) .player-score {
    background: #f8fafc !important;
    color: #0f172a !important;
    border: 1px solid #94a3b8 !important;
    height: 20px !important;
    min-width: 20px !important;
    font-size: 9.5px !important;
    font-weight: 800 !important;
    padding: 0 3px !important;
    border-radius: 3px !important;
    display: inline-flex !important;
    align-items: center !important;
    justify-content: center !important;
  }

  /* PRINT ENGINE (@media print) */
  @media print {
    @page {
      size: A4 landscape;
      margin: 0;
    }

    @page roster-portrait {
      size: A4 portrait;
      margin: 0;
    }

    *, *::before, *::after {
      box-shadow: none !important;
      text-shadow: none !important;
    }

    :global(html), :global(body) {
      width: 100% !important;
      max-width: 100% !important;
      height: auto !important;
      min-height: auto !important;
      overflow: visible !important;
      position: static !important;
      background: #ffffff !important;
      color: #0f172a !important;
      margin: 0 !important;
      padding: 0 !important;
      -webkit-print-color-adjust: exact !important;
      print-color-adjust: exact !important;
    }

    :global(.app-shell),
    :global(.workspace-column),
    :global(.main-content),
    :global(main) {
      overflow: visible !important;
      height: auto !important;
      position: static !important;
      width: 100% !important;
      max-width: 100% !important;
      margin: 0 !important;
      padding: 0 !important;
      background: transparent !important;
    }

    .no-print, .booklet-page-label-bar {
      display: none !important;
    }

    .print-overlay-wrapper {
      position: static !important;
      overflow: visible !important;
      height: auto !important;
      width: 100% !important;
      max-width: 100% !important;
      background: #ffffff !important;
      padding: 0 !important;
      margin: 0 !important;
      display: block !important;
      border: none !important;
    }

    .print-page-box, .booklet-preview-container, .booklet-document-flow {
      position: static !important;
      overflow: visible !important;
      height: auto !important;
      width: 100% !important;
      max-width: 100% !important;
      box-shadow: none !important;
      border: none !important;
      background: transparent !important;
      padding: 0 !important;
      margin: 0 !important;
      display: block !important;
      gap: 0 !important;
    }

    .print-page-box.roster-a4-sheet {
      width: 100% !important;
      height: 285mm !important;
      padding: 0 !important;
      overflow: hidden !important;
      display: block !important;
    }
    /* landscape: 210 mm minus the 6 mm margins */
    .print-page-box.roster-a4-sheet.is-landscape {
      height: 198mm !important;
    }
    /* Compact roster: never stretch the table to fill the page (the booklet roster does). */
    .roster-a4-sheet .simple-roster-day-section {
      flex: 0 0 auto !important;
      display: block !important;
    }
    .roster-a4-sheet .simple-roster-table {
      height: auto !important;
      flex: none !important;
    }

    /* Common Booklet Page Base */
    .booklet-page {
      width: 100% !important;
      max-width: 285mm !important;
      height: 198mm !important;
      max-height: 198mm !important;
      margin: 0 auto !important;
      padding: 0 !important;
      overflow: hidden !important;
      box-sizing: border-box !important;
      box-shadow: none !important;
      border: none !important;
      border-radius: 0 !important;
      page-break-inside: avoid !important;
      break-inside: avoid !important;
      page-break-after: always !important;
      break-after: page !important;
    }
    .booklet-page:last-child {
      page-break-after: avoid !important;
      break-after: avoid !important;
    }

    /* Page 1: Sampul */
    .booklet-cover-page {
      width: 100% !important;
      max-width: 285mm !important;
      height: 198mm !important;
      max-height: 198mm !important;
      margin: 0 auto !important;
      padding: 0 !important;
      overflow: hidden !important;
      box-sizing: border-box !important;
      page-break-after: always !important;
      break-after: page !important;
      page-break-inside: avoid !important;
      break-inside: avoid !important;
    }

    .sampul-page-container {
      width: 100% !important;
      height: 100% !important;
      max-height: 100% !important;
      aspect-ratio: auto !important;
      border-radius: 0 !important;
      background: #111827 url("/img/cover/cover_clean_template.png") no-repeat center center !important;
      background-size: 100% 100% !important;
      overflow: hidden !important;
      position: relative !important;
      box-sizing: border-box !important;
    }

    .sampul-org-bar {
      position: absolute !important;
      top: 3.2% !important;
      height: 9.8% !important;
      left: 38% !important;
      right: 8% !important;
      display: flex !important;
      align-items: center !important;
      justify-content: flex-end !important;
      text-align: right !important;
      font-family: "Arial Narrow", "Helvetica Neue Condensed", Arial, sans-serif !important;
      font-size: 11.5pt !important;
      font-weight: 800 !important;
      color: #ffffff !important;
      line-height: 1 !important;
      letter-spacing: 0.02em !important;
      text-transform: uppercase !important;
      text-shadow: none !important;
      z-index: 10 !important;
      white-space: nowrap !important;
      overflow: hidden !important;
      text-overflow: ellipsis !important;
    }

    .sampul-logo-container {
      position: absolute !important;
      top: 15.5% !important;
      right: 3.5% !important;
      width: 28% !important;
      height: 30% !important;
      display: flex !important;
      align-items: center !important;
      justify-content: center !important;
      z-index: 10 !important;
    }

    .sampul-event-logo {
      max-width: 100% !important;
      max-height: 100% !important;
      object-fit: contain !important;
      filter: drop-shadow(0 4px 10px rgba(0, 0, 0, 0.45)) !important;
    }

    .sampul-main-group {
      position: absolute !important;
      top: 66% !important;
      left: 57.5% !important;
      width: 39% !important;
      transform: translateY(-50%) !important;
      z-index: 10 !important;
      display: flex !important;
      flex-direction: column !important;
      justify-content: center !important;
      gap: 4px !important;
    }

    .sampul-bagan-label {
      font-family: 'Roboto', 'Oswald', sans-serif !important;
      font-size: 26pt !important;
      font-weight: 900 !important;
      color: #ffffff !important;
      letter-spacing: 0.04em !important;
      line-height: 1.1 !important;
      text-transform: uppercase !important;
      margin: 0 !important;
    }

    .sampul-tatami-title {
      font-family: 'Roboto', 'Oswald', sans-serif !important;
      font-size: 26pt !important;
      font-weight: 900 !important;
      color: #ffffff !important;
      letter-spacing: 0.03em !important;
      line-height: 1.15 !important;
      text-transform: uppercase !important;
      margin: 0 0 6px 0 !important;
    }

    .sampul-event-title {
      font-family: 'Inter', Arial, sans-serif !important;
      font-size: 13.5pt !important;
      font-weight: 800 !important;
      color: #facc15 !important;
      line-height: 1.25 !important;
      text-transform: uppercase !important;
      margin: 0 !important;
    }

    .sampul-event-dates {
      font-family: 'Inter', Arial, sans-serif !important;
      font-size: 11.5pt !important;
      font-weight: 700 !important;
      color: #facc15 !important;
      line-height: 1.2 !important;
      margin: 0 !important;
    }

    .sampul-supported-by {
      position: absolute !important;
      top: 84.0% !important;
      left: 81.0% !important;
      font-family: 'Calibri', 'Inter', sans-serif !important;
      font-size: 11pt !important;
      font-weight: 800 !important;
      color: #ffffff !important;
      letter-spacing: 0.06em !important;
      text-transform: uppercase !important;
      z-index: 10 !important;
    }

    /* Page 2: Roster (100% A4 Portrait 210mm x 297mm) */
    .booklet-roster-page {
      page: roster-portrait !important;
      width: 100% !important;
      max-width: 210mm !important;
      height: 297mm !important;
      max-height: 297mm !important;
      margin: 0 auto !important;
      padding: 6mm 8mm !important;
      overflow: hidden !important;
      display: flex !important;
      flex-direction: column !important;
      justify-content: space-between !important;
      box-sizing: border-box !important;
      page-break-after: always !important;
      break-after: page !important;
      page-break-inside: avoid !important;
      break-inside: avoid !important;
    }

    .roster-page-container {
      width: 100% !important;
      height: 100% !important;
      max-height: 100% !important;
      margin: 0 auto !important;
      border-radius: 0 !important;
      box-sizing: border-box !important;
      padding: 0 !important;
      overflow: hidden !important;
      display: flex !important;
      flex-direction: column !important;
      justify-content: space-between !important;
    }

    .simple-roster-day-section {
      flex: 1 !important;
      min-height: 0 !important;
      display: flex !important;
      flex-direction: column !important;
      margin-top: 3px !important;
    }

    .simple-roster-table {
      width: 100% !important;
      height: 100% !important;
      flex: 1 !important;
      border-collapse: collapse !important;
      border: 1.5px solid #111827 !important;
      table-layout: fixed !important;
    }

    th.is-booklet-target-tatami {
      background: #dbeafe !important;
      border-top: 2.5px solid #1d4ed8 !important;
    }

    td.is-booklet-target-tatami {
      background: #eff6ff !important;
      font-weight: 600 !important;
    }

    .th-target-badge {
      font-size: 7.5px !important;
      font-weight: 800 !important;
      background: #1d4ed8 !important;
      color: #ffffff !important;
      padding: 1px 4px !important;
      border-radius: 2px !important;
      margin-left: 4px !important;
    }

    /* Page 3+: Bagan (100% A4 Landscape 285mm x 198mm) */
    .booklet-bracket-page {
      width: 100% !important;
      max-width: 285mm !important;
      height: 198mm !important;
      max-height: 198mm !important;
      margin: 0 auto !important;
      padding: 4mm 6mm !important;
      overflow: hidden !important;
      box-sizing: border-box !important;
      page-break-after: always !important;
      break-after: page !important;
      page-break-inside: avoid !important;
      break-inside: avoid !important;
    }

    .booklet-bracket-page:last-child {
      page-break-after: avoid !important;
      break-after: avoid !important;
    }

    .bracket-page-container {
      width: 100% !important;
      height: 100% !important;
      max-height: 100% !important;
      margin: 0 auto !important;
      overflow: hidden !important;
      display: flex !important;
      flex-direction: column !important;
      box-sizing: border-box !important;
      padding: 0 !important;
    }

    .bracket-print-header {
      display: grid !important;
      grid-template-columns: 180px 1fr 180px !important;
      align-items: center !important;
      padding: 0 4px 3px 4px !important;
      border-bottom: 1.5px solid #111827 !important;
      margin-bottom: 3px !important;
      height: 42px !important;
      box-sizing: border-box !important;
      flex-shrink: 0 !important;
    }

    .bracket-print-logo-left {
      display: flex !important;
      align-items: center !important;
      justify-content: flex-start !important;
      height: 100% !important;
    }

    .bracket-print-logo-left img {
      max-height: 40px !important;
      width: auto !important;
      max-width: 170px !important;
      object-fit: contain !important;
    }

    .bracket-print-title {
      text-align: center !important;
      width: 100% !important;
      margin: 0 auto !important;
    }

    .bracket-print-title p {
      font-size: 9.5px !important;
      color: #4b5563 !important;
      margin: 2px 0 0 0 !important;
      text-align: center !important;
      white-space: nowrap !important;
      overflow: hidden !important;
      text-overflow: ellipsis !important;
    }

    .bracket-print-spacer {
      width: 100% !important;
      height: 100% !important;
    }

    .bracket-scroll-viewport {
      background: #ffffff !important;
      border: none !important;
      padding: 0 !important;
      margin: 0 !important;
      min-height: 0 !important;
      height: 154mm !important;
      max-height: 154mm !important;
      overflow: hidden !important;
      width: 100% !important;
      flex: 1 !important;
    }

    .booklet-bracket-outer {
      padding: 0 !important;
      min-width: 0 !important;
      width: 100% !important;
      height: 100% !important;
      overflow: hidden !important;
      position: relative !important;
    }

    .bracket-grid {
      min-height: auto !important;
      height: 100% !important;
      display: flex !important;
      flex-direction: row !important;
      justify-content: space-between !important;
      gap: 16px !important;
      width: 100% !important;
      position: relative !important;
      z-index: 2 !important;
    }

    .bracket-grid.rounds-1,
    .bracket-grid.is-single-round {
      justify-content: center !important;
      align-items: center !important;
    }

    .bracket-grid.rounds-2 {
      justify-content: center !important;
      gap: 56px !important;
    }

    .bracket-grid.rounds-3 {
      justify-content: center !important;
      gap: 24px !important;
    }

    .bracket-grid.rounds-4 {
      justify-content: space-between !important;
      gap: 14px !important;
    }

    .round-col {
      display: flex !important;
      flex-direction: column !important;
      flex: 1 1 0 !important;
      min-width: 0 !important;
      height: 100% !important;
      position: relative !important;
    }

    .round-col.is-final {
      flex: 1.15 1 0 !important;
      height: 100% !important;
      position: relative !important;
    }

    .round-matches {
      display: flex !important;
      flex-direction: column !important;
      justify-content: space-around !important;
      flex: 1 !important;
      gap: 2px !important;
      height: 100% !important;
      padding: 0 !important;
    }

    .match {
      background: #ffffff !important;
      border: 1px solid #475569 !important;
      border-radius: 4px !important;
      box-shadow: none !important;
      margin: 0 !important;
      width: 100% !important;
      page-break-inside: avoid !important;
      break-inside: avoid !important;
    }

    .match-header {
      background: #f1f5f9 !important;
      color: #1e293b !important;
      border-bottom: 1px solid #cbd5e1 !important;
      padding: 2px 6px !important;
      font-size: 8.5px !important;
      font-weight: 800 !important;
      line-height: 1.15 !important;
      display: flex !important;
      align-items: center !important;
      justify-content: space-between !important;
    }

    .match-num {
      color: #0f172a !important;
      font-size: 8.5px !important;
      font-weight: 800 !important;
      font-family: var(--font-mono) !important;
      text-transform: uppercase !important;
    }

    .player {
      background: #ffffff !important;
      min-height: 25px !important;
      padding: 2.5px 5px !important;
      gap: 5px !important;
      display: flex !important;
      flex-direction: row !important;
      align-items: center !important;
      flex-wrap: nowrap !important;
    }

    .player.is-winner {
      background: #fef3c7 !important;
      border-left: 3.5px solid #d97706 !important;
      -webkit-print-color-adjust: exact !important;
      print-color-adjust: exact !important;
    }

    .player-side {
      font-family: var(--font-mono) !important;
      font-size: 8.5px !important;
      font-weight: 800 !important;
      letter-spacing: 0.02em !important;
      text-transform: uppercase !important;
      min-width: 26px !important;
      padding: 2px 4px !important;
      border-radius: 3px !important;
      text-align: center !important;
      line-height: 1 !important;
      color: #ffffff !important;
      display: inline-flex !important;
      align-items: center !important;
      justify-content: center !important;
      flex-shrink: 0 !important;
    }

    .player-side.aka {
      background: #dc2626 !important;
      border: 0.5px solid #b91c1c !important;
      color: #ffffff !important;
    }

    .player-side.ao {
      background: #2563eb !important;
      border: 0.5px solid #1d4ed8 !important;
      color: #ffffff !important;
    }

    .player-dojo-logo {
      width: 20px !important;
      height: 20px !important;
      object-fit: contain !important;
      border: none !important;
      background: transparent !important;
      flex-shrink: 0 !important;
      padding: 0 !important;
      border-radius: 0 !important;
    }

    .player-info {
      flex: 1 1 0% !important;
      min-width: 0 !important;
      display: flex !important;
      flex-direction: column !important;
      justify-content: center !important;
      overflow: hidden !important;
      line-height: 1.15 !important;
    }

    .player-name {
      color: #0f172a !important;
      font-size: 10px !important;
      font-weight: 800 !important;
      line-height: 1.2 !important;
      white-space: nowrap !important;
      overflow: hidden !important;
      text-overflow: ellipsis !important;
    }

    .player-name.empty {
      color: #94a3b8 !important;
      font-style: italic !important;
    }

    .player-sub {
      color: #475569 !important;
      font-size: 6.5px !important;
      font-weight: 600 !important;
      line-height: 1.15 !important;
      letter-spacing: -0.015em !important;
      margin-top: 1px !important;
      white-space: nowrap !important;
      overflow: hidden !important;
      text-overflow: ellipsis !important;
    }

    .player-score-wrap {
      flex-shrink: 0 !important;
    }

    .player-meta-extra {
      margin-top: 1px !important;
      display: flex !important;
      gap: 3px !important;
    }

    .meta-kata {
      font-size: 7.5px !important;
      padding: 0.5px 3px !important;
      border-radius: 2px !important;
      border-width: 0.5px !important;
      border-style: solid !important;
      line-height: 1 !important;
      font-weight: 700 !important;
      display: inline-block !important;
    }

    .meta-kata.aka {
      background: #fef2f2 !important;
      border-color: #fca5a5 !important;
      color: #991b1b !important;
    }

    .meta-kata.ao {
      background: #eff6ff !important;
      border-color: #93c5fd !important;
      color: #1e40af !important;
    }

    .player-score-wrap {
      display: flex !important;
      align-items: center !important;
      justify-content: center !important;
      min-width: 20px !important;
    }

    .player-score {
      background: #f8fafc !important;
      color: #0f172a !important;
      border: 1px solid #94a3b8 !important;
      height: 20px !important;
      min-width: 20px !important;
      font-size: 9.5px !important;
      font-weight: 800 !important;
      padding: 0 3px !important;
      border-radius: 3px !important;
      font-family: var(--font-mono) !important;
      display: inline-flex !important;
      align-items: center !important;
      justify-content: center !important;
    }

    .player-score-small {
      font-size: 7.5px !important;
      color: #64748b !important;
    }

    .cal-final-podium-wrap {
      position: absolute !important;
      bottom: 0 !important;
      right: 0 !important;
      width: 100% !important;
      max-width: 290px !important;
      z-index: 10 !important;
      display: flex !important;
      flex-direction: column !important;
      align-items: center !important;
      gap: 4px !important;
      visibility: visible !important;
    }

    .cal-podium-brand {
      display: flex !important;
      align-items: center !important;
      justify-content: flex-end !important;
      padding: 0 4px !important;
      background: transparent !important;
      border: none !important;
      width: 100% !important;
    }

    .podium-brand-logo {
      width: auto !important;
      max-width: 130px !important;
      height: auto !important;
      max-height: 38px !important;
      display: block !important;
      object-fit: contain !important;
    }

    .cal-podium-card {
      background: #ffffff !important;
      border: 1.5px solid #d97706 !important;
      border-radius: 4px !important;
      width: 100% !important;
      overflow: hidden !important;
    }

    .cal-podium-header {
      background: #fef3c7 !important;
      border-bottom: 1px solid #d97706 !important;
      padding: 2px 6px !important;
      display: flex !important;
      align-items: center !important;
      justify-content: space-between !important;
    }

    .cal-podium-header-title {
      color: #92400e !important;
      font-size: 8.5px !important;
      font-weight: 900 !important;
      display: flex !important;
      align-items: center !important;
      gap: 4px !important;
    }

    .cal-podium-body {
      padding: 2px 0 !important;
      background: #ffffff !important;
    }

    .cal-podium-row {
      padding: 2.5px 6px !important;
      border-bottom: 0.5px solid #f1f5f9 !important;
      display: flex !important;
      align-items: center !important;
      gap: 6px !important;
    }

    .cal-podium-row:last-child {
      border-bottom: none !important;
    }

    .cal-podium-rank {
      flex-shrink: 0 !important;
    }

    .cal-podium-meta {
      flex: 1 !important;
      min-width: 0 !important;
      line-height: 1.15 !important;
    }

    .cal-podium-name {
      font-size: 8.5px !important;
      font-weight: 800 !important;
      color: #0f172a !important;
      white-space: nowrap !important;
      overflow: hidden !important;
      text-overflow: ellipsis !important;
    }

    .cal-podium-sub {
      font-size: 7.5px !important;
      font-weight: 600 !important;
      color: #1e293b !important;
      white-space: nowrap !important;
      overflow: hidden !important;
      text-overflow: ellipsis !important;
    }

    .bracket-page-footer {
      display: none !important;
    }

    :global(svg line) {
      stroke: #09090b !important;
      stroke-width: 1.5px !important;
    }
  }
</style>
