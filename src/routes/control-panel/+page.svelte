<script lang="ts">
  import { onMount } from "svelte";
  import { page } from "$app/state";
  import { invoke } from "@tauri-apps/api/core";
  import { emit } from "@tauri-apps/api/event";
  import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
  import type { DetailBaganItem, BaganItem, SaveMatchInput } from "$lib/types";
  import ControlPanelView from "$lib/components/ControlPanelView.svelte";

  let matchId = $derived(Number(page.url.searchParams.get("matchId") || "0"));
  let baganId = $derived(Number(page.url.searchParams.get("baganId") || "0"));

  let currentMatch = $state<DetailBaganItem | null>(null);
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

    try {
      if (!matchId || !baganId) {
        throw new Error("Parameter matchId atau baganId tidak ditemukan di URL.");
      }

      // 1. Fetch all matches in this bagan
      const matches: DetailBaganItem[] = await invoke("get_detail_bagan", { baganId });
      const found = matches.find((m) => m.id === matchId);
      if (!found) {
        throw new Error(`Partai dengan ID ${matchId} tidak ditemukan dalam bagan #${baganId}.`);
      }
      currentMatch = found;
      tatamiNumber = found.assigned_tatami_id || 1;

      // 2. Fetch bagan info directly by baganId
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

      // 3. Resolve logos if present
      const map: Record<string, string> = {};
      for (const logoKey of [found.atlet1_logo, found.atlet2_logo]) {
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
      errorMessage = err?.message || String(err);
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

  async function handleSaveMatchResult(input: SaveMatchInput) {
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
      // Close this window after successful save
      await handleCloseWindow();
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
      onBack={handleCloseWindow}
      onSave={handleSaveMatchResult}
    />
  {/if}
</div>
