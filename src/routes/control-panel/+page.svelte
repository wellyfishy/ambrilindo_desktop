<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { invoke, setConnectionConfig } from "$lib/api_client";
  import { emit } from "@tauri-apps/api/event";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import type { DetailBaganItem, BaganItem, SaveMatchInput, TeamMatchInfo, AssignBoutAthletesInput } from "$lib/types";
  import ControlPanelView from "$lib/components/ControlPanelView.svelte";

  let matchId = $derived(Number(page.url.searchParams.get("matchId") || "0"));
  let baganId = $derived(Number(page.url.searchParams.get("baganId") || "0"));
  let isFestival = $derived(
    page.url.searchParams.get("isFestival") === "true" ||
    (matchId === 0 && baganId === 0)
  );

  let currentMatch = $state<DetailBaganItem | null>(null);
  let allMatchesInBagan = $state<DetailBaganItem[]>([]);
  let teamMatchInfo = $state<TeamMatchInfo | null>(null);
  let baganInfo = $state<BaganItem | null>(null);
  let baganName = $state<string>("");
  let eventName = $state<string>("AMBRILINDO OPEN KARATE CHAMPIONSHIP");
  let tatamiNumber = $state<number>(1);
  let logoMap = $state<Record<string, string>>({});
  let isLoading = $state<boolean>(true);
  let errorMessage = $state<string | null>(null);

  async function loadMatchData() {
    isLoading = true;
    errorMessage = null;

    const qHost = page.url.searchParams.get("hostUrl") || page.url.searchParams.get("host");
    if (qHost) {
      setConnectionConfig("client", qHost);
    }

    const urlTatami = parseInt(page.url.searchParams.get("tatami") || "1", 10);
    tatamiNumber = !isNaN(urlTatami) && urlTatami > 0 ? urlTatami : 1;

    // Handle stateless Festival match (no database required)
    if (isFestival) {
      const qAka = page.url.searchParams.get("aka") || "Aka Contestant";
      const qAkaClub = page.url.searchParams.get("aka_club") || "None";
      const qAo = page.url.searchParams.get("ao") || "Ao Contestant";
      const qAoClub = page.url.searchParams.get("ao_club") || "None";
      const qCat = page.url.searchParams.get("cat") || "FESTIVAL";
      const qDisc = (page.url.searchParams.get("disc") || "kumite").toLowerCase();

      currentMatch = {
        id: 0,
        bagan_id: 0,
        round: 1,
        urutan: 1,
        atlet1_id: null,
        atlet2_id: null,
        score1: "0",
        score2: "0",
        scorekecil1: null,
        scorekecil2: null,
        vr1: false,
        vr2: false,
        kata1: null,
        kata2: null,
        pemenang: null,
        hantei: false,
        selesai: false,
        team: false,
        assigned_tatami_id: tatamiNumber,
        kode: "000",
        atlet1_nama: qAka,
        atlet1_kontingen: qAkaClub,
        atlet2_nama: qAo,
        atlet2_kontingen: qAoClub,
      };
      baganName = qCat;
      baganInfo = {
        id: 0,
        event_id: 1,
        nama_bagan: qCat,
        nomor_tanding_id: null,
        tipe_tanding: qDisc === "kata" ? "1" : "2",
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
        kode: "000",
        is_bob: false,
        has_vr: false,
      };
      isLoading = false;
      return;
    }

    try {
      if (!matchId || !baganId) {
        throw new Error("Parameter matchId atau baganId tidak ditemukan di URL.");
      }

      // 1. Fetch all matches in this bagan
      const matches: DetailBaganItem[] = await invoke("get_detail_bagan", { baganId });
      allMatchesInBagan = matches;

      // 2. Fetch team match info if this is Kumite Beregu
      try {
        const tInfo: TeamMatchInfo = await invoke("get_team_match_info", { matchId });
        if (tInfo && tInfo.is_team_match) {
          teamMatchInfo = tInfo;
          // Determine target match: active bout or currently focused bout
          let targetBoutId = tInfo.active_bout_id;
          if (currentMatch && tInfo.bouts.some((b) => b.bout_id === currentMatch?.id)) {
            targetBoutId = currentMatch.id;
          }
          const foundBout = matches.find((m) => m.id === targetBoutId);
          if (foundBout) {
            currentMatch = foundBout;
            tatamiNumber = foundBout.assigned_tatami_id || 1;
          } else {
            const foundParent = matches.find((m) => m.id === matchId);
            if (foundParent) {
              currentMatch = foundParent;
              tatamiNumber = foundParent.assigned_tatami_id || 1;
            }
          }
        } else {
          teamMatchInfo = null;
          const found = matches.find((m) => m.id === matchId);
          if (!found) {
            throw new Error(`Partai dengan ID ${matchId} tidak ditemukan dalam bagan #${baganId}.`);
          }
          currentMatch = found;
          tatamiNumber = found.assigned_tatami_id || 1;
        }
      } catch (e) {
        console.warn("get_team_match_info check:", e);
        const found = matches.find((m) => m.id === matchId);
        if (!found) {
          throw new Error(`Partai dengan ID ${matchId} tidak ditemukan dalam bagan #${baganId}.`);
        }
        currentMatch = found;
        tatamiNumber = found.assigned_tatami_id || 1;
      }

      // 3. Fetch bagan info directly by baganId
      try {
        const b: BaganItem | null = await invoke("get_bagan_by_id", { baganId });
        if (b) {
          baganInfo = b;
          baganName = b.nama_bagan || "";
        }
      } catch (e) {
        console.warn("Gagal memuat info bagan by ID, fallback get_bagans:", e);
        try {
          const bagans: BaganItem[] = await invoke("get_bagans", { eventId: 1 });
          const b = bagans.find((x) => x.id === baganId);
          if (b) {
            baganInfo = b;
            baganName = b.nama_bagan || "";
          }
        } catch (_) {}
      }

      // 4. Resolve logos if present
      const map: Record<string, string> = {};
      const logoCandidates = [
        currentMatch?.atlet1_logo,
        currentMatch?.atlet2_logo,
        teamMatchInfo?.team_aka_logo,
        teamMatchInfo?.team_ao_logo,
      ];
      for (const logoKey of logoCandidates) {
        if (logoKey && !map[logoKey]) {
          try {
            const dataUrl: string | null = await invoke("get_media_data_url", {
              relativePath: logoKey,
            });
            if (dataUrl) map[logoKey] = dataUrl;
          } catch (e) {
            console.warn("Gagal memuat logo:", logoKey, e);
          }
        }
      }
      logoMap = map;
    } catch (err: any) {
      console.error("Gagal memuat detail partai:", err);
      // Fallback for browser dev/test mode when Tauri internals are not available
      if (typeof window !== "undefined" && !(window as any).__TAURI_INTERNALS__) {
        console.warn("Menggunakan fallback dummy data untuk browser / test mode");
        currentMatch = {
          id: matchId || 1,
          bagan_id: baganId || 1,
          round: 1,
          urutan: 1,
          atlet1_id: 101,
          atlet2_id: 102,
          score1: "0",
          score2: "0",
          scorekecil1: null,
          scorekecil2: null,
          vr1: false,
          vr2: false,
          kata1: "0 - Blank",
          kata2: "0 - Blank",
          pemenang: null,
          hantei: false,
          selesai: false,
          team: false,
          assigned_tatami_id: 1,
          kode: "KAT-SEN-01",
          atlet1_nama: "Ahmad Fauzi",
          atlet1_kontingen: "INKAI JAWA BARAT",
          atlet2_nama: "Budi Santoso",
          atlet2_kontingen: "LEMKARI DKI JAKARTA",
        };
        baganName = "KATA PERORANGAN SENIOR PUTRA";
        baganInfo = {
          id: baganId || 1,
          event_id: 1,
          nama_bagan: "KATA PERORANGAN SENIOR PUTRA",
          nomor_tanding_id: 1,
          tipe_tanding: "1", // 1 is Kata
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
          kode: "KAT-SEN-PUTRA",
          is_bob: false,
          has_vr: false,
        };
        tatamiNumber = 1;
        errorMessage = null;
      } else {
        errorMessage = err?.message || String(err);
      }
    } finally {
      isLoading = false;
    }
  }

  async function handleCloseWindow() {
    try {
      await invoke("close_control_panel_window", { matchId });
    } catch (_) {}
    try {
      const appWindow = getCurrentWebviewWindow();
      await appWindow.close();
    } catch (e) {
      console.warn("Tauri getCurrentWebviewWindow failed, fallback window.close():", e);
      window.close();
    }
  }

  async function handleSwitchBout(boutId: number) {
    const target = allMatchesInBagan.find((m) => m.id === boutId);
    if (target) {
      currentMatch = target;
      tatamiNumber = target.assigned_tatami_id || tatamiNumber;
    } else {
      try {
        const matches: DetailBaganItem[] = await invoke("get_detail_bagan", { baganId });
        allMatchesInBagan = matches;
        const found = matches.find((m) => m.id === boutId);
        if (found) {
          currentMatch = found;
          tatamiNumber = found.assigned_tatami_id || tatamiNumber;
        }
      } catch (_) {}
    }

    if (teamMatchInfo) {
      try {
        const tInfo: TeamMatchInfo = await invoke("get_team_match_info", {
          matchId: teamMatchInfo.parent_match_id,
        });
        teamMatchInfo = tInfo;
      } catch (_) {}
    }
  }

  async function handleAddBout() {
    if (!teamMatchInfo) return;
    try {
      await invoke("add_team_bout", { parentMatchId: teamMatchInfo.parent_match_id });
      await loadMatchData();
    } catch (err: any) {
      alert("Gagal menambah partai: " + (err?.message || err));
    }
  }

  async function handleDeleteBout(boutId: number) {
    if (!confirm("Hapus partai beregu ini?")) return;
    try {
      await invoke("delete_team_bout", { boutId });
      await loadMatchData();
    } catch (err: any) {
      alert("Gagal menghapus partai: " + (err?.message || err));
    }
  }

  async function handleAssignBoutAthletes(input: AssignBoutAthletesInput) {
    try {
      await invoke("assign_bout_athletes", { input });
      await loadMatchData();
    } catch (err: any) {
      alert("Gagal menyimpan atlet partai: " + (err?.message || err));
    }
  }

  async function handleFinishTeamMatch(winnerOverride?: string) {
    if (!teamMatchInfo) return;
    try {
      await invoke("finish_team_match", {
        parentMatchId: teamMatchInfo.parent_match_id,
        winnerOverride: winnerOverride || null,
      });
      try {
        await emit("bracket_updated", {
          matchId: teamMatchInfo.parent_match_id,
          baganId,
        });
      } catch (_) {}
      alert("Pertandingan Beregu Selesai! Skor akhir dan bagan turnamen telah diperbarui.");
      await handleCloseWindow();
    } catch (err: any) {
      alert("Gagal menyelesaikan pertandingan beregu: " + (err?.message || err));
    }
  }

  async function handleSaveMatchResult(input: SaveMatchInput) {
    if (isFestival) {
      // Festival matches are stateless (not saved to database)
      await handleCloseWindow();
      return;
    }
    try {
      await invoke("save_match_result", { input });
      // Notify parent bracket window to reload data
      try {
        await emit("bracket_updated", {
          matchId: input.match_id,
          baganId: input.bagan_id,
        });
      } catch (emitErr) {
        console.warn("Emit bracket_updated failed:", emitErr);
      }
      if (teamMatchInfo?.is_team_match) {
        // If team match, reload state to update team score and active bout
        await loadMatchData();
      } else {
        // Close this window after successful save for standard individual match
        await handleCloseWindow();
      }
    } catch (err: any) {
      alert("Gagal menyimpan hasil partai: " + (err?.message || err));
    }
  }

  onMount(() => {
    loadMatchData();
  });
</script>

<div class="w-screen h-screen bg-[#0d0f12] overflow-hidden">
  {#if isLoading}
    <div class="w-full h-full flex flex-col items-center justify-center text-zinc-400 space-y-3 font-mono">
      <div class="w-8 h-8 border-2 border-indigo-500 border-t-transparent rounded-full animate-spin"></div>
      <span class="text-xs">Memuat Cockpit Control Panel Partai #{matchId}...</span>
    </div>
  {:else if errorMessage || !currentMatch}
    <div class="w-full h-full flex flex-col items-center justify-center text-red-400 p-6 text-center">
      <div class="w-12 h-12 rounded-full bg-red-500/10 border border-red-500/30 flex items-center justify-center mb-3">
        <svg class="w-6 h-6 text-red-400" fill="none" stroke="currentColor" viewBox="0 0 24 24">
          <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
        </svg>
      </div>
      <h2 class="text-base font-bold text-white mb-1">Gagal Membuka Control Panel</h2>
      <p class="text-xs text-zinc-400 max-w-md mb-4">{errorMessage || "Partai tidak ditemukan."}</p>
      <button
        type="button"
        onclick={handleCloseWindow}
        class="px-4 py-2 bg-[#1e2029] hover:bg-[#2b2d31] border border-white/10 text-white text-xs font-bold rounded-lg transition"
      >
        Tutup Jendela
      </button>
    </div>
  {:else}
    <ControlPanelView
      match={currentMatch}
      baganInfo={baganInfo}
      baganName={baganName}
      eventName={eventName}
      tatamiNumber={tatamiNumber}
      logoMap={logoMap}
      teamMatchInfo={teamMatchInfo}
      onBack={handleCloseWindow}
      onSave={handleSaveMatchResult}
      onRefreshTeam={loadMatchData}
      onSwitchBout={handleSwitchBout}
      onAddBout={handleAddBout}
      onDeleteBout={handleDeleteBout}
      onAssignBoutAthletes={handleAssignBoutAthletes}
      onFinishTeamMatch={handleFinishTeamMatch}
    />
  {/if}
</div>
