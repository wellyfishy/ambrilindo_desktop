<script lang="ts">
  import { onMount, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import type {
    DetailBaganItem,
    BaganItem,
    SaveMatchInput,
    BaganPodium,
    PodiumAthlete,
    EnrichedAtletItem,
    UpdateMatchAthletesInput,
  } from "$lib/types";
  import ControlPanelView from "$lib/components/ControlPanelView.svelte";

  interface Props {
    baganId: number;
    baganName: string;
    baganInfo?: BaganItem | null;
    eventName?: string;
    eventVenue?: string;
    eventDate?: string;
    hostUrl?: string;
    onDispatchToTatami?: (tatamiNum: number, matchId: number) => void;
    onBaganUpdated?: () => void;
  }

  let {
    baganId,
    baganName,
    baganInfo = null,
    eventName = "",
    eventVenue = "Venue Pertandingan",
    eventDate = "",
    hostUrl = "",
    onDispatchToTatami,
    onBaganUpdated,
  }: Props = $props();

  let matches = $state<DetailBaganItem[]>([]);
  let isLoading = $state(true);
  let logoMap = $state<Record<string, string>>({});
  let activeMatchForControl = $state<DetailBaganItem | null>(null);

  let currentBaganInfo = $state<BaganItem | null>(null);
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

  let dbPodium = $state<BaganPodium | null>(null);

  async function handleSaveMatchResult(input: SaveMatchInput) {
    try {
      await invoke("save_match_result", { input });
      activeMatchForControl = null;
      await loadBracketData();
    } catch (err: any) {
      console.error("Gagal menyimpan hasil partai:", err);
      // Fallback for browser mode / test harness
      const m = matches.find((x) => x.id === input.match_id);
      if (m) {
        m.score1 = input.score1;
        m.score2 = input.score2;
        m.pemenang = input.pemenang;
        m.selesai = input.selesai;
        m.kata1 = input.kata1 || null;
        m.kata2 = input.kata2 || null;
        m.vr1 = input.vr1;
        m.vr2 = input.vr2;
        m.hantei = input.hantei;
      }
      activeMatchForControl = null;
    }
  }

  async function handleOpenMatchControl(match: DetailBaganItem) {
    try {
      await invoke("open_control_panel_window", {
        matchId: match.id,
        baganId: match.bagan_id || baganId,
        tatami: match.assigned_tatami_id || 1,
      });
    } catch (err) {
      console.warn("Tauri open_control_panel_window failed, fallback to window.open or in-page:", err);
      if (typeof window !== "undefined" && window.open) {
        window.open(
          `/control-panel?matchId=${match.id}&baganId=${match.bagan_id || baganId}&tatami=${match.assigned_tatami_id || 1}`,
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
      console.warn("Tauri update_bagan_details failed (fallback in-memory):", err);
      if (currentBaganInfo) {
        currentBaganInfo.nama_bagan = editBaganName.trim() || currentBaganInfo.nama_bagan;
      }
      for (const [mIdStr, val] of Object.entries(editedMatchups)) {
        const m = matches.find((x) => x.id === Number(mIdStr));
        if (m) {
          const a1Id = val.atlet1_id ? Number(val.atlet1_id) : null;
          const a2Id = val.atlet2_id ? Number(val.atlet2_id) : null;
          m.atlet1_id = a1Id;
          m.atlet2_id = a2Id;
          const foundA1 = categoryAthletes.find((a) => a.id === a1Id);
          if (foundA1) {
            m.atlet1_nama = foundA1.nama_atlet;
            m.atlet1_kontingen = foundA1.perguruan_nama || foundA1.utusan_nama || null;
            m.atlet1_logo = foundA1.utusan_logo || null;
            m.atlet1_is_priority = foundA1.is_priority;
          } else if (!a1Id) {
            m.atlet1_nama = null;
            m.atlet1_kontingen = null;
            m.atlet1_logo = null;
            m.atlet1_is_priority = false;
          }
          const foundA2 = categoryAthletes.find((a) => a.id === a2Id);
          if (foundA2) {
            m.atlet2_nama = foundA2.nama_atlet;
            m.atlet2_kontingen = foundA2.perguruan_nama || foundA2.utusan_nama || null;
            m.atlet2_logo = foundA2.utusan_logo || null;
            m.atlet2_is_priority = foundA2.is_priority;
          } else if (!a2Id) {
            m.atlet2_nama = null;
            m.atlet2_kontingen = null;
            m.atlet2_logo = null;
            m.atlet2_is_priority = false;
          }
        }
      }
      isEditMode = false;
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

  // Zoom & Viewport adaptivity state
  let zoom = $state(1.0);
  let userHasZoomed = $state(false);
  let contentWidth = $state(1064);
  let contentHeight = $state(692);

  let viewportEl = $state<HTMLDivElement | null>(null);
  let svgEl = $state<SVGSVGElement | null>(null);
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
    } catch (err) {
      console.error("Gagal memuat detail bagan:", err);
      // Fallback mock data matching tournament Kata Kadet Putra for test & dev environments
      if (typeof window !== "undefined" && !(window as any).__TAURI_INTERNALS__) {
        matches = [
          // Round 1 (Babak 16 Besar - 8 matches)
          { id: 1, bagan_id: 2, round: 1, urutan: 1, atlet1_id: 1, atlet2_id: 2, score1: "0", score2: "5", pemenang: "2", selesai: true, kata1: "4 - Aoyagi", kata2: "6 - Bassai Dai", team: false, hantei: false, vr1: false, vr2: false, kode: "M-01", scorekecil1: null, scorekecil2: null, assigned_tatami_id: 1, atlet1_nama: "BAGAS SATRIA WIJAYA", atlet1_kontingen: "INKAI • DOJO BUSHIDO SAMARINDA", atlet2_nama: "GUNTUR BUMI PERSADA", atlet2_kontingen: "KKI • KKI KUTAI TIMUR" },
          { id: 2, bagan_id: 2, round: 1, urutan: 2, atlet1_id: 3, atlet2_id: 4, score1: "2", score2: "3", pemenang: "2", selesai: true, kata1: "4 - Aoyagi", kata2: "7 - Bassai Sho", team: false, hantei: false, vr1: false, vr2: false, kode: "M-02", scorekecil1: null, scorekecil2: null, assigned_tatami_id: 1, atlet1_nama: "BRYAN NICHOLAS SETIAWAN", atlet1_kontingen: "LEMKARI • KUTAI KARTANEGARA", atlet2_nama: "ZULFIKAR AKBAR", atlet2_kontingen: "GOJU RYU • BALIKPAPAN" },
          { id: 3, bagan_id: 2, round: 1, urutan: 3, atlet1_id: 5, atlet2_id: 6, score1: "0", score2: "5", pemenang: "2", selesai: true, kata1: "2 - Anan Dai", kata2: "7 - Bassai Sho", team: false, hantei: false, vr1: false, vr2: false, kode: "M-03", scorekecil1: null, scorekecil2: null, assigned_tatami_id: 1, atlet1_nama: "NAUFAL AZIZ PRADANA", atlet1_kontingen: "INKAI • SAMARINDA SEBERANG", atlet2_nama: "PRADIPTA ARYA", atlet2_kontingen: "WADOKAI • WADOKAI BONTANG" },
          { id: 4, bagan_id: 2, round: 1, urutan: 4, atlet1_id: 7, atlet2_id: 8, score1: "5", score2: "0", pemenang: "1", selesai: true, kata1: "8 - Chatanyara Kusanku", kata2: "2 - Anan Dai", team: false, hantei: false, vr1: false, vr2: false, kode: "M-04", scorekecil1: null, scorekecil2: null, assigned_tatami_id: 1, atlet1_nama: "FAREL AL GHAZALI", atlet1_kontingen: "SHINDOKA • SHINDOKA BERAU", atlet2_nama: "SULTAN HAIDAR ALI", atlet2_kontingen: "PORBBIKO • PORBBIKO PASER" },
          { id: 5, bagan_id: 2, round: 1, urutan: 5, atlet1_id: 9, atlet2_id: 10, score1: "-", score2: "-", pemenang: null, selesai: false, kata1: null, kata2: null, team: false, hantei: false, vr1: false, vr2: false, kode: "M-05", scorekecil1: null, scorekecil2: null, assigned_tatami_id: 1, atlet1_nama: "FANDI AHMAD MAULANA", atlet1_kontingen: "BKC • BKC KOTA SAMARINDA", atlet2_nama: "WIDODO CAHYONO", atlet2_kontingen: "KKI • KKI KUTAI TIMUR" },
          { id: 6, bagan_id: 2, round: 1, urutan: 6, atlet1_id: 11, atlet2_id: 12, score1: "-", score2: "-", pemenang: null, selesai: false, kata1: null, kata2: null, team: false, hantei: false, vr1: false, vr2: false, kode: "M-06", scorekecil1: null, scorekecil2: null, assigned_tatami_id: 1, atlet1_nama: "RAFI AHMAD KURNIAWAN", atlet1_kontingen: "PORBBIKO • PORBBIKO PASER", atlet2_nama: "GILANG PRASETYO", atlet2_kontingen: "SHINDOKA • SHINDOKA BERAU" },
          { id: 7, bagan_id: 2, round: 1, urutan: 7, atlet1_id: 13, atlet2_id: 14, score1: "-", score2: "-", pemenang: null, selesai: false, kata1: null, kata2: null, team: false, hantei: false, vr1: false, vr2: false, kode: "M-07", scorekecil1: null, scorekecil2: null, assigned_tatami_id: 1, atlet1_nama: "LEONARDUS PRATAMA", atlet1_kontingen: "INKANAS • INKANAS BALIKPAPAN", atlet2_nama: "FADIL RAMADHAN", atlet2_kontingen: "LEMKARI • KUTAI KARTANEGARA" },
          { id: 8, bagan_id: 2, round: 1, urutan: 8, atlet1_id: 15, atlet2_id: 16, score1: "-", score2: "-", pemenang: null, selesai: false, kata1: null, kata2: null, team: false, hantei: false, vr1: false, vr2: false, kode: "M-08", scorekecil1: null, scorekecil2: null, assigned_tatami_id: 1, atlet1_nama: "NABIL FATHURRAHMAN", atlet1_kontingen: "INKAI • DOJO BUSHIDO SAMARINDA", atlet2_nama: "ALDY REVALDY", atlet2_kontingen: "WADOKAI • WADOKAI BONTANG" },

          // Round 2 (Perempat Final - 4 matches)
          { id: 9, bagan_id: 2, round: 2, urutan: 1, atlet1_id: 2, atlet2_id: 4, score1: "-", score2: "-", pemenang: null, selesai: false, kata1: null, kata2: null, team: false, hantei: false, vr1: false, vr2: false, kode: "M-09", scorekecil1: null, scorekecil2: null, assigned_tatami_id: 1, atlet1_nama: "GUNTUR BUMI PERSADA", atlet1_kontingen: "KKI • KKI KUTAI TIMUR", atlet2_nama: "ZULFIKAR AKBAR", atlet2_kontingen: "GOJU RYU • BALIKPAPAN" },
          { id: 10, bagan_id: 2, round: 2, urutan: 2, atlet1_id: 6, atlet2_id: 7, score1: "-", score2: "-", pemenang: null, selesai: false, kata1: null, kata2: null, team: false, hantei: false, vr1: false, vr2: false, kode: "M-10", scorekecil1: null, scorekecil2: null, assigned_tatami_id: 1, atlet1_nama: "PRADIPTA ARYA", atlet1_kontingen: "WADOKAI • WADOKAI BONTANG", atlet2_nama: "FAREL AL GHAZALI", atlet2_kontingen: "SHINDOKA • SHINDOKA BERAU" },
          { id: 11, bagan_id: 2, round: 2, urutan: 3, atlet1_id: null, atlet2_id: null, score1: "-", score2: "-", pemenang: null, selesai: false, kata1: null, kata2: null, team: false, hantei: false, vr1: false, vr2: false, kode: "M-11", scorekecil1: null, scorekecil2: null, assigned_tatami_id: 1, atlet1_nama: "BYE", atlet1_kontingen: "—", atlet2_nama: "BYE", atlet2_kontingen: "—" },
          { id: 12, bagan_id: 2, round: 2, urutan: 4, atlet1_id: null, atlet2_id: null, score1: "-", score2: "-", pemenang: null, selesai: false, kata1: null, kata2: null, team: false, hantei: false, vr1: false, vr2: false, kode: "M-12", scorekecil1: null, scorekecil2: null, assigned_tatami_id: 1, atlet1_nama: "BYE", atlet1_kontingen: "—", atlet2_nama: "BYE", atlet2_kontingen: "—" },

          // Round 3 (Semi Final - 2 matches)
          { id: 13, bagan_id: 2, round: 3, urutan: 1, atlet1_id: null, atlet2_id: null, score1: "-", score2: "-", pemenang: null, selesai: false, kata1: null, kata2: null, team: false, hantei: false, vr1: false, vr2: false, kode: "M-13", scorekecil1: null, scorekecil2: null, assigned_tatami_id: 1, atlet1_nama: "BYE", atlet1_kontingen: "—", atlet2_nama: "BYE", atlet2_kontingen: "—" },
          { id: 14, bagan_id: 2, round: 3, urutan: 2, atlet1_id: null, atlet2_id: null, score1: "-", score2: "-", pemenang: null, selesai: false, kata1: null, kata2: null, team: false, hantei: false, vr1: false, vr2: false, kode: "M-14", scorekecil1: null, scorekecil2: null, assigned_tatami_id: 1, atlet1_nama: "BYE", atlet1_kontingen: "—", atlet2_nama: "BYE", atlet2_kontingen: "—" },

          // Round 4 (Partai Final - 1 match)
          { id: 15, bagan_id: 2, round: 4, urutan: 1, atlet1_id: null, atlet2_id: null, score1: "-", score2: "-", pemenang: null, selesai: false, kata1: null, kata2: null, team: false, hantei: false, vr1: false, vr2: false, kode: "M-15", scorekecil1: null, scorekecil2: null, assigned_tatami_id: 1, atlet1_nama: "BYE", atlet1_kontingen: "—", atlet2_nama: "BYE", atlet2_kontingen: "—" },
        ];
        currentBaganInfo = {
          id: 2,
          event_id: 1,
          nama_bagan: "KATA PERORANGAN KADET PUTRA",
          nomor_tanding_id: 2,
          tipe_tanding: "1",
          juara_1_id: null,
          juara_2_id: null,
          juara_3a_id: null,
          juara_3b_id: null,
          peringkat_5_id: null,
          peringkat_6_id: null,
          peringkat_7_id: null,
          peringkat_8_id: null,
          round_robin: false,
          pool: 1,
          kode: "002",
          is_bob: false,
          has_vr: false,
        };
      } else {
        matches = [];
      }
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

  function getRoundBadge(roundNum: number, totalElimRounds: number): string {
    if (poolCount === 2 && !isFinalBagan) {
      if (roundNum === 4) return "SF";
      if (roundNum === 3) return "R3";
      if (roundNum === 2) return "R2";
      return "R1";
    }
    if (poolCount === 4 && !isFinalBagan) {
      if (roundNum === 4) return "R4";
      if (roundNum === 3) return "R3";
      if (roundNum === 2) return "R2";
      return "R1";
    }
    if (roundNum === 4) return "FN";
    if (roundNum === 3) return totalElimRounds >= 4 ? "SF" : "FN";
    if (roundNum === 2) return totalElimRounds >= 4 ? "R2" : "SF";
    return "R1";
  }

  function getRoundTitle(roundNum: number, totalElimRounds: number): string {
    if (poolCount === 2 && !isFinalBagan) {
      if (roundNum === 4) return "SEMI FINAL";
      if (roundNum === 3) return "BABAK 8 BESAR";
      if (roundNum === 2) return "BABAK 16 BESAR";
      return "BABAK 32 BESAR";
    }
    if (poolCount === 4 && !isFinalBagan) {
      if (roundNum === 4) return "BABAK 8 BESAR";
      if (roundNum === 3) return "BABAK 16 BESAR";
      if (roundNum === 2) return "BABAK 32 BESAR";
      return "BABAK 64 BESAR";
    }
    if (roundNum === 4) return "PARTAI FINAL";
    if (roundNum === 3) return totalElimRounds >= 4 ? "SEMI FINAL" : "PARTAI FINAL";
    if (roundNum === 2) return totalElimRounds >= 4 ? "PEREMPAT FINAL" : "SEMI FINAL";
    return "BABAK 16 BESAR";
  }

  function getSlotCountForRound(roundNum: number): number {
    switch (roundNum) {
      case 1: return 8;
      case 2: return 4;
      case 3: return 2;
      case 4: return 1;
      default: return 1;
    }
  }

  function getSlotHeightClass(roundNum: number): string {
    switch (roundNum) {
      case 1: return "h-[82px]";
      case 2: return "h-[164px]";
      case 3: return "h-[328px]";
      case 4: return "h-[656px]";
      default: return "h-[82px]";
     }
   }

  function getMatchForSlot(roundMatches: DetailBaganItem[], slotNum: number): DetailBaganItem | undefined {
    return roundMatches.find((m) => (m.urutan || 0) === slotNum) || roundMatches[slotNum - 1];
  }

  // Calculate podium / Hasil Juara standings (Juara 1, 2, 3A, 3B for Juara 3 Bersama)
  let podium = $derived.by(() => {
    // 1. Initial values from dbPodium if loaded from backend
    let j1 = dbPodium?.j1
      ? {
          name: dbPodium.j1.name,
          contingent: dbPodium.j1.contingent,
          logo: dbPodium.j1.logo,
          is_priority: dbPodium.j1.is_priority,
        }
      : { name: "Belum Ditentukan", contingent: "—", logo: null as string | null, is_priority: false };

    let j2 = dbPodium?.j2
      ? {
          name: dbPodium.j2.name,
          contingent: dbPodium.j2.contingent,
          logo: dbPodium.j2.logo,
          is_priority: dbPodium.j2.is_priority,
        }
      : { name: "Belum Ditentukan", contingent: "—", logo: null as string | null, is_priority: false };

    let j3a = dbPodium?.j3a
      ? {
          name: dbPodium.j3a.name,
          contingent: dbPodium.j3a.contingent,
          logo: dbPodium.j3a.logo,
          is_priority: dbPodium.j3a.is_priority,
        }
      : { name: "Belum Ditentukan", contingent: "—", logo: null as string | null, is_priority: false };

    let j3b = dbPodium?.j3b
      ? {
          name: dbPodium.j3b.name,
          contingent: dbPodium.j3b.contingent,
          logo: dbPodium.j3b.logo,
          is_priority: dbPodium.j3b.is_priority,
        }
      : { name: "Belum Ditentukan", contingent: "—", logo: null as string | null, is_priority: false };

    // 2. Real-time in-memory calculation from matches (reactive instant update)
    if (poolCount === 2 && !isFinalBagan) {
      // 2-pool preliminary: Round 4 is the pool Semifinal!
      const semiMatch = matches.find((m) => m.round === 4);
      if (semiMatch && semiMatch.selesai && semiMatch.pemenang) {
        const loserIsA1 = semiMatch.pemenang === "2";
        const loserName = loserIsA1
          ? semiMatch.atlet1_nama || (semiMatch.atlet1_id ? `Atlet #${semiMatch.atlet1_id}` : "BYE")
          : semiMatch.atlet2_nama || (semiMatch.atlet2_id ? `Atlet #${semiMatch.atlet2_id}` : "BYE");
        const loserContingent = loserIsA1 ? semiMatch.atlet1_kontingen || "—" : semiMatch.atlet2_kontingen || "—";
        const loserLogo = loserIsA1 ? semiMatch.atlet1_logo || null : semiMatch.atlet2_logo || null;
        const loserPriority = !!(loserIsA1 ? semiMatch.atlet1_is_priority : semiMatch.atlet2_is_priority);

        const isPoolA = (baganName || "").toLowerCase().includes("pool a") || (baganName || "").toLowerCase().includes("pool 1");
        if (isPoolA) {
          if (j3a.name === "Belum Ditentukan") {
            j3a = { name: loserName, contingent: loserContingent, logo: loserLogo, is_priority: loserPriority };
          }
        } else {
          if (j3b.name === "Belum Ditentukan") {
            j3b = { name: loserName, contingent: loserContingent, logo: loserLogo, is_priority: loserPriority };
          }
        }
      }
    } else {
      // Single pool or Final bagan:
      const finalMatch = matches.find((m) => m.round === 4) || elimRounds[elimRounds.length - 1]?.[1]?.[0];
      const semiMatches = matches
        .filter((m) => m.round === 3)
        .sort((a, b) => (a.urutan || 0) - (b.urutan || 0));
      const semi1 = semiMatches[0];
      const semi2 = semiMatches[1];

      if (finalMatch && finalMatch.selesai) {
        if (finalMatch.pemenang === "1") {
          if (j1.name === "Belum Ditentukan") {
            j1 = {
              name: finalMatch.atlet1_nama || (finalMatch.atlet1_id ? `Atlet #${finalMatch.atlet1_id}` : "BYE"),
              contingent: finalMatch.atlet1_kontingen || "—",
              logo: finalMatch.atlet1_logo || null,
              is_priority: !!finalMatch.atlet1_is_priority,
            };
          }
          if (j2.name === "Belum Ditentukan") {
            j2 = {
              name: finalMatch.atlet2_nama || (finalMatch.atlet2_id ? `Atlet #${finalMatch.atlet2_id}` : "BYE"),
              contingent: finalMatch.atlet2_kontingen || "—",
              logo: finalMatch.atlet2_logo || null,
              is_priority: !!finalMatch.atlet2_is_priority,
            };
          }
        } else if (finalMatch.pemenang === "2") {
          if (j1.name === "Belum Ditentukan") {
            j1 = {
              name: finalMatch.atlet2_nama || (finalMatch.atlet2_id ? `Atlet #${finalMatch.atlet2_id}` : "BYE"),
              contingent: finalMatch.atlet2_kontingen || "—",
              logo: finalMatch.atlet2_logo || null,
              is_priority: !!finalMatch.atlet2_is_priority,
            };
          }
          if (j2.name === "Belum Ditentukan") {
            j2 = {
              name: finalMatch.atlet1_nama || (finalMatch.atlet1_id ? `Atlet #${finalMatch.atlet1_id}` : "BYE"),
              contingent: finalMatch.atlet1_kontingen || "—",
              logo: finalMatch.atlet1_logo || null,
              is_priority: !!finalMatch.atlet1_is_priority,
            };
          }
        }
      }

      // Semifinal 1 loser -> Juara 3A
      if (semi1 && semi1.selesai) {
        if (semi1.pemenang === "1") {
          if (j3a.name === "Belum Ditentukan") {
            j3a = {
              name: semi1.atlet2_nama || (semi1.atlet2_id ? `Atlet #${semi1.atlet2_id}` : "BYE"),
              contingent: semi1.atlet2_kontingen || "—",
              logo: semi1.atlet2_logo || null,
              is_priority: !!semi1.atlet2_is_priority,
            };
          }
        } else if (semi1.pemenang === "2") {
          if (j3a.name === "Belum Ditentukan") {
            j3a = {
              name: semi1.atlet1_nama || (semi1.atlet1_id ? `Atlet #${semi1.atlet1_id}` : "BYE"),
              contingent: semi1.atlet1_kontingen || "—",
              logo: semi1.atlet1_logo || null,
              is_priority: !!semi1.atlet1_is_priority,
            };
          }
        }
      }

      // Semifinal 2 loser -> Juara 3B
      if (semi2 && semi2.selesai) {
        if (semi2.pemenang === "1") {
          if (j3b.name === "Belum Ditentukan") {
            j3b = {
              name: semi2.atlet2_nama || (semi2.atlet2_id ? `Atlet #${semi2.atlet2_id}` : "BYE"),
              contingent: semi2.atlet2_kontingen || "—",
              logo: semi2.atlet2_logo || null,
              is_priority: !!semi2.atlet2_is_priority,
            };
          }
        } else if (semi2.pemenang === "2") {
          if (j3b.name === "Belum Ditentukan") {
            j3b = {
              name: semi2.atlet1_nama || (semi2.atlet1_id ? `Atlet #${semi2.atlet1_id}` : "BYE"),
              contingent: semi2.atlet1_kontingen || "—",
              logo: semi2.atlet1_logo || null,
              is_priority: !!semi2.atlet1_is_priority,
            };
          }
        }
      }
    }

    return { j1, j2, j3a, j3b };
  });

  interface ConnectorLine {
    x1: number;
    y1: number;
    x2: number;
    y2: number;
  }

  // Exact mathematical bracket connectors (zero DOM measurement jitter, 100% print/screen alignment)
  let connectorLines = $derived.by(() => {
    const list: ConnectorLine[] = [];
    if (elimRounds.length < 2) return list;

    const addFork = (xFrom: number, xTo: number, yA: number, yB: number, yOut: number) => {
      const xMid = Math.round((xFrom + xTo) / 2);
      list.push({ x1: xFrom, y1: yA, x2: xMid, y2: yA });
      list.push({ x1: xFrom, y1: yB, x2: xMid, y2: yB });
      list.push({ x1: xMid, y1: yA, x2: xMid, y2: yB });
      list.push({ x1: xMid, y1: yOut, x2: xTo, y2: yOut });
    };

    // Column width = 242px, gap = 32px (gap-8)
    // Round 1 (x: 0 -> 242) into Round 2 (x: 274 -> 516)
    addFork(242, 274, 77, 159, 118);
    addFork(242, 274, 241, 323, 282);
    addFork(242, 274, 405, 487, 446);
    addFork(242, 274, 569, 651, 610);

    if (elimRounds.length >= 3) {
      // Round 2 (x: 274 -> 516) into Round 3 (x: 548 -> 790)
      addFork(516, 548, 118, 282, 200);
      addFork(516, 548, 446, 610, 528);
    }

    if (elimRounds.length >= 4) {
      // Round 3 (x: 548 -> 790) into Round 4 (x: 822 -> 1064)
      addFork(790, 822, 200, 528, 364);
    }

    return list;
  });

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
    baganInfo={baganInfo}
    baganName={baganName}
    eventName={eventName}
    tatamiNumber={activeMatchForControl.assigned_tatami_id || 1}
    hostUrl={hostUrl}
    logoMap={logoMap}
    onBack={() => { activeMatchForControl = null; }}
    onSave={handleSaveMatchResult}
  />
{:else}
<div class="h-full flex flex-col p-4 select-none {isDarkMode ? 'bg-[#14161b] text-zinc-200' : 'bg-slate-100 text-slate-800'} overflow-hidden font-sans print-sheet">
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
        onclick={() => window.print()}
        class="px-3 py-1.5 {isDarkMode ? 'bg-white/[0.05] hover:bg-white/[0.08] text-zinc-200 border-white/[0.1]' : 'bg-white hover:bg-slate-50 text-slate-700 border-slate-300'} text-xs font-semibold border rounded-lg transition flex items-center space-x-1.5 shadow-xs active:scale-95"
        title="Cetak format kertas A4 Landscape"
      >
        <svg class="w-3.5 h-3.5 {isDarkMode ? 'text-zinc-400' : 'text-slate-500'}" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M17 17h2a2 2 0 002-2v-4a2 2 0 00-2-2H5a2 2 0 00-2 2v4a2 2 0 002 2h2m2 4h6a2 2 0 002-2v-4H7v4a2 2 0 002 2zm8-12V5a2 2 0 00-2-2H9a2 2 0 00-2 2v4h10z" />
        </svg>
        <span>Cetak (A4)</span>
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
      {:else}
        <!-- Edit Mode Active Actions -->
        <div class="flex items-center gap-1.5">
          <span class="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg bg-amber-500/20 text-amber-300 border border-amber-500/40 text-[11px] font-bold">
            <span class="w-2 h-2 rounded-full bg-amber-400 animate-pulse"></span>
            <span>Mode Edit</span>
          </span>
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
    <!-- Center Category & Tournament Info (True Center) -->
    <div class="banner-title-wrapper text-center w-full px-16">
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
      <p class="text-[11px] font-bold {isDarkMode ? 'text-zinc-200' : 'text-slate-800'} mt-0.5 flex items-center justify-center gap-1.5 flex-wrap">
        <span>{eventName || "AMBRILINDO KARATE CHAMPIONSHIP"}</span>
        <span>•</span>
        <span>{eventVenue}</span>
        {#if eventDate}
          <span>•</span>
          <span>{eventDate}</span>
        {/if}
      </p>
    </div>

    <!-- Right Match Counter -->
    <div class="banner-info-wrapper absolute right-0 top-1/2 -translate-y-1/2 flex justify-end items-center text-right text-[10px] {isDarkMode ? 'text-zinc-400 font-bold' : 'text-slate-600 font-bold'} font-mono no-print z-10">
      <span>{elimMatchesCount} Partai</span>
    </div>
  </div>

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
          <!-- Dynamic SVG Connector Canvas -->
          <svg
            bind:this={svgEl}
            class="bracket-svg absolute inset-0 pointer-events-none w-full h-full z-0 overflow-visible"
            viewBox="0 0 1064 692"
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
            {#each elimRounds as [roundNum, roundMatches]}
              {@const slotCount = getSlotCountForRound(roundNum)}
              {@const slotHeightClass = getSlotHeightClass(roundNum)}

              <div class="round-col round-col-elim w-[242px] shrink-0 flex flex-col relative">
                <!-- Column Header with Black/Dark Circle Step Badge (Centered) -->
                <div class="col-header-line h-8 shrink-0 flex items-center justify-center pb-1 border-b-2 {isDarkMode ? 'border-white/[0.25]' : 'border-slate-900'} mb-1">
                  <div class="flex items-center justify-center space-x-2 text-center w-full">
                    <span class="col-header-badge w-5 h-5 rounded-full {isDarkMode ? 'bg-zinc-800 border border-white/40 text-white' : 'bg-slate-900 text-white'} text-[10px] font-black font-mono flex items-center justify-center shrink-0">
                      {getRoundBadge(roundNum, elimRounds.length)}
                    </span>
                    <span class="col-header-title text-[12px] font-black uppercase tracking-tight text-center {isDarkMode ? 'text-white' : 'text-slate-950'}">
                      {getRoundTitle(roundNum, elimRounds.length)}
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
                          class="match-card w-full {isEditMode ? 'cursor-default ring-1 ring-amber-500/50' : 'cursor-pointer hover:ring-2 hover:ring-amber-500/80'} transition-all {isDarkMode ? 'bg-[#181a20] border-zinc-700 hover:border-zinc-500' : 'bg-white border-slate-400/90 hover:border-slate-800'} border rounded-md overflow-hidden shadow-xs relative z-10 {roundNum === 4 ? (isDarkMode ? 'border-amber-500/80 shadow-[0_0_15px_rgba(245,158,11,0.15)]' : 'border-amber-500 shadow-md') : ''}"
                          onclick={() => { if (!isEditMode) handleOpenMatchControl(match); }}
                          role="button"
                          tabindex="0"
                          onkeydown={(e) => { if (e.key === 'Enter' && !isEditMode) handleOpenMatchControl(match); }}
                          title={isEditMode ? "Mode Edit: Pilih atlet peserta partai" : "Klik untuk membuka Kontrol Pertandingan (Control Panel)"}
                        >
                          <!-- Match Card Header -->
                          <div class="card-header-bar flex items-center justify-between px-1.5 h-[13px] {isDarkMode ? 'bg-[#101215] border-white/[0.12] text-zinc-100' : 'bg-slate-200 border-slate-400 text-slate-950'} border-b text-[8px] font-black font-mono tracking-wider">
                            <span class="flex items-center space-x-1">
                              {#if roundNum === 4}
                                <span class="final-title {isDarkMode ? 'text-amber-300' : 'text-amber-800'} font-black flex items-center gap-1">
                                  <svg class="w-2.5 h-2.5 fill-current shrink-0" viewBox="0 0 24 24">
                                    <path d="M19 5h-2V3H7v2H5c-1.1 0-2 .9-2 2v1c0 2.55 1.92 4.63 4.39 4.94A5.01 5.01 0 0011 15.9V19H7v2h10v-2h-4v-3.1a5.01 5.01 0 003.61-2.96C19.08 12.63 21 10.55 21 8V7c0-1.1-.9-2-2-2zM5 8V7h2v3.82C5.84 10.4 5 9.3 5 8zm14 0c0 1.3-.84 2.4-2 2.82V7h2v1z"/>
                                  </svg>
                                  <span>{roundNum === 4 && poolCount === 2 && !isFinalBagan ? "SEMI FINAL" : "PARTAI FINAL"}</span>
                                </span>
                              {:else if roundNum === 3}
                                <span class="font-black">SEMI {match.urutan || slotNum}</span>
                              {:else}
                                <span class="font-black">MATCH {match.urutan || slotNum}</span>
                              {/if}
                            </span>
                            {#if isEditMode}
                              <span class="text-[7.5px] font-black text-amber-300 bg-amber-500/20 border border-amber-500/40 px-1 py-0.2 rounded tracking-wide">
                                EDIT
                              </span>
                            {:else if match.assigned_tatami_id}
                              <span class="text-[7.5px] font-black text-white bg-red-600 px-1 py-0.2 rounded tracking-wide">
                                LIVE T{match.assigned_tatami_id}
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
                                  {#if match.selesai && match.kata1 && match.kata1 !== '0 - Blank'}
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
                              <div class="shrink-0 flex items-center">
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
                                  {#if match.selesai && match.kata2 && match.kata2 !== '0 - Blank'}
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
                              <div class="shrink-0 flex items-center">
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
                  {#if roundNum === 4}
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
        </div>
      </div>
    {/if}
  </div>
</div>
{/if}

<style>
  @media print {
    @page {
      size: A4 landscape;
      margin: 4mm 6mm;
    }
    :global(html), :global(body) {
      background: #ffffff !important;
      background-color: #ffffff !important;
      color: #000000 !important;
      width: 285mm !important;
      max-width: 285mm !important;
      height: 198mm !important;
      max-height: 198mm !important;
      overflow: hidden !important;
      margin: 0 !important;
      padding: 0 !important;
      -webkit-print-color-adjust: exact !important;
      print-color-adjust: exact !important;
    }
    :global(.no-print) {
      display: none !important;
    }
    .print-sheet {
      background: #ffffff !important;
      background-color: #ffffff !important;
      color: #000000 !important;
      width: 285mm !important;
      max-width: 285mm !important;
      height: 198mm !important;
      max-height: 198mm !important;
      padding: 0 !important;
      margin: 0 !important;
      overflow: hidden !important;
      page-break-inside: avoid !important;
      break-inside: avoid !important;
      page-break-after: avoid !important;
      break-after: avoid !important;
    }
    .bracket-banner {
      position: relative !important;
      border-bottom: 1.5px solid #0f172a !important;
      padding-top: 0 !important;
      padding-bottom: 2px !important;
      margin-bottom: 3px !important;
      height: 38px !important;
      box-sizing: border-box !important;
      display: flex !important;
      align-items: center !important;
      justify-content: center !important;
      width: 1064px !important;
    }
    .banner-title-wrapper {
      width: 100% !important;
      text-align: center !important;
      padding-left: 16px !important;
      padding-right: 16px !important;
    }
    .bracket-banner h1 {
      font-size: 13px !important;
      line-height: 1.15 !important;
      color: #020617 !important;
      text-align: center !important;
    }
    .bracket-banner p,
    .bracket-banner span {
      font-size: 8.5px !important;
      color: #334155 !important;
      text-align: center !important;
    }
    .bracket-viewport {
      background: #ffffff !important;
      background-color: #ffffff !important;
      border: none !important;
      overflow: hidden !important;
      padding: 0 !important;
      margin: 0 !important;
      width: 285mm !important;
      max-width: 285mm !important;
      height: 184mm !important;
      max-height: 184mm !important;
    }
    .zoom-container {
      width: 1064px !important;
      height: 692px !important;
      min-width: 0 !important;
      min-height: 0 !important;
      background: #ffffff !important;
      overflow: hidden !important;
      display: block !important;
    }
    .bracket-outer {
      position: relative !important;
      transform: none !important;
      width: 1064px !important;
      height: 692px !important;
      background: #ffffff !important;
      overflow: hidden !important;
      page-break-inside: avoid !important;
      break-inside: avoid !important;
      page-break-after: avoid !important;
      break-after: avoid !important;
    }
    .bracket-svg {
      position: absolute !important;
      top: 0 !important;
      left: 0 !important;
      width: 100% !important;
      height: 100% !important;
      pointer-events: none !important;
      overflow: visible !important;
      display: block !important;
      z-index: 1 !important;
    }
    .bracket-grid {
      position: relative !important;
      background: transparent !important;
      z-index: 2 !important;
    }

    /* Print Column Headers */
    .col-header-badge {
      background: #000000 !important;
      background-color: #000000 !important;
      color: #ffffff !important;
      border: 1.5px solid #000000 !important;
      font-weight: 900 !important;
    }
    .col-header-title {
      color: #000000 !important;
      text-align: center !important;
      font-weight: 900 !important;
    }
    .col-header-line {
      height: 32px !important;
      padding-bottom: 4px !important;
      margin-bottom: 4px !important;
      border-bottom: 2px solid #000000 !important;
      justify-content: center !important;
      text-align: center !important;
    }

    /* Print Match Cards */
    .match-card {
      background: #ffffff !important;
      background-color: #ffffff !important;
      border: 1.8px solid #000000 !important;
      box-shadow: none !important;
    }
    .card-header-bar {
      background: #e2e8f0 !important;
      background-color: #e2e8f0 !important;
      border-bottom: 1.5px solid #000000 !important;
      color: #000000 !important;
      padding: 0 4px !important;
      height: 13px !important;
      min-height: 13px !important;
      line-height: 13px !important;
      font-size: 7.5px !important;
      font-weight: 900 !important;
    }
    .card-header-bar span {
      color: #000000 !important;
      font-weight: 900 !important;
    }
    .card-header-bar .final-title {
      color: #92400e !important;
      font-weight: 950 !important;
    }

    /* Player Rows */
    .player-row {
      background: #ffffff !important;
      background-color: #ffffff !important;
      border-color: #000000 !important;
      min-height: 17px !important;
      padding: 0.5px 3px !important;
    }
    .player-name-text {
      color: #000000 !important;
      font-weight: 900 !important;
      font-size: 8.5px !important;
      line-height: 10px !important;
    }
    .player-sub-text {
      color: #0f172a !important;
      font-weight: 800 !important;
      font-size: 7px !important;
      line-height: 8.5px !important;
      margin-top: 0.5px !important;
      display: block !important;
      white-space: nowrap !important;
      overflow: hidden !important;
      text-overflow: ellipsis !important;
    }
    .kata-row {
      margin-top: 0.5px !important;
      line-height: 1 !important;
    }
    .score-box {
      background: #f8fafc !important;
      background-color: #f8fafc !important;
      border: 1.5px solid #000000 !important;
      color: #000000 !important;
      font-weight: 900 !important;
      min-width: 18px !important;
      height: 15px !important;
      font-size: 8.5px !important;
      padding: 0 1px !important;
    }

    /* Winner Row Highlighting in Print */
    .player-row-winner {
      background: #fef3c7 !important;
      background-color: #fef3c7 !important;
      border-left: 5px solid #000000 !important;
    }
    .player-row-winner .player-name-text {
      color: #000000 !important;
      font-weight: 950 !important;
    }
    .player-row-winner .player-sub-text {
      color: #451a03 !important;
      font-weight: 800 !important;
    }
    .player-row-winner .score-box {
      background: #fde047 !important;
      background-color: #fde047 !important;
      border: 2px solid #000000 !important;
      color: #000000 !important;
      font-weight: 950 !important;
    }

    /* Side Pills in Print */
    .side-pill-aka {
      background: #b91c1c !important;
      background-color: #b91c1c !important;
      color: #ffffff !important;
      font-weight: 900 !important;
      font-size: 7px !important;
      width: 22px !important;
      min-width: 22px !important;
      max-width: 22px !important;
      display: inline-flex !important;
      align-items: center !important;
      justify-content: center !important;
      padding: 0.5px 0 !important;
      text-align: center !important;
    }
    .side-pill-ao {
      background: #1d4ed8 !important;
      background-color: #1d4ed8 !important;
      color: #ffffff !important;
      font-weight: 900 !important;
      font-size: 7px !important;
      width: 22px !important;
      min-width: 22px !important;
      max-width: 22px !important;
      display: inline-flex !important;
      align-items: center !important;
      justify-content: center !important;
      padding: 0.5px 0 !important;
      text-align: center !important;
    }

    /* Empty Slots */
    .empty-slot {
      background: #ffffff !important;
      background-color: #ffffff !important;
      border: 1.5px dashed #64748b !important;
      color: #334155 !important;
      font-weight: 800 !important;
    }

    /* Kata Badges in Print */
    .kata-badge-aka {
      background: #fee2e2 !important;
      background-color: #fee2e2 !important;
      color: #7f1d1d !important;
      border: 1px solid #991b1b !important;
      font-size: 6px !important;
      font-weight: 900 !important;
      line-height: 7.5px !important;
      padding: 0.5px 2px !important;
      margin-top: 0px !important;
      display: inline-flex !important;
      align-items: center !important;
      flex-shrink: 0 !important;
      max-width: 125px !important;
    }
    .kata-badge-ao {
      background: #dbeafe !important;
      background-color: #dbeafe !important;
      color: #1e3a8a !important;
      border: 1px solid #1e40af !important;
      font-size: 6px !important;
      font-weight: 900 !important;
      line-height: 7.5px !important;
      padding: 0.5px 2px !important;
      margin-top: 0px !important;
      display: inline-flex !important;
      align-items: center !important;
      flex-shrink: 0 !important;
      max-width: 125px !important;
    }

    /* Connector SVG Lines in Print */
    :global(svg line.connector-line) {
      stroke: #000000 !important;
      stroke-width: 1.6px !important;
    }

    /* Hasil Juara Card in Print */
    .podium-card {
      background: #ffffff !important;
      background-color: #ffffff !important;
      border: 2px solid #000000 !important;
      box-shadow: none !important;
    }
    .podium-header {
      background: #fef3c7 !important;
      background-color: #fef3c7 !important;
      border-bottom: 1.5px solid #000000 !important;
      color: #000000 !important;
      font-weight: 900 !important;
      padding: 2px 6px !important;
    }
    .podium-header span {
      color: #000000 !important;
      font-weight: 900 !important;
    }
    .podium-row {
      background: #ffffff !important;
      background-color: #ffffff !important;
      border-bottom: 1.2px solid #000000 !important;
      padding: 1.5px 6px !important;
    }
    .podium-name {
      color: #000000 !important;
      font-weight: 900 !important;
      font-size: 10px !important;
      line-height: 1.15 !important;
    }
    .podium-sub {
      color: #0f172a !important;
      font-weight: 800 !important;
      font-size: 8px !important;
      line-height: 1 !important;
    }

    /* Contingent Dojo Logos in Print */
    .player-dojo-logo {
      width: 12px !important;
      height: 12px !important;
      max-width: 12px !important;
      max-height: 12px !important;
      object-fit: contain !important;
      flex-shrink: 0 !important;
      display: inline-block !important;
      border: none !important;
    }
    .podium-dojo-logo {
      width: 14px !important;
      height: 14px !important;
      max-width: 14px !important;
      max-height: 14px !important;
      object-fit: contain !important;
      flex-shrink: 0 !important;
      display: inline-block !important;
      border: none !important;
    }
    .podium-brand-logo {
      height: 38px !important;
      max-height: 42px !important;
      max-width: 180px !important;
      object-fit: contain !important;
      display: block !important;
      margin: 0 0 6px auto !important;
    }
    .seed-star-icon svg {
      width: 9.5px !important;
      height: 9.5px !important;
      fill: #d97706 !important;
      color: #d97706 !important;
    }
    .logo-star-wrapper.has-priority {
      margin-right: 3px !important;
    }
  }

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
