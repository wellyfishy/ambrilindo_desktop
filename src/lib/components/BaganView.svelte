<script lang="ts">
  import { flushSync, onMount, tick } from "svelte";
  import { invoke, withHostParam } from "$lib/api_client";
  import { printWithOrientation } from "$lib/print_helper";
  import { listen } from "@tauri-apps/api/event";
  import type {
    DetailBaganItem,
    BaganItem,
    TatamiItem,
    SaveMatchInput,
    BaganPodium,
    PodiumAthlete,
    EnrichedAtletItem,
    UpdateMatchAthletesInput,
    RepechageInfo,
    RepechageResult,
    RoundRobinData,
    RoundRobinAthleteStanding,
    RoundRobinMatrixCell,
  } from "$lib/types";
  import ControlPanelView from "$lib/components/ControlPanelView.svelte";
  import { initLiveSync, isMatchLive, getRunningTatamiForMatch, subscribeLiveSync, subscribeBracketUpdates } from "$lib/liveSync";
  import BobDrawingModal from "$lib/components/BobDrawingModal.svelte";
  import { FORKI_LOGO_DATA_URL, AMBRILINDO_LOGO_DATA_URL } from "$lib/assets_base64";
  import FestivalBracketSheet from "./FestivalBracketSheet.svelte";
  import BracketTree from "./BracketTree.svelte";
  import BracketPaperPage from "./BracketPaperPage.svelte";
  import { computePodium, deriveBracketMeta, groupRounds } from "$lib/bracket_layout";

  interface Props {
    baganId: number;
    baganName: string;
    baganInfo?: BaganItem | null;
    eventName?: string;
    eventVenue?: string;
    eventDate?: string;
    hostUrl?: string;
    tatamis?: TatamiItem[];
    activeRole?: string;
    onDispatchToTatami?: (tatamiNum: number, matchId: number) => void;
    onBaganUpdated?: () => void;
    onShowToast?: (message: string, type?: "success" | "error" | "info") => void;
  }

  let {
    baganId,
    baganName,
    baganInfo = null,
    eventName = "",
    eventVenue = "Venue Pertandingan",
    eventDate = "",
    hostUrl = "",
    tatamis = [],
    activeRole = "",
    onDispatchToTatami,
    onBaganUpdated,
    onShowToast = () => {},
  }: Props = $props();

  let liveSyncVersion = $state(0);

  let matches = $state<DetailBaganItem[]>([]);
  let isLoading = $state(true);
  let logoMap = $state<Record<string, string>>({});
  let activeMatchForControl = $state<DetailBaganItem | null>(null);

  let currentBaganInfo = $state<BaganItem | null>(null);

  // Delete this category's bracket (all its pools / final), from the bracket page itself
  let showDeleteBagan = $state(false);
  let deleteNeedsResultsConfirm = $state(false);
  let deleteResultsConfirmed = $state(false);
  let deleteBaganError = $state("");
  let isDeletingBagan = $state(false);
  let deleteCategoryId = $derived((currentBaganInfo ?? baganInfo)?.nomor_tanding_id ?? null);
  let deleteEventId = $derived((currentBaganInfo ?? baganInfo)?.event_id ?? null);
  let finishedHere = $derived(matches.filter((m) => m.selesai && !m.team).length);

  function openDeleteBagan() {
    deleteBaganError = "";
    deleteResultsConfirmed = false;
    deleteNeedsResultsConfirm = finishedHere > 0;
    showDeleteBagan = true;
  }

  async function deleteThisBagan() {
    if (!deleteCategoryId || !deleteEventId) return;
    if (deleteNeedsResultsConfirm && !deleteResultsConfirmed) return;
    isDeletingBagan = true;
    deleteBaganError = "";
    try {
      await invoke("delete_category_bagans", {
        eventId: deleteEventId,
        categoryId: deleteCategoryId,
        force: deleteNeedsResultsConfirm && deleteResultsConfirmed,
      });
      showDeleteBagan = false;
      onShowToast(`Bagan ${baganName} dihapus`, "success");
      onBaganUpdated?.();
    } catch (err: any) {
      const msg = typeof err === "string" ? err : err?.message || String(err);
      // Another pool / the final of this category already has results: ask explicitly
      if (/partai selesai/i.test(msg)) {
        deleteNeedsResultsConfirm = true;
        deleteBaganError = msg;
      } else {
        deleteBaganError = `Gagal menghapus bagan: ${msg}`;
      }
    } finally {
      isDeletingBagan = false;
    }
  }
  $effect(() => {
    currentBaganInfo = baganInfo;
  });

  let displayBaganName = $derived(currentBaganInfo?.nama_bagan || baganName);

  // Edit Mode state
  let isEditMode = $state(false);
  let isSavingEdit = $state(false);
  let editBaganName = $state("");
  let editedMatchups = $state<Record<number, { atlet1_id: number | null; atlet2_id: number | null }>>({});
  let categoryAthletes = $state<EnrichedAtletItem[]>([]);

  // Late entry (Edit Bagan): register an athlete straight into this bracket's category,
  // then place them by hand in a free slot.
  let lateModalOpen = $state(false);
  let lateName = $state("");
  let lateUtusanId = $state<number | null>(null);
  let latePerguruanId = $state<number | null>(null);
  let lateSaving = $state(false);
  let lateError = $state<string | null>(null);
  let lateUtusanList = $state<{ id: number; nama_utusan: string }[]>([]);
  let latePerguruanList = $state<{ id: number; nama_perguruan: string }[]>([]);

  /** Athletes of the category not placed in any slot of this bracket (while editing). */
  let unplacedAthletes = $derived.by(() => {
    if (!isEditMode) return [];
    const placed = new Set<number>();
    for (const v of Object.values(editedMatchups)) {
      if (v.atlet1_id) placed.add(Number(v.atlet1_id));
      if (v.atlet2_id) placed.add(Number(v.atlet2_id));
    }
    return categoryAthletes.filter((a) => !placed.has(a.id) && !(a.additional_code || "").startsWith("MEMBER_"));
  });

  async function openLateModal() {
    lateName = "";
    lateError = null;
    lateModalOpen = true;
    const evId = currentBaganInfo?.event_id ?? baganInfo?.event_id;
    if (!evId) return;
    try {
      const [u, p] = await Promise.all([
        invoke<any[]>("get_contingents", { eventId: evId }),
        invoke<any[]>("get_perguruans", { eventId: evId }),
      ]);
      lateUtusanList = (u || []).map((x) => ({ id: x.id, nama_utusan: x.nama_utusan || "-" }));
      latePerguruanList = (p || []).map((x) => ({ id: x.id, nama_perguruan: x.nama_perguruan || "-" }));
    } catch (e) {
      console.warn("Gagal memuat kontingen/perguruan:", e);
    }
  }

  async function saveLateAthlete() {
    const evId = currentBaganInfo?.event_id ?? baganInfo?.event_id;
    const catId = currentBaganInfo?.nomor_tanding_id ?? baganInfo?.nomor_tanding_id;
    if (!lateName.trim()) {
      lateError = "Nama atlet wajib diisi.";
      return;
    }
    if (!evId || !catId) {
      lateError = "Bagan ini tidak terhubung ke nomor tanding.";
      return;
    }
    lateSaving = true;
    lateError = null;
    try {
      const created = await invoke<EnrichedAtletItem>("create_athlete", {
        input: {
          id: null,
          event_id: evId,
          nama_atlet: lateName.trim().toUpperCase(),
          nik: null,
          kode_atlet: null,
          additional_code: null,
          is_priority: false,
          perguruan_id: latePerguruanId,
          utusan_id: lateUtusanId,
          nomor_tanding_id: catId,
          allow_late: true,
        },
      });
      categoryAthletes = [...categoryAthletes, created];
      lateModalOpen = false;
      onShowToast(`${created.nama_atlet} ditambahkan. Pilih slot kosongnya di bagan, lalu Simpan.`, "success");
    } catch (e: any) {
      lateError = `${e?.message || e}`;
    } finally {
      lateSaving = false;
    }
  }

  let dbPodium = $state<BaganPodium | null>(null);

  // WKF Repechage state
  let repechageInfo = $state<RepechageInfo | null>(null);
  let isGeneratingRepechage = $state(false);
  let isRepechageBagan = $derived(
    displayBaganName.toUpperCase().includes("REFERCHANGE") || displayBaganName.toUpperCase().includes("REPECHAGE")
  );

  // Round-Robin Tournament state
  let roundRobinData = $state<RoundRobinData | null>(null);
  let isRoundRobin = $derived(
    !!currentBaganInfo?.round_robin ||
    displayBaganName.toUpperCase().includes("ROUND-ROBIN") ||
    displayBaganName.toUpperCase().includes("ROUND ROBIN")
  );

  // Best of the Best (BOB) state
  let showBobModal = $state(false);
  let isBobBagan = $derived(
    !!currentBaganInfo?.is_bob ||
    displayBaganName.toUpperCase().includes("BEST OF THE BEST") ||
    displayBaganName.toUpperCase().includes("BOB")
  );

  // Festival Tournament state
  let isFestivalBagan = $derived(
    displayBaganName.toUpperCase().includes("FESTIVAL")
  );

  let festivalGroupCount = $derived.by(() => {
    if (currentBaganInfo?.pool && currentBaganInfo.pool > 0) {
      return currentBaganInfo.pool;
    }
    const maxFinalUrutan = Math.max(...matches.filter((m) => m.round === 2).map((m) => m.urutan ?? 0), 0);
    if (maxFinalUrutan > 0) return maxFinalUrutan;
    const maxSemiUrutan = Math.max(...matches.filter((m) => m.round === 1).map((m) => m.urutan ?? 0), 0);
    if (maxSemiUrutan > 0) return Math.ceil(maxSemiUrutan / 2);
    return 1;
  });

  let festivalGroups = $derived.by(() => {
    const count = Math.min(6, Math.max(1, festivalGroupCount));
    const groups = [];

    for (let g = 1; g <= count; g++) {
      const semi1 = matches.find((m) => m.round === 1 && m.urutan === 2 * g - 1) || null;
      const semi2 = matches.find((m) => m.round === 1 && m.urutan === 2 * g) || null;
      const finalMatch = matches.find((m) => m.round === 2 && m.urutan === g) || null;

      let emas: PodiumAthlete | null = null;
      let perak: PodiumAthlete | null = null;
      let perunggu1: PodiumAthlete | null = null;
      let perunggu2: PodiumAthlete | null = null;

      if (finalMatch && finalMatch.selesai && finalMatch.pemenang) {
        if (finalMatch.pemenang === "1") {
          emas = finalMatch.atlet1_id ? {
            id: finalMatch.atlet1_id,
            name: finalMatch.atlet1_nama || `Atlet #${finalMatch.atlet1_id}`,
            contingent: finalMatch.atlet1_kontingen || "",
            logo: finalMatch.atlet1_logo || null,
            is_priority: finalMatch.atlet1_is_priority || false,
          } : null;
          perak = finalMatch.atlet2_id ? {
            id: finalMatch.atlet2_id,
            name: finalMatch.atlet2_nama || `Atlet #${finalMatch.atlet2_id}`,
            contingent: finalMatch.atlet2_kontingen || "",
            logo: finalMatch.atlet2_logo || null,
            is_priority: finalMatch.atlet2_is_priority || false,
          } : null;
        } else if (finalMatch.pemenang === "2") {
          emas = finalMatch.atlet2_id ? {
            id: finalMatch.atlet2_id,
            name: finalMatch.atlet2_nama || `Atlet #${finalMatch.atlet2_id}`,
            contingent: finalMatch.atlet2_kontingen || "",
            logo: finalMatch.atlet2_logo || null,
            is_priority: finalMatch.atlet2_is_priority || false,
          } : null;
          perak = finalMatch.atlet1_id ? {
            id: finalMatch.atlet1_id,
            name: finalMatch.atlet1_nama || `Atlet #${finalMatch.atlet1_id}`,
            contingent: finalMatch.atlet1_kontingen || "",
            logo: finalMatch.atlet1_logo || null,
            is_priority: finalMatch.atlet1_is_priority || false,
          } : null;
        }
      }

      if (semi1 && semi1.selesai && semi1.pemenang) {
        const loserId = semi1.pemenang === "1" ? semi1.atlet2_id : semi1.atlet1_id;
        const loserNama = semi1.pemenang === "1" ? semi1.atlet2_nama : semi1.atlet1_nama;
        const loserKontingen = semi1.pemenang === "1" ? semi1.atlet2_kontingen : semi1.atlet1_kontingen;
        const loserLogo = semi1.pemenang === "1" ? semi1.atlet2_logo : semi1.atlet1_logo;
        if (loserId && loserNama && !loserNama.toUpperCase().includes("BYE")) {
          perunggu1 = {
            id: loserId,
            name: loserNama,
            contingent: loserKontingen || "",
            logo: loserLogo || null,
            is_priority: false,
          };
        }
      }

      if (semi2 && semi2.selesai && semi2.pemenang) {
        const loserId = semi2.pemenang === "1" ? semi2.atlet2_id : semi2.atlet1_id;
        const loserNama = semi2.pemenang === "1" ? semi2.atlet2_nama : semi2.atlet1_nama;
        const loserKontingen = semi2.pemenang === "1" ? semi2.atlet2_kontingen : semi2.atlet1_kontingen;
        const loserLogo = semi2.pemenang === "1" ? semi2.atlet2_logo : semi2.atlet1_logo;
        if (loserId && loserNama && !loserNama.toUpperCase().includes("BYE")) {
          perunggu2 = {
            id: loserId,
            name: loserNama,
            contingent: loserKontingen || "",
            logo: loserLogo || null,
            is_priority: false,
          };
        }
      }

      const isCompleted = !!(finalMatch && finalMatch.selesai);

      groups.push({
        groupNum: g,
        matches: { semi1, semi2, final: finalMatch },
        podium: { emas, perak, perunggu1, perunggu2 },
        isCompleted,
      });
    }

    return groups;
  });

  let festivalTotalSlots = $derived(festivalGroups.length <= 4 ? 4 : 6);

  // Festival Quick Scoring Modal State
  let festivalScoreModalMatch = $state<DetailBaganItem | null>(null);
  let festWinner = $state<string>("1");
  let festScore1 = $state<string>("0");
  let festScore2 = $state<string>("0");
  let isSavingFestMatch = $state(false);

  async function handleUpdateLayoutType(newLayout: string) {
    if (!baganId) return;
    try {
      await invoke("update_bagan_layout_type", {
        baganId,
        layoutType: newLayout,
      });
      if (currentBaganInfo) {
        currentBaganInfo.layout_type = newLayout;
      }
      onShowToast?.(`Template bagan festival diubah ke ${newLayout === 'bands' ? 'Horizontal Bands' : '4-Quadrant Grid'}`, "success");
    } catch (err: any) {
      console.error("Gagal mengubah template bagan:", err);
      onShowToast?.(`Gagal mengubah template: ${err?.message || err}`, "error");
    }
  }

  function openFestScoreModal(m: DetailBaganItem) {
    if (!m.atlet1_id || !m.atlet2_id) return;
    festivalScoreModalMatch = m;
    festWinner = m.pemenang || (m.atlet2_id ? "1" : "1");
    festScore1 = m.score1 ?? "0";
    festScore2 = m.score2 ?? "0";
  }

  async function saveFestScore() {
    if (!festivalScoreModalMatch) return;
    isSavingFestMatch = true;
    try {
      const input: SaveMatchInput = {
        match_id: festivalScoreModalMatch.id,
        bagan_id: festivalScoreModalMatch.bagan_id || baganId,
        score1: festScore1,
        score2: festScore2,
        pemenang: festWinner,
        selesai: true,
        vr1: false,
        vr2: false,
        hantei: false,
      };
      await handleSaveMatchResult(input);
      festivalScoreModalMatch = null;
      await loadBracketData();
      onBaganUpdated?.();
    } catch (err) {
      console.error("Gagal menyimpan skor festival:", err);
    } finally {
      isSavingFestMatch = false;
    }
  }

  async function resetFestMatch() {
    if (!festivalScoreModalMatch) return;
    isSavingFestMatch = true;
    try {
      const input: SaveMatchInput = {
        match_id: festivalScoreModalMatch.id,
        bagan_id: festivalScoreModalMatch.bagan_id || baganId,
        score1: "-",
        score2: "-",
        pemenang: null,
        selesai: false,
        vr1: false,
        vr2: false,
        hantei: false,
      };
      await handleSaveMatchResult(input);
      festivalScoreModalMatch = null;
      await loadBracketData();
      onBaganUpdated?.();
    } catch (err) {
      console.error("Gagal mereset skor festival:", err);
    } finally {
      isSavingFestMatch = false;
    }
  }

  async function loadRoundRobinData() {
    if (!baganId) return;
    try {
      const data = await invoke<RoundRobinData>("get_round_robin_data", { baganId });
      roundRobinData = data;
    } catch {
      roundRobinData = null;
    }
  }

  async function loadRepechageInfo() {
    if (!baganId) return;
    try {
      const info = await invoke<RepechageInfo>("get_repechage_info", { baganId });
      repechageInfo = info;
    } catch (err) {
      console.error("Gagal memuat info repechage:", err);
    }
  }

  async function handleGenerateRepechage() {
    if (!baganId) return;
    isGeneratingRepechage = true;
    try {
      const res = await invoke<RepechageResult>("generate_repechage_bracket", { mainBaganId: baganId });
      await loadRepechageInfo();
      if (onBaganUpdated) onBaganUpdated();
      alert(res.message);
    } catch (err: any) {
      alert(typeof err === "string" ? err : err.message || "Gagal membuat bagan referchange.");
    } finally {
      isGeneratingRepechage = false;
    }
  }

  async function handleSaveMatchResult(input: SaveMatchInput) {
    try {
      await invoke("save_match_result", { input });
      activeMatchForControl = null;
      await loadBracketData();
    } catch (err: any) {
      console.error("Gagal menyimpan hasil partai:", err);
      // Never show an unsaved result as saved: the operator must see that it was refused.
      onShowToast?.(`Hasil partai tidak tersimpan: ${err?.message || err}`, "error");
      alert(`Hasil partai tidak tersimpan. ${err?.message || err}`);
    }
  }

  function resolveTatamiNumber(assignedTatamiId: number | null | undefined): number {
    if (!assignedTatamiId) return 1;
    const found = tatamis?.find((t) => t.id === assignedTatamiId || t.tatami_number === assignedTatamiId);
    return found?.tatami_number ?? (assignedTatamiId < 20 ? assignedTatamiId : 1);
  }

  async function handleOpenMatchControl(match: DetailBaganItem) {
    let targetTatamiNum = 1;
    if (activeRole && activeRole.startsWith("admin_tatami_")) {
      targetTatamiNum = parseInt(activeRole.replace("admin_tatami_", ""), 10) || 1;
    } else {
      targetTatamiNum = resolveTatamiNumber(match.assigned_tatami_id);
    }

    try {
      await invoke("open_control_panel_window", {
        matchId: match.id,
        baganId: match.bagan_id || baganId,
        tatami: targetTatamiNum,
      });
    } catch (err) {
      console.warn("Tauri open_control_panel_window failed, fallback to window.open or in-page:", err);
      if (typeof window !== "undefined" && window.open) {
        window.open(
          withHostParam(`/control-panel?matchId=${match.id}&baganId=${match.bagan_id || baganId}&tatami=${targetTatamiNum}`),
          "control_panel",
          "width=1366,height=820"
        );
      } else {
        activeMatchForControl = match;
      }
    }
  }

  async function enterEditMode() {
    editBaganName = currentBaganInfo?.nama_bagan || baganName;
    const map: Record<number, { atlet1_id: number | null; atlet2_id: number | null }> = {};
    for (const m of matches) {
      map[m.id] = {
        atlet1_id: m.atlet1_id ?? null,
        atlet2_id: m.atlet2_id ?? null,
      };
    }
    editedMatchups = map;

    try {
      categoryAthletes = await invoke<EnrichedAtletItem[]>("get_athletes_for_bagan", { baganId });
    } catch (e) {
      console.warn("get_athletes_for_bagan failed (fallback to matches):", e);
      const fallbackList: EnrichedAtletItem[] = [];
      const seenIds = new Set<number>();
      for (const m of matches) {
        if (m.atlet1_id && !seenIds.has(m.atlet1_id)) {
          seenIds.add(m.atlet1_id);
          fallbackList.push({
            id: m.atlet1_id,
            event_id: null,
            nama_atlet: m.atlet1_nama || `Atlet #${m.atlet1_id}`,
            nik: null,
            kode_atlet: null,
            additional_code: null,
            is_priority: !!m.atlet1_is_priority,
            perguruan_id: null,
            perguruan_nama: null,
            utusan_id: null,
            utusan_nama: m.atlet1_kontingen || null,
            utusan_logo: m.atlet1_logo || null,
            nomor_tanding_id: null,
            nomor_tanding_nama: null,
            nomor_tanding_tipe: null,
            is_bob: null,
          });
        }
        if (m.atlet2_id && !seenIds.has(m.atlet2_id)) {
          seenIds.add(m.atlet2_id);
          fallbackList.push({
            id: m.atlet2_id,
            event_id: null,
            nama_atlet: m.atlet2_nama || `Atlet #${m.atlet2_id}`,
            nik: null,
            kode_atlet: null,
            additional_code: null,
            is_priority: !!m.atlet2_is_priority,
            perguruan_id: null,
            perguruan_nama: null,
            utusan_id: null,
            utusan_nama: m.atlet2_kontingen || null,
            utusan_logo: m.atlet2_logo || null,
            nomor_tanding_id: null,
            nomor_tanding_nama: null,
            nomor_tanding_tipe: null,
            is_bob: null,
          });
        }
      }
      categoryAthletes = fallbackList;
    }
    isEditMode = true;
  }

  function cancelEditMode() {
    isEditMode = false;
    editBaganName = currentBaganInfo?.nama_bagan || baganName;
    editedMatchups = {};
  }

  async function saveEditMode() {
    isSavingEdit = true;
    try {
      const matchupsPayload: UpdateMatchAthletesInput[] = Object.entries(editedMatchups).map(
        ([mIdStr, val]) => {
          const a1 = val.atlet1_id as any;
          const a2 = val.atlet2_id as any;
          return {
            match_id: Number(mIdStr),
            atlet1_id:
              a1 === null || a1 === undefined || a1 === "" || a1 === "null" || Number(a1) === 0 || isNaN(Number(a1))
                ? null
                : Number(a1),
            atlet2_id:
              a2 === null || a2 === undefined || a2 === "" || a2 === "null" || Number(a2) === 0 || isNaN(Number(a2))
                ? null
                : Number(a2),
          };
        }
      );

      await invoke("update_bagan_details", {
        baganId,
        namaBagan: editBaganName.trim() || undefined,
        matchups: matchupsPayload,
      });

      isEditMode = false;
      await loadBracketData();
      if (onBaganUpdated) onBaganUpdated();
    } catch (err: any) {
      // Stay in edit mode with the operator's changes; nothing was saved.
      onShowToast(`Perubahan bagan tidak tersimpan: ${err?.message || err}`, "error");
      alert(`Perubahan bagan tidak tersimpan. ${err?.message || err}`);
    } finally {
      isSavingEdit = false;
    }
  }

  async function resolveLogos(items: DetailBaganItem[]) {
    const needed = new Set<string>();
    for (const item of items) {
      if (item.atlet1_logo) needed.add(item.atlet1_logo);
      if (item.atlet2_logo) needed.add(item.atlet2_logo);
    }

    for (const path of needed) {
      if (logoMap[path]) continue;
      if (path.startsWith("http://") || path.startsWith("https://") || path.startsWith("data:")) {
        logoMap[path] = path;
        continue;
      }
      try {
        const dataUrl = await invoke<string | null>("get_media_data_url", { relativePath: path });
        if (dataUrl) {
          logoMap[path] = dataUrl;
          continue;
        }
      } catch {
        // Not running in Tauri desktop or IPC unavailable, fallback to HTTP
      }
      if (hostUrl) {
        logoMap[path] = `${hostUrl.replace(/\/$/, "")}/media/${path.replace(/^\//, "")}`;
      }
    }
  }

  // App Theme state: Dark mode by default for desktop app, white for print
  let isDarkMode = $state(true);

  /**
   * Bracket/festival print the light paper page. Round robin prints its screen view, so
   * render it in the light palette for the duration of the print dialog.
   */
  function printBagan() {
    if (isRoundRobin && isDarkMode) {
      flushSync(() => (isDarkMode = false));
      window.addEventListener("afterprint", () => (isDarkMode = true), { once: true });
    }
    printWithOrientation("landscape", "5mm");
  }

  // Zoom & Viewport adaptivity state
  let zoom = $state(1.0);
  let userHasZoomed = $state(false);
  let contentWidth = $state(1064);
  let contentHeight = $state(692);

  let viewportEl = $state<HTMLDivElement | null>(null);
  let gridEl = $state<HTMLDivElement | null>(null);

  function updateContentDimensions() {
    if (gridEl) {
      contentWidth = Math.max(1064, gridEl.scrollWidth);
      contentHeight = Math.max(692, gridEl.scrollHeight);
    }
  }

  function zoomIn() {
    userHasZoomed = true;
    zoom = Math.min(2.5, Math.round((zoom + 0.15) * 100) / 100);
  }

  function zoomOut() {
    userHasZoomed = true;
    zoom = Math.max(0.5, Math.round((zoom - 0.15) * 100) / 100);
  }

  function resetZoom() {
    userHasZoomed = true;
    zoom = 1.0;
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

  async function loadPodiumData() {
    try {
      const data = await invoke<BaganPodium>("get_bagan_podium", { baganId });
      if (data) {
        dbPodium = data;
        const logosToResolve = [data.j1?.logo, data.j2?.logo, data.j3a?.logo, data.j3b?.logo].filter(Boolean) as string[];
        for (const path of logosToResolve) {
          if (!logoMap[path]) {
            if (path.startsWith("http://") || path.startsWith("https://") || path.startsWith("data:")) {
              logoMap[path] = path;
            } else {
              try {
                const dataUrl = await invoke<string | null>("get_media_data_url", { relativePath: path });
                if (dataUrl) logoMap[path] = dataUrl;
              } catch {}
            }
          }
        }
      }
    } catch {
      // In browser fallback or if IPC not ready
    }
  }

  async function loadBracketData() {
    isLoading = true;
    try {
      const data = await invoke<DetailBaganItem[]>("get_detail_bagan", { baganId });
      matches = data;
      resolveLogos(data);
      try {
        const b = await invoke<BaganItem | null>("get_bagan_by_id", { baganId });
        if (b) currentBaganInfo = b;
      } catch {}
      await loadPodiumData();
      await loadRepechageInfo();
      await loadRoundRobinData();
    } catch (err) {
      console.error("Gagal memuat detail bagan:", err);
      // Keep whatever was shown before; never substitute stand-in athletes.
      onShowToast(`Bagan gagal dimuat: ${err}`, "error");
    } finally {
      isLoading = false;
      await tick();
      requestAnimationFrame(() => {
        updateContentDimensions();
        if (!userHasZoomed) {
          fitToScreen();
        }
      });
    }
  }

  // Group matches by round (1 = Babak 16 Besar, 2 = Perempat Final, 3 = Semi Final, 4 = Final)
  let rounds = $derived(groupRounds(matches));

  // Main elimination tree rounds (1 to 4)
  let elimRounds = $derived(rounds.filter(([r]) => r <= 4));
  let elimMatchesCount = $derived(matches.filter((m) => (m.round || 1) <= 4).length);

  let poolCount = $derived.by(() => {
    const p = currentBaganInfo?.pool ?? (baganInfo?.pool ?? 1);
    if (p === 2 || p === 4) return p;
    const name = (displayBaganName || "").toLowerCase();
    if (
      name.includes("pool a") ||
      name.includes("pool b") ||
      name.includes("pool 1") ||
      name.includes("pool 2") ||
      name.includes("2 pool")
    ) {
      return 2;
    }
    if (
      name.includes("pool c") ||
      name.includes("pool d") ||
      name.includes("4 pool")
    ) {
      return 4;
    }
    return 1;
  });

  let isFinalBagan = $derived.by(() => {
    if (currentBaganInfo?.pool === 0 || baganInfo?.pool === 0) return true;
    const name = (displayBaganName || "").toUpperCase().trim();
    return name.endsWith("- FINAL") || name.endsWith("FINAL");
  });


  // Shared bracket metadata + Hasil Juara (same rules as the booklets; see bracket_layout.ts)
  let bracketMeta = $derived({
    poolCount,
    isFinalBagan,
    isRepechageBagan,
    baganName: baganName || "",
  });
  let podium = $derived(computePodium(matches, bracketMeta, dbPodium));


  $effect(() => {
    if (baganId) {
      loadBracketData();
    }
  });

  $effect(() => {
    if (matches.length > 0) {
      tick().then(() => {
        updateContentDimensions();
      });
    }
  });

  onMount(() => {
    initLiveSync(hostUrl);
    const unsubLive = subscribeLiveSync(() => {
      liveSyncVersion++;
    });
    // Results saved on another laptop (or while this one was offline) arrive over the LAN.
    let bracketReloadTimer: ReturnType<typeof setTimeout> | null = null;
    const unsubBracket = subscribeBracketUpdates(() => {
      if (isEditMode) return; // do not throw away the operator's unsaved edits
      if (bracketReloadTimer) clearTimeout(bracketReloadTimer);
      bracketReloadTimer = setTimeout(() => {
        bracketReloadTimer = null;
        loadBracketData();
      }, 300);
    });
    loadBracketData();

    let ro: ResizeObserver | null = null;
    if (typeof ResizeObserver !== "undefined") {
      ro = new ResizeObserver(() => {
        updateContentDimensions();
      });
      if (gridEl) ro.observe(gridEl);
      if (viewportEl) ro.observe(viewportEl);
    }

    const handleWindowResize = () => {
      updateContentDimensions();
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
      });
    }

    let unlistenBracketUpdated: (() => void) | undefined;
    listen("bracket_updated", (event) => {
      console.log("Bracket updated event received from control panel:", event);
      loadBracketData();
    })
      .then((unsub) => {
        unlistenBracketUpdated = unsub;
      })
      .catch((err) => {
        console.warn("Gagal listen bracket_updated:", err);
      });

    let bc: BroadcastChannel | null = null;
    if (typeof BroadcastChannel !== "undefined") {
      bc = new BroadcastChannel("ambrilindo_live_sync");
      bc.onmessage = (event) => {
        if (event.data?.type === "bracket_updated" || event.data?.type === "tatami_live_state") {
          loadBracketData();
        }
      };
    }

    return () => {
      unsubLive();
      unsubBracket();
      if (bracketReloadTimer) clearTimeout(bracketReloadTimer);
      ro?.disconnect();
      window.removeEventListener("resize", handleWindowResize);
      window.removeEventListener("keydown", handleKeyDown);
      if (unlistenBracketUpdated) unlistenBracketUpdated();
      bc?.close();
    };
  });
</script>

{#if activeMatchForControl}
  <ControlPanelView
    match={activeMatchForControl}
    baganInfo={currentBaganInfo || baganInfo}
    baganName={displayBaganName || baganName || currentBaganInfo?.nama_bagan || ""}
    eventName={eventName}
    tatamiNumber={activeRole && activeRole.startsWith("admin_tatami_") ? (parseInt(activeRole.replace("admin_tatami_", ""), 10) || 1) : resolveTatamiNumber(activeMatchForControl.assigned_tatami_id)}
    hostUrl={hostUrl}
    logoMap={logoMap}
    onBack={() => { activeMatchForControl = null; }}
    onSave={handleSaveMatchResult}
  />
{:else}
<div class="h-full flex flex-col p-4 select-none {isDarkMode ? 'bg-[#14161b] text-zinc-200' : 'bg-slate-100 text-slate-800'} overflow-hidden font-sans print-sheet {isRoundRobin ? '' : 'has-paper-print'}">
  <!-- Top Control Bar (Hidden on Print) -->
  <div class="no-print flex items-center justify-between pb-2.5 mb-2 border-b {isDarkMode ? 'border-white/[0.08]' : 'border-slate-300'} shrink-0">
    <div class="flex items-center space-x-2">
      <span class="text-xs font-black {isDarkMode ? 'text-white' : 'text-slate-800'} uppercase tracking-wider">
        Bagan Pertandingan
      </span>
      <span class="{isDarkMode ? 'text-zinc-600' : 'text-slate-300'}">•</span>
      <span class="text-xs {isDarkMode ? 'text-zinc-400' : 'text-slate-500'} font-mono">
        {elimMatchesCount} Partai Terdaftar
      </span>
    </div>

    <!-- Zoom & Action Toolbar -->
    <div class="flex items-center space-x-2">
      <!-- Zoom Toolbar -->
      <div class="flex items-center {isDarkMode ? 'bg-[#181a20] border-white/[0.08]' : 'bg-white border-slate-300'} border rounded-lg p-0.5 text-xs font-mono shadow-xs">
        <button
          type="button"
          onclick={zoomOut}
          disabled={zoom <= 0.5}
          class="w-7 h-7 flex items-center justify-center {isDarkMode ? 'text-zinc-400 hover:text-white hover:bg-white/[0.08]' : 'text-slate-600 hover:text-slate-900 hover:bg-slate-100'} rounded-md transition disabled:opacity-30 disabled:hover:bg-transparent active:scale-95"
          title="Perkecil Tampilan (Ctrl + -)"
        >
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M20 12H4" />
          </svg>
        </button>

        <button
          type="button"
          onclick={resetZoom}
          class="px-2 h-7 flex items-center justify-center font-bold {isDarkMode ? 'text-zinc-200 hover:text-white hover:bg-white/[0.08]' : 'text-slate-700 hover:text-slate-900 hover:bg-slate-100'} rounded-md transition min-w-[50px] text-[11px]"
          title="Klik untuk reset ke 100% (Ctrl + 0)"
        >
          {Math.round(zoom * 100)}%
        </button>

        <button
          type="button"
          onclick={zoomIn}
          disabled={zoom >= 2.5}
          class="w-7 h-7 flex items-center justify-center {isDarkMode ? 'text-zinc-400 hover:text-white hover:bg-white/[0.08]' : 'text-slate-600 hover:text-slate-900 hover:bg-slate-100'} rounded-md transition disabled:opacity-30 disabled:hover:bg-transparent active:scale-95"
          title="Perbesar Tampilan (Ctrl + +)"
        >
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2.5" d="M12 4v16m8-8H4" />
          </svg>
        </button>

        <div class="h-4 w-px {isDarkMode ? 'bg-white/[0.1]' : 'bg-slate-200'} mx-0.5"></div>

        <button
          type="button"
          onclick={fitToScreen}
          class="px-2.5 h-7 flex items-center space-x-1 font-semibold {isDarkMode ? 'text-amber-400 hover:text-amber-300 hover:bg-amber-400/10' : 'text-amber-700 hover:text-amber-800 hover:bg-amber-100/60'} rounded-md transition active:scale-95"
          title="Sesuaikan skala otomatis dengan layar"
        >
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 8V4m0 0h4M4 4l5 5m11-1V4m0 0h-4m4 0l-5 5M4 16v4m0 0h4m-4 0l5-5m11 5l-5-5m5 5v-4m0 4h-4" />
          </svg>
          <span class="text-[11px] font-sans">Sesuaikan</span>
        </button>
      </div>

      <!-- Print Button -->
      <button
        type="button"
        onclick={printBagan}
        class="px-3 py-1.5 {isDarkMode ? 'bg-white/[0.05] hover:bg-white/[0.08] text-zinc-200 border-white/[0.1]' : 'bg-white hover:bg-slate-50 text-slate-700 border-slate-300'} text-xs font-semibold border rounded-lg transition flex items-center space-x-1.5 shadow-xs active:scale-95"
        title="Cetak format kertas A4 Landscape"
      >
        <svg class="w-3.5 h-3.5 {isDarkMode ? 'text-zinc-400' : 'text-slate-500'}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
        </svg>
        <span>Cetak (A4)</span>
      </button>

      <!-- Festival Indicator -->
      {#if isFestivalBagan}
        <div class="px-2.5 py-1 rounded-lg bg-emerald-500/15 border border-emerald-500/30 text-emerald-300 text-[11px] font-bold flex items-center gap-1.5 shadow-xs">
          <span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
          <span>BAGAN FESTIVAL (4 ATLET / GRUP • {festivalGroupCount} GRUP)</span>
        </div>
      {/if}

      <!-- Round Robin or Repechage Actions -->
      {#if isRoundRobin}
        <div class="px-2.5 py-1 rounded-lg bg-indigo-500/15 border border-indigo-500/30 text-indigo-300 text-[11px] font-bold flex items-center gap-1.5 shadow-xs">
          <span class="w-1.5 h-1.5 rounded-full bg-indigo-400"></span>
          <span>ROUND-ROBIN (BERGER)</span>
        </div>
      {:else if !isRepechageBagan}
        {#if repechageInfo?.repechage_bagan_id}
          <button
            type="button"
            onclick={() => {
              if (repechageInfo?.repechage_bagan_id) {
                baganId = repechageInfo.repechage_bagan_id;
                baganName = repechageInfo.repechage_bagan_name || "BAGAN REFERCHANGE";
                loadBracketData();
              }
            }}
            class="px-3 py-1.5 bg-emerald-500/15 hover:bg-emerald-500/25 text-emerald-300 border border-emerald-500/40 text-xs font-semibold rounded-lg transition flex items-center space-x-1.5 shadow-xs active:scale-95"
            title="Buka bagan repechage perebutan Juara 3 Bersama"
          >
            <span class="w-2 h-2 rounded-full bg-emerald-400"></span>
            <span>Bagan Referchange</span>
          </button>
        {:else if repechageInfo?.can_generate}
          <button
            type="button"
            onclick={handleGenerateRepechage}
            disabled={isGeneratingRepechage}
            class="px-3 py-1.5 bg-amber-500 hover:bg-amber-400 text-black font-bold text-xs rounded-lg transition flex items-center space-x-1.5 shadow-sm active:scale-95 disabled:opacity-50"
            title="Otomatis telusuri atlet yang kalah dari kedua finalis untuk memperebutkan Juara 3 Bersama"
          >
            <svg class="w-3.5 h-3.5 fill-current" viewBox="0 0 24 24">
              <path d="M12 2l3.09 6.26L22 9.27l-5 4.87 1.18 6.88L12 17.77l-6.18 3.25L7 14.14 2 9.27l6.91-1.01L12 2z"/>
            </svg>
            <span>{isGeneratingRepechage ? "Membuat..." : "+ Buat Referchange"}</span>
          </button>
        {/if}
      {:else}
        <div class="px-2.5 py-1 rounded-lg bg-amber-500/15 border border-amber-500/30 text-amber-300 text-[11px] font-bold flex items-center gap-1.5">
          <span class="w-1.5 h-1.5 rounded-full bg-amber-400"></span>
          <span>WKF REPECHAGE (JUARA 3A & 3B)</span>
        </div>
      {/if}

      {#if isBobBagan}
        <div class="px-2.5 py-1 rounded-lg bg-gradient-to-r from-amber-500/25 to-yellow-500/25 border border-amber-500/50 text-amber-300 text-[11px] font-black tracking-wide flex items-center gap-1.5 shadow-sm">
          <span>👑</span>
          <span>BEST OF THE BEST (BOB)</span>
        </div>
      {/if}

      <!-- Drawing BOB Modal Button -->
      <button
        type="button"
        onclick={() => (showBobModal = true)}
        class="px-3 py-1.5 bg-gradient-to-r from-amber-500/15 to-yellow-500/15 hover:from-amber-500/25 hover:to-yellow-500/25 text-amber-300 border border-amber-500/40 text-xs font-semibold rounded-lg transition flex items-center space-x-1.5 shadow-xs active:scale-95"
        title="Buka dialog drawing Best of the Best (BOB)"
      >
        <span>🏆</span>
        <span>Drawing BOB</span>
      </button>

      {#if !isEditMode}
        <!-- Edit Mode Button -->
        <button
          type="button"
          onclick={enterEditMode}
          class="px-3 py-1.5 {isDarkMode ? 'bg-amber-500/15 hover:bg-amber-500/25 text-amber-300 border-amber-500/40' : 'bg-amber-50 hover:bg-amber-100 text-amber-800 border-amber-300'} text-xs font-semibold border rounded-lg transition flex items-center space-x-1.5 shadow-xs active:scale-95"
          title="Ubah nama bagan dan susunan atlet"
        >
          <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
            <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M11 5H6a2 2 0 00-2 2v11a2 2 0 002 2h11a2 2 0 002-2v-5m-1.414-9.414a2 2 0 112.828 2.828L11.828 15H9v-2.828l8.586-8.586z" />
          </svg>
          <span>Edit Bagan</span>
        </button>
        {#if !isFestivalBagan && !isBobBagan && deleteCategoryId}
          <button
            type="button"
            onclick={openDeleteBagan}
            class="px-3 py-1.5 {isDarkMode ? 'bg-red-500/10 hover:bg-red-500/20 text-red-300 border-red-500/30' : 'bg-red-50 hover:bg-red-100 text-red-700 border-red-300'} text-xs font-semibold border rounded-lg transition flex items-center space-x-1.5 shadow-xs active:scale-95"
            title="Hapus bagan nomor tanding ini (atlet tetap terdaftar, bisa diundi lagi)"
          >
            <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24"><path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M19 7l-.867 12.142A2 2 0 0116.138 21H7.862a2 2 0 01-1.995-1.858L5 7m5 4v6m4-6v6m1-10V4a1 1 0 00-1-1h-4a1 1 0 00-1 1v3M4 7h16"/></svg>
            <span>Hapus Bagan</span>
          </button>
        {/if}
      {:else}
        <!-- Edit Mode Active Actions -->
        <div class="flex items-center gap-1.5">
          <span class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-amber-500/20 text-amber-300 border border-amber-500/40 text-[11px] font-bold">
            <span class="w-2 h-2 rounded-full bg-amber-400 animate-pulse"></span>
            <span>Mode Edit</span>
          </span>
          {#if !isFestivalBagan}
            <button
              type="button"
              onclick={openLateModal}
              disabled={isSavingEdit}
              class="px-3 py-1.5 {isDarkMode ? 'bg-white/[0.05] hover:bg-white/[0.08] text-zinc-200 border-white/[0.1]' : 'bg-white hover:bg-slate-50 text-slate-700 border-slate-300'} text-xs font-semibold border rounded-lg transition active:scale-95 disabled:opacity-50"
              title="Daftarkan atlet yang datang terlambat lalu taruh di slot kosong"
            >
              + Atlet Terlambat
            </button>
          {/if}
          <button
            type="button"
            onclick={cancelEditMode}
            disabled={isSavingEdit}
            class="px-3 py-1.5 {isDarkMode ? 'bg-white/[0.05] hover:bg-white/[0.08] text-zinc-300 border-white/[0.1]' : 'bg-white hover:bg-slate-50 text-slate-700 border-slate-300'} text-xs font-semibold border rounded-lg transition flex items-center space-x-1 shadow-xs active:scale-95 disabled:opacity-50"
          >
            <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M6 18L18 6M6 6l12 12" />
            </svg>
            <span>Batal</span>
          </button>
          <button
            type="button"
            onclick={saveEditMode}
            disabled={isSavingEdit}
            class="px-3.5 py-1.5 bg-emerald-600 hover:bg-emerald-500 text-white text-xs font-bold rounded-lg transition flex items-center space-x-1.5 shadow-sm active:scale-95 disabled:opacity-50"
          >
            {#if isSavingEdit}
              <div class="w-3.5 h-3.5 border-2 border-white border-t-transparent rounded-full animate-spin"></div>
              <span>Menyimpan...</span>
            {:else}
              <svg class="w-3.5 h-3.5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M5 13l4 4L19 7" />
              </svg>
              <span>Simpan</span>
            {/if}
          </button>
        </div>
      {/if}
    </div>
  </div>

  <!-- Tournament Header Banner -->
  <div class="bracket-banner relative flex items-center justify-center pb-2.5 pt-1 border-b-2 {isDarkMode ? 'border-white/[0.15]' : 'border-slate-900'} mb-3 shrink-0">
    <!-- Top-Left Official Logo Cluster: FORKI & Ambrilindo -->
    <div class="banner-logos-left absolute left-1 top-2 flex items-center space-x-2 shrink-0 bg-white/95 dark:bg-white p-1 rounded shadow-xs border border-slate-200 dark:border-zinc-700 z-10">
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
      {#if isEditMode}
        <div class="flex items-center justify-center gap-2 max-w-xl mx-auto">
          {#if currentBaganInfo?.kode || baganInfo?.kode}
            <span class="text-xs font-mono font-bold {isDarkMode ? 'text-amber-400 bg-amber-500/15' : 'text-amber-800 bg-amber-100'} px-2 py-1 rounded border border-amber-500/30 shrink-0">
              {currentBaganInfo?.kode || baganInfo?.kode}
            </span>
          {/if}
          <input
            type="text"
            bind:value={editBaganName}
            placeholder="Nama Bagan..."
            class="w-full text-center text-sm md:text-base font-black px-3 py-1 rounded border-2 border-amber-500 focus:outline-none focus:ring-2 focus:ring-amber-400 transition {isDarkMode ? 'bg-black/60 text-white' : 'bg-white text-slate-900 shadow-inner'}"
          />
        </div>
      {:else}
        <h1 class="text-base md:text-lg font-black {isDarkMode ? 'text-white' : 'text-slate-950'} tracking-tight uppercase leading-tight truncate">
          {#if currentBaganInfo?.kode || baganInfo?.kode}
            ({currentBaganInfo?.kode || baganInfo?.kode})
          {/if}
          {displayBaganName}
        </h1>
      {/if}
      {#if isFestivalBagan}
        <!-- Pure Judul Saja: Subtext festival dihilangkan sesuai instruksi operator -->
      {:else}
        <p class="text-[11px] font-bold {isDarkMode ? 'text-zinc-200' : 'text-slate-800'} mt-0.5 flex items-center justify-center gap-1.5 flex-wrap">
          <span>{eventName || "AMBRILINDO KARATE CHAMPIONSHIP"}</span>
          <span>•</span>
          <span>{eventVenue}</span>
          {#if eventDate}
            <span>•</span>
            <span>{eventDate}</span>
          {/if}
        </p>
      {/if}
    </div>

    <!-- Right Match Counter & Action -->
    <div class="banner-info-wrapper absolute right-0 top-1/2 -translate-y-1/2 flex justify-end items-center text-right text-[10px] {isDarkMode ? 'text-zinc-400 font-bold' : 'text-slate-600 font-bold'} font-mono no-print z-10 gap-2 pr-2">
      {#if isFestivalBagan}
        <!-- Template Layout Selector -->
        <div class="flex items-center space-x-1">
          <span class="text-[10px] text-zinc-400 font-sans hidden sm:inline">Template:</span>
          <select
            value={currentBaganInfo?.layout_type || "quadrant"}
            onchange={(e) => handleUpdateLayoutType(e.currentTarget.value)}
            class="h-7 px-2 text-[10.5px] font-bold rounded-lg border {isDarkMode ? 'bg-[#181a20] text-amber-300 border-white/20' : 'bg-white text-slate-800 border-slate-300 shadow-xs'} outline-none cursor-pointer"
            title="Pilih Template Layout Bagan Festival"
          >
            <option value="quadrant">⊞ 4-Quadrant Grid (Default)</option>
            <option value="bands">☰ Horizontal Bands (Baris Lebar)</option>
          </select>
        </div>

        <button
          type="button"
          onclick={printBagan}
          class="px-2.5 py-1 rounded-lg bg-emerald-500 hover:bg-emerald-400 text-black font-bold text-xs transition flex items-center space-x-1.5 shadow-sm active:scale-95 cursor-pointer"
          title="Cetak format kertas A4 Landscape (1 Lembar Presisi)"
        >
          <svg class="w-3.5 h-3.5 fill-current" viewBox="0 0 24 24">
            <path d="M19 8H5c-1.66 0-3 1.34-3 3v6h4v4h12v-4h4v-6c0-1.66-1.34-3-3-3zm-3 11H8v-5h8v5zm3-7c-.55 0-1-.45-1-1s.45-1 1-1 1 .45 1 1-.45 1-1 1zm-1-9H6v4h12V3z"/>
          </svg>
          <span>Cetak (A4)</span>
        </button>
      {/if}
      <span>{isFestivalBagan ? `${festivalGroups.length * 3} Partai` : `${elimMatchesCount} Partai`}</span>
    </div>
  </div>

  {#if isEditMode && unplacedAthletes.length > 0}
    <div class="no-print mb-2 px-3 py-2 rounded-xl border text-xs {isDarkMode ? 'bg-rose-500/10 border-rose-500/30 text-rose-200' : 'bg-rose-50 border-rose-200 text-rose-800'}">
      <span class="font-bold">Belum ada di bagan ({unplacedAthletes.length}):</span>
      {unplacedAthletes.map((a) => a.nama_atlet).join(", ")}
      <span class="opacity-75"> — pilih di slot kosong (BYE) yang partainya belum dimainkan.</span>
    </div>
  {/if}

  <!-- Interactive Bracket Tree Viewport -->
  <div
    bind:this={viewportEl}
    onwheel={handleWheel}
    class="bracket-viewport flex-1 overflow-x-auto overflow-y-auto py-2 px-1 scrollbar-thin relative {isDarkMode ? 'bg-[#101216] border-white/[0.06]' : 'bg-white/70 border-slate-200/80'} rounded-xl border"
  >
    {#if isLoading}
      <div class="h-64 flex items-center justify-center text-sm {isDarkMode ? 'text-zinc-400' : 'text-slate-500'} font-medium">
        Memuat struktur bagan turnamen...
      </div>
    {:else if isRoundRobin && roundRobinData}
      <!-- Round-Robin Tournament Manager View -->
      <div class="round-robin-container w-full min-h-full p-4 space-y-6">
        <!-- Header Card -->
        <div class="rounded-xl border p-4 {isDarkMode ? 'bg-[#181a20] border-white/10' : 'bg-white border-slate-200'} flex flex-col md:flex-row items-start md:items-center justify-between gap-4 shadow-xs">
          <div>
            <div class="flex items-center gap-2">
              <span class="px-2 py-0.5 rounded text-[10px] font-bold font-mono uppercase bg-indigo-500/20 text-indigo-400 border border-indigo-500/30">
                SISTEM ROUND-ROBIN (BERGER)
              </span>
              {#if roundRobinData.is_completed}
                <span class="px-2 py-0.5 rounded text-[10px] font-bold font-mono uppercase bg-emerald-500/20 text-emerald-400 border border-emerald-500/30 flex items-center gap-1">
                  <span class="w-1.5 h-1.5 rounded-full bg-emerald-400"></span>
                  KOMPETISI SELESAI (PODIUM SAH)
                </span>
              {:else}
                <span class="px-2 py-0.5 rounded text-[10px] font-bold font-mono uppercase bg-amber-500/20 text-amber-400 border border-amber-500/30 flex items-center gap-1">
                  <span class="w-1.5 h-1.5 rounded-full bg-amber-400 animate-pulse"></span>
                  PERTANDINGAN BERJALAN
                </span>
              {/if}
            </div>
            <h2 class="text-base font-black mt-1 {isDarkMode ? 'text-white' : 'text-slate-900'}">
              {roundRobinData.nomor_tanding_name || displayBaganName}
            </h2>
            <p class="text-xs {isDarkMode ? 'text-zinc-400' : 'text-slate-500'}">
              {roundRobinData.athletes.length} Atlet • {roundRobinData.total_matches} Total Pertandingan
            </p>
          </div>

          <!-- Progress Meter -->
          <div class="w-full md:w-64 space-y-1.5">
            <div class="flex items-center justify-between text-xs font-mono">
              <span class="{isDarkMode ? 'text-zinc-400' : 'text-slate-600'}">Progres Partai</span>
              <span class="font-bold {isDarkMode ? 'text-white' : 'text-slate-800'}">
                {roundRobinData.finished_matches} / {roundRobinData.total_matches} ({roundRobinData.total_matches > 0 ? Math.round((roundRobinData.finished_matches / roundRobinData.total_matches) * 100) : 0}%)
              </span>
            </div>
            <div class="w-full h-2 rounded-full overflow-hidden {isDarkMode ? 'bg-zinc-800' : 'bg-slate-200'}">
              <div
                class="h-full bg-linear-to-r from-amber-500 to-emerald-500 transition-all duration-300"
                style="width: {roundRobinData.total_matches > 0 ? (roundRobinData.finished_matches / roundRobinData.total_matches * 100) : 0}%;"
              ></div>
            </div>
          </div>
        </div>

        <!-- Podium Leaderboard (Juara 1, 2, 3A, 3B) -->
        <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
          <!-- Gold -->
          <div class="rounded-xl border p-3 flex items-center space-x-3 {isDarkMode ? 'bg-[#181a20] border-amber-500/30' : 'bg-amber-50/70 border-amber-200'} shadow-xs">
            <div class="w-10 h-10 rounded-xl bg-amber-500/20 text-amber-400 border border-amber-500/30 flex items-center justify-center font-black text-sm shrink-0">
              🥇 1
            </div>
            <div class="min-w-0 flex-1">
              <div class="text-[10px] font-bold text-amber-500 uppercase tracking-wider">JUARA 1 (EMAS)</div>
              <div class="text-xs font-bold truncate {isDarkMode ? 'text-white' : 'text-slate-900'}">
                {roundRobinData.standings[0]?.athlete_name || "—"}
              </div>
              <div class="text-[10px] truncate {isDarkMode ? 'text-zinc-400' : 'text-slate-500'}">
                {roundRobinData.standings[0]?.utusan_name || "—"}
              </div>
            </div>
          </div>

          <!-- Silver -->
          <div class="rounded-xl border p-3 flex items-center space-x-3 {isDarkMode ? 'bg-[#181a20] border-zinc-400/30' : 'bg-slate-50 border-slate-300'} shadow-xs">
            <div class="w-10 h-10 rounded-xl bg-zinc-500/20 text-zinc-300 border border-zinc-400/30 flex items-center justify-center font-black text-sm shrink-0">
              🥈 2
            </div>
            <div class="min-w-0 flex-1">
              <div class="text-[10px] font-bold {isDarkMode ? 'text-zinc-300' : 'text-slate-600'} uppercase tracking-wider">JUARA 2 (PERAK)</div>
              <div class="text-xs font-bold truncate {isDarkMode ? 'text-white' : 'text-slate-900'}">
                {roundRobinData.standings[1]?.athlete_name || "—"}
              </div>
              <div class="text-[10px] truncate {isDarkMode ? 'text-zinc-400' : 'text-slate-500'}">
                {roundRobinData.standings[1]?.utusan_name || "—"}
              </div>
            </div>
          </div>

          <!-- Bronze 3A -->
          <div class="rounded-xl border p-3 flex items-center space-x-3 {isDarkMode ? 'bg-[#181a20] border-amber-700/30' : 'bg-orange-50/60 border-orange-200'} shadow-xs">
            <div class="w-10 h-10 rounded-xl bg-amber-700/20 text-amber-600 border border-amber-700/30 flex items-center justify-center font-black text-sm shrink-0">
              🥉 3A
            </div>
            <div class="min-w-0 flex-1">
              <div class="text-[10px] font-bold text-amber-600 uppercase tracking-wider">JUARA 3 BERSAMA</div>
              <div class="text-xs font-bold truncate {isDarkMode ? 'text-white' : 'text-slate-900'}">
                {roundRobinData.standings[2]?.athlete_name || "—"}
              </div>
              <div class="text-[10px] truncate {isDarkMode ? 'text-zinc-400' : 'text-slate-500'}">
                {roundRobinData.standings[2]?.utusan_name || "—"}
              </div>
            </div>
          </div>

          <!-- Bronze 3B -->
          <div class="rounded-xl border p-3 flex items-center space-x-3 {isDarkMode ? 'bg-[#181a20] border-amber-700/30' : 'bg-orange-50/60 border-orange-200'} shadow-xs">
            <div class="w-10 h-10 rounded-xl bg-amber-700/20 text-amber-600 border border-amber-700/30 flex items-center justify-center font-black text-sm shrink-0">
              🥉 3B
            </div>
            <div class="min-w-0 flex-1">
              <div class="text-[10px] font-bold text-amber-600 uppercase tracking-wider">JUARA 3 BERSAMA</div>
              <div class="text-xs font-bold truncate {isDarkMode ? 'text-white' : 'text-slate-900'}">
                {roundRobinData.standings[3]?.athlete_name || "—"}
              </div>
              <div class="text-[10px] truncate {isDarkMode ? 'text-zinc-400' : 'text-slate-500'}">
                {roundRobinData.standings[3]?.utusan_name || "—"}
              </div>
            </div>
          </div>
        </div>

        <!-- Standings Table & Cross-Table Grid -->
        <div class="grid grid-cols-1 lg:grid-cols-12 gap-6">
          <!-- Standings Table (7 cols) -->
          <div class="lg:col-span-7 rounded-xl border overflow-hidden {isDarkMode ? 'bg-[#181a20] border-white/10' : 'bg-white border-slate-200'} shadow-xs flex flex-col">
            <div class="px-4 py-3 border-b {isDarkMode ? 'border-white/10 bg-white/[0.02]' : 'border-slate-200 bg-slate-50'} flex items-center justify-between">
              <h3 class="text-xs font-black uppercase tracking-wider font-mono {isDarkMode ? 'text-white' : 'text-slate-800'}">
                KLASEMEN SEMENTARA / STANDINGS
              </h3>
              <span class="text-[10px] font-mono {isDarkMode ? 'text-zinc-400' : 'text-slate-500'}">
                3 Poin / Menang • 1 Poin / Seri
              </span>
            </div>
            <div class="overflow-x-auto">
              <table class="w-full text-left text-xs">
                <thead class="text-[10px] uppercase font-mono font-bold {isDarkMode ? 'bg-black/30 text-zinc-400 border-b border-white/5' : 'bg-slate-100 text-slate-600 border-b border-slate-200'}">
                  <tr>
                    <th class="py-2.5 px-3 text-center w-10">Pos</th>
                    <th class="py-2.5 px-3">Atlet / Kontingen</th>
                    <th class="py-2.5 px-2 text-center">M</th>
                    <th class="py-2.5 px-2 text-center text-emerald-500">W</th>
                    <th class="py-2.5 px-2 text-center text-amber-500">D</th>
                    <th class="py-2.5 px-2 text-center text-rose-500">L</th>
                    <th class="py-2.5 px-2 text-center font-mono">PF</th>
                    <th class="py-2.5 px-2 text-center font-mono">PA</th>
                    <th class="py-2.5 px-2 text-center font-mono">+/-</th>
                    <th class="py-2.5 px-3 text-center font-mono font-bold text-amber-400">PTS</th>
                  </tr>
                </thead>
                <tbody class="divide-y {isDarkMode ? 'divide-white/5' : 'divide-slate-100'}">
                  {#each roundRobinData.standings as s}
                    <tr class="transition {isDarkMode ? 'hover:bg-white/[0.03]' : 'hover:bg-slate-50/80'} {s.rank === 1 ? (isDarkMode ? 'bg-amber-500/5' : 'bg-amber-50/40') : ''}">
                      <td class="py-2.5 px-3 text-center font-mono font-bold">
                        {#if s.rank === 1}
                          <span class="text-amber-400">🥇 1</span>
                        {:else if s.rank === 2}
                          <span class="text-zinc-300">🥈 2</span>
                        {:else if s.rank === 3}
                          <span class="text-amber-600">🥉 3</span>
                        {:else}
                          <span class="{isDarkMode ? 'text-zinc-500' : 'text-slate-400'}">{s.rank}</span>
                        {/if}
                      </td>
                      <td class="py-2.5 px-3">
                        <div class="font-bold {isDarkMode ? 'text-white' : 'text-slate-900'}">{s.athlete_name}</div>
                        <div class="text-[10px] {isDarkMode ? 'text-zinc-400' : 'text-slate-500'} truncate">
                          {s.utusan_name || s.perguruan_name || "—"}
                        </div>
                      </td>
                      <td class="py-2.5 px-2 text-center font-mono {isDarkMode ? 'text-zinc-300' : 'text-slate-700'}">{s.played}</td>
                      <td class="py-2.5 px-2 text-center font-mono font-bold text-emerald-500">{s.won}</td>
                      <td class="py-2.5 px-2 text-center font-mono font-bold text-amber-500">{s.drawn}</td>
                      <td class="py-2.5 px-2 text-center font-mono font-bold text-rose-500">{s.lost}</td>
                      <td class="py-2.5 px-2 text-center font-mono">{s.points_for}</td>
                      <td class="py-2.5 px-2 text-center font-mono">{s.points_against}</td>
                      <td class="py-2.5 px-2 text-center font-mono font-semibold {s.points_diff > 0 ? 'text-emerald-400' : s.points_diff < 0 ? 'text-rose-400' : (isDarkMode ? 'text-zinc-400' : 'text-slate-500')}">
                        {s.points_diff > 0 ? `+${s.points_diff}` : s.points_diff}
                      </td>
                      <td class="py-2.5 px-3 text-center font-mono font-black text-sm {isDarkMode ? 'text-amber-400 bg-amber-500/10' : 'text-amber-700 bg-amber-100/50'}">
                        {s.standings_points}
                      </td>
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          </div>

          <!-- Cross-Table Matrix (5 cols) -->
          <div class="lg:col-span-5 rounded-xl border overflow-hidden {isDarkMode ? 'bg-[#181a20] border-white/10' : 'bg-white border-slate-200'} shadow-xs flex flex-col">
            <div class="px-4 py-3 border-b {isDarkMode ? 'border-white/10 bg-white/[0.02]' : 'border-slate-200 bg-slate-50'} flex items-center justify-between">
              <h3 class="text-xs font-black uppercase tracking-wider font-mono {isDarkMode ? 'text-white' : 'text-slate-800'}">
                MATRIKS SKOR CROSS-TABLE
              </h3>
              <span class="text-[10px] font-mono {isDarkMode ? 'text-zinc-400' : 'text-slate-500'}">
                Klik sel untuk skor
              </span>
            </div>
            <div class="p-3 overflow-x-auto">
              <table class="w-full text-center text-[11px] border-collapse">
                <thead>
                  <tr>
                    <th class="p-2 border {isDarkMode ? 'border-white/10 text-zinc-400' : 'border-slate-200 text-slate-500'} text-left text-[10px] font-mono uppercase">Atlet</th>
                    {#each roundRobinData.standings as s}
                      <th class="p-2 border {isDarkMode ? 'border-white/10' : 'border-slate-200'} font-mono text-[10px] font-bold max-w-[60px] truncate" title={s.athlete_name}>
                        #{s.rank}
                      </th>
                    {/each}
                  </tr>
                </thead>
                <tbody>
                  {#each roundRobinData.matrix_rows as row, rIdx}
                    {@const standing = roundRobinData.standings[rIdx]}
                    <tr>
                      <td class="p-2 border {isDarkMode ? 'border-white/10' : 'border-slate-200'} text-left font-bold text-xs max-w-[120px] truncate" title={standing?.athlete_name}>
                        <span class="font-mono text-[10px] text-amber-500 mr-1">#{standing?.rank}</span>
                        {standing?.athlete_name.split(" ")[0]}
                      </td>
                      {#each row as cell}
                        {#if cell.is_self}
                          <td class="p-2 border {isDarkMode ? 'border-white/10 bg-zinc-800/80 text-zinc-600' : 'border-slate-200 bg-slate-200 text-slate-400'} font-black text-sm select-none">
                            ✕
                          </td>
                        {:else if cell.match_id}
                          {@const matchItem = matches.find((m: DetailBaganItem) => m.id === cell.match_id)}
                          <td
                            class="p-1 border {isDarkMode ? 'border-white/10' : 'border-slate-200'} cursor-pointer hover:ring-2 hover:ring-amber-400 transition"
                            onclick={() => {
                              if (matchItem) {
                                activeMatchForControl = matchItem;
                              }
                            }}
                          >
                            {#if cell.is_finished}
                              <div class="px-1.5 py-1 rounded font-mono font-bold text-[11px] flex items-center justify-center gap-1 {cell.row_result === 'W' ? 'bg-emerald-500/20 text-emerald-400 border border-emerald-500/40' : cell.row_result === 'L' ? 'bg-rose-500/20 text-rose-400 border border-rose-500/40' : 'bg-amber-500/20 text-amber-300 border border-amber-500/40'}">
                                <span>{cell.row_result}</span>
                                <span>{cell.row_score || 0}-{cell.col_score || 0}</span>
                              </div>
                            {:else}
                              <div class="px-1 py-1 rounded font-mono text-[10px] {isDarkMode ? 'bg-white/[0.04] text-zinc-400 hover:bg-white/[0.08]' : 'bg-slate-100 text-slate-600 hover:bg-slate-200'}">
                                R{cell.round}-M{cell.urutan}
                              </div>
                            {/if}
                          </td>
                        {:else}
                          <td class="p-2 border {isDarkMode ? 'border-white/10 text-zinc-600' : 'border-slate-200 text-slate-400'} font-mono text-[10px]">—</td>
                        {/if}
                      {/each}
                    </tr>
                  {/each}
                </tbody>
              </table>
            </div>
          </div>
        </div>

        <!-- Match Schedule List (Round by Round) -->
        <div class="rounded-xl border overflow-hidden {isDarkMode ? 'bg-[#181a20] border-white/10' : 'bg-white border-slate-200'} shadow-xs p-4">
          <div class="flex items-center justify-between pb-3 mb-4 border-b {isDarkMode ? 'border-white/10' : 'border-slate-200'}">
            <div>
              <h3 class="text-xs font-black uppercase tracking-wider font-mono {isDarkMode ? 'text-white' : 'text-slate-800'}">
                JADWAL DAN HASIL PERTANDINGAN
              </h3>
              <p class="text-[10px] {isDarkMode ? 'text-zinc-400' : 'text-slate-500'}">
                Urutan partai putaran Round-Robin
              </p>
            </div>
          </div>

          <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
            {#each matches as m}
              <div
                role="button"
                tabindex="0"
                class="rounded-xl border p-3 flex flex-col justify-between transition hover:shadow-md cursor-pointer {isDarkMode ? 'bg-[#14161b] border-white/10 hover:border-amber-500/40' : 'bg-slate-50 border-slate-200 hover:border-amber-400'}"
                onclick={() => { activeMatchForControl = m; }}
                onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); activeMatchForControl = m; } }}
              >
                <div class="flex items-center justify-between text-[10px] font-mono mb-2 pb-1 border-b {isDarkMode ? 'border-white/5 text-zinc-400' : 'border-slate-200 text-slate-500'}">
                  <span class="font-bold text-amber-500">Ronde {m.round} • Partai #{m.urutan}</span>
                  {#if m.selesai}
                    <span class="px-1.5 py-0.5 rounded font-bold uppercase bg-emerald-500/20 text-emerald-400 border border-emerald-500/30">
                      Selesai
                    </span>
                  {:else if m.assigned_tatami_id}
                    <span class="px-1.5 py-0.5 rounded font-bold uppercase bg-cyan-500/20 text-cyan-400 border border-cyan-500/30">
                      Tatami {resolveTatamiNumber(m.assigned_tatami_id)}
                    </span>
                  {:else}
                    <span class="px-1.5 py-0.5 rounded uppercase {isDarkMode ? 'bg-zinc-800 text-zinc-400' : 'bg-slate-200 text-slate-600'}">
                      Antrean
                    </span>
                  {/if}
                </div>

                <!-- Aka vs Ao -->
                <div class="space-y-1.5 text-xs">
                  <!-- Aka -->
                  <div class="flex items-center justify-between p-1.5 rounded {m.pemenang === '1' ? (isDarkMode ? 'bg-red-500/20 border border-red-500/40 font-bold' : 'bg-red-100 border border-red-300 font-bold') : (isDarkMode ? 'bg-white/[0.02]' : 'bg-white')}">
                    <div class="flex items-center gap-1.5 min-w-0">
                      <span class="w-2 h-2 rounded-full bg-red-500 shrink-0"></span>
                      <span class="truncate {isDarkMode ? 'text-zinc-200' : 'text-slate-800'}">{m.atlet1_nama || `Atlet #${m.atlet1_id}`}</span>
                    </div>
                    <span class="font-mono font-black text-sm px-1.5">{m.score1 ?? "-"}</span>
                  </div>

                  <!-- Ao -->
                  <div class="flex items-center justify-between p-1.5 rounded {m.pemenang === '2' ? (isDarkMode ? 'bg-blue-500/20 border border-blue-500/40 font-bold' : 'bg-blue-100 border border-blue-300 font-bold') : (isDarkMode ? 'bg-white/[0.02]' : 'bg-white')}">
                    <div class="flex items-center gap-1.5 min-w-0">
                      <span class="w-2 h-2 rounded-full bg-blue-500 shrink-0"></span>
                      <span class="truncate {isDarkMode ? 'text-zinc-200' : 'text-slate-800'}">{m.atlet2_nama || `Atlet #${m.atlet2_id}`}</span>
                    </div>
                    <span class="font-mono font-black text-sm px-1.5">{m.score2 ?? "-"}</span>
                  </div>
                </div>

                <div class="mt-3 pt-2 border-t {isDarkMode ? 'border-white/5' : 'border-slate-200'} flex items-center justify-between text-[10px]">
                  <span class="{isDarkMode ? 'text-zinc-500' : 'text-slate-400'} font-mono">{m.kode || `P-${m.id}`}</span>
                  <span class="font-bold text-amber-500 hover:underline flex items-center gap-0.5">
                    Buka Skor →
                  </span>
                </div>
              </div>
            {/each}
          </div>
        </div>
      </div>
    {:else if isFestivalBagan}
      <!-- Snippet: Standard Karate Match Card for Festival -->
      {#snippet festivalMatchCard(m: DetailBaganItem | null, stageLabel: string, isFinal: boolean = false)}
        {#if m}
          {@const isByeMatch = !isFinal && (!m.atlet1_id || !m.atlet2_id)}
          {@const isWaitingFinalists = isFinal && !m.atlet1_id && !m.atlet2_id}
          <div
            class="match-card festival-match-card w-full {isWaitingFinalists ? 'cursor-not-allowed opacity-85' : isByeMatch ? 'cursor-default' : 'cursor-pointer hover:ring-2 hover:ring-amber-500/80'} transition-all {isDarkMode ? 'bg-[#181a20] border-zinc-700 hover:border-zinc-500' : 'bg-white border-slate-400/90 hover:border-slate-800'} border rounded-md overflow-hidden shadow-xs relative z-10 {isFinal ? (isDarkMode ? 'border-amber-500/80 shadow-[0_0_10px_rgba(245,158,11,0.15)]' : 'border-amber-500 shadow-xs') : ''}"
            onclick={() => {
              if (isWaitingFinalists) {
                alert('Pertandingan final belum dapat dibuka karena pemenang semifinal belum ditentukan.');
                return;
              }
              if (!isByeMatch) handleOpenMatchControl(m);
            }}
            role="button"
            tabindex="0"
            onkeydown={(e) => {
              if (e.key === 'Enter') {
                if (isWaitingFinalists) {
                  alert('Pertandingan final belum dapat dibuka karena pemenang semifinal belum ditentukan.');
                  return;
                }
                if (!isByeMatch) handleOpenMatchControl(m);
              }
            }}
            title={isWaitingFinalists ? 'Menunggu pemenang semifinal' : isByeMatch ? 'Pertandingan BYE' : 'Klik untuk membuka Kontrol Pertandingan (Control Panel & Scoreboard)'}
          >
            <!-- Match Card Header Bar -->
            <div class="card-header-bar flex items-center justify-between px-1.5 h-[14px] {isDarkMode ? 'bg-[#101215] border-white/[0.12] text-zinc-100' : 'bg-slate-200 border-slate-400 text-slate-950'} border-b text-[8px] font-black font-mono tracking-wider">
              <span class="flex items-center space-x-1.5">
                {#if isFinal}
                  <span class="final-title {isDarkMode ? 'text-amber-300' : 'text-amber-800'} font-black flex items-center gap-1">
                    <svg class="w-2.5 h-2.5 fill-current shrink-0" viewBox="0 0 24 24">
                      <path d="M19 5h-2V3H7v2H5c-1.1 0-2 .9-2 2v1c0 2.55 1.92 4.63 4.39 4.94A5.01 5.01 0 0011 15.9V19H7v2h10v-2h-4v-3.1a5.01 5.01 0 003.61-2.96C19.08 12.63 21 10.55 21 8V7c0-1.1-.9-2-2-2zM5 8V7h2v3.82C5.84 10.4 5 9.3 5 8zm14 0c0 1.3-.84 2.4-2 2.82V7h2v1z"/>
                    </svg>
                    <span>FINAL</span>
                  </span>
                {:else}
                  <span class="font-black">{stageLabel}</span>
                {/if}
                <span class="text-zinc-400 font-normal">#{m.urutan || m.id}</span>
              </span>
              {#if isByeMatch}
                <span class="text-[7.5px] font-black font-mono text-amber-500 dark:text-amber-400 bg-amber-500/15 border border-amber-500/30 px-1 py-0.2 rounded tracking-wide">
                  BYE
                </span>
              {:else if m.selesai}
                <span class="text-[7.5px] font-black text-emerald-400 bg-emerald-950/40 border border-emerald-500/30 px-1 rounded">
                  SELESAI
                </span>
              {:else if liveSyncVersion >= 0 && isMatchLive(m.id, tatamis)}
                {@const liveTNum = getRunningTatamiForMatch(m.id, tatamis) || resolveTatamiNumber(m.assigned_tatami_id)}
                <span class="text-[7.5px] font-black text-white bg-red-600 px-1 py-0.2 rounded tracking-wide animate-pulse shadow-xs">
                  LIVE T{liveTNum}
                </span>
              {/if}
            </div>

            <!-- AKA (Red) Row -->
            <div class="player-row {m.pemenang === '1' ? 'player-row-winner ' + (isDarkMode ? 'bg-amber-500/15 border-l-4 border-amber-500' : 'bg-[#fef3c7] border-l-4 border-amber-500') : (isDarkMode ? 'bg-transparent text-zinc-100' : 'bg-white text-slate-900')} flex items-center justify-between px-1.5 py-[1px] min-h-[17px] transition relative">
              <div class="flex items-center space-x-1.5 mr-1 min-w-0 flex-1">
                <span class="side-pill-aka w-[22px] inline-flex items-center justify-center text-[7px] font-mono font-black text-white bg-[#dc2626] py-[0.5px] rounded shrink-0 tracking-wider shadow-xs">
                  AKA
                </span>
                {#if m.atlet1_logo && logoMap[m.atlet1_logo]}
                  <img
                    src={logoMap[m.atlet1_logo]}
                    alt="Logo"
                    class="player-dojo-logo w-3.5 h-3.5 max-w-3.5 max-h-3.5 object-contain shrink-0"
                  />
                {/if}
                <div class="min-w-0 flex-1 leading-tight">
                  <div class="flex items-center space-x-1 min-w-0">
                    {#if m.pemenang === '1'}
                      <span class="text-amber-400 shrink-0" title="Pemenang">
                        <svg class="w-2.5 h-2.5 fill-current" viewBox="0 0 24 24">
                          <path d="M5 16L3 5l5.5 5L12 4l3.5 6L21 5l-2 11H5zm14 3c0 .6-.4 1-1 1H6c-.6 0-1-.4-1-1v-1h14v1z"/>
                        </svg>
                      </span>
                    {/if}
                    <span class="player-name-text truncate block text-[9px] leading-[11px] {m.atlet1_nama ? (isDarkMode ? (m.pemenang === '1' ? 'font-black text-amber-300' : 'font-black text-white') : 'font-black text-black') : (isDarkMode ? 'italic text-zinc-400 font-bold' : 'italic text-slate-600 font-bold')}">
                      {m.atlet1_nama || (m.atlet1_id ? `Atlet #${m.atlet1_id}` : (isFinal ? '...' : 'BYE'))}
                    </span>
                  </div>
                  {#if m.atlet1_nama}
                    <div class="player-sub-text truncate min-w-0 text-[7.5px] font-bold leading-[9px] mt-[0.5px] {isDarkMode ? 'text-zinc-200' : 'text-slate-800'}">
                      {m.atlet1_kontingen || '-'}
                    </div>
                  {:else if !isFinal && m.atlet1_id}
                    <div class="player-sub-text truncate min-w-0 text-[7.5px] font-bold leading-[9px] mt-[0.5px] {isDarkMode ? 'text-zinc-400' : 'text-slate-500'}">
                      —
                    </div>
                  {/if}
                </div>
              </div>
              <div class="shrink-0 flex items-center">
                <span class="score-box min-w-[18px] h-[15px] px-0.5 font-mono text-[8.5px] flex items-center justify-center rounded border {m.pemenang === '1' ? (isDarkMode ? 'bg-amber-400/20 border-amber-400 text-amber-300 font-black' : 'bg-[#fde047] border-amber-500 text-amber-950 font-black') : (isDarkMode ? 'bg-white/[0.08] border-white/20 text-white font-black' : 'bg-slate-100 border-slate-400 text-black font-black')}">
                  {m.score1 ?? '-'}
                </span>
              </div>
            </div>

            <!-- AO (Blue) Row -->
            <div class="player-row {m.pemenang === '2' ? 'player-row-winner ' + (isDarkMode ? 'bg-amber-500/15 border-l-4 border-amber-500' : 'bg-[#fef3c7] border-l-4 border-amber-500') : (isDarkMode ? 'bg-transparent text-zinc-100' : 'bg-white text-slate-900')} flex items-center justify-between px-1.5 py-[1px] min-h-[17px] transition relative border-t {isDarkMode ? 'border-zinc-700/60' : 'border-slate-300'}">
              <div class="flex items-center space-x-1.5 mr-1 min-w-0 flex-1">
                <span class="side-pill-ao w-[22px] inline-flex items-center justify-center text-[7px] font-mono font-black text-white bg-[#2563eb] py-[0.5px] rounded shrink-0 tracking-wider shadow-xs">
                  AO
                </span>
                {#if m.atlet2_logo && logoMap[m.atlet2_logo]}
                  <img
                    src={logoMap[m.atlet2_logo]}
                    alt="Logo"
                    class="player-dojo-logo w-3.5 h-3.5 max-w-3.5 max-h-3.5 object-contain shrink-0"
                  />
                {/if}
                <div class="min-w-0 flex-1 leading-tight">
                  <div class="flex items-center space-x-1 min-w-0">
                    {#if m.pemenang === '2'}
                      <span class="text-amber-400 shrink-0" title="Pemenang">
                        <svg class="w-2.5 h-2.5 fill-current" viewBox="0 0 24 24">
                          <path d="M5 16L3 5l5.5 5L12 4l3.5 6L21 5l-2 11H5zm14 3c0 .6-.4 1-1 1H6c-.6 0-1-.4-1-1v-1h14v1z"/>
                        </svg>
                      </span>
                    {/if}
                    <span class="player-name-text truncate block text-[9px] leading-[11px] {m.atlet2_nama ? (isDarkMode ? (m.pemenang === '2' ? 'font-black text-amber-300' : 'font-black text-white') : 'font-black text-black') : (isDarkMode ? 'italic text-zinc-400 font-bold' : 'italic text-slate-600 font-bold')}">
                      {m.atlet2_nama || (m.atlet2_id ? `Atlet #${m.atlet2_id}` : (isFinal ? '...' : 'BYE'))}
                    </span>
                  </div>
                  {#if m.atlet2_nama}
                    <div class="player-sub-text truncate min-w-0 text-[7.5px] font-bold leading-[9px] mt-[0.5px] {isDarkMode ? 'text-zinc-200' : 'text-slate-800'}">
                      {m.atlet2_kontingen || '-'}
                    </div>
                  {:else if !isFinal && m.atlet2_id}
                    <div class="player-sub-text truncate min-w-0 text-[7.5px] font-bold leading-[9px] mt-[0.5px] {isDarkMode ? 'text-zinc-400' : 'text-slate-500'}">
                      —
                    </div>
                  {/if}
                </div>
              </div>
              <div class="shrink-0 flex items-center">
                <span class="score-box min-w-[18px] h-[15px] px-0.5 font-mono text-[8.5px] flex items-center justify-center rounded border {m.pemenang === '2' ? (isDarkMode ? 'bg-amber-400/20 border-amber-400 text-amber-300 font-black' : 'bg-[#fde047] border-amber-500 text-amber-950 font-black') : (isDarkMode ? 'bg-white/[0.08] border-white/20 text-white font-black' : 'bg-slate-100 border-slate-400 text-black font-black')}">
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

      <!-- Snippet: Group Quadrant / Band View -->
            <div class="festival-canvas-wrapper w-full h-full max-w-[1064px] mx-auto p-1 overflow-hidden">
        <FestivalBracketSheet
          {baganId}
          baganKode={currentBaganInfo?.kode || baganInfo?.kode}
          categoryName={displayBaganName}
          discipline={displayBaganName.toLowerCase().includes("kata") ? "kata" : "kumite"}
          poolCount={festivalGroupCount}
          athletesCount={matches.length > 0 ? new Set(matches.flatMap(m => [m.atlet1_id, m.atlet2_id].filter(Boolean))).size : festivalGroupCount * 4}
          venue={eventVenue}
          {matches}
          layoutTemplate={currentBaganInfo?.layout_type || "quadrant"}
          showHeader={false}
          interactive={true}
          {isDarkMode}
          onMatchClick={handleOpenMatchControl}
        />
      </div>
    {:else if elimRounds.length === 0}
      <div class="h-64 flex flex-col items-center justify-center text-center">
        <p class="text-sm font-bold {isDarkMode ? 'text-zinc-300' : 'text-slate-700'}">Belum ada partai untuk bagan ini</p>
        <p class="text-xs {isDarkMode ? 'text-zinc-500' : 'text-slate-400'} mt-1 font-normal">Pengundian nomor tanding belum dilakukan.</p>
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
          <BracketTree
            {matches}
            meta={bracketMeta}
            {podium}
            theme={isDarkMode ? "dark" : "light"}
            interactive
            {logoMap}
            {isEditMode}
            bind:editedMatchups
            {categoryAthletes}
            {tatamis}
            {liveSyncVersion}
            onMatchClick={handleOpenMatchControl}
            bind:gridEl
          />
        </div>
      </div>
    {/if}
  </div>
</div>
{/if}

<!-- Print-only page: identical to the bracket pages in the results/tatami booklets -->
{#if !isRoundRobin && matches.length > 0}
  <div class="bagan-print-page" aria-hidden="true">
    <BracketPaperPage
      kind={isFestivalBagan ? "festival" : "elim"}
      {matches}
      categoryName={displayBaganName}
      baganKode={currentBaganInfo?.kode || baganInfo?.kode}
      {eventName}
      {eventVenue}
      {eventDate}
      {logoMap}
      meta={bracketMeta}
      {podium}
      festivalGroupCount={festivalGroupCount}
      festivalLayout={currentBaganInfo?.layout_type || "quadrant"}
      widthMm={287}
      heightMm={200}
    />
  </div>
{/if}

<!-- Festival Quick Score Modal -->
{#if festivalScoreModalMatch}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/75 backdrop-blur-xs animate-fade-in no-print">
    <div class="bg-[#181a20] border border-white/15 rounded-2xl w-full max-w-md shadow-2xl overflow-hidden font-sans text-zinc-200">
      <div class="px-5 py-3.5 border-b border-white/10 flex items-center justify-between bg-[#20232b]">
        <div class="flex items-center space-x-2">
          <span class="px-2 py-0.5 rounded text-[10px] font-mono font-bold bg-emerald-500/20 text-emerald-400 border border-emerald-500/30">
            {festivalScoreModalMatch.round === 2 ? 'FINAL' : 'SEMIFINAL'}
          </span>
          <h3 class="text-xs font-bold text-white uppercase">
            Input Hasil Partai #{festivalScoreModalMatch.urutan}
          </h3>
        </div>
        <button
          onclick={() => { festivalScoreModalMatch = null; }}
          class="text-zinc-400 hover:text-white p-1 rounded"
        >
          ✕
        </button>
      </div>

      <div class="p-5 space-y-4">
        <!-- Aka (Red) Selection -->
        <label
          class="flex items-center justify-between p-3 rounded-xl border cursor-pointer transition {festWinner === '1' ? 'bg-red-500/15 border-red-500/50' : 'bg-white/[0.02] border-white/10 hover:bg-white/[0.04]'}"
        >
          <div class="flex items-center space-x-2.5 min-w-0">
            <input
              type="radio"
              bind:group={festWinner}
              value="1"
              class="accent-red-500 w-4 h-4"
            />
            <div class="min-w-0">
              <span class="text-[10px] font-mono font-bold text-red-400 uppercase block">AKA (MERAH)</span>
              <span class="text-xs font-bold text-white truncate block">
                {festivalScoreModalMatch.atlet1_nama || '—'}
              </span>
              <span class="text-[10px] text-zinc-400 block truncate">
                {festivalScoreModalMatch.atlet1_kontingen || ''}
              </span>
            </div>
          </div>
          <input
            type="text"
            bind:value={festScore1}
            class="w-14 text-center text-sm font-mono font-black py-1.5 rounded-lg bg-black/60 border border-white/20 text-white outline-none focus:border-red-400"
          />
        </label>

        <!-- Ao (Blue) Selection -->
        <label
          class="flex items-center justify-between p-3 rounded-xl border cursor-pointer transition {festWinner === '2' ? 'bg-blue-500/15 border-blue-500/50' : 'bg-white/[0.02] border-white/10 hover:bg-white/[0.04]'}"
        >
          <div class="flex items-center space-x-2.5 min-w-0">
            <input
              type="radio"
              bind:group={festWinner}
              value="2"
              class="accent-blue-500 w-4 h-4"
            />
            <div class="min-w-0">
              <span class="text-[10px] font-mono font-bold text-blue-400 uppercase block">AO (BIRU)</span>
              <span class="text-xs font-bold text-white truncate block">
                {festivalScoreModalMatch.atlet2_nama || '—'}
              </span>
              <span class="text-[10px] text-zinc-400 block truncate">
                {festivalScoreModalMatch.atlet2_kontingen || ''}
              </span>
            </div>
          </div>
          <input
            type="text"
            bind:value={festScore2}
            class="w-14 text-center text-sm font-mono font-black py-1.5 rounded-lg bg-black/60 border border-white/20 text-white outline-none focus:border-blue-400"
          />
        </label>
      </div>

      <div class="px-5 py-3.5 border-t border-white/10 flex items-center justify-between bg-[#20232b]">
        <button
          type="button"
          onclick={resetFestMatch}
          disabled={isSavingFestMatch}
          class="px-3 py-1.5 text-xs text-rose-400 hover:text-rose-300 font-semibold rounded-lg hover:bg-rose-500/10 transition"
        >
          Reset Partai
        </button>

        <div class="flex items-center space-x-2">
          <button
            type="button"
            onclick={() => { festivalScoreModalMatch = null; }}
            class="px-3 py-1.5 text-xs text-zinc-400 hover:text-white rounded-lg transition"
          >
            Batal
          </button>
          <button
            type="button"
            onclick={saveFestScore}
            disabled={isSavingFestMatch}
            class="px-4 py-1.5 text-xs font-bold rounded-lg bg-emerald-500 hover:bg-emerald-400 text-black shadow-md transition"
          >
            {isSavingFestMatch ? 'Menyimpan...' : 'Simpan Pemenang'}
          </button>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- Best of the Best (BOB) Drawing Modal -->
<BobDrawingModal
  eventId={currentBaganInfo?.event_id || 1}
  isOpen={showBobModal}
  onClose={() => (showBobModal = false)}
  onSuccess={(result) => {
    showBobModal = false;
    baganId = result.bagan_id;
    baganName = result.nama_bob;
    if (onBaganUpdated) onBaganUpdated();
    loadBracketData();
  }}
/>

{#if lateModalOpen}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs no-print" role="dialog" aria-modal="true">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl p-6 shadow-2xl w-full max-w-md text-zinc-200">
      <h3 class="text-base font-black text-white uppercase tracking-tight">Atlet Terlambat</h3>
      <div class="mt-4 space-y-3">
        <label class="block text-[11px] font-mono text-zinc-400 uppercase">
          Nama atlet
          <input
            type="text"
            bind:value={lateName}
            class="mt-1 w-full bg-[#14161b] text-sm text-white px-3 py-2 rounded-xl border border-white/[0.1] focus:border-white/40 outline-none uppercase"
          />
        </label>
        <label class="block text-[11px] font-mono text-zinc-400 uppercase">
          Kontingen
          <select bind:value={lateUtusanId} class="mt-1 w-full bg-[#14161b] text-sm text-white px-3 py-2 rounded-xl border border-white/[0.1] outline-none">
            <option value={null}>—</option>
            {#each lateUtusanList as u (u.id)}<option value={u.id}>{u.nama_utusan}</option>{/each}
          </select>
        </label>
        <label class="block text-[11px] font-mono text-zinc-400 uppercase">
          Perguruan
          <select bind:value={latePerguruanId} class="mt-1 w-full bg-[#14161b] text-sm text-white px-3 py-2 rounded-xl border border-white/[0.1] outline-none">
            <option value={null}>—</option>
            {#each latePerguruanList as p (p.id)}<option value={p.id}>{p.nama_perguruan}</option>{/each}
          </select>
        </label>
        {#if lateError}
          <div class="text-xs text-rose-300 font-semibold">{lateError}</div>
        {/if}
      </div>
      <div class="flex justify-end gap-2 mt-6">
        <button type="button" onclick={() => (lateModalOpen = false)} class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl">Batal</button>
        <button type="button" onclick={saveLateAthlete} disabled={lateSaving} class="h-9 px-4 bg-white text-black hover:bg-zinc-200 font-bold text-xs rounded-xl shadow-md disabled:opacity-50">
          {lateSaving ? "Menyimpan..." : "Tambahkan"}
        </button>
      </div>
    </div>
  </div>
{/if}

<style>
  .bagan-print-page {
    display: none;
  }

  @media print {
    @page {
      size: A4 landscape;
      margin: 5mm;
    }
    :global(.no-print) {
      display: none !important;
    }
    /* The screen view stays on screen; the paper page is what gets printed. */
    .print-sheet.has-paper-print {
      display: none !important;
    }
    .bagan-print-page {
      display: block !important;
      position: fixed;
      top: 0;
      left: 0;
    }
    /* Round robin prints its screen view: let it flow instead of clipping to the viewport. */
    .print-sheet:not(.has-paper-print),
    .print-sheet:not(.has-paper-print) .bracket-viewport {
      height: auto !important;
      overflow: visible !important;
      padding: 0 !important;
      border: none !important;
      background: #ffffff !important;
    }
  }

  /* Screen Classes for Festival Bands & Quadrants */
  .festival-bands-grid {
    display: flex;
    flex-direction: column;
    height: 100%;
    width: 100%;
  }
  .festival-quadrants-grid {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    height: 100%;
    width: 100%;
  }
  .festival-quadrants-grid-4 {
    grid-template-rows: repeat(2, 1fr) !important;
  }
  .festival-quadrants-grid-6 {
    grid-template-rows: repeat(3, 1fr) !important;
  }
  .group-band {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    min-height: 200px;
  }
  .group-quadrant {
    flex: 1;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    min-height: 170px;
    overflow: hidden;
  }
  .band-grid-2col,
  .quadrant-grid-2col {
    display: flex;
    align-items: stretch;
    justify-content: space-between;
    gap: 0.25rem;
    flex: 1;
  }
  .band-col-semi,
  .band-col-final {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
  }
  .festival-match-card {
    min-height: 54px;
  }
  .connector-slot {
    height: 54px;
    min-height: 54px;
    max-height: 54px;
  }
  .connector-spine-track {
    top: 27px;
    bottom: 27px;
  }
  .quadrant-connector-fork {
    width: 24px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
  }
  .band-connector-fork {
    width: 36px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
  }
</style>

{#if showDeleteBagan}
  <div class="fixed inset-0 z-50 flex items-center justify-center p-4 bg-black/70 backdrop-blur-xs font-sans">
    <div class="bg-[#181a20] border border-white/10 rounded-2xl p-6 shadow-2xl w-full max-w-md text-zinc-200 space-y-4">
      <h3 class="text-sm font-black uppercase tracking-wider font-mono text-white">Hapus bagan?</h3>
      <p class="text-xs text-zinc-300 leading-relaxed">
        Bagan <strong class="text-white">{baganName}</strong> akan dihapus, termasuk pool dan final nomor ini bila ada.
        Atletnya tetap terdaftar dan nomor ini bisa diundi lagi dari menu Drawing.
      </p>
      {#if deleteBaganError}
        <p class="text-xs text-red-300">{deleteBaganError}</p>
      {/if}
      {#if deleteNeedsResultsConfirm}
        <label class="flex items-start gap-2.5 p-3 rounded-xl bg-red-500/10 border border-red-500/25 cursor-pointer">
          <input type="checkbox" bind:checked={deleteResultsConfirmed} class="mt-0.5 w-4 h-4 rounded text-red-500 focus:ring-0" />
          <span class="text-xs text-red-300">
            Nomor ini sudah bertanding{finishedHere > 0 ? ` (${finishedHere} partai selesai di bagan ini)` : ""}. Saya paham hasil partai tersebut ikut terhapus.
          </span>
        </label>
      {/if}
      <div class="flex justify-end gap-2 pt-1">
        <button
          type="button"
          onclick={() => (showDeleteBagan = false)}
          disabled={isDeletingBagan}
          class="h-9 px-3.5 bg-white/[0.04] text-zinc-200 border border-white/[0.1] font-semibold text-xs rounded-xl"
        >
          Batal
        </button>
        <button
          type="button"
          onclick={deleteThisBagan}
          disabled={isDeletingBagan || (deleteNeedsResultsConfirm && !deleteResultsConfirmed)}
          class="h-9 px-4 bg-red-600 hover:bg-red-500 text-white font-bold text-xs rounded-xl shadow-md disabled:opacity-40"
        >
          {isDeletingBagan ? "Menghapus…" : "Hapus Bagan"}
        </button>
      </div>
    </div>
  </div>
{/if}
