<script lang="ts">
  import { onMount } from "svelte";
  import { invoke } from "$lib/api_client";
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
    TatamiBookletData,
    TatamiItem,
    TatamiLiveStatus,
    TimetableCategoryOption,
    TimetableEditorData,
    TimetableProgressSummary,
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
  let isEditMode = $state(false);
  let optionsMenuOpen = $state(false);

  // --- Confirmation Modal State (Explicit Bulletproof Confirmation) ---
  let showConfirmModal = $state(false);
  let isConfirmLoading = $state(false);
  let confirmModalConfig = $state<{
    action: "reset_all" | "delete_day" | "delete_tatami" | "clear_day" | null;
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
    action: "reset_all" | "delete_day" | "delete_tatami" | "clear_day";
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
        optionsMenuOpen = false;
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
        optionsMenuOpen = false;
        showConfirmModal = false;
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
  let bookletPruneEmptyRounds = $state(true);

  function groupMatchesByRound(matches: DetailBaganItem[], pruneEmptyRounds: boolean = true): Map<number, DetailBaganItem[]> {
    const map = new Map<number, DetailBaganItem[]>();
    // Filter out round > 4 (round 5 is final metadata in db, not an active bracket column)
    const validMatches = matches.filter((m) => !m.round || m.round <= 4);

    let startRound = 1;
    if (pruneEmptyRounds) {
      // Find lowest round that has at least 1 real athlete (Prune empty leading BYE rounds)
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

  function formatRoundTitle(colIdx: number, totalCols: number): string {
    const stepFromFinal = totalCols - 1 - colIdx;
    if (stepFromFinal === 0) return "FINAL";
    if (stepFromFinal === 1 && totalCols > 2) return "SEMI FINAL";
    if (stepFromFinal === 2 && totalCols > 3) return "PEREMPAT FINAL";
    return `BABAK ${colIdx + 1}`;
  }

  function getRoundStepBadge(colIdx: number, totalCols: number): string {
    const stepFromFinal = totalCols - 1 - colIdx;
    if (stepFromFinal === 0) return "FN";
    if (stepFromFinal === 1) return "SF";
    if (stepFromFinal === 2) return "QF";
    if (stepFromFinal === 3) return "R1";
    return `R${colIdx + 1}`;
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

  function getElementRelativeCoords(element: HTMLElement, ancestor: HTMLElement) {
    let x = 0;
    let y = 0;
    let curr: HTMLElement | null = element;
    while (curr && curr !== ancestor) {
      x += curr.offsetLeft;
      y += curr.offsetTop;
      curr = curr.offsetParent as HTMLElement | null;
    }
    return {
      x,
      y,
      width: element.offsetWidth,
      height: element.offsetHeight,
    };
  }

  function drawBookletConnectors() {
    if (!tatamiBookletData) return;
    const outerEls = document.querySelectorAll<HTMLElement>(".booklet-bracket-outer");
    outerEls.forEach((outer) => {
      const svg = outer.querySelector<SVGSVGElement>(".bracket-svg");
      if (!svg) return;
      svg.innerHTML = "";
      const outerRect = outer.getBoundingClientRect();
      const outerWidth = outer.offsetWidth || outerRect.width;
      const outerHeight = outer.offsetHeight || outerRect.height;
      if (outerWidth === 0 || outerHeight === 0) return;

      const isPrint = document.body.classList.contains("is-printing") || (window.matchMedia && window.matchMedia("print").matches);
      const strokeColor = isPrint ? "#111827" : "#1e293b";
      const strokeWidth = isPrint ? "1.4" : "1.8";

      const cols = Array.from(outer.querySelectorAll<HTMLElement>(".round-col"));
      for (let ri = 0; ri < cols.length - 1; ri++) {
        const fromCards = Array.from(cols[ri].querySelectorAll<HTMLElement>(".match"));
        const toCards = Array.from(cols[ri + 1].querySelectorAll<HTMLElement>(".match"));
        if (fromCards.length === 0 || toCards.length === 0) continue;

        const getCardGeometry = (card: HTMLElement) => {
          const r = card.getBoundingClientRect();
          if (outerRect.width > 0 && outerRect.height > 0 && r.width > 0 && r.height > 0) {
            return {
              xFrom: r.right - outerRect.left,
              xTo: r.left - outerRect.left,
              yCenter: r.top + r.height / 2 - outerRect.top,
            };
          }
          const coords = getElementRelativeCoords(card, outer);
          if (coords.width > 0 && coords.height > 0) {
            return {
              xFrom: coords.x + coords.width,
              xTo: coords.x,
              yCenter: coords.y + coords.height / 2,
            };
          }
          return {
            xFrom: r.right - outerRect.left,
            xTo: r.left - outerRect.left,
            yCenter: r.top + r.height / 2 - outerRect.top,
          };
        };

        // Case 1: 1-to-1 linear connector (e.g. 1 match feeding into final or semi)
        if (fromCards.length === 1 && toCards.length === 1) {
          const geomA = getCardGeometry(fromCards[0]);
          const geomOut = getCardGeometry(toCards[0]);
          const xMid = geomA.xFrom + (geomOut.xTo - geomA.xFrom) / 2;
          addSvgLine(svg, geomA.xFrom, geomA.yCenter, xMid, geomA.yCenter, strokeColor, strokeWidth);
          addSvgLine(svg, xMid, geomA.yCenter, xMid, geomOut.yCenter, strokeColor, strokeWidth);
          addSvgLine(svg, xMid, geomOut.yCenter, geomOut.xTo, geomOut.yCenter, strokeColor, strokeWidth);
        } else {
          // Case 2: Standard elimination bracket tree
          for (let i = 0; i < toCards.length; i++) {
            const cardA = fromCards[i * 2];
            const cardB = fromCards[i * 2 + 1];
            const cardOut = toCards[i];
            if (!cardA || !cardOut) continue;

            const geomA = getCardGeometry(cardA);
            const geomOut = getCardGeometry(cardOut);
            const xMid = geomA.xFrom + (geomOut.xTo - geomA.xFrom) / 2;

            if (cardB) {
              const geomB = getCardGeometry(cardB);
              const yMid = (geomA.yCenter + geomB.yCenter) / 2;

              addSvgLine(svg, geomA.xFrom, geomA.yCenter, xMid, geomA.yCenter, strokeColor, strokeWidth);
              addSvgLine(svg, geomB.xFrom, geomB.yCenter, xMid, geomB.yCenter, strokeColor, strokeWidth);
              addSvgLine(svg, xMid, geomA.yCenter, xMid, geomB.yCenter, strokeColor, strokeWidth);
              addSvgLine(svg, xMid, yMid, xMid, geomOut.yCenter, strokeColor, strokeWidth);
              addSvgLine(svg, xMid, geomOut.yCenter, geomOut.xTo, geomOut.yCenter, strokeColor, strokeWidth);
            } else {
              addSvgLine(svg, geomA.xFrom, geomA.yCenter, xMid, geomA.yCenter, strokeColor, strokeWidth);
              addSvgLine(svg, xMid, geomA.yCenter, xMid, geomOut.yCenter, strokeColor, strokeWidth);
              addSvgLine(svg, xMid, geomOut.yCenter, geomOut.xTo, geomOut.yCenter, strokeColor, strokeWidth);
            }
          }
        }
      }
    });
  }

  function addSvgLine(svg: SVGSVGElement, x1: number, y1: number, x2: number, y2: number, strokeColor = "#111827", strokeWidth = "1.4") {
    const line = document.createElementNS("http://www.w3.org/2000/svg", "line");
    line.setAttribute("x1", x1.toString());
    line.setAttribute("y1", y1.toString());
    line.setAttribute("x2", x2.toString());
    line.setAttribute("y2", y2.toString());
    line.setAttribute("stroke", strokeColor);
    line.setAttribute("stroke-width", strokeWidth);
    line.setAttribute("stroke-linecap", "round");
    svg.appendChild(line);
  }

  let tatamis = $state<TatamiItem[]>([]);
  let days = $state<EnrichedTimetableDay[]>([]);
  let categories = $state<TimetableCategoryOption[]>([]);
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
  let autoStartTime = $state("08:30");
  let autoIncludeFestival = $state(true);
  let autoFestivalDur = $state(45); // in minutes
  let autoIncludeIshoma = $state(true);
  let autoOverwriteExisting = $state(true);
  let autoSortMode = $state<"wkf_standard" | "kata_first" | "age_asc" | "age_desc" | "balanced">("wkf_standard");
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

  // --- Cell Picker & Conflict Modals ---
  let pickerModalOpen = $state(false);
  let targetRowIndex = $state<number | null>(null);
  let targetTatamiId = $state<number | null>(null);
  let pickerSearch = $state("");
  let pickerAgeFilter = $state("all");
  let customTextInput = $state("");

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
      categories = data.categories;
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

  // Calculate timeline per tatami (Start time -> match durations -> finish time)
  function parseTimeToMinutes(time: string): number {
    const parts = (time || "08:30").split(":");
    const h = parseInt(parts[0], 10) || 8;
    const m = parseInt(parts[1], 10) || 0;
    return h * 60 + m;
  }

  function formatMinutesToTime(min: number): string {
    const h = Math.floor(min / 60) % 24;
    const m = Math.floor(min % 60);
    return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
  }

  let tatamiTimelines = $derived.by(() => {
    if (!activeDay) return {};
    const timelines: Record<number, { totalMatches: number; totalAthletes: number; finishTime: string }> = {};

    tatamis.forEach((t) => {
      let curMin = parseTimeToMinutes(startTimeStr);
      let matchCount = 0;
      let athCount = 0;

      activeDay.rows.forEach((row) => {
        if (row.row_type === "label") {
          // Break row
          if (curMin < 12 * 60) {
            curMin = 13 * 60; // Advance to 13:00 after ISHOMA
          } else {
            curMin += 30;
          }
        } else {
          const cell = row.cells.find((c) => c.tatami_id === t.id);
          if (cell) {
            if (cell.nomor_tanding_id) {
              matchCount += cell.matches;
              athCount += cell.atlet_count;
              curMin += cell.est_duration_minutes || 15;
            } else if (cell.custom_text) {
              if (cell.is_break) {
                if (curMin < 12 * 60) curMin = 13 * 60;
                else curMin += 30;
              } else {
                athCount += cell.atlet_count || 10;
                matchCount += cell.matches || 10;
                curMin += 45;
              }
            }
          }
        }
      });

      timelines[t.id] = {
        totalMatches: matchCount,
        totalAthletes: athCount,
        finishTime: formatMinutesToTime(curMin),
      };
    });

    return timelines;
  });

  function getTatamiCellSeq(targetTatamiId: number, targetRowIdx: number): number {
    if (!activeDay) return 1;
    let seq = 1;
    for (let r = 0; r < activeDay.rows.length; r++) {
      const row = activeDay.rows[r];
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

  let simpleRosterDensityClass = $derived.by(() => {
    if (!activeDay) return "density-normal";
    const totalRows = activeDay.rows.length;
    if (totalRows <= 20) return "density-spacious";
    if (totalRows <= 35) return "density-normal";
    if (totalRows <= 50) return "density-compact";
    return "density-ultra-compact";
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
        if (cell && (cell.nomor_tanding_id || (cell.custom_text && !cell.is_break))) {
          const seq = getTatamiCellSeq(cell.tatami_id, rIdx);
          const name = cell.nomor_tanding_nama || cell.custom_text || "";
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

  // --- WKF Hierarchy & Parsing Helpers ---
  function parseWeightClass(name: string): number {
    const m = name.match(/([+-]?\d+)\s*kg/i);
    if (m) {
      const wStr = m[1];
      const isPlus = wStr.includes("+");
      const num = parseInt(wStr.replace(/[^\d]/g, ""), 10) || 0;
      return isPlus ? 1000 + num : num;
    }
    return 999;
  }

  function getAgeGroupRank(ageGroupStr: string, categoryName: string = ""): number {
    const raw = (ageGroupStr || "").toLowerCase().trim();
    const name = (categoryName || "").toLowerCase().trim();
    const combined = `${raw} ${name}`;

    if (combined.includes("pra usia dini")) return 0;
    if (combined.includes("usia dini")) return 1;
    if (combined.includes("pra pemula")) return 2;
    if (combined.includes("pemula")) return 3;
    if (combined.includes("kadet") || combined.includes("cadet")) return 4;
    if (combined.includes("junior")) return 5;
    if (combined.includes("under-21") || combined.includes("under 21") || combined.includes("u-21") || combined.includes("u21")) return 6;
    if (combined.includes("senior")) return 7;
    if (combined.includes("veteran")) return 8;

    return 9; // Lainnya
  }

  function getCategoryWkfTuple(cat: TimetableCategoryOption) {
    const lower = cat.nama.toLowerCase();

    // 1. Discipline: Kata (0) vs Kumite (1)
    const isKata = cat.discipline === "kata" || lower.includes("kata");
    const discRank = isKata ? 0 : 1;

    // 2. Age group: strictly computed via robust age group matcher (Under-21 is ALWAYS rank 6, Senior ALWAYS rank 7)
    const ageRank = getAgeGroupRank(cat.age_group, cat.nama);

    // 3. Team type: Perorangan (0) vs Beregu (1)
    const isBeregu = lower.includes("beregu") || lower.includes("team");
    const teamRank = isBeregu ? 1 : 0;

    // 4. Gender: Putra (0) vs Putri (1) vs Campuran/Mix (2)
    let genderRank = 2;
    if (
      lower.includes("putra") ||
      lower.includes(" pa ") ||
      lower.includes(" pa-") ||
      lower.includes("pria") ||
      lower.endsWith(" putra") ||
      lower.endsWith(" pa")
    ) {
      genderRank = 0;
    } else if (
      lower.includes("putri") ||
      lower.includes(" pi ") ||
      lower.includes(" pi-") ||
      lower.includes("wanita") ||
      lower.endsWith(" putri") ||
      lower.endsWith(" pi")
    ) {
      genderRank = 1;
    }

    // 5. Weight class ascending (for Kumite; 0 for Kata)
    const weightRank = isKata ? 0 : parseWeightClass(cat.nama);

    return { isKata, discRank, ageRank, teamRank, genderRank, weightRank };
  }

  function detectRowConflicts(cells: EnrichedTimetableCell[]): AthleteConflictDetail[] {
    const rowConflicts: AthleteConflictDetail[] = [];
    for (let i = 0; i < cells.length; i++) {
      for (let j = i + 1; j < cells.length; j++) {
        const cellA = cells[i];
        const cellB = cells[j];
        if (cellA.tatami_id === cellB.tatami_id) continue;
        if (!cellA.nomor_tanding_id || !cellB.nomor_tanding_id) continue;

        const athA = ntAthletesMap[cellA.nomor_tanding_id];
        const athB = ntAthletesMap[cellB.nomor_tanding_id];
        if (!athA || !athB) continue;

        const setB = new Set(athB);
        for (const ath of athA) {
          if (setB.has(ath)) {
            rowConflicts.push({
              athlete_name: ath,
              tatami1_number: cellA.tatami_number,
              category1_id: cellA.nomor_tanding_id,
              category1_name: cellA.nomor_tanding_nama || "",
              tatami2_number: cellB.tatami_number,
              category2_id: cellB.nomor_tanding_id,
              category2_name: cellB.nomor_tanding_nama || "",
            });
          }
        }
      }
    }
    return rowConflicts;
  }

  function executeAutoScheduler() {
    if (!activeDay) return;

    // Backup for Undo
    backupDaysForUndo = JSON.parse(JSON.stringify(days));

    // 1. Gather all candidates (unassigned categories or all if overwrite is active)
    const candidates = categories.filter((c) => {
      if (!autoOverwriteExisting && assignedCategoryIds.has(c.id)) return false;
      if (c.is_festival && !autoIncludeFestival) return false;
      const rank = getAgeGroupRank(c.age_group, c.nama);
      const normalizedGroup = ORDERED_GROUPS[rank] || "Lainnya";
      return autoSelectedAgeGroups[normalizedGroup] !== false && autoSelectedAgeGroups[c.age_group] !== false;
    });

    if (candidates.length === 0) {
      notify("Tidak ada kategori yang memenuhi syarat untuk dijadwalkan otomatis.");
      autoScheduleModalOpen = false;
      return;
    }

    const tatamiIds = tatamis.map((t) => t.id);
    if (tatamiIds.length === 0) {
      notify("Tidak ada tatami aktif untuk event ini.");
      autoScheduleModalOpen = false;
      return;
    }

    // 2. Sort master candidates based on selected sort mode
    const sortedCandidates = [...candidates];
    if (autoSortMode === "wkf_standard") {
      sortedCandidates.sort((a, b) => {
        const tA = getCategoryWkfTuple(a);
        const tB = getCategoryWkfTuple(b);
        if (tA.discRank !== tB.discRank) return tA.discRank - tB.discRank;
        if (tA.ageRank !== tB.ageRank) return tA.ageRank - tB.ageRank;
        if (tA.teamRank !== tB.teamRank) return tA.teamRank - tB.teamRank;
        if (tA.genderRank !== tB.genderRank) return tA.genderRank - tB.genderRank;
        if (tA.weightRank !== tB.weightRank) return tA.weightRank - tB.weightRank;
        return a.id - b.id;
      });
    } else if (autoSortMode === "kata_first") {
      sortedCandidates.sort((a, b) => {
        const tA = getCategoryWkfTuple(a);
        const tB = getCategoryWkfTuple(b);
        if (tA.discRank !== tB.discRank) return tA.discRank - tB.discRank;
        if (tA.ageRank !== tB.ageRank) return tA.ageRank - tB.ageRank;
        if (tA.teamRank !== tB.teamRank) return tA.teamRank - tB.teamRank;
        if (tA.genderRank !== tB.genderRank) return tA.genderRank - tB.genderRank;
        if (tA.weightRank !== tB.weightRank) return tA.weightRank - tB.weightRank;
        return a.id - b.id;
      });
    } else if (autoSortMode === "age_asc") {
      sortedCandidates.sort((a, b) => {
        const tA = getCategoryWkfTuple(a);
        const tB = getCategoryWkfTuple(b);
        if (tA.ageRank !== tB.ageRank) return tA.ageRank - tB.ageRank;
        if (tA.discRank !== tB.discRank) return tA.discRank - tB.discRank;
        if (tA.teamRank !== tB.teamRank) return tA.teamRank - tB.teamRank;
        if (tA.genderRank !== tB.genderRank) return tA.genderRank - tB.genderRank;
        if (tA.weightRank !== tB.weightRank) return tA.weightRank - tB.weightRank;
        return a.id - b.id;
      });
    } else if (autoSortMode === "age_desc") {
      sortedCandidates.sort((a, b) => {
        const tA = getCategoryWkfTuple(a);
        const tB = getCategoryWkfTuple(b);
        if (tA.ageRank !== tB.ageRank) return tB.ageRank - tA.ageRank;
        if (tA.discRank !== tB.discRank) return tA.discRank - tB.discRank;
        if (tA.teamRank !== tB.teamRank) return tA.teamRank - tB.teamRank;
        if (tA.genderRank !== tB.genderRank) return tA.genderRank - tB.genderRank;
        if (tA.weightRank !== tB.weightRank) return tA.weightRank - tB.weightRank;
        return a.id - b.id;
      });
    } else {
      // Balanced: standard WKF ordering as base hierarchy, load-balancing across tatamis
      sortedCandidates.sort((a, b) => {
        const tA = getCategoryWkfTuple(a);
        const tB = getCategoryWkfTuple(b);
        if (tA.discRank !== tB.discRank) return tA.discRank - tB.discRank;
        if (tA.ageRank !== tB.ageRank) return tA.ageRank - tB.ageRank;
        if (tA.teamRank !== tB.teamRank) return tA.teamRank - tB.teamRank;
        if (tA.genderRank !== tB.genderRank) return tA.genderRank - tB.genderRank;
        if (tA.weightRank !== tB.weightRank) return tA.weightRank - tB.weightRank;
        return a.id - b.id;
      });
    }

    interface EnrichedCandidate extends TimetableCategoryOption {
      globalRank: number;
      waveKey: string;
      discRank: number;
      ageRank: number;
    }

    const enrichedCandidates: EnrichedCandidate[] = sortedCandidates.map((c, idx) => {
      const t = getCategoryWkfTuple(c);
      const waveKey = (autoSortMode === "age_asc" || autoSortMode === "age_desc")
        ? `${t.ageRank}_${t.discRank}`
        : `${t.discRank}_${t.ageRank}`;
      return {
        ...c,
        globalRank: idx,
        waveKey,
        discRank: t.discRank,
        ageRank: t.ageRank,
      };
    });

    const startMin = parseTimeToMinutes(autoStartTime);
    const maxEndTime = Infinity; // Jadwalkan seluruh nomor tanding tanpa batasan jam sewenang-wenang
    const ishomaStartTime = 12 * 60;
    const ishomaEndTime = 13 * 60;

    // 3. Multi-Trial Simulation Solver (25 trials to achieve optimal match balancing & 0 collisions)
    interface TrialResult {
      tatamiRowsList: Record<number, EnrichedTimetableCell[]>;
      tatamiMinutes: Record<number, number>;
      tatamiMatches: Record<number, number>;
      unscheduled: EnrichedCandidate[];
      scheduledCount: number;
      score: number;
    }

    let bestTrial: TrialResult | null = null;
    const trialCount = 25;

    for (let trialSeed = 0; trialSeed < trialCount; trialSeed++) {
      let pool = enrichedCandidates.map((c) => ({ ...c }));
      let stalledIterations = 0;
      const currentClocks: Record<number, number> = {};
      const tatamiMatches: Record<number, number> = {};
      const tatamiAthletes: Record<number, number> = {};
      const tatamiMinutes: Record<number, number> = {};
      const tatamiHasKumite: Record<number, boolean> = {};
      const tatamiLastDisc: Record<number, number> = {};
      const tatamiLastAge: Record<number, number> = {};
      const ishomaInserted: Record<number, boolean> = {};
      const tatamiRowsList: Record<number, EnrichedTimetableCell[]> = {};
      const activeIntervals: Array<{
        tid: number;
        start: number;
        end: number;
        athletes: Set<string>;
      }> = [];

      tatamiIds.forEach((tid) => {
        currentClocks[tid] = startMin;
        tatamiMatches[tid] = 0;
        tatamiAthletes[tid] = 0;
        tatamiMinutes[tid] = startMin;
        tatamiHasKumite[tid] = false;
        tatamiLastDisc[tid] = 0;
        tatamiLastAge[tid] = 0;
        ishomaInserted[tid] = false;
        tatamiRowsList[tid] = [];
      });

      // Optional: Add initial Festival session across all tatamis (only if no individual festival categories in pool)
      if (autoIncludeFestival && !pool.some((c) => c.is_festival)) {
        const festEnd = startMin + autoFestivalDur;
        const festPerTatami = Math.round(50 / tatamiIds.length);
        tatamiIds.forEach((tid) => {
          tatamiRowsList[tid].push({
            id: 0,
            row_id: 0,
            tatami_id: tid,
            tatami_number: tatamis.find((t) => t.id === tid)?.tatami_number ?? 1,
            nomor_tanding_id: null,
            nomor_tanding_nama: null,
            custom_text: `FESTIVAL (${festPerTatami} Atlet)`,
            atlet_count: festPerTatami,
            matches: festPerTatami,
            discipline: "kata",
            est_duration_minutes: autoFestivalDur,
            progress_status: "festival",
            progress_pct: 0,
            finished_matches: 0,
            remaining_matches: festPerTatami,
            is_festival: true,
            is_break: false,
          });
          currentClocks[tid] = festEnd;
          tatamiMinutes[tid] = festEnd;
          tatamiAthletes[tid] += festPerTatami;
          tatamiMatches[tid] += festPerTatami;
        });
      }

      function insertIshomaCell(tid: number, start: number) {
        if (!autoIncludeIshoma || ishomaInserted[tid]) return;
        const dur = Math.max(1, ishomaEndTime - start);
        const tNum = tatamis.find((t) => t.id === tid)?.tatami_number ?? 1;
        tatamiRowsList[tid].push({
          id: 0,
          row_id: 0,
          tatami_id: tid,
          tatami_number: tNum,
          nomor_tanding_id: null,
          nomor_tanding_nama: null,
          custom_text: "ISHOMA",
          atlet_count: 0,
          matches: 0,
          discipline: "kata",
          est_duration_minutes: dur,
          progress_status: "belum_disentuh",
          progress_pct: 0,
          finished_matches: 0,
          remaining_matches: 0,
          is_festival: false,
          is_break: true,
        });
        currentClocks[tid] = ishomaEndTime;
        tatamiMinutes[tid] = ishomaEndTime;
        ishomaInserted[tid] = true;
      }

      while (pool.length > 0) {
        // Filter tatamis that are still within maxEndTime
        const openTatamis = tatamiIds.filter((tid) => currentClocks[tid] < maxEndTime);
        if (openTatamis.length === 0) break;

        // Check if any open tatami should take ISHOMA
        let handledIshoma = false;
        for (const tid of openTatamis) {
          if (autoIncludeIshoma && !ishomaInserted[tid] && currentClocks[tid] < ishomaEndTime) {
            const curClock = currentClocks[tid];
            const canFitBefore13 = pool.some((c) => curClock + (c.est_duration_minutes || 20) <= ishomaEndTime);
            if (curClock >= 12 * 60 + 30 || (curClock >= 12 * 60 && !canFitBefore13)) {
              insertIshomaCell(tid, curClock);
              handledIshoma = true;
            }
          }
        }
        if (handledIshoma) continue;

        // Sort tatamis by earliest clock, breaking ties by fewer assigned items
        openTatamis.sort((a, b) => {
          const cDiff = currentClocks[a] - currentClocks[b];
          if (cDiff !== 0) return cDiff;
          if (autoEqualMatches) {
            const mDiff = tatamiMatches[a] - tatamiMatches[b];
            if (mDiff !== 0) return mDiff;
          }
          return tatamiRowsList[a].length - tatamiRowsList[b].length;
        });

        const chosenTid = openTatamis[0];
        const s = currentClocks[chosenTid];
        const curRowIdx = tatamiRowsList[chosenTid].length;

        // STRICT HIERARCHY RULE:
        // Urutan progresi kategori usia WAJIB konsisten dan tidak boleh dibalik:
        // Pra Usia Dini (0) -> Usia Dini (1) -> Pra Pemula (2) -> Pemula (3) -> Kadet (4) -> Junior (5) -> Under-21 (6) -> Senior (7).
        // Under NO circumstances can a later class (e.g. Senior) leapfrog over an unassigned younger class (e.g. Under-21)!
        const activeWave = pool[0].waveKey;
        const currentDisc = pool[0].discRank;
        const unscheduledAgesOfDisc = pool
          .filter((c) => c.discRank === currentDisc)
          .map((c) => c.ageRank);
        const minUnscheduledAge = unscheduledAgesOfDisc.length > 0
          ? Math.min(...unscheduledAgesOfDisc)
          : 0;

        // Collect athletes already participating at curRowIdx across other tatamis
        const rowAthletes = new Set<string>();
        if (autoPreventConflicts) {
          tatamiIds.forEach((otherTid) => {
            if (otherTid !== chosenTid && curRowIdx < tatamiRowsList[otherTid].length) {
              const otherCell = tatamiRowsList[otherTid][curRowIdx];
              if (otherCell.nomor_tanding_id) {
                const aths = ntAthletesMap[otherCell.nomor_tanding_id] || [];
                aths.forEach((a) => rowAthletes.add(a));
              }
            }
          });
        }

        function isValidTatamiOrder(cand: EnrichedCandidate, tid: number): boolean {
          const cDisc = cand.discRank;
          const cAge = cand.ageRank;
          const lDisc = tatamiLastDisc[tid];
          const lAge = tatamiLastAge[tid];
          if (cDisc > lDisc) {
            return true;
          } else if (cDisc === lDisc) {
            return cAge >= lAge;
          }
          return false;
        }

        interface CandidateFeasibility {
          candIdx: number;
          cand: EnrichedCandidate;
          dur: number;
          end: number;
          score: number;
        }

        const feasible: CandidateFeasibility[] = [];

        for (let idx = 0; idx < pool.length; idx++) {
          const cand = pool[idx];
          // Stop scanning once outside the active wave
          if (cand.waveKey !== activeWave) break;

          // STRICT: Do not allow selecting an older age group if a younger age group in this discipline remains unassigned!
          if (cand.discRank === currentDisc && cand.ageRank > minUnscheduledAge) {
            break;
          }

          // Tatami must not reverse age progression within the same discipline:
          if (cand.discRank === tatamiLastDisc[chosenTid] && cand.ageRank < tatamiLastAge[chosenTid]) {
            continue;
          }

          if (!isValidTatamiOrder(cand, chosenTid)) {
            continue;
          }

          const isCandKata = cand.discipline === "kata" || cand.nama.toLowerCase().includes("kata");

          // Strict WKF Discipline Transition: Tatami that had Kumite cannot take Kata!
          if (tatamiHasKumite[chosenTid] && isCandKata) {
            continue;
          }

          const dur = cand.est_duration_minutes || (cand.matches * 3) || 15;
          const e = s + dur;

          // Check ISHOMA constraint: category cannot run past 12:45 into ISHOMA
          if (autoIncludeIshoma && !ishomaInserted[chosenTid] && s < ishomaEndTime && e > ishomaEndTime && s >= 12 * 60) {
            continue;
          }

          if (e > maxEndTime) continue;

          // Check active cross-tatami athlete conflicts
          if (autoPreventConflicts) {
            const candAthletes = ntAthletesMap[cand.id] || [];
            let conflictFound = false;

            if (candAthletes.length > 0) {
              // 1. Same-row collision check
              for (const ath of candAthletes) {
                if (rowAthletes.has(ath)) {
                  conflictFound = true;
                  break;
                }
              }

              // 2. Overlapping time interval collision check
              if (!conflictFound) {
                const candSet = new Set(candAthletes);
                for (const act of activeIntervals) {
                  if (act.tid !== chosenTid && s < act.end && e > act.start) {
                    for (const ath of candSet) {
                      if (act.athletes.has(ath)) {
                        conflictFound = true;
                        break;
                      }
                    }
                    if (conflictFound) break;
                  }
                }
              }
            }

            if (conflictFound) continue;
          }

          // Candidate is feasible within the current wave!
          const jitter = trialSeed > 0 ? (Math.random() * 0.8) : 0;
          const loadBalancingWeight = autoEqualMatches ? (tatamiMatches[chosenTid] * 0.05) : 0;
          const score = cand.globalRank + jitter + loadBalancingWeight;

          feasible.push({
            candIdx: idx,
            cand,
            dur,
            end: e,
            score,
          });
        }

        if (feasible.length > 0) {
          feasible.sort((a, b) => a.score - b.score);
          const pick = feasible[0];
          const cat = pool.splice(pick.candIdx, 1)[0];
          const dur = pick.dur;
          const end = pick.end;
          const tNum = tatamis.find((t) => t.id === chosenTid)?.tatami_number ?? 1;

          const isKata = cat.discipline === "kata" || cat.nama.toLowerCase().includes("kata");
          if (!isKata) {
            tatamiHasKumite[chosenTid] = true;
          }

          tatamiLastDisc[chosenTid] = cat.discRank;
          tatamiLastAge[chosenTid] = cat.ageRank;

          const candAthletesSet = new Set(ntAthletesMap[cat.id] || []);
          activeIntervals.push({
            tid: chosenTid,
            start: s,
            end: end,
            athletes: candAthletesSet,
          });

          tatamiRowsList[chosenTid].push({
            id: 0,
            row_id: 0,
            tatami_id: chosenTid,
            tatami_number: tNum,
            nomor_tanding_id: cat.id,
            nomor_tanding_nama: cat.nama,
            custom_text: "",
            atlet_count: cat.atlet_count,
            matches: cat.matches,
            discipline: cat.discipline,
            est_duration_minutes: dur,
            progress_status: cat.progress_status,
            progress_pct: cat.progress_pct,
            finished_matches: 0,
            remaining_matches: cat.matches,
            is_festival: cat.is_festival,
            is_break: false,
          });

          currentClocks[chosenTid] = end;
          tatamiMinutes[chosenTid] = end;
          tatamiMatches[chosenTid] += cat.matches;
          tatamiAthletes[chosenTid] += cat.atlet_count;
          stalledIterations = 0;
        } else {
          stalledIterations++;
          // Fallback deadlock resolver: if tatamis are stalled across multiple rounds,
          // force assign pool[0] to chosenTid to guarantee 100% of categories get scheduled!
          if (stalledIterations > tatamiIds.length * 2) {
            const forcedCat = pool.shift()!;
            const tNum = tatamis.find((t) => t.id === chosenTid)?.tatami_number ?? 1;
            const dur = forcedCat.est_duration_minutes || (forcedCat.matches * 3) || 15;
            const end = s + dur;

            const isKata = forcedCat.discipline === "kata" || forcedCat.nama.toLowerCase().includes("kata");
            if (!isKata) {
              tatamiHasKumite[chosenTid] = true;
            }

            tatamiLastDisc[chosenTid] = forcedCat.discRank;
            tatamiLastAge[chosenTid] = forcedCat.ageRank;

            const candAthletesSet = new Set(ntAthletesMap[forcedCat.id] || []);
            activeIntervals.push({
              tid: chosenTid,
              start: s,
              end: end,
              athletes: candAthletesSet,
            });

            tatamiRowsList[chosenTid].push({
              id: 0,
              row_id: 0,
              tatami_id: chosenTid,
              tatami_number: tNum,
              nomor_tanding_id: forcedCat.id,
              nomor_tanding_nama: forcedCat.nama,
              custom_text: "",
              atlet_count: forcedCat.atlet_count,
              matches: forcedCat.matches,
              discipline: forcedCat.discipline,
              est_duration_minutes: dur,
              progress_status: forcedCat.progress_status,
              progress_pct: forcedCat.progress_pct,
              finished_matches: 0,
              remaining_matches: forcedCat.matches,
              is_festival: forcedCat.is_festival,
              is_break: false,
            });

            currentClocks[chosenTid] = end;
            tatamiMinutes[chosenTid] = end;
            tatamiMatches[chosenTid] += forcedCat.matches;
            tatamiAthletes[chosenTid] += forcedCat.atlet_count;
            stalledIterations = 0;
            continue;
          }

          // Advance clock to wait for active conflicting match to end
          const futureEnds = activeIntervals
            .filter((a) => a.end > s)
            .map((a) => a.end);
          if (futureEnds.length > 0) {
            const nextClear = Math.min(...futureEnds);
            if (nextClear > s && nextClear <= maxEndTime) {
              currentClocks[chosenTid] = nextClear;
              continue;
            }
          }
          if (autoIncludeIshoma && !ishomaInserted[chosenTid] && s < 12 * 60) {
            currentClocks[chosenTid] = 12 * 60;
            continue;
          }
          currentClocks[chosenTid] += 5;
          if (currentClocks[chosenTid] >= maxEndTime) {
            break;
          }
        }
      }

      // Calculate trial score
      const scheduledCount = sortedCandidates.length - pool.length;
      const matchVals = tatamiIds.map((tid) => tatamiMatches[tid]);
      const maxM = Math.max(...matchVals);
      const minM = Math.min(...matchVals);
      const matchSpread = maxM - minM;

      const clockVals = tatamiIds.map((tid) => currentClocks[tid]);
      const maxC = Math.max(...clockVals);
      const minC = Math.min(...clockVals);
      const clockSpread = maxC - minC;

      // Primary: unscheduled count (lowest)
      // Secondary: match spread between tatamis (lowest)
      // Tertiary: clock difference
      const trialScore = (pool.length * 100000) + (matchSpread * 1000) + (clockSpread * 2);

      const trialResult: TrialResult = {
        tatamiRowsList,
        tatamiMinutes,
        tatamiMatches,
        unscheduled: pool,
        scheduledCount,
        score: trialScore,
      };

      if (!bestTrial || trialScore < bestTrial.score) {
        bestTrial = trialResult;
      }
    }

    if (!bestTrial) {
      notify("Gagal menghasilkan simulasi jadwal.");
      autoScheduleModalOpen = false;
      return;
    }

    // 4. Construct synchronized grid rows from bestTrial
    const maxRows = Math.max(...tatamiIds.map((tid) => bestTrial!.tatamiRowsList[tid].length), 1);
    const newDayRows: EnrichedTimetableRow[] = [];

    for (let r = 0; r < maxRows; r++) {
      const rowCells: EnrichedTimetableCell[] = tatamiIds.map((tid) => {
        const list = bestTrial!.tatamiRowsList[tid];
        if (r < list.length) {
          return list[r];
        }
        return {
          id: 0,
          row_id: 0,
          tatami_id: tid,
          tatami_number: tatamis.find((t) => t.id === tid)?.tatami_number ?? 1,
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
        };
      });

      const rowConflicts = detectRowConflicts(rowCells);

      newDayRows.push({
        id: 0,
        day_id: activeDay.id,
        order: r,
        row_type: "slot",
        time_label: `Sesi ${r + 1}`,
        label_text: "",
        cells: rowCells,
        conflicts: rowConflicts,
      });
    }

    activeDay.rows = newDayRows;

    // Recalculate total conflicts across all rows
    const totalNewConflicts = newDayRows.reduce((acc, row) => acc + row.conflicts.length, 0);
    summary.total_conflicts = totalNewConflicts;

    markDirty();
    autoScheduleModalOpen = false;

    // Build balance summary text
    const balanceTxt = tatamiIds
      .map((tid) => {
        const tObj = tatamis.find((t) => t.id === tid);
        return `Tatami ${tObj?.tatami_number}: ${bestTrial!.tatamiMatches[tid]} partai (${formatMinutesToTime(bestTrial!.tatamiMinutes[tid])})`;
      })
      .join(" • ");

    const unscheduledMsg = bestTrial.unscheduled.length > 0 
      ? ` (${bestTrial.unscheduled.length} kategori belum masuk batas jam)` 
      : "";

    autoResultBanner = {
      title: `Auto-Jadwal Berhasil Diterapkan (${bestTrial.scheduledCount} Kategori Terjadwal, ${totalNewConflicts} Bentrok Atlet)${unscheduledMsg}`,
      summary: balanceTxt,
    };

    notify(`Auto-jadwal selesai: ${bestTrial.scheduledCount} kategori teralokasi, ${totalNewConflicts} bentrok.`);
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
      setTimeout(() => {
        drawBookletConnectors();
      }, 150);
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

  function addSlotRow() {
    if (!activeDay) return;
    const newCells: EnrichedTimetableCell[] = tatamis.map((t) => ({
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
    }));

    activeDay.rows = [
      ...activeDay.rows,
      {
        id: 0,
        day_id: activeDay.id,
        order: activeDay.rows.length,
        row_type: "slot",
        time_label: `Sesi ${activeDay.rows.length + 1}`,
        label_text: "",
        cells: newCells,
        conflicts: [],
      },
    ];
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

    const cell = targetRow.cells.find((c) => c.tatami_id === targetTatamiId);
    if (cell) {
      cell.nomor_tanding_id = cat.id;
      cell.nomor_tanding_nama = cat.nama;
      cell.atlet_count = cat.atlet_count;
      cell.matches = cat.matches;
      cell.discipline = cat.discipline;
      cell.est_duration_minutes = cat.est_duration_minutes;
      cell.is_festival = cat.is_festival;
      cell.is_break = false;
      cell.custom_text = "";
      cell.progress_status = cat.progress_status;
      cell.progress_pct = cat.progress_pct;
      markDirty();
      notify(`Kategori ditempatkan di Tatami.`);
    }
  }

  function clearCell(rowIdx: number, tatamiId: number) {
    if (!activeDay) return;
    const cell = activeDay.rows[rowIdx]?.cells.find((c) => c.tatami_id === tatamiId);
    if (!cell) return;

    cell.nomor_tanding_id = null;
    cell.nomor_tanding_nama = null;
    cell.custom_text = "";
    cell.atlet_count = 0;
    cell.matches = 0;
    cell.est_duration_minutes = 0;
    cell.is_festival = false;
    cell.is_break = false;
    cell.progress_status = "belum_disentuh";
    markDirty();
  }

  // --- Save Timetable Transaction ---
  async function handleSaveTimetable() {
    if (!eventId) return;
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
      notify("Jadwal pertandingan berhasil disimpan!");
      await loadTimetable();
    } catch (err: any) {
      console.error("Gagal menyimpan jadwal:", err);
      notify(`Gagal menyimpan jadwal: ${err}`);
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
    if (printMode === "booklet") {
      document.body.classList.add("is-printing");
      void document.body.offsetHeight;
      setTimeout(() => {
        drawBookletConnectors();
        setTimeout(() => {
          drawBookletConnectors();
          window.print();
          setTimeout(() => {
            document.body.classList.remove("is-printing");
            drawBookletConnectors();
          }, 500);
        }, 120);
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

  $effect(() => {
    // Re-draw bracket connectors when prune toggle changes
    if (printMode === "booklet" && tatamiBookletData) {
      const _ = bookletPruneEmptyRounds;
      setTimeout(() => {
        drawBookletConnectors();
      }, 80);
    }
  });

  onMount(() => {
    loadTimetable();
    const handleBeforePrint = () => {
      if (printMode === "booklet") {
        document.body.classList.add("is-printing");
        void document.body.offsetHeight;
        drawBookletConnectors();
      }
    };
    const handleAfterPrint = () => {
      if (printMode === "booklet") {
        document.body.classList.remove("is-printing");
        drawBookletConnectors();
      }
    };
    window.addEventListener("beforeprint", handleBeforePrint);
    window.addEventListener("afterprint", handleAfterPrint);
    const handleResize = () => {
      if (printMode === "booklet") {
        drawBookletConnectors();
      }
    };
    window.addEventListener("resize", handleResize);
    return () => {
      window.removeEventListener("beforeprint", handleBeforePrint);
      window.removeEventListener("afterprint", handleAfterPrint);
      window.removeEventListener("resize", handleResize);
    };
  });
</script>

{#if printMode === "none"}
<div class="h-full flex flex-col p-6 select-none bg-[#14161b] text-zinc-200 overflow-hidden font-sans relative">
  <!-- Top Live Tatamis Strip (Matching Ambrilindo Local Live Banner) -->
  {#if summary.tatamis_live && summary.tatamis_live.length > 0}
    <div class="mb-3 px-3.5 py-2 bg-[#181a20] border border-white/[0.08] rounded-xl flex items-center justify-between shrink-0 overflow-x-auto gap-3 scrollbar-none">
      <div class="flex items-center space-x-2 shrink-0">
        <span class="w-2 h-2 rounded-full bg-emerald-400 animate-pulse"></span>
        <span class="text-[10px] font-mono font-bold uppercase tracking-wider text-zinc-400">Live Tatami:</span>
      </div>

      <div class="flex items-center space-x-4 overflow-x-auto text-xs scrollbar-none">
        {#each summary.tatamis_live as tLive}
          <div class="flex items-center space-x-2 shrink-0 px-2.5 py-1 rounded-lg border {tLive.is_running ? 'bg-emerald-500/[0.08] border-emerald-500/30 text-emerald-300' : 'bg-white/[0.02] border-white/[0.06] text-zinc-400'}">
            <span class="font-bold">Tatami {tLive.tatami_number}:</span>
            {#if tLive.is_running}
              <span class="font-semibold text-white">{tLive.category_name}</span>
              <span class="text-[10px] font-mono text-zinc-400">({tLive.atlet1_name || 'Aka'} vs {tLive.atlet2_name || 'Ao'})</span>
            {:else}
              <span class="text-[11px] font-mono opacity-60">Standby</span>
            {/if}
          </div>
        {/each}
      </div>
    </div>
  {/if}

  <!-- Header Bar -->
  <div class="flex items-center justify-between pb-4 border-b border-white/[0.08] shrink-0">
    <div class="flex items-center space-x-2.5">
      <h2 class="text-xl font-black text-white tracking-tight uppercase">
        Jadwal Pertandingan
      </h2>
      <span class="px-2 py-0.5 rounded text-[11px] font-mono font-bold bg-white/[0.06] text-zinc-300 border border-white/[0.1]">
        {days.length} HARI • {totalSlotsCount} SESI
      </span>
      {#if isDirty}
        <span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-amber-500/20 text-amber-300 border border-amber-500/40 animate-pulse">
          BELUM DISIMPAN
        </span>
      {/if}
    </div>

    <!-- Actions Toolbar (Clean, Intuitive Edit Mode & Operational Read Mode) -->
    <div class="flex items-center space-x-2">
      <!-- Kop Surat -->
      <button
        onclick={() => (kopModalOpen = true)}
        class="h-9 px-3 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-200 hover:text-white font-semibold text-xs rounded-xl border border-white/[0.1] transition-all flex items-center space-x-1.5"
        title="Pengaturan Kop Surat & Info Panitia"
      >
        <svg class="w-3.5 h-3.5 text-zinc-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12h6m-6 4h6m2 5H7a2 2 0 01-2-2V5a2 2 0 012-2h5.586a1 1 0 01.707.293l5.414 5.414a1 1 0 01.293.707V19a2 2 0 01-2 2z"/></svg>
        <span>Kop Surat</span>
      </button>

      <!-- Cetak Roster 1 Halaman -->
      <button
        onclick={() => (printMode = "simple_roster")}
        class="h-9 px-3 bg-white/[0.04] hover:bg-white/[0.08] text-sky-300 hover:text-white font-semibold text-xs rounded-xl border border-sky-500/20 transition-all flex items-center space-x-1.5"
        title="Cetak Run Sheet Ringkas 1 Halaman (A4/F4)"
      >
        <svg class="w-3.5 h-3.5 text-sky-400" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z"/></svg>
        <span>Cetak 1 Halaman</span>
      </button>

      <!-- Mode Edit Toggle Button (Intuitive & Safe) -->
      <button
        onclick={() => (isEditMode = !isEditMode)}
        class="h-9 px-3.5 rounded-xl text-xs font-bold transition-all flex items-center space-x-1.5 {isEditMode ? 'bg-amber-500/20 text-amber-300 border border-amber-500/50 shadow-inner' : 'bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 border border-white/[0.1]'}"
        title={isEditMode ? 'Selesai & Kunci Jadwal' : 'Aktifkan Mode Edit untuk mengubah jadwal'}
      >
        <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
        </svg>
        <span>{isEditMode ? 'Mode Edit: Aktif' : 'Mode Edit'}</span>
      </button>

      {#if isEditMode}
        <!-- Auto-Jadwal Cerdas (Only accessible in Edit Mode) -->
        <button
          onclick={openAutoScheduleModal}
          class="h-9 px-3.5 bg-gradient-to-r from-amber-500 to-amber-600 hover:from-amber-600 hover:to-amber-700 text-black font-black text-xs rounded-xl shadow-md transition-all active:scale-95 flex items-center space-x-1.5"
        >
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M13 10V3L4 14h7v7l9-11h-7z"/></svg>
          <span>Auto-Jadwal Cerdas</span>
        </button>
      {/if}

      <!-- Simpan Perubahan (Muncul jika ada perubahan atau saat Mode Edit aktif) -->
      {#if isDirty || isEditMode}
        <button
          onclick={handleSaveTimetable}
          disabled={isSaving}
          class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md transition-all active:scale-95 flex items-center space-x-1.5 disabled:opacity-50 animate-in fade-in"
        >
          {#if isSaving}
            <span class="w-3.5 h-3.5 border-2 border-black border-t-transparent rounded-full animate-spin"></span>
            <span>Menyimpan...</span>
          {:else}
            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M5 13l4 4L19 7" />
            </svg>
            <span>Simpan Perubahan</span>
          {/if}
        </button>
      {/if}

      {#if isEditMode}
        <!-- Opsi Tambahan / More Actions (···) Dropdown (Only in Edit Mode) -->
        <div class="relative">
          <button
            onclick={() => (optionsMenuOpen = !optionsMenuOpen)}
            class="h-9 w-9 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white font-bold text-sm rounded-xl border border-white/[0.1] transition-all flex items-center justify-center"
            title="Opsi Jadwal Lainnya"
          >
            <svg class="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 12h.01M12 12h.01M19 12h.01M6 12a1 1 0 11-2 0 1 1 0 012 0zm7 0a1 1 0 11-2 0 1 1 0 012 0zm7 0a1 1 0 11-2 0 1 1 0 012 0z" />
            </svg>
          </button>

          {#if optionsMenuOpen}
            <!-- Backdrop overlay to dismiss dropdown -->
            <div
              class="fixed inset-0 z-30"
              onclick={() => (optionsMenuOpen = false)}
              aria-hidden="true"
            ></div>

            <div class="absolute right-0 mt-2 w-56 bg-[#181a20] border border-white/10 rounded-xl shadow-2xl py-1.5 z-40 animate-in fade-in zoom-in-95">
              <button
                onclick={() => {
                  optionsMenuOpen = false;
                  promptClearDaySlots();
                }}
                class="w-full px-3.5 py-2 text-left text-xs font-semibold text-zinc-300 hover:text-white hover:bg-white/[0.06] transition flex items-center space-x-2"
              >
                <svg class="w-3.5 h-3.5 text-zinc-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
                </svg>
                <span>Kosongkan Sesi Hari Ini</span>
              </button>

              <div class="my-1 border-t border-white/[0.08]"></div>

              <button
                onclick={() => {
                  optionsMenuOpen = false;
                  promptResetAllTimetable();
                }}
                class="w-full px-3.5 py-2 text-left text-xs font-semibold text-rose-400 hover:text-rose-200 hover:bg-rose-500/10 transition flex items-center space-x-2"
              >
                <svg class="w-3.5 h-3.5 text-rose-400" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                </svg>
                <span>Reset Seluruh Jadwal</span>
              </button>
            </div>
          {/if}
        </div>
      {/if}
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

  <!-- KPI Stats Cards (5-column responsive strip) -->
  <div class="grid grid-cols-2 sm:grid-cols-3 md:grid-cols-5 gap-3 my-4 shrink-0">
    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold tracking-wider text-zinc-400 uppercase">Slot Waktu</span>
      <div class="flex items-baseline justify-between mt-1">
        <span class="text-2xl font-black font-mono text-white">{totalSlotsCount}</span>
        <span class="text-[10px] font-mono text-zinc-500">sesi</span>
      </div>
    </div>

    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold tracking-wider text-sky-400 uppercase">Total Pertandingan</span>
      <div class="flex items-baseline justify-between mt-1">
        <span class="text-2xl font-black font-mono text-white">{summary.total_matches}</span>
        <span class="text-[10px] font-mono text-zinc-500">match</span>
      </div>
    </div>

    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold tracking-wider text-emerald-400 uppercase">Match Selesai</span>
      <div class="flex items-baseline justify-between mt-1">
        <span class="text-2xl font-black font-mono text-white">{summary.finished_matches}</span>
        <span class="text-[10px] font-mono text-emerald-400/80">{summary.overall_pct}%</span>
      </div>
    </div>

    <div class="bg-[#181a20] border border-white/[0.08] rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold tracking-wider text-amber-400 uppercase">Arena Tatami</span>
      <div class="flex items-baseline justify-between mt-1">
        <span class="text-2xl font-black font-mono text-white">{tatamis.length}</span>
        <span class="text-[10px] font-mono text-zinc-500">tatami</span>
      </div>
    </div>

    <div class="bg-[#181a20] border {summary.total_conflicts > 0 ? 'border-rose-500/50 bg-rose-500/[0.05]' : 'border-white/[0.08]'} rounded-xl p-3 flex flex-col justify-between">
      <span class="text-[10px] font-bold tracking-wider {summary.total_conflicts > 0 ? 'text-rose-400' : 'text-zinc-400'} uppercase">Bentrok Atlet</span>
      <div class="flex items-baseline justify-between mt-1">
        <span class="text-2xl font-black font-mono {summary.total_conflicts > 0 ? 'text-rose-400 animate-pulse' : 'text-white'}">{summary.total_conflicts}</span>
        <span class="text-[10px] font-mono {summary.total_conflicts > 0 ? 'text-rose-400' : 'text-zinc-500'}">konflik</span>
      </div>
    </div>
  </div>

  <!-- Main Body: Category Pool Sidebar + Timetable Grid -->
  <div class="flex-1 flex min-h-0 gap-3 overflow-hidden">
    <!-- 1. Left Category Pool Sidebar (Collapsible) -->
    {#if isSidebarOpen}
      <aside class="w-80 bg-[#181a20] border border-white/[0.08] rounded-2xl flex flex-col shrink-0 p-3 overflow-hidden shadow-lg">
        <div class="flex items-center justify-between pb-2 border-b border-white/[0.06] shrink-0">
          <div>
            <h4 class="text-xs font-black uppercase text-white tracking-wide">Pool Kategori ({categories.length})</h4>
            <span class="text-[10px] font-mono {unscheduledTournamentCount === 0 ? 'text-emerald-400' : 'text-amber-400'}">
              {unscheduledTournamentCount === 0 ? 'Semua Kategori Terjadwal' : `${unscheduledTournamentCount} Kategori Belum Terjadwal`}
            </span>
          </div>
          <button
            onclick={() => (isSidebarOpen = false)}
            class="text-zinc-400 hover:text-white p-1 rounded-lg hover:bg-white/[0.06] transition flex items-center space-x-1"
            title="Tutup Pool Kategori"
          >
            <span class="text-[10px] font-mono">Tutup</span>
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
            placeholder="Cari kategori..."
            class="w-full bg-[#14161b] text-xs text-white px-2.5 py-1.5 rounded-lg border border-white/[0.08] focus:border-white/30 outline-none"
          />

          <select
            bind:value={poolGroupFilter}
            class="w-full bg-[#14161b] text-xs text-zinc-300 px-2 py-1.5 rounded-lg border border-white/[0.08] focus:border-white/30 outline-none"
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
          {#each poolFilteredCategories as cat}
            {@const isAssigned = assignedCategoryIds.has(cat.id)}
            <div
              class="p-2 rounded-lg border transition {isAssigned ? 'bg-[#14161b]/40 border-white/[0.04] opacity-50' : 'bg-[#14161b] border-white/[0.08] hover:border-white/20'}"
            >
              <!-- Row 1: Discipline Badge, Title & Tatami Assign Buttons -->
              <div class="flex items-center justify-between gap-1.5">
                <div class="flex items-center space-x-1.5 min-w-0">
                  <span class="px-1 py-0.2 rounded text-[8.5px] font-mono font-bold uppercase shrink-0 {cat.discipline === 'kata' ? 'bg-purple-500/20 text-purple-300' : 'bg-sky-500/20 text-sky-300'}">
                    {cat.discipline}
                  </span>
                  <span class="text-[11px] font-bold text-white truncate" title={cat.nama}>
                    {cat.nama}
                  </span>
                </div>

                {#if isAssigned}
                  <span class="text-[8.5px] font-mono font-bold text-emerald-400 shrink-0">✓ Terjadwal</span>
                {:else}
                  <div class="flex items-center space-x-0.5 shrink-0">
                    {#each tatamis as t}
                      <button
                        onclick={() => quickAssignFromPool(cat, t.id)}
                        class="px-1.5 py-0.5 rounded bg-white/[0.06] hover:bg-white text-zinc-300 hover:text-black text-[8.5px] font-bold font-mono transition"
                        title="Tempatkan di Tatami {t.tatami_number}"
                      >
                        +T{t.tatami_number}
                      </button>
                    {/each}
                  </div>
                {/if}
              </div>

              <!-- Row 2: Age group & match counts -->
              <div class="flex items-center justify-between mt-1 pt-1 border-t border-white/[0.03] text-[9.5px] font-mono text-zinc-400">
                <span class="truncate">{cat.age_group}</span>
                <span class="shrink-0">{cat.atlet_count} Atlet • {cat.matches} Match</span>
              </div>
            </div>
          {/each}
          {#if poolFilteredCategories.length === 0}
            <div class="py-10 text-center text-zinc-500 text-xs">
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
        title="Buka Pool Kategori"
      >
        <svg class="w-3.5 h-3.5 text-zinc-400 group-hover:text-white transition" fill="none" viewBox="0 0 24 24" stroke="currentColor">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 5l7 7-7 7" />
        </svg>
        <span class="text-[10px] font-mono font-bold uppercase tracking-wider [writing-mode:vertical-rl] rotate-180">Pool Kategori</span>
        {#if unscheduledTournamentCount > 0}
          <span class="px-1 py-0.5 rounded text-[8.5px] font-mono font-bold bg-amber-500/20 text-amber-300">
            {unscheduledTournamentCount}
          </span>
        {/if}
      </button>
    {/if}

    <!-- 2. Right Timetable Workspace Grid -->
    <div class="flex-1 flex flex-col min-w-0 overflow-hidden">
      <!-- Days Switcher, Quick Sesi Buttons & Tatami Controls -->
      <div class="flex flex-wrap items-center justify-between gap-2 pb-3 shrink-0">
        <!-- Day Tabs -->
        <div class="flex items-center space-x-1.5 overflow-x-auto scrollbar-none">
          {#each days as day, idx}
            <button
              onclick={() => (activeDayIndex = idx)}
              class="px-3.5 py-1.5 rounded-xl text-xs font-semibold transition border flex items-center space-x-2 {activeDayIndex === idx ? 'bg-white text-black border-white shadow-sm' : 'bg-white/[0.04] text-zinc-300 border-white/[0.08] hover:bg-white/[0.08]'}"
            >
              <span>{day.label}</span>
              <span class="text-[10px] font-mono opacity-60">({day.rows.length} row)</span>
            </button>
          {/each}

          {#if isEditMode}
            <button
              onclick={handleAddDay}
              class="h-8 px-2.5 bg-white/[0.03] hover:bg-white/[0.06] text-zinc-400 hover:text-white text-xs font-semibold rounded-xl border border-dashed border-white/[0.15] transition flex items-center space-x-1 cursor-pointer"
            >
              <span>+ Tambah Hari</span>
            </button>

            {#if days.length > 1}
              <button
                onclick={promptDeleteCurrentDay}
                disabled={isDeletingDay}
                class="h-8 px-2 bg-rose-500/[0.06] hover:bg-rose-500/20 text-rose-400 text-xs font-semibold rounded-xl border border-rose-500/20 transition flex items-center space-x-1 cursor-pointer"
                title="Hapus Hari Ini"
              >
                <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16" />
                </svg>
                <span>Hapus Hari</span>
              </button>
            {/if}
          {/if}
        </div>

        <!-- Quick Session & Day Toolbar -->
        <div class="flex items-center space-x-2">
          {#if isEditMode}
            <!-- Kosongkan Sesi Hari Ini -->
            <button
              onclick={promptClearDaySlots}
              class="h-8 px-2.5 bg-white/[0.04] hover:bg-rose-500/10 text-zinc-400 hover:text-rose-300 text-xs font-semibold rounded-xl border border-white/[0.08] hover:border-rose-500/30 transition flex items-center space-x-1 cursor-pointer"
              title="Kosongkan Semua Sesi Pertandingan pada Hari Ini"
            >
              <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 4v5h.582m15.356 2A8.001 8.001 0 004.582 9m0 0H9m11 11v-5h-.581m0 0a8.003 8.003 0 01-15.357-2m15.357 2H15" />
              </svg>
              <span>Kosongkan Hari</span>
            </button>

            <!-- + Sesi Festival Prompt -->
            <button
              onclick={() => (festivalPromptOpen = true)}
              class="h-8 px-2.5 bg-white/[0.04] hover:bg-white/[0.08] text-amber-300 text-xs font-semibold rounded-xl border border-amber-500/20 transition flex items-center space-x-1.5 cursor-pointer"
              title="Tambah Sesi Festival Serentak di Seluruh Tatami"
            >
              <svg class="w-3.5 h-3.5 text-amber-300" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M3 21v-4m0 0V5a2 2 0 012-2h6.5l1 1H21l-3 6 3 6h-8.5l-1-1H5a2 2 0 00-2 2zm9-13.5V9" />
              </svg>
              <span>+ Sesi Festival</span>
            </button>

            <!-- + ISHOMA Prompt -->
            <button
              onclick={() => (ishomaPromptOpen = true)}
              class="h-8 px-2.5 bg-white/[0.04] hover:bg-white/[0.08] text-amber-200 text-xs font-semibold rounded-xl border border-amber-500/20 transition flex items-center space-x-1.5 cursor-pointer"
              title="Tambah Sesi ISHOMA Serentak di Seluruh Tatami"
            >
              <svg class="w-3.5 h-3.5 text-amber-200" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 8v4l3 3m6-3a9 9 0 11-18 0 9 9 0 0118 0z" />
              </svg>
              <span>+ ISHOMA</span>
            </button>

            <!-- + Tambah Tatami -->
            <button
              onclick={handleAddTatami}
              class="h-8 px-2.5 bg-white/[0.04] hover:bg-white/[0.08] text-zinc-300 hover:text-white text-xs font-semibold rounded-xl border border-white/10 transition cursor-pointer"
              title="Tambah Tatami Baru ke Turnamen"
            >
              + Tatami
            </button>

            <!-- Editable Time & Date in Edit Mode -->
            <div class="flex items-center space-x-1.5 bg-[#181a20] px-2.5 py-1 rounded-xl border border-white/[0.08]">
              <span class="text-[10px] font-mono text-zinc-400 uppercase">Mulai:</span>
              <input
                type="time"
                bind:value={startTimeStr}
                class="bg-transparent text-xs text-white outline-none font-mono cursor-pointer"
              />
            </div>

            {#if activeDay}
              <input
                type="date"
                bind:value={activeDay.tanggal}
                onchange={markDirty}
                class="bg-[#181a20] border border-white/[0.08] text-xs text-white px-2 py-1 rounded-xl outline-none font-mono cursor-pointer"
              />
            {/if}
          {:else}
            <!-- Clean Read-Only Time & Date Info Badge in Mode Baca -->
            <div class="flex items-center space-x-2 text-xs font-mono text-zinc-400 bg-[#181a20] px-3 py-1.5 rounded-xl border border-white/[0.08]">
              <span>Mulai: <strong class="text-white">{startTimeStr}</strong></span>
              {#if activeDay?.tanggal}
                <span class="text-zinc-600">•</span>
                <span>Tanggal: <strong class="text-white">{activeDay.tanggal}</strong></span>
              {/if}
            </div>
          {/if}
        </div>
      </div>

      <!-- Timetable Grid Table Container -->
      <div class="flex-1 overflow-y-auto min-h-0 rounded-2xl border border-white/[0.08] bg-[#181a20] shadow-lg flex flex-col scrollbar-thin">
        {#if isLoading}
          <div class="flex-1 flex items-center justify-center py-20 text-zinc-500">
            <div class="w-5 h-5 border-2 border-white/20 border-t-white rounded-full animate-spin mr-3"></div>
            <span class="text-xs font-mono">Memuat jadwal pertandingan...</span>
          </div>
        {:else if !activeDay || activeDay.rows.length === 0}
          <div class="flex-1 flex flex-col items-center justify-center py-20 text-zinc-500">
            <span class="text-sm font-semibold text-zinc-400">Belum Ada Slot Pertandingan di Hari Ini</span>
            <div class="flex items-center space-x-2 mt-3">
              <button
                onclick={openAutoScheduleModal}
                class="px-4 py-2 bg-gradient-to-r from-amber-500 to-amber-600 hover:from-amber-600 hover:to-amber-700 text-black font-black text-xs rounded-xl shadow-md transition flex items-center space-x-1.5"
              >
                <svg class="w-3.5 h-3.5 text-black" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
                </svg>
                <span>Auto-Jadwal Cerdas</span>
              </button>
              <button
                onclick={addSlotRow}
                class="px-3.5 py-2 bg-white text-black font-bold text-xs rounded-xl shadow-md transition hover:bg-zinc-200"
              >
                + Tambah Slot Manual
              </button>
            </div>
          </div>
        {:else}
          <table class="w-full text-left border-collapse">
            <thead class="bg-[#111215] text-zinc-400 font-mono text-[11px] uppercase tracking-wider sticky top-0 z-10 border-b border-white/[0.08]">
              <tr>
                <th class="py-3 px-4 w-40 font-semibold">Waktu / Sesi</th>
                {#each tatamis as tatami}
                  {@const tMeta = tatamiTimelines[tatami.id]}
                  <th class="py-2.5 px-3 font-semibold border-l border-white/[0.06]">
                    <div class="flex items-center justify-between">
                      <div>
                        <span class="text-white font-bold">Tatami {tatami.tatami_number}</span>
                        {#if tMeta}
                          <span class="block text-[10px] font-mono text-zinc-400 font-normal">
                            {tMeta.totalMatches} Partai • ~{tMeta.finishTime}
                          </span>
                        {/if}
                      </div>

                      <!-- Per Tatami Actions (Buku Tatami, Delete Tatami) -->
                      <div class="flex items-center space-x-1">
                        <button
                          onclick={() => openTatamiBooklet(tatami)}
                          disabled={tatamiBookletLoading && printSelectedTatami?.id === tatami.id}
                          class="px-2 py-0.5 rounded bg-amber-500/10 hover:bg-amber-500/20 text-amber-300 text-[9px] font-bold border border-amber-500/20 flex items-center space-x-1 transition disabled:opacity-50"
                          title="Cetak Buku Tatami LO (Sampul + Roster + Bagan Lengkap)"
                        >
                          {#if tatamiBookletLoading && printSelectedTatami?.id === tatami.id}
                            <span class="w-2.5 h-2.5 border border-amber-400 border-t-transparent rounded-full animate-spin"></span>
                          {:else}
                            <svg class="w-2.5 h-2.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 6.253v13m0-13C10.832 5.477 9.246 5 7.5 5S4.168 5.477 3 6.253v13C4.168 18.477 5.754 18 7.5 18s3.332.477 4.5 1.253m0-13C13.168 5.477 14.754 5 16.5 5c1.747 0 3.332.477 4.5 1.253v13C19.832 18.477 18.247 18 16.5 18c-1.746 0-3.332.477-4.5 1.253" />
                            </svg>
                          {/if}
                          <span>Buku</span>
                        </button>

                        {#if isEditMode && tatamis.length > 1}
                          <button
                            onclick={() => promptRemoveTatami(tatami.id)}
                            class="text-zinc-500 hover:text-rose-400 p-0.5 rounded hover:bg-rose-500/10 transition cursor-pointer"
                            title="Hapus Tatami ini"
                          >
                            <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                            </svg>
                          </button>
                        {/if}
                      </div>
                    </div>
                  </th>
                {/each}
                {#if isEditMode}
                  <th class="py-3 px-3 w-20 text-center font-semibold border-l border-white/[0.06]">Aksi</th>
                {/if}
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
                        <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-amber-500/20 text-amber-300">BREAK</span>
                        <input
                          type="text"
                          bind:value={row.label_text}
                          onchange={markDirty}
                          placeholder="Keterangan Break / Upacara"
                          class="flex-1 bg-[#14161b] text-amber-200 font-bold text-xs px-3 py-1.5 rounded-lg border border-amber-500/20 outline-none uppercase"
                        />
                      </div>
                    </td>
                    {#if isEditMode}
                      <td class="py-3 px-3 text-center align-middle border-l border-white/[0.06]">
                        <div class="flex items-center justify-center space-x-1 opacity-60 group-hover:opacity-100 transition">
                          <button onclick={() => moveRow(rIdx, "up")} disabled={rIdx === 0} class="p-1 text-zinc-400 hover:text-white disabled:opacity-20" title="Geser ke Atas">
                            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 15l7-7 7 7"/></svg>
                          </button>
                          <button onclick={() => moveRow(rIdx, "down")} disabled={rIdx === activeDay.rows.length - 1} class="p-1 text-zinc-400 hover:text-white disabled:opacity-20" title="Geser ke Bawah">
                            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/></svg>
                          </button>
                          <button onclick={() => deleteRow(rIdx)} class="p-1 text-zinc-500 hover:text-rose-400" title="Hapus Baris">
                            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
                          </button>
                        </div>
                      </td>
                    {/if}
                  </tr>
                {:else}
                  <!-- Slot Row -->
                  <tr class="hover:bg-white/[0.02] transition group {row.conflicts.length > 0 ? 'bg-rose-500/[0.03]' : ''}">
                    <!-- Time Column -->
                    <td class="py-3 px-4 align-top w-40">
                      <div class="space-y-1">
                        <input
                          type="text"
                          bind:value={row.time_label}
                          onchange={markDirty}
                          class="w-full bg-[#14161b] text-zinc-200 font-mono text-xs px-2.5 py-1.5 rounded-lg border border-white/[0.08] focus:border-white/30 outline-none"
                        />
                        <span class="text-[10px] font-mono text-zinc-500 block px-0.5">
                          Sesi #{rIdx + 1}
                        </span>
                        {#if row.conflicts.length > 0}
                          <button
                            onclick={() => {
                              activeConflictRow = row;
                              conflictModalOpen = true;
                            }}
                            class="w-full mt-1 px-2 py-1 bg-rose-500/10 border border-rose-500/40 rounded-md text-[10px] font-bold text-rose-300 text-left flex items-center space-x-1 hover:bg-rose-500/20 transition cursor-pointer"
                          >
                            <svg class="w-3 h-3 text-rose-400 shrink-0" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z"/>
                            </svg>
                            <span class="truncate">{row.conflicts.length} Bentrok</span>
                          </button>
                        {/if}
                      </div>
                    </td>

                    <!-- Tatami Cells -->
                    {#each tatamis as tatami}
                      {@const cell = row.cells.find((c) => c.tatami_id === tatami.id)}
                      <td class="py-2.5 px-3 align-top border-l border-white/[0.06]">
                        {#if cell && cell.nomor_tanding_id}
                          <!-- Category Card -->
                          <div class="bg-[#14161b] border border-white/[0.1] rounded-xl p-2.5 relative group/cell hover:border-white/20 transition shadow-sm">
                            <div class="flex items-center justify-between mb-1">
                              <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold uppercase {cell.discipline === 'kata' ? 'bg-purple-500/20 text-purple-300' : 'bg-sky-500/20 text-sky-300'}">
                                {cell.discipline}
                              </span>

                              <div class="flex items-center space-x-1 opacity-0 group-hover/cell:opacity-100 transition">
                                <button
                                  onclick={() => clearCell(rIdx, tatami.id)}
                                  class="text-[10px] text-zinc-500 hover:text-rose-400 p-0.5 rounded hover:bg-rose-500/10 transition"
                                  title="Hapus Dari Tatami"
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

                            <div class="flex items-center justify-between mt-2 pt-1.5 border-t border-white/[0.04] text-[10px] font-mono text-zinc-400">
                              <span>{cell.atlet_count} Atlet • {cell.matches} Match</span>
                              <span class="text-zinc-500">~{cell.est_duration_minutes}m</span>
                            </div>

                            {#if cell.progress_status === 'selesai'}
                              <div class="mt-1 flex items-center space-x-1 text-[9px] font-mono text-emerald-400">
                                <span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
                                <span>SELESAI</span>
                              </div>
                            {:else if cell.progress_status === 'running'}
                              <div class="mt-1 flex items-center space-x-1 text-[9px] font-mono text-amber-400 animate-pulse">
                                <span class="w-1.5 h-1.5 rounded-full bg-amber-400"></span>
                                <span>SEDANG MAIN</span>
                              </div>
                            {/if}
                          </div>
                        {:else if cell && cell.custom_text}
                          <!-- Custom Text Card -->
                          <div class="bg-[#14161b] border border-amber-500/20 rounded-xl p-2.5 relative group/cell hover:border-amber-500/40 transition">
                            <div class="flex items-center justify-between mb-1">
                              <span class="px-1.5 py-0.5 rounded text-[9px] font-mono font-bold bg-amber-500/20 text-amber-300">
                                {cell.is_festival ? 'FESTIVAL' : cell.is_break ? 'BREAK' : 'CATATAN'}
                              </span>
                              <button
                                onclick={() => clearCell(rIdx, tatami.id)}
                                class="text-[10px] text-zinc-500 hover:text-rose-400 p-0.5 opacity-0 group-hover/cell:opacity-100 transition"
                                title="Hapus Dari Tatami"
                              >
                                <svg class="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                                  <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
                                </svg>
                              </button>
                            </div>
                            <div class="font-bold text-amber-200 text-xs line-clamp-2">
                              {cell.custom_text}
                            </div>
                          </div>
                        {:else}
                          <!-- Empty Placeholder -->
                          {#if isEditMode}
                            <div class="w-full h-18 rounded-xl border border-dashed border-white/[0.08] hover:border-white/[0.2] transition flex items-center justify-center text-zinc-500 text-[11px] font-mono">
                              <span>Pilih dari Pool Kiri</span>
                            </div>
                          {:else}
                            <div class="w-full h-12 rounded-xl border border-white/[0.04] bg-white/[0.01] flex items-center justify-center text-zinc-600 font-mono text-xs">
                              —
                            </div>
                          {/if}
                        {/if}
                      </td>
                    {/each}

                    <!-- Row Actions (Only in Edit Mode) -->
                    {#if isEditMode}
                      <td class="py-3 px-3 text-center align-top border-l border-white/[0.06]">
                        <div class="flex items-center justify-center space-x-1 opacity-60 group-hover:opacity-100 transition">
                          <button onclick={() => moveRow(rIdx, "up")} disabled={rIdx === 0} class="p-1 text-zinc-400 hover:text-white disabled:opacity-20" title="Geser ke Atas">
                            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 15l7-7 7 7"/></svg>
                          </button>
                          <button onclick={() => moveRow(rIdx, "down")} disabled={rIdx === activeDay.rows.length - 1} class="p-1 text-zinc-400 hover:text-white disabled:opacity-20" title="Geser ke Bawah">
                            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 9l-7 7-7-7"/></svg>
                          </button>
                          <button onclick={() => deleteRow(rIdx)} class="p-1 text-zinc-500 hover:text-rose-400" title="Hapus Baris">
                            <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12"/></svg>
                          </button>
                        </div>
                      </td>
                    {/if}
                  </tr>
                {/if}
              {/each}
            </tbody>
          </table>
        {/if}
      </div>
    </div>
  </div>
</div>

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
            bind:value={autoStartTime}
            class="w-full bg-[#14161b] text-white text-xs px-3 py-2 rounded-xl border border-white/10 outline-none"
          />
        </div>

        <!-- Overwrite Schedule Option -->
        <div class="p-3 rounded-xl bg-[#14161b] border border-white/[0.06]">
          <label class="flex items-start space-x-2.5 cursor-pointer">
            <input type="checkbox" bind:checked={autoOverwriteExisting} class="rounded accent-amber-500 mt-0.5" />
            <div>
              <span class="text-xs font-semibold text-white block">Timpa / Reset Jadwal Hari Ini</span>
              <span class="text-[10px] text-zinc-400 font-mono block mt-0.5">Jadwalkan ulang seluruh kategori dari awal (menimpa slot kategori yang sudah ada).</span>
            </div>
          </label>
        </div>

        <!-- Festival Option -->
        <div class="p-3 rounded-xl bg-[#14161b] border border-white/[0.06] space-y-2">
          <label class="flex items-center space-x-2 cursor-pointer">
            <input type="checkbox" bind:checked={autoIncludeFestival} class="rounded accent-amber-500" />
            <span class="text-xs font-semibold text-white">Sertakan Sesi Festival Serentak di Tatami</span>
          </label>
          {#if autoIncludeFestival}
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
              <span class="text-xs font-semibold text-white">Partai Sama Rata Tiap Arena</span>
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
            <option value="wkf_standard">Standar WKF: Usia Rendah ke Besar, Kata Perorangan/Beregu ke Kumite Perorangan/Beregu</option>
            <option value="kata_first">Kata Dahulu, Lalu Kumite</option>
            <option value="age_asc">Urut Usia: Usia Dini ke Senior</option>
            <option value="age_desc">Urut Usia: Senior ke Usia Dini</option>
            <option value="balanced">Beban Merata Antar Arena</option>
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
    <div class="no-print mb-4 pb-3 border-b border-white/10 flex items-center justify-between text-zinc-200 max-w-5xl mx-auto w-full">
      <div class="flex items-center space-x-3">
        <h3 class="text-sm font-bold text-white uppercase tracking-tight">Pratinjau Dokumen Siap Cetak</h3>
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

        {#if printMode === "booklet"}
          <!-- Option: Ringkas Babak BYE vs Bagan Penuh -->
          <div class="flex items-center bg-white/[0.04] p-1 rounded-xl border border-white/10 space-x-1">
            <button
              type="button"
              onclick={() => (bookletPruneEmptyRounds = true)}
              class="px-2.5 py-1 rounded-lg text-xs font-bold transition flex items-center space-x-1.5 {bookletPruneEmptyRounds ? 'bg-white text-black shadow-sm' : 'text-zinc-400 hover:text-white hover:bg-white/[0.08]'}"
              title="Sembunyikan babak awal yang 100% BYE (misal langsung mulai dari Semi Final)"
            >
              <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M13 10V3L4 14h7v7l9-11h-7z" />
              </svg>
              <span>Ringkas (Tanpa BYE)</span>
            </button>
            <button
              type="button"
              onclick={() => (bookletPruneEmptyRounds = false)}
              class="px-2.5 py-1 rounded-lg text-xs font-bold transition flex items-center space-x-1.5 {!bookletPruneEmptyRounds ? 'bg-white text-black shadow-sm' : 'text-zinc-400 hover:text-white hover:bg-white/[0.08]'}"
              title="Tampilkan seluruh babak bagan penuh termasuk babak yang seluruhnya BYE"
            >
              <svg class="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16" />
              </svg>
              <span>Bagan Penuh</span>
            </button>
          </div>
        {/if}

        <button
          onclick={triggerPrint}
          class="h-9 px-4 bg-sky-500 hover:bg-sky-600 text-white font-bold text-xs rounded-xl shadow-md flex items-center space-x-1.5 transition active:scale-95"
        >
          <svg class="w-3.5 h-3.5 text-white" fill="none" viewBox="0 0 24 24" stroke="currentColor">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
          </svg>
          <span>Cetak / Simpan PDF</span>
        </button>
        <button
          onclick={() => (printMode = "none")}
          class="h-9 px-3.5 bg-white/[0.08] hover:bg-white/[0.15] text-zinc-300 font-semibold text-xs rounded-xl transition"
        >
          Tutup
        </button>
      </div>
    </div>

    <!-- Print Paper Page Container (White Paper A4 Landscape) -->
    <div class="max-w-5xl mx-auto w-full print-page-box {printMode === 'booklet' ? 'booklet-preview-container' : 'bg-white p-6 shadow-2xl rounded-lg text-black font-sans'}">
      {#if printMode === "simple_roster" && activeDay}
        <!-- Simple 1-Page Synchronized Roster Table (1:1 Ambrilindo Local Spec) -->
        <div class="simple-roster-page {simpleRosterDensityClass}">
          <!-- Kop Surat -->
          <div class="simple-roster-header">
            <div class="simple-roster-kop-left">
              {#if kopLogo}
                <img src={kopLogo} class="simple-roster-logo" alt="Logo Event" />
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
            <div class="simple-roster-day-badge-box">
              <div class="simple-roster-day-label">{activeDay.label}</div>
              {#if activeDay.tanggal}
                <div class="simple-roster-day-date">{activeDay.tanggal}</div>
              {/if}
            </div>
          </div>

          <!-- Day Section -->
          <div class="simple-roster-day-section">
            <div class="simple-roster-day-bar">
              <span class="day-bar-title">{activeDay.label}</span>
              {#if activeDay.tanggal}
                <span class="day-bar-date">{activeDay.tanggal}</span>
              {/if}
            </div>

            <!-- Synchronized Run Sheet Table -->
            <table class="simple-roster-table">
              <thead>
                <tr>
                  {#each tatamis as t}
                    {@const tMeta = tatamiTimelines[t.id]}
                    <th style="width: {100 / tatamis.length}%;">
                      TATAMI {t.tatami_number}
                      {#if tMeta}
                        <span class="th-sub">({tMeta.totalMatches} Partai • {tMeta.totalAthletes} Atlet)</span>
                      {/if}
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
                            <span class="roster-ishoma-label">{row.label_text || 'ISHOMA / ISTIRAHAT SIANG'}</span>
                          </div>
                        </td>
                      </tr>
                    {:else}
                      <tr>
                        {#each tatamis as t}
                          {@const cell = row.cells.find((c) => c.tatami_id === t.id)}
                          {#if cell && cell.nomor_tanding_id}
                            {@const seq = getTatamiCellSeq(t.id, rIdx)}
                            <td class="roster-match-cell">
                              <div class="roster-item-line">
                                <span class="roster-num-badge">#{seq}</span>
                                <span class="roster-item-title">{cell.nomor_tanding_nama}</span>
                                <span class="roster-atlet-count">({cell.atlet_count})</span>
                              </div>
                            </td>
                          {:else if cell && cell.custom_text}
                            {@const isFest = cell.is_festival || cell.custom_text.toUpperCase().includes('FESTIVAL')}
                            {@const seq = getTatamiCellSeq(t.id, rIdx)}
                            <td class="roster-match-cell {isFest ? 'is-festival' : ''}">
                              <div class="roster-item-line">
                                <span class="roster-num-badge">#{seq}</span>
                                <span class="roster-item-title">{cell.custom_text}</span>
                                {#if cell.atlet_count > 0}
                                  <span class="roster-atlet-count">({cell.atlet_count})</span>
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

          <!-- Bottom Section (Ambrilindo Local Spec - Pure - NO SIGNATURES) -->
          <div class="simple-roster-bottom-section">
            <div class="simple-roster-bottom-left">
              <div class="simple-roster-reminder-box">
                <span class="reminder-badge">PENTING</span>
                <span class="reminder-text">Jadwal bersifat estimasi. Atlet &amp; official wajib berada di area pemanasan 30 menit sebelum partai dimulai.</span>
              </div>

              {#if unassignedCategoriesForPrint.length > 0}
                <div class="simple-roster-unassigned">
                  <span class="unassigned-title">Kelas tidak ditandingkan:</span>
                  <span class="unassigned-items">
                    {unassignedCategoriesForPrint.map((c) => `${c.nama} (${c.atlet_count})`).join(', ')}
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
                        <div class="detailed-card is-break">
                          <div class="detailed-time">
                            {row.time_label || "12:00 - 13:00"} <span class="detailed-dur">(60 menit)</span>
                          </div>
                          <div class="detailed-title">{row.label_text || "ISHOMA / ISTIRAHAT SIANG"}</div>
                        </div>
                      {:else}
                        {@const cell = row.cells.find((c) => c.tatami_id === t.id)}
                        {#if cell && cell.nomor_tanding_id}
                          {@const seq = getTatamiCellSeq(t.id, rIdx)}
                          <div class="detailed-card">
                            <div class="detailed-time">
                              {row.time_label || `Sesi ${rIdx + 1}`} <span class="detailed-dur">({cell.est_duration_minutes || 15} menit)</span>
                            </div>
                            <div class="detailed-title">
                              <span class="roster-num-badge">#{seq}</span>
                              {cell.nomor_tanding_nama} ({cell.atlet_count} Atlet • {cell.matches} Partai)
                            </div>
                          </div>
                        {:else if cell && cell.custom_text}
                          {@const isFest = cell.is_festival || cell.custom_text.toUpperCase().includes("FESTIVAL")}
                          {@const seq = getTatamiCellSeq(t.id, rIdx)}
                          <div class="detailed-card {isFest ? 'is-festival' : ''}">
                            <div class="detailed-time">
                              {row.time_label || `Sesi ${rIdx + 1}`} <span class="detailed-dur">({cell.est_duration_minutes || 45} menit)</span>
                            </div>
                            <div class="detailed-title">
                              <span class="roster-num-badge">#{seq}</span>
                              {cell.custom_text}
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
                    <div class="font-bold">{cell.nomor_tanding_nama}</div>
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
                                          {cell.nomor_tanding_nama}
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
                                          {isFest ? '🥊 ' : ''}{cell.custom_text}
                                        </span>
                                        {#if cell.atlet_count > 0}
                                          <span class="roster-atlet-count">({cell.atlet_count})</span>
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

                    {#if unassignedCategoriesForPrint.length > 0}
                      <div class="simple-roster-unassigned">
                        <span class="unassigned-title">KELAS TIDAK DITANDINGKAN:</span>
                        <span class="unassigned-items">
                          {unassignedCategoriesForPrint.map((c) => `${c.nama} (${c.atlet_count})`).join(', ')}
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
            {@const roundMap = groupMatchesByRound(bCat.matches, bookletPruneEmptyRounds)}
            {@const sortedRounds = Array.from(roundMap.keys()).sort((a, b) => a - b)}
            {@const totalRounds = sortedRounds.length}
            {@const subParts = [
              tatamiBookletData.event_name,
              venueLine1 || 'Samarinda',
              tatamiBookletData.day_date || activeDay?.tanggal || eventDateRangeStr
            ].filter(Boolean)}
            <div class="booklet-page booklet-bracket-page">
              <div class="booklet-page-label-bar no-print">
                <span>Halaman {bIdx + 3} • Bagan {bIdx + 1}/{tatamiBookletData.ordered_categories.filter((c) => c.bagan_id && c.matches && c.matches.length > 0).length}: ({bCat.bagan_kode || bCat.category_id}) {bCat.category_name}</span>
              </div>
              <div class="bracket-page-container">
                <!-- 1. Print Header -->
                <div class="bracket-print-header">
                  <div class="bracket-print-logo-left">
                    <img
                      src={bookletLogoMap[tatamiBookletData.kop_surat?.logo || ""] || tatamiBookletData.kop_surat?.logo || kopLogo || "/img/cover/wkf_logo.png"}
                      alt="Logo Event"
                      onerror={(e) => { (e.currentTarget as HTMLImageElement).src = '/img/cover/wkf_logo.png'; }}
                    />
                  </div>
                  <div class="bracket-print-title">
                    <h2>({bCat.bagan_kode || bCat.category_id}) {bCat.category_name}</h2>
                    <p><i class="fas fa-trophy"></i> {subParts.join(' • ')}</p>
                  </div>
                  <div class="bracket-print-spacer"></div>
                </div>

                <!-- 2. Elimination Grid with exact geometry -->
                <div class="bracket-scroll-viewport">
                  <div class="bracket-outer booklet-bracket-outer" id="bracket-outer-{bCat.category_id}">
                    <svg class="bracket-svg"></svg>
                    <div class="bracket-grid rounds-{sortedRounds.length} {sortedRounds.length === 1 ? 'is-single-round' : ''}">
                      {#each sortedRounds as rNum, rIdx}
                        {@const isFinalRound = (rIdx === sortedRounds.length - 1)}
                        {@const rMatches = roundMap.get(rNum) || []}
                        <div class="round-col {isFinalRound ? 'is-final' : ''}" data-round={rNum}>
                          <div class="round-label {isFinalRound ? 'text-amber-800' : ''}">
                            <span class="round-step {isFinalRound ? 'is-final-step' : ''}">
                              {getRoundStepBadge(rIdx, totalRounds)}
                            </span>
                            <span>{getRoundTitleText(rIdx, totalRounds)}</span>
                          </div>
                          <div class="round-matches">
                            {#each rMatches as m, mIdx}
                              <div class="match {isFinalRound ? 'is-final-card' : ''}" data-id="r{rNum}m{m.urutan || mIdx + 1}">
                                <div class="match-header">
                                  <span class="match-num">{formatMatchTitle(rIdx, totalRounds, m, mIdx)}</span>
                                </div>

                                <!-- AKA row -->
                                <div class="player player-aka {m.pemenang === '1' ? 'is-winner' : ''}">
                                  <div class="player-side aka" title="Aka">Aka</div>
                                  {#if m.atlet1_logo}
                                    <img src={bookletLogoMap[m.atlet1_logo] || m.atlet1_logo} class="player-dojo-logo" alt="Logo Dojo" />
                                  {/if}
                                  <div class="player-info">
                                    <div class="player-name {!m.atlet1_nama ? 'empty' : ''}">
                                      <span>
                                        {#if m.pemenang === '1'}<span class="winner-crown" title="Pemenang">👑</span>{/if}
                                        {#if m.atlet1_is_priority}<span class="priority-star" title="Unggulan">★</span>{/if}
                                        {m.atlet1_nama || 'BYE'}
                                      </span>
                                      {#if m.vr1}
                                        <span class="meta-vr-badge aka">VR</span>
                                      {/if}
                                    </div>
                                    <div class="player-sub">
                                      {m.atlet1_kontingen || (m.atlet1_nama ? '-' : '—')}
                                    </div>
                                    {#if m.selesai && m.kata1 && m.kata1 !== '0 - Blank'}
                                      <div class="player-meta-extra">
                                        <span class="meta-kata aka">{m.kata1}</span>
                                      </div>
                                    {/if}
                                  </div>
                                  <div class="player-score-wrap">
                                    <div class="player-score">{m.score1 ?? '-'}</div>
                                    {#if m.scorekecil1}
                                      <span class="player-score-small">({m.scorekecil1})</span>
                                    {/if}
                                  </div>
                                </div>

                                <!-- AO row -->
                                <div class="player player-ao {m.pemenang === '2' ? 'is-winner' : ''}">
                                  <div class="player-side ao" title="Ao">Ao</div>
                                  {#if m.atlet2_logo}
                                    <img src={bookletLogoMap[m.atlet2_logo] || m.atlet2_logo} class="player-dojo-logo" alt="Logo Dojo" />
                                  {/if}
                                  <div class="player-info">
                                    <div class="player-name {!m.atlet2_nama ? 'empty' : ''}">
                                      <span>
                                        {#if m.pemenang === '2'}<span class="winner-crown" title="Pemenang">👑</span>{/if}
                                        {#if m.atlet2_is_priority}<span class="priority-star" title="Unggulan">★</span>{/if}
                                        {m.atlet2_nama || 'BYE'}
                                      </span>
                                      {#if m.vr2}
                                        <span class="meta-vr-badge ao">VR</span>
                                      {/if}
                                    </div>
                                    <div class="player-sub">
                                      {m.atlet2_kontingen || (m.atlet2_nama ? '-' : '—')}
                                    </div>
                                    {#if m.selesai && m.kata2 && m.kata2 !== '0 - Blank'}
                                      <div class="player-meta-extra">
                                        <span class="meta-kata ao">{m.kata2}</span>
                                      </div>
                                    {/if}
                                  </div>
                                  <div class="player-score-wrap">
                                    <div class="player-score">{m.score2 ?? '-'}</div>
                                    {#if m.scorekecil2}
                                      <span class="player-score-small">({m.scorekecil2})</span>
                                    {/if}
                                  </div>
                                </div>
                              </div>
                            {/each}
                          </div>

                          <!-- Column 4 / Final Column: Absolute Bottom Right Podium Card + Logo -->
                          {#if isFinalRound}
                            {@const w1 = bCat.winners.find((w) => w.juara === '1')}
                            {@const w2 = bCat.winners.find((w) => w.juara === '2')}
                            {@const w3a = bCat.winners.find((w) => w.juara === '3a' || (w.juara === '3' && w !== w1 && w !== w2))}
                            {@const w3b = bCat.winners.find((w) => w.juara === '3b' || (w.juara === '3' && w !== w1 && w !== w2 && w !== w3a))}
                            <div class="cal-final-podium-wrap">
                              <div class="cal-podium-brand">
                                <img src="/img/ambrilindo-logo.png" class="podium-brand-logo" alt="Ambrilindo" />
                              </div>
                              <div class="cal-podium-card">
                                <div class="cal-podium-header">
                                  <div class="cal-podium-header-title">
                                    <i class="fas fa-award text-warning"></i>
                                    <span>Hasil Juara</span>
                                  </div>
                                </div>
                                <div class="cal-podium-body">
                                  <!-- Juara 1 -->
                                  <div class="cal-podium-row">
                                    <div class="cal-podium-rank"><span class="medal gold">1</span></div>
                                    <div class="cal-podium-meta">
                                      <div class="cal-podium-name">{w1?.nama_atlet || 'Belum Ditentukan'}</div>
                                      <div class="cal-podium-sub">{w1 ? (w1.perguruan_nama || w1.utusan_nama || '-') : '—'}</div>
                                    </div>
                                  </div>

                                  <!-- Juara 2 -->
                                  <div class="cal-podium-row">
                                    <div class="cal-podium-rank"><span class="medal silver">2</span></div>
                                    <div class="cal-podium-meta">
                                      <div class="cal-podium-name">{w2?.nama_atlet || 'Belum Ditentukan'}</div>
                                      <div class="cal-podium-sub">{w2 ? (w2.perguruan_nama || w2.utusan_nama || '-') : '—'}</div>
                                    </div>
                                  </div>

                                  <!-- Juara 3A -->
                                  <div class="cal-podium-row">
                                    <div class="cal-podium-rank"><span class="medal bronze">3</span></div>
                                    <div class="cal-podium-meta">
                                      <div class="cal-podium-name">{w3a?.nama_atlet || 'Belum Ditentukan'}</div>
                                      <div class="cal-podium-sub">{w3a ? (w3a.perguruan_nama || w3a.utusan_nama || '-') : '—'}</div>
                                    </div>
                                  </div>

                                  <!-- Juara 3B -->
                                  <div class="cal-podium-row">
                                    <div class="cal-podium-rank"><span class="medal bronze">3</span></div>
                                    <div class="cal-podium-meta">
                                      <div class="cal-podium-name">{w3b?.nama_atlet || 'Belum Ditentukan'}</div>
                                      <div class="cal-podium-sub">{w3b ? (w3b.perguruan_nama || w3b.utusan_nama || '-') : '—'}</div>
                                    </div>
                                  </div>
                                </div>
                              </div>
                            </div>
                          {/if}
                        </div>
                      {/each}
                    </div>
                  </div>
                </div>
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
    background: #fef9c3 !important;
    border-color: #fde047 !important;
  }
  .roster-match-cell.is-festival .roster-item-title {
    color: #854d0e !important;
  }
  .roster-match-cell.is-festival .roster-num-badge {
    background: #fef08a !important;
    color: #713f12 !important;
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

  .bracket-print-title h2 {
    font-size: 13px;
    font-weight: 800;
    color: #111827;
    margin: 0;
    line-height: 1.15;
    text-align: center;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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
  .bracket-grid.rounds-1 .round-col,
  .bracket-grid.is-single-round .round-col {
    flex: 0 0 320px;
    max-width: 320px;
    width: 320px;
    margin: 0 auto;
  }

  .bracket-grid.rounds-2 {
    justify-content: center;
    gap: 48px;
  }
  .bracket-grid.rounds-2 .round-col {
    flex: 0 0 240px;
    max-width: 240px;
    width: 240px;
  }

  .bracket-grid.rounds-3 {
    justify-content: center;
    gap: 24px;
  }
  .bracket-grid.rounds-3 .round-col {
    flex: 1 1 0;
    max-width: 260px;
  }

  .bracket-grid.rounds-4 {
    justify-content: space-between;
    gap: 14px;
  }
  .bracket-grid.rounds-4 .round-col {
    flex: 1 1 0;
    max-width: none;
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

  .round-col.is-final .round-matches {
    display: flex;
    flex-direction: column;
    justify-content: space-around;
    padding-top: 0;
    padding-bottom: 0;
    height: 100%;
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

  .player + .player {
    border-top: 1px solid #e2e8f0;
  }

  .player.is-winner {
    background: #fef3c7 !important;
    border-left: 3.5px solid #d97706 !important;
  }

  .player.is-winner .player-name span,
  .player.is-winner .player-name {
    color: #78350f !important;
    font-weight: 800 !important;
  }

  .player.is-winner .player-score {
    color: #92400e !important;
    font-weight: 800 !important;
    border-color: #d97706 !important;
    background: #fde68a !important;
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

  :global(body.is-printing) .bracket-grid.rounds-1 .round-col,
  :global(body.is-printing) .bracket-grid.is-single-round .round-col {
    flex: 0 0 340px !important;
    max-width: 340px !important;
    width: 340px !important;
    margin: 0 auto !important;
  }

  :global(body.is-printing) .bracket-grid.rounds-2 {
    justify-content: center !important;
    gap: 56px !important;
  }

  :global(body.is-printing) .bracket-grid.rounds-2 .round-col {
    flex: 0 0 250px !important;
    max-width: 250px !important;
    width: 250px !important;
  }

  :global(body.is-printing) .bracket-grid.rounds-3 {
    justify-content: center !important;
    gap: 24px !important;
  }

  :global(body.is-printing) .bracket-grid.rounds-3 .round-col {
    flex: 1 1 0 !important;
    max-width: 260px !important;
  }

  :global(body.is-printing) .bracket-grid.rounds-4 {
    justify-content: space-between !important;
    gap: 14px !important;
  }

  :global(body.is-printing) .bracket-grid.rounds-4 .round-col {
    flex: 1 1 0 !important;
    max-width: none !important;
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

  :global(body.is-printing) .round-col.is-final .round-matches {
    display: flex !important;
    flex-direction: column !important;
    justify-content: space-around !important;
    padding: 0 !important;
    height: 100% !important;
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

    .bracket-print-title h2 {
      font-size: 14px !important;
      font-weight: 800 !important;
      color: #111827 !important;
      margin: 0 !important;
      line-height: 1.15 !important;
      text-align: center !important;
      white-space: nowrap !important;
      overflow: hidden !important;
      text-overflow: ellipsis !important;
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

    .bracket-grid.rounds-1 .round-col,
    .bracket-grid.is-single-round .round-col {
      flex: 0 0 340px !important;
      max-width: 340px !important;
      width: 340px !important;
      margin: 0 auto !important;
    }

    .bracket-grid.rounds-2 {
      justify-content: center !important;
      gap: 56px !important;
    }

    .bracket-grid.rounds-2 .round-col {
      flex: 0 0 250px !important;
      max-width: 250px !important;
      width: 250px !important;
    }

    .bracket-grid.rounds-3 {
      justify-content: center !important;
      gap: 24px !important;
    }

    .bracket-grid.rounds-3 .round-col {
      flex: 1 1 0 !important;
      max-width: 260px !important;
    }

    .bracket-grid.rounds-4 {
      justify-content: space-between !important;
      gap: 14px !important;
    }

    .bracket-grid.rounds-4 .round-col {
      flex: 1 1 0 !important;
      max-width: none !important;
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

    .round-col.is-final .round-matches {
      display: flex !important;
      flex-direction: column !important;
      justify-content: space-around !important;
      padding-top: 0 !important;
      padding-bottom: 0 !important;
      height: 100% !important;
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

    .player + .player {
      border-top: 1px solid #e2e8f0 !important;
    }

    .player.is-winner {
      background: #fef3c7 !important;
      border-left: 3.5px solid #d97706 !important;
      -webkit-print-color-adjust: exact !important;
      print-color-adjust: exact !important;
    }

    .player.is-winner .player-name span,
    .player.is-winner .player-name {
      color: #78350f !important;
      font-weight: 800 !important;
    }

    .player.is-winner .player-score {
      color: #92400e !important;
      font-weight: 800 !important;
      border-color: #d97706 !important;
      background: #fde68a !important;
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

    .cal-podium-rank .medal {
      display: inline-flex !important;
      align-items: center !important;
      justify-content: center !important;
      width: 14px !important;
      height: 14px !important;
      border-radius: 50% !important;
      font-size: 7px !important;
      font-weight: 800 !important;
      font-family: var(--font-mono) !important;
      line-height: 1 !important;
    }

    .cal-podium-rank .medal.gold {
      background: #fde047 !important;
      color: #854d0e !important;
      border: 1px solid #ca8a04 !important;
    }

    .cal-podium-rank .medal.silver {
      background: #e2e8f0 !important;
      color: #334155 !important;
      border: 1px solid #94a3b8 !important;
    }

    .cal-podium-rank .medal.bronze {
      background: #fed7aa !important;
      color: #9a3412 !important;
      border: 1px solid #f97316 !important;
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
